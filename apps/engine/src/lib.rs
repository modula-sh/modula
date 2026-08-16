//! Modula engine — gRPC server over local IPC + CLI subcommands.
//!
//! `main.rs` is a thin entrypoint; this lib owns every module and exposes
//! the CLI dispatcher (`cli::run`) so tests or embedders can drive the
//! engine without going through the binary.

pub mod cli;
pub mod core;
pub mod grpc;
pub use modula_platform as platform;
pub mod server;
pub mod services;
pub mod state;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "modula_engine=info".into()),
        )
        .init();
}
