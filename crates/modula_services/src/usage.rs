//! Per-agent-run cost + token parsing. Reads the `type: result` event that
//! Claude emits at the end of every stream-json run. Pure log-file helpers with
//! no repo/business logic — `RunService::usage` drives them over its runs.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Serialize, Default)]
pub struct UsageTokens {
    pub input: i64,
    pub output: i64,
    pub cache_creation: i64,
    pub cache_read: i64,
}

#[derive(Serialize)]
pub struct UsageRun {
    pub run_id: i64,
    pub log: String,
    pub agent: String,
    pub mtime: String,
    pub duration_ms: i64,
    pub cost_usd: f64,
    pub tokens: UsageTokens,
}

pub struct LogSummary {
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub tokens: UsageTokens,
}

pub fn log_summary(path: &Path) -> Option<LogSummary> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if !line.contains("\"type\":\"result\"") {
            continue;
        }
        let event: JsonValue = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if event.get("type").and_then(|v| v.as_str()) != Some("result") {
            continue;
        }
        let usage = event.get("usage");
        let token = |key: &str| {
            usage
                .and_then(|u| u.get(key))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        };
        return Some(LogSummary {
            cost_usd: event
                .get("total_cost_usd")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            duration_ms: event
                .get("duration_ms")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            tokens: UsageTokens {
                input: token("input_tokens"),
                output: token("output_tokens"),
                cache_creation: token("cache_creation_input_tokens"),
                cache_read: token("cache_read_input_tokens"),
            },
        });
    }
    None
}
