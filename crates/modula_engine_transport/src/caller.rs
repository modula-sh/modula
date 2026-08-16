/// Identity of a caller, established at IPC connection accept time.
///
/// This is the single chokepoint for caller identity: the listener verifies the
/// peer at accept time, produces a `LocalCaller`, and the gRPC runtime makes it
/// available to every handler via `Request::extensions()`. Handlers and services
/// extract identity from here; they never call OS credential APIs directly.
///
/// On Unix: the peer UID is checked against the engine UID via `SO_PEERCRED` /
/// `getpeereid`; connections from other UIDs are rejected before `LocalCaller`
/// is constructed. On Windows: the pipe DACL enforces same-user access at bind
/// time, so `LocalCaller` is a marker type with no per-connection UID check.
#[derive(Clone, Debug)]
pub struct LocalCaller {
    #[cfg(unix)]
    pub(crate) uid: u32,
}

impl LocalCaller {
    #[cfg(unix)]
    pub(crate) fn new(uid: u32) -> Self {
        Self { uid }
    }

    #[cfg(windows)]
    pub(crate) fn new() -> Self {
        Self {}
    }

    /// The peer UID (Unix only). On Windows the DACL enforces same-user access
    /// without per-connection creds, so this always returns `None` there.
    pub fn uid(&self) -> Option<u32> {
        #[cfg(unix)]
        {
            Some(self.uid)
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}
