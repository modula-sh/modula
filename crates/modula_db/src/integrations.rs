//! Integration config rows. Identity = `(workspace_id, id)` where `id` is one
//! of the fixed integration ids (`github`, `jira`, `linear`); `data` holds the
//! integration's form config as JSON TEXT.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use modula_types::Integration;
use serde_json::{json, Value as Json};
use sqlx::{Executor, Sqlite};

use crate::{Error, Result};

/// Raw `integrations` columns. Private serialization detail: the repository
/// decodes the JSON TEXT `data` column into the [`Integration`] domain type at
/// its boundary.
#[derive(Debug, Clone, sqlx::FromRow)]
struct IntegrationRecord {
    id: String,
    data: String,
}

impl From<IntegrationRecord> for Integration {
    fn from(r: IntegrationRecord) -> Self {
        Integration {
            id: r.id,
            data: serde_json::from_str(&r.data).unwrap_or_else(|_| json!({})),
        }
    }
}

#[derive(Clone, Default)]
pub struct IntegrationRepository;

impl IntegrationRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Integration>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, IntegrationRecord>(
            "SELECT id, data FROM integrations WHERE workspace_id = ? ORDER BY id",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Integration::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Option<Integration>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, IntegrationRecord>(
            "SELECT id, data FROM integrations WHERE workspace_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Integration::from))
    }

    pub async fn upsert<'e, E>(&self, exec: E, ws_id: &str, id: &str, data: &Json) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT INTO integrations (workspace_id, id, data) VALUES (?, ?, ?) \
             ON CONFLICT(workspace_id, id) DO UPDATE SET data = excluded.data",
        )
        .bind(ws_id)
        .bind(id)
        .bind(data.to_string())
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM integrations WHERE workspace_id = ? AND id = ?")
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("no {id} integration connected")));
        }
        Ok(())
    }
}
