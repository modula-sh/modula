//! `comment` subcommand: list a task's thread / append a thread entry.

use anyhow::Result;
use modula_client::AppendEntry;

use super::Body;
use crate::transport::EngineTransport;
use crate::{format, CommentCmd};

pub async fn comment(tx: &EngineTransport, cmd: CommentCmd) -> Result<()> {
    let ws = tx.workspace_id().to_string();
    match cmd {
        CommentCmd::List { task } => {
            let bundle = tx.client().get_threads(&ws, &task).await?;
            print!("{}", format::threads(&bundle));
        }
        CommentCmd::Create { task, body } => {
            let body = Body::parse(&body)?;
            let author = body
                .string("author")
                .ok_or_else(|| anyhow::anyhow!("comment body needs an \"author\""))?;
            let kind = body
                .string("kind")
                .ok_or_else(|| anyhow::anyhow!("comment body needs a \"kind\""))?;
            let entry = tx
                .client()
                .append_entry(AppendEntry {
                    workspace_id: ws,
                    task_id: task,
                    content: body.string("content").unwrap_or_default(),
                    author,
                    kind,
                    variant_id: body.string("variant"),
                    round: body.int("round"),
                    verdict: body.string("verdict"),
                    affected_variants: body.strings("affected_variants"),
                })
                .await?;
            println!(
                "posted {} by {} (entry {})",
                entry.kind, entry.author, entry.id
            );
        }
    }
    Ok(())
}
