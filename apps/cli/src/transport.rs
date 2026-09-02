//! The CLI's connection to a running engine. `EngineTransport` is a thin wrapper
//! over [`ModulaClient`] (the one client both the CLI and the Tauri backend use):
//! it resolves the workspace the scoped commands act on and exposes the
//! client-backed lookups the commands share, returning `modula_types` domain
//! types. It never touches the DB or service layer.
//!
//! Construction is per-invocation (the CLI is short-lived); the underlying
//! channel is dialed lazily on first use.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use modula_client::ModulaClient;
use modula_types::{Task, Workspace};

pub struct EngineTransport {
    client: ModulaClient,
    /// Canonical workspace id for the scoped command families; resolved by
    /// [`Self::resolve_workspace`] before the command runs. `None` for the
    /// global `workspace` commands, which never read it.
    workspace_id: Option<String>,
}

impl EngineTransport {
    /// Resolve the local IPC endpoint (`--socket` > `MODULA_ENGINE_SOCKET` >
    /// default). The channel is dialed lazily on first call.
    pub fn connect(socket: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            client: ModulaClient::connect(socket)?,
            workspace_id: None,
        })
    }

    pub fn client(&self) -> &ModulaClient {
        &self.client
    }

    /// Resolve the workspace id the scoped commands act on. `--workspace`
    /// (`arg`) is matched against the live list by id or slug; otherwise
    /// `$MODULA_WORKSPACE` is taken as a canonical id.
    pub async fn resolve_workspace(&mut self, arg: Option<&str>) -> Result<()> {
        let id = match arg {
            Some(arg) => self.client.workspace_by_ref(arg).await?.id,
            None => std::env::var("MODULA_WORKSPACE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    anyhow!("MODULA_WORKSPACE is not set (pass --workspace <id|slug>)")
                })?,
        };
        self.workspace_id = Some(id);
        Ok(())
    }

    pub fn workspace_id(&self) -> &str {
        self.workspace_id
            .as_deref()
            .expect("resolve_workspace runs before any scoped command")
    }

    /// Match a `--workspace` / `workspace get` argument against the live list by
    /// canonical id or by the engine-supplied `slug`.
    pub async fn workspace_by_ref(&self, arg: &str) -> Result<Workspace> {
        Ok(self.client.workspace_by_ref(arg).await?)
    }

    /// Find a task by its UUID within the resolved workspace.
    pub async fn task_by_id(&self, id: &str) -> Result<Task> {
        Ok(self.client.task_by_id(self.workspace_id(), id).await?)
    }

    /// Resolve the task that owns a variant; `variant get`/`patch` take only a
    /// variant id, so the client scans every task's `variants[]` for the owner.
    pub async fn task_owning_variant(&self, variant: &str) -> Result<Task> {
        Ok(self
            .client
            .task_owning_variant(self.workspace_id(), variant)
            .await?)
    }
}
