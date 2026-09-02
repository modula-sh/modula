//! `NotImplemented` stand-in for the proprietary Modula Remote plugin. The
//! public build registers it so the engine and desktop compile and link against
//! a stable API; `scripts/swap-plugins.sh` overwrites this crate with the real
//! one.

/// Whether this build has a working remote implementation. The desktop hides
/// the panel when it is `false`.
pub const AVAILABLE: bool = false;

pub mod client;
pub mod types;

use std::sync::Arc;

use modula_plugin::{Plugin, PluginMetadata, PluginRegistry};

/// Fieldless rather than a unit struct so `RemotePlugin::default()` reads the
/// same here as in the real crate, which carries state.
#[derive(Default)]
pub struct RemotePlugin {}

impl Plugin for RemotePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "remote",
            version: env!("CARGO_PKG_VERSION"),
            description: "Modula Remote: not available in this build",
        }
    }

    /// Migrators ignore versions they do not own, so a DB built by the real
    /// plugin still opens here.
    fn register(self: Arc<Self>, _registry: &mut PluginRegistry) {}
}
