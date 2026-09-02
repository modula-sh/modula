//! `config` subcommand: read-only workspace config.

use anyhow::Result;

use crate::transport::EngineTransport;
use crate::{format, ConfigCmd};

pub async fn config(tx: &EngineTransport, cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Get => {
            let cfg = tx.client().get_config(tx.workspace_id()).await?;
            print!("{}", format::config(&cfg));
        }
    }
    Ok(())
}
