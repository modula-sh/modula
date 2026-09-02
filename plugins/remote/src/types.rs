//! Mirrors the real plugin's types exactly: the desktop deserializes these
//! in the webview, so the shape must not drift.

use serde::{Deserialize, Serialize};

/// The remote host's live state. `password_hash` is deliberately absent: the
/// hash never leaves the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteStatus {
    pub enabled: bool,
    pub running: bool,
    pub password_set: bool,
    pub node_id: String,
    pub direct_addresses: Vec<String>,
    pub connected_devices: u32,
    pub last_error: String,
}

/// A device paired with this host. `connected` is live endpoint state, not a
/// stored column — repositories leave it `false` and the service overlays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub scope: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub connected: bool,
}
