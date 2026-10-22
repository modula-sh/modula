//! Conversation chat lifecycle.
//!
//! The provider process and the client stream are decoupled: a single tokio
//! task per (workspace, conversation) owns the provider child, persists results
//! to the DB, and fans events out via a `broadcast::Sender`. Browser tabs come
//! and go without affecting the run — they `send` (start a new run), `attach`
//! (subscribe to an in-flight run + replay everything streamed so far), or
//! `cancel` (signal the task to kill the child, persist what it has, and exit).
//! Queued messages enter a run mid-turn when its provider accepts stdin input.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value as JsonValue};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};

use uuid::Uuid;

use crate::events::{
    EventSink, CONVERSATION_CREATE, CONVERSATION_DELETE, CONVERSATION_RUN_ENDED,
    CONVERSATION_RUN_STARTED, CONVERSATION_UPDATE,
};
use crate::providers::{self, ChatEvent, ProviderRuntime, ProviderService};
use crate::workspaces::WorkspaceService;
use modula_core::error::{ApiError, ApiResult};
use modula_core::repositories::Repositories;
use modula_db::conversations::{
    ConversationCreate, ConversationRepository, NewMessage, SessionAnchor,
};
use modula_db::Database;
use modula_types::{
    ChatMessage, Conversation, MessagePart, QueuedMessage, ToolCall, ToolStatus, TurnStats,
};

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
    Session {
        id: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
        parent_id: Option<String>,
    },
    ToolResult {
        id: String,
        output: String,
        is_error: bool,
        output_truncated: bool,
    },
    Delta {
        text: String,
    },
    Thinking {
        text: String,
    },
    /// A queued message taken into the running turn. Not replayed.
    User {
        text: String,
    },
    Done,
    Error {
        message: String,
    },
}

impl WireEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, WireEvent::Done | WireEvent::Error { .. })
    }
}

/// The most of a tool's output a transcript stores. Past it the head is kept
/// and the call marked truncated; the provider's own session has the rest.
const TOOL_OUTPUT_CAP: usize = 30_000;

/// The reply a run is building: its parts in the order the provider produced
/// them, and what the provider reports about the turn. Clients fold the same
/// stream events into the same parts.
struct Reply {
    parts: Vec<MessagePart>,
    stats: TurnStats,
    started: Instant,
    /// The provider's id for where its session stands, as last reported.
    anchor: Option<String>,
    /// Stands in for the id of a call whose provider gave none.
    unnamed_calls: usize,
}

impl Reply {
    fn new() -> Self {
        Self {
            parts: Vec::new(),
            stats: TurnStats::default(),
            started: Instant::now(),
            anchor: None,
            unnamed_calls: 0,
        }
    }

    fn text(&mut self, text: &str) {
        match self.parts.last_mut() {
            Some(MessagePart::Text { text: t }) => t.push_str(text),
            _ => self.parts.push(MessagePart::Text {
                text: text.to_string(),
            }),
        }
    }

    fn thinking(&mut self, text: &str) {
        match self.parts.last_mut() {
            Some(MessagePart::Thinking { text: t }) => t.push_str(text),
            _ => self.parts.push(MessagePart::Thinking {
                text: text.to_string(),
            }),
        }
    }

    /// The paragraph break owed to the trailing text or thinking part, as the
    /// event that carries it. `None` when nothing needs separating.
    fn paragraph_break(&mut self) -> Option<WireEvent> {
        const BREAK: &str = "\n\n";
        let (text, thinking) = match self.parts.last_mut()? {
            MessagePart::Text { text } => (text, false),
            MessagePart::Thinking { text } => (text, true),
            MessagePart::Tool(_) => return None,
        };
        if text.is_empty() || text.ends_with(BREAK) {
            return None;
        }
        text.push_str(BREAK);
        let text = BREAK.to_string();
        Some(if thinking {
            WireEvent::Thinking { text }
        } else {
            WireEvent::Delta { text }
        })
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut ToolCall> {
        self.parts.iter_mut().find_map(|p| match p {
            MessagePart::Tool(t) if t.id == id => Some(t),
            _ => None,
        })
    }

    /// Record a call, or its input as now known for one already recorded.
    /// Returns the event announcing a new call.
    fn tool_use(
        &mut self,
        id: String,
        name: String,
        input: JsonValue,
        parent_id: Option<String>,
    ) -> Option<WireEvent> {
        if let Some(tool) = (!id.is_empty()).then(|| self.tool_mut(&id)).flatten() {
            tool.name = name;
            tool.input = input;
            return None;
        }
        let id = if id.is_empty() {
            self.unnamed_calls += 1;
            format!("call-{}", self.unnamed_calls)
        } else {
            id
        };
        self.parts.push(MessagePart::Tool(ToolCall {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
            parent_id: parent_id.clone(),
            status: ToolStatus::Running,
            output: None,
            output_truncated: false,
        }));
        Some(WireEvent::ToolUse {
            id,
            name,
            input,
            parent_id,
        })
    }

    /// Complete a recorded call. Returns the event carrying what was stored.
    fn tool_result(&mut self, id: String, mut output: String, is_error: bool) -> Option<WireEvent> {
        let tool = self.tool_mut(&id)?;
        let output_truncated = output.len() > TOOL_OUTPUT_CAP;
        if output_truncated {
            let end = (0..=TOOL_OUTPUT_CAP)
                .rev()
                .find(|&i| output.is_char_boundary(i))
                .unwrap_or(0);
            output.truncate(end);
        }
        tool.status = if is_error {
            ToolStatus::Failed
        } else {
            ToolStatus::Completed
        };
        tool.output = Some(output.clone());
        tool.output_truncated = output_truncated;
        Some(WireEvent::ToolResult {
            id,
            output,
            is_error,
            output_truncated,
        })
    }

    /// Hand over the parts to persist. Mid-turn, calls still running stay
    /// behind so their results land on them in the next message.
    fn take(&mut self, last: bool) -> Vec<MessagePart> {
        let (keep, done) = std::mem::take(&mut self.parts).into_iter().partition(|p| {
            !last && matches!(p, MessagePart::Tool(t) if t.status == ToolStatus::Running)
        });
        self.parts = keep;
        done
    }
}

/// A message's parts as plain text: its text parts.
fn plain_text(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Per-conversation in-flight run. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct RunSlot {
    tx: broadcast::Sender<WireEvent>,
    /// Replayed verbatim to each new subscriber so a tab opened mid-turn sees
    /// every delta from the start, not just events after it attached.
    buffer: Arc<Mutex<Vec<WireEvent>>>,
    cancel: Arc<Notify>,
    queued: Arc<Notify>,
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

    /// The conversations in `ws` with a run in flight.
    pub async fn running(&self, ws: &str) -> std::collections::HashSet<String> {
        self.inner
            .read()
            .await
            .keys()
            .filter(|(w, _)| w == ws)
            .map(|(_, id)| id.clone())
            .collect()
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

    /// The conversation with its newest `message_limit` messages, or all of them.
    pub async fn get(
        &self,
        ws: &str,
        id: &str,
        message_limit: Option<u32>,
    ) -> ApiResult<Conversation> {
        let mut conv = self.conversations.get(&self.pool, ws, id).await?;
        conv.messages = self.messages(ws, id, None, message_limit).await?;
        Ok(conv)
    }

    /// Messages in order: the newest `limit` of those older than `before_seq`.
    pub async fn messages(
        &self,
        ws: &str,
        id: &str,
        before_seq: Option<u32>,
        limit: Option<u32>,
    ) -> ApiResult<Vec<ChatMessage>> {
        Ok(self
            .conversations
            .messages(&self.pool, ws, id, before_seq, limit)
            .await?)
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

    /// Clone `id` into a new conversation holding its transcript through
    /// `through_seq` (nothing, when `None`) and return the clone's id. The clone
    /// has no provider session yet: its first turn forks the source's at that
    /// message when the provider can, and otherwise replays the transcript.
    pub async fn fork(&self, ws: &str, id: &str, through_seq: Option<u32>) -> ApiResult<String> {
        let mut tx = self.pool.begin().await?;
        let source = self.conversations.get(&mut *tx, ws, id).await?;
        if through_seq.is_some_and(|seq| seq >= source.message_count) {
            return Err(ApiError::BadRequest(format!(
                "conversation has no message {}",
                through_seq.unwrap_or_default()
            )));
        }
        let clone_id = Uuid::new_v4().to_string();
        let create = ConversationCreate {
            id: clone_id.clone(),
            title: Some(source.title),
            provider_id: source.provider_id,
            model: source.model,
            context: source.context,
        };
        self.conversations.create(&mut *tx, ws, &create).await?;
        if let Some(seq) = through_seq {
            self.conversations
                .copy_messages(&mut *tx, ws, id, &clone_id, seq)
                .await?;
            let origin = self
                .conversations
                .message_anchor(&mut *tx, ws, id, seq)
                .await?;
            if let Some(origin) = origin {
                self.conversations
                    .set_fork_origin(&mut *tx, ws, &clone_id, &origin)
                    .await?;
            }
        }
        tx.commit().await?;
        self.publish(ws, CONVERSATION_CREATE, &clone_id).await;
        Ok(clone_id)
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

    /// Read-modify-write the queue in one transaction, then publish. An edit
    /// that leaves the queue unchanged neither writes nor publishes.
    async fn edit_queue<R>(
        &self,
        ws: &str,
        id: &str,
        edit: impl FnOnce(&mut Vec<QueuedMessage>) -> R,
    ) -> ApiResult<(R, Vec<QueuedMessage>)> {
        let mut tx = self.pool.begin().await?;
        let before = self.conversations.get(&mut *tx, ws, id).await?.queued;
        let mut queued = before.clone();
        let out = edit(&mut queued);
        if queued != before {
            self.conversations
                .set_queued(&mut *tx, ws, id, &queued)
                .await?;
            tx.commit().await?;
            self.publish(ws, CONVERSATION_UPDATE, id).await;
        }
        Ok((out, queued))
    }

    /// Queue `message` behind the in-flight run. Returns the resulting queue.
    pub async fn enqueue(
        &self,
        ws: &str,
        id: &str,
        message: &str,
    ) -> ApiResult<Vec<QueuedMessage>> {
        let content = message.trim();
        if content.is_empty() {
            return Err(ApiError::BadRequest("message is required".into()));
        }
        let msg = QueuedMessage {
            id: Uuid::new_v4().to_string(),
            content: content.to_string(),
        };
        Ok(self.edit_queue(ws, id, |q| q.push(msg)).await?.1)
    }

    /// Drop a queued message. An unknown `queued_id` is a no-op so two clients
    /// can tap the same x.
    pub async fn dequeue(
        &self,
        ws: &str,
        id: &str,
        queued_id: &str,
    ) -> ApiResult<Vec<QueuedMessage>> {
        Ok(self
            .edit_queue(ws, id, |q| q.retain(|m| m.id != queued_id))
            .await?
            .1)
    }

    /// Take the head of the queue. The caller must already hold the run slot,
    /// which is what keeps two drains from starting messages out of order.
    async fn pop_queued(&self, ws: &str, id: &str) -> ApiResult<Option<QueuedMessage>> {
        Ok(self
            .edit_queue(ws, id, |q| (!q.is_empty()).then(|| q.remove(0)))
            .await?
            .0)
    }

    /// Put a popped message back at the head after a failed auto-send.
    async fn requeue(&self, ws: &str, id: &str, msg: QueuedMessage) -> ApiResult<()> {
        self.edit_queue(ws, id, |q| q.insert(0, msg)).await?;
        Ok(())
    }

    /// Cheap pre-check so the common empty-queue drain never claims the run slot.
    async fn queue_is_empty(&self, ws: &str, id: &str) -> bool {
        self.conversations
            .get(&self.pool, ws, id)
            .await
            .is_ok_and(|c| c.queued.is_empty())
    }

    async fn clear_queued(&self, ws: &str, id: &str) -> ApiResult<()> {
        self.edit_queue(ws, id, |q| q.clear()).await?;
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

/// Drop the replay once what it holds is persisted; `event` reaches only
/// clients already attached.
async fn restart(slot: &RunSlot, event: WireEvent) {
    let mut buf = slot.buffer.lock().await;
    buf.retain(|e| matches!(e, WireEvent::Session { .. }));
    let _ = slot.tx.send(event);
}

/// What a conversation turn needs from the engine. Named here rather than
/// taking `AppState` so this module sits below the state that composes it.
#[derive(Clone)]
pub struct ConvRuntime {
    pub repos: Repositories,
    pub conv_runs: ConvRunRegistry,
    pub conversations: ConversationService,
    pub workspaces: WorkspaceService,
    pub engine_socket: String,
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
    rt: ConvRuntime,
    ws_id: String,
    conv_id: String,
    user_msg: String,
    model_override: Option<String>,
) -> ApiResult<ConvStream> {
    let key = (ws_id, conv_id);
    let slot = claim_run(&rt, &key).await.ok_or_else(|| {
        ApiError::Conflict("a turn is already in flight for this conversation".into())
    })?;
    run_claimed(rt, key, slot, user_msg, model_override).await
}

/// Claim the conversation's run slot atomically before any provider work.
/// `None` means a turn is already in flight.
async fn claim_run(rt: &ConvRuntime, key: &ConvKey) -> Option<RunSlot> {
    let (tx, _) = broadcast::channel::<WireEvent>(256);
    let slot = RunSlot {
        tx,
        buffer: Arc::new(Mutex::new(Vec::new())),
        cancel: Arc::new(Notify::new()),
        queued: Arc::new(Notify::new()),
    };
    rt.conv_runs
        .insert_if_absent(key.clone(), slot.clone())
        .await
        .then_some(slot)
}

/// Run a turn in a slot the caller has already claimed.
async fn run_claimed(
    rt: ConvRuntime,
    key: ConvKey,
    slot: RunSlot,
    user_msg: String,
    model_override: Option<String>,
) -> ApiResult<ConvStream> {
    let (ws_id, conv_id) = key.clone();

    // Fallible setup runs after the claim, so every error path must release the
    // slot. Do it once here rather than threading cleanup through each `?`.
    let setup = open_send_setup(&rt, &ws_id, &conv_id, &user_msg, model_override).await;
    let (runtime, child, stdin, held, stdout, stderr, session_id) = match setup {
        Ok(v) => v,
        Err(e) => {
            rt.conv_runs.remove(&key).await;
            return Err(e);
        }
    };

    let (replay, rx) = subscribe(&slot).await;

    let runs = rt.conv_runs.clone();
    let convs = rt.repos.conversations.clone();
    let pool = rt.repos.pool.clone();
    let events = rt.conversations.events.clone();
    // `conv_runs` is the truth for "a turn is in flight"; these two events mirror
    // its insert and its removal exactly, which is what lets a client that is not
    // already attached know to attach. Emitted around the spawn rather than inside
    // `run_to_completion`, which has four exit paths.
    publish_run(&events, &ws_id, &conv_id, true).await;
    // `run_to_completion` consumes one; the transition still has to be announced
    // after it returns.
    let ended = events.clone();
    let drain_rt = rt.clone();
    tokio::spawn(async move {
        run_to_completion(
            slot.clone(),
            runtime,
            child,
            stdin,
            held,
            drain_rt.conversations.clone(),
            stdout,
            stderr,
            convs,
            pool,
            events,
            ws_id.clone(),
            conv_id.clone(),
            session_id,
        )
        .await;
        runs.remove(&key).await;
        publish_run(&ended, &ws_id, &conv_id, false).await;
        drain_queue(drain_rt, ws_id, conv_id).await;
    });

    Ok((replay, rx))
}

/// A run transition. Carries no content — the turn's messages travel on the
/// stream, and `Attach` replays them to whoever wants them.
async fn publish_run(events: &Arc<dyn EventSink>, ws: &str, conv_id: &str, running: bool) {
    let type_ = if running {
        CONVERSATION_RUN_STARTED
    } else {
        CONVERSATION_RUN_ENDED
    };
    events.publish(ws, type_, json!({ "id": conv_id })).await;
}

/// Resolve the conversation, persist the user turn, and spawn the provider child.
/// Returns the running child + its piped streams, a prompt still waiting on a
/// session id, and the session id if known. Separated from `open_send` so the
/// caller can release the conversation slot on any setup error.
type SendSetup = (
    Arc<dyn ProviderRuntime>,
    tokio::process::Child,
    Option<tokio::process::ChildStdin>,
    Option<String>,
    tokio::process::ChildStdout,
    tokio::process::ChildStderr,
    Option<String>,
);

async fn open_send_setup(
    rt: &ConvRuntime,
    ws_id: &str,
    conv_id: &str,
    user_msg: &str,
    model_override: Option<String>,
) -> ApiResult<SendSetup> {
    // The setup writes `model`, `title` and `session_id` — all replicated by the
    // sync feed — so they publish one `conversation.update` once they're all done.
    let mut changed = false;
    if let Some(ref m) = model_override {
        let trimmed = m.trim();
        let v = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        rt.repos
            .conversations
            .set_model(&rt.repos.pool, ws_id, conv_id, v)
            .await?;
        changed = true;
    }
    let conv = rt
        .repos
        .conversations
        .get(&rt.repos.pool, ws_id, conv_id)
        .await?;
    let provider = rt
        .repos
        .providers
        .get(&rt.repos.pool, ws_id, &conv.provider_id)
        .await?;
    let runtime = ProviderService::runtime_from_provider(&provider, conv.model.clone())?;

    rt.repos
        .conversations
        .append_message(&rt.repos.pool, ws_id, conv_id, &NewMessage::user(user_msg))
        .await?;

    // Set the title eagerly from the first user message so the sidebar shows it
    // right away (within one ~2s snapshot poll) instead of "Untitled" — and so
    // it persists even if the provider turn fails before any content streams.
    if conv.title.is_empty() {
        let title = derive_title(user_msg);
        if !title.is_empty() {
            rt.repos
                .conversations
                .set_title(&rt.repos.pool, ws_id, conv_id, &title)
                .await?;
            changed = true;
        }
    }

    // `conv` was read before the user message was appended. A session it has
    // already holds its context. A clone's first turn forks the session it was
    // cut from when the provider can; any other turn without a session starts
    // one from the workspace context and whatever transcript there is.
    let existing_session_id = conv.session_id.clone();
    let fork = match existing_session_id {
        Some(_) => None,
        None => {
            rt.repos
                .conversations
                .fork_origin(&rt.repos.pool, ws_id, conv_id)
                .await?
        }
    };
    let forked = fork
        .as_ref()
        .and_then(|from| Some((runtime.build_command_chat_fork(user_msg, from)?, from)));
    let (prompt, mut cmd, preset_session, fork) = match (&existing_session_id, forked) {
        (Some(sid), _) => (
            user_msg.to_string(),
            runtime.build_command_chat_resume(user_msg, sid),
            None,
            None,
        ),
        (None, Some((cmd, from))) => (user_msg.to_string(), cmd, None, Some(from)),
        (None, None) => {
            let history = rt
                .repos
                .conversations
                .messages(
                    &rt.repos.pool,
                    ws_id,
                    conv_id,
                    Some(conv.message_count),
                    None,
                )
                .await?;
            let prompt = [
                build_context(rt, ws_id, &conv).await,
                replay_transcript(&history),
                user_msg.to_string(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
            let preset = uuid::Uuid::new_v4().to_string();
            match runtime.build_command_chat_first(&prompt, &preset) {
                Some(cmd) => (prompt, cmd, Some(preset), None),
                None => {
                    let cmd = runtime.build_command_chat(&prompt);
                    (prompt, cmd, None, None)
                }
            }
        }
    };
    let session_id = existing_session_id.clone().or(preset_session.clone());

    let opening = runtime.chat_open(session_id.as_deref(), fork);
    let ws_dir = rt.workspaces.workspace_dir(ws_id).await?;
    cmd.current_dir(&ws_dir)
        .stdin(if opening.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MODULA_WORKSPACE", ws_id)
        .env("MODULA_ENGINE_SOCKET", &rt.engine_socket);
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
    // Held open for the turn so queued messages can enter; closing it ends the run.
    let mut stdin = child.stdin.take();
    let mut held = None;
    if let (Some(opening), Some(pipe)) = (opening, stdin.as_mut()) {
        let prompt_line = runtime.chat_input(&prompt, session_id.as_deref());
        if prompt_line.is_none() {
            held = Some(prompt);
        }
        for line in opening.iter().chain(&prompt_line) {
            pipe.write_all(line.as_bytes())
                .await
                .map_err(|e| ApiError::Internal(format!("write provider stdin: {e}")))?;
        }
    }
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
            let _ = rt
                .repos
                .conversations
                .set_session_id(&rt.repos.pool, ws_id, conv_id, sid)
                .await;
            changed = true;
        }
    }
    if changed {
        rt.conversations
            .publish(ws_id, CONVERSATION_UPDATE, conv_id)
            .await;
    }

    Ok((runtime, child, stdin, held, stdout, stderr, session_id))
}

/// Subscribe to an in-flight run. If nothing is running, returns a handle whose
/// replay is a single `Done` so the client can fall back to the persisted state.
pub async fn open_attach(rt: ConvRuntime, ws_id: String, conv_id: String) -> ConvStream {
    let key = (ws_id, conv_id);
    if let Some(slot) = rt.conv_runs.get(&key).await {
        subscribe(&slot).await
    } else {
        (vec![WireEvent::Done], broadcast::channel(1).1)
    }
}

/// Signal an in-flight run to wind down. Returns 404 if there's no run.
pub async fn cancel(rt: ConvRuntime, ws_id: String, conv_id: String) -> ApiResult<()> {
    let key = (ws_id, conv_id);
    let slot = rt
        .conv_runs
        .get(&key)
        .await
        .ok_or_else(|| ApiError::NotFound("no in-flight run for this conversation".into()))?;
    // Clear before signalling: the run task drains the queue as it exits, so a
    // queue emptied afterwards would let the interrupt auto-send a discarded
    // message.
    rt.conversations.clear_queued(&key.0, &key.1).await?;
    slot.cancel.notify_one();
    Ok(())
}

/// Hand the queue to the run in flight, or start the next queued turn. The
/// empty check is an early-out only;
/// the run slot is still claimed before the head is popped, so two drains racing
/// (a run ending as an idle `Enqueue` drains) cannot start the queue's messages
/// out of order — the loser leaves the queue to the winner's own end-of-run
/// drain. A failed send puts the head back.
///
/// `open_send` recurses back into this through the task it spawns, so the
/// returned future is boxed rather than opaque — otherwise the two hidden types
/// depend on each other and neither can be proven `Send`.
pub fn drain_queue(
    rt: ConvRuntime,
    ws_id: String,
    conv_id: String,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let key = (ws_id, conv_id);
        if rt.conversations.queue_is_empty(&key.0, &key.1).await {
            return;
        }
        if let Some(slot) = rt.conv_runs.get(&key).await {
            slot.queued.notify_one();
            return;
        }
        let Some(slot) = claim_run(&rt, &key).await else {
            return;
        };
        let Ok(Some(head)) = rt.conversations.pop_queued(&key.0, &key.1).await else {
            rt.conv_runs.remove(&key).await;
            return;
        };
        if let Err(e) = run_claimed(rt.clone(), key.clone(), slot, head.content.clone(), None).await
        {
            tracing::warn!(ws = %key.0, conv = %key.1, error = %e, "queued message not sent");
            let _ = rt.conversations.requeue(&key.0, &key.1, head).await;
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_to_completion(
    slot: RunSlot,
    runtime: Arc<dyn ProviderRuntime>,
    mut child: tokio::process::Child,
    mut stdin: Option<tokio::process::ChildStdin>,
    mut held: Option<String>,
    queue: ConversationService,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    convs: ConversationRepository,
    pool: Database,
    events: Arc<dyn EventSink>,
    ws: String,
    cid: String,
    mut session_id: Option<String>,
) {
    let stderr_task = tokio::spawn(providers::drain_stderr(stderr));

    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut reply = Reply::new();
    let mut canceled = false;
    let mut parser_terminal = false;
    // Written but not yet acknowledged; a turn ending with one pending is followed by another.
    let mut pending_input = usize::from(stdin.is_some());
    // Only inject while a tool runs; otherwise the message could wait unseen until the turn ends.
    let mut tools_running = 0usize;

    loop {
        tokio::select! {
            _ = slot.cancel.notified() => {
                canceled = true;
                break;
            }
            _ = slot.queued.notified(), if tools_running > 0 => {
                if let Some(pipe) = stdin.as_mut() {
                    pending_input += inject_queued(
                        &queue, &runtime, pipe, session_id.as_deref(), &slot, &ws, &cid,
                        &mut reply,
                    ).await;
                }
            }
            res = lines.next_line() => match res {
                Ok(Some(line)) if line.is_empty() => continue,
                Ok(Some(line)) => {
                    let mut terminal = false;
                    for event in runtime.parse_stream_line(&line) {
                        match event {
                            ChatEvent::Session { id } if session_id.is_none() => {
                                let _ = convs.set_session_id(&pool, &ws, &cid, &id).await;
                                events
                                    .publish(&ws, CONVERSATION_UPDATE, json!({ "id": &cid }))
                                    .await;
                                session_id = Some(id.clone());
                                let line = held.take().and_then(|p| runtime.chat_input(&p, Some(&id)));
                                if let (Some(line), Some(pipe)) = (line, stdin.as_mut()) {
                                    if let Err(e) = pipe.write_all(line.as_bytes()).await {
                                        tracing::warn!("write provider stdin: {e}");
                                    }
                                }
                                emit(&slot, WireEvent::Session { id }).await;
                            }
                            ChatEvent::ToolUse { id, name, input, parent_id } => {
                                if let Some(ev) = reply.tool_use(id, name, input, parent_id) {
                                    tools_running += 1;
                                    emit(&slot, ev).await;
                                }
                            }
                            ChatEvent::ToolResult { id, output, is_error } => {
                                tools_running = tools_running.saturating_sub(1);
                                if let Some(ev) = reply.tool_result(id, output, is_error) {
                                    emit(&slot, ev).await;
                                }
                            }
                            ChatEvent::Break => {
                                if let Some(ev) = reply.paragraph_break() {
                                    emit(&slot, ev).await;
                                }
                            }
                            ChatEvent::Delta { text } => {
                                reply.text(&text);
                                emit(&slot, WireEvent::Delta { text }).await;
                            }
                            ChatEvent::Thinking { text } => {
                                reply.thinking(&text);
                                emit(&slot, WireEvent::Thinking { text }).await;
                            }
                            ChatEvent::Usage { input_tokens, output_tokens, cost_usd } => {
                                let stats = &mut reply.stats;
                                *stats.input_tokens.get_or_insert(0) += input_tokens;
                                *stats.output_tokens.get_or_insert(0) += output_tokens;
                                if let Some(usd) = cost_usd {
                                    *stats.cost_usd.get_or_insert(0.0) += usd;
                                }
                            }
                            ChatEvent::Anchor { id } => reply.anchor = Some(id),
                            ChatEvent::Duration { ms } => reply.stats.duration_ms = Some(ms),
                            ChatEvent::Model { name } => reply.stats.model = Some(name),
                            ChatEvent::InputAccepted => {
                                pending_input = pending_input.saturating_sub(1);
                            }
                            ChatEvent::Done => {
                                parser_terminal = true;
                                if pending_input == 0 {
                                    terminal = true;
                                    break;
                                }
                            }
                            ChatEvent::Error { message } => {
                                persist_reply(&convs, &pool, &ws, &cid, &mut reply, Persist::CUT_SHORT).await;
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
                    if tools_running > 0 {
                        if let Some(pipe) = stdin.as_mut() {
                            pending_input += inject_queued(
                                &queue, &runtime, pipe, session_id.as_deref(), &slot, &ws, &cid,
                                &mut reply,
                            ).await;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("conversation stream read error: {e}");
                    persist_reply(&convs, &pool, &ws, &cid, &mut reply, Persist::CUT_SHORT).await;
                    emit(&slot, WireEvent::Error { message: e.to_string() }).await;
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = stderr_task.await;
                    return;
                }
            }
        }
    }
    drop(stdin);

    // Only a turn the provider finished leaves its session somewhere a clone
    // can fork from.
    let finished = parser_terminal && !canceled;
    let session = session_id.as_deref().filter(|_| finished);
    persist_reply(
        &convs,
        &pool,
        &ws,
        &cid,
        &mut reply,
        Persist::Final { session },
    )
    .await;

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

/// Write the queue into the running turn and return how many went in. Each is
/// persisted after the reply so far, in the order the provider saw it.
#[allow(clippy::too_many_arguments)]
async fn inject_queued(
    queue: &ConversationService,
    runtime: &Arc<dyn ProviderRuntime>,
    stdin: &mut tokio::process::ChildStdin,
    session_id: Option<&str>,
    slot: &RunSlot,
    ws: &str,
    cid: &str,
    reply: &mut Reply,
) -> usize {
    let mut injected = 0;
    while let Ok(Some(msg)) = queue.pop_queued(ws, cid).await {
        let written = match runtime.chat_input(&msg.content, session_id) {
            Some(line) => stdin.write_all(line.as_bytes()).await.is_ok(),
            None => false,
        };
        if !written {
            let _ = queue.requeue(ws, cid, msg).await;
            break;
        }
        injected += 1;
        persist_reply(
            &queue.conversations,
            &queue.pool,
            ws,
            cid,
            reply,
            Persist::Partial,
        )
        .await;
        if let Err(e) = queue
            .conversations
            .append_message(&queue.pool, ws, cid, &NewMessage::user(&msg.content))
            .await
        {
            tracing::error!(ws = %ws, conv = %cid, error = %e, "failed to persist queued message");
        }
        restart(slot, WireEvent::User { text: msg.content }).await;
    }
    injected
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

/// How much of its turn a persisted reply covers.
#[derive(Clone, Copy)]
enum Persist<'a> {
    /// The reply so far, with the turn still running.
    Partial,
    /// The end of the turn. `session` is the provider session it finished in;
    /// `None` when the turn was cut short.
    Final { session: Option<&'a str> },
}

impl Persist<'_> {
    const CUT_SHORT: Self = Self::Final { session: None };
}

/// Persist the reply so far as one assistant message. The one that ends the
/// turn carries the turn's stats, with the engine's own clock standing in for
/// a duration the provider didn't report, and where the turn left its session.
async fn persist_reply(
    convs: &ConversationRepository,
    pool: &Database,
    ws: &str,
    cid: &str,
    reply: &mut Reply,
    persist: Persist<'_>,
) {
    let last = matches!(persist, Persist::Final { .. });
    let parts = reply.take(last);
    if parts.is_empty() {
        return;
    }
    let content = plain_text(&parts);
    let stats = last.then(|| {
        let mut stats = reply.stats.clone();
        stats
            .duration_ms
            .get_or_insert(reply.started.elapsed().as_millis() as u64);
        stats
    });
    let anchor = match (persist, &reply.anchor) {
        (
            Persist::Final {
                session: Some(session),
            },
            Some(anchor),
        ) => Some(SessionAnchor {
            session: session.to_string(),
            anchor: anchor.clone(),
        }),
        _ => None,
    };
    let msg = NewMessage {
        role: "assistant",
        content: &content,
        parts: &parts,
        stats: stats.as_ref(),
        anchor: anchor.as_ref(),
    };
    if let Err(e) = convs.append_message(pool, ws, cid, &msg).await {
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

/// The longest a tool call's input runs in a replayed transcript.
const REPLAY_INPUT_CAP: usize = 400;

/// A transcript as the opening of a new provider session, for a chat that
/// can't pick up the session its messages came from. Tool calls are named
/// with their input; what they returned is left out, since the agent can run
/// them again and their output is most of a transcript's size.
fn replay_transcript(messages: &[ChatMessage]) -> String {
    if messages.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "<conversation_history>\n\
         This chat continues an earlier conversation. Its transcript so far follows; \
         tool calls are listed without their output.\n",
    );
    for msg in messages {
        out.push_str(&format!("\n[{}]\n", msg.role));
        if msg.parts.is_empty() {
            out.push_str(msg.content.trim());
            out.push('\n');
            continue;
        }
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } if !text.trim().is_empty() => {
                    out.push_str(text.trim());
                    out.push('\n');
                }
                MessagePart::Tool(tool) => {
                    let input: String = tool
                        .input
                        .to_string()
                        .chars()
                        .take(REPLAY_INPUT_CAP)
                        .collect();
                    out.push_str(&format!("(tool {}: {input})\n", tool.name));
                }
                _ => {}
            }
        }
    }
    out.push_str("</conversation_history>");
    out
}

async fn build_context(rt: &ConvRuntime, ws_id: &str, conv: &Conversation) -> String {
    let ctx = &conv.context;
    let project_id = ctx["project"].as_str();
    let task_id = ctx["task"].as_str();
    let variant_id = ctx["variant"].as_str();

    let mut lines: Vec<String> = Vec::new();
    let mut description: Option<String> = None;

    // Task (and, when present, its selected variant). Variant is meaningful only
    // alongside a task, so it's resolved inside this block.
    if let Some(tid) = task_id {
        if let Ok(t) = rt.repos.tasks.get(&rt.repos.pool, ws_id, tid).await {
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
                let variant = rt
                    .repos
                    .variants
                    .get(&rt.repos.pool, ws_id, tid, vid)
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
    let all_projects = rt
        .repos
        .projects
        .list(&rt.repos.pool, ws_id)
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
    use super::{derive_title, plain_text, replay_transcript, Reply, WireEvent, TOOL_OUTPUT_CAP};
    use modula_types::{ChatMessage, MessagePart, ToolCall, ToolStatus};
    use serde_json::json;

    fn tool(reply: &Reply, i: usize) -> &modula_types::ToolCall {
        match &reply.parts[i] {
            MessagePart::Tool(t) => t,
            other => panic!("expected a tool at {i}, got {other:?}"),
        }
    }

    #[test]
    fn reply_keeps_text_and_tools_in_order() {
        let mut reply = Reply::new();
        reply.text("Let me ");
        reply.text("look.");
        assert!(reply
            .tool_use("t1".into(), "Read".into(), json!({"file_path": "a"}), None)
            .is_some());
        reply.text("Done.");
        assert_eq!(reply.parts.len(), 3);
        assert_eq!(
            reply.parts[0],
            MessagePart::Text {
                text: "Let me look.".into()
            }
        );
        assert_eq!(tool(&reply, 1).status, ToolStatus::Running);
        assert_eq!(plain_text(&reply.parts), "Let me look.\n\nDone.");
    }

    #[test]
    fn reply_split_mid_turn_keeps_running_calls_for_their_results() {
        let mut reply = Reply::new();
        reply.text("Working.");
        reply.tool_use("done".into(), "Read".into(), json!({}), None);
        reply.tool_result("done".into(), "ok".into(), false);
        reply.tool_use("slow".into(), "Bash".into(), json!({}), None);

        let persisted = reply.take(false);
        assert_eq!(persisted.len(), 2, "the text and the finished call");
        assert_eq!(tool(&reply, 0).id, "slow");
        assert!(reply
            .tool_result("slow".into(), "late".into(), false)
            .is_some());

        // The turn's end takes everything, finished or not.
        reply.tool_use("cut".into(), "Bash".into(), json!({}), None);
        assert_eq!(reply.take(true).len(), 2);
        assert!(reply.parts.is_empty());
    }

    #[test]
    fn reply_breaks_paragraphs_only_between_text() {
        let mut reply = Reply::new();
        assert!(reply.paragraph_break().is_none(), "nothing to separate yet");
        reply.text("one");
        assert!(matches!(
            reply.paragraph_break(),
            Some(WireEvent::Delta { text }) if text == "\n\n"
        ));
        assert!(reply.paragraph_break().is_none(), "already broken");
        reply.text("two");
        assert_eq!(
            reply.parts,
            [MessagePart::Text {
                text: "one\n\ntwo".into()
            }]
        );
        reply.tool_use("t1".into(), "Bash".into(), json!({}), None);
        assert!(
            reply.paragraph_break().is_none(),
            "a tool already separates"
        );
    }

    #[test]
    fn reply_completes_a_call_once_and_restates_its_input() {
        let mut reply = Reply::new();
        reply.tool_use(
            "t1".into(),
            "fileChange".into(),
            json!({"changes": []}),
            None,
        );
        // The finished item restates the call: its input is updated, not announced again.
        let restated = json!({"changes": [{"path": "a.rs"}]});
        assert!(reply
            .tool_use("t1".into(), "fileChange".into(), restated.clone(), None)
            .is_none());
        assert!(matches!(
            reply.tool_result("t1".into(), "boom".into(), true),
            Some(WireEvent::ToolResult { is_error: true, .. })
        ));
        assert!(reply
            .tool_result("nope".into(), "x".into(), false)
            .is_none());
        assert_eq!(reply.parts.len(), 1);
        let call = tool(&reply, 0);
        assert_eq!(call.input, restated);
        assert_eq!(
            (call.status, call.output.as_deref()),
            (ToolStatus::Failed, Some("boom"))
        );
    }

    #[test]
    fn replayed_transcript_keeps_the_text_and_names_the_tool_calls() {
        assert_eq!(replay_transcript(&[]), "");
        let message = |role: &str, content: &str, parts: Vec<MessagePart>| ChatMessage {
            role: role.into(),
            content: content.into(),
            ts: String::new(),
            seq: 0,
            parts,
            stats: None,
        };
        let replay = replay_transcript(&[
            message("user", "What is in a.txt?", vec![]),
            message(
                "assistant",
                "It holds alpha.",
                vec![
                    MessagePart::Tool(ToolCall {
                        id: "t1".into(),
                        name: "Read".into(),
                        input: json!({"file_path": "a.txt"}),
                        parent_id: None,
                        status: ToolStatus::Completed,
                        output: Some("1\tSECRET-OUTPUT".into()),
                        output_truncated: false,
                    }),
                    MessagePart::Text {
                        text: "It holds alpha.".into(),
                    },
                ],
            ),
        ]);
        assert!(replay.starts_with("<conversation_history>"));
        assert!(replay.ends_with("</conversation_history>"));
        assert!(replay.contains("[user]\nWhat is in a.txt?\n"));
        assert!(replay
            .contains("[assistant]\n(tool Read: {\"file_path\":\"a.txt\"})\nIt holds alpha.\n"));
        assert!(!replay.contains("SECRET-OUTPUT"));
    }

    #[test]
    fn reply_names_calls_the_provider_left_unnamed() {
        let mut reply = Reply::new();
        reply.tool_use(String::new(), "Bash".into(), json!({}), None);
        reply.tool_use(String::new(), "Bash".into(), json!({}), None);
        assert_eq!(reply.parts.len(), 2);
        assert_ne!(tool(&reply, 0).id, tool(&reply, 1).id);
    }

    #[test]
    fn reply_caps_a_stored_output_on_a_char_boundary() {
        let mut reply = Reply::new();
        reply.tool_use("t1".into(), "Read".into(), json!({}), None);
        // Two-byte chars, with the cap falling inside one.
        let big = format!("a{}", "é".repeat(TOOL_OUTPUT_CAP));
        reply.tool_result("t1".into(), big, false);
        let call = tool(&reply, 0);
        assert!(call.output_truncated);
        let stored = call.output.as_deref().unwrap();
        assert!(stored.len() <= TOOL_OUTPUT_CAP && stored.len() >= TOOL_OUTPUT_CAP - 1);

        reply.tool_use("t2".into(), "Read".into(), json!({}), None);
        reply.tool_result("t2".into(), "short".into(), false);
        assert!(!tool(&reply, 1).output_truncated);
    }

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
