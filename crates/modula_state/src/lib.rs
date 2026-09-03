use std::sync::Arc;

use modula_engine_transport::EngineEndpoint;
use modula_services::labels::LabelService;
use modula_services::tasks::TaskService;
use modula_services::threads::ThreadService;

use modula_plugin::{PluginContext, PluginRegistry};

use modula_core::paths::Paths;
use modula_core::repositories::Repositories;
use modula_services::agents::AgentService;
use modula_services::config::ConfigService;
use modula_services::conversations::{ConvRunRegistry, ConvRuntime, ConversationService};
use modula_services::diffs::DiffService;
use modula_services::events::{Bus, EventService, EventSink};
use modula_services::generate::GenerationService;
use modula_services::integrations::IntegrationsService;
use modula_services::logs::LogsService;
use modula_services::loop_registry::LoopRegistry;
use modula_services::pr::PrService;
use modula_services::processes::ProcessesService;
use modula_services::projects::ProjectService;
use modula_services::providers::ProviderService;
use modula_services::runs::RunService;
use modula_services::scheduler::SchedulerHandle;
use modula_services::search::SearchService;
use modula_services::snapshot::SnapshotService;
use modula_services::workspaces::WorkspaceService;

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
    /// One-off provider text generation for the field assist.
    pub generation: GenerationService,
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
    /// Workspace-wide search. A read-only cross-domain aggregator: it owns no
    /// transaction, writes nothing, and publishes no event.
    pub search: SearchService,
    pub scheduler: SchedulerHandle,
    pub loops: LoopRegistry,
    /// In-flight conversation runs; outlives any individual SSE response so
    /// clients can detach and reattach without dropping the provider process.
    pub conv_runs: ConvRunRegistry,
    /// What a conversation turn needs, assembled once so the service layer
    /// never reaches back up to this struct.
    pub conv: ConvRuntime,
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
    /// Build paths, open the DB, start the scheduler, and initialize every
    /// plugin. Call once per engine process.
    pub async fn new(endpoint: EngineEndpoint, registry: &PluginRegistry) -> anyhow::Result<Self> {
        let engine_socket = endpoint
            .as_local_ipc()
            .map(|ipc| ipc.path().to_string_lossy().into_owned())
            .unwrap_or_default();
        let paths = Arc::new(Paths::from_env()?);
        let db = modula_db::open(&paths.modula.join("db.sqlite")).await?;
        // Registration order, after the core schema. Every migrator ignores
        // versions it does not own; they share one `_sqlx_migrations` table.
        registry.migrate(&db).await?;
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
        let projects = ProjectService::new(
            db.clone(),
            repos.projects.clone(),
            repos.tasks.clone(),
            event_sink.clone(),
        );
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
        let generation =
            GenerationService::new(db.clone(), repos.providers.clone(), workspaces.clone());
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
        let search = SearchService::new(workspaces.clone(), &repos);
        // Plugins own their state; the engine keeps no field per plugin.
        let ctx = PluginContext {
            db: db.clone(),
            bus: bus.clone(),
            modula_dir: paths.modula.clone(),
            engine_socket: engine_socket.clone(),
        };
        for service in registry.services() {
            service
                .init(&ctx)
                .await
                .map_err(|e| anyhow::anyhow!("a plugin service failed to initialize: {e}"))?;
        }
        for plugin in registry.plugins() {
            tracing::info!("[plugin] {} {} registered", plugin.name, plugin.version);
        }

        let conv_runs = ConvRunRegistry::default();
        let conv = ConvRuntime {
            repos: repos.clone(),
            conv_runs: conv_runs.clone(),
            conversations: conversations.clone(),
            workspaces: workspaces.clone(),
            engine_socket: engine_socket.clone(),
        };
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
            generation,
            diffs,
            pr,
            processes,
            snapshot,
            logs,
            search,
            scheduler,
            loops,
            conv,
            conv_runs,
            endpoint,
            engine_socket,
            bus,
        })
    }
}
