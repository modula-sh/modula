use modula_client::{CreateProvider, CreatedProvider, GenerateText, ModulaClient, UpdateProvider};
use modula_types::{CatalogProvider, McpServer, Provider};
use tauri::State;

#[tauri::command]
pub async fn provider_catalog(
    engine: State<'_, ModulaClient>,
) -> Result<Vec<CatalogProvider>, String> {
    Ok(engine.provider_catalog().await?)
}

#[tauri::command]
pub async fn provider_list(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<Provider>, String> {
    Ok(engine.list_providers(&workspace_id).await?)
}

#[tauri::command]
pub async fn provider_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    provider_id: String,
) -> Result<Provider, String> {
    Ok(engine.get_provider(&workspace_id, &provider_id).await?)
}

#[tauri::command]
pub async fn provider_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    name: String,
    provider_type: String,
    config_dir: String,
    description: Option<String>,
    mcp_servers: Vec<McpServer>,
) -> Result<CreatedProvider, String> {
    Ok(engine
        .create_provider(CreateProvider {
            workspace_id,
            name,
            r#type: provider_type,
            config_dir,
            description,
            mcp_servers,
        })
        .await?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn provider_update(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    provider_id: String,
    name: Option<String>,
    provider_type: Option<String>,
    config_dir: Option<String>,
    description: Option<String>,
    clear_description: bool,
    mcp_servers: Option<Vec<McpServer>>,
) -> Result<(), String> {
    engine
        .update_provider(UpdateProvider {
            workspace_id,
            provider_id,
            name,
            r#type: provider_type,
            config_dir,
            description,
            clear_description,
            mcp_servers,
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn provider_generate(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    provider_id: String,
    model: Option<String>,
    instruction: String,
    current_text: String,
    field_label: Option<String>,
) -> Result<String, String> {
    Ok(engine
        .generate_text(GenerateText {
            workspace_id,
            provider_id,
            model,
            instruction,
            current_text,
            field_label,
        })
        .await?)
}

#[tauri::command]
pub async fn provider_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    provider_id: String,
) -> Result<(), String> {
    engine.delete_provider(&workspace_id, &provider_id).await?;
    Ok(())
}
