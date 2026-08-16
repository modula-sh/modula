//! Pipeline status rows. Read-only API for now; the default 11 statuses are
//! seeded on workspace create. Editing the pipeline is a future endpoint.

use modula_types::PipelineStatus;
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::Result;

/// Raw `pipeline_statuses` columns. Private serialization detail: the repository
/// maps it into the [`PipelineStatus`] domain type at its boundary. The `position`
/// column drives `ORDER BY` in SQL only, so it is not selected into the record —
/// the returned `Vec` is already ordered.
#[derive(Debug, Clone, sqlx::FromRow)]
struct PipelineRecord {
    key: String,
    label: String,
    tone: String,
    station: Option<String>,
    terminal: bool,
    error: bool,
}

impl From<PipelineRecord> for PipelineStatus {
    fn from(r: PipelineRecord) -> Self {
        PipelineStatus {
            key: r.key,
            label: r.label,
            tone: r.tone,
            station: r.station,
            terminal: r.terminal,
            error: r.error,
        }
    }
}

/// Default pipeline shipped with every new workspace. Order is the rendered
/// column order; `terminal`/`error` flags drive frontend coloring.
const DEFAULT: &[PipelineSeed] = &[
    PipelineSeed {
        key: "planning",
        label: "Planning",
        tone: "zinc",
        station: Some("PLAN"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "ready_for_research",
        label: "Ready / Research",
        tone: "blue",
        station: Some("RESEARCH"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "researching",
        label: "Researching",
        tone: "yellow",
        station: Some("RESEARCH"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "needs_clarification",
        label: "Needs Clarification",
        tone: "orange",
        station: Some("RESEARCH"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "ready_for_workers",
        label: "Ready / Workers",
        tone: "blue",
        station: Some("BUILD"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "in_progress",
        label: "In Progress",
        tone: "purple",
        station: Some("BUILD"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "ready_for_review",
        label: "Ready / Review",
        tone: "blue",
        station: Some("REVIEW"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "in_review",
        label: "In Review",
        tone: "yellow",
        station: Some("REVIEW"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "ready_for_acceptance",
        label: "Ready / Accept",
        tone: "blue",
        station: Some("ACCEPTED"),
        terminal: false,
        error: false,
    },
    PipelineSeed {
        key: "accepted",
        label: "Accepted",
        tone: "green",
        station: Some("ACCEPTED"),
        terminal: true,
        error: false,
    },
    PipelineSeed {
        key: "blocked",
        label: "Blocked",
        tone: "red",
        station: None,
        terminal: false,
        error: true,
    },
];

struct PipelineSeed {
    key: &'static str,
    label: &'static str,
    tone: &'static str,
    station: Option<&'static str>,
    terminal: bool,
    error: bool,
}

pub(crate) async fn seed_defaults(conn: &mut SqliteConnection, ws_id: &str) -> Result<()> {
    for (i, row) in DEFAULT.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pipeline_statuses \
             (workspace_id, position, key, label, tone, station, terminal, error) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(i as i64)
        .bind(row.key)
        .bind(row.label)
        .bind(row.tone)
        .bind(row.station)
        .bind(row.terminal as i64)
        .bind(row.error as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct PipelineRepository;

impl PipelineRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<PipelineStatus>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, PipelineRecord>(
            "SELECT key, label, tone, station, terminal, error \
             FROM pipeline_statuses WHERE workspace_id = ? ORDER BY position",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(PipelineStatus::from)
        .collect())
    }
}
