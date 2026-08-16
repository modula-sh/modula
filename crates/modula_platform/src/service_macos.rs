//! macOS autostart via a launchd **user agent**. `install` writes the plist into
//! `~/Library/LaunchAgents`; `load`/`unload` register and deregister it with the
//! per-user `launchctl`. This is the launchd logic previously inline in `cli.rs`.

use std::path::PathBuf;
use std::process::Command;

use super::service::ServiceManager;

const LABEL: &str = "com.modula.engine";

pub struct LaunchdServiceManager;

fn home() -> std::io::Result<PathBuf> {
    super::home_dir().ok_or_else(|| std::io::Error::other("could not determine home directory"))
}

fn plist_path() -> std::io::Result<PathBuf> {
    Ok(home()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

impl ServiceManager for LaunchdServiceManager {
    fn install(&self) -> std::io::Result<()> {
        let plist_path = plist_path()?;
        let binary = std::env::current_exe()?;
        std::fs::create_dir_all(plist_path.parent().unwrap())?;
        let logs_dir = home()?.join(".modula").join("logs");
        std::fs::create_dir_all(&logs_dir)?;
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>engine</string>
    </array>
    <key>StandardOutPath</key>
    <string>{}/engine.log</string>
    <key>StandardErrorPath</key>
    <string>{}/engine.err.log</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
            binary.display(),
            logs_dir.display(),
            logs_dir.display(),
        );
        std::fs::write(&plist_path, plist)
    }

    fn load(&self) -> std::io::Result<()> {
        launchctl("load")
    }

    fn unload(&self) -> std::io::Result<()> {
        launchctl("unload")
    }
}

fn launchctl(action: &str) -> std::io::Result<()> {
    Command::new("launchctl")
        .arg(action)
        .arg(plist_path()?)
        .output()
        .map(|_| ())
}
