//! `modula` subcommand surface — argument parsing + dispatch.
//!
//! `Engine`/`Status`/`Install` run the server and host management; the
//! `Task`/`Variant`/`Comment`/`Config` families are a thin gRPC client over the
//! already-running engine ([`transport`]) — the surface every spawned agent uses
//! instead of `curl`. Reads print formatted plain text ([`format`]); writes take
//! a single JSON-string body. `Workspace` is host-global: it ignores the
//! workspace. A failed CRUD command prints `error: <detail>` to stderr and
//! exits non-zero.
//!
//! Plugins graft their own subcommands on in [`command`]; those are matched
//! before this enum is parsed, so a plugin name must not shadow a core one.

mod commands;
mod format;
mod transport;

use std::path::PathBuf;

use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};
use modula_plugin::PluginRegistry;

use transport::EngineTransport;

const EXAMPLES: &str = "\
Examples:
  modula status                       check the engine is up
  modula task list --ws my-project    list a workspace's tasks
  modula task get <TASK_ID>           show one task in full
  modula task patch <TASK_ID> '{\"status\":\"in_review\"}'

Docs: https://docs.modula.sh";

#[derive(Parser)]
#[command(
    name = "modula",
    version,
    about = "Turn tasks into shipped code with agents",
    long_about = "Turn tasks into shipped code with agents.\n\nOne binary: the local engine, and the client commands you and your agents drive it with.",
    after_help = EXAMPLES
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// Workspace to act on, by id or slug [env: MODULA_WORKSPACE]
    #[arg(
        long = "workspace",
        visible_alias = "ws",
        global = true,
        value_name = "ID|SLUG"
    )]
    pub workspace: Option<String>,
    /// Engine socket or pipe path [env: MODULA_ENGINE_SOCKET]
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Read and write tasks
    #[command(subcommand)]
    Task(TaskCmd),
    /// Show the workspace roadmap
    ///
    /// Task order, pipeline status, dependencies and notes.
    #[command(subcommand)]
    Roadmap(RoadmapCmd),
    /// Register and transition task variants
    #[command(subcommand)]
    Variant(VariantCmd),
    /// Read and append task threads
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Show the workspace config
    ///
    /// Limits, pipeline keys, projects, providers and agents.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// List and create workspaces
    ///
    /// Host-global: these ignore `--workspace` / `$MODULA_WORKSPACE`.
    #[command(subcommand)]
    Workspace(WorkspaceCmd),
    /// Check the engine and list workspaces
    Status,
    /// Run the engine
    ///
    /// Serves gRPC over local IPC only — a Unix socket, or a Windows named
    /// pipe. No TCP listener unless `--grpc-tcp` is passed.
    Engine {
        /// Also serve gRPC over loopback TCP — dev only
        ///
        /// Insecure: no auth, no TLS. Example: `127.0.0.1:9101`.
        #[arg(long, value_name = "ADDR")]
        grpc_tcp: Option<std::net::SocketAddr>,
        /// Allow a non-loopback `--grpc-tcp` address. Without it a non-loopback
        /// bind is refused.
        #[arg(long, hide = true)]
        grpc_tcp_allow_remote: bool,
    },
    /// Run the engine at login
    ///
    /// Registers a login item: launchd on macOS, systemd --user on Linux, a
    /// registry Run key on Windows. Optional — the desktop app starts the
    /// engine itself.
    Install,
    /// Link this binary as `modula` on your PATH
    ///
    /// A symlink on macOS/Linux, a shim on Windows. Dev builds relink every
    /// launch; a shipped app relinks on update.
    LinkCli,
}

#[derive(Subcommand)]
pub enum TaskCmd {
    /// List all tasks
    List,
    /// Show one task in full
    Get {
        /// Task UUID
        task: String,
    },
    /// Create a task, or upsert an external one
    ///
    /// Body is `{"title":"…"}`. Add `"external_id"` and `"source"`
    /// (jira|linear|github) to upsert from a scanner.
    Create {
        /// JSON object body
        body: String,
    },
    /// Update a task
    ///
    /// A body carrying `status`, `notes` or `depends_on` advances the roadmap;
    /// any other body (`approved`, `max_variants`, `worktree`, `title`,
    /// `description`) edits the task row.
    Patch {
        /// Task UUID
        task: String,
        /// JSON object body
        body: String,
    },
}

#[derive(Subcommand)]
pub enum RoadmapCmd {
    /// List roadmap rows in order
    List,
}

#[derive(Subcommand)]
pub enum VariantCmd {
    /// Show one variant and the task that owns it
    Get {
        /// Variant UUID
        variant: String,
    },
    /// Register variants on a task
    ///
    /// Body is `{"count":N}`. New variants have no status until promoted.
    Create {
        /// Task UUID
        task: String,
        /// JSON object body
        body: String,
    },
    /// Transition a variant
    ///
    /// Body is `{"status":"…"}`, or `{"action":"accept"}` /
    /// `{"action":"rework"}` for a code-reviewer verdict.
    Patch {
        /// Variant UUID
        variant: String,
        /// JSON object body
        body: String,
    },
}

#[derive(Subcommand)]
pub enum CommentCmd {
    /// List a task's thread
    ///
    /// Task-scoped entries first, then each variant's thread.
    List {
        /// Task UUID
        task: String,
    },
    /// Append a thread entry
    ///
    /// Body carries `author`, `kind` (comment|question|verdict|rework) and
    /// `content`; variant-scoped entries add `variant` and `round`; verdicts
    /// add `verdict`, plus `affected_variants` for KICK_BACK.
    Create {
        /// Task UUID
        task: String,
        /// JSON object body
        body: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Print the workspace config
    Get,
}

#[derive(Subcommand)]
pub enum WorkspaceCmd {
    /// List all workspaces
    List,
    /// Show one workspace
    Get {
        /// Workspace UUID or slug
        workspace: String,
    },
    /// Create a workspace
    ///
    /// Body is `{"name":"…"}`, plus an optional `"description"`.
    Create {
        /// JSON object body
        body: String,
    },
}

/// The `modula` command tree, with every plugin's subcommands grafted on.
pub fn command(registry: &PluginRegistry) -> clap::Command {
    registry
        .clis()
        .fold(Cli::command(), |cmd, cli| cli.command(cmd))
}

/// What the caller must still do. Serving is the engine's job, so `run` reports
/// the `engine` subcommand rather than starting a server itself.
pub enum Outcome {
    Done,
    ServeEngine {
        socket: Option<PathBuf>,
        grpc_tcp: Option<std::net::SocketAddr>,
        grpc_tcp_allow_remote: bool,
    },
}

/// Dispatch parsed arguments. A plugin's subcommand is handed straight to it,
/// before the [`Cli`] parse that cannot represent it.
pub async fn run(registry: PluginRegistry, matches: ArgMatches) -> anyhow::Result<Outcome> {
    if let Some((name, sub)) = matches.subcommand() {
        if let Some(cli) = registry.cli_owner(name) {
            let socket = matches.get_one::<PathBuf>("socket").cloned();
            let client = modula_client::ModulaClient::connect(socket)?;
            die_on_err(cli.run(&client, sub).await)?;
            return Ok(Outcome::Done);
        }
    }
    dispatch_core(Cli::from_arg_matches(&matches)?).await
}

async fn dispatch_core(cli: Cli) -> anyhow::Result<Outcome> {
    let Cli {
        command,
        workspace,
        socket,
    } = cli;
    match command {
        Command::Engine {
            grpc_tcp,
            grpc_tcp_allow_remote,
        } => Ok(Outcome::ServeEngine {
            socket,
            grpc_tcp,
            grpc_tcp_allow_remote,
        }),
        Command::Install => install().map(|_| Outcome::Done),
        Command::LinkCli => link_cli().map(|_| Outcome::Done),
        Command::Status => die_on_err(status(socket).await).map(|_| Outcome::Done),
        client_cmd => {
            die_on_err(dispatch(socket, workspace, client_cmd).await).map(|_| Outcome::Done)
        }
    }
}

/// Connect to the running engine, resolve the workspace for the scoped command
/// families, and run the requested CRUD command. The host-global `workspace`
/// family never reads the workspace, so it skips resolution.
async fn dispatch(
    socket: Option<PathBuf>,
    workspace: Option<String>,
    command: Command,
) -> anyhow::Result<()> {
    let mut tx = EngineTransport::connect(socket)?;
    if !matches!(command, Command::Workspace(_)) {
        tx.resolve_workspace(workspace.as_deref()).await?;
    }
    match command {
        Command::Task(cmd) => commands::task(&tx, cmd).await,
        Command::Roadmap(cmd) => commands::roadmap(&tx, cmd).await,
        Command::Variant(cmd) => commands::variant(&tx, cmd).await,
        Command::Comment(cmd) => commands::comment(&tx, cmd).await,
        Command::Config(cmd) => commands::config(&tx, cmd).await,
        Command::Workspace(cmd) => commands::workspace(&tx, cmd).await,
        _ => unreachable!("non-client commands are handled in run()"),
    }
}

/// CRUD commands surface their own one-line error and exit non-zero, so the
/// shared `main` doesn't double-print it with anyhow's `Error:` prefix.
fn die_on_err(result: anyhow::Result<()>) -> anyhow::Result<()> {
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    Ok(())
}

/// `modula status` — the gRPC health check over IPC that doubles as the
/// "is the engine up?" probe, plus the workspace list.
async fn status(socket: Option<PathBuf>) -> anyhow::Result<()> {
    let client = modula_client::ModulaClient::connect(socket)?;
    if !client.is_serving().await {
        println!("engine: {} (not_serving)", client.endpoint());
        return Ok(());
    }
    println!("engine: {} (serving)", client.endpoint());

    let workspaces = client.list_workspaces().await?;
    print!("{}", format::workspace_list(&workspaces));
    Ok(())
}

fn install() -> anyhow::Result<()> {
    modula_platform::service_manager().install()?;
    println!("registered the modula engine to start at login.");
    Ok(())
}

/// Symlink/shim this running binary as `modula` on PATH, unconditionally — the
/// dev script calls it every launch so a rebuild is immediately on PATH. The
/// version-gated production path lives in the desktop shell, not here.
fn link_cli() -> anyhow::Result<()> {
    let target = std::env::current_exe()?;
    match modula_platform::cli_linker().ensure_linked(&target)? {
        modula_platform::LinkOutcome::Linked(path) => {
            println!("linked modula -> {}", path.display());
        }
        modula_platform::LinkOutcome::NeedsPath(path) => {
            let dir = path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            println!(
                "linked modula -> {} (add {dir} to your PATH to use `modula`)",
                path.display()
            );
        }
    }
    Ok(())
}
