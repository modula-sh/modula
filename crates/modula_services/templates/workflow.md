# Modula — Workflow (pseudo-code runbook)

Step-by-step view of how an external task becomes shipped code, end-to-end. A scan agent (JIRA / Linear / GitHub) mirrors issues into `tasks`; from there the pipeline is source-agnostic. Each agent's role is written in pseudo-code; the authoritative behaviour lives in each agent's `agents.prompt` DB row.

For schemas, state machine, and ownership: see [`overview.md`](overview.md).

---

## Architecture in one paragraph

**Agents** (claude processes — `agent/<name>/`) do the actual work for one piece of work each. They never spawn other agents and never write the DB directly — every read or write is a `modula` CLI call to the engine over the local IPC socket. **The central dispatcher** (one in-process tokio task) ticks every ~5s: it reads unprocessed events from `events`, evaluates each agent's `rules` against each event, and spawns matching agents. Status writes (which create the next event) are owned by agents.

```
Scan agent ─┐
            │
   task.create / task.update (approved)  ── PM rule fires ──> Project Manager ── task.update (pipeline_status=ready_for_research) ──┐
                                                                                                                                            │
                                                              Researcher rule fires <─── task.update (pipeline_status=ready_for_research) ─┘
                                                              │
                                                              ▼
                                                          Researcher  ─> modula variant create <task>
                                                              │           modula task patch <task> (ready_for_workers)
                                                              │           writes phases.md + phase-N-plan.md/task.md
                                                              ▼
                                                  variant.update (ready_for_workers)
                                                              │
                                                              ▼  Worker rule fires
                                                           Worker  ─> code in .worktrees/<branch>
                                                              │       modula variant patch <v> → ready_for_review
                                                              ▼
                                                  variant.update (ready_for_review)
                                                              │
                                                              ▼  Code-Reviewer rule fires
                                                       Code-Reviewer  ─> variant-thread comments + verdict
                                                              │           ACCEPT → variant.status: accepted
                                                              │           REQUEST_CHANGES → variant.status: rework (loops back)
                                                              │           on final ACCEPT → roadmap → ready_for_review
                                                              ▼
                                                  task.update (pipeline_status=ready_for_review)
                                                              │
                                                              ▼  Reviewer rule fires
                                                          Reviewer  ─> task-thread verdict
                                                              │          APPROVE → roadmap: ready_for_acceptance
                                                              │          KICK BACK → variant.status: rework + roadmap: in_progress
                                                              ▼
                                                            Human  ─> picks winner, merges, sets accepted
```

Every status mutation is itself an event (`task.create`, `task.update`, `task.delete`, `task.reset`, `variant.update`, `thread.append`). `task.update` covers both task-field changes (`approved`, `title`, `description`, …) AND pipeline transitions on the roadmap (`pipeline_status`); each changed field rides flat on `event.data`, and is present only on the update that set it. The dispatcher does not edit status — it only spawns. Agents own all terminal writes; the engine emits events whenever a row changes.

---

## Step 0 — Human

```
1. Open the Modula desktop app (it launches the bundled engine on open).
2. In the dashboard:
   - add at least one provider (type + config_dir).
   - add the projects you want under factory management.
   - fill in each scan agent's scope inline in its prompt (jira-scan: site + jql / account_id + project_keys; linear-scan: team_keys; github-scan: repos).
   - tune the agents’ rules + schedule (e.g. jira-scan runs on a 4×/day cron).
3. Optionally set workspace_settings.max_spawns_per_run (default 5).
4. Quit from the tray to unload the engine.
```

All of this writes DB rows through the engine (dashboard over gRPC).

---

## Step 1 — Source Scan (scheduled or manual)

One scan agent per source (jira-scan / linear-scan / github-scan), identical in shape — they differ only in the source they read and their scope fields.

```
read scope inline from the scan agent's own prompt   # not the config
fetch issues from the source (JIRA / Linear / GitHub) via its MCP tools
for each fetched issue:
    modula task create <body>   # upsert on (workspace_id, external_id):
                                #   new external_id → create (approved=null)
                                #   existing        → patch in place
```

Every create emits a `task.create` (new) or `task.update` (existing) event, which downstream agents (e.g. PM) can react to.

---

## Step 2 — Human approves tasks

```
in dashboard, for each task to ship: set approved = true
for each task to skip:                set approved = false
                                        (null = pending — agents skip)
```

Each approval emits `task.update`. PM's rule sees the change.

---

## Step 3 — Project Manager (event-driven or manual)

PM fires when its rule matches. Typical rule:

```
event.type == "task.create" and event.data.approved == true
event.type == "task.update" and event.data.approved == true
```

PM also does its own filter inside the run — the rules are a coarse "wake up when a task gets approved."

```
modula task list   → filter approved == true
modula roadmap list

for each task where approved == true:
    if already on roadmap:
        if status == 'planning':
            re-evaluate deps; modula task patch <id> { status: ready_for_research, depends_on, notes }
        else:
            leave alone
    else:
        modula task patch <id> { status: planning, depends_on: [...] }
        when finished evaluating: patch again with status: ready_for_research
```

Each roadmap transition emits `task.update` (with `pipeline_status`); the next tick routes to the Researcher.

---

## Step 4 — Researcher (one task per spawn)

Rule:

```
event.type == "task.update" and event.data.pipeline_status == "ready_for_research"
```

That `ready_for_research` flip is set by PM (or by a human re-research) — when the rule fires, the agent picks up and immediately advances the roadmap to `researching` to claim the work.

```
required arg: --task-id <ID>

modula roadmap list → verify <task>.status == 'ready_for_research'
                      modula task patch <TASK> { status: researching }  (claim)

modula comment list <TASK>
  if last own entry was kind: question AND there are newer human entries,
  those are answers — modula comment create a kind: comment acknowledging which
  ones are now resolved, then proceed.

investigate the project(s) referenced by the task  (read-only)

if you hit ambiguity a one-sentence human answer would resolve:
    create <workspace>/specs/<TASK>/ if missing
    modula comment create <TASK> <body>  (author=researcher, kind=question)
    modula task patch <TASK> { status: needs_clarification }
    exit
    # event re-routes you here once human flips back to ready_for_research

decide variant count = 1..(task.max_variants ?? 1)
                       # tasks with worktree=false always have max_variants=1
                       # enforced server-side; same rule still applies.

variants = modula variant create <TASK> { count: variant_count }
           # server mints UUIDs; returns [{id, position}, ...] in created[]

for v in variants:
    split the variant's work into phases (1..5, coherent chunks)
    create <workspace>/specs/<TASK>/<v.id>/phases.md
        # sections: Problem, Approach, Projects touched, Phases (checklist),
        #          Risks & tradeoffs, Test plan
    for each phase p:
        create phase-<p>-plan.md  (scope, approach, notes)
        create phase-<p>-task.md  (checklist)

modula task patch <TASK> { status: ready_for_workers }    # or 'blocked' if structurally un-researchable
exit
```

Each variant creation emits a `variant.update`; the Worker's rule (`event.data.status == "ready_for_workers"`) fires for each new variant on the next tick. `needs_clarification` is a soft pause — the rule doesn't match, so the row sits until a human replies and flips back to `ready_for_research`.

Throughout: the Researcher reads `wiki/index.md` and relevant pages before investigating, cites pages with `[[link]]` in `phases.md` or per-phase plans, and writes durable findings back to the wiki per `wiki/SCHEMA.md` — see [`overview.md`](overview.md) → "AI Wiki".

---

## Step 5 — Worker (one variant per spawn)

A Worker runs in either **fresh mode** or **rework mode**. Mode is detected from the variant-thread: rework mode = at least one prior `kind: verdict` entry exists; otherwise fresh.

Rule:

```
event.type == "variant.update" and (event.data.status == "ready_for_workers" or event.data.status == "rework")
```

```
required args: --task-id <ID> --variant-id <V>

modula task get <TASK> → find variant entry; verify status is ready_for_workers or rework
                         read tasks.<id>.worktree   (default true; false ⇒ direct mode)

modula variant patch <V>  { status: in_progress }   # claim the variant

modula config get → resolve project.path + project.base_branch

read phases.md  → "Projects touched" + Phases checklist (which are pending)
for each project:
    if worktree mode (default):
        git worktree add .worktrees/<branch> -b <branch> <base_branch>   # fresh
        OR reuse existing worktree                                       # rework
    else (direct mode, worktree=false):
        cd <project_path> && git checkout <base_branch> && git pull --ff-only
        on fresh runs only: git tag -f modula/<task>-<variant>/start   # for Code-Reviewer diff
        no new branch, no worktree

if rework mode:
    modula comment list <TASK>  → variant thread (Code-Reviewer's per-variant concerns)
                                   + task thread (Reviewer's task-level kick-back, if any)
    incorporate the feedback (update phases.md — re-open a phase or add a rework phase)

work the next pending phase(s); tick items in phase-<N>-task.md as you complete them
commit per-project; never push
self-review every diff; build/lint/test per project's CLAUDE.md

update phases.md — check the phase off when its task list is fully done

if any phase remains unchecked:
    exit                                                  # next iteration / run resumes
else:
    append "Done" / "Rework round N" section to phases.md
    if rework mode: modula comment create <TASK> <body>  (author=worker, kind=rework, variant=<V>, round=N, content=…)
    modula variant patch <V>  { status: ready_for_review }
    exit
```

The `variant.update` event from the final patch routes to the Code-Reviewer.

---

## Step 6 — Code-Reviewer (one variant per spawn)

Rule:

```
event.type == "variant.update" and event.data.status == "ready_for_review"
```

```
required args: --task-id <ID> --variant-id <V>

modula task get <TASK> → variant entry; expect status == 'ready_for_review';
                          tasks.<id>.worktree determines where the diff lives.
modula variant patch <V>  { status: in_review }   # claim the variant

modula config get  → project paths + base branches
modula comment list <TASK>  → variant thread (prior rounds, including Worker rework summaries)

determine round = (count of prior `kind: verdict` entries in this variant thread) + 1

for each project in phases.md "Projects touched":
    if worktree mode (default):
        cd <project>/.worktrees/<branch>
        diff <base_branch>..<branch>
    else (direct mode, worktree=false):
        cd <project>
        diff modula/<task>-<variant>/start..<base_branch>   # tag set by Worker
    run lint/typecheck/tests per project's CLAUDE.md

evaluate:
    adherence to phases.md + per-phase plans, project conventions, correctness, simplicity, test coverage
    in rework mode: verify prior feedback was actually addressed

modula comment create <TASK> <body>  (author=code-reviewer, kind=comment, variant=<V>, round=N) — one per concern, with file:line refs
modula comment create <TASK> <body>  (author=code-reviewer, kind=verdict, variant=<V>, round=N, verdict=ACCEPT|REQUEST_CHANGES)

if verdict == ACCEPT:
    modula variant patch <V>  { action: accept }   # → status: accepted
elif verdict == REQUEST_CHANGES:
    modula variant patch <V>  { action: rework }   # → status: rework

exit
```

REQUEST_CHANGES → `variant.update` (status=rework) → Worker rule re-fires.
ACCEPT → `variant.update` (status=accepted). When *all* variants are at `accepted`, PM (or a manual promotion) flips the roadmap row to `in_review`, producing the event the Reviewer subscribes to.

---

## Step 7 — Reviewer (one task per spawn) — kick-back authority

Rule:

```
event.type == "task.update" and event.data.pipeline_status == "ready_for_review"
```

That `ready_for_review` flip is set by the Code-Reviewer after accepting the final variant — when the rule fires, the agent claims by advancing the roadmap to `in_review`.

```
required arg: --task-id <ID>

modula roadmap list → verify <task>.status == 'ready_for_review'
                      modula task patch <TASK> { status: in_review }  (claim)
modula task get <TASK> → verify every variant has status == 'accepted' AND none at 'rework'
modula comment list <TASK>  → task thread (your prior verdicts) + variant threads (per-variant CR conversations)

determine round = (count of prior `kind: verdict` entries in task thread) + 1

form a task-level judgment by reading each variant thread:
    architectural fit, cross-variant patterns, risk profile

(optional) modula comment create <TASK> <body>  (author=reviewer, kind=comment, round=N, content=…)
modula comment create <TASK> <body>  (author=reviewer, kind=verdict, round=N, verdict=APPROVE|KICK_BACK,
                                        affected_variants=[...] when KICK_BACK)

verdict == APPROVE:
    modula task patch <TASK> { status: ready_for_acceptance }
    human takes over

verdict == KICK_BACK:
    for each kicked variant:
        modula variant patch <V>  { status: rework }
    modula task patch <TASK> { status: in_progress }
    Worker will read the task thread on rework to understand why
exit
```

After kick-back: Worker rule sees `variant.update` (status=rework) → reworks → Code-Reviewer → variants land back at `accepted` → roadmap moves to `in_review` → Reviewer rule re-fires. Loop until APPROVE.

---

## Step 8 — Human reviews and accepts

```
1. Open dashboard, find task at roadmap.status == ready_for_acceptance.
2. Read the task thread — Reviewer's verdict + cross-variant comparison.
3. Read each variant's thread — Code-Reviewer's per-variant findings.
4. Decide which variant wins (call it v_winning).

for each project the winning variant touched:
    cd <project>/.worktrees/feature/<TASK>-<v_winning>
    inspect diff
    open one PR per project; merge

# Mark complete:
modula variant patch <v_winning>  { status: accepted }   # human override
modula task patch <TASK>          { status: accepted }
```

Other variants' worktrees can be cleaned up at leisure (`git worktree remove`).

---

## Event vocabulary

The engine publishes an event on every successful mutation and on agent-run lifecycle transitions. Agents subscribe via rule strings.

| event.type | Emitted by | event.data shape |
|---|---|---|
| `task.create` | `modula task create` (new row) | `{ task_id, source, approved }` |
| `task.update` | `modula task patch` (task fields, or scan-agent upsert) | `{ task_id, …changed fields… }` (each at top level) |
| `task.update` | `modula task patch` (pipeline transition) | `{ task_id, pipeline_status }` |
| `task.update` | label attach/detach on a task | `{ task_id, label_id, label_action: "attached"\|"detached" }` |
| `task.delete` | task delete | `{ task_id }` |
| `task.reset` | task reset | `{ task_id }` |
| `variant.update` | `modula variant create` (per variant) or `modula variant patch` | `{ task_id, variant_id, status }` |
| `thread.append` | `modula comment create` | `{ task_id, variant_id?, kind, author, verdict? }` |
| `agent.create` / `agent.update` / `agent.delete` | agent CRUD | `{ agent_id }` |
| `provider.create` / `provider.update` / `provider.delete` | provider CRUD | `{ provider_id }` |
| `run.spawned` | agent spawn (manual, scheduled, or dispatched; per loop iteration) | `{ run_id, agent_id, agent_name, pid, iter? }` |
| `run.exited` | agent process exit (reaper or loop controller) | `{ run_id, pid }` |

Don't write agent rules that match `run.*` events to spawn agents — a spawn publishes `run.spawned`, so such a rule re-triggers itself.

Adding a new event family (`mention.*`, `integration.*`) is a no-op on the table — just start inserting; agents subscribe by writing a rule that matches the new `event.type`.

---

## Rule expression DSL

Tiny expression grammar evaluated against `{ event: { type, data } }`.

| Supported |
|---|
| Identifiers joined by `.` (`event.type`, `event.data.status`) |
| String literals `'…'` or `"…"` |
| Operators `==`, `!=` |
| Booleans `and`, `or` |
| Parens for grouping |

Each agent has `rules: [string, ...]` — a JSON array of expression strings. The agent fires if **any** of them returns true against the event. Invalid expressions are rejected when the agent is created or updated.

Examples:

```
# Worker — picks up freshly created variants and rework
event.type == "variant.update" and (event.data.status == "ready_for_workers" or event.data.status == "rework")

# Code-Reviewer
event.type == "variant.update" and event.data.status == "ready_for_review"

# Reviewer
event.type == "task.update" and event.data.pipeline_status == "ready_for_review"

# Researcher
event.type == "task.update" and event.data.pipeline_status == "ready_for_research"
```

Adding a new field to a task / roadmap row → simply reference `event.data.<field>` in a new rule. No engine changes required.

---

## State summary

The set of allowed roadmap status keys, their order, and their display metadata live in `pipeline_statuses` rows (one set per workspace, seeded on create). The list below is the **default** factory pipeline:

```
ROADMAP STATUS:
  planning             PM evaluating
  ready_for_research   PM done; Researcher rule fires
  researching          Researcher locked
  needs_clarification  Researcher posted a question; awaiting human reply in task thread
  ready_for_workers    specs written; Worker rule fires per variant
  in_progress          Worker(s) and/or Code-Reviewer(s) running, OR kick-back loop in flight
  ready_for_review     all variants accepted; transient (usually skipped — agent flips straight to in_review)
  in_review            Reviewer rule fires
  ready_for_acceptance Reviewer wrote APPROVE; awaiting human acceptance
  accepted             human merged + chose winner (terminal)
  blocked              stuck (manual recovery; error accent in dashboard)

VARIANT STATUS (per variant):
  ready_for_workers   Researcher just created the variant
  in_progress         Worker is running (claimed at start of run)
  ready_for_review    Worker finished (fresh or rework completion)
  in_review           Code-Reviewer is running (claimed at start of run)
  rework              Code-Reviewer REQUEST_CHANGES or Reviewer KICK_BACK
  accepted            Code-Reviewer ACCEPTed (or human override)
```

The variant pipeline is a clean subset of the roadmap pipeline — same vocabulary at a smaller scope.

---

## Recovery / re-run safety

The dispatcher is the recovery mechanism. Every tick re-evaluates unprocessed events; anything that was missed last time gets dispatched this time. Most stuck states resolve themselves within one tick (~5s default).

Per-agent re-run idempotency:

| Agent | Re-run behavior |
|---|---|
| **Scan agents** (JIRA / Linear / GitHub) | Re-scan; upsert existing rows in-place; never touch human/Worker fields. |
| **PM** | Picks up `roadmap.status == planning` rows from interrupted prior runs and finishes the transition to `ready_for_research`. Cron-scheduled and event-driven. |
| **Researcher** | Reads `roadmap.status == researching` as its starting state. On resume from `needs_clarification`, reads the task thread, treats post-question human entries as answers, and proceeds. |
| **Worker** | Sets `variant.status = in_progress` on entry to claim the variant. Reuses existing worktree, resumes from the first unchecked phase in `phases.md`. Setting `ready_for_review` at the end is idempotent. |
| **Code-Reviewer** | Sets `variant.status = in_review` on entry. Reads prior thread rounds to decide rework verification scope. |
| **Reviewer** | Reads `roadmap.status == in_review` as its starting state. Idempotent over re-runs (last verdict wins). |

### Crash recovery

The dispatcher's reaper:

1. Wakes on SIGCHLD when any spawned child exits (1s safety-net tick for engine-restart leftovers).
2. For each row in `agent_processes`, calls `waitpid(pid, WNOHANG)`. Exited / not-our-child results flip their `agent_runs` row to `completed` and delete the `agent_processes` row.

If the underlying status mutation never landed (e.g. agent crashed before its final write), the row simply stays at its in-flight status. A human can re-trigger the agent from the dashboard's Agents page, or set the variant/roadmap status back to the upstream value to re-emit the event and let the rule re-fire.

Duplicate dispatch is structurally prevented: events are marked `processed = 1` after the tick handles them, so the same event can't be re-routed even if the dispatcher restarts mid-tick.

## Manual recovery — when to step in

Most cases are auto-handled by the next dispatcher tick. The few that need a human nudge:

| Symptom | Action |
|---|---|
| Stuck `researching` with partial specs | Delete the partial `specs/<TASK>/` folder, `modula task patch <TASK> {status: ready_for_research}`. Next tick re-fires the Researcher. |
| Stuck `needs_clarification` (no human reply yet) | Open task in dashboard, answer the Researcher's question(s) in the thread composer, then click "Resume research" — flips status back to `ready_for_research`; next tick re-fires the Researcher. |
| Variant at `ready_for_review` but never code-reviewed | Should self-resolve on next tick. To force: click Run on the code-reviewer agent in the dashboard. |
| Variant stuck at `in_progress` or `in_review` (interrupted agent — process died mid-run) | Auto-reaped (`agent_processes` row removed when PID dies). To replay the work: set the variant back to the upstream status (`ready_for_workers` for an in_progress Worker, `ready_for_review` for an in_review Code-Reviewer) via `modula variant patch <V>` — the event re-emits and the rule re-fires. |
| Want to force kick-back on accepted variant | `modula variant patch <V> {status: rework}` and `modula task patch <TASK> {status: in_progress}`. (Dashboard has a per-variant "Rework" button.) |
| Worker says "no projects to touch" | `phases.md` Projects touched is empty. Researcher under-specified. Set task to `blocked` (or delete the spec folders), then re-research. |

## Manual control surface

| Intent | How |
|---|---|
| Run one specific agent with explicit args | Click Run on the agent card in the dashboard. |
| Emit a synthetic event (debugging rules) | Emit it from the dashboard — the next tick routes it through the rule evaluator. |
| Inspect recent events / runs | The dashboard Events / Runs panels. |
| Force kick-back | `modula variant patch <V> {status: rework}` + `modula task patch <TASK> {status: in_progress}`; next dispatcher tick re-fires the affected agent. |
| Watch an agent's live log | Open the log in the dashboard, or `tail -f ~/.modula/<workspace>/logs/<agent>-*-<ts>.log`. |
