//! `roadmap` subcommand: the standalone roadmap view. `task list` still renders
//! each task's pipeline status, but the roadmap (dependencies + notes, ordered)
//! is its own command so the fetch is no longer hidden behind `task list`.

use anyhow::Result;

use crate::transport::EngineTransport;
use crate::{format, RoadmapCmd};

pub async fn roadmap(tx: &EngineTransport, cmd: RoadmapCmd) -> Result<()> {
    match cmd {
        RoadmapCmd::List => {
            let entries = tx.client().list_roadmap(tx.workspace_id()).await?;
            print!("{}", format::roadmap_list(&entries));
        }
    }
    Ok(())
}
