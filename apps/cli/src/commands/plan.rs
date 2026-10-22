//! `plan` subcommand: get / set / patch. Each takes only a variant id; the
//! owning task is resolved through the transport.

use std::io::Read;

use anyhow::{anyhow, bail, Context, Result};
use modula_types::{Plan, PlanPatch, PlanSummary, PlanTaskStatus};
use serde::Deserialize;

use crate::transport::EngineTransport;
use crate::{format, PlanCmd};

const PATCH_SHAPES: &str = "\
patch body must be exactly one of:
  {\"task\":\"1.2\",\"status\":\"in_progress|done|skipped|pending\",\"note\":\"…\"}
  {\"phase\":\"2\",\"add_task\":\"Title\"}
  {\"add_phase\":{\"title\":\"…\",\"scope\":\"…\",\"approach\":\"…\",\"notes\":\"…\",\"tasks\":[\"…\"]}}
  {\"phase\":\"1\",\"deviations\":\"…\"}
  {\"summary\":{\"kind\":\"done|rework\",\"round\":N,\"content\":\"…\"}}";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchBody {
    task: Option<String>,
    status: Option<PlanTaskStatus>,
    note: Option<String>,
    phase: Option<String>,
    add_task: Option<String>,
    deviations: Option<String>,
    add_phase: Option<AddPhase>,
    summary: Option<PlanSummary>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddPhase {
    title: String,
    scope: String,
    approach: String,
    #[serde(default)]
    notes: String,
    tasks: Vec<String>,
}

pub async fn plan(tx: &EngineTransport, cmd: PlanCmd) -> Result<()> {
    let ws = tx.workspace_id().to_string();
    match cmd {
        PlanCmd::Get { variant, md } => {
            let owner = tx.task_owning_variant(&variant).await?;
            let text = tx.client().get_plan(&ws, &owner.id, &variant).await?;
            let plan = Plan::parse(&text).map_err(|e| anyhow!(e))?;
            if md {
                print!("{}", plan.to_markdown());
            } else {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        PlanCmd::Set { variant, file } => {
            let text = match file {
                Some(path) => std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?,
                None => {
                    let mut text = String::new();
                    std::io::stdin().read_to_string(&mut text)?;
                    text
                }
            };
            let owner = tx.task_owning_variant(&variant).await?;
            let progress = tx.client().set_plan(&ws, &owner.id, &variant, text).await?;
            println!(
                "plan set for variant {variant}: {}",
                format::plan_progress(&progress)
            );
        }
        PlanCmd::Patch { variant, body } => {
            let patch = parse_patch(&body)?;
            let (noun, change) = match &patch {
                PlanPatch::SetTaskStatus { status, .. } => ("task", status.as_str()),
                PlanPatch::AddTask { .. } => ("task", "added"),
                PlanPatch::AddPhase { .. } => ("phase", "added"),
                PlanPatch::AppendDeviations { .. } => ("phase", "deviations appended"),
                PlanPatch::AddSummary(_) => ("summary", "added"),
            };
            let owner = tx.task_owning_variant(&variant).await?;
            let (id, progress) = tx
                .client()
                .patch_plan(&ws, &owner.id, &variant, patch)
                .await?;
            println!(
                "plan {variant}: {noun} {id} {change} ({})",
                format::plan_progress(&progress)
            );
        }
    }
    Ok(())
}

fn parse_patch(body: &str) -> Result<PlanPatch> {
    let b: PatchBody = serde_json::from_str(body)
        .map_err(|e| anyhow!("invalid patch body: {e}\n{PATCH_SHAPES}"))?;
    Ok(
        match (
            b.task,
            b.status,
            b.note,
            b.phase,
            b.add_task,
            b.deviations,
            b.add_phase,
            b.summary,
        ) {
            (Some(task), Some(status), note, None, None, None, None, None) => {
                PlanPatch::SetTaskStatus { task, status, note }
            }
            (None, None, None, Some(phase), Some(title), None, None, None) => {
                PlanPatch::AddTask { phase, title }
            }
            (None, None, None, Some(phase), None, Some(content), None, None) => {
                PlanPatch::AppendDeviations { phase, content }
            }
            (None, None, None, None, None, None, Some(p), None) => PlanPatch::AddPhase {
                title: p.title,
                scope: p.scope,
                approach: p.approach,
                notes: p.notes,
                tasks: p.tasks,
            },
            (None, None, None, None, None, None, None, Some(summary)) => {
                PlanPatch::AddSummary(summary)
            }
            _ => bail!("{PATCH_SHAPES}"),
        },
    )
}
