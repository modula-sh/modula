//! Windows autostart via a per-user **registry Run key**
//! (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). This matches the
//! `currentUser` NSIS install model: admin-free, scoped to the logged-in user.
//!
//! `install` writes the value (the OS then launches the engine at every login);
//! `load`/`unload` are no-ops, because a Run key is not a live supervisor — its
//! mere presence is the autostart, and removing it (uninstall) is the only
//! deregistration. There is no per-session register/start handshake as on launchd.

use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

use super::service::ServiceManager;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Modula";

pub struct RegistryServiceManager;

/// UTF-16, null-terminated copy of `s` for the wide Win32 registry APIs.
fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

impl ServiceManager for RegistryServiceManager {
    fn install(&self) -> std::io::Result<()> {
        let binary = std::env::current_exe()?;
        let command = format!("\"{}\" engine", binary.display());
        let data = wide(&command);

        let subkey = wide(RUN_KEY);
        let value = wide(VALUE_NAME);
        // SAFETY: all pointers reference live, null-terminated wide buffers; the
        // byte length covers the data including its terminator.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * std::mem::size_of::<u16>()) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status as i32));
        }
        Ok(())
    }

    fn load(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn unload(&self) -> std::io::Result<()> {
        Ok(())
    }
}
