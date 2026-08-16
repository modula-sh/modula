use std::sync::Arc;

use crate::services::labels::LabelService;
use crate::services::tasks::TaskService;
use crate::services::threads::ThreadService;
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
use modula_engine_transport::EngineEndpoint;

use crate::core::paths::Paths;
use crate::services::agents::AgentService;
use crate::services::config::ConfigService;
use crate::services::conversations::{ConvRunRegistry, ConversationService};
use crate::services::diffs::DiffService;
use crate::services::events::{Bus, EventService, EventSink};
use crate::services::integrations::IntegrationsService;
use crate::services::logs::LogsService;
use crate::services::loop_registry::LoopRegistry;
use crate::services::pr::PrService;
use crate::services::processes::ProcessesService;
use crate::services::projects::ProjectService;
use crate::services::providers::ProviderService;
use crate::services::runs::RunService;
use crate::services::scheduler::SchedulerHandle;
use crate::services::snapshot::SnapshotService;
use crate::services::workspaces::WorkspaceService;

/// The data-access seam: every `*Repository` owns a clone of the pool. Composed
/// once in `AppState::new`; handlers and services reach the DB through these
/// instead of loose free functions over `&db`.
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

#[derive(Clone)]
pub struct AppState {
    pub paths: Arc<Paths>,
    pub repos: Repositories,
    /// Task business service, owning its repositories and the event sink,
    /// including the filesystem-touching task ops (`reset`).
    pub tasks: TaskService,
    /// Thread (comment/verdict) business service, owning its repositories and
    /// the event sink. One of the composed engine service impls.
    pub threads: ThreadService,
    /// Label CRUD + task-association business service, owning its repository.
    pub labels: LabelService,
    /// Integration config CRUD + external search/fetch business service.
    pub integrations: IntegrationsService,
    /// Agent CRUD + manual `trigger`/`kill`/`list_running` business service,
    /// owning the repositories and the runtime handles (scheduler, loops, bus,
    /// engine socket, paths) those ops need.
    pub agents: AgentService,
    /// Provider CRUD + catalog business service, owning the provider repository
    /// and the single hydration path from a provider domain value to a runtime
    /// instance (`runtime_for_type` / `runtime_from_provider`).
    pub providers: ProviderService,
    /// Workspace CRUD business service (owns the workspace repo + scheduler).
    pub workspaces: WorkspaceService,
    /// Project CRUD + git-inspection business service.
    pub projects: ProjectService,
    /// Config aggregation service backing `config.get`.
    pub config: ConfigService,
    /// Event business service owning the single publish path (persist to the
    /// event log, then broadcast) plus the log read/backfill queries.
    pub events: EventService,
    /// Agent-run history + per-run usage aggregation service.
    pub runs: RunService,
    /// Conversation CRUD business service. Distinct from `conv_runs`, which is
    /// the in-flight streaming registry (runtime), not durable state.
    pub conversations: ConversationService,
    /// Variant diff aggregation service (owns repos + DIs `WorkspaceService`).
    pub diffs: DiffService,
    /// Variant PR-link resolution service.
    pub pr: PrService,
    /// Running-agent discovery + kill service (DI'd by `agents` and `snapshot`).
    pub processes: ProcessesService,
    /// Dashboard snapshot aggregation service (DIs the domain services it reads).
    pub snapshot: SnapshotService,
    /// Log-path resolution service (DIs `WorkspaceService`); `tail_lines` streams.
    pub logs: LogsService,
    pub scheduler: SchedulerHandle,
    pub loops: LoopRegistry,
    /// In-flight conversation runs; outlives any individual SSE response so
    /// clients can detach and reattach without dropping the provider process.
    pub conv_runs: ConvRunRegistry,
    /// The typed endpoint this engine is bound to. Local IPC by default; the
    /// `RemoteGrpc` seam is reserved for a future transport.
    pub endpoint: EngineEndpoint,
    /// Local IPC socket path handed to spawned agents as `MODULA_ENGINE_SOCKET`,
    /// the var the CLI resolves. Empty only for a (future) non-local endpoint.
    pub engine_socket: String,
    /// Workspace event broadcast bus. `EventService::publish` broadcasts here
    /// after persisting to the DB; gRPC watch streams subscribe. Cloneable;
    /// backed by `Arc`.
    pub bus: Bus,
}

impl AppState {
    /// Build paths, open the DB (running migrations), and start the cron
    /// scheduler. Call once per engine process.
    pub async fn new(endpoint: EngineEndpoint) -> anyhow::Result<Self> {
        let engine_socket = endpoint
            .as_local_ipc()
            .map(|ipc| ipc.path().to_string_lossy().into_owned())
            .unwrap_or_default();
        let paths = Arc::new(Paths::from_env()?);
        let db = modula_db::open(&paths.modula.join("db.sqlite")).await?;
        let repos = Repositories::new(&db);
        let loops = LoopRegistry::default();
        let bus = Bus::new();
        let events = EventService::new(
            db.clone(),
            repos.events.clone(),
            repos.workspaces.clone(),
            bus.clone(),
        );
        let event_sink: Arc<dyn EventSink> = Arc::new(events.clone());
        let threads = ThreadService::new(
            db.clone(),
            repos.tasks.clone(),
            repos.variants.clone(),
            repos.threads.clone(),
            event_sink.clone(),
        );
        let labels = LabelService::new(db.clone(), repos.labels.clone(), event_sink.clone());
        let integrations = IntegrationsService::new(db.clone(), repos.integrations.clone());
        let providers = ProviderService::new(
            db.clone(),
            repos.providers.clone(),
            paths.clone(),
            event_sink.clone(),
        );
        let scheduler = SchedulerHandle::start(
            paths.modula.clone(),
            loops.clone(),
            engine_socket.clone(),
            event_sink.clone(),
            repos.clone(),
        )
        .await?;
        scheduler.reconfigure().await?;
        let workspaces = WorkspaceService::new(
            db.clone(),
            repos.workspaces.clone(),
            paths.clone(),
            scheduler.clone(),
        );
        let tasks = TaskService::new(
            db.clone(),
            repos.tasks.clone(),
            repos.variants.clone(),
            repos.roadmap.clone(),
            repos.pipeline.clone(),
            repos.labels.clone(),
            repos.agents.clone(),
            repos.task_agent_settings.clone(),
            repos.threads.clone(),
            repos.agent_runs.clone(),
            workspaces.clone(),
            event_sink.clone(),
        );
        let projects = ProjectService::new(db.clone(), repos.projects.clone(), repos.tasks.clone());
        let config = ConfigService::new(
            db.clone(),
            repos.workspaces.clone(),
            repos.settings.clone(),
            repos.pipeline.clone(),
            repos.providers.clone(),
            repos.projects.clone(),
            repos.agents.clone(),
        );
        let processes =
            ProcessesService::new(db.clone(), repos.agent_processes.clone(), loops.clone());
        let logs = LogsService::new(workspaces.clone());
        let agents = AgentService::new(
            repos.clone(),
            scheduler.clone(),
            loops.clone(),
            event_sink.clone(),
            engine_socket.clone(),
            processes.clone(),
            workspaces.clone(),
        );
        let runs = RunService::new(db.clone(), repos.agent_runs.clone(), workspaces.clone());
        let conversations =
            ConversationService::new(db.clone(), repos.conversations.clone(), event_sink);
        let diffs = DiffService::new(
            db.clone(),
            repos.tasks.clone(),
            repos.variants.clone(),
            repos.projects.clone(),
            workspaces.clone(),
        );
        let pr = PrService::new(
            db.clone(),
            repos.tasks.clone(),
            repos.variants.clone(),
            repos.projects.clone(),
        );
        let snapshot = SnapshotService::new(
            tasks.clone(),
            config.clone(),
            runs.clone(),
            conversations.clone(),
            processes.clone(),
            workspaces.clone(),
        );
        Ok(Self {
            paths,
            repos,
            tasks,
            threads,
            labels,
            integrations,
            agents,
            providers,
            workspaces,
            projects,
            config,
            events,
            runs,
            conversations,
            diffs,
            pr,
            processes,
            snapshot,
            logs,
            scheduler,
            loops,
            conv_runs: ConvRunRegistry::default(),
            endpoint,
            engine_socket,
            bus,
        })
    }
}
