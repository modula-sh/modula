//! `task` subcommand: list / get / create (or upsert) / patch.

use anyhow::Result;
use modula_client::{CreateTask, SetRoadmapStatus, UpdateTask, UpsertTask};

use super::Body;
use crate::cli::transport::EngineTransport;
use crate::cli::{format, TaskCmd};

pub async fn task(tx: &EngineTransport, cmd: TaskCmd) -> Result<()> {
    let ws = tx.workspace_id().to_string();
    match cmd {
        TaskCmd::List => {
            let tasks = tx.client().list_tasks(&ws).await?;
            let roadmap = tx.client().list_roadmap(&ws).await?;
            print!("{}", format::task_list(&tasks, &roadmap));
        }
        TaskCmd::Get { task } => {
            let view = tx.task_by_id(&task).await?;
            let roadmap = tx.client().list_roadmap(&ws).await?;
            let row = roadmap.iter().find(|r| r.task == task);
            print!("{}", format::task_detail(&view, row));
        }
        TaskCmd::Create { body } => create(tx, &ws, &body).await?,
        TaskCmd::Patch { task, body } => patch(tx, &ws, &task, &body).await?,
    }
    Ok(())
}

/// A body carrying `external_id` + `source` is an external upsert (from a
/// scanner); anything else is an internal create that mints the display id.
async fn create(tx: &EngineTransport, ws: &str, body: &str) -> Result<()> {
    let body = Body::parse(body)?;
    if body.has("external_id") && body.has("source") {
        let resp = tx
            .client()
            .upsert_task(UpsertTask {
                workspace_id: ws.to_string(),
                external_id: body.string("external_id").unwrap_or_default(),
                source: body.string("source").unwrap_or_default(),
                title: body.string("title").unwrap_or_default(),
                description: body.string("description"),
                source_data: body.json("source_data"),
                status: body.string("status"),
                url: body.string("url"),
                synced_at: body.string("synced_at"),
                approved: body.boolean("approved"),
                max_variants: body.int("max_variants"),
                worktree: body.boolean("worktree"),
            })
            .await?;
        let verb = if resp.upserted == "updated" {
            "updated"
        } else {
            "created"
        };
        println!("{verb} task: {} ({})", resp.id, resp.external_id);
    } else {
        let resp = tx
            .client()
            .create_task(CreateTask {
                workspace_id: ws.to_string(),
                title: body.string("title").unwrap_or_default(),
                description: body.string("description"),
                approved: body.boolean("approved"),
                max_variants: body.int("max_variants"),
                worktree: body.boolean("worktree"),
                source_data: body.json("source_data"),
            })
            .await?;
        println!("created task: {} ({})", resp.id, resp.external_id);
    }
    Ok(())
}

/// A body addressing `status` / `notes` / `depends_on` advances the roadmap;
/// any other body edits the task row. The two key sets are disjoint in practice.
async fn patch(tx: &EngineTransport, ws: &str, task: &str, body: &str) -> Result<()> {
    let body = Body::parse(body)?;
    let touches_roadmap = body.has("status") || body.has("notes") || body.has("depends_on");
    if touches_roadmap {
        let resp = tx
            .client()
            .set_roadmap_status(SetRoadmapStatus {
                workspace_id: ws.to_string(),
                task_id: task.to_string(),
                status: body.string("status").unwrap_or_default(),
                depends_on: body.strings("depends_on"),
                notes: body.string("notes"),
            })
            .await?;
        println!("task {task}: pipeline_status → {}", resp.status);
    } else {
        let id = tx
            .client()
            .update_task(UpdateTask {
                workspace_id: ws.to_string(),
                task_id: task.to_string(),
                approved: body.boolean("approved"),
                max_variants: body.int("max_variants"),
                worktree: body.boolean("worktree"),
                description: body.string("description"),
                title: body.string("title"),
            })
            .await?;
        println!("patched task: {id}");
    }
    Ok(())
}
