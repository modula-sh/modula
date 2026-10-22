//! PTY host for terminal-mode conversations: one running provider CLI per
//! conversation id, spawned from the engine's `TerminalLaunch` spec. Output is
//! kept in a bounded ring so a view that navigates away and back can replay it.
//! A child outlives its view; it dies on exit or app quit, and the next open
//! relaunches it, which resumes the stored session.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use modula_client::ModulaClient;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

const RING_CAP: usize = 1 << 20;

type PtyMap = HashMap<String, Arc<Pty>>;

#[derive(Default)]
pub struct PtyHost(Arc<tokio::sync::Mutex<PtyMap>>);

struct Pty {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    // Ring and sinks share a lock so a reattach neither drops nor repeats bytes.
    out: Mutex<Output>,
}

struct Output {
    ring: VecDeque<u8>,
    on_data: Channel<InvokeResponseBody>,
    on_exit: Channel<u32>,
}

fn push_capped(ring: &mut VecDeque<u8>, chunk: &[u8], cap: usize) {
    ring.extend(chunk);
    let over = ring.len().saturating_sub(cap);
    ring.drain(..over);
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

async fn get(host: &PtyHost, id: &str) -> Result<Arc<Pty>, String> {
    host.0
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| format!("no terminal for conversation {id}"))
}

/// Attach to the conversation's PTY, replaying its buffered output, or launch
/// it. The map stays locked across launch so two concurrent opens (StrictMode)
/// can't start two sessions.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn pty_open(
    engine: State<'_, ModulaClient>,
    host: State<'_, PtyHost>,
    workspace_id: String,
    conversation_id: String,
    prompt: Option<String>,
    cols: u16,
    rows: u16,
    on_data: Channel<InvokeResponseBody>,
    on_exit: Channel<u32>,
) -> Result<(), String> {
    let mut map = host.0.lock().await;
    if let Some(pty) = map.get(&conversation_id) {
        let mut out = pty.out.lock().unwrap();
        if !out.ring.is_empty() {
            let snapshot = out.ring.iter().copied().collect();
            let _ = on_data.send(InvokeResponseBody::Raw(snapshot));
        }
        out.on_data = on_data;
        out.on_exit = on_exit;
        drop(out);
        let _ = pty.master.lock().unwrap().resize(size(cols, rows));
        return Ok(());
    }

    let spec = engine
        .terminal_launch(&workspace_id, &conversation_id, prompt)
        .await?;
    let pair = native_pty_system()
        .openpty(size(cols, rows))
        .map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::from_argv(spec.argv.into_iter().map(Into::into).collect());
    cmd.cwd(spec.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Carries the engine's PATH: the desktop process never enriches its own.
    for (k, v) in spec.env {
        cmd.env(k, v);
    }
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    // Drop our slave handle so the reader sees EOF once the child exits.
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let pty = Arc::new(Pty {
        killer: Mutex::new(child.clone_killer()),
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        out: Mutex::new(Output {
            ring: VecDeque::new(),
            on_data,
            on_exit,
        }),
    });
    map.insert(conversation_id.clone(), pty.clone());

    let map = host.0.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        // macOS reports a closed PTY as EIO rather than EOF.
        while let Ok(n @ 1..) = reader.read(&mut buf) {
            let mut out = pty.out.lock().unwrap();
            push_capped(&mut out.ring, &buf[..n], RING_CAP);
            let _ = out.on_data.send(InvokeResponseBody::Raw(buf[..n].to_vec()));
        }
        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
        let mut map = map.blocking_lock();
        if map
            .get(&conversation_id)
            .is_some_and(|p| Arc::ptr_eq(p, &pty))
        {
            map.remove(&conversation_id);
        }
        drop(map);
        let _ = pty.out.lock().unwrap().on_exit.send(code);
    });
    Ok(())
}

#[tauri::command]
pub async fn pty_write(
    host: State<'_, PtyHost>,
    conversation_id: String,
    data: String,
) -> Result<(), String> {
    let pty = get(&host, &conversation_id).await?;
    let mut writer = pty.writer.lock().unwrap();
    writer.write_all(data.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pty_resize(
    host: State<'_, PtyHost>,
    conversation_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let pty = get(&host, &conversation_id).await?;
    let result = pty.master.lock().unwrap().resize(size(cols, rows));
    result.map_err(|e| e.to_string())
}

/// Kill every child on app quit. Process exit then closes the masters, which
/// hangs up each session's process group.
pub fn kill_all(host: &PtyHost) {
    for pty in host.0.blocking_lock().drain().map(|(_, p)| p) {
        let _ = pty.killer.lock().unwrap().kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_newest_bytes_within_cap() {
        let mut ring = VecDeque::new();
        push_capped(&mut ring, b"abcd", 6);
        push_capped(&mut ring, b"efgh", 6);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), b"cdefgh");
        push_capped(&mut ring, b"0123456789", 6);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), b"456789");
    }
}
