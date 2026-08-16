//! `modula` subcommand surface — argument parsing + dispatch.
//!
//! `Engine`/`Status`/`Install` run the server and host management; the
//! `Task`/`Variant`/`Comment`/`Config` families are a thin gRPC client over the
//! already-running engine ([`transport`]) — the surface every spawned agent uses
//! instead of `curl`. Reads print formatted plain text ([`format`]); writes take
//! a single JSON-string body. A failed CRUD command prints `error: <detail>` to
//! stderr and exits non-zero.

mod commands;
mod format;
mod transport;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use transport::EngineTransport;

#[derive(Parser)]
#[command(name = "modula", version, about = "Modula engine + CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// Target workspace by id or slug, overriding `$MODULA_WORKSPACE` for the
    /// task / variant / comment / config commands.
    #[arg(
        long = "workspace",
        visible_alias = "ws",
        global = true,
        value_name = "ID|SLUG"
    )]
    pub workspace: Option<String>,
    /// Override the engine IPC socket/pipe path for both `engine` (serve) and
    /// the client commands. Falls back to `MODULA_ENGINE_SOCKET`, then the
    /// default per-user runtime path.
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the gRPC engine over local IPC (Unix socket / Windows named pipe).
    /// Opens zero TCP listeners by default.
    Engine {
        /// DEV ONLY, INSECURE: also serve gRPC over loopback TCP at this address
        /// (e.g. `127.0.0.1:9101`). No auth/TLS; local development only.
        #[arg(long, value_name = "ADDR")]
        grpc_tcp: Option<std::net::SocketAddr>,
        /// Allow a non-loopback `--grpc-tcp` address. Without it a non-loopback
        /// bind is refused.
        #[arg(long, hide = true)]
        grpc_tcp_allow_remote: bool,
    },
    /// Print engine health + workspaces list over the IPC endpoint.
    Status,
    /// Register the engine to start at login (launchd on macOS, systemd --user
    /// on Linux, a registry Run key on Windows). Optional — the desktop app
    /// launches the engine itself; use this to also run it headless at login.
    Install,
    /// Link this binary as `modula` on your PATH (symlink on macOS/Linux, a shim
    /// on Windows). `scripts/dev.sh` runs this every dev launch so the terminal
    /// `modula` tracks your latest build; a shipped app links itself on update.
    LinkCli,
    /// Read and write tasks (the engine endpoint comes from `--socket` /
    /// `$MODULA_ENGINE_SOCKET`; the workspace from `--workspace` /
    /// `$MODULA_WORKSPACE`).
    #[command(subcommand)]
    Task(TaskCmd),
    /// Read the workspace roadmap (task ordering, pipeline status,
    /// dependencies, notes) — the standalone view `task list` used to fetch
    /// implicitly.
    #[command(subcommand)]
    Roadmap(RoadmapCmd),
    /// Register and transition task variants.
    #[command(subcommand)]
    Variant(VariantCmd),
    /// Read and append task / variant thread entries.
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Read the workspace config (pipeline keys, projects, providers, agents).
    #[command(subcommand)]
    Config(ConfigCmd),
    /// List and create workspaces (not workspace-scoped — these ignore
    /// `$MODULA_WORKSPACE` / `--workspace`).
    #[command(subcommand)]
    Workspace(WorkspaceCmd),
}

#[derive(Subcommand)]
pub enum TaskCmd {
    /// List every task (id, external_id, title, source, pipeline status,
    /// approved, max_variants, variants summary).
    List,
    /// Show one task in full, including its pipeline status, variants, labels.
    Get {
        /// Task UUID.
        task: String,
    },
    /// Create an internal task, or upsert an external one. Body is a JSON
    /// object: `{"title":"…"}` for internal; add `"external_id"` + `"source"`
    /// (jira|linear|github) to upsert from a scanner.
    Create {
        /// JSON object body.
        body: String,
    },
    /// Patch a task. Routing depends on the body keys: a body containing any of
    /// `status`, `notes`, `depends_on` advances the roadmap (pipeline status);
    /// any other body (`approved`, `max_variants`, `worktree`, `description`,
    /// `title`) edits the task row.
    Patch {
        /// Task UUID.
        task: String,
        /// JSON object body.
        body: String,
    },
}

#[derive(Subcommand)]
pub enum RoadmapCmd {
    /// List every roadmap row in order (task, pipeline status, depends_on, notes).
    List,
}

#[derive(Subcommand)]
pub enum VariantCmd {
    /// Show one variant (status, position) and its owning task.
    Get {
        /// Variant UUID.
        variant: String,
    },
    /// Register N variants on a task (no status until promoted). Body is
    /// `{"count":N}`.
    Create {
        /// Task UUID.
        task: String,
        /// JSON object body, e.g. `{"count":2}`.
        body: String,
    },
    /// Transition a variant. Body is `{"status":"…"}` or
    /// `{"action":"accept"}` / `{"action":"rework"}` (code-reviewer verdict).
    Patch {
        /// Variant UUID.
        variant: String,
        /// JSON object body.
        body: String,
    },
}

#[derive(Subcommand)]
pub enum CommentCmd {
    /// List a task's thread: task-scoped entries then each variant's thread.
    List {
        /// Task UUID.
        task: String,
    },
    /// Append a thread entry. Body carries `author`, `kind`
    /// (comment|question|verdict|rework), `content`, and — for variant-scoped
    /// entries — `variant` + `round`; verdicts add `verdict` (+
    /// `affected_variants` for KICK_BACK).
    Create {
        /// Task UUID.
        task: String,
        /// JSON object body.
        body: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Print the workspace config (limits, pipeline, providers, projects, agents).
    Get,
}

#[derive(Subcommand)]
pub enum WorkspaceCmd {
    /// List every workspace (id, name, slug, description).
    List,
    /// Show one workspace by id or slug.
    Get {
        /// Workspace UUID or slug.
        workspace: String,
    },
    /// Create a workspace. Body is `{"name":"…"}` (+ optional `"description"`).
    Create {
        /// JSON object body.
        body: String,
    },
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        command,
        workspace,
        socket,
    } = cli;
    match command {
        Command::Engine {
            grpc_tcp,
            grpc_tcp_allow_remote,
        } => {
            crate::server::serve(crate::server::ServeOptions {
                socket,
                grpc_tcp,
                grpc_tcp_allow_remote,
            })
            .await
        }
        Command::Install => install(),
        Command::LinkCli => link_cli(),
        Command::Status => die_on_err(status(socket).await),
        client_cmd => die_on_err(dispatch(socket, workspace, client_cmd).await),
    }
}

/// Connect to the running engine, resolve the workspace for the scoped command
/// families, and run the requested CRUD command. The global `workspace` family
/// never reads the workspace, so it skips resolution.
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
    crate::platform::service_manager().install()?;
    println!("registered the modula engine to start at login.");
    Ok(())
}

/// Symlink/shim this running binary as `modula` on PATH, unconditionally — the
/// dev script calls it every launch so a rebuild is immediately on PATH. The
/// version-gated production path lives in the desktop shell, not here.
fn link_cli() -> anyhow::Result<()> {
    let target = std::env::current_exe()?;
    match crate::platform::cli_linker().ensure_linked(&target)? {
        crate::platform::LinkOutcome::Linked(path) => {
            println!("linked modula -> {}", path.display());
        }
        crate::platform::LinkOutcome::NeedsPath(path) => {
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
