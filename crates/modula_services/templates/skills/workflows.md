## Skill: Workflows & Roadmap

The roadmap moves a task through a pipeline defined in config. **Never hardcode status keys.** Read them from config:

```sh
modula config get        # the `pipeline` section lists every status key
```

### Claim and transition work

Claim work before doing anything else, so concurrent runs don't duplicate it. Claim by advancing the roadmap (task-level) status, a variant status, or both.

```sh
# Set the roadmap status. A body with status, notes, or depends_on routes to the roadmap.
modula task patch <task-id> '{"status":"<key>"}'

# Read the task's current roadmap status.
modula task get <task-id>
```

### Follow the pipeline

`planning` → `ready_for_research` → `researching` → `ready_for_workers` (per variant) → `in_progress` → `ready_for_review` → `in_review` → `ready_for_acceptance` → human acceptance.

Each agent advances only the transitions it owns. Your role's instructions list yours.

### Pause or stop a task

| Status | Meaning |
| --- | --- |
| `needs_clarification` | Soft pause. A human answers and sets the task back to `ready_for_research`. |
| `blocked` | Hard stop: the task can't proceed as written. Set it with a `notes` body that says why: `'{"status":"blocked","notes":"…"}'`. Never abandon the task silently. |

### Rules

- Make **one** roadmap claim per run, and only if the task isn't already claimed.
- Emit only the transitions your role owns.
- The dispatcher routes your writes to the next agent. Never spawn agents yourself.
