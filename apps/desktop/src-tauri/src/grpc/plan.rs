//! Per-variant plan command. The engine returns the plan document as JSON text;
//! it is parsed here so the webview receives an object.

use modula_client::ModulaClient;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn plan_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    variant_id: String,
) -> Result<Value, String> {
    let text = engine
        .get_plan(&workspace_id, &task_id, &variant_id)
        .await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
