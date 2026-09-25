//! Per-agent-run cost + token parsing. Reads the `type: result` event that
//! Claude and Gemini emit at the end of every stream-json run. Pure log-file
//! helpers with no repo/business logic — `RunService::usage` drives them over its runs.

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
        let int = |v: Option<&JsonValue>, key: &str| {
            v.and_then(|u| u.get(key))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        };
        // Gemini reports `stats` instead of `usage`, and no cost.
        if event.get("usage").is_none() {
            if let Some(stats) = event.get("stats") {
                return Some(LogSummary {
                    cost_usd: 0.0,
                    duration_ms: int(Some(stats), "duration_ms"),
                    tokens: UsageTokens {
                        input: int(Some(stats), "input_tokens"),
                        output: int(Some(stats), "output_tokens"),
                        cache_creation: 0,
                        cache_read: int(Some(stats), "cached"),
                    },
                });
            }
        }
        let usage = event.get("usage");
        return Some(LogSummary {
            cost_usd: event
                .get("total_cost_usd")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            duration_ms: int(Some(&event), "duration_ms"),
            tokens: UsageTokens {
                input: int(usage, "input_tokens"),
                output: int(usage, "output_tokens"),
                cache_creation: int(usage, "cache_creation_input_tokens"),
                cache_read: int(usage, "cache_read_input_tokens"),
            },
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_summary_reads_gemini_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("run.log");
        fs::write(
            &path,
            concat!(
                r#"{"type":"init","session_id":"s1","model":"auto"}"#,
                "\n",
                r#"{"type":"result","status":"success","stats":{"total_tokens":36787,"input_tokens":34727,"output_tokens":144,"cached":512,"input":34727,"duration_ms":12992,"tool_calls":2,"models":{}}}"#,
                "\n",
            ),
        )
        .unwrap();
        let s = log_summary(&path).unwrap();
        assert_eq!(s.cost_usd, 0.0);
        assert_eq!(s.duration_ms, 12992);
        assert_eq!(s.tokens.input, 34727);
        assert_eq!(s.tokens.output, 144);
        assert_eq!(s.tokens.cache_read, 512);
    }
}
