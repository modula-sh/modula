-- Statuses where the task waits on a human. Backfills the seeded defaults.
ALTER TABLE pipeline_statuses ADD COLUMN awaits_human INTEGER NOT NULL DEFAULT 0;
UPDATE pipeline_statuses SET awaits_human = 1
  WHERE key IN ('needs_clarification', 'ready_for_acceptance');
