//! PTY host for terminal-mode conversations: one running provider CLI per
//! conversation, shared by every client that opens it (the desktop and paired
//! phones). Output is kept in a bounded ring so a client that attaches late
//! replays it. A child outlives its viewers; it dies on exit or engine shutdown,
//! and the next open relaunches it, which resumes the stored session.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use modula_core::error::{ApiError, ApiResult};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;

use crate::conversations::{terminal_launch, ConvKey, ConvRuntime};

const RING_CAP: usize = 1 << 20;
/// Replay goes out in pieces so one attach never sends a single huge message.
const REPLAY_CHUNK: usize = 64 * 1024;
/// A viewer this many reads behind is dropped rather than buffered without
/// bound; its stream ends and it can reattach.
const SUBSCRIBER_BACKLOG: usize = 1024;

#[derive(Debug, Clone)]
pub enum TermEvent {
    Data(Vec<u8>),
    Exit(u32),
}

type PtyMap = HashMap<ConvKey, Arc<Pty>>;

#[derive(Clone, Default)]
pub struct TerminalHost(Arc<tokio::sync::Mutex<PtyMap>>);

struct Pty {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    // Ring and subscribers share a lock so an attach neither drops nor repeats bytes.
    out: Mutex<Output>,
}

#[derive(Default)]
struct Output {
    ring: VecDeque<u8>,
    subs: Vec<mpsc::Sender<TermEvent>>,
}

impl Output {
    fn subscribe(&mut self) -> mpsc::Receiver<TermEvent> {
        let (tx, rx) = mpsc::channel(SUBSCRIBER_BACKLOG);
        let replay: Vec<u8> = self.ring.iter().copied().collect();
        for chunk in replay.chunks(REPLAY_CHUNK) {
            let _ = tx.try_send(TermEvent::Data(chunk.to_vec()));
        }
        self.subs.push(tx);
        rx
    }

    fn broadcast(&mut self, event: TermEvent) {
        self.subs.retain(|tx| tx.try_send(event.clone()).is_ok());
    }
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

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

impl TerminalHost {
    async fn get(&self, key: &ConvKey) -> ApiResult<Arc<Pty>> {
        self.0
            .lock()
            .await
            .get(key)
            .cloned()
            .ok_or_else(|| ApiError::NotFound(format!("no terminal for conversation {}", key.1)))
    }

    pub async fn write(&self, ws: &str, conv: &str, data: &[u8]) -> ApiResult<()> {
        let pty = self.get(&(ws.to_string(), conv.to_string())).await?;
        let mut writer = pty.writer.lock().unwrap();
        writer.write_all(data).map_err(internal)
    }

    pub async fn resize(&self, ws: &str, conv: &str, cols: u16, rows: u16) -> ApiResult<()> {
        let pty = self.get(&(ws.to_string(), conv.to_string())).await?;
        let result = pty.master.lock().unwrap().resize(size(cols, rows));
        result.map_err(internal)
    }

    /// Kill every child on engine shutdown. Process exit then closes the
    /// masters, which hangs up each session's process group.
    pub async fn kill_all(&self) {
        for pty in self.0.lock().await.drain().map(|(_, p)| p) {
            let _ = pty.killer.lock().unwrap().kill();
        }
    }
}

/// Attach to the conversation's PTY, replaying its buffered output, or launch
/// it. The map stays locked across launch so two concurrent opens can't start
/// two sessions.
pub async fn open(
    rt: &ConvRuntime,
    ws: &str,
    conv: &str,
    prompt: Option<&str>,
    cols: u16,
    rows: u16,
) -> ApiResult<mpsc::Receiver<TermEvent>> {
    let key: ConvKey = (ws.to_string(), conv.to_string());
    let mut map = rt.terminals.0.lock().await;
    if let Some(pty) = map.get(&key) {
        let rx = pty.out.lock().unwrap().subscribe();
        let _ = pty.master.lock().unwrap().resize(size(cols, rows));
        return Ok(rx);
    }

    let spec = terminal_launch(rt, ws, conv, prompt).await?;
    let pair = native_pty_system()
        .openpty(size(cols, rows))
        .map_err(internal)?;
    let mut cmd = CommandBuilder::from_argv(spec.argv.into_iter().map(Into::into).collect());
    cmd.cwd(spec.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    for (k, v) in spec.env {
        cmd.env(k, v);
    }
    let mut child = pair.slave.spawn_command(cmd).map_err(internal)?;
    // Drop our slave handle so the reader sees EOF once the child exits.
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(internal)?;
    let writer = pair.master.take_writer().map_err(internal)?;

    let mut out = Output::default();
    let rx = out.subscribe();
    let pty = Arc::new(Pty {
        killer: Mutex::new(child.clone_killer()),
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        out: Mutex::new(out),
    });
    map.insert(key.clone(), pty.clone());

    let map = rt.terminals.0.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        // macOS reports a closed PTY as EIO rather than EOF.
        while let Ok(n @ 1..) = reader.read(&mut buf) {
            let mut out = pty.out.lock().unwrap();
            push_capped(&mut out.ring, &buf[..n], RING_CAP);
            out.broadcast(TermEvent::Data(buf[..n].to_vec()));
        }
        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
        let mut map = map.blocking_lock();
        if map.get(&key).is_some_and(|p| Arc::ptr_eq(p, &pty)) {
            map.remove(&key);
        }
        drop(map);
        pty.out.lock().unwrap().broadcast(TermEvent::Exit(code));
    });
    Ok(rx)
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
