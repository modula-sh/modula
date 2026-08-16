use modula_client::ModulaClient;
use modula_types::{ExternalItem, Integration};
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn integration_list(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<Integration>, String> {
    Ok(engine.list_integrations(&workspace_id).await?)
}

#[tauri::command]
pub async fn integration_connect(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    id: String,
    data: Value,
) -> Result<(), String> {
    engine.connect_integration(&workspace_id, &id, data).await?;
    Ok(())
}

#[tauri::command]
pub async fn integration_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    id: String,
) -> Result<(), String> {
    engine.delete_integration(&workspace_id, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn integration_search(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    id: String,
    query: String,
    params: Value,
) -> Result<Vec<ExternalItem>, String> {
    Ok(engine
        .search_integration(&workspace_id, &id, &query, params)
        .await?)
}

#[tauri::command]
pub async fn integration_fetch(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    id: String,
    key: String,
    params: Value,
) -> Result<ExternalItem, String> {
    Ok(engine
        .fetch_integration_item(&workspace_id, &id, &key, params)
        .await?)
}

#[tauri::command]
pub async fn integration_repos(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    id: String,
) -> Result<Vec<String>, String> {
    Ok(engine.list_integration_repos(&workspace_id, &id).await?)
}
