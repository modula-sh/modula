## Skill: Tasks, Variants & Comments

### Reading tasks and variants

    modula task get <task-id>        # one task with its variants
    modula variant get <variant-id>  # one variant (status, position) + owning task
    modula comment list <task-id>    # the task thread, then each variant's thread

A task carries human-owned fields (`approved`, `max_variants`, `worktree`) and a
`variants[]` block. Each variant has a `status` that drives the pipeline. Read
the printed `status:`, `approved:`, and per-variant lines directly.

### Registering variants (researcher only)

    modula variant create <task-id> '{"count":2}'

The engine mints the UUIDs and prints the created variants (id + position). A
freshly registered variant has NO status and is NOT workable until promoted.

### Variant status

    # Set a status directly.
    modula variant patch <variant-id> '{"status":"in_progress"}'

    # Code-reviewer applies a verdict via an action (not a raw status).
    modula variant patch <variant-id> '{"action":"accept"}'
    # or '{"action":"rework"}'

`variant patch` takes only the variant id — the CLI resolves the owning task.

### Comments / thread entries

Threads are **append-only**. Post entries with an `author`, a `kind`, and
`content`. Variant-scoped entries carry `variant` + `round`; task-scoped entries
omit `variant`.

    modula comment create <task-id> \
      '{"author":"<you>","kind":"comment","variant":"<variant-id>","round":N,"content":"…"}'

`kind` values:
- `comment` — an observation or note.
- `question` — a question for the human (code-reviewer must post at least one).
- `verdict` — a decision; requires `verdict` set to the agent's allowed values
  (e.g. `ACCEPT` / `REQUEST_CHANGES` for code-reviewer, `APPROVE` / `KICK_BACK`
  for reviewer; `KICK_BACK` requires `affected_variants`).
- `rework` — the worker's summary of what a rework round fixed.
