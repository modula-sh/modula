# Modula — Factory Overview

This is the source of truth for how the factory operates. Everything in `.modula/` and the SQLite DB follows from this file.

**See also**: [`workflow.md`](workflow.md) — same pipeline written as pseudo-code for each agent. Easier to skim when you need to understand "what does PM actually do, step by step" without reading prose.

## What this factory is

A pipeline that turns approved external tasks (JIRA / Linear / GitHub) into shipped code, run by a small set of Claude agents. Each agent has a narrow role and persists its output through the engine (via the `modula` CLI over local IPC) so the next agent in the pipeline can pick it up. Structured state lives in a single SQLite database; markdown artifacts (the variant spec folder + per-workspace wiki + agent prompts) live on disk.

## Where state lives

The factory has two parts: a **single `modula` binary** that ships with the desktop app (engine + gRPC-over-local-IPC API + scheduler + central dispatcher — framework docs and seed material are embedded inside the binary), and **per-machine workspace state** under the user's home dir.

```
~/.modula/                          # ← per-machine; not in git
├── db.sqlite                        # ALL structured state — one global DB
│                                    #   tables: workspaces, workspace_settings,
│                                    #   pipeline_statuses, providers, projects,
│                                    #   agents, agent_skills, tasks, variants,
│                                    #   labels, task_labels, task_agent_settings,
│                                    #   roadmap_rows, thread_entries, events,
│                                    #   agent_runs, agent_processes, conversations
└── <workspace-slug>/               # one folder per workspace, named by slug
    ├── specs/
    │   └── <task-slug>/            # e.g. mod-0001-some-new-adjustment
    │       └── v<position>/        # one dir per variant (1-based position)
    │           ├── phases.md        # Researcher's design + phase checklist
    │           ├── phase-1-plan.md  # per-phase plan
    │           ├── phase-1-task.md  # per-phase checklist (Worker ticks off)
    │           └── ...
    ├── logs/                        # Stream-json log files from agent runs
    ├── wiki/                        # Agent-maintained codebase knowledge base
    │   ├── SCHEMA.md
    │   ├── index.md
    │   ├── log.md
    │   ├── general/
    │   └── <project>/               # one dir per `projects[].name`
    └── overview.md / workflow.md    # Framework docs, copied in on workspace creation

# Agent prompts live in the `agents.prompt` DB column, not on disk.
# Workspace root is overridable via `MODULA_DIR` (default `~/.modula/`).
#
# Projects under each workspace are referenced by absolute path and can live
# ANYWHERE on disk — the workspace just groups them. Each project entry in
# the DB stores its absolute `path` and `base_branch`.
```

What stays on disk (because it's the right tool):

| Surface | Why on disk |
|---|---|
| `phases.md`, `phase-N-plan.md`, `phase-N-task.md` | Markdown artifacts a human reads / pastes into PRs. |
| `logs/*.log` | Append-only stream-json; `tail -f` is the operator's friend. |
| `wiki/` | Obsidian-compatible markdown the user opens in a wiki reader. |

Everything else (tasks, roadmap, variants, providers, projects, agents, agent prompts, threads, events, runs, processes, pipeline) lives in `~/.modula/db.sqlite`.

## Roles

The scan agents (JIRA / Linear / GitHub) mirror external issues on demand or on a `schedule_cron`; the rest fire when matching events arrive at the central dispatcher. All reads and writes below go through the `modula` CLI.

| Agent | Triggered by | Reads | Writes | Codes? |
|---|---|---|---|---|
| **JIRA Scan** | manual or `schedule_cron` | JIRA | `task create` (upsert) | no |
| **Linear Scan** | manual or `schedule_cron` | Linear (MCP) | `task create` (upsert) | no |
| **GitHub Scan** | manual or `schedule_cron` | GitHub (MCP) | `task create` (upsert) | no |
| **Project Manager** | `task.create` / `task.update` rules, or manual | `task list`, `roadmap list` | roadmap via `task patch` | no |
| **Researcher** | `task.update` rule (pipeline_status == `ready_for_research`) | `task get`, `roadmap list`, task-thread | `variant create`, roadmap via `task patch`, task-thread `comment create`; writes `phases.md` + per-phase `plan` / `task` files on disk | no |
| **Worker** | `variant.update` rule (status == `ready_for_workers` or `rework`) | one variant, `phases.md` + per-phase files, task-thread, variant-thread | variant status via `variant patch`; ticks `phases.md` + per-phase task files; writes code in N worktrees | **yes** |
| **Code-Reviewer** | `variant.update` rule (status == `ready_for_review`) | one variant's worktree(s), `task get`, variant-thread | variant-thread `comment create` + verdict; ACCEPT → variant `accept`, REQUEST_CHANGES → variant `rework` | no |
| **Reviewer** | `task.update` rule (pipeline_status == `ready_for_review`) | every variant's thread + roadmap | task-thread verdict (`comment create`); APPROVE → roadmap to `ready_for_acceptance`; KICK BACK → variant `rework` + roadmap to `in_progress` | no |

Agents **never** spawn other agents. Status transitions are driven by **events** the engine emits whenever a mutation lands — the dispatcher routes each event to whichever agents have matching rules. Status writes are owned by agents; the dispatcher only spawns.

## Central dispatcher

One in-process tokio task. Three wakeup triggers:

1. **Event ticker** (default `MODULA_DISPATCH_INTERVAL_SECS = 5`s): lists unprocessed events (oldest first, last 24h). For each, evaluates every agent's `rules`. Any rule that returns true → spawn that agent (one `agent_runs` row per spawn; `agent_processes` tracks the live PID). Marks the event processed. Agents flagged `spawn_per_variant` fan out: a task-scoped event becomes one synthetic `variant.update` per variant, re-evaluated against the agent's rules — one spawn per matching variant.
2. **SIGCHLD**: when a spawned child exits the kernel signals the engine. The reaper calls `waitpid(pid, WNOHANG)` on each row in `agent_processes`, flips finished runs to `completed`, and deletes their process row — typically within milliseconds of the child exiting.
3. **Safety-net ticker** (1s): catches engine-restart leftovers where children have been reparented to init (SIGCHLD never reaches us for those).

Scheduled agents (`schedule_cron`) bypass the event loop — the in-process scheduler calls `spawn_agent` directly. Such agents typically emit events as they do their work (e.g. `jira-scan` upserts tasks, each producing `task.update`).

Status-driven loops keep working because every mutation emits an event:

| Operation | Event |
|---|---|
| `task create` (new row) | `task.create` |
| `task patch` (task fields) | `task.update` |
| `task patch` (pipeline status/notes/depends_on) | `task.update` (with `pipeline_status`) |
| task delete | `task.delete` |
| task reset | `task.reset` |
| `variant create` / `variant patch` | `variant.update` |
| `comment create` | `thread.append` |

So when the Worker sets `variant.status = ready_for_review` via `variant patch`, the engine inserts a `variant.update` event; the next dispatcher tick routes it to the Code-Reviewer; the chain continues.

## Rule grammar

Each agent has a `rules: [string, ...]` field — a JSON array of expression strings. An agent fires if **any** expression evaluates true against the event:

```
event.type == "variant.update" and event.data.status == "in_review"
```

Supported syntax (deliberately tiny):
- Identifiers joined by `.` (`event.type`, `event.data.status`, …)
- String literals `'…'` / `"…"`
- Operators `==`, `!=`
- Boolean operators `and`, `or`
- Parens for grouping

Invalid rules are rejected when an agent is created or updated — they can't be saved.

## Workflow at a glance

```
Scan agent ──> task.create / task.update
                       │
   (human flips approved=true via dashboard → task.update)
                       ▼
                Project Manager ──> task.update (pipeline_status=ready_for_research)
                       ▼
                   Researcher ──> variant rows + task.update (pipeline_status=ready_for_workers)
                       │           specs/<id>/<v>/{phases.md, phase-N-plan.md, phase-N-task.md}
                       ▼
                     Worker ──> variant.update (status=ready_for_review)
                       │         code in .worktrees/<branch>
                       ▼
                Code-Reviewer ──> variant.update (status=accepted | rework)
                       │           (on final accept) task.update (pipeline_status=ready_for_review)
                       ▼
                   Reviewer ──> task-thread verdict
                                APPROVE → task.update (pipeline_status=ready_for_acceptance)
                                KICK_BACK → variant.update rework + task.update (pipeline_status=in_progress)
                       ▼
                     Human ──> picks winner, merges, sets status `accepted`
```

The default 11-status roadmap pipeline (seeded into every new workspace's `pipeline_statuses` table):

`planning → ready_for_research → researching → ready_for_workers → in_progress → ready_for_review → in_review → ready_for_acceptance → accepted` (with `needs_clarification` as a soft pause from the Researcher, and `blocked` as a terminal error state).

The pipeline is per-workspace — keys, labels, tones, station grouping, and terminal/error flags all live in the `pipeline_statuses` table. Customizing it means updating rows there *and* updating any rule strings / agent prompts that reference the keys.

## Data model — what every agent reads through the CLI

Agents read and mutate workspace state through the `modula` CLI (see the Engine
CLI skill), which talks to the engine over the local IPC socket and auto-detects
the running engine and current workspace. The CLI is the only surface agents use.

### Tasks — `modula task list` / `get <id>` / `create <body>` / `patch <id> <body>`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",   // server-minted UUID — never sent on POST
  "external_id": "ENG-1234",                       // tracker key for external tasks; auto display id (e.g. "MOD-001") for internal
  "title": "Short task title",
  "source": "jira",              // jira | linear | internal | …
  "status": "open",              // mirrored from external system (external tasks only)
  "source_data": {},             // freeform JSON — external integrations store tracker-specific metadata here
  "url": "https://...",
  "approved": null,              // true | false | null
  "description": "",
  "max_variants": null,          // int 1..10 or null (default 1)
  "worktree": null,              // true (default) → worktree-per-project; false → direct mode (forces max_variants = 1)
  "synced_at": "2026-05-04",
  "variants": [{ "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8", "position": 1, "status": "ready_for_workers" }]
}
```

Tasks enter in two ways:
- **External integrations** — `jira-scan`, `linear-scan`, and `github-scan` sync tasks from external systems via `task create` upsert. The upsert key is `(workspace_id, external_id)`; each integration owns rows with a matching `source:`.
- **Internal** — created from the dashboard's "+ new" button. Server mints a UUID `id`, `source: internal`, and an auto display id `external_id` (workspace prefix + per-workspace counter, e.g. `MOD-001`).

Field ownership:

| Field | Owner | Notes |
|---|---|---|
| `external_id`, `title`, `source`, `status`, `source_data`, `url`, `description`, `synced_at` | external integration (or human via dashboard on internal rows) | mirrored from external system; on external rows these fields are re-mirrored on the next sync. The opaque `id` (UUID) is server-minted on create and immutable; on internal rows `external_id` is likewise server-generated and immutable. |
| `approved` | Human | agents read only |
| `max_variants`, `worktree` | Human (dashboard) | Researcher reads `max_variants`; Worker + Code-Reviewer read `worktree`. |
| `variants[]` | Researcher creates them via `variant create`; dispatcher/agents transition variant status via `variant patch`. | Status is the single source of truth for "where is this variant in the pipeline." |

`approved` semantics: `true` = eligible; `false` = rejected; `null` = pending. Agents skip non-`true` rows.

### Variant state machine

| status | set by | meaning |
|---|---|---|
| `ready_for_workers` | Researcher | variant exists; no Worker has run yet |
| `in_progress` | Worker on resume (set on its first PUT after pickup) | Worker running |
| `ready_for_review` | Worker on completion | code on disk; ready for code review |
| `in_review` | Code-Reviewer on pickup | Code-Reviewer running |
| `rework` | Code-Reviewer (REQUEST_CHANGES) or Reviewer (KICK_BACK) | needs Worker rework |
| `accepted` | Code-Reviewer (ACCEPT) | eligible for task-level Reviewer |

Transitions are driven by `variant.update` events — every `variant patch` emits one, and an agent's rule fires when the right status appears. Worker mode (fresh vs rework) is detected from the variant-thread: rework mode = at least one prior `kind: verdict` entry exists; otherwise fresh.

### Roadmap — `modula roadmap list`, pipeline transitions via `modula task patch`

```json
{
  "task": "550e8400-e29b-41d4-a716-446655440000",                 // task UUID
  "status": "planning",                                              // see state machine
  "depends_on": ["6ba7b810-9dad-11d1-80b4-00c04fd430c8"],            // task UUIDs that must reach `accepted` first
  "notes": "",
  "position": 0
}
```

Variants live on the task, not the roadmap row. The roadmap is purely orchestration (status + dependencies); the task is the data.

| state | set by | meaning |
|---|---|---|
| `planning` | PM | PM is currently evaluating this task (deps, ordering) |
| `ready_for_research` | PM | PM finished; Researcher can pick up |
| `researching` | Researcher | Researcher is locked on this row |
| `needs_clarification` | Researcher | Researcher hit ambiguity; posted a `kind: question` to the task-thread. Human answers via dashboard composer and flips back to `ready_for_research`. |
| `ready_for_workers` | Researcher | Specs written; Workers can pick up variants |
| `in_progress` | Worker (on first run) | Workers + Code-Reviewers running |
| `ready_for_review` | (transitional) | All variants accepted; Reviewer will pick up on next event |
| `in_review` | dispatcher or PM-style agent | Reviewer is locked on this row |
| `ready_for_acceptance` | Reviewer (APPROVE) | Awaiting human acceptance |
| `accepted` | Human | Best variant chosen, PRs merged, task complete (terminal) |
| `blocked` | any agent | Stuck — manual recovery required (error state) |

### Threads — `modula comment list <task>`, `modula comment create <task> <body>`

Append-only entries scoped to a task or a variant. `comment list` returns the task-scoped entries then each variant's thread; `comment create` appends to the task thread, or to a variant's thread when the body carries `variant` + `round`. Schema (per entry):

```json
{
  "ts": "2026-05-04T14:30:00Z",
  "author": "code-reviewer",       // code-reviewer | worker | researcher | reviewer | human | ...
  "kind": "comment",               // comment | question | verdict | rework
  "round": 1,
  "content": "Free-form markdown…",
  "verdict": "ACCEPT",             // ACCEPT | REQUEST_CHANGES (variant-thread); APPROVE | KICK_BACK (task-thread)
  "affected_variants": ["<variant-uuid>"] // only on KICK_BACK
}
```

Who writes what:

- **Task-thread**: human comments, Researcher's `kind: question` (clarification), Reviewer's `kind: verdict`.
- **Variant-thread**: Worker's `kind: rework` summary per round, Code-Reviewer's `comment` + `question` + `verdict`.

Each thread entry triggers a `thread.append` event. Agents subscribe to these when the conversation has to drive them (e.g. Researcher resumes when a human reply lands).

### Specs folder — on disk only

```
specs/<task-slug>/           # one dir per task (external-id + title slug). No top-level files.
└── v<position>/                # one dir per variant (1-based position)
    ├── phases.md              # Researcher's design + phase checklist;
    │                          #   sections: Problem, Approach, Projects touched,
    │                          #   Phases (checklist), Risks & tradeoffs, Test plan
    ├── phase-1-plan.md        # one plan per phase (scope, approach, notes)
    ├── phase-1-task.md        # one task list per phase (Worker ticks off)
    └── ...
```

The folder is the agents' shared scratch space for the markdown that doesn't fit cleanly into the DB. Anything structured (statuses, variants, verdicts, comments) goes through the `modula` CLI instead.

Researcher writes the initial files. Worker ticks task checkboxes as it works and checks off phases in `phases.md` when done; only flips variant `ready_for_review` once every phase is checked off. A single-shot Worker finishes all phases in one run; a looping Worker may take many iterations per phase.

## AI Wiki

Each workspace has a `wiki/` directory — an agent-maintained markdown knowledge base for the codebases the workspace operates on. The pattern follows [karpathy's llm-wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f): raw sources (the project repos, tasks, threads) are immutable; the wiki is the AI-owned synthesis layer that compounds over time so agents don't rediscover the same architectural facts on every task.

### Layout

```
wiki/
├── SCHEMA.md       Conventions — agents read this before reading or writing.
├── index.md        Catalog of every page (one-line summary per row).
├── log.md          Chronological append-only — ## [YYYY-MM-DD] kind | title
├── general/        Workspace-wide pages (cross-project, factory ops).
└── <project>/      One dir per project (matches `projects.name` rows).
```

There is exactly one `index.md` and one `log.md` at the wiki root; project subdirs are categories, not nested wikis. Pages cross-reference with Obsidian wiki-link syntax `[[page-name]]` (or `[[<project>/page-name]]` when disambiguation is needed).

### Read / write / lint contract

| Agent | Role |
|---|---|
| **Researcher** | Primary reader + writer. Reads `index.md` and relevant pages before investigating. Writes durable findings (architecture, conventions, quirks). Cites pages with `[[link]]` in `phases.md` / phase plans. |
| **Worker** | Reader; light writer. Reads relevant pages before implementing. Updates when implementation contradicts or fills a gap. |
| **Code-Reviewer** | Reader + maintainer. Reads pages to review against. Updates when review surfaces a new pattern or contradiction. |
| **PM / Reviewer** | No wiki interaction — these agents operate above the codebase level. |

### What goes in (and what doesn't)

**In the wiki**: durable codebase truths — architectural patterns and rationale, module responsibilities, recurring conventions, quirks, cross-project contracts, contradictions between docs and code.

**Not in the wiki**: task-specific work (→ `specs/<task-slug>/v<position>/phases.md` + phase files), review conversation (→ thread entries via API), build/test logs (→ `logs/`), code itself, anything already authoritative in `<project>/CLAUDE.md`.

### Seeding and ownership

`wiki/SCHEMA.md` is copied into every new workspace from the embedded templates. Agents read the in-workspace copy when updating — agent prompts only carry a short pointer to the schema.

## Worktree convention

A variant may touch one or more projects. The default mode (`tasks.worktree: true` or null) reuses the **same branch name** in every project the variant touches, with one worktree per project:

```
<project>/.worktrees/<branch-name>/
```

### Branch naming

Branches use a fixed `feature/` prefix:

```
feature/<task-slug>-v<position>
```

(slug-based, e.g. `feature/mod-0001-some-new-adjustment-v1`; never UUIDs.)
Each project's `.gitignore` should ignore `.worktrees/`.

**Direct mode** (`tasks.worktree: false`, always paired with `max_variants: 1`) skips both the worktree and the new branch — the Worker commits on each project's `base_branch` directly. Before its first commit, the Worker tags the start point as `modula/<task-slug>-v<position>/start` so Code-Reviewer can diff later. Use direct mode for rapid prototyping where you don't want PR overhead.

Either way, the Worker discovers which projects to touch by reading the variant's `phases.md` → "Projects touched" section. The dispatcher passes only the variant identification arg(s); the Worker resolves project paths and base branches from `modula config get` itself.

## Project index

Projects under factory management are rows in the `projects` table — each entry has `name`, absolute `path`, and `base_branch`, scoped to the workspace. The dashboard exposes CRUD; agents read them via `modula config get`. Workspace-specific narrative about each project — what it is, how to build/run/deploy it, conventions, gotchas — belongs in that workspace's wiki at `<workspace>/wiki/<project-name>/`, not in this framework doc.

Inside each project repo, the project's own `CLAUDE.md` is the source of truth for that project's coding rules — agents read it before touching code.

## Operating the factory

The end-to-end *sequence* (what each agent does, what the human does at each stage) is in [`workflow.md`](workflow.md). This section covers the operational surface — dashboard, logs, ad-hoc commands.

### Dashboard

Launch the Modula desktop app from `~/Applications` or your tray. It launches the bundled engine on open, keeps it running while the app sits in the tray (closing the window only hides it), and stops the engine on **Quit**. The dashboard surfaces tasks, roadmap, running agents, recent events + runs, and live log streams; the **Agents** page shows what's defined in the `agents` table and exposes a Run button per agent. Scheduled agents (e.g. `jira-scan`) fire from the engine's in-process scheduler whenever the engine is running.

The engine listens on a local IPC socket (a per-user Unix-domain socket; a named pipe on Windows) — no TCP port is opened. Once it's loaded via launchctl, it survives the GUI being closed — quitting from the tray is what unloads it.

### Watching logs from the terminal

```bash
# follow one log
tail -f ~/.modula/<workspace>/logs/researcher-20260504-154311.log

# follow all logs in one workspace
tail -f ~/.modula/<workspace>/logs/*.log
```

Logs are stream-json, one event per line — every agent runs `claude -p --output-format stream-json --verbose`. The dashboard's log viewer renders these as single-line markers (`── INIT`, `▸ TEXT`, `→ TOOL`, `← RSLT`, `═══ DONE`); raw `tail -f` shows the JSON.

### Catch-up commands

Day-to-day, everything flows through the dashboard. The `modula` CLI covers task,
roadmap, variant, comment, config, and workspace state directly. For one-off manual runs
(mostly debugging) — triggering an agent on demand, emitting a synthetic event to
exercise the dispatcher rules, or inspecting recent events and runs — use the
dashboard: the Agents page has a Run button per agent and the Events / Runs panel
lists recent activity.

For interrupted/stuck states and manual recovery, see [`workflow.md`](workflow.md) → "Manual recovery — when to step in".

Every agent runs with `--permission-mode bypassPermissions` so it doesn't stall on prompts. The engine sets this when invoking `claude`; there is no agent-side override.

## Agent invocation convention

The engine is the only spawner of agents. For each invocation (manual, scheduled, or dispatcher-fired) it:

- Resolves the agent's `provider_id` to a `ProviderRuntime` via `ProviderService::runtime_from_provider` (no fallback — an agent without a resolvable provider can't run).
- Builds the env:
  - `MODULA_WORKSPACE` — the workspace id (also the directory name)
  - `MODULA_ENGINE_SOCKET` — path to the engine's local IPC socket (the `modula` CLI resolves it automatically)
  - `CLAUDE_CONFIG_DIR` / `OPENCODE_CONFIG_DIR` — the resolved provider's config dir (name depends on provider type)
  - `MODULA_AGENT_EXTRA` — rendered "Inputs for this run" block, built from the agent's declared `args[]`
  - `MODULA_LOG_TS` — log file naming
  - `MODULA_LOOP_ITER` + `MODULA_LOOP_TOTAL` — 1-based current iteration and configured total (both `1` for non-looping agents)
  - Optional `MODULA_CLAUDE_MODEL` (claude) / `MODULA_PROVIDER_MODEL` (opencode)
- Opens a log file at `<workspace>/logs/<agent>[-<tag>]-<ts>.log`.
- Spawns the provider binary (e.g. `claude` or `opencode run`) via `ProviderRuntime::build_command`, as a detached child (`setsid`). The process survives engine restarts.

Providers are rows in the `providers` table — each entry pairs an opaque `id` (UUID) with a human-readable `name` (e.g. "claude-personal"), a config dir, and a provider type. Many agents can share one provider.

### Dispatch flow

Dispatch is centralized: one task evaluates every agent's rules against every event. When a rule matches, the engine creates an `agent_runs` row and spawns the agent. Detection of running processes uses `agent_processes`: the row holds the live PID; reap on next tick when the PID is gone.

### Spawn caps

`workspace_settings.max_spawns_per_run` lives in the DB (one row per workspace, default 5). Agents that genuinely fan out (e.g. PM evaluating many tasks) check this themselves and cap their own work.

## Configuration

Workspace configuration is a set of DB tables, written by the dashboard (or by direct API calls):

- `workspace_settings` — `max_spawns_per_run`. (Scan-agent scope is not stored here — it lives inline in each scan agent's prompt.)
- `pipeline_statuses` — ordered list of roadmap statuses (key, label, tone, optional station, terminal/error flags). Seeded with the default 11 on workspace create.
- `providers` — claude config dirs.
- `projects` — paths + base branches.
- `agents` — per-agent definition: description, provider, optional model + cron schedule, manual flag, **rules** (JSON array of expression strings), args.
- `task_agent_settings` — per-task overrides of an agent's spawn behaviour (one row per task × agent); holds the ralph-loop `loop_amount`, set from the task view.

The engine is the only writer of every table from the dashboard's side; agents never write to these tables (they only read them via `modula config get`).

## Working principles

- **Source of truth in the DB and on disk for markdown.** Every state transition is an API call that emits an event. No agent should rely on conversation memory across runs.
- **Don't over-engineer.** Add fields, tables, and abstractions only when an agent actually fails without them.
- **Human owns approval.** Agents never flip `approved`. Researcher and below trust the DB.
- **One variant = one worker = one branch name = one worktree per affected project** (the default). A variant is a single coherent solution; if that solution spans projects, the same branch name appears in each project's repo with its own worktree. The human opens one PR per project. *Direct-mode tasks (`worktree: false`, max_variants forced to 1) collapse this to "one task = one worker, no new branch, no worktree, edits land on `base_branch`."*
- **Config, not prompts.** User- or environment-specific values live in DB tables (providers, projects, workspace_settings), not embedded in agent prompts.
