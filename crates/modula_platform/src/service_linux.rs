//! Linux autostart via a systemd **user** unit. `install` writes the unit into
//! `~/.config/systemd/user`; `load`/`unload` enable+start / disable+stop it with
//! `systemctl --user`. Matches modula's per-user, admin-free model; the journal
//! captures stdout/stderr, so no log-file redirection is needed.

use std::path::PathBuf;
use std::process::Command;

use super::service::ServiceManager;

const UNIT: &str = "modula-engine.service";

pub struct SystemdServiceManager;

fn unit_path() -> std::io::Result<PathBuf> {
    let home = super::home_dir()
        .ok_or_else(|| std::io::Error::other("could not determine home directory"))?;
    Ok(home.join(".config").join("systemd").join("user").join(UNIT))
}

impl ServiceManager for SystemdServiceManager {
    fn install(&self) -> std::io::Result<()> {
        let unit_path = unit_path()?;
        let binary = std::env::current_exe()?;
        std::fs::create_dir_all(unit_path.parent().unwrap())?;
        let unit = format!(
            "[Unit]\n\
             Description=Modula engine\n\n\
             [Service]\n\
             ExecStart={} engine\n\
             Restart=no\n\n\
             [Install]\n\
             WantedBy=default.target\n",
            binary.display(),
        );
        std::fs::write(&unit_path, unit)?;
        systemctl(&["daemon-reload"])
    }

    fn load(&self) -> std::io::Result<()> {
        systemctl(&["enable", "--now", UNIT])
    }

    fn unload(&self) -> std::io::Result<()> {
        systemctl(&["disable", "--now", UNIT])
    }
}

fn systemctl(args: &[&str]) -> std::io::Result<()> {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map(|_| ())
}
