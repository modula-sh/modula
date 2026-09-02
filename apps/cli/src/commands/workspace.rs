//! `workspace` subcommand: list / get / create. These call the global
//! `WorkspaceService`, so they're not workspace-scoped and ignore `--workspace`
//! / `$MODULA_WORKSPACE`. `get` accepts an id or a slug.

use anyhow::Result;

use super::Body;
use crate::transport::EngineTransport;
use crate::{format, WorkspaceCmd};

pub async fn workspace(tx: &EngineTransport, cmd: WorkspaceCmd) -> Result<()> {
    match cmd {
        WorkspaceCmd::List => {
            let workspaces = tx.client().list_workspaces().await?;
            print!("{}", format::workspace_list(&workspaces));
        }
        WorkspaceCmd::Get { workspace } => {
            let view = tx.workspace_by_ref(&workspace).await?;
            print!("{}", format::workspace_detail(&view));
        }
        WorkspaceCmd::Create { body } => {
            let body = Body::parse(&body)?;
            let resp = tx
                .client()
                .create_workspace(
                    &body.string("name").unwrap_or_default(),
                    body.string("description"),
                )
                .await?;
            println!("created workspace: {} ({})", resp.id, resp.name);
        }
    }
    Ok(())
}
