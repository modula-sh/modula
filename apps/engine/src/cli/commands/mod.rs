//! CRUD command implementations, one module per resource. Each `pub` entry
//! function takes its parsed subcommand plus the connected [`EngineTransport`],
//! drives the `modula-client` (`tx.client()`) which returns `modula_types`
//! domain types, and prints formatted text via the formatters (`super::format`)
//! — never JSON.

mod comment;
mod config;
mod roadmap;
mod task;
mod variant;
mod workspace;

pub use comment::comment;
pub use config::config;
pub use roadmap::roadmap;
pub use task::task;
pub use variant::variant;
pub use workspace::workspace;

use anyhow::{bail, Result};
use serde_json::{Map, Value};

/// A parsed JSON write body. Commands build a typed gRPC request from it with
/// the field accessors below, so a typo gives a CLI-level error instead of a
/// confusing server rejection.
pub struct Body(Map<String, Value>);

impl Body {
    /// Parse a write body, rejecting anything that isn't a JSON object.
    pub fn parse(s: &str) -> Result<Self> {
        let value: Value =
            serde_json::from_str(s).map_err(|e| anyhow::anyhow!("body is not valid JSON: {e}"))?;
        match value {
            Value::Object(map) => Ok(Self(map)),
            _ => bail!("body must be a JSON object"),
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(Value::as_str).map(str::to_string)
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.0.get(key).and_then(Value::as_i64)
    }

    pub fn boolean(&self, key: &str) -> Option<bool> {
        self.0.get(key).and_then(Value::as_bool)
    }

    /// A JSON array of strings, or empty when the key is absent.
    pub fn strings(&self, key: &str) -> Vec<String> {
        self.0
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A nested JSON value passed through verbatim (for schemaless fields like
    /// `source_data`); the client converts it to a proto `Struct` at the edge.
    pub fn json(&self, key: &str) -> Option<Value> {
        self.0.get(key).cloned()
    }
}
