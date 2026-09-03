//! One-off text generation: a throwaway provider session on a single prompt.
//! Nothing is persisted. Prompt composition and cleanup live here so every
//! client gets identical output.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use uuid::Uuid;

use crate::providers::{self, ChatEvent, ProviderService};
use crate::workspaces::WorkspaceService;
use modula_core::error::{ApiError, ApiResult};
use modula_db::providers::ProviderRepository;
use modula_db::Database;

/// Bound on a single generation, so a wedged CLI can't hold the request open.
const GENERATE_TIMEOUT: Duration = Duration::from_secs(120);

/// The prompt every generation is sent.
const PROMPT_TEMPLATE: &str = "You are writing the contents of {target} in an app. \
Return only the resulting text: no preamble, no explanation, no code fences.\n\n\
Instruction: {instruction}";

/// `{target}`, with and without a caller-supplied field name.
const TARGET_LABELED: &str = "the \"{label}\" field";
const TARGET_UNLABELED: &str = "a text field";

pub struct GenerateParams {
    pub provider_id: String,
    pub model: Option<String>,
    pub instruction: String,
    pub field_label: Option<String>,
}

#[derive(Clone)]
pub struct GenerationService {
    pool: Database,
    providers: ProviderRepository,
    workspaces: WorkspaceService,
}

impl GenerationService {
    pub fn new(
        pool: Database,
        providers: ProviderRepository,
        workspaces: WorkspaceService,
    ) -> Self {
        Self {
            pool,
            providers,
            workspaces,
        }
    }

    pub async fn generate(&self, ws_id: &str, params: GenerateParams) -> ApiResult<String> {
        if params.instruction.trim().is_empty() {
            return Err(ApiError::BadRequest("instruction is required".into()));
        }
        let provider = self
            .providers
            .get(&self.pool, ws_id, &params.provider_id)
            .await?;
        let runtime = ProviderService::runtime_from_provider(&provider, params.model)?;
        let ws_dir = self.workspaces.workspace_dir(ws_id).await?;

        let prompt = compose_prompt(&params.instruction, params.field_label.as_deref());
        // Only the chat-first command passes Claude's `--include-partial-messages`,
        // without which it emits no deltas. codex/opencode return None here.
        let mut cmd = match runtime.build_command_chat_first(&prompt, &Uuid::new_v4().to_string()) {
            Some(c) => c,
            None => runtime.build_command(&prompt, None),
        };
        // No MODULA_WORKSPACE / MODULA_ENGINE_SOCKET: a text generator has no
        // business driving the engine.
        cmd.current_dir(&ws_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in runtime.env_vars() {
            cmd.env(k, v);
        }

        let mut tokio_cmd = Command::from(cmd);
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
        let stderr_task = tokio::spawn(providers::drain_stderr(stderr));

        let drain = async {
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            let mut text = String::new();
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|e| ApiError::Internal(format!("read provider stream: {e}")))?
            {
                for event in runtime.parse_stream_line(&line) {
                    match event {
                        ChatEvent::Delta { text: t } => text.push_str(&t),
                        ChatEvent::Done => return Ok(Some(text)),
                        ChatEvent::Error { message } => return Err(ApiError::Internal(message)),
                        _ => {}
                    }
                }
            }
            Ok(None)
        };

        // `kill_on_drop` reaps the child when this future is dropped on expiry.
        let drained = tokio::time::timeout(GENERATE_TIMEOUT, drain)
            .await
            .map_err(|_| ApiError::Internal("provider timed out".into()))??;

        let text = match drained {
            Some(text) => text,
            None => {
                let status = child.wait().await;
                let stderr_text = stderr_task.await.unwrap_or_default();
                if matches!(&status, Ok(s) if s.success()) {
                    String::new()
                } else {
                    let mut msg = match &status {
                        Ok(s) => format!("provider exited with status {s}"),
                        Err(e) => format!("provider wait failed: {e}"),
                    };
                    let trimmed = stderr_text.trim();
                    if !trimmed.is_empty() {
                        msg.push_str(": ");
                        msg.push_str(trimmed);
                    }
                    return Err(ApiError::Internal(msg));
                }
            }
        };

        let text = strip_code_fence(&text);
        if text.is_empty() {
            return Err(ApiError::Internal("provider returned no text".into()));
        }
        Ok(text)
    }
}

fn compose_prompt(instruction: &str, field_label: Option<&str>) -> String {
    let target = match field_label.map(str::trim).filter(|l| !l.is_empty()) {
        Some(label) => TARGET_LABELED.replace("{label}", label),
        None => TARGET_UNLABELED.to_string(),
    };
    PROMPT_TEMPLATE
        .replace("{target}", &target)
        .replace("{instruction}", instruction.trim())
}

/// Unwrap a response that is entirely one fenced block; leave anything else
/// (including text that merely contains a fence) alone.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    lines.next();
    let body: Vec<&str> = lines.collect();
    match body.split_last() {
        Some((last, rest)) if last.trim() == "```" => rest.join("\n").trim().to_string(),
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_a_whole_fenced_block() {
        assert_eq!(strip_code_fence("```\nhello\n```"), "hello");
        assert_eq!(
            strip_code_fence("```markdown\n# Title\nbody\n```"),
            "# Title\nbody"
        );
    }

    #[test]
    fn leaves_bare_text_alone() {
        assert_eq!(strip_code_fence("  hello there\n"), "hello there");
    }

    #[test]
    fn leaves_an_embedded_fence_alone() {
        let text = "Run this:\n```sh\nls\n```\nThen stop.";
        assert_eq!(strip_code_fence(text), text);
    }

    #[test]
    fn prompt_names_the_target_field() {
        let labeled = compose_prompt("fix grammar", Some("Description"));
        assert!(labeled.contains("the \"Description\" field"));
        assert!(labeled.ends_with("Instruction: fix grammar"));
        assert!(compose_prompt("write one", None).contains("a text field"));
    }
}
