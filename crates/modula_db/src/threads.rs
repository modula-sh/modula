//! Thread entries. Append-only log; one logical thread per scope.
//! scope='task' rows have `variant_id IS NULL`; scope='variant' rows carry
//! the variant id.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use sqlx::{Executor, Sqlite};

use modula_types::{ThreadBundle, ThreadEntry};

use crate::{Error, Result};

#[derive(Debug, Clone, sqlx::FromRow)]
struct ThreadEntryRecord {
    id: i64,
    variant_id: Option<String>,
    ts: String,
    author: String,
    kind: String,
    round: Option<i64>,
    content: String,
    verdict: Option<String>,
    affected_variants: Option<String>,
}

impl From<ThreadEntryRecord> for ThreadEntry {
    fn from(r: ThreadEntryRecord) -> Self {
        let affected_variants = r
            .affected_variants
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        Self {
            id: r.id,
            ts: r.ts,
            author: r.author,
            kind: r.kind,
            round: r.round,
            content: r.content,
            verdict: r.verdict,
            affected_variants,
        }
    }
}

const SELECT_COLS: &str =
    "id, variant_id, ts, author, kind, round, content, verdict, affected_variants";

/// A thread entry that matched a search, carrying its owning task's display
/// fields — a comment hit is shown (and navigated to) as its task. See
/// [`ThreadRepository::search`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThreadMatch {
    pub task_id: String,
    pub task_title: String,
    pub task_external_id: Option<String>,
    pub author: String,
    pub kind: String,
    pub content: String,
}

#[derive(Clone, Default)]
pub struct ThreadRepository;

impl ThreadRepository {
    pub fn new() -> Self {
        Self
    }

    /// A task's full thread as the `ThreadBundle` domain aggregate: task-scoped
    /// entries (`variant_id IS NULL`) plus each variant's entries keyed by
    /// variant id. Ordered by id within each group.
    pub async fn list_for_task<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
    ) -> Result<ThreadBundle>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let rows = sqlx::query_as::<_, ThreadEntryRecord>(&format!(
            "SELECT {SELECT_COLS} FROM thread_entries \
             WHERE workspace_id = ? AND task_id = ? ORDER BY id"
        ))
        .bind(ws_id)
        .bind(task_id)
        .fetch_all(exec)
        .await?;

        let mut task_thread = Vec::new();
        let mut variant_threads: BTreeMap<String, Vec<ThreadEntry>> = BTreeMap::new();
        for r in rows {
            let variant_id = r.variant_id.clone();
            let entry = ThreadEntry::from(r);
            match variant_id {
                Some(v) => variant_threads.entry(v).or_default().push(entry),
                None => task_thread.push(entry),
            }
        }
        Ok(ThreadBundle {
            task: task_id.to_string(),
            task_thread,
            variant_threads,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: Option<&str>,
        author: &str,
        kind: &str,
        content: &str,
        round: Option<i64>,
        verdict: Option<&str>,
        affected_variants: Option<&Json>,
    ) -> Result<ThreadEntry>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let scope = if variant_id.is_some() {
            "variant"
        } else {
            "task"
        };
        let affected = affected_variants
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| Error::Internal(format!("json: {e}")))?;

        let row = sqlx::query_as::<_, ThreadEntryRecord>(&format!(
            "INSERT INTO thread_entries \
               (workspace_id, scope, task_id, variant_id, ts, author, kind, round, content, verdict, affected_variants) \
             VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?, ?, ?, ?, ?, ?) \
             RETURNING {SELECT_COLS}"
        ))
        .bind(ws_id)
        .bind(scope)
        .bind(task_id)
        .bind(variant_id)
        .bind(author)
        .bind(kind)
        .bind(round)
        .bind(content)
        .bind(verdict)
        .bind(affected)
        .fetch_one(exec)
        .await?;
        Ok(row.into())
    }

    pub async fn delete_for_task<'e, E>(&self, exec: E, ws_id: &str, task_id: &str) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM thread_entries WHERE workspace_id = ? AND task_id = ?")
            .bind(ws_id)
            .bind(task_id)
            .execute(exec)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Fetch a single entry scoped by workspace + task + id, so an id from another
    /// task or workspace can never be reached. Returns the domain entry alongside
    /// its `variant_id` (dropped by `ThreadEntry`) which the caller needs to route
    /// the edit/delete event to the right thread.
    pub async fn get<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        id: i64,
    ) -> Result<Option<(ThreadEntry, Option<String>)>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, ThreadEntryRecord>(&format!(
            "SELECT {SELECT_COLS} FROM thread_entries \
             WHERE workspace_id = ? AND task_id = ? AND id = ?"
        ))
        .bind(ws_id)
        .bind(task_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(|r| {
            let variant_id = r.variant_id.clone();
            (ThreadEntry::from(r), variant_id)
        }))
    }

    pub async fn update_content<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        id: i64,
        content: &str,
    ) -> Result<ThreadEntry>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row = sqlx::query_as::<_, ThreadEntryRecord>(&format!(
            "UPDATE thread_entries SET content = ? \
             WHERE workspace_id = ? AND task_id = ? AND id = ? RETURNING {SELECT_COLS}"
        ))
        .bind(content)
        .bind(ws_id)
        .bind(task_id)
        .bind(id)
        .fetch_one(exec)
        .await?;
        Ok(row.into())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, task_id: &str, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query(
            "DELETE FROM thread_entries WHERE workspace_id = ? AND task_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(id)
        .execute(exec)
        .await?;
        Ok(res.rows_affected() > 0)
    }
    /// Thread entries whose content matches `query`, joined to their task for
    /// the display fields. Entries outlive their task's soft delete, so the
    /// join filters those out.
    pub async fn search<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ThreadMatch>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, ThreadMatch>(
            "SELECT te.task_id, t.title AS task_title, t.external_id AS task_external_id, \
                    te.author, te.kind, te.content \
             FROM thread_entries te \
             JOIN tasks t ON t.workspace_id = te.workspace_id AND t.id = te.task_id \
             WHERE te.workspace_id = ? AND t.deleted_at IS NULL \
               AND te.content LIKE ? ESCAPE '\\' \
             ORDER BY te.id DESC LIMIT ?",
        )
        .bind(ws_id)
        .bind(crate::search::like_pattern(query))
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }
}
