//! `RunService` — agent-run history and per-run usage aggregation. Owns the
//! agent-run repository and DIs `WorkspaceService` to validate a workspace and
//! resolve its log directory. The gRPC `RunService`/`UsageService` handlers map
//! the returned rows to proto and never touch a repo.

use modula_db::agent_runs::AgentRunRepository;
use modula_db::Database;
use modula_types::AgentRun;

use crate::usage::{log_summary, UsageRun};
use crate::workspaces::WorkspaceService;
use modula_core::error::ApiResult;

const LIST_LIMIT: i64 = 200;
const USAGE_LIMIT: i64 = 500;

#[derive(Clone)]
pub struct RunService {
    pool: Database,
    agent_runs: AgentRunRepository,
    workspaces: WorkspaceService,
}

impl RunService {
    pub fn new(
        pool: Database,
        agent_runs: AgentRunRepository,
        workspaces: WorkspaceService,
    ) -> Self {
        Self {
            pool,
            agent_runs,
            workspaces,
        }
    }

    pub async fn list_recent(&self, ws: &str) -> ApiResult<Vec<AgentRun>> {
        self.workspaces.get(ws).await?;
        Ok(self
            .agent_runs
            .list_recent(&self.pool, ws, LIST_LIMIT)
            .await?)
    }

    pub async fn list_for_agent(&self, ws: &str, agent_id: &str) -> ApiResult<Vec<AgentRun>> {
        self.workspaces.get(ws).await?;
        Ok(self
            .agent_runs
            .list_for_agent(&self.pool, ws, agent_id, LIST_LIMIT)
            .await?)
    }

    /// Per-run cost + token usage: the recent runs whose log files carry a
    /// `type: result` summary. Runs without a parseable summary are skipped.
    pub async fn usage(&self, ws: &str) -> ApiResult<Vec<UsageRun>> {
        let ws_dir = self.workspaces.workspace_dir(ws).await?;
        let logs_dir = ws_dir.join("logs");
        let runs = self
            .agent_runs
            .list_recent(&self.pool, ws, USAGE_LIMIT)
            .await?;
        let mut out = Vec::new();
        for run in runs {
            let Some(name) = run.log_path.as_deref() else {
                continue;
            };
            let Some(s) = log_summary(&logs_dir.join(name)) else {
                continue;
            };
            out.push(UsageRun {
                run_id: run.id,
                log: name.to_string(),
                agent: run.agent_name,
                mtime: run.finished_at.unwrap_or(run.created_at),
                duration_ms: s.duration_ms,
                cost_usd: s.cost_usd,
                tokens: s.tokens,
            });
        }
        Ok(out)
    }
}
