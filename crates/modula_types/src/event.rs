use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The internal event vocabulary — the `type` strings persisted to the DB
/// event log and published on the engine's bus. Emit sites and the decoders
/// ([`WorkspaceEvent::from_parts`] / [`RunStatus::from_parts`]) share these so
/// they can't drift. Agent seed rules (`modula_db::agents`) keep these types
/// as inline string literals, so the consts here aren't exhaustive of every
/// use.
///
/// [`RunStatus::from_parts`]: crate::RunStatus::from_parts
pub mod event_types {
    pub const TASK_CREATE: &str = "task.create";
    pub const TASK_UPDATE: &str = "task.update";
    pub const TASK_DELETE: &str = "task.delete";
    pub const TASK_RESET: &str = "task.reset";
    pub const VARIANT_UPDATE: &str = "variant.update";
    pub const CONVERSATION_CREATE: &str = "conversation.create";
    pub const CONVERSATION_UPDATE: &str = "conversation.update";
    pub const CONVERSATION_DELETE: &str = "conversation.delete";
    pub const THREAD_APPEND: &str = "thread.append";
    pub const THREAD_UPDATE: &str = "thread.update";
    pub const THREAD_DELETE: &str = "thread.delete";
    pub const AGENT_CREATE: &str = "agent.create";
    pub const AGENT_UPDATE: &str = "agent.update";
    pub const AGENT_DELETE: &str = "agent.delete";
    pub const PROVIDER_CREATE: &str = "provider.create";
    pub const PROVIDER_UPDATE: &str = "provider.update";
    pub const PROVIDER_DELETE: &str = "provider.delete";

    /// Run lifecycle events.
    pub const RUN_SPAWNED: &str = "run.spawned";
    pub const RUN_EXITED: &str = "run.exited";
}

pub(crate) fn str_at(data: &Value, key: &str) -> String {
    data.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn opt_str(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

pub(crate) fn opt_i64(data: &Value, key: &str) -> i64 {
    data.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// A typed live workspace event (`dto::workspace_event`). The envelope fields
/// (`seq`/`workspace_id`/`created_at`) sit alongside the flattened, `type`-tagged
/// payload so the frontend can drive query invalidation off `type` + ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceEvent {
    pub seq: i64,
    pub workspace_id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub event: WorkspaceEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkspaceEventKind {
    #[serde(rename = "task_created")]
    TaskCreated {
        task_id: String,
        source: String,
        approved: Option<bool>,
    },
    #[serde(rename = "task_updated")]
    TaskUpdated {
        task_id: String,
        changed_fields: Value,
        pipeline_status: Option<String>,
    },
    #[serde(rename = "task_deleted")]
    TaskDeleted { task_id: String },
    #[serde(rename = "task_reset")]
    TaskReset { task_id: String },
    #[serde(rename = "variant_updated")]
    VariantUpdated {
        task_id: String,
        variant_id: String,
        status: String,
    },
    #[serde(rename = "conversation_created")]
    ConversationCreated { conversation_id: String },
    #[serde(rename = "conversation_updated")]
    ConversationUpdated { conversation_id: String },
    #[serde(rename = "conversation_deleted")]
    ConversationDeleted { conversation_id: String },
    #[serde(rename = "thread_appended")]
    ThreadAppended {
        task_id: String,
        variant_id: Option<String>,
        kind: String,
        author: String,
        verdict: Option<String>,
    },
    #[serde(rename = "thread_updated")]
    ThreadUpdated {
        task_id: String,
        variant_id: Option<String>,
        entry_id: i64,
        author: String,
    },
    #[serde(rename = "thread_deleted")]
    ThreadDeleted {
        task_id: String,
        variant_id: Option<String>,
        entry_id: i64,
    },
    #[serde(rename = "agent_run")]
    AgentRun {
        agent_id: String,
        agent_name: String,
        run_status: String,
        task_id: Option<String>,
        variant_id: Option<String>,
    },
    #[serde(rename = "agent_created")]
    AgentCreated { agent_id: String },
    #[serde(rename = "agent_updated")]
    AgentUpdated { agent_id: String },
    #[serde(rename = "agent_deleted")]
    AgentDeleted { agent_id: String },
    #[serde(rename = "provider_created")]
    ProviderCreated { provider_id: String },
    #[serde(rename = "provider_updated")]
    ProviderUpdated { provider_id: String },
    #[serde(rename = "provider_deleted")]
    ProviderDeleted { provider_id: String },
    #[serde(rename = "unknown")]
    Unknown,
}

impl WorkspaceEvent {
    /// Decode a raw event-log/bus record — a [`event_types`] string plus its
    /// schemaless JSON payload — into the typed event. Returns `None` for
    /// event types not surfaced on the watch stream. The single decode point
    /// for the live watch stream and the unary backfill, so the two can't
    /// drift; each emit site's payload shape lives with the emitting service.
    pub fn from_parts(
        seq: i64,
        workspace_id: &str,
        created_at: &str,
        type_: &str,
        data: &Value,
    ) -> Option<Self> {
        use event_types as t;
        let event = match type_ {
            t::TASK_CREATE => WorkspaceEventKind::TaskCreated {
                task_id: str_at(data, "task_id"),
                source: str_at(data, "source"),
                approved: data.get("approved").and_then(Value::as_bool),
            },
            t::TASK_UPDATE => {
                // Changed-fields are everything except the routing/shortcut keys.
                let changed = data
                    .as_object()
                    .map(|m| {
                        Value::Object(
                            m.iter()
                                .filter(|(k, _)| {
                                    !matches!(k.as_str(), "task_id" | "pipeline_status")
                                })
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect(),
                        )
                    })
                    .unwrap_or(Value::Null);
                WorkspaceEventKind::TaskUpdated {
                    task_id: str_at(data, "task_id"),
                    changed_fields: changed,
                    pipeline_status: opt_str(data, "pipeline_status"),
                }
            }
            t::TASK_DELETE => WorkspaceEventKind::TaskDeleted {
                task_id: str_at(data, "task_id"),
            },
            t::TASK_RESET => WorkspaceEventKind::TaskReset {
                task_id: str_at(data, "task_id"),
            },
            t::VARIANT_UPDATE => WorkspaceEventKind::VariantUpdated {
                task_id: str_at(data, "task_id"),
                variant_id: str_at(data, "variant_id"),
                status: str_at(data, "status"),
            },
            t::CONVERSATION_CREATE => WorkspaceEventKind::ConversationCreated {
                conversation_id: str_at(data, "id"),
            },
            t::CONVERSATION_UPDATE => WorkspaceEventKind::ConversationUpdated {
                conversation_id: str_at(data, "id"),
            },
            t::CONVERSATION_DELETE => WorkspaceEventKind::ConversationDeleted {
                conversation_id: str_at(data, "id"),
            },
            t::THREAD_APPEND => WorkspaceEventKind::ThreadAppended {
                task_id: str_at(data, "task_id"),
                variant_id: opt_str(data, "variant_id"),
                kind: str_at(data, "kind"),
                author: str_at(data, "author"),
                verdict: opt_str(data, "verdict"),
            },
            t::THREAD_UPDATE => WorkspaceEventKind::ThreadUpdated {
                task_id: str_at(data, "task_id"),
                variant_id: opt_str(data, "variant_id"),
                entry_id: opt_i64(data, "entry_id"),
                author: str_at(data, "author"),
            },
            t::THREAD_DELETE => WorkspaceEventKind::ThreadDeleted {
                task_id: str_at(data, "task_id"),
                variant_id: opt_str(data, "variant_id"),
                entry_id: opt_i64(data, "entry_id"),
            },
            t::RUN_SPAWNED => WorkspaceEventKind::AgentRun {
                agent_id: str_at(data, "agent_id"),
                agent_name: str_at(data, "agent_name"),
                run_status: "spawned".to_string(),
                task_id: opt_str(data, "task_id"),
                variant_id: opt_str(data, "variant_id"),
            },
            t::RUN_EXITED => WorkspaceEventKind::AgentRun {
                agent_id: str_at(data, "agent_id"),
                agent_name: str_at(data, "agent_name"),
                run_status: "exited".to_string(),
                task_id: opt_str(data, "task_id"),
                variant_id: opt_str(data, "variant_id"),
            },
            t::AGENT_CREATE => WorkspaceEventKind::AgentCreated {
                agent_id: str_at(data, "agent_id"),
            },
            t::AGENT_UPDATE => WorkspaceEventKind::AgentUpdated {
                agent_id: str_at(data, "agent_id"),
            },
            t::AGENT_DELETE => WorkspaceEventKind::AgentDeleted {
                agent_id: str_at(data, "agent_id"),
            },
            t::PROVIDER_CREATE => WorkspaceEventKind::ProviderCreated {
                provider_id: str_at(data, "provider_id"),
            },
            t::PROVIDER_UPDATE => WorkspaceEventKind::ProviderUpdated {
                provider_id: str_at(data, "provider_id"),
            },
            t::PROVIDER_DELETE => WorkspaceEventKind::ProviderDeleted {
                provider_id: str_at(data, "provider_id"),
            },
            _ => return None,
        };
        Some(Self {
            seq,
            workspace_id: workspace_id.to_string(),
            created_at: created_at.to_string(),
            event,
        })
    }
}

impl From<pb::WorkspaceEvent> for WorkspaceEvent {
    fn from(e: pb::WorkspaceEvent) -> Self {
        use pb::workspace_event::Event;
        let event = match e.event {
            Some(Event::TaskCreated(t)) => WorkspaceEventKind::TaskCreated {
                task_id: t.task_id,
                source: t.source,
                approved: t.approved,
            },
            Some(Event::TaskUpdated(t)) => WorkspaceEventKind::TaskUpdated {
                task_id: t.task_id,
                changed_fields: t
                    .changed_fields
                    .map(struct_to_json)
                    .unwrap_or_else(|| json!({})),
                pipeline_status: t.pipeline_status,
            },
            Some(Event::TaskDeleted(t)) => WorkspaceEventKind::TaskDeleted { task_id: t.task_id },
            Some(Event::TaskReset(t)) => WorkspaceEventKind::TaskReset { task_id: t.task_id },
            Some(Event::VariantUpdated(v)) => WorkspaceEventKind::VariantUpdated {
                task_id: v.task_id,
                variant_id: v.variant_id,
                status: v.status,
            },
            Some(Event::ConversationCreated(c)) => WorkspaceEventKind::ConversationCreated {
                conversation_id: c.conversation_id,
            },
            Some(Event::ConversationUpdated(c)) => WorkspaceEventKind::ConversationUpdated {
                conversation_id: c.conversation_id,
            },
            Some(Event::ConversationDeleted(c)) => WorkspaceEventKind::ConversationDeleted {
                conversation_id: c.conversation_id,
            },
            Some(Event::ThreadAppended(t)) => WorkspaceEventKind::ThreadAppended {
                task_id: t.task_id,
                variant_id: t.variant_id,
                kind: t.kind,
                author: t.author,
                verdict: t.verdict,
            },
            Some(Event::ThreadUpdated(t)) => WorkspaceEventKind::ThreadUpdated {
                task_id: t.task_id,
                variant_id: t.variant_id,
                entry_id: t.entry_id,
                author: t.author,
            },
            Some(Event::ThreadDeleted(t)) => WorkspaceEventKind::ThreadDeleted {
                task_id: t.task_id,
                variant_id: t.variant_id,
                entry_id: t.entry_id,
            },
            Some(Event::AgentRun(a)) => WorkspaceEventKind::AgentRun {
                agent_id: a.agent_id,
                agent_name: a.agent_name,
                run_status: a.run_status,
                task_id: a.task_id,
                variant_id: a.variant_id,
            },
            Some(Event::AgentCreated(a)) => WorkspaceEventKind::AgentCreated {
                agent_id: a.agent_id,
            },
            Some(Event::AgentUpdated(a)) => WorkspaceEventKind::AgentUpdated {
                agent_id: a.agent_id,
            },
            Some(Event::AgentDeleted(a)) => WorkspaceEventKind::AgentDeleted {
                agent_id: a.agent_id,
            },
            Some(Event::ProviderCreated(p)) => WorkspaceEventKind::ProviderCreated {
                provider_id: p.provider_id,
            },
            Some(Event::ProviderUpdated(p)) => WorkspaceEventKind::ProviderUpdated {
                provider_id: p.provider_id,
            },
            Some(Event::ProviderDeleted(p)) => WorkspaceEventKind::ProviderDeleted {
                provider_id: p.provider_id,
            },
            None => WorkspaceEventKind::Unknown,
        };
        Self {
            seq: e.seq,
            workspace_id: e.workspace_id,
            created_at: e.created_at,
            event,
        }
    }
}

impl From<WorkspaceEvent> for pb::WorkspaceEvent {
    fn from(e: WorkspaceEvent) -> Self {
        use pb::workspace_event::Event;
        let event = match e.event {
            WorkspaceEventKind::TaskCreated {
                task_id,
                source,
                approved,
            } => Some(Event::TaskCreated(pb::TaskCreatedEvent {
                task_id,
                source,
                approved,
            })),
            WorkspaceEventKind::TaskUpdated {
                task_id,
                changed_fields,
                pipeline_status,
            } => Some(Event::TaskUpdated(pb::TaskUpdatedEvent {
                task_id,
                changed_fields: json_to_struct(changed_fields),
                pipeline_status,
            })),
            WorkspaceEventKind::TaskDeleted { task_id } => {
                Some(Event::TaskDeleted(pb::TaskDeletedEvent { task_id }))
            }
            WorkspaceEventKind::TaskReset { task_id } => {
                Some(Event::TaskReset(pb::TaskResetEvent { task_id }))
            }
            WorkspaceEventKind::VariantUpdated {
                task_id,
                variant_id,
                status,
            } => Some(Event::VariantUpdated(pb::VariantUpdatedEvent {
                task_id,
                variant_id,
                status,
            })),
            WorkspaceEventKind::ConversationCreated { conversation_id } => {
                Some(Event::ConversationCreated(pb::ConversationCreatedEvent {
                    conversation_id,
                }))
            }
            WorkspaceEventKind::ConversationUpdated { conversation_id } => {
                Some(Event::ConversationUpdated(pb::ConversationUpdatedEvent {
                    conversation_id,
                }))
            }
            WorkspaceEventKind::ConversationDeleted { conversation_id } => {
                Some(Event::ConversationDeleted(pb::ConversationDeletedEvent {
                    conversation_id,
                }))
            }
            WorkspaceEventKind::ThreadAppended {
                task_id,
                variant_id,
                kind,
                author,
                verdict,
            } => Some(Event::ThreadAppended(pb::ThreadAppendedEvent {
                task_id,
                variant_id,
                kind,
                author,
                verdict,
            })),
            WorkspaceEventKind::ThreadUpdated {
                task_id,
                variant_id,
                entry_id,
                author,
            } => Some(Event::ThreadUpdated(pb::ThreadUpdatedEvent {
                task_id,
                variant_id,
                entry_id,
                author,
            })),
            WorkspaceEventKind::ThreadDeleted {
                task_id,
                variant_id,
                entry_id,
            } => Some(Event::ThreadDeleted(pb::ThreadDeletedEvent {
                task_id,
                variant_id,
                entry_id,
            })),
            WorkspaceEventKind::AgentRun {
                agent_id,
                agent_name,
                run_status,
                task_id,
                variant_id,
            } => Some(Event::AgentRun(pb::AgentRunEvent {
                agent_id,
                agent_name,
                run_status,
                task_id,
                variant_id,
            })),
            WorkspaceEventKind::AgentCreated { agent_id } => {
                Some(Event::AgentCreated(pb::AgentCreatedEvent { agent_id }))
            }
            WorkspaceEventKind::AgentUpdated { agent_id } => {
                Some(Event::AgentUpdated(pb::AgentUpdatedEvent { agent_id }))
            }
            WorkspaceEventKind::AgentDeleted { agent_id } => {
                Some(Event::AgentDeleted(pb::AgentDeletedEvent { agent_id }))
            }
            WorkspaceEventKind::ProviderCreated { provider_id } => {
                Some(Event::ProviderCreated(pb::ProviderCreatedEvent {
                    provider_id,
                }))
            }
            WorkspaceEventKind::ProviderUpdated { provider_id } => {
                Some(Event::ProviderUpdated(pb::ProviderUpdatedEvent {
                    provider_id,
                }))
            }
            WorkspaceEventKind::ProviderDeleted { provider_id } => {
                Some(Event::ProviderDeleted(pb::ProviderDeletedEvent {
                    provider_id,
                }))
            }
            WorkspaceEventKind::Unknown => None,
        };
        Self {
            seq: e.seq,
            workspace_id: e.workspace_id,
            created_at: e.created_at,
            event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(event: WorkspaceEventKind) -> WorkspaceEvent {
        WorkspaceEvent {
            seq: 9,
            workspace_id: "w1".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            event,
        }
    }

    #[test]
    fn round_trip_each_variant() {
        let kinds = vec![
            WorkspaceEventKind::TaskCreated {
                task_id: "t1".into(),
                source: "api".into(),
                approved: Some(true),
            },
            WorkspaceEventKind::TaskUpdated {
                task_id: "t1".into(),
                changed_fields: json!({"name": "x"}),
                pipeline_status: Some("in_progress".into()),
            },
            WorkspaceEventKind::TaskDeleted {
                task_id: "t1".into(),
            },
            WorkspaceEventKind::TaskReset {
                task_id: "t1".into(),
            },
            WorkspaceEventKind::VariantUpdated {
                task_id: "t1".into(),
                variant_id: "v1".into(),
                status: "in_progress".into(),
            },
            WorkspaceEventKind::ConversationCreated {
                conversation_id: "c1".into(),
            },
            WorkspaceEventKind::ThreadAppended {
                task_id: "t1".into(),
                variant_id: Some("v1".into()),
                kind: "comment".into(),
                author: "worker".into(),
                verdict: None,
            },
            WorkspaceEventKind::ThreadDeleted {
                task_id: "t1".into(),
                variant_id: None,
                entry_id: 3,
            },
            WorkspaceEventKind::AgentRun {
                agent_id: "a1".into(),
                agent_name: "worker".into(),
                run_status: "running".into(),
                task_id: Some("t1".into()),
                variant_id: None,
            },
            WorkspaceEventKind::AgentCreated {
                agent_id: "a1".into(),
            },
            WorkspaceEventKind::AgentUpdated {
                agent_id: "a1".into(),
            },
            WorkspaceEventKind::AgentDeleted {
                agent_id: "a1".into(),
            },
            WorkspaceEventKind::ProviderCreated {
                provider_id: "p1".into(),
            },
            WorkspaceEventKind::ProviderUpdated {
                provider_id: "p1".into(),
            },
            WorkspaceEventKind::ProviderDeleted {
                provider_id: "p1".into(),
            },
            WorkspaceEventKind::Unknown,
        ];
        for k in kinds {
            let d = envelope(k);
            assert_eq!(d.clone(), WorkspaceEvent::from(pb::WorkspaceEvent::from(d)));
        }
    }

    // Locks the raw-record decode: every event type in the vocabulary maps to
    // its kind, and the proto round-trip preserves it.
    #[test]
    fn from_parts_decodes_every_event_type() {
        use event_types as t;
        let cases: Vec<(&str, Value, WorkspaceEventKind)> = vec![
            (
                t::TASK_CREATE,
                json!({"task_id": "t1", "source": "api", "approved": true}),
                WorkspaceEventKind::TaskCreated {
                    task_id: "t1".into(),
                    source: "api".into(),
                    approved: Some(true),
                },
            ),
            (
                t::TASK_DELETE,
                json!({"task_id": "t1"}),
                WorkspaceEventKind::TaskDeleted {
                    task_id: "t1".into(),
                },
            ),
            (
                t::TASK_RESET,
                json!({"task_id": "t1"}),
                WorkspaceEventKind::TaskReset {
                    task_id: "t1".into(),
                },
            ),
            (
                t::VARIANT_UPDATE,
                json!({"task_id": "t1", "variant_id": "v1", "status": "in_progress"}),
                WorkspaceEventKind::VariantUpdated {
                    task_id: "t1".into(),
                    variant_id: "v1".into(),
                    status: "in_progress".into(),
                },
            ),
            (
                t::CONVERSATION_CREATE,
                json!({"id": "c1"}),
                WorkspaceEventKind::ConversationCreated {
                    conversation_id: "c1".into(),
                },
            ),
            (
                t::CONVERSATION_UPDATE,
                json!({"id": "c1"}),
                WorkspaceEventKind::ConversationUpdated {
                    conversation_id: "c1".into(),
                },
            ),
            (
                t::CONVERSATION_DELETE,
                json!({"id": "c1"}),
                WorkspaceEventKind::ConversationDeleted {
                    conversation_id: "c1".into(),
                },
            ),
            (
                t::THREAD_APPEND,
                json!({"task_id": "t1", "kind": "comment", "author": "worker", "verdict": ""}),
                WorkspaceEventKind::ThreadAppended {
                    task_id: "t1".into(),
                    variant_id: None,
                    kind: "comment".into(),
                    author: "worker".into(),
                    verdict: None,
                },
            ),
            (
                t::THREAD_UPDATE,
                json!({"task_id": "t1", "variant_id": "v1", "entry_id": 4, "author": "human"}),
                WorkspaceEventKind::ThreadUpdated {
                    task_id: "t1".into(),
                    variant_id: Some("v1".into()),
                    entry_id: 4,
                    author: "human".into(),
                },
            ),
            (
                t::THREAD_DELETE,
                json!({"task_id": "t1", "entry_id": 4}),
                WorkspaceEventKind::ThreadDeleted {
                    task_id: "t1".into(),
                    variant_id: None,
                    entry_id: 4,
                },
            ),
            (
                t::RUN_SPAWNED,
                json!({"agent_id": "a1", "agent_name": "worker", "task_id": "t1"}),
                WorkspaceEventKind::AgentRun {
                    agent_id: "a1".into(),
                    agent_name: "worker".into(),
                    run_status: "spawned".into(),
                    task_id: Some("t1".into()),
                    variant_id: None,
                },
            ),
            (
                t::RUN_EXITED,
                json!({"agent_id": "a1", "agent_name": "worker"}),
                WorkspaceEventKind::AgentRun {
                    agent_id: "a1".into(),
                    agent_name: "worker".into(),
                    run_status: "exited".into(),
                    task_id: None,
                    variant_id: None,
                },
            ),
            (
                t::AGENT_CREATE,
                json!({"agent_id": "a1"}),
                WorkspaceEventKind::AgentCreated {
                    agent_id: "a1".into(),
                },
            ),
            (
                t::AGENT_UPDATE,
                json!({"agent_id": "a1"}),
                WorkspaceEventKind::AgentUpdated {
                    agent_id: "a1".into(),
                },
            ),
            (
                t::AGENT_DELETE,
                json!({"agent_id": "a1"}),
                WorkspaceEventKind::AgentDeleted {
                    agent_id: "a1".into(),
                },
            ),
            (
                t::PROVIDER_CREATE,
                json!({"provider_id": "p1"}),
                WorkspaceEventKind::ProviderCreated {
                    provider_id: "p1".into(),
                },
            ),
            (
                t::PROVIDER_UPDATE,
                json!({"provider_id": "p1"}),
                WorkspaceEventKind::ProviderUpdated {
                    provider_id: "p1".into(),
                },
            ),
            (
                t::PROVIDER_DELETE,
                json!({"provider_id": "p1"}),
                WorkspaceEventKind::ProviderDeleted {
                    provider_id: "p1".into(),
                },
            ),
        ];
        for (type_, data, want) in cases {
            let got = WorkspaceEvent::from_parts(7, "ws", "now", type_, &data)
                .unwrap_or_else(|| panic!("{type_} should decode"));
            assert_eq!(got.seq, 7);
            assert_eq!(got.workspace_id, "ws");
            assert_eq!(got.created_at, "now");
            assert_eq!(got.event, want, "kind mismatch for {type_}");
            // Proto round-trip must not lose or reshape the kind.
            assert_eq!(
                got.clone(),
                WorkspaceEvent::from(pb::WorkspaceEvent::from(got)),
                "proto round-trip for {type_}"
            );
        }
    }

    // The changed-fields payload drops the routing/shortcut keys and survives
    // the trip into the proto `Struct`.
    #[test]
    fn from_parts_task_update_filters_routing_keys() {
        let data = json!({"task_id": "t1", "pipeline_status": "in_review", "name": "x"});
        let ev = WorkspaceEvent::from_parts(1, "ws", "", event_types::TASK_UPDATE, &data).unwrap();
        assert_eq!(
            ev.event,
            WorkspaceEventKind::TaskUpdated {
                task_id: "t1".into(),
                changed_fields: json!({"name": "x"}),
                pipeline_status: Some("in_review".into()),
            }
        );
        let pb::WorkspaceEvent { event, .. } = ev.into();
        match event {
            Some(pb::workspace_event::Event::TaskUpdated(t)) => {
                let fields = t.changed_fields.expect("struct");
                assert!(fields.fields.contains_key("name"));
                assert!(!fields.fields.contains_key("task_id"));
                assert!(!fields.fields.contains_key("pipeline_status"));
            }
            other => panic!("expected TaskUpdated, got {other:?}"),
        }
    }

    #[test]
    fn from_parts_unknown_type_is_skipped() {
        assert!(WorkspaceEvent::from_parts(1, "ws", "", "something.else", &json!({})).is_none());
    }

    // Locks the JSON the frontend consumes via `dto::workspace_event`: the
    // payload tag + fields flattened next to the envelope fields.
    #[test]
    fn serde_matches_dto() {
        let d = envelope(WorkspaceEventKind::TaskUpdated {
            task_id: "t1".into(),
            changed_fields: json!({"name": "x"}),
            pipeline_status: Some("in_progress".into()),
        });
        let want = json!({
            "type": "task_updated", "task_id": "t1",
            "changed_fields": {"name": "x"}, "pipeline_status": "in_progress",
            "seq": 9, "workspace_id": "w1", "created_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }
}
