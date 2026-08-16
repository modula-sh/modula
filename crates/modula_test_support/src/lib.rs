//! E2E test harness — boots the engine binary against a tempdir-rooted
//! workspace root over a unique per-test local IPC socket, prepends provider
//! shims (`claude`, `opencode`, `codex` → mock-claude) to PATH, and hands out
//! connected gRPC service clients.
//!
//! Build artifacts are picked up from the workspace target dir. Each test gets
//! its own socket and tempdir, so they can run in parallel with zero TCP
//! listeners — the default engine transport.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use modula_engine_transport::LocalIpcEndpoint;
use modula_rpc::v1::{
    agent_service_client::AgentServiceClient, config_service_client::ConfigServiceClient,
    conversation_service_client::ConversationServiceClient,
    event_service_client::EventServiceClient, health_service_client::HealthServiceClient,
    label_service_client::LabelServiceClient, provider_service_client::ProviderServiceClient,
    roadmap_service_client::RoadmapServiceClient, run_service_client::RunServiceClient,
    snapshot_service_client::SnapshotServiceClient, task_service_client::TaskServiceClient,
    thread_service_client::ThreadServiceClient, variant_service_client::VariantServiceClient,
    workspace_service_client::WorkspaceServiceClient, CreateWorkspaceRequest, HealthCheckRequest,
    HealthStatus,
};
use serde_json::Value as Json;
use tempfile::TempDir;
use tonic::transport::Channel;

/// Decode cap for the unary services whose assembled document can exceed
/// tonic's 4 MB default (`ConfigService`, `SnapshotService`) — mirrors the
/// server's raised limit for those services.
const MAX_LARGE_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

pub struct Harness {
    /// The local IPC socket/pipe the engine is bound to.
    pub socket: PathBuf,
    pub modula_dir: PathBuf,
    channel: Channel,
    /// Workspace UUID → on-disk dir (`<modula>/<slug>`), as reported by the
    /// engine on creation. The slug, not the UUID, names the directory, and the
    /// engine sets it as the cwd for every spawn — so tests must read and write
    /// workspace files here, matching production.
    ws_dirs: Mutex<HashMap<String, PathBuf>>,
    _tempdir: TempDir,
    _shim_dir: TempDir,
    engine: Child,
}

impl Harness {
    pub async fn start() -> Result<Self> {
        Self::start_with_env(&[]).await
    }

    /// Start with extra environment variables for the engine process — useful
    /// for tests that want a faster dispatcher tick, for example.
    pub async fn start_with_env(extra_env: &[(&str, &str)]) -> Result<Self> {
        ensure_binaries_built()?;
        let tempdir = TempDir::new()?;
        let modula_dir = tempdir.path().join("modula");
        std::fs::create_dir_all(&modula_dir)?;

        let shim_dir = TempDir::new()?;
        install_provider_shims(shim_dir.path())?;

        let socket = unique_socket();
        let engine_bin = modula_bin();
        if !engine_bin.exists() {
            anyhow::bail!(
                "engine binary not found at {} — `cargo build` must have failed",
                engine_bin.display()
            );
        }

        // Prepend the shim dir to PATH using the OS path separator (`:` / `;`).
        let mut search = vec![shim_dir.path().to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            search.extend(std::env::split_paths(&existing));
        }
        let path_env = std::env::join_paths(search)?;

        let log_stdout = std::env::var("MODULA_TEST_ENGINE_STDOUT").is_ok();
        let (stdout, stderr) = if log_stdout {
            (Stdio::inherit(), Stdio::inherit())
        } else {
            (Stdio::null(), Stdio::null())
        };
        let mut cmd = Command::new(&engine_bin);
        cmd.arg("--socket")
            .arg(&socket)
            .arg("engine")
            .env("MODULA_DIR", &modula_dir)
            .env("PATH", &path_env)
            .stdout(stdout)
            .stderr(stderr);
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().context("spawn engine")?;

        let channel = match connect_healthy(&socket, Duration::from_secs(10)).await {
            Ok(channel) => channel,
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        };

        Ok(Self {
            socket,
            modula_dir,
            channel,
            ws_dirs: Mutex::new(HashMap::new()),
            _tempdir: tempdir,
            _shim_dir: shim_dir,
            engine: child,
        })
    }

    /// A clone of the connected gRPC channel — build any service client from it.
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    /// PID of the spawned engine process. For tests that drive a real
    /// termination signal (the harness `Drop` otherwise SIGKILLs, which skips
    /// the engine's own signal-driven socket cleanup).
    pub fn engine_pid(&self) -> u32 {
        self.engine.id()
    }

    pub fn health(&self) -> HealthServiceClient<Channel> {
        HealthServiceClient::new(self.channel.clone())
    }

    pub fn workspaces(&self) -> WorkspaceServiceClient<Channel> {
        WorkspaceServiceClient::new(self.channel.clone())
    }

    pub fn config(&self) -> ConfigServiceClient<Channel> {
        ConfigServiceClient::new(self.channel.clone())
            .max_decoding_message_size(MAX_LARGE_MESSAGE_SIZE)
    }

    pub fn tasks(&self) -> TaskServiceClient<Channel> {
        TaskServiceClient::new(self.channel.clone())
    }

    pub fn variants(&self) -> VariantServiceClient<Channel> {
        VariantServiceClient::new(self.channel.clone())
    }

    pub fn providers(&self) -> ProviderServiceClient<Channel> {
        ProviderServiceClient::new(self.channel.clone())
    }

    pub fn agents(&self) -> AgentServiceClient<Channel> {
        AgentServiceClient::new(self.channel.clone())
    }

    pub fn labels(&self) -> LabelServiceClient<Channel> {
        LabelServiceClient::new(self.channel.clone())
    }

    pub fn roadmap(&self) -> RoadmapServiceClient<Channel> {
        RoadmapServiceClient::new(self.channel.clone())
    }

    pub fn events(&self) -> EventServiceClient<Channel> {
        EventServiceClient::new(self.channel.clone())
    }

    pub fn threads(&self) -> ThreadServiceClient<Channel> {
        ThreadServiceClient::new(self.channel.clone())
    }

    pub fn runs(&self) -> RunServiceClient<Channel> {
        RunServiceClient::new(self.channel.clone())
    }

    pub fn conversations(&self) -> ConversationServiceClient<Channel> {
        ConversationServiceClient::new(self.channel.clone())
    }

    pub fn snapshots(&self) -> SnapshotServiceClient<Channel> {
        SnapshotServiceClient::new(self.channel.clone())
            .max_decoding_message_size(MAX_LARGE_MESSAGE_SIZE)
    }

    /// Create a workspace and record its on-disk dir (the engine returns the
    /// slug-based `path`). Returns the workspace UUID for subsequent calls.
    pub async fn create_workspace(&self, name: &str) -> Result<String> {
        let resp = self
            .workspaces()
            .create(CreateWorkspaceRequest {
                name: name.to_string(),
                description: None,
            })
            .await?
            .into_inner();
        self.ws_dirs
            .lock()
            .unwrap()
            .insert(resp.id.clone(), PathBuf::from(resp.path));
        Ok(resp.id)
    }

    /// Write a per-agent mock recipe under <ws-dir>/mock-recipes/<agent>.json.
    pub fn write_recipe(&self, workspace: &str, agent: &str, recipe: &Json) -> Result<()> {
        let dir = self.workspace_path(workspace).join("mock-recipes");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(format!("{agent}.json")),
            serde_json::to_vec_pretty(recipe)?,
        )?;
        Ok(())
    }

    /// On-disk dir for a workspace UUID — the slug-based path the engine
    /// reported at creation. Falls back to `<modula>/<uuid>` for workspaces
    /// not created through `create_workspace`.
    pub fn workspace_path(&self, workspace: &str) -> PathBuf {
        self.ws_dirs
            .lock()
            .unwrap()
            .get(workspace)
            .cloned()
            .unwrap_or_else(|| self.modula_dir.join(workspace))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.engine.kill();
        let _ = self.engine.wait();
        // SIGKILL skips the engine's own signal-driven cleanup, so unlink the
        // UDS path ourselves. (Windows named pipes vanish with the process.)
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// A unique local IPC endpoint for one harness instance: a short UDS path on
/// Unix, a uniquely-named pipe on Windows. Kept well under `sun_path` limits by
/// living in a short-named dir under the system temp dir. The engine requires
/// the socket's parent dir to be owned by its UID (see `ipc_security`), and
/// `/tmp` itself is root-owned on Linux — so the path nests one level down,
/// letting the engine create the dir.
fn unique_socket() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    #[cfg(unix)]
    {
        std::env::temp_dir()
            .join(format!("modula-test-{pid}"))
            .join(format!("{n}.sock"))
    }
    #[cfg(windows)]
    {
        PathBuf::from(format!(r"\\.\pipe\modula-test-{pid}-{n}"))
    }
}

/// Dial the engine over IPC and poll `HealthService.Check` until it reports
/// `SERVING`, returning the live channel.
async fn connect_healthy(socket: &Path, timeout: Duration) -> Result<Channel> {
    let endpoint = LocalIpcEndpoint::new(socket);
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(channel) = endpoint.connect().await {
            let mut health = HealthServiceClient::new(channel.clone());
            if let Ok(resp) = health
                .check(HealthCheckRequest {
                    service: String::new(),
                })
                .await
            {
                if resp.into_inner().status == HealthStatus::Serving as i32 {
                    return Ok(channel);
                }
            }
        }
        if Instant::now() >= deadline {
            anyhow::bail!("engine did not become healthy on {}", socket.display());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

static BUILD_RESULT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// Build the engine + mock-claude once per test binary so changes to those
/// crates are always picked up. Run before spawning the engine.
fn ensure_binaries_built() -> Result<()> {
    let res = BUILD_RESULT.get_or_init(|| {
        let manifest = workspace_root().join("Cargo.toml");
        let status = Command::new(env!("CARGO"))
            .arg("build")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("-p")
            .arg("modula-engine")
            .arg("-p")
            .arg("modula-mock-claude")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!("cargo build exited {s}")),
            Err(e) => Err(format!("cargo build failed: {e}")),
        }
    });
    res.clone().map_err(|e| anyhow::anyhow!(e))
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <repo>/crates/modula_test_support; the workspace
    // root is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("test-support crate must sit at <workspace>/crates/modula_test_support")
        .to_path_buf()
}

fn workspace_target_dir() -> PathBuf {
    workspace_root().join("target")
}

/// Path to the compiled `modula` binary — the engine the harness spawns *and*
/// the CRUD CLI agents drive. The e2e crate can't use `CARGO_BIN_EXE_modula`
/// (Cargo only defines it for the crate that owns the bin), so tests resolve
/// the debug-build path through this helper instead.
pub fn modula_bin() -> PathBuf {
    workspace_target_dir().join("debug").join(exe("modula"))
}

/// Platform executable file name: Windows binaries carry a `.exe` suffix so a
/// `PATH` lookup (and Rust's own `Command` resolution) can find them.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Install mock-claude as `claude`, `opencode`, and `codex` under `dir`.
/// The mock keys on MODULA_AGENT_NAME for recipe lookup, not argv, so one
/// binary handles all provider types without change.
fn install_provider_shims(dir: &Path) -> Result<()> {
    let mock = workspace_target_dir()
        .join("debug")
        .join(exe("mock-claude"));
    if !mock.exists() {
        anyhow::bail!(
            "mock-claude not built at {} — run `cargo build -p mock-claude` first",
            mock.display()
        );
    }
    for name in ["claude", "opencode", "codex"] {
        let shim = dir.join(exe(name));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&mock, &shim)
            .with_context(|| format!("symlink {} -> {}", shim.display(), mock.display()))?;
        #[cfg(not(unix))]
        std::fs::copy(&mock, &shim)?;
    }
    Ok(())
}
