use std::time::Duration;

use modula_rpc::v1::{
    agent_service_server::AgentServiceServer, config_service_server::ConfigServiceServer,
    conversation_service_server::ConversationServiceServer, diff_service_server::DiffServiceServer,
    event_service_server::EventServiceServer, health_service_server::HealthServiceServer,
    integration_service_server::IntegrationServiceServer, label_service_server::LabelServiceServer,
    log_service_server::LogServiceServer, project_service_server::ProjectServiceServer,
    provider_service_server::ProviderServiceServer, roadmap_service_server::RoadmapServiceServer,
    run_service_server::RunServiceServer, snapshot_service_server::SnapshotServiceServer,
    task_service_server::TaskServiceServer, thread_service_server::ThreadServiceServer,
    usage_service_server::UsageServiceServer, variant_service_server::VariantServiceServer,
    wiki_service_server::WikiServiceServer, workspace_service_server::WorkspaceServiceServer,
};
use tonic::transport::server::Router;

use crate::state::AppState;

/// Decode/encode cap for unary services whose payload can exceed tonic's 4 MB
/// default — the assembled config and the unary snapshot fetch. Diffs, logs,
/// and the streaming snapshot avoid this via chunked server streams (phase-5).
const MAX_UNARY_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Keep idle watch streams (events, run status, conversation attach) healthy
/// over IPC by pinging on a fixed interval.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

mod agent;
mod chunk;
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
mod roadmap;
mod run;
mod snapshot;
mod task;
mod thread;
mod usage;
mod wiki;
mod workspace;

pub use agent::AgentHandler;
pub use config::ConfigHandler;
pub use conversation::ConversationHandler;
pub use diff::DiffHandler;
pub use event::EventHandler;
pub use health::HealthHandler;
pub use integration::IntegrationHandler;
pub use label::LabelHandler;
pub use log::LogHandler;
pub use project::ProjectHandler;
pub use provider::ProviderHandler;
pub use roadmap::RoadmapHandler;
pub use run::RunHandler;
pub use snapshot::SnapshotHandler;
pub use task::{TaskHandler, VariantHandler};
pub use thread::ThreadHandler;
pub use usage::UsageHandler;
pub use wiki::WikiHandler;
pub use workspace::WorkspaceHandler;

/// Build the combined tonic Router for all engine services.
pub fn make_router(state: AppState) -> Router {
    tonic::transport::Server::builder()
        .http2_keepalive_interval(Some(KEEPALIVE_INTERVAL))
        .add_service(HealthServiceServer::new(HealthHandler {
            state: state.clone(),
        }))
        .add_service(WorkspaceServiceServer::new(WorkspaceHandler {
            state: state.clone(),
        }))
        .add_service(
            ConfigServiceServer::new(ConfigHandler {
                state: state.clone(),
            })
            .max_decoding_message_size(MAX_UNARY_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_UNARY_MESSAGE_SIZE),
        )
        .add_service(TaskServiceServer::new(TaskHandler {
            state: state.clone(),
        }))
        .add_service(VariantServiceServer::new(VariantHandler {
            state: state.clone(),
        }))
        .add_service(ThreadServiceServer::new(ThreadHandler {
            state: state.clone(),
        }))
        .add_service(LabelServiceServer::new(LabelHandler {
            state: state.clone(),
        }))
        .add_service(IntegrationServiceServer::new(IntegrationHandler {
            state: state.clone(),
        }))
        .add_service(RoadmapServiceServer::new(RoadmapHandler {
            state: state.clone(),
        }))
        .add_service(AgentServiceServer::new(AgentHandler {
            state: state.clone(),
        }))
        .add_service(ProviderServiceServer::new(ProviderHandler {
            state: state.clone(),
        }))
        .add_service(ProjectServiceServer::new(ProjectHandler {
            state: state.clone(),
        }))
        .add_service(UsageServiceServer::new(UsageHandler {
            state: state.clone(),
        }))
        .add_service(RunServiceServer::new(RunHandler {
            state: state.clone(),
        }))
        .add_service(EventServiceServer::new(EventHandler {
            state: state.clone(),
        }))
        .add_service(ConversationServiceServer::new(ConversationHandler {
            state: state.clone(),
        }))
        .add_service(LogServiceServer::new(LogHandler {
            state: state.clone(),
        }))
        .add_service(DiffServiceServer::new(DiffHandler {
            state: state.clone(),
        }))
        .add_service(
            SnapshotServiceServer::new(SnapshotHandler {
                state: state.clone(),
            })
            .max_encoding_message_size(MAX_UNARY_MESSAGE_SIZE),
        )
        .add_service(WikiServiceServer::new(WikiHandler { state }))
}
