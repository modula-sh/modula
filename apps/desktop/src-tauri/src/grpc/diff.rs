//! Per-variant diff and PR commands. The client reassembles the chunked diff
//! stream (a size workaround, not a live stream) and returns the original JSON;
//! the PR info is unary JSON. Both are schemaless payloads the frontend consumes
//! directly.

use modula_client::ModulaClient;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn variant_diff(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    variant_id: String,
) -> Result<Value, String> {
    Ok(engine
        .variant_diff(&workspace_id, &task_id, &variant_id)
        .await?)
}

#[tauri::command]
pub async fn variant_pr(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    variant_id: String,
) -> Result<Value, String> {
    Ok(engine
        .variant_pr(&workspace_id, &task_id, &variant_id)
        .await?)
}
