//! Fast child-exit trigger — the SIGCHLD abstraction the dispatcher's reaper
//! waits on. On unix the kernel signals a child's exit, letting the dispatcher
//! flip a run to `completed` within milliseconds; no other target has an
//! equivalent, so there the watcher simply never fires and the periodic
//! reap-safety-net ticker is the sole reaper. Selecting the impl here is what
//! keeps the dispatcher branch-free — the `#[cfg]` lives at this boundary, not
//! in the caller.

/// Resolves once each time a child process exits. Construct with
/// [`ChildExitWatcher::new`] and `await` [`recv`](Self::recv) in a select loop;
/// on targets without an OS exit signal it never resolves, so the caller must
/// pair it with a periodic reap.
#[cfg(unix)]
pub struct ChildExitWatcher(Option<tokio::signal::unix::Signal>);

#[cfg(unix)]
impl Default for ChildExitWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
impl ChildExitWatcher {
    pub fn new() -> Self {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::child()) {
            Ok(s) => Self(Some(s)),
            Err(e) => {
                tracing::warn!("[platform] SIGCHLD listener init: {e}; ticker-only reaps");
                Self(None)
            }
        }
    }

    pub async fn recv(&mut self) {
        match self.0.as_mut() {
            Some(s) => {
                s.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
    }
}

#[cfg(not(unix))]
#[derive(Default)]
pub struct ChildExitWatcher;

#[cfg(not(unix))]
impl ChildExitWatcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn recv(&mut self) {
        std::future::pending::<()>().await
    }
}
