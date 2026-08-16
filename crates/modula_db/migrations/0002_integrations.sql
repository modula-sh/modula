CREATE TABLE integrations (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id           TEXT NOT NULL,
  data         TEXT NOT NULL DEFAULT '{}',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id)
);
CREATE TRIGGER trg_integrations_updated AFTER UPDATE ON integrations
BEGIN
  UPDATE integrations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;
