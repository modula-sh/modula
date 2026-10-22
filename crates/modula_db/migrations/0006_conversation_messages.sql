-- One row per message, so a transcript appends and pages without rewriting or
-- reading the whole conversation. `parts` holds an assistant message's text,
-- thinking and tool calls in order; `content` is its plain text.
CREATE TABLE conversation_messages (
  workspace_id    TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  seq             INTEGER NOT NULL,
  role            TEXT NOT NULL,
  content         TEXT NOT NULL DEFAULT '',
  parts           TEXT NOT NULL DEFAULT '[]',
  stats           TEXT,
  ts              TEXT NOT NULL,
  PRIMARY KEY (workspace_id, conversation_id, seq),
  FOREIGN KEY (workspace_id, conversation_id)
    REFERENCES conversations(workspace_id, id) ON DELETE CASCADE
);

-- Copy every transcript out of `conversations.data`, which is left in place
-- but no longer read or written. A legacy assistant message stored its tool
-- calls apart from its text, without results; they come first, as they rendered.
INSERT INTO conversation_messages (workspace_id, conversation_id, seq, role, content, parts, ts)
SELECT c.workspace_id,
       c.id,
       m.key,
       COALESCE(m.value ->> '$.role', ''),
       COALESCE(m.value ->> '$.content', ''),
       (SELECT COALESCE(json_group_array(json(p.part) ORDER BY p.ord), '[]')
          FROM (SELECT t.key AS ord,
                       json_object('type', 'tool',
                                   'id', 'legacy-' || t.key,
                                   'name', COALESCE(t.value ->> '$.name', ''),
                                   'input', json(COALESCE(t.value -> '$.input', 'null')),
                                   'status', 'completed') AS part
                  FROM json_each(m.value, '$.tools') t
                UNION ALL
                SELECT 1000000000,
                       json_object('type', 'text', 'text', m.value ->> '$.content')
                 WHERE m.value ->> '$.role' = 'assistant'
                   AND COALESCE(m.value ->> '$.content', '') <> '') p),
       COALESCE(m.value ->> '$.ts', '')
  FROM conversations c, json_each(c.data, '$.messages') m
 WHERE json_valid(c.data);

-- A new message is activity on its conversation: the list sorts by it.
CREATE TRIGGER trg_conversation_messages_touch AFTER INSERT ON conversation_messages
BEGIN
  UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
   WHERE workspace_id = NEW.workspace_id AND id = NEW.conversation_id;
END;
