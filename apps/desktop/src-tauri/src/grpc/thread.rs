use modula_client::{AppendEntry, ModulaClient};
use modula_types::ThreadBundle;
use tauri::State;

#[tauri::command]
pub async fn thread_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
) -> Result<ThreadBundle, String> {
    Ok(engine.get_threads(&workspace_id, &task_id).await?)
}

/// Post a human comment. The desktop only ever appends `human`/`comment`
/// entries; agent verdicts and rework summaries come from the CLI.
#[tauri::command]
pub async fn thread_append(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    content: String,
    variant: Option<String>,
) -> Result<(), String> {
    engine
        .append_entry(AppendEntry {
            workspace_id,
            task_id,
            content,
            author: "human".into(),
            kind: "comment".into(),
            variant_id: variant,
            round: None,
            verdict: None,
            affected_variants: Vec::new(),
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn thread_edit(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    entry_id: i64,
    content: String,
    author: String,
) -> Result<(), String> {
    engine
        .edit_entry(&workspace_id, &task_id, entry_id, &content, &author)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn thread_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    entry_id: i64,
    author: String,
) -> Result<(), String> {
    engine
        .delete_entry(&workspace_id, &task_id, entry_id, &author)
        .await?;
    Ok(())
}
