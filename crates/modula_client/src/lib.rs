//! `modula-client` — the one engine client both the CLI and the Tauri backend
//! use. [`ModulaClient`] owns a single IPC `Channel`, exposes methods that take
//! client-owned request structs (not protos) and return `modula_types` domain
//! types, converting protos to/from domain at the edge. No caller threads
//! `modula_rpc` proto types or ad-hoc JSON through itself anymore.
//!
//! The channel is dialed lazily and cached, reconnecting itself after a
//! transient drop, so the long-lived desktop and the short-lived CLI share the
//! same construction path.
//!
//! [`ModulaClient::call_raw`] and [`ModulaClient::stream_raw`] are the one
//! intentional exception to "methods return `modula_types`": they pass encoded
//! bytes through by gRPC path for a plugin that tunnels calls; only the remote
//! plugin's dispatcher should use them.

mod agent;
mod codec;
mod config;
mod conversation;
mod diff;
mod error;
mod event;
mod health;
mod integration;
mod label;
mod log;
mod project;
mod provider;
mod raw;
mod request;
mod roadmap;
mod run;
mod snapshot;
mod task;
mod thread;
mod usage;
mod wiki;
mod workspace;

pub use agent::{CreatedAgent, KillOutcome, TriggeredAgent};
pub use error::{rpc as rpc_error, ClientError};
pub use project::CreatedProject;
pub use provider::CreatedProvider;
pub use request::{
    AppendEntry, CreateProvider, CreateTask, SetRoadmapStatus, UpdateProvider, UpdateTask,
    UpsertTask, WriteAgent,
};
pub use roadmap::RoadmapStatus;
pub use task::{CreatedTask, CreatedVariant, ResetOutcome, UpsertOutcome};
pub use wiki::WikiFile;
pub use workspace::CreatedWorkspace;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use modula_engine_transport::LocalIpcEndpoint;
use modula_rpc::v1::{
    agent_service_client::AgentServiceClient, config_service_client::ConfigServiceClient,
    conversation_service_client::ConversationServiceClient, diff_service_client::DiffServiceClient,
    event_service_client::EventServiceClient, health_service_client::HealthServiceClient,
    integration_service_client::IntegrationServiceClient, label_service_client::LabelServiceClient,
    log_service_client::LogServiceClient, project_service_client::ProjectServiceClient,
    provider_service_client::ProviderServiceClient, roadmap_service_client::RoadmapServiceClient,
    run_service_client::RunServiceClient, task_service_client::TaskServiceClient,
    thread_service_client::ThreadServiceClient, usage_service_client::UsageServiceClient,
    variant_service_client::VariantServiceClient, wiki_service_client::WikiServiceClient,
    workspace_service_client::WorkspaceServiceClient,
};
use tonic::transport::Channel;

/// Decode cap for `ConfigService`, whose assembled document (pipeline +
/// providers + projects + agents) can exceed tonic's 4 MB default — mirrors the
/// server's raised limit for the same service.
const MAX_CONFIG_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct ModulaClient(Arc<Inner>);

struct Inner {
    endpoint: LocalIpcEndpoint,
    channel: Mutex<Option<Channel>>,
}

impl ModulaClient {
    /// Resolve the local IPC endpoint (`--socket` > `MODULA_ENGINE_SOCKET` >
    /// per-user default). No connection is made here; the engine may not be up.
    pub fn connect(socket: Option<PathBuf>) -> Result<Self, ClientError> {
        let endpoint = LocalIpcEndpoint::resolve(socket)?;
        Ok(Self(Arc::new(Inner {
            endpoint,
            channel: Mutex::new(None),
        })))
    }

    pub fn endpoint(&self) -> &LocalIpcEndpoint {
        &self.0.endpoint
    }

    /// A connected channel, dialing and caching on first use. A failed dial is
    /// not cached, so a later call retries once the engine is up. Public so a
    /// plugin can build a client for its own services on the same connection.
    pub async fn channel(&self) -> Result<Channel, ClientError> {
        if let Some(channel) = self.0.channel.lock().unwrap().clone() {
            return Ok(channel);
        }
        let channel = self.0.endpoint.connect().await?;
        *self.0.channel.lock().unwrap() = Some(channel.clone());
        Ok(channel)
    }

    async fn tasks(&self) -> Result<TaskServiceClient<Channel>, ClientError> {
        Ok(TaskServiceClient::new(self.channel().await?))
    }

    async fn variants(&self) -> Result<VariantServiceClient<Channel>, ClientError> {
        Ok(VariantServiceClient::new(self.channel().await?))
    }

    async fn roadmap(&self) -> Result<RoadmapServiceClient<Channel>, ClientError> {
        Ok(RoadmapServiceClient::new(self.channel().await?))
    }

    async fn threads(&self) -> Result<ThreadServiceClient<Channel>, ClientError> {
        Ok(ThreadServiceClient::new(self.channel().await?))
    }

    async fn workspaces(&self) -> Result<WorkspaceServiceClient<Channel>, ClientError> {
        Ok(WorkspaceServiceClient::new(self.channel().await?))
    }

    async fn health(&self) -> Result<HealthServiceClient<Channel>, ClientError> {
        Ok(HealthServiceClient::new(self.channel().await?))
    }

    async fn config_client(&self) -> Result<ConfigServiceClient<Channel>, ClientError> {
        Ok(ConfigServiceClient::new(self.channel().await?)
            .max_decoding_message_size(MAX_CONFIG_MESSAGE_SIZE))
    }

    async fn agents(&self) -> Result<AgentServiceClient<Channel>, ClientError> {
        Ok(AgentServiceClient::new(self.channel().await?))
    }

    async fn providers(&self) -> Result<ProviderServiceClient<Channel>, ClientError> {
        Ok(ProviderServiceClient::new(self.channel().await?))
    }

    async fn projects(&self) -> Result<ProjectServiceClient<Channel>, ClientError> {
        Ok(ProjectServiceClient::new(self.channel().await?))
    }

    async fn labels(&self) -> Result<LabelServiceClient<Channel>, ClientError> {
        Ok(LabelServiceClient::new(self.channel().await?))
    }

    async fn integrations(&self) -> Result<IntegrationServiceClient<Channel>, ClientError> {
        Ok(IntegrationServiceClient::new(self.channel().await?))
    }

    async fn wiki(&self) -> Result<WikiServiceClient<Channel>, ClientError> {
        Ok(WikiServiceClient::new(self.channel().await?))
    }

    async fn usage(&self) -> Result<UsageServiceClient<Channel>, ClientError> {
        Ok(UsageServiceClient::new(self.channel().await?))
    }

    async fn conversations(&self) -> Result<ConversationServiceClient<Channel>, ClientError> {
        Ok(ConversationServiceClient::new(self.channel().await?))
    }

    async fn diffs(&self) -> Result<DiffServiceClient<Channel>, ClientError> {
        Ok(DiffServiceClient::new(self.channel().await?))
    }

    async fn events(&self) -> Result<EventServiceClient<Channel>, ClientError> {
        Ok(EventServiceClient::new(self.channel().await?))
    }

    async fn runs(&self) -> Result<RunServiceClient<Channel>, ClientError> {
        Ok(RunServiceClient::new(self.channel().await?))
    }

    async fn logs(&self) -> Result<LogServiceClient<Channel>, ClientError> {
        Ok(LogServiceClient::new(self.channel().await?))
    }
}
