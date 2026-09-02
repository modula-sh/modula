//! `variant` subcommand: get / create / patch. `get`/`patch` take only a
//! variant id; the owning task is resolved through the transport.

use anyhow::{anyhow, Result};

use super::Body;
use crate::transport::EngineTransport;
use crate::{format, VariantCmd};

pub async fn variant(tx: &EngineTransport, cmd: VariantCmd) -> Result<()> {
    let ws = tx.workspace_id().to_string();
    match cmd {
        VariantCmd::Get { variant } => {
            let owner = tx.task_owning_variant(&variant).await?;
            let view = owner
                .variants
                .iter()
                .find(|v| v.id == variant)
                .ok_or_else(|| anyhow!("variant {variant} vanished from its owning task"))?;
            print!("{}", format::variant_detail(view, &owner));
        }
        VariantCmd::Create { task, body } => {
            let count = Body::parse(&body)?.int("count").unwrap_or(0).max(0) as u32;
            let created = tx.client().create_variants(&ws, &task, count).await?;
            if created.is_empty() {
                println!("created 0 variants");
            } else {
                for v in created {
                    println!("created variant: {} (position {})", v.id, v.position);
                }
            }
        }
        VariantCmd::Patch { variant, body } => {
            let body = Body::parse(&body)?;
            let owner = tx.task_owning_variant(&variant).await?;
            let status = tx
                .client()
                .update_variant(
                    &ws,
                    &owner.id,
                    &variant,
                    body.string("status"),
                    body.string("action"),
                )
                .await?;
            println!("variant {variant} → {status}");
        }
    }
    Ok(())
}
