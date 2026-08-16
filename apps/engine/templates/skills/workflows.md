## Skill: Workflows & Roadmap

The roadmap moves a task through a config-driven pipeline. **Never hardcode
status keys** — read the valid keys from config:

    modula config get        # the `pipeline` section lists every status key

### Claiming and transitioning

Claim work by advancing the roadmap (task-level) and/or a variant status before
doing anything else, so concurrent runs don't double-work.

    # Roadmap (task-level) status. A body with status / notes / depends_on
    # routes to the roadmap; the task's current pipeline status is shown in
    # `modula task get`.
    modula task patch <task-id> '{"status":"<key>"}'

    # Read the task's current roadmap status.
    modula task get <task-id>

### Typical pipeline flow

`planning` → `ready_for_research` → `researching` → `ready_for_workers`
(per-variant) → `in_progress` → `ready_for_review` → `in_review` →
`ready_for_acceptance` → human acceptance. Each agent advances only the
transitions it owns; check your role's instructions for which.

### Status semantics

- `needs_clarification` — soft pause; a human answers and flips the task back to
  `ready_for_research`.
- `blocked` — hard stop; the task can't proceed as written. Set it with a
  `notes` body explaining why (`'{"status":"blocked","notes":"…"}'`); don't
  abandon the row silently.

### Discipline

- Make **one** roadmap-claim write per run (only if not already claimed).
- Only emit transitions your role is allowed to make.
- After your writes, the dispatcher routes the resulting events to the next
  agent — don't spawn anything yourself.
