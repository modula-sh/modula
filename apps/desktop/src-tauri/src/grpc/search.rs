use modula_client::ModulaClient;
use modula_types::SearchHit;
use tauri::State;

#[tauri::command]
pub async fn search_query(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    query: String,
    kinds: Vec<String>,
    limit: u32,
) -> Result<Vec<SearchHit>, String> {
    Ok(engine.search(&workspace_id, &query, &kinds, limit).await?)
}
