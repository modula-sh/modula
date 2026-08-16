//! Provider rows. Identity = `(workspace_id, id)` where id is a UUID.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use modula_types::Provider;
use serde_json::Map;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::{Error, Result};

const DEFAULT_PROVIDER_NAME: &str = "Claude";
const DEFAULT_PROVIDER_TYPE: &str = "claude";
const DEFAULT_PROVIDER_DIR: &str = "~/.claude";
const DEFAULT_PROVIDER_DESC: &str = "Default Claude provider.";

/// Seed the default provider and return its UUID.
pub(crate) async fn seed_default(conn: &mut SqliteConnection, ws_id: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO providers (workspace_id, id, name, type, config_dir, description) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(ws_id)
    .bind(&id)
    .bind(DEFAULT_PROVIDER_NAME)
    .bind(DEFAULT_PROVIDER_TYPE)
    .bind(DEFAULT_PROVIDER_DIR)
    .bind(DEFAULT_PROVIDER_DESC)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}

/// Raw `providers` columns. Private serialization detail: the repository maps
/// it into the [`Provider`] domain type at its boundary. The enriched fields
/// (`config_dir_exists`, `mcp_*`, `agents_using`) no single row carries are left
/// empty for the owning `ProviderService` to fill.
#[derive(Debug, Clone, sqlx::FromRow)]
struct ProviderRecord {
    id: String,
    name: String,
    r#type: String,
    config_dir: String,
    description: Option<String>,
}

impl From<ProviderRecord> for Provider {
    fn from(r: ProviderRecord) -> Self {
        Provider {
            id: r.id,
            name: r.name,
            r#type: r.r#type,
            description: r.description,
            config_dir: r.config_dir,
            config_dir_exists: false,
            mcp_server_count: 0,
            mcp_endpoints: Vec::new(),
            agents_using: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_summary: Map::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ProviderRepository;

impl ProviderRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Provider>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, ProviderRecord>(
            "SELECT id, name, type, config_dir, description \
             FROM providers WHERE workspace_id = ? ORDER BY name",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Provider::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Provider>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, ProviderRecord>(
            "SELECT id, name, type, config_dir, description \
             FROM providers WHERE workspace_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Provider::from)
        .ok_or_else(|| Error::NotFound(format!("unknown provider: {id}")))
    }

    /// Create a provider and return its UUID.
    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        name: &str,
        r#type: &str,
        config_dir: &str,
        description: Option<&str>,
    ) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO providers (workspace_id, id, name, type, config_dir, description) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(name)
        .bind(r#type)
        .bind(config_dir)
        .bind(description)
        .execute(exec)
        .await?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn patch<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        name: Option<&str>,
        r#type: Option<&str>,
        config_dir: Option<&str>,
        description: Option<Option<String>>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE providers SET ");
        let mut sets = qb.separated(", ");
        if let Some(n) = name {
            sets.push("name = ").push_bind_unseparated(n);
        }
        if let Some(t) = r#type {
            sets.push("type = ").push_bind_unseparated(t);
        }
        if let Some(d) = config_dir {
            sets.push("config_dir = ").push_bind_unseparated(d);
        }
        if let Some(desc) = description {
            sets.push("description = ")
                .push_bind_unseparated(desc.filter(|s| !s.is_empty()));
        }
        // Nothing set — no-op.
        if qb.sql().ends_with("SET ") {
            return Ok(());
        }
        qb.push(" WHERE workspace_id = ")
            .push_bind(ws_id)
            .push(" AND id = ")
            .push_bind(id);
        let res = qb.build().execute(exec).await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown provider: {id}")));
        }
        Ok(())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM providers WHERE workspace_id = ? AND id = ?")
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown provider: {id}")));
        }
        Ok(())
    }

    pub async fn agents_using<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        provider_id: &str,
    ) -> Result<Vec<String>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_scalar(
            "SELECT name FROM agents WHERE workspace_id = ? AND provider_id = ? ORDER BY name",
        )
        .bind(ws_id)
        .bind(provider_id)
        .fetch_all(exec)
        .await?)
    }
}
