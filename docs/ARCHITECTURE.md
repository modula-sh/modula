# Architecture

How Modula is put together, and the rules to follow when editing it. For the
plugin seam and the open-source/proprietary split, see [`PLUGINS.md`](PLUGINS.md).

## Crates

The engine is split by layer; each crate depends only on the ones below it.

| Crate | Holds |
|:------|:------|
| `crates/modula_types` | plain domain models (`Task`, `Variant`, `Agent`, …) + the single proto↔domain conversion layer. No business logic. |
| `crates/modula_rpc` | `proto/*.proto` (one per gRPC service), tonic/prost codegen, the proto↔`tonic::Status` error mapping (`DomainError`), JSON helpers. |
| `crates/modula_db` | stateless `*Repository` structs over sqlx, the schema (`migrations/`), and `modula_db::open`. |
| `crates/modula_platform` | cross-platform host plumbing: IPC socket/pipe security, service install, process management, paths. |
| `crates/modula_engine_transport` | the local IPC endpoint, server bind, stale-endpoint handling. |
| `crates/modula_core` | `ApiError`, paths, the `Repositories` bundle, slug/validation helpers. |
| `crates/modula_services` | the business layer — `*Service` impls — plus the embedded `templates/`. |
| `crates/modula_state` | `AppState`: the composed services and runtime handles. |
| `crates/modula_grpc` | thin gRPC handlers. |
| `crates/modula_plugin` | the `Plugin` trait and its `PluginService`/`PluginGrpc`/`PluginCli` components, `PluginRegistry`, `PluginContext`, `Bus`. |
| `crates/modula_client` | one `ModulaClient` over the IPC channel; shared by the CLI and the Tauri backend. |
| `apps/cli` | the `modula` client surface (crate `modula-cli`). |
| `apps/engine` | `server.rs` + `run()`: registers plugins, binds the socket, owns the `modula` binary. |
| `apps/desktop` | Tauri shell + React frontend. |
| `plugins/remote` | Modula Remote — a stub in a public build. See [`PLUGINS.md`](PLUGINS.md). |

Test-only: `crates/modula_mock_claude` (a `mock-claude` binary that replays JSON
recipes as stream-json), `crates/modula_test_support` (the `Harness` that boots
an engine and shims `claude`/`opencode`/`codex` onto `PATH`), and `tests/e2e`
(one integration binary per `tests/*.rs`).

**`apps/engine` depends on `apps/cli`, not the reverse** — the engine ships the
CLI and installs it with `link-cli`. So `apps/cli` need not depend back on the
engine in order to serve, `cli::run` returns `Outcome::ServeEngine` for the
`engine` subcommand and `apps/engine` acts on it.

## Where things live

```
apps/engine/src/
  main.rs                — thin entrypoint
  lib.rs                 — run(): registers plugins, parses, serves or delegates to the CLI
  server.rs              — tonic gRPC server over local IPC; AppState boot, plugin start, shutdown

apps/cli/src/
  lib.rs                 — clap tree (engine, status, install, link-cli + the client families)
  transport.rs           — EngineTransport: workspace resolution over ModulaClient
  commands/ format/      — one module per command family, and its renderer

crates/modula_services/src/
  tasks.rs threads.rs labels.rs agents.rs workspaces.rs projects.rs config.rs
  events/ runs.rs conversations.rs usage.rs integrations.rs   — domain services
  providers/             — ProviderService: CRUD + ProviderRuntime hydration
  diffs.rs pr.rs processes.rs logs.rs snapshot.rs             — cross-domain reads
  diff.rs branches.rs tools.rs wiki.rs mcp_config/            — agnostic fs/git helpers
  spawn.rs scheduler.rs loop_registry.rs                      — agent-launch runtime
  dispatcher/            — tick loop, rule matching, spawn, reap; expr.rs is the rule evaluator
crates/modula_services/templates/     — workspace seed material, embedded via include_dir!
  agents/<name>.md skills/ wiki/ overview.md workflow.md config.schema.md

crates/modula_grpc/src/  — one module per service; error.rs maps ApiError → tonic::Status
crates/modula_core/src/  — error.rs paths.rs repositories.rs slug.rs validation.rs

apps/desktop/
  src/                   — React app (App.tsx, router.tsx, views/, components/, contexts/)
  src-tauri/             — Tauri glue (crate `modula-desktop`): tray, launchctl, invoke handlers
```

## Rules — read before editing

1. **Three-layer data access.** Thin gRPC handlers (`modula_grpc`) call the
   business layer (`modula_services`), which calls `modula_db` repositories.
   Handlers **never** touch repositories directly. Repositories are stateless —
   each method takes a caller-supplied executor — so the owning service holds the
   `SqlitePool` and opens transactions; the unit of work lives in the service.
   Never construct SQL outside `modula_db`, and never write YAML to disk for
   state. Migrations live in `crates/modula_db/migrations/`, applied by
   `modula_db::open` at startup.

2. **Depend downward only.** A layer never reaches back up. `modula_services`
   takes `modula_core::repositories::Repositories`, not `AppState`; where a
   service needs sibling services it takes a purpose-built struct
   (`conversations::ConvRuntime`) that `AppState` assembles once.

3. **Typed errors, converted at the boundary.** Fallible code returns
   `ApiResult<T>` (`modula_core::error::ApiError` — `BadRequest`/`NotFound`/
   `Forbidden`/`Conflict`/`Internal`). `modula_grpc::error::to_status` maps it to
   `tonic::Status` at the edge, preserving the detail message. Underlying errors
   convert via `From` so `?` just works — never `.map_err` a generic conversion.
   A `From` impl may encode only mappings correct at *every* call site: e.g.
   `From<sqlx::Error>` maps UNIQUE→`Conflict` and everything else (including
   `RowNotFound`) to `Internal`. Site-specific outcomes stay explicit at the
   query — a real not-found is `fetch_optional(...).ok_or_else(...)`, and a site
   needing a different mapping keeps its own `.map_err` (see `roadmap.rs`
   FK→`NotFound`). Don't add a global mapping that would fire wrongly elsewhere.

4. **Dispatch is event-driven.** Every workspace event goes through the single
   publish path, `EventService::publish` (injected as `EventSink`): persist a row
   into `events`, then broadcast on the in-process `Bus` — never one without the
   other. One tokio task (`services::dispatcher`) ticks every ~5s, evaluates each
   agent's `rules` against unprocessed events, and spawns matches (one
   `agent_runs` row per spawn; `agent_processes` tracks live PIDs). The
   dispatcher never mutates task/variant/roadmap status — only agents do, via the
   API.

5. **Spawn = pure Rust.** No bash launcher. `services::spawn` invokes the
   provider binary via `ProviderRuntime::build_command` with provider-specific
   env (`CLAUDE_CONFIG_DIR` + optional `MODULA_CLAUDE_MODEL`;
   `OPENCODE_CONFIG_DIR` / `CODEX_HOME` + optional `MODULA_PROVIDER_MODEL`) plus
   common env (`MODULA_WORKSPACE`, `MODULA_ENGINE_SOCKET`, `MODULA_AGENT_NAME`,
   `MODULA_AGENT_EXTRA`, `MODULA_LOG_TS`, `MODULA_LOOP_ITER`,
   `MODULA_LOOP_TOTAL`) and `setsid`, so the child survives engine restarts.
   Spawned agents reach the engine through the `modula` CLI over the IPC socket —
   never HTTP or loopback TCP.

6. **Providers are mandatory for agents.** Every `agents.provider_id` must
   resolve to a `providers.id` whose `config_dir` exists on disk. Validation is in
   `ProviderService::runtime_from_provider` at spawn time — there is no fallback.

7. **Pipeline is config-driven.** Never hardcode roadmap status keys
   (`'planning'`, `'in_progress'`, …). Read them from `pipeline_statuses` (per
   workspace); the React app provides them via `PipelineContext`.

8. **Tasks are source-agnostic.** Never hardcode "JIRA". Use `task.source`
   (`'jira'`/`'linear'`/`'github'`/`'internal'`/…) and the React helpers
   (`sourceLabel`, `externalStatusTextClass`, `SourceIcon`). To branch on external
   vs internal, check `task.source !== "internal"` — there is no legacy null case.

9. **Tauri is the only frontend↔engine surface.** The React app never opens a
   socket or speaks gRPC. The Tauri backend owns the client and exposes unary
   calls via `invoke` and streams via `Channel`. No loopback TCP, no
   browser-facing origin, no CORS dependency.

10. **Import the module, not the function.** Bring the parent module into scope
    (`use modula_services::branches;`) and call it qualified
    (`branches::branches_for_task(...)`) so call sites show it isn't local — per
    The Rust Book ch. 7. Types/enums/traits are still imported by name. Functions
    passed as values (`.map(struct_to_json)`) may stay imported.

11. **Comments explain why, not what.** A comment earns its place only by saying
    something the code can't — intent, a non-obvious constraint, a reason for an
    odd choice. Never restate the syntax. Keep them short; default to none. In
    `apps/desktop/` (TSX + CSS) this is stricter: at most **one short line**,
    never a multi-line block.

12. **Shared UI primitives.** Don't reinvent buttons, inputs, or pills — use
    `<Button>`, `<TextInput>`, `<Pill>`, `<FieldRow>`. A variant means a new tone,
    not a new component.

13. **Theming.** Use semantic tokens (`bg-bg`, `bg-surface`, `text-fg`, …) for
    surfaces. Tonal colors (green/yellow/red/blue/purple/orange) stay raw because
    they encode status. Two border tokens by role: `border-edge` is structural
    chrome (the content card, the dividers between docked panes), `border-border`
    outlines components (buttons, pills, inputs, section boxes). Changing one must
    not drag the other along — that's why they're separate.

14. **No prop-drilling for app-wide state.** Four contexts cover everything:
    `WorkspaceContext`, `PipelineContext`, `SnapshotContext`, `SelectionContext`,
    all provided once by `RootLayout` and consumed via hooks.

15. **Rules of Hooks.** `useMemo`/`useEffect` must run on every render — keep them
    ABOVE any early return.

16. **One title bar, one card.** `views/Titlebar.tsx` spans the window and owns
    all window-level chrome — back/forward, sidebar toggle, platform window
    buttons; views never place their own. It and the sidebar sit flat on the
    `bg-chrome` base plate; the content (header, outlet, right-hand drawers) is a
    raised rounded card inset from the bottom-right. Only the title bar's two ends
    are platform-specific: macOS leaves a gutter for the overlay traffic lights,
    Windows and Linux run undecorated with app-drawn caption buttons
    (`components/WindowControls.tsx`). No platform gives a native drag region, so
    `useTitlebarDrag` provides one. Tauri merges platform config with RFC 7386
    semantics — arrays are *replaced*, not merged — so
    `tauri.windows.conf.json` and `tauri.linux.conf.json` each restate the whole
    `app.windows` entry: any size/background change in `tauri.conf.json` must be
    mirrored in both.

17. **Workspace vs framework separation.** `~/.modula/db.sqlite` +
    `~/.modula/<workspace>/` is per-machine state. Agent prompts live in the
    `agents.prompt` column, NOT on disk. `crates/modula_services/templates/` holds
    embedded markdown: `wiki/`, `overview.md`, `workflow.md` are copied to each
    workspace at creation; `agents/<name>.md` and `skills/` are seeded into the
    DB by `modula_db::agents::seed_defaults` and `agent_skills::sync_all`. Edit
    those `.md` files to tweak prompts — no Rust change needed. Never put
    framework code under `~/.modula/`; never put per-machine markdown under
    `templates/`.

## Workspace state (per-machine, not in git)

`~/.modula/db.sqlite` is the single global SQLite DB holding all structured
state: workspaces, workspace_settings, pipeline_statuses, providers, projects,
agents, agent_skills, tasks, variants, labels, task_labels,
task_agent_settings, roadmap_rows, thread_entries, events, agent_runs,
agent_processes, conversations.

`~/.modula/<workspace>/` holds the markdown:

- `specs/<task-uuid>/<variant-uuid>/` — `phases.md`, `phase-N-plan.md`, `phase-N-task.md`
- `logs/` — stream-json log files from agent runs
- `wiki/` — agent-maintained knowledge base
- `overview.md`, `workflow.md` — copied in at workspace creation

## Boundaries

- This repo owns the engine, the desktop app, and the tests.
- `~/.modula/` owns workspace state; it is per-machine and not in this history.
- Spawned agents are detached children (`setsid`); the engine never blocks on them.
- The engine is the only writer of the DB. Agents read and mutate through the
  `modula` CLI over the IPC socket, and every mutation publishes an `events` row
  that the dispatcher routes back to the next agent.
