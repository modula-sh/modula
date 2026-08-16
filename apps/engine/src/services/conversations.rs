//! Conversation chat lifecycle.
//!
//! The provider process and the client stream are decoupled: a single tokio
//! task per (workspace, conversation) owns the provider child, persists results
//! to the DB, and fans events out via a `broadcast::Sender`. Browser tabs come
//! and go without affecting the run — they `send` (start a new run), `attach`
//! (subscribe to an in-flight run + replay everything streamed so far), or
//! `cancel` (signal the task to kill the child, persist what it has, and exit).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use serde_json::{json, Value as JsonValue};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};

use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::services::events::{
    EventSink, CONVERSATION_CREATE, CONVERSATION_DELETE, CONVERSATION_UPDATE,
};
use crate::services::providers::{ChatEvent, ProviderRuntime, ProviderService};
use crate::state::AppState;
use modula_db::conversations::{ConversationCreate, ConversationRepository};
use modula_db::Database;
use modula_types::Conversation;

pub type ConvKey = (String, String);

/// Transport-agnostic stream handle: the replay buffer captured at subscribe
/// time plus a live receiver. The gRPC `ConversationService` builds its
/// per-client streams from this, so the detach/reattach semantics stay in one
/// place — dropping the returned receiver detaches the client without touching
/// the underlying run.
pub type ConvStream = (Vec<WireEvent>, broadcast::Receiver<WireEvent>);

/// One event in the provider→client stream. Buffered for replay and mapped to
/// the typed gRPC `ConvEvent` by the `ConversationService` handler.
#[derive(Clone, Debug)]
pub enum WireEvent {
    Session { id: String },
    ToolUse { name: String, input: JsonValue },
    Delta { text: String },
    Done,
    Error { message: String },
}

impl WireEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, WireEvent::Done | WireEvent::Error { .. })
    }
}

/// Per-conversation in-flight run. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct RunSlot {
    tx: broadcast::Sender<WireEvent>,
    /// Replayed verbatim to each new subscriber so a tab opened mid-turn sees
    /// every delta from the start, not just events after it attached.
    buffer: Arc<Mutex<Vec<WireEvent>>>,
    cancel: Arc<Notify>,
}

#[derive(Default, Clone)]
pub struct ConvRunRegistry {
    inner: Arc<RwLock<HashMap<ConvKey, RunSlot>>>,
}

impl ConvRunRegistry {
    async fn get(&self, key: &ConvKey) -> Option<RunSlot> {
        self.inner.read().await.get(key).cloned()
    }

    /// Atomically claim `key`: inserts `slot` and returns `true` only if no run
    /// was already registered. Holds the write lock across the check and insert
    /// so two concurrent sends for the same conversation can't both win.
    async fn insert_if_absent(&self, key: ConvKey, slot: RunSlot) -> bool {
        let mut map = self.inner.write().await;
        if let std::collections::hash_map::Entry::Vacant(e) = map.entry(key) {
            e.insert(slot);
            true
        } else {
            false
        }
    }

    async fn remove(&self, key: &ConvKey) {
        self.inner.write().await.remove(key);
    }
}

/// Conversation CRUD business service. Owns the conversation repository and the
/// [`EventSink`] it notifies on create/update/delete. The streaming lifecycle
/// (`open_send`/`open_attach`/`cancel` + [`ConvRunRegistry`]) is the runtime
/// half and stays as free functions; this is the durable-state half.
#[derive(Clone)]
pub struct ConversationService {
    pool: Database,
    conversations: ConversationRepository,
    events: Arc<dyn EventSink>,
}

impl ConversationService {
    pub fn new(
        pool: Database,
        conversations: ConversationRepository,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            conversations,
            events,
        }
    }

    pub async fn list(&self, ws: &str) -> ApiResult<Vec<Conversation>> {
        Ok(self.conversations.list(&self.pool, ws).await?)
    }

    pub async fn get(&self, ws: &str, id: &str) -> ApiResult<Conversation> {
        Ok(self.conversations.get(&self.pool, ws, id).await?)
    }

    pub async fn create(
        &self,
        ws: &str,
        title: Option<String>,
        provider_id: &str,
        model: Option<String>,
        context: JsonValue,
    ) -> ApiResult<String> {
        let provider_id = provider_id.trim().to_string();
        if provider_id.is_empty() {
            return Err(ApiError::BadRequest("provider_id is required".into()));
        }
        let id = Uuid::new_v4().to_string();
        let create = ConversationCreate {
            id: id.clone(),
            title,
            provider_id,
            model,
            context,
        };
        self.conversations.create(&self.pool, ws, &create).await?;
        self.publish(ws, CONVERSATION_CREATE, &id).await;
        Ok(id)
    }

    pub async fn update(&self, ws: &str, id: &str, title: Option<String>) -> ApiResult<()> {
        if let Some(title) = title {
            self.conversations
                .set_title(&self.pool, ws, id, &title)
                .await?;
        }
        self.publish(ws, CONVERSATION_UPDATE, id).await;
        Ok(())
    }

    pub async fn delete(&self, ws: &str, id: &str) -> ApiResult<()> {
        self.conversations.delete(&self.pool, ws, id).await?;
        self.publish(ws, CONVERSATION_DELETE, id).await;
        Ok(())
    }

    async fn publish(&self, ws: &str, kind: &str, id: &str) {
        self.events.publish(ws, kind, json!({ "id": id })).await;
    }
}

/// Push an event into the slot. Buffer push and broadcast happen under the
/// same mutex so a `subscribe()` interleaved between them can't miss it.
async fn emit(slot: &RunSlot, event: WireEvent) {
    let mut buf = slot.buffer.lock().await;
    buf.push(event.clone());
    let _ = slot.tx.send(event);
}

/// Snapshot the buffer and subscribe atomically — see `emit`.
async fn subscribe(slot: &RunSlot) -> (Vec<WireEvent>, broadcast::Receiver<WireEvent>) {
    let buf = slot.buffer.lock().await;
    (buf.clone(), slot.tx.subscribe())
}

/// Start a turn and return the transport-agnostic stream handle. Errors if a
/// turn is already in flight. The background task owns the provider child, so
/// dropping the returned receiver detaches without cancelling the run.
pub async fn open_send(
    state: AppState,
    ws_id: String,
    conv_id: String,
    user_msg: String,
    model_override: Option<String>,
) -> ApiResult<ConvStream> {
    let key = (ws_id.clone(), conv_id.clone());

    let (tx, _) = broadcast::channel::<WireEvent>(256);
    let slot = RunSlot {
        tx,
        buffer: Arc::new(Mutex::new(Vec::new())),
        cancel: Arc::new(Notify::new()),
    };

    // Claim the conversation slot atomically before any provider work. If a turn
    // is already in flight this rejects without spawning a second child.
    if !state
        .conv_runs
        .insert_if_absent(key.clone(), slot.clone())
        .await
    {
        return Err(ApiError::Conflict(
            "a turn is already in flight for this conversation".into(),
        ));
    }

    // Fallible setup runs after the claim, so every error path must release the
    // slot. Do it once here rather than threading cleanup through each `?`.
    let setup = open_send_setup(&state, &ws_id, &conv_id, &user_msg, model_override).await;
    let (runtime, child, stdout, stderr, has_session) = match setup {
        Ok(v) => v,
        Err(e) => {
            state.conv_runs.remove(&key).await;
            return Err(e);
        }
    };

    let (replay, rx) = subscribe(&slot).await;

    let runs = state.conv_runs.clone();
    let convs = state.repos.conversations.clone();
    let pool = state.repos.pool.clone();
    tokio::spawn(async move {
        run_to_completion(
            slot.clone(),
            runtime,
            child,
            stdout,
            stderr,
            convs,
            pool,
            ws_id.clone(),
            conv_id.clone(),
            has_session,
        )
        .await;
        runs.remove(&key).await;
    });

    Ok((replay, rx))
}

/// Resolve the conversation, persist the user turn, and spawn the provider child.
/// Returns the running child + its piped streams and whether a session id is
/// already established (resume vs. first turn). Separated from `open_send` so the
/// caller can release the conversation slot on any setup error.
type SendSetup = (
    Arc<dyn ProviderRuntime>,
    tokio::process::Child,
    tokio::process::ChildStdout,
    tokio::process::ChildStderr,
    bool,
);

async fn open_send_setup(
    state: &AppState,
    ws_id: &str,
    conv_id: &str,
    user_msg: &str,
    model_override: Option<String>,
) -> ApiResult<SendSetup> {
    if let Some(ref m) = model_override {
        let trimmed = m.trim();
        let v = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        state
            .repos
            .conversations
            .set_model(&state.repos.pool, ws_id, conv_id, v)
            .await?;
    }
    let conv = state
        .repos
        .conversations
        .get(&state.repos.pool, ws_id, conv_id)
        .await?;
    let provider = state
        .repos
        .providers
        .get(&state.repos.pool, ws_id, &conv.provider_id)
        .await?;
    let runtime = ProviderService::runtime_from_provider(&provider, conv.model.clone())?;

    state
        .repos
        .conversations
        .append_message(&state.repos.pool, ws_id, conv_id, "user", user_msg, &[])
        .await?;

    // Set the title eagerly from the first user message so the sidebar shows it
    // right away (within one ~2s snapshot poll) instead of "Untitled" — and so
    // it persists even if the provider turn fails before any content streams.
    if conv.title.is_empty() {
        let title = derive_title(user_msg);
        if !title.is_empty() {
            state
                .repos
                .conversations
                .set_title(&state.repos.pool, ws_id, conv_id, &title)
                .await?;
        }
    }

    // `conv` was read before the user message was appended, so an empty message
    // list means this is the conversation's first turn.
    let is_first_turn = conv.messages.is_empty();
    let prompt = if is_first_turn {
        let ctx = build_context(state, ws_id, &conv).await;
        if ctx.is_empty() {
            user_msg.to_string()
        } else {
            format!("{ctx}\n\n{user_msg}")
        }
    } else {
        user_msg.to_string()
    };

    let existing_session_id = conv.session_id.clone();
    let (mut cmd, preset_session) = if let Some(ref sid) = existing_session_id {
        (runtime.build_command_chat_resume(&prompt, sid), None)
    } else {
        let preset = uuid::Uuid::new_v4().to_string();
        match runtime.build_command_chat_first(&prompt, &preset) {
            Some(c) => (c, Some(preset)),
            None => (runtime.build_command(&prompt, None), None),
        }
    };

    let ws_dir = state.workspaces.workspace_dir(ws_id).await?;
    cmd.current_dir(&ws_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MODULA_WORKSPACE", ws_id)
        .env("MODULA_ENGINE_SOCKET", &state.engine_socket);
    for (k, v) in runtime.env_vars() {
        cmd.env(k, v);
    }

    let mut tokio_cmd = Command::from(cmd);
    // kill_on_drop matters only on cancel — the background task owns the
    // handle until the run finishes; client disconnects don't touch it.
    tokio_cmd.kill_on_drop(true);
    let mut child = tokio_cmd
        .spawn()
        .map_err(|e| ApiError::Internal(format!("spawn provider: {e}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ApiError::Internal("no stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ApiError::Internal("no stderr".into()))?;

    if let Some(ref sid) = preset_session {
        if existing_session_id.is_none() {
            let _ = state
                .repos
                .conversations
                .set_session_id(&state.repos.pool, ws_id, conv_id, sid)
                .await;
        }
    }

    let has_session = existing_session_id.is_some() || preset_session.is_some();
    Ok((runtime, child, stdout, stderr, has_session))
}

/// Subscribe to an in-flight run. If nothing is running, returns a handle whose
/// replay is a single `Done` so the client can fall back to the persisted state.
pub async fn open_attach(state: AppState, ws_id: String, conv_id: String) -> ConvStream {
    let key = (ws_id, conv_id);
    if let Some(slot) = state.conv_runs.get(&key).await {
        subscribe(&slot).await
    } else {
        (vec![WireEvent::Done], broadcast::channel(1).1)
    }
}

/// Signal an in-flight run to wind down. Returns 404 if there's no run.
pub async fn cancel(state: AppState, ws_id: String, conv_id: String) -> ApiResult<()> {
    let key = (ws_id, conv_id);
    let slot = state
        .conv_runs
        .get(&key)
        .await
        .ok_or_else(|| ApiError::NotFound("no in-flight run for this conversation".into()))?;
    slot.cancel.notify_one();
    Ok(())
}

/// Max bytes of stderr to retain for an error fallback message.
const STDERR_CAPTURE_CAP: usize = 4096;

#[allow(clippy::too_many_arguments)]
async fn run_to_completion(
    slot: RunSlot,
    runtime: Arc<dyn ProviderRuntime>,
    mut child: tokio::process::Child,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    convs: ConversationRepository,
    pool: Database,
    ws: String,
    cid: String,
    initial_session_captured: bool,
) {
    let stderr_task = tokio::spawn(drain_stderr(stderr));

    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut accumulated = String::new();
    let mut accumulated_tools: Vec<serde_json::Value> = Vec::new();
    let mut session_captured = initial_session_captured;
    let mut canceled = false;
    let mut parser_terminal = false;

    loop {
        tokio::select! {
            _ = slot.cancel.notified() => {
                canceled = true;
                break;
            }
            res = lines.next_line() => match res {
                Ok(Some(line)) if line.is_empty() => continue,
                Ok(Some(line)) => {
                    let mut terminal = false;
                    for event in runtime.parse_stream_line(&line) {
                        match event {
                            ChatEvent::Session { id } if !session_captured => {
                                let _ = convs.set_session_id(&pool, &ws, &cid, &id).await;
                                session_captured = true;
                                emit(&slot, WireEvent::Session { id }).await;
                            }
                            ChatEvent::ToolUse { name, input } => {
                                accumulated_tools.push(json!({ "name": &name, "input": &input }));
                                emit(&slot, WireEvent::ToolUse { name, input }).await;
                            }
                            ChatEvent::Delta { text } => {
                                accumulated.push_str(&text);
                                emit(&slot, WireEvent::Delta { text }).await;
                            }
                            ChatEvent::Done => {
                                parser_terminal = true;
                                terminal = true;
                                break;
                            }
                            ChatEvent::Error { message } => {
                                persist_and_emit(
                                    &convs, &pool, &ws, &cid,
                                    &accumulated, &accumulated_tools,
                                ).await;
                                emit(&slot, WireEvent::Error { message }).await;
                                let _ = child.kill().await;
                                let _ = child.wait().await;
                                let _ = stderr_task.await;
                                return;
                            }
                            _ => {}
                        }
                    }
                    if terminal {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("conversation stream read error: {e}");
                    persist_and_emit(
                        &convs, &pool, &ws, &cid,
                        &accumulated, &accumulated_tools,
                    ).await;
                    emit(&slot, WireEvent::Error { message: e.to_string() }).await;
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = stderr_task.await;
                    return;
                }
            }
        }
    }

    persist_and_emit(&convs, &pool, &ws, &cid, &accumulated, &accumulated_tools).await;

    if canceled {
        let _ = child.kill().await;
        let _ = child.wait().await;
        let _ = stderr_task.await;
        // End the stream cleanly; any partial response was persisted above.
        emit(&slot, WireEvent::Done).await;
        return;
    }

    let exit_status = child.wait().await;
    let stderr_text = stderr_task.await.unwrap_or_default();

    if parser_terminal {
        emit(&slot, WireEvent::Done).await;
        return;
    }

    // Provider closed stdout without emitting a Done/Error event — surface
    // whatever we can so the user isn't staring at silence.
    let failed = matches!(&exit_status, Ok(s) if !s.success());
    if failed {
        let mut msg = match &exit_status {
            Ok(s) => format!("provider exited with status {s}"),
            Err(e) => format!("provider wait failed: {e}"),
        };
        let trimmed = stderr_text.trim();
        if !trimmed.is_empty() {
            msg.push_str(": ");
            msg.push_str(trimmed);
        }
        emit(&slot, WireEvent::Error { message: msg }).await;
    } else {
        emit(&slot, WireEvent::Done).await;
    }
}

async fn drain_stderr(stderr: tokio::process::ChildStderr) -> String {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(STDERR_CAPTURE_CAP.min(1024));
    let mut reader = tokio::io::BufReader::new(stderr);
    let mut chunk = [0u8; 1024];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                let remaining = STDERR_CAPTURE_CAP.saturating_sub(buf.len());
                let take = n.min(remaining);
                if take > 0 {
                    buf.extend_from_slice(&chunk[..take]);
                }
                // Keep draining past the cap so the child's stderr pipe never blocks.
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Derive a conversation title from the first user message: the first non-empty
/// line, trimmed and capped at 120 chars. Empty/whitespace input yields "".
/// Truncation is by `chars()` so a multi-byte boundary is never split.
fn derive_title(user_msg: &str) -> String {
    user_msg
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}

async fn persist_and_emit(
    convs: &ConversationRepository,
    pool: &Database,
    ws: &str,
    cid: &str,
    accumulated: &str,
    accumulated_tools: &[serde_json::Value],
) {
    if accumulated.is_empty() && accumulated_tools.is_empty() {
        return;
    }
    if let Err(e) = convs
        .append_message(pool, ws, cid, "assistant", accumulated, accumulated_tools)
        .await
    {
        // The streaming design relies on this persisted turn to heal a mid-stream
        // broadcast Lagged via refetch, so a dropped write must at least be loud.
        tracing::error!(
            ws = %ws,
            conv = %cid,
            error = %e,
            "failed to persist assistant message; conversation turn lost"
        );
    }
}

async fn build_context(state: &AppState, ws_id: &str, conv: &Conversation) -> String {
    let ctx = &conv.context;
    let project_id = ctx["project"].as_str();
    let task_id = ctx["task"].as_str();
    let variant_id = ctx["variant"].as_str();

    let mut lines: Vec<String> = Vec::new();
    let mut description: Option<String> = None;

    // Task (and, when present, its selected variant). Variant is meaningful only
    // alongside a task, so it's resolved inside this block.
    if let Some(tid) = task_id {
        if let Ok(t) = state.repos.tasks.get(&state.repos.pool, ws_id, tid).await {
            let label = match t.external_id {
                Some(ref eid) => format!("{eid} — {}", t.title),
                None => t.title.clone(),
            };
            lines.push(format!("task: {label}"));
            lines.push(format!("task_id: {}", t.id));
            if let Some(ref s) = t.status {
                lines.push(format!("task_status: {s}"));
            }
            // Expose the human-facing 1-based position plus the raw variant UUID
            // and status in case the model needs to reference them directly.
            if let Some(vid) = variant_id {
                let variant = state
                    .repos
                    .variants
                    .get(&state.repos.pool, ws_id, tid, vid)
                    .await
                    .ok()
                    .flatten();
                if let Some(ref v) = variant {
                    lines.push(format!("variant: {}", v.position));
                }
                lines.push(format!("variant_id: {vid}"));
                if let Some(s) = variant.and_then(|v| v.status) {
                    lines.push(format!("variant_status: {s}"));
                }
            }
            if !t.description.trim().is_empty() {
                description = Some(t.description.clone());
            }
        }
    }

    // Projects in scope: the one selected, or all of them when none is chosen.
    let all_projects = state
        .repos
        .projects
        .list(&state.repos.pool, ws_id)
        .await
        .unwrap_or_default();
    let projects: Vec<_> = match project_id {
        Some(pid) => all_projects.into_iter().filter(|p| p.id == pid).collect(),
        None => all_projects,
    };
    if !projects.is_empty() {
        lines.push("projects:".to_string());
        for p in &projects {
            lines.push(format!(
                "  - {} ({}, base: {})",
                p.name, p.path, p.base_branch
            ));
        }
    }

    // Description goes last — it's the largest block. The closing tag delimits it.
    if let Some(desc) = description {
        lines.push("task_description: |".to_string());
        for l in desc.lines() {
            lines.push(format!("  {l}"));
        }
    }

    if lines.is_empty() {
        return String::new();
    }
    format!("<modula_context>\n{}\n</modula_context>", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::derive_title;

    #[test]
    fn empty_or_whitespace_yields_empty() {
        assert_eq!(derive_title(""), "");
        assert_eq!(derive_title("   \n\t  \n"), "");
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        assert_eq!(derive_title("  hello world  "), "hello world");
    }

    #[test]
    fn collapses_to_first_non_empty_line() {
        assert_eq!(
            derive_title("\n\n  first line  \nsecond line"),
            "first line"
        );
    }

    #[test]
    fn caps_at_120_chars() {
        let long = "a".repeat(200);
        assert_eq!(derive_title(&long).chars().count(), 120);
    }

    #[test]
    fn truncates_on_char_boundary() {
        // 200 multi-byte chars: cap by chars, never split a char.
        let long = "é".repeat(200);
        let title = derive_title(&long);
        assert_eq!(title.chars().count(), 120);
    }
}
