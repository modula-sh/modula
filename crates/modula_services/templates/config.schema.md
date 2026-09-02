# Workspace configuration

Workspace configuration is **not** a file — it's a set of SQLite tables in `~/.modula/db.sqlite`, managed through the engine's gRPC API. The dashboard is the primary editor; agents read the assembled config via `modula config get` and never write to these tables.

For the full data model and event flow, see [`overview.md`](overview.md) and [`workflow.md`](workflow.md).

## Tables

| Table | What it holds | Managed by |
|---|---|---|
| `workspaces` | Workspace id, name, slug, description. | `WorkspaceService` (`modula workspace list/get/create`). |
| `workspace_settings` | One row per workspace: `max_spawns_per_run`. (Scan-agent scope is not stored here — it lives inline in each scan agent's prompt.) | Read under `limits` in `modula config get`. |
| `pipeline_statuses` | Ordered roadmap statuses (key, label, tone, optional station, terminal/error flags). Seeded with the default 11 on workspace create. | Read under `pipeline` in `modula config get`. |
| `providers` | Provider config dirs agents bind to (`id`, `name`, `type`, `config_dir`, optional `description`). | `ProviderService`. |
| `projects` | Repos under factory management (`id` UUID, `name`, absolute `path`, `base_branch`). | `ProjectService`. |
| `agents` | Per-agent definition (`id` UUID, `name`, `description`, `provider_id`, optional `model` + `schedule_cron` + `schedule_tz` + `schedule_enabled`, `manual` flag, `rules` (JSON array of expression strings), `args` (JSON array of `ArgDef`)). | `AgentService`. |
| `task_agent_settings` | Per-task overrides of an agent's spawn behaviour, one row per `(task, agent)`. Columns hold settings (`loop_amount` today). | Set from the task view (dashboard). |

`modula config get` returns the assembled config (`limits`, `pipeline`, `providers`, `projects`, `agents`) in one call — the read surface agents use.

## Snapshot

`SnapshotService` returns the combined view the dashboard reads: tasks, roadmap, providers, projects, agents, pipeline, plus running processes and the most recent events / runs. It exposes a unary fetch and a server-streaming subscription; the dashboard subscribes to the stream for live updates rather than polling.

## Rule grammar (agents.rules)

Tiny expression evaluated against `{ event: { type, data } }`:

- Identifiers joined by `.` (e.g. `event.type`, `event.data.status`)
- String literals (`'…'` or `"…"`)
- Boolean literals `true` / `false` (e.g. `event.data.approved == true`; `'true'` coerces to the same, and a bare path like `event.data.approved` tests truthiness)
- Operators `==`, `!=`
- Boolean `and`, `or`
- Parens for grouping

Each agent's `rules` is a JSON array of expression strings; an agent fires if **any** of them returns true. Invalid expressions are rejected when the agent is created or updated.

## ArgDef

Each entry in `agents.args` is `{ "flag": "--name", "required": bool, "help"?: string }`. The dashboard renders an input per declared flag; `args` keys also wire event payloads to CLI args — the dispatcher fills each `--name` flag from `event.data.<name>` of the same name.

## Loop

The ralph-loop re-spawn count is a per-task setting: `task_agent_settings.loop_amount` for the `(task, agent)` pair (default 1 when no row exists, so agents with no task context never loop). Set it from the task view in the dashboard. When `>1`, the engine re-spawns the agent that many times in sequence; `MODULA_LOOP_ITER` (1-based) and `MODULA_LOOP_TOTAL` are exposed to the prompt.
