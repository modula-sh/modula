use modula_client::ModulaClient;
use modula_types::ConvEvent;
use serde_json::{json, Value};
use tauri::ipc::Channel as IpcChannel;
use tauri::State;
use tokio_stream::{Stream, StreamExt};

#[tauri::command]
pub async fn conversation_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
) -> Result<modula_types::Conversation, String> {
    Ok(engine
        .get_conversation(&workspace_id, &conversation_id)
        .await?)
}

#[tauri::command]
pub async fn conversation_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    provider_id: String,
    title: Option<String>,
    model: Option<String>,
    context: Option<Value>,
) -> Result<Value, String> {
    let id = engine
        .create_conversation(&workspace_id, &provider_id, title, model, context)
        .await?;
    Ok(json!({ "id": id }))
}

#[tauri::command]
pub async fn conversation_rename(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
    title: String,
) -> Result<(), String> {
    engine
        .rename_conversation(&workspace_id, &conversation_id, &title)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn conversation_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
) -> Result<(), String> {
    engine
        .delete_conversation(&workspace_id, &conversation_id)
        .await?;
    Ok(())
}

/// Cancel an in-flight run (explicit user action — distinct from a stream drop,
/// which only detaches).
#[tauri::command]
pub async fn conversation_cancel(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
) -> Result<(), String> {
    engine
        .cancel_conversation(&workspace_id, &conversation_id)
        .await?;
    Ok(())
}

/// Send a message and forward the run's `ConvEvent` stream to `on_event`. When
/// the frontend channel is gone (webview reload/navigation) the forward fails
/// and we drop the stream, which detaches from the run without cancelling it —
/// a later `conversation_attach` reattaches and resumes.
#[tauri::command]
pub async fn conversation_send(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
    message: String,
    model: Option<String>,
    on_event: IpcChannel<ConvEvent>,
) -> Result<(), String> {
    let stream = engine
        .send_message(&workspace_id, &conversation_id, &message, model)
        .await?;
    forward(stream, on_event).await
}

/// Attach to an in-flight run: replays buffered events then streams live ones.
#[tauri::command]
pub async fn conversation_attach(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
    on_event: IpcChannel<ConvEvent>,
) -> Result<(), String> {
    let stream = engine
        .attach_conversation(&workspace_id, &conversation_id)
        .await?;
    forward(stream, on_event).await
}

async fn forward(
    stream: impl Stream<Item = Result<ConvEvent, modula_client::ClientError>>,
    on_event: IpcChannel<ConvEvent>,
) -> Result<(), String> {
    let mut stream = std::pin::pin!(stream);
    while let Some(event) = stream.next().await {
        if on_event.send(event?).is_err() {
            break;
        }
    }
    Ok(())
}
