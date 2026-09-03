//! Conversation rows. Identity = `(workspace_id, id)`. One row per chat
//! thread; the full message log lives in the `data` JSON column.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use serde::Deserialize;
use serde_json::{json, Value as Json};
use sqlx::{Executor, Sqlite};

use modula_types::{ChatMessage, Conversation};

use crate::{Error, Result};

#[derive(Debug, Clone, sqlx::FromRow)]
struct ConversationRecord {
    id: String,
    title: String,
    provider_id: String,
    model: Option<String>,
    context: String,
    session_id: Option<String>,
    data: String,
    created_at: String,
    updated_at: String,
}

fn message_from_json(v: &Json) -> ChatMessage {
    let s = |k: &str| {
        v.get(k)
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string()
    };
    ChatMessage {
        role: s("role"),
        content: s("content"),
        ts: s("ts"),
    }
}

impl From<ConversationRecord> for Conversation {
    fn from(r: ConversationRecord) -> Self {
        let context = serde_json::from_str(&r.context).unwrap_or_else(|_| json!({}));
        let data: Json = serde_json::from_str(&r.data).unwrap_or_else(|_| json!({}));
        let messages = data
            .get("messages")
            .and_then(Json::as_array)
            .map(|arr| arr.iter().map(message_from_json).collect())
            .unwrap_or_default();
        Self {
            id: r.id,
            title: r.title,
            provider_id: r.provider_id,
            model: r.model,
            context,
            session_id: r.session_id,
            messages,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const SELECT_COLS: &str =
    "id, title, provider_id, model, context, session_id, data, created_at, updated_at";

#[derive(Debug, Deserialize)]
pub struct ConversationCreate {
    pub id: String,
    pub title: Option<String>,
    pub provider_id: String,
    pub model: Option<String>,
    pub context: serde_json::Value,
}

/// `data` is the raw transcript JSON; `LIKE` also sees its envelope, so the
/// caller must re-check the decoded messages.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationMatch {
    pub id: String,
    pub title: String,
    pub data: String,
}

#[derive(Clone, Default)]
pub struct ConversationRepository;

impl ConversationRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Conversation>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let rows = sqlx::query_as::<_, ConversationRecord>(&format!(
            "SELECT {SELECT_COLS} FROM conversations WHERE workspace_id = ? ORDER BY updated_at DESC, id DESC"
        ))
        .bind(ws_id)
        .fetch_all(exec)
        .await?;
        Ok(rows.into_iter().map(Conversation::from).collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Conversation>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, ConversationRecord>(&format!(
            "SELECT {SELECT_COLS} FROM conversations WHERE workspace_id = ? AND id = ?"
        ))
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Conversation::from)
        .ok_or_else(|| Error::NotFound(format!("unknown conversation: {id}")))
    }

    pub async fn create<'e, E>(&self, exec: E, ws_id: &str, c: &ConversationCreate) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let title = c.title.as_deref().unwrap_or("").trim().to_string();
        sqlx::query(
            "INSERT INTO conversations \
               (workspace_id, id, title, provider_id, model, context) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(&c.id)
        .bind(title)
        .bind(&c.provider_id)
        .bind(c.model.as_deref())
        .bind(c.context.to_string())
        .execute(exec)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db)
                if db.code().as_deref() == Some("2067") || db.code().as_deref() == Some("1555") =>
            {
                Error::Conflict(format!("conversation {:?} already exists", c.id))
            }
            other => Error::Internal(format!("sqlx: {other}")),
        })?;
        Ok(())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM conversations WHERE workspace_id = ? AND id = ?")
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown conversation: {id}")));
        }
        Ok(())
    }

    pub async fn append_message<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        role: &str,
        content: &str,
        tools: &[serde_json::Value],
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let mut msg = serde_json::json!({
            "role": role,
            "content": content,
            "ts": ts,
        });
        if !tools.is_empty() {
            msg["tools"] = serde_json::Value::Array(tools.to_vec());
        }
        let res = sqlx::query(
            "UPDATE conversations \
             SET data = json_set(data, '$.messages[#]', json(?)) \
             WHERE workspace_id = ? AND id = ?",
        )
        .bind(msg.to_string())
        .bind(ws_id)
        .bind(id)
        .execute(exec)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown conversation: {id}")));
        }
        Ok(())
    }

    pub async fn set_session_id<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        session_id: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "UPDATE conversations SET session_id = ? WHERE workspace_id = ? AND id = ? AND session_id IS NULL",
        )
        .bind(session_id)
        .bind(ws_id)
        .bind(id)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn set_model<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        model: Option<&str>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query("UPDATE conversations SET model = ? WHERE workspace_id = ? AND id = ?")
            .bind(model)
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub async fn set_title<'e, E>(&self, exec: E, ws_id: &str, id: &str, title: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query("UPDATE conversations SET title = ? WHERE workspace_id = ? AND id = ?")
            .bind(title)
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }
    pub async fn search<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ConversationMatch>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let pattern = crate::search::like_pattern(query);
        Ok(sqlx::query_as::<_, ConversationMatch>(
            "SELECT id, title, data FROM conversations \
             WHERE workspace_id = ? \
               AND (title LIKE ? ESCAPE '\\' OR data LIKE ? ESCAPE '\\') \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(ws_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }
}
