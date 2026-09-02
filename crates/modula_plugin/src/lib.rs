//! The plugin seam: the [`Plugin`] trait, the [`PluginService`] /
//! [`PluginGrpc`] / [`PluginCli`] components it registers, and the
//! [`PluginRegistry`] the engine drives them through. See `docs/PLUGINS.md`.

pub mod bus;

use std::path::PathBuf;
use std::sync::Arc;

use modula_db::Database;

/// Re-exported so a plugin implements the traits against one dependency.
pub use async_trait::async_trait;
pub use bus::{Bus, BusEvent};
pub use clap::ArgMatches;
pub use modula_client::ModulaClient;
/// The shared domain error, converted at the gRPC edge with `Status::from`.
pub use modula_rpc::status::DomainError;
pub use sqlx::migrate::{MigrateError, Migrator};
pub use tonic::transport::server::Router;

pub type Result<T> = std::result::Result<T, DomainError>;

/// Identity for logs and `modula status`. `name` is the registry key.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

/// The engine-owned runtime handed to [`PluginService::init`]. Only
/// process-wide handles: `modula-db`'s repositories are stateless structs.
#[derive(Clone)]
pub struct PluginContext {
    /// Already migrated: core schema, then each registered migrator in
    /// registration order.
    pub db: Database,
    /// Live delivery; the durable record is the `events` table.
    pub bus: Bus,
    /// `~/.modula` — the root for any file a plugin owns.
    pub modula_dir: PathBuf,
    /// This engine's IPC socket/pipe, empty for a non-local endpoint. Dial it
    /// with `ModulaClient::connect` rather than reaching into services.
    pub engine_socket: String,
}

impl PluginContext {
    /// A client for this engine's own gRPC surface.
    pub fn client(&self) -> anyhow::Result<ModulaClient> {
        Ok(ModulaClient::connect(
            (!self.engine_socket.is_empty()).then(|| PathBuf::from(&self.engine_socket)),
        )?)
    }
}

/// One compiled-in feature. A plugin *is* its components: it implements
/// whichever of [`PluginService`], [`PluginGrpc`] and [`PluginCli`] it needs.
pub trait Plugin: Send + Sync + 'static {
    fn metadata(&self) -> PluginMetadata;

    /// Claim the seams this plugin fills, by handing clones of `self` to the
    /// registry. Runs before the database is open.
    fn register(self: Arc<Self>, registry: &mut PluginRegistry);
}

/// Long-lived state and background work.
#[async_trait]
pub trait PluginService: Send + Sync + 'static {
    /// Build state from the engine runtime. Runs before the router, so the
    /// gRPC seam sees it initialized.
    async fn init(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    /// Background work. The listener is bound, so IPC back into the engine
    /// is available.
    async fn start(&self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// gRPC services to graft onto the engine's router.
pub trait PluginGrpc: Send + Sync + 'static {
    /// Also where per-service tuning like `max_encoding_message_size` belongs.
    fn add_to(&self, router: Router) -> Router;
}

/// Subcommands grafted onto `modula`. Runs in the short-lived CLI process,
/// which reaches the engine over gRPC like any other client.
#[async_trait]
pub trait PluginCli: Send + Sync + 'static {
    /// The top-level subcommand names [`Self::command`] added. Matched before
    /// the engine's own parse, so they must not shadow a core command.
    fn names(&self) -> &'static [&'static str];

    fn command(&self, cmd: clap::Command) -> clap::Command;

    async fn run(&self, client: &ModulaClient, matches: &ArgMatches) -> anyhow::Result<()>;
}

/// Everything the composition root put together, indexed by seam. One plugin
/// may appear in several of these lists.
#[derive(Default, Clone)]
pub struct PluginRegistry {
    plugins: Vec<PluginMetadata>,
    /// Constructors, not values: `Migrator` is not `Clone`; this registry is.
    migrations: Vec<fn() -> Migrator>,
    services: Vec<Arc<dyn PluginService>>,
    grpc: Vec<Arc<dyn PluginGrpc>>,
    clis: Vec<Arc<dyn PluginCli>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The composition root's entrypoint. Panics on a duplicate name — two
    /// plugins sharing a key is a composition-root bug.
    pub fn register(&mut self, plugin: impl Plugin) -> &mut Self {
        let plugin = Arc::new(plugin);
        let meta = plugin.metadata();
        assert!(
            !self.plugins.iter().any(|p| p.name == meta.name),
            "plugin {} is registered twice",
            meta.name
        );
        self.plugins.push(meta);
        plugin.register(self);
        self
    }

    /// Schema this plugin owns. Versions must not collide with the core set or
    /// another plugin's.
    pub fn migrations(&mut self, migrator: fn() -> Migrator) -> &mut Self {
        self.migrations.push(migrator);
        self
    }

    pub fn service(&mut self, service: Arc<dyn PluginService>) -> &mut Self {
        self.services.push(service);
        self
    }

    pub fn grpc(&mut self, grpc: Arc<dyn PluginGrpc>) -> &mut Self {
        self.grpc.push(grpc);
        self
    }

    pub fn cli(&mut self, cli: Arc<dyn PluginCli>) -> &mut Self {
        self.clis.push(cli);
        self
    }

    pub fn plugins(&self) -> &[PluginMetadata] {
        &self.plugins
    }

    /// Apply every registered schema to an already-core-migrated pool.
    /// `ignore_missing` because all migrators share one `_sqlx_migrations` table.
    pub async fn migrate(&self, db: &Database) -> std::result::Result<(), MigrateError> {
        for build in &self.migrations {
            let mut migrator = build();
            migrator.set_ignore_missing(true);
            migrator.run(db).await?;
        }
        Ok(())
    }

    pub fn services(&self) -> impl Iterator<Item = &Arc<dyn PluginService>> {
        self.services.iter()
    }

    pub fn grpc_services(&self) -> impl Iterator<Item = &Arc<dyn PluginGrpc>> {
        self.grpc.iter()
    }

    /// The CLI component that claimed a top-level subcommand, if any.
    pub fn cli_owner(&self, name: &str) -> Option<&Arc<dyn PluginCli>> {
        self.clis.iter().find(|c| c.names().contains(&name))
    }

    pub fn clis(&self) -> impl Iterator<Item = &Arc<dyn PluginCli>> {
        self.clis.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One type, both seams — the shape a real plugin uses.
    struct Stub(&'static str);

    impl Plugin for Stub {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                name: self.0,
                version: "0",
                description: "",
            }
        }

        fn register(self: Arc<Self>, registry: &mut PluginRegistry) {
            registry.cli(self);
        }
    }

    #[async_trait]
    impl PluginCli for Stub {
        fn names(&self) -> &'static [&'static str] {
            &["stub"]
        }
        fn command(&self, cmd: clap::Command) -> clap::Command {
            cmd
        }
        async fn run(&self, _c: &ModulaClient, _m: &ArgMatches) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_registry_finds_the_cli_that_claimed_a_subcommand() {
        let mut registry = PluginRegistry::new();
        registry.register(Stub("a"));
        assert!(registry.cli_owner("stub").is_some());
        assert!(registry.cli_owner("task").is_none());
    }

    #[test]
    #[should_panic(expected = "registered twice")]
    fn a_duplicate_name_is_a_composition_bug() {
        let mut registry = PluginRegistry::new();
        registry.register(Stub("a"));
        registry.register(Stub("a"));
    }
}
