//! Autostart/service-management strategy. Distinct from process lifecycle:
//! macOS uses launchd, Linux uses systemd, Windows uses a per-user registry Run
//! key, so this trait splits three ways where [`super::ProcessManager`] splits
//! two. Only the trait is declared here; the per-OS impls land in phase 4.

/// Install/load/unload the engine as a login-time service. `install` writes the
/// platform's service definition (plist / unit / registry entry); `load` and
/// `unload` register and deregister it with the OS supervisor.
pub trait ServiceManager: Send + Sync {
    fn install(&self) -> std::io::Result<()>;
    fn load(&self) -> std::io::Result<()>;
    fn unload(&self) -> std::io::Result<()>;
}
