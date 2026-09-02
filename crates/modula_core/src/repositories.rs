//! The engine's repository set, built once and shared. Lives in `core` so a
//! service can take it without depending on the state that composes it.

use modula_db::agent_processes::AgentProcessRepository;
use modula_db::agent_runs::AgentRunRepository;
use modula_db::agent_skills::AgentSkillRepository;
use modula_db::agents::AgentRepository;
use modula_db::conversations::ConversationRepository;
use modula_db::events::EventRepository;
use modula_db::integrations::IntegrationRepository;
use modula_db::labels::LabelRepository;
use modula_db::pipeline::PipelineRepository;
use modula_db::projects::ProjectRepository;
use modula_db::providers::ProviderRepository;
use modula_db::roadmap::RoadmapRepository;
use modula_db::settings::SettingsRepository;
use modula_db::task_agent_settings::TaskAgentSettingsRepository;
use modula_db::tasks::TaskRepository;
use modula_db::threads::ThreadRepository;
use modula_db::variants::VariantRepository;
use modula_db::workspaces::WorkspaceRepository;
use modula_db::Database;

/// Every repository, constructed once in `AppState::new`; handlers and
/// services reach the DB through these rather than free functions over `&db`.
#[derive(Clone)]
pub struct Repositories {
    /// The shared pool. Direct callers (handlers, runtime) pass `&repos.pool` as
    /// the executor to a repository method; services own a pool clone to begin
    /// transactions.
    pub pool: Database,
    pub tasks: TaskRepository,
    pub variants: VariantRepository,
    pub threads: ThreadRepository,
    pub roadmap: RoadmapRepository,
    pub labels: LabelRepository,
    pub pipeline: PipelineRepository,
    pub agents: AgentRepository,
    pub agent_runs: AgentRunRepository,
    pub agent_skills: AgentSkillRepository,
    pub agent_processes: AgentProcessRepository,
    pub events: EventRepository,
    pub integrations: IntegrationRepository,
    pub providers: ProviderRepository,
    pub projects: ProjectRepository,
    pub workspaces: WorkspaceRepository,
    pub settings: SettingsRepository,
    pub conversations: ConversationRepository,
    pub task_agent_settings: TaskAgentSettingsRepository,
}

impl Repositories {
    pub fn new(db: &Database) -> Self {
        Self {
            pool: db.clone(),
            tasks: TaskRepository::new(),
            variants: VariantRepository::new(),
            threads: ThreadRepository::new(),
            roadmap: RoadmapRepository::new(),
            labels: LabelRepository::new(),
            pipeline: PipelineRepository::new(),
            agents: AgentRepository::new(),
            agent_runs: AgentRunRepository::new(),
            agent_skills: AgentSkillRepository::new(),
            agent_processes: AgentProcessRepository::new(),
            events: EventRepository::new(),
            integrations: IntegrationRepository::new(),
            providers: ProviderRepository::new(),
            projects: ProjectRepository::new(),
            workspaces: WorkspaceRepository::new(),
            settings: SettingsRepository::new(),
            conversations: ConversationRepository::new(),
            task_agent_settings: TaskAgentSettingsRepository::new(),
        }
    }
}
