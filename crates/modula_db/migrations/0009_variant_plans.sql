-- One plan document (JSON text) per variant. The engine parses and validates
-- it; the cascade drops it with its variant on task reset or delete.
CREATE TABLE variant_plans (
  workspace_id TEXT NOT NULL,
  task_id      TEXT NOT NULL,
  variant_id   TEXT NOT NULL,
  plan         TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, task_id, variant_id),
  FOREIGN KEY (workspace_id, task_id, variant_id)
    REFERENCES variants(workspace_id, task_id, id) ON DELETE CASCADE
);
CREATE TRIGGER trg_variant_plans_updated AFTER UPDATE ON variant_plans
BEGIN
  UPDATE variant_plans SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;
