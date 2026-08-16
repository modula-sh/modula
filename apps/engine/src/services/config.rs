//! `ConfigService` — the read-only aggregation behind `config.get`. It owns the
//! repositories whose rows compose a workspace's config and returns the
//! [`WorkspaceConfig`] domain aggregate; the gRPC handler converts it to proto
//! with one `.into()`.
//!
//! It maps the **base** provider/project domain types (no fs/git enrichment) so
//! `config.get` stays cheap — the enriching `Provider`/`Project` variants live
//! on `ProviderService`/`ProjectService`.

use modula_db::agents::AgentRepository;
use modula_db::pipeline::PipelineRepository;
use modula_db::projects::ProjectRepository;
use modula_db::providers::ProviderRepository;
use modula_db::settings::SettingsRepository;
use modula_db::workspaces::WorkspaceRepository;
use modula_db::Database;
use modula_types::{ConfigAgent, ConfigProject, ConfigProvider, WorkspaceConfig};

use crate::core::error::{ApiError, ApiResult};

#[derive(Clone)]
pub struct ConfigService {
    pool: Database,
    workspaces: WorkspaceRepository,
    settings: SettingsRepository,
    pipeline: PipelineRepository,
    providers: ProviderRepository,
    projects: ProjectRepository,
    agents: AgentRepository,
}

impl ConfigService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: Database,
        workspaces: WorkspaceRepository,
        settings: SettingsRepository,
        pipeline: PipelineRepository,
        providers: ProviderRepository,
        projects: ProjectRepository,
        agents: AgentRepository,
    ) -> Self {
        Self {
            pool,
            workspaces,
            settings,
            pipeline,
            providers,
            projects,
            agents,
        }
    }

    pub async fn get(&self, ws: &str) -> ApiResult<WorkspaceConfig> {
        if !self.workspaces.exists(&self.pool, ws).await? {
            return Err(ApiError::NotFound(format!("workspace not found: {ws}")));
        }
        Ok(WorkspaceConfig {
            limits: self.settings.get(&self.pool, ws).await?,
            pipeline: self.pipeline.list(&self.pool, ws).await?,
            providers: self
                .providers
                .list(&self.pool, ws)
                .await?
                .into_iter()
                .map(ConfigProvider::from)
                .collect(),
            projects: self
                .projects
                .list(&self.pool, ws)
                .await?
                .into_iter()
                .map(ConfigProject::from)
                .collect(),
            // Config carries the base agent (no computed `next_fire`).
            agents: self
                .agents
                .list(&self.pool, ws)
                .await?
                .into_iter()
                .map(ConfigAgent::from)
                .collect(),
        })
    }
}
