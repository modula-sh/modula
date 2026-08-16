use modula_client::{ModulaClient, WikiFile};
use modula_types::WikiNode;
use tauri::State;

#[tauri::command]
pub async fn wiki_tree(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<WikiNode>, String> {
    Ok(engine.wiki_tree(&workspace_id).await?)
}

#[tauri::command]
pub async fn wiki_file(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
) -> Result<WikiFile, String> {
    Ok(engine.wiki_file(&workspace_id, &path).await?)
}

#[tauri::command]
pub async fn wiki_create_file(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
    content: String,
) -> Result<(), String> {
    engine
        .wiki_create_file(&workspace_id, &path, &content)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn wiki_write_file(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
    content: String,
) -> Result<(), String> {
    engine
        .wiki_write_file(&workspace_id, &path, &content)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn wiki_create_folder(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
) -> Result<(), String> {
    engine.wiki_create_folder(&workspace_id, &path).await?;
    Ok(())
}

#[tauri::command]
pub async fn wiki_rename(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    from: String,
    to: String,
) -> Result<(), String> {
    engine.wiki_rename(&workspace_id, &from, &to).await?;
    Ok(())
}

#[tauri::command]
pub async fn wiki_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
) -> Result<(), String> {
    engine.wiki_delete(&workspace_id, &path).await?;
    Ok(())
}
