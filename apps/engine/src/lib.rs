//! Modula engine — gRPC server over local IPC + CLI subcommands.
//!
//! `main.rs` is a thin entrypoint; this lib is the composition root: it
//! registers the plugins, then either serves or hands off to the CLI.

pub mod server;

pub use modula_plugin::{Plugin, PluginContext, PluginMetadata, PluginRegistry};

/// Parse the CLI — including any subcommand a plugin grafts on — and run it.
pub fn run() -> anyhow::Result<()> {
    let mut registry = PluginRegistry::new();
    registry.register(modula_plugin_remote::RemotePlugin::default());
    let matches = modula_cli::command(&registry).get_matches();
    // A GUI/launchd-spawned engine inherits a minimal PATH; recover the user's
    // real one before the runtime starts (env mutation must be single-threaded).
    if matches.subcommand_name() == Some("engine") {
        modula_platform::enrich_path_from_user_env();
    }
    init_tracing();
    tokio::runtime::Runtime::new()?.block_on(async move {
        match modula_cli::run(registry.clone(), matches).await? {
            modula_cli::Outcome::Done => Ok(()),
            modula_cli::Outcome::ServeEngine {
                socket,
                grpc_tcp,
                grpc_tcp_allow_remote,
            } => {
                server::serve(server::ServeOptions {
                    registry,
                    socket,
                    grpc_tcp,
                    grpc_tcp_allow_remote,
                })
                .await
            }
        }
    })
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "modula_engine=info".into()),
        )
        .init();
}
