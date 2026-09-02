use std::net::SocketAddr;
use std::path::PathBuf;

use modula_engine_transport::{EngineEndpoint, LocalIpcEndpoint, LocalListener};

use modula_plugin::PluginRegistry;

use modula_grpc as grpc;
use modula_services::dispatcher::Dispatcher;
use modula_state::AppState;

/// How the `modula engine` subcommand should bind its gRPC server.
pub struct ServeOptions {
    /// The plugins this binary composed.
    pub registry: PluginRegistry,
    /// Override the IPC socket/pipe path (`--socket`). Falls back to
    /// `MODULA_ENGINE_SOCKET` then the default per-user runtime path.
    pub socket: Option<PathBuf>,
    /// DEV ONLY, INSECURE: also serve gRPC over loopback TCP at this address.
    /// No auth/TLS; never for production. Bypasses the IPC transport entirely.
    pub grpc_tcp: Option<SocketAddr>,
    /// Required for a non-loopback `--grpc-tcp` address; without it a
    /// non-loopback bind is refused.
    pub grpc_tcp_allow_remote: bool,
}

pub async fn serve(opts: ServeOptions) -> anyhow::Result<()> {
    let registry = opts.registry.clone();
    let endpoint = EngineEndpoint::LocalIpc(LocalIpcEndpoint::resolve(opts.socket)?);

    if let Some(addr) = opts.grpc_tcp {
        if !addr.ip().is_loopback() && !opts.grpc_tcp_allow_remote {
            anyhow::bail!(
                "refusing to bind --grpc-tcp to non-loopback {addr} without --grpc-tcp-allow-remote"
            );
        }
        write_pidfile();
        let state = start_state(endpoint, &registry).await?;
        tracing::warn!("INSECURE dev gRPC over TCP on {addr} — no auth/TLS, local dev only");
        grpc::make_router(state, &registry)
            .serve_with_shutdown(addr, shutdown_signal())
            .await?;
        return Ok(());
    }

    let ipc = endpoint
        .as_local_ipc()
        .expect("default endpoint is local IPC");
    // Bind first: this runs the stale-endpoint probe and refuses to clobber a
    // live engine *before* the pidfile is touched or any DB/scheduler is opened.
    let listener = LocalListener::bind(ipc).await?;
    write_pidfile();
    let result = serve_ipc(endpoint.clone(), listener, &registry).await;
    cleanup_endpoint(ipc);
    result
}

async fn serve_ipc(
    endpoint: EngineEndpoint,
    listener: LocalListener,
    registry: &PluginRegistry,
) -> anyhow::Result<()> {
    let state = start_state(endpoint.clone(), registry).await?;
    tracing::info!("engine listening on {endpoint}");
    grpc::make_router(state, registry)
        .serve_with_incoming_shutdown(listener.incoming(), shutdown_signal())
        .await?;
    Ok(())
}

/// Build `AppState` and start the event-driven dispatcher (services-direct; no
/// self-RPC). Shared by the IPC and dev-TCP paths.
async fn start_state(
    endpoint: EngineEndpoint,
    registry: &PluginRegistry,
) -> anyhow::Result<AppState> {
    let state = AppState::new(endpoint, registry).await?;
    Dispatcher::new(
        state.repos.clone(),
        state.workspaces.clone(),
        state.loops.clone(),
        state.engine_socket.clone(),
        std::sync::Arc::new(state.events.clone()),
    )
    .spawn();
    // `serve_ipc` binds the listener before this runs, so a plugin that calls
    // back into the engine over IPC can never race the socket's existence.
    for service in registry.services() {
        if let Err(e) = service.start().await {
            tracing::warn!("[plugin] a service failed to start: {e}");
        }
    }
    Ok(state)
}

/// Resolves when the process receives a termination signal: SIGTERM/SIGINT on
/// Unix, a console-ctrl event on Windows. The desktop stops the engine by pid
/// (signal), so this — not only a clean internal exit — must drive the
/// socket/pipe cleanup that follows `serve`.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
        tokio::select! {
            _ = term.recv() => tracing::info!("received SIGTERM; shutting down"),
            _ = int.recv() => tracing::info!("received SIGINT; shutting down"),
        }
    }
    #[cfg(windows)]
    {
        use std::sync::Arc;

        use tokio::sync::Notify;

        let notify = Arc::new(Notify::new());
        let cb = Arc::clone(&notify);
        if let Err(e) = modula_platform::ctrl_handler::set_ctrl_handler(move || cb.notify_one()) {
            tracing::warn!("console-ctrl handler unavailable ({e}); using ctrl_c fallback");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
        notify.notified().await;
        tracing::info!("received console-ctrl event; shutting down");
    }
}

/// Remove the Unix socket file after the server stops so the next start is
/// clean. On Windows the named pipe is released automatically on process exit.
#[cfg(unix)]
fn cleanup_endpoint(ipc: &LocalIpcEndpoint) {
    if let Err(e) = std::fs::remove_file(ipc.path()) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("could not remove socket {}: {e}", ipc.path().display());
        }
    }
}

#[cfg(windows)]
fn cleanup_endpoint(_ipc: &LocalIpcEndpoint) {}

/// Record this process's pid so the desktop shell can stop the engine on quit.
/// Best-effort — a write failure just leaves the engine running.
fn write_pidfile() {
    let Some(path) = modula_platform::engine_pid_file() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, std::process::id().to_string()) {
        tracing::warn!("could not write pidfile {}: {e}", path.display());
    }
}
