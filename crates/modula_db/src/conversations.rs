//! Conversation rows. Identity = `(workspace_id, id)`. One row per chat
//! thread; its messages are rows of `conversation_messages`, ordered by `seq`.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use serde::Deserialize;
use serde_json::json;
use sqlx::{Executor, Sqlite};

use modula_types::{ChatMessage, Conversation, MessagePart, QueuedMessage, TurnStats};

use crate::{Error, Result};

#[derive(Debug, Clone, sqlx::FromRow)]
struct ConversationRecord {
    id: String,
    title: String,
    provider_id: String,
    model: Option<String>,
    context: String,
    session_id: Option<String>,
    queued: String,
    created_at: String,
    updated_at: String,
    message_count: u32,
}

/// The conversation without its messages; [`ConversationRepository::messages`]
/// reads those.
impl From<ConversationRecord> for Conversation {
    fn from(r: ConversationRecord) -> Self {
        let context = serde_json::from_str(&r.context).unwrap_or_else(|_| json!({}));
        Self {
            id: r.id,
            title: r.title,
            provider_id: r.provider_id,
            model: r.model,
            context,
            session_id: r.session_id,
            messages: Vec::new(),
            message_count: r.message_count,
            queued: serde_json::from_str(&r.queued).unwrap_or_default(),
            created_at: r.created_at,
            updated_at: r.updated_at,
            running: false,
        }
    }
}

const SELECT_COLS: &str =
    "id, title, provider_id, model, context, session_id, queued, created_at, updated_at, \
     (SELECT COUNT(*) FROM conversation_messages m \
       WHERE m.workspace_id = conversations.workspace_id \
         AND m.conversation_id = conversations.id) AS message_count";

#[derive(Debug, Clone, sqlx::FromRow)]
struct MessageRecord {
    seq: u32,
    role: String,
    content: String,
    parts: String,
    stats: Option<String>,
    ts: String,
}

impl From<MessageRecord> for ChatMessage {
    fn from(r: MessageRecord) -> Self {
        Self {
            role: r.role,
            content: r.content,
            ts: r.ts,
            seq: r.seq,
            parts: serde_json::from_str(&r.parts).unwrap_or_default(),
            stats: r.stats.and_then(|s| serde_json::from_str(&s).ok()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConversationCreate {
    pub id: String,
    pub title: Option<String>,
    pub provider_id: String,
    pub model: Option<String>,
    pub context: serde_json::Value,
}

/// A message to append; its `seq` and `ts` are assigned on insert.
#[derive(Debug, Clone, Copy)]
pub struct NewMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
    pub parts: &'a [MessagePart],
    pub stats: Option<&'a TurnStats>,
}

impl<'a> NewMessage<'a> {
    pub fn user(content: &'a str) -> Self {
        Self {
            role: "user",
            content,
            parts: &[],
            stats: None,
        }
    }
}

/// `content` is the first message matching the query, when one does.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationMatch {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
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

    /// Messages in conversation order. `limit` keeps the newest that many;
    /// `before_seq` restricts to those older than it.
    pub async fn messages<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        before_seq: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<ChatMessage>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let mut rows = sqlx::query_as::<_, MessageRecord>(
            "SELECT seq, role, content, parts, stats, ts FROM conversation_messages \
             WHERE workspace_id = ? AND conversation_id = ? AND seq < ? \
             ORDER BY seq DESC LIMIT ?",
        )
        .bind(ws_id)
        .bind(id)
        .bind(before_seq.map_or(i64::MAX, i64::from))
        .bind(limit.map_or(-1, i64::from))
        .fetch_all(exec)
        .await?;
        rows.reverse();
        Ok(rows.into_iter().map(ChatMessage::from).collect())
    }

    pub async fn append_message<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        msg: &NewMessage<'_>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let parts = serde_json::to_string(msg.parts).unwrap_or_else(|_| "[]".to_string());
        let stats = msg.stats.and_then(|s| serde_json::to_string(s).ok());
        sqlx::query(
            "INSERT INTO conversation_messages \
               (workspace_id, conversation_id, seq, role, content, parts, stats, ts) \
             SELECT ?1, ?2, COALESCE(MAX(seq) + 1, 0), ?3, ?4, ?5, ?6, ?7 \
               FROM conversation_messages WHERE workspace_id = ?1 AND conversation_id = ?2",
        )
        .bind(ws_id)
        .bind(id)
        .bind(msg.role)
        .bind(msg.content)
        .bind(parts)
        .bind(stats)
        .bind(ts)
        .execute(exec)
        .await
        .map_err(|e| match e {
            // The foreign key is the only constraint an append can break.
            sqlx::Error::Database(db) if db.code().as_deref() == Some("787") => {
                Error::NotFound(format!("unknown conversation: {id}"))
            }
            other => Error::Internal(format!("sqlx: {other}")),
        })?;
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

    pub async fn set_queued<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        queued: &[QueuedMessage],
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let json = serde_json::to_string(queued).unwrap_or_else(|_| "[]".to_string());
        let res =
            sqlx::query("UPDATE conversations SET queued = ? WHERE workspace_id = ? AND id = ?")
                .bind(json)
                .bind(ws_id)
                .bind(id)
                .execute(exec)
                .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown conversation: {id}")));
        }
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
            "SELECT id, title, content FROM ( \
               SELECT c.id, c.title, c.updated_at, \
                      (SELECT m.content FROM conversation_messages m \
                        WHERE m.workspace_id = c.workspace_id AND m.conversation_id = c.id \
                          AND m.content LIKE ?2 ESCAPE '\\' \
                        ORDER BY m.seq LIMIT 1) AS content \
                 FROM conversations c WHERE c.workspace_id = ?1) \
             WHERE title LIKE ?2 ESCAPE '\\' OR content IS NOT NULL \
             ORDER BY updated_at DESC LIMIT ?3",
        )
        .bind(ws_id)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{providers::ProviderRepository, workspaces::WorkspaceRepository};
    use tempfile::tempdir;

    async fn seeded(dir: &tempfile::TempDir) -> (crate::Database, String, ConversationRepository) {
        let pool = crate::open(&dir.path().join("t.sqlite")).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let ws = WorkspaceRepository::new()
            .create(&mut conn, "Modula", None)
            .await
            .unwrap();
        drop(conn);
        let provider_id = ProviderRepository::new()
            .create(&pool, &ws, "Claude", "claude", "/tmp", None)
            .await
            .unwrap();
        let convs = ConversationRepository::new();
        convs
            .create(
                &pool,
                &ws,
                &ConversationCreate {
                    id: "c1".into(),
                    title: None,
                    provider_id,
                    model: None,
                    context: json!({}),
                },
            )
            .await
            .unwrap();
        (pool, ws, convs)
    }

    #[tokio::test]
    async fn messages_append_in_order_and_page_from_the_tail() {
        let dir = tempdir().unwrap();
        let (pool, ws, convs) = seeded(&dir).await;
        for i in 0..5 {
            convs
                .append_message(&pool, &ws, "c1", &NewMessage::user(&format!("m{i}")))
                .await
                .unwrap();
        }
        let parts = vec![
            MessagePart::Tool(modula_types::ToolCall {
                id: "t1".into(),
                name: "Read".into(),
                input: json!({"file_path": "a.txt"}),
                parent_id: None,
                status: modula_types::ToolStatus::Completed,
                output: Some("alpha".into()),
                output_truncated: false,
            }),
            MessagePart::Text {
                text: "done".into(),
            },
        ];
        let stats = TurnStats {
            duration_ms: Some(1200),
            ..Default::default()
        };
        convs
            .append_message(
                &pool,
                &ws,
                "c1",
                &NewMessage {
                    role: "assistant",
                    content: "done",
                    parts: &parts,
                    stats: Some(&stats),
                },
            )
            .await
            .unwrap();

        let conv = convs.get(&pool, &ws, "c1").await.unwrap();
        assert_eq!(conv.message_count, 6);
        assert!(conv.messages.is_empty());

        let all = convs.messages(&pool, &ws, "c1", None, None).await.unwrap();
        assert_eq!(
            all.iter().map(|m| m.seq).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5]
        );
        assert_eq!(all[5].parts, parts);
        assert_eq!(all[5].stats, Some(stats));

        let tail = convs
            .messages(&pool, &ws, "c1", None, Some(2))
            .await
            .unwrap();
        assert_eq!(tail.iter().map(|m| m.seq).collect::<Vec<_>>(), [4, 5]);
        let older = convs
            .messages(&pool, &ws, "c1", Some(4), Some(3))
            .await
            .unwrap();
        assert_eq!(older.iter().map(|m| m.seq).collect::<Vec<_>>(), [1, 2, 3]);

        assert!(matches!(
            convs
                .append_message(&pool, &ws, "nope", &NewMessage::user("x"))
                .await,
            Err(Error::NotFound(_))
        ));
    }

    /// The transcript migration: a legacy `data` blob becomes message rows, an
    /// assistant message's tools ahead of its text.
    #[tokio::test]
    async fn legacy_transcript_backfills_into_message_rows() {
        let dir = tempdir().unwrap();
        let (pool, ws, convs) = seeded(&dir).await;
        let data = json!({ "messages": [
            { "role": "user", "content": "hi", "ts": "t0" },
            { "role": "assistant", "content": "ok", "ts": "t1",
              "tools": [{ "name": "Bash", "input": { "command": "ls" } }] },
        ]});
        sqlx::query("UPDATE conversations SET data = ? WHERE id = 'c1'")
            .bind(data.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let backfill = include_str!("../migrations/0006_conversation_messages.sql")
            .split_once("INSERT INTO")
            .and_then(|(_, rest)| rest.split_once(';'))
            .map(|(insert, _)| format!("INSERT INTO{insert}"))
            .unwrap();
        sqlx::query(&backfill).execute(&pool).await.unwrap();

        let msgs = convs.messages(&pool, &ws, "c1", None, None).await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(
            (msgs[0].role.as_str(), msgs[0].content.as_str()),
            ("user", "hi")
        );
        assert!(msgs[0].parts.is_empty());
        let MessagePart::Tool(tool) = &msgs[1].parts[0] else {
            panic!("expected the tool first");
        };
        assert_eq!(
            (tool.name.as_str(), &tool.input),
            ("Bash", &json!({"command": "ls"}))
        );
        assert_eq!(msgs[1].parts[1], MessagePart::Text { text: "ok".into() });
    }

    #[tokio::test]
    async fn queued_round_trips_through_the_column() {
        let dir = tempdir().unwrap();
        let (pool, ws, convs) = seeded(&dir).await;

        assert!(convs.get(&pool, &ws, "c1").await.unwrap().queued.is_empty());

        let queued = vec![QueuedMessage {
            id: "q1".into(),
            content: "next".into(),
        }];
        convs.set_queued(&pool, &ws, "c1", &queued).await.unwrap();
        assert_eq!(convs.get(&pool, &ws, "c1").await.unwrap().queued, queued);

        assert!(matches!(
            convs.set_queued(&pool, &ws, "nope", &queued).await,
            Err(Error::NotFound(_))
        ));
    }
}
