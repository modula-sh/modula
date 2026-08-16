//! Conversions between `serde_json::Value` and `google.protobuf.Struct`,
//! used by RPC handlers that carry genuinely schemaless payloads (agent-run
//! `data`, task `source_data`, etc.). Kept here so every handler shares one
//! implementation rather than re-deriving it per service.

use prost_types::{value::Kind, ListValue, Struct, Value};
use serde_json::Value as Json;

/// Convert a protobuf `Struct` into a JSON object value.
pub fn struct_to_json(s: Struct) -> Json {
    Json::Object(
        s.fields
            .into_iter()
            .map(|(k, v)| (k, value_to_json(v)))
            .collect(),
    )
}

/// Convert a JSON value into a protobuf `Struct`. Returns `None` for non-object
/// values, since `Struct` only models objects.
pub fn json_to_struct(v: Json) -> Option<Struct> {
    match v {
        Json::Object(m) => Some(Struct {
            fields: m.into_iter().map(|(k, v)| (k, json_to_value(v))).collect(),
        }),
        _ => None,
    }
}

fn value_to_json(v: Value) -> Json {
    match v.kind {
        Some(Kind::NullValue(_)) | None => Json::Null,
        Some(Kind::BoolValue(b)) => Json::Bool(b),
        Some(Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Some(Kind::StringValue(s)) => Json::String(s),
        Some(Kind::ListValue(l)) => Json::Array(l.values.into_iter().map(value_to_json).collect()),
        Some(Kind::StructValue(s)) => struct_to_json(s),
    }
}

fn json_to_value(v: Json) -> Value {
    let kind = match v {
        Json::Null => Kind::NullValue(0),
        Json::Bool(b) => Kind::BoolValue(b),
        Json::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or(0.0)),
        Json::String(s) => Kind::StringValue(s),
        Json::Array(a) => Kind::ListValue(ListValue {
            values: a.into_iter().map(json_to_value).collect(),
        }),
        Json::Object(m) => Kind::StructValue(Struct {
            fields: m.into_iter().map(|(k, v)| (k, json_to_value(v))).collect(),
        }),
    };
    Value { kind: Some(kind) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_nested_object() {
        // Numbers use floats because protobuf `Struct` only models `double`;
        // integers widen to f64, which is the documented lossy behavior.
        let original = json!({
            "flag": true,
            "count": 3.5,
            "name": "agent",
            "args": ["a", "b"],
            "nested": {"k": "v"},
            "missing": null,
        });
        let s = json_to_struct(original.clone()).unwrap();
        assert_eq!(struct_to_json(s), original);
    }

    #[test]
    fn integers_widen_to_float() {
        let s = json_to_struct(json!({"n": 3})).unwrap();
        assert_eq!(struct_to_json(s), json!({"n": 3.0}));
    }

    #[test]
    fn non_object_is_none() {
        assert!(json_to_struct(json!("scalar")).is_none());
        assert!(json_to_struct(json!([1, 2, 3])).is_none());
    }
}
