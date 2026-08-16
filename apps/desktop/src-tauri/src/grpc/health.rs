use modula_client::ModulaClient;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct EngineHealth {
    pub serving: bool,
}

/// Unary health probe over local IPC, the simplest path through the facade.
#[tauri::command]
pub async fn engine_health(engine: State<'_, ModulaClient>) -> Result<EngineHealth, String> {
    Ok(EngineHealth {
        serving: engine.is_serving().await,
    })
}
