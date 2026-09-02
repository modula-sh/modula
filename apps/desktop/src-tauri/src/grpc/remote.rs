//! Modula Remote's control plane. The plugin crate is always present — the
//! public build links a `NotImplemented` stub — so these compile unconditionally
//! and `remote_available` tells the webview which one it got.

use modula_client::ModulaClient;
use modula_plugin_remote::client::RemoteClient;
use modula_plugin_remote::types::{RemoteDevice, RemoteStatus};
use serde::Serialize;
use tauri::State;

/// Whether this build has a working remote implementation; the panel hides itself
/// when false rather than rendering eight failing calls.
#[tauri::command]
pub fn remote_available() -> bool {
    modula_plugin_remote::AVAILABLE
}

/// Remote access is a property of the machine, so no command is workspace-scoped.
/// Every mutating command returns the whole status, mirroring `RemoteService`.
#[tauri::command]
pub async fn remote_status(engine: State<'_, ModulaClient>) -> Result<RemoteStatus, String> {
    Ok(engine.remote_status().await?)
}

#[tauri::command]
pub async fn remote_enable(engine: State<'_, ModulaClient>) -> Result<RemoteStatus, String> {
    Ok(engine.enable_remote().await?)
}

#[tauri::command]
pub async fn remote_disable(engine: State<'_, ModulaClient>) -> Result<RemoteStatus, String> {
    Ok(engine.disable_remote().await?)
}

#[tauri::command]
pub async fn remote_set_password(
    engine: State<'_, ModulaClient>,
    password: String,
) -> Result<RemoteStatus, String> {
    Ok(engine.set_remote_password(&password).await?)
}

/// `PairingCode` is not `Serialize` in the plugin, so mirror it for the webview.
#[derive(Serialize)]
pub struct PairingCode {
    pub qr_payload: String,
    pub expires_at: i64,
}

#[tauri::command]
pub async fn remote_begin_pairing(engine: State<'_, ModulaClient>) -> Result<PairingCode, String> {
    let code = engine.begin_remote_pairing().await?;
    Ok(PairingCode {
        qr_payload: code.qr_payload,
        expires_at: code.expires_at,
    })
}

#[tauri::command]
pub async fn remote_devices(engine: State<'_, ModulaClient>) -> Result<Vec<RemoteDevice>, String> {
    Ok(engine.list_remote_devices().await?)
}

#[tauri::command]
pub async fn remote_revoke_device(
    engine: State<'_, ModulaClient>,
    id: String,
) -> Result<RemoteStatus, String> {
    Ok(engine.revoke_remote_device(&id).await?)
}

#[tauri::command]
pub async fn remote_set_device_scope(
    engine: State<'_, ModulaClient>,
    id: String,
    scope: String,
) -> Result<RemoteStatus, String> {
    Ok(engine.set_remote_device_scope(&id, &scope).await?)
}
