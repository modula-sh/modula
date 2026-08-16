//! Workspace settings: run limits. One row per workspace.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use modula_types::ConfigLimits;
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::Result;

/// Raw `workspace_settings` columns. Private serialization detail: the repository
/// maps it into the [`ConfigLimits`] domain type at its boundary (`i64`→`i32`).
#[derive(Debug, Clone, sqlx::FromRow)]
struct SettingsRecord {
    max_spawns_per_run: i64,
}

impl From<SettingsRecord> for ConfigLimits {
    fn from(r: SettingsRecord) -> Self {
        ConfigLimits {
            max_spawns_per_run: r.max_spawns_per_run as i32,
        }
    }
}

pub(crate) async fn seed_defaults(conn: &mut SqliteConnection, ws_id: &str) -> Result<()> {
    sqlx::query("INSERT INTO workspace_settings (workspace_id) VALUES (?)")
        .bind(ws_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

#[derive(Clone, Default)]
pub struct SettingsRepository;

impl SettingsRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str) -> Result<ConfigLimits>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, SettingsRecord>(
            "SELECT max_spawns_per_run FROM workspace_settings WHERE workspace_id = ?",
        )
        .bind(ws_id)
        .fetch_one(exec)
        .await?
        .into())
    }
}
