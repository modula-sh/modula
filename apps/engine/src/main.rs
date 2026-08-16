use clap::Parser;

use modula_engine::cli::{run, Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // A GUI/launchd-spawned engine inherits a minimal PATH; recover the user's
    // real one before the runtime starts (env mutation must be single-threaded).
    if matches!(cli.command, Command::Engine { .. }) {
        modula_engine::platform::enrich_path_from_user_env();
    }
    modula_engine::init_tracing();
    tokio::runtime::Runtime::new()?.block_on(run(cli))
}
