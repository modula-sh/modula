use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub ts: String,
}

impl From<pb::ChatMessage> for ChatMessage {
    fn from(m: pb::ChatMessage) -> Self {
        Self {
            role: m.role,
            content: m.content,
            ts: m.ts,
        }
    }
}

impl From<ChatMessage> for pb::ChatMessage {
    fn from(m: ChatMessage) -> Self {
        Self {
            role: m.role,
            content: m.content,
            ts: m.ts,
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
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
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
            created_at: c.created_at,
            updated_at: c.updated_at,
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
            created_at: c.created_at,
            updated_at: c.updated_at,
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
    ToolUse { name: String, input: Value },
    #[serde(rename = "delta")]
    Delta { text: String },
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
                name: t.name,
                input: t.input.map(struct_to_json).unwrap_or(Value::Null),
            },
            Some(Event::Delta(d)) => ConvEvent::Delta { text: d.text },
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
            ConvEvent::ToolUse { name, input } => Some(Event::ToolUse(pb::ToolUseEvent {
                name,
                input: json_to_struct(input),
            })),
            ConvEvent::Delta { text } => Some(Event::Delta(pb::DeltaEvent { text })),
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
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hi".into(),
                ts: "2026-01-01T00:00:00Z".into(),
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
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
            "messages": [{"role": "user", "content": "hi", "ts": "2026-01-01T00:00:00Z"}],
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(serde_json::to_value(conversation()).unwrap(), want);
    }

    #[test]
    fn conv_event_round_trip_each_variant() {
        let variants = vec![
            ConvEvent::Session { id: "s".into() },
            ConvEvent::ToolUse {
                name: "Read".into(),
                input: json!({"path": "f"}),
            },
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
                name: "Read".into(),
                input: json!({"path": "f"}),
            })
            .unwrap(),
            json!({"kind": "tooluse", "name": "Read", "input": {"path": "f"}})
        );
        assert_eq!(
            serde_json::to_value(ConvEvent::Done).unwrap(),
            json!({"kind": "done"})
        );
    }
}
