use modula_client::ModulaClient;
use modula_types::Label;
use serde_json::{json, Value};
use tauri::State;

#[tauri::command]
pub async fn label_list(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    label_type: String,
) -> Result<Vec<Label>, String> {
    Ok(engine.list_labels(&workspace_id, &label_type).await?)
}

#[tauri::command]
pub async fn label_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    name: String,
    label_type: String,
) -> Result<Value, String> {
    let id = engine
        .create_label(&workspace_id, &name, &label_type)
        .await?;
    Ok(json!({ "id": id }))
}

#[tauri::command]
pub async fn label_attach(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    label_id: String,
) -> Result<(), String> {
    engine
        .attach_label(&workspace_id, &task_id, &label_id)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn label_detach(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    label_id: String,
) -> Result<(), String> {
    engine
        .detach_label(&workspace_id, &task_id, &label_id)
        .await?;
    Ok(())
}
