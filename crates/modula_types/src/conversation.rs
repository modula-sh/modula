use modula_rpc::json::{json_to_struct, json_to_value, struct_to_json, value_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    #[default]
    Running,
    Completed,
    Failed,
}

impl From<i32> for ToolStatus {
    fn from(v: i32) -> Self {
        match pb::ToolStatus::try_from(v) {
            Ok(pb::ToolStatus::Completed) => Self::Completed,
            Ok(pb::ToolStatus::Failed) => Self::Failed,
            _ => Self::Running,
        }
    }
}

impl From<ToolStatus> for i32 {
    fn from(s: ToolStatus) -> Self {
        (match s {
            ToolStatus::Running => pb::ToolStatus::Running,
            ToolStatus::Completed => pb::ToolStatus::Completed,
            ToolStatus::Failed => pb::ToolStatus::Failed,
        }) as i32
    }
}

/// One tool call as the provider reported it: `name` and `input` are the
/// provider's own, never renamed or reshaped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
    /// The call this one ran under — a sub-agent's calls point at its spawner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub status: ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub output_truncated: bool,
}

impl From<pb::ToolCall> for ToolCall {
    fn from(t: pb::ToolCall) -> Self {
        Self {
            id: t.id,
            name: t.name,
            input: t.input.map(value_to_json).unwrap_or(Value::Null),
            parent_id: t.parent_id,
            status: t.status.into(),
            output: t.output,
            output_truncated: t.output_truncated,
        }
    }
}

impl From<ToolCall> for pb::ToolCall {
    fn from(t: ToolCall) -> Self {
        Self {
            id: t.id,
            name: t.name,
            input: Some(json_to_value(t.input)),
            parent_id: t.parent_id,
            status: t.status.into(),
            output: t.output,
            output_truncated: t.output_truncated,
        }
    }
}

/// One step of an assistant message, in the order the provider produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text { text: String },
    Thinking { text: String },
    Tool(ToolCall),
}

impl MessagePart {
    /// `None` for a part kind this build doesn't know.
    fn from_pb(p: pb::MessagePart) -> Option<Self> {
        use pb::message_part::Part;
        Some(match p.part? {
            Part::Text(text) => MessagePart::Text { text },
            Part::Thinking(text) => MessagePart::Thinking { text },
            Part::Tool(t) => MessagePart::Tool(t.into()),
        })
    }
}

impl From<MessagePart> for pb::MessagePart {
    fn from(p: MessagePart) -> Self {
        use pb::message_part::Part;
        let part = match p {
            MessagePart::Text { text } => Part::Text(text),
            MessagePart::Thinking { text } => Part::Thinking(text),
            MessagePart::Tool(t) => Part::Tool(t.into()),
        };
        Self { part: Some(part) }
    }
}

/// What the provider reported for the turn behind a message.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TurnStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Prompt tokens, cached ones included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl TurnStats {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl From<pb::TurnStats> for TurnStats {
    fn from(s: pb::TurnStats) -> Self {
        Self {
            duration_ms: s.duration_ms,
            input_tokens: s.input_tokens,
            output_tokens: s.output_tokens,
            cost_usd: s.cost_usd,
            model: s.model,
        }
    }
}

impl From<TurnStats> for pb::TurnStats {
    fn from(s: TurnStats) -> Self {
        Self {
            duration_ms: s.duration_ms,
            input_tokens: s.input_tokens,
            output_tokens: s.output_tokens,
            cost_usd: s.cost_usd,
            model: s.model,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    /// The message as plain text; an assistant message's is its text parts.
    pub content: String,
    pub ts: String,
    /// Position in the conversation, from 0.
    #[serde(default)]
    pub seq: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<MessagePart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<TurnStats>,
}

impl From<pb::ChatMessage> for ChatMessage {
    fn from(m: pb::ChatMessage) -> Self {
        Self {
            role: m.role,
            content: m.content,
            ts: m.ts,
            seq: m.seq,
            parts: m
                .parts
                .into_iter()
                .filter_map(MessagePart::from_pb)
                .collect(),
            stats: m.stats.map(TurnStats::from),
        }
    }
}

impl From<ChatMessage> for pb::ChatMessage {
    fn from(m: ChatMessage) -> Self {
        Self {
            role: m.role,
            content: m.content,
            ts: m.ts,
            seq: m.seq,
            parts: m.parts.into_iter().map(pb::MessagePart::from).collect(),
            stats: m.stats.map(pb::TurnStats::from),
        }
    }
}

/// A message waiting to be sent when the current run ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub id: String,
    pub content: String,
}

impl From<pb::QueuedMessage> for QueuedMessage {
    fn from(m: pb::QueuedMessage) -> Self {
        Self {
            id: m.id,
            content: m.content,
        }
    }
}

impl From<QueuedMessage> for pb::QueuedMessage {
    fn from(m: QueuedMessage) -> Self {
        Self {
            id: m.id,
            content: m.content,
        }
    }
}

/// A conversation with its messages (`dto::conversation` / frontend
/// `ConversationDetail`). `context` is schemaless and defaults to `{}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub context: Value,
    pub session_id: Option<String>,
    /// The tail the caller asked for, or every message.
    pub messages: Vec<ChatMessage>,
    /// How many messages the conversation holds in all.
    pub message_count: u32,
    pub queued: Vec<QueuedMessage>,
    pub created_at: String,
    pub updated_at: String,
    /// A provider run is in flight right now; filled by the gRPC layer.
    pub running: bool,
}

impl From<pb::Conversation> for Conversation {
    fn from(c: pb::Conversation) -> Self {
        Self {
            id: c.id,
            title: c.title,
            provider_id: c.provider_id,
            model: c.model,
            context: c.context.map(struct_to_json).unwrap_or_else(|| json!({})),
            session_id: c.session_id,
            messages: c.messages.into_iter().map(ChatMessage::from).collect(),
            message_count: c.message_count,
            queued: c.queued.into_iter().map(QueuedMessage::from).collect(),
            created_at: c.created_at,
            updated_at: c.updated_at,
            running: c.running,
        }
    }
}

impl From<Conversation> for pb::Conversation {
    fn from(c: Conversation) -> Self {
        Self {
            id: c.id,
            title: c.title,
            provider_id: c.provider_id,
            model: c.model,
            context: json_to_struct(c.context),
            session_id: c.session_id,
            messages: c.messages.into_iter().map(pb::ChatMessage::from).collect(),
            message_count: c.message_count,
            queued: c.queued.into_iter().map(pb::QueuedMessage::from).collect(),
            created_at: c.created_at,
            updated_at: c.updated_at,
            running: c.running,
        }
    }
}

/// One frame from a conversation stream (`dto::conv_event`), tagged by `kind`
/// so the frontend `Channel` consumer can switch on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ConvEvent {
    #[serde(rename = "session")]
    Session { id: String },
    #[serde(rename = "tooluse")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
    },
    #[serde(rename = "toolresult")]
    ToolResult {
        id: String,
        output: String,
        is_error: bool,
        output_truncated: bool,
    },
    #[serde(rename = "thinking")]
    Thinking { text: String },
    #[serde(rename = "delta")]
    Delta { text: String },
    #[serde(rename = "user")]
    User { text: String },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "unknown")]
    Unknown,
}

impl From<pb::ConvEvent> for ConvEvent {
    fn from(e: pb::ConvEvent) -> Self {
        use pb::conv_event::Event;
        match e.event {
            Some(Event::Session(s)) => ConvEvent::Session { id: s.id },
            Some(Event::ToolUse(t)) => ConvEvent::ToolUse {
                id: t.id,
                name: t.name,
                input: t.input.map(struct_to_json).unwrap_or(Value::Null),
                parent_id: t.parent_id,
            },
            Some(Event::ToolResult(r)) => ConvEvent::ToolResult {
                id: r.id,
                output: r.output,
                is_error: r.is_error,
                output_truncated: r.output_truncated,
            },
            Some(Event::Thinking(t)) => ConvEvent::Thinking { text: t.text },
            Some(Event::Delta(d)) => ConvEvent::Delta { text: d.text },
            Some(Event::User(u)) => ConvEvent::User { text: u.text },
            Some(Event::Done(_)) => ConvEvent::Done,
            Some(Event::Error(err)) => ConvEvent::Error {
                message: err.message,
            },
            None => ConvEvent::Unknown,
        }
    }
}

impl From<ConvEvent> for pb::ConvEvent {
    fn from(e: ConvEvent) -> Self {
        use pb::conv_event::Event;
        let event = match e {
            ConvEvent::Session { id } => Some(Event::Session(pb::SessionEvent { id })),
            ConvEvent::ToolUse {
                id,
                name,
                input,
                parent_id,
            } => Some(Event::ToolUse(pb::ToolUseEvent {
                name,
                input: json_to_struct(input),
                id,
                parent_id,
            })),
            ConvEvent::ToolResult {
                id,
                output,
                is_error,
                output_truncated,
            } => Some(Event::ToolResult(pb::ToolResultEvent {
                id,
                output,
                is_error,
                output_truncated,
            })),
            ConvEvent::Thinking { text } => Some(Event::Thinking(pb::ThinkingEvent { text })),
            ConvEvent::Delta { text } => Some(Event::Delta(pb::DeltaEvent { text })),
            ConvEvent::User { text } => Some(Event::User(pb::UserEvent { text })),
            ConvEvent::Done => Some(Event::Done(pb::DoneEvent {})),
            ConvEvent::Error { message } => Some(Event::Error(pb::ErrorEvent { message })),
            ConvEvent::Unknown => None,
        };
        Self { event }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conversation() -> Conversation {
        Conversation {
            id: "c1".into(),
            title: "Chat".into(),
            provider_id: "p1".into(),
            model: Some("opus".into()),
            context: json!({"task": "t1"}),
            session_id: None,
            messages: vec![
                ChatMessage {
                    role: "user".into(),
                    content: "hi".into(),
                    ts: "2026-01-01T00:00:00Z".into(),
                    seq: 0,
                    parts: vec![],
                    stats: None,
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "done".into(),
                    ts: "2026-01-01T00:00:01Z".into(),
                    seq: 1,
                    parts: vec![
                        MessagePart::Tool(ToolCall {
                            id: "t1".into(),
                            name: "Read".into(),
                            input: json!({"file_path": "f"}),
                            parent_id: None,
                            status: ToolStatus::Completed,
                            output: Some("alpha".into()),
                            output_truncated: false,
                        }),
                        MessagePart::Text {
                            text: "done".into(),
                        },
                    ],
                    stats: Some(TurnStats {
                        duration_ms: Some(1500),
                        ..Default::default()
                    }),
                },
            ],
            message_count: 2,
            queued: vec![QueuedMessage {
                id: "q1".into(),
                content: "next".into(),
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            running: true,
        }
    }

    #[test]
    fn conversation_round_trip() {
        let d = conversation();
        assert_eq!(d, Conversation::from(pb::Conversation::from(d.clone())));
    }

    #[test]
    fn conversation_serde_matches_dto() {
        let want = json!({
            "id": "c1", "title": "Chat", "provider_id": "p1", "model": "opus",
            "context": {"task": "t1"}, "session_id": null,
            "messages": [
                {"role": "user", "content": "hi", "ts": "2026-01-01T00:00:00Z", "seq": 0},
                {
                    "role": "assistant", "content": "done", "ts": "2026-01-01T00:00:01Z", "seq": 1,
                    "parts": [
                        {"type": "tool", "id": "t1", "name": "Read", "input": {"file_path": "f"},
                         "status": "completed", "output": "alpha"},
                        {"type": "text", "text": "done"},
                    ],
                    "stats": {"duration_ms": 1500},
                },
            ],
            "message_count": 2,
            "queued": [{"id": "q1", "content": "next"}],
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
            "running": true,
        });
        assert_eq!(serde_json::to_value(conversation()).unwrap(), want);
    }

    #[test]
    fn conv_event_round_trip_each_variant() {
        let variants = vec![
            ConvEvent::Session { id: "s".into() },
            ConvEvent::ToolUse {
                id: "t1".into(),
                name: "Read".into(),
                input: json!({"path": "f"}),
                parent_id: Some("t0".into()),
            },
            ConvEvent::ToolResult {
                id: "t1".into(),
                output: "alpha".into(),
                is_error: false,
                output_truncated: false,
            },
            ConvEvent::Thinking { text: "hm".into() },
            ConvEvent::Delta { text: "x".into() },
            ConvEvent::Done,
            ConvEvent::Error {
                message: "boom".into(),
            },
            ConvEvent::Unknown,
        ];
        for d in variants {
            assert_eq!(d.clone(), ConvEvent::from(pb::ConvEvent::from(d)));
        }
    }

    #[test]
    fn conv_event_serde_matches_dto() {
        assert_eq!(
            serde_json::to_value(ConvEvent::ToolUse {
                id: "t1".into(),
                name: "Read".into(),
                input: json!({"path": "f"}),
                parent_id: None,
            })
            .unwrap(),
            json!({"kind": "tooluse", "id": "t1", "name": "Read", "input": {"path": "f"}})
        );
        assert_eq!(
            serde_json::to_value(ConvEvent::Done).unwrap(),
            json!({"kind": "done"})
        );
    }
}
