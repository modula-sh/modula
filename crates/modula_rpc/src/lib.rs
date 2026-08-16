pub mod v1 {
    tonic::include_proto!("modula.v1");
}

pub mod convert;
pub mod json;
pub mod status;

pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/modula_v1_descriptor.bin"));

#[cfg(test)]
mod tests {
    use super::v1::{workspace_event, TaskCreatedEvent, TaskUpdatedEvent, WorkspaceEvent};
    use prost::Message;
    use prost_types::{value::Kind, Struct, Value};
    use std::collections::BTreeMap;

    // Typed round-trip: WorkspaceEvent with a TaskCreated oneof variant.
    #[test]
    fn workspace_event_typed_round_trip() {
        let original = WorkspaceEvent {
            seq: 42,
            workspace_id: "ws-1".into(),
            created_at: "2024-01-01T00:00:00Z".into(),
            event: Some(workspace_event::Event::TaskCreated(TaskCreatedEvent {
                task_id: "task-123".into(),
                source: "api".into(),
                approved: Some(true),
            })),
        };
        let mut buf = Vec::new();
        original.encode(&mut buf).unwrap();
        let decoded = WorkspaceEvent::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.seq, 42);
        assert_eq!(decoded.workspace_id, "ws-1");
        let Some(workspace_event::Event::TaskCreated(ev)) = decoded.event else {
            panic!("expected TaskCreated variant");
        };
        assert_eq!(ev.task_id, "task-123");
        assert_eq!(ev.approved, Some(true));
    }

    // Schemaless round-trip: TaskUpdatedEvent with a google.protobuf.Struct field.
    #[test]
    fn task_updated_struct_round_trip() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".into(),
            Value {
                kind: Some(Kind::StringValue("new name".into())),
            },
        );
        let original = TaskUpdatedEvent {
            task_id: "task-456".into(),
            changed_fields: Some(Struct { fields }),
            pipeline_status: Some("in_progress".into()),
        };
        let mut buf = Vec::new();
        original.encode(&mut buf).unwrap();
        let decoded = TaskUpdatedEvent::decode(buf.as_slice()).unwrap();
        assert_eq!(decoded.task_id, "task-456");
        assert_eq!(decoded.pipeline_status.as_deref(), Some("in_progress"));
        let fields = decoded.changed_fields.unwrap().fields;
        match fields["name"].kind.as_ref().unwrap() {
            Kind::StringValue(s) => assert_eq!(s, "new name"),
            _ => panic!("expected StringValue"),
        }
    }
}
