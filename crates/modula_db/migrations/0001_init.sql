-- TEXT UUID primary keys are minted in Rust at insert time. updated_at is
-- maintained by an AFTER UPDATE trigger, never written by the application.

-- slug backs on-disk paths only (the UUID stays canonical); unique so two
-- workspaces can't share a directory.
CREATE TABLE workspaces (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  slug        TEXT NOT NULL,
  description TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX idx_workspaces_slug ON workspaces(slug);
CREATE TRIGGER trg_workspaces_updated AFTER UPDATE ON workspaces
BEGIN
  UPDATE workspaces SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE pipeline_statuses (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  position     INTEGER NOT NULL,
  key          TEXT NOT NULL,
  label        TEXT NOT NULL,
  tone         TEXT NOT NULL,
  station      TEXT,
  terminal     INTEGER NOT NULL DEFAULT 0,
  error        INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, key)
);
CREATE TRIGGER trg_pipeline_statuses_updated AFTER UPDATE ON pipeline_statuses
BEGIN
  UPDATE pipeline_statuses SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE workspace_settings (
  workspace_id        TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  max_spawns_per_run  INTEGER NOT NULL DEFAULT 5,
  created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE TRIGGER trg_workspace_settings_updated AFTER UPDATE ON workspace_settings
BEGIN
  UPDATE workspace_settings SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE providers (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id           TEXT NOT NULL,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL DEFAULT 'claude',
  config_dir   TEXT NOT NULL,
  description  TEXT,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id)
);
CREATE TRIGGER trg_providers_updated AFTER UPDATE ON providers
BEGIN
  UPDATE providers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE projects (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id           TEXT NOT NULL,
  name         TEXT NOT NULL,
  path         TEXT NOT NULL,
  base_branch  TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id)
);
CREATE TRIGGER trg_projects_updated AFTER UPDATE ON projects
BEGIN
  UPDATE projects SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- skills: JSON array of opted-in skill slugs (hidden skills are injected
-- unconditionally at spawn time, never listed here).
CREATE TABLE agents (
  workspace_id      TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id                TEXT NOT NULL,
  name              TEXT NOT NULL,
  description       TEXT NOT NULL DEFAULT '',
  provider_id       TEXT NOT NULL,
  model             TEXT,
  schedule_cron     TEXT,
  schedule_tz       TEXT,
  schedule_enabled  INTEGER NOT NULL DEFAULT 0,
  manual            INTEGER NOT NULL DEFAULT 1,
  rules             TEXT NOT NULL DEFAULT '[]',
  args              TEXT NOT NULL DEFAULT '[]',
  skills            TEXT NOT NULL DEFAULT '[]',
  prompt            TEXT NOT NULL DEFAULT '',
  spawn_per_variant INTEGER NOT NULL DEFAULT 0,
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id),
  FOREIGN KEY (workspace_id, provider_id) REFERENCES providers(workspace_id, id)
);
CREATE TRIGGER trg_agents_updated AFTER UPDATE ON agents
BEGIN
  UPDATE agents SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- Reusable system-prompt fragments assembled at spawn time in `position`
-- order; `hidden` ones are injected into every agent regardless of opt-in.
CREATE TABLE agent_skills (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  slug         TEXT NOT NULL,
  name         TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  prompt       TEXT NOT NULL DEFAULT '',
  hidden       INTEGER NOT NULL DEFAULT 0,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, slug)
);
CREATE TRIGGER trg_agent_skills_updated AFTER UPDATE ON agent_skills
BEGIN
  UPDATE agent_skills SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- internal_id: per-(workspace, source) counter (derived live as MAX+1) backing
-- display ids like "MOD-001". Tasks are soft-deleted (deleted_at stamped, row
-- kept) so a number is never reused — reuse would re-point existing links at
-- the wrong task.
CREATE TABLE tasks (
  workspace_id  TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id            TEXT NOT NULL,
  title         TEXT NOT NULL,
  source        TEXT NOT NULL,
  external_id   TEXT,
  internal_id   INTEGER NOT NULL,
  status        TEXT,
  url           TEXT,
  approved      INTEGER,
  description   TEXT NOT NULL DEFAULT '',
  max_variants  INTEGER,
  worktree      INTEGER,
  synced_at     TEXT,
  deleted_at    TEXT,
  source_data   TEXT NOT NULL DEFAULT '{}',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id),
  UNIQUE (workspace_id, external_id),
  UNIQUE (workspace_id, source, internal_id)
);
CREATE TRIGGER trg_tasks_updated AFTER UPDATE ON tasks
BEGIN
  UPDATE tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- Per-task overrides of an agent's spawn behaviour; missing row means
-- loop_amount 1 (agents with no task context never loop).
CREATE TABLE task_agent_settings (
  workspace_id TEXT NOT NULL,
  task_id      TEXT NOT NULL,
  agent_id     TEXT NOT NULL,
  loop_amount  INTEGER NOT NULL DEFAULT 1,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, task_id, agent_id),
  FOREIGN KEY (workspace_id, task_id)  REFERENCES tasks(workspace_id, id)   ON DELETE CASCADE,
  FOREIGN KEY (workspace_id, agent_id) REFERENCES agents(workspace_id, id)  ON DELETE CASCADE
);
CREATE TRIGGER trg_task_agent_settings_updated AFTER UPDATE ON task_agent_settings
BEGIN
  UPDATE task_agent_settings SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- One canonical row per (type, name) so dedup and rename are trivial.
-- `type` future-proofs for agent/provider labels; today only 'task' is used.
CREATE TABLE labels (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id           TEXT NOT NULL,
  type         TEXT NOT NULL DEFAULT 'task',
  name         TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id),
  UNIQUE (workspace_id, type, name)
);
CREATE TRIGGER trg_labels_updated AFTER UPDATE ON labels
BEGIN
  UPDATE labels SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE task_labels (
  workspace_id TEXT NOT NULL,
  task_id      TEXT NOT NULL,
  label_id     TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, task_id, label_id),
  FOREIGN KEY (workspace_id, task_id)  REFERENCES tasks(workspace_id, id)   ON DELETE CASCADE,
  FOREIGN KEY (workspace_id, label_id) REFERENCES labels(workspace_id, id)  ON DELETE CASCADE
);
CREATE INDEX idx_task_labels_task  ON task_labels(workspace_id, task_id);
CREATE INDEX idx_task_labels_label ON task_labels(workspace_id, label_id);

CREATE TABLE variants (
  workspace_id TEXT NOT NULL,
  task_id    TEXT NOT NULL,
  id           TEXT NOT NULL,
  status       TEXT,  -- NULL until promoted (e.g. researcher → 'ready_for_workers')
  position     INTEGER NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, task_id, id),
  FOREIGN KEY (workspace_id, task_id)
    REFERENCES tasks(workspace_id, id) ON DELETE CASCADE
);
CREATE TRIGGER trg_variants_updated AFTER UPDATE ON variants
BEGIN
  UPDATE variants SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE roadmap_rows (
  workspace_id TEXT NOT NULL,
  task_id    TEXT NOT NULL,
  status       TEXT NOT NULL,
  depends_on   TEXT NOT NULL DEFAULT '[]',
  notes        TEXT NOT NULL DEFAULT '',
  position     INTEGER NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, task_id),
  FOREIGN KEY (workspace_id, task_id)
    REFERENCES tasks(workspace_id, id) ON DELETE CASCADE
);
CREATE TRIGGER trg_roadmap_rows_updated AFTER UPDATE ON roadmap_rows
BEGIN
  UPDATE roadmap_rows SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE thread_entries (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  workspace_id      TEXT NOT NULL,
  scope             TEXT NOT NULL,
  task_id         TEXT NOT NULL,
  variant_id        TEXT,
  ts                TEXT NOT NULL,
  author            TEXT NOT NULL,
  kind              TEXT NOT NULL,
  round             INTEGER,
  content           TEXT NOT NULL,
  verdict           TEXT,
  affected_variants TEXT,
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_thread_lookup
  ON thread_entries(workspace_id, task_id, variant_id, id);
CREATE TRIGGER trg_thread_entries_updated AFTER UPDATE ON thread_entries
BEGIN
  UPDATE thread_entries SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  workspace_id TEXT NOT NULL,
  type         TEXT NOT NULL,
  data         TEXT NOT NULL DEFAULT '{}',
  processed    INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_events_processing ON events(processed, created_at);
CREATE TRIGGER trg_events_updated AFTER UPDATE ON events
BEGIN
  UPDATE events SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

-- One row per ralph-loop iteration; loop_group_id links them (the iter=1 row
-- points at its own id). Single-shot runs are a group of one.
CREATE TABLE agent_runs (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  workspace_id  TEXT NOT NULL,
  agent_id      TEXT NOT NULL,
  agent_name    TEXT NOT NULL,
  event_id      INTEGER REFERENCES events(id) ON DELETE SET NULL,
  status        TEXT NOT NULL,
  attempts      INTEGER NOT NULL DEFAULT 0,
  data          TEXT NOT NULL DEFAULT '{}',
  started_at    TEXT,
  finished_at   TEXT,
  log_path      TEXT,
  loop_iter     INTEGER NOT NULL DEFAULT 1,
  loop_total    INTEGER NOT NULL DEFAULT 1,
  loop_group_id INTEGER,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_agent_runs_status ON agent_runs(workspace_id, status);
CREATE INDEX idx_agent_runs_loop_group ON agent_runs(workspace_id, loop_group_id);
CREATE TRIGGER trg_agent_runs_updated AFTER UPDATE ON agent_runs
BEGIN
  UPDATE agent_runs SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE agent_processes (
  pid          INTEGER PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  agent_id     TEXT NOT NULL,
  agent_name   TEXT NOT NULL,
  agent_run_id INTEGER NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE TRIGGER trg_agent_processes_updated AFTER UPDATE ON agent_processes
BEGIN
  UPDATE agent_processes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;

CREATE TABLE conversations (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  id           TEXT NOT NULL,
  title        TEXT NOT NULL DEFAULT '',
  provider_id  TEXT NOT NULL,
  model        TEXT,
  session_id   TEXT,
  data         TEXT NOT NULL DEFAULT '{"messages":[]}',
  context      TEXT NOT NULL DEFAULT '{}',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, id),
  FOREIGN KEY (workspace_id, provider_id) REFERENCES providers(workspace_id, id)
);
CREATE INDEX idx_conversations_ws_updated ON conversations (workspace_id, updated_at DESC);
CREATE TRIGGER trg_conversations_updated AFTER UPDATE ON conversations
BEGIN
  UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE rowid = NEW.rowid;
END;
