# Modula — Dashboard repo (Rust)

One-binary desktop app for the Modula AI software factory. Structured workspace state lives in a single global SQLite DB at `~/.modula/db.sqlite`; markdown artifacts (spec folders, logs, wiki, agent prompts) live under `~/.modula/<workspace>/`.

- **`apps/engine/`** — Rust crate `modula-engine`; produces the `modula` binary. Subcommands: `engine` (gRPC server over a local IPC socket — no TCP port), `status` (health probe), `install` (writes launchd plist). tonic/prost + tokio + tokio-cron-scheduler + sqlx (sqlite).
- **`apps/desktop/`** — Tauri shell + React frontend (React 19 + Vite + Tailwind). React app sits at `apps/desktop/src/`; Tauri Rust glue at `apps/desktop/src-tauri/` (crate `modula-desktop`). The Tauri Rust backend owns the gRPC client and bridges the webview via `invoke` (unary) and `Channel` (streams).
- **`crates/modula_rpc/`** — protobuf definitions (`proto/*.proto`, one per gRPC service) generated with tonic/prost, the proto↔`tonic::Status` error mapping (`DomainError`), and JSON helpers.
- **`crates/modula_types/`** — plain domain models (`Task`, `Variant`, `Agent`, …) + serde + the single proto↔domain conversion layer. Data and conversions only; no business logic.
- **`crates/modula_client/`** — one `ModulaClient` over the IPC channel; client-owned request structs, domain-returning methods, edge proto conversion + streaming. Shared by the CLI and the Tauri backend.
- **`crates/modula_db/`** — stateless `*Repository` structs wrapping sqlx; every method takes a caller-supplied executor (`impl Executor` / `&mut SqliteConnection`). Owns the schema (`migrations/`) and `modula_db::open`.
- **`crates/modula_platform/`** — cross-platform host plumbing (IPC socket/pipe security, service install, process, paths).
- **`crates/modula_engine_transport/`** — the local IPC endpoint, server bind, and stale-endpoint handling shared by the engine and clients.
- **`crates/modula_mock_claude/`** — test mock for the `claude` CLI (crate `modula-mock-claude`, binary `mock-claude`); reads JSON recipes and emits stream-json output (plus an optional append-line mutation for loop counter tests).
- **`crates/modula_test_support/`** — shared E2E harness (`Harness`) used by the e2e crate. Boots the engine on an ephemeral port and prepends provider shims (`claude`, `opencode`, `codex` → `mock-claude`) to PATH.
- **`tests/e2e/`** — integration test crate (`modula-e2e`). Each `tests/*.rs` file is its own integration binary; they all `use modula_test_support::Harness`.

## Run / dev

```bash
bash scripts/dev.sh                                # engine (IPC socket) + Vite dev server + Tauri shell

cargo build --workspace                            # full workspace build
cargo test --workspace -- --test-threads=1         # E2E tests
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## Before committing

Run the same checks CI gates on, or the PR will fail:

- Rust: `cargo fmt --all` then `cargo clippy --workspace --all-targets -- -D warnings`
- Frontend (`apps/desktop`): `pnpm exec biome check --write .` then `pnpm build`

### Commit messages

- Follow Conventional Commits: `<type>[optional scope]: <description>` (types: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`, `perf`, `ci`).
- Concise — one line; add a short body only when the *why* isn't obvious from the diff.
- No `Co-Authored-By` trailer.

```
feat(conversations): derive chat title from first user message
fix(spawn): validate provider config_dir before launching child
```

### Pull requests

- Concise description: what changed and why, not a step-by-step play-by-play.
- No co-author line.

## Architecture rules — read before editing

1. **Pipeline is config-driven, not code-driven.** Never hardcode roadmap status keys (`'planning'`, `'in_progress'`, etc.). Read them from the `pipeline_statuses` rows (per workspace). The engine snapshot includes `config.pipeline`; the React app provides them via `PipelineContext`.

2. **Tasks are source-agnostic.** Never hardcode "JIRA" in code paths. Use `task.source` (`'jira'` / `'linear'` / `'github'` / `'internal'` / …) and the React helpers (`sourceLabel`, `externalStatusTextClass`, `SourceIcon`/`sourceDisplayName`). To branch on external vs internal, check `task.source !== "internal"` directly — there is no legacy null-source case to special-case.

3. **Shared UI primitives.** React: don't reinvent buttons, inputs, or pills — use `<Button>`, `<TextInput>`, `<Pill>`, `<FieldRow>`. Adding a variant means a new tone, not a new component.

4. **Theming.** Use semantic tokens (`bg-bg`, `bg-surface`, `text-fg`, …) for surfaces. Tonal colors (green/yellow/red/blue/purple/orange) stay raw because they encode meaning (status). Two border tokens, by role: `border-edge` is the app's structural chrome — the content card and the dividers between the panes docked inside it — and `border-border` outlines components (buttons, pills, inputs, section boxes). Changing one must not drag the other along; that's why they're separate.

5. **Providers are mandatory for agents.** Every `agents.provider_id` must resolve to a `providers.id` whose `config_dir` exists on disk. Validation happens in `ProviderService::runtime_from_provider` (called by `services::spawn::resolve`) at spawn time — there is no fallback.

6. **Three-layer data access.** Structured state lives in `~/.modula/db.sqlite` and is reached top-down: thin gRPC handlers (`grpc/*`) call the business layer (`services/*` `*Service` impls), which calls the `modula_db` `*Repository` structs. gRPC handlers **never** touch repositories directly. Repositories are stateless — each method takes a caller-supplied executor — so the owning service holds the `SqlitePool` and opens transactions (`pool.begin()`); the unit of work lives in the service, not scattered across repositories. Never construct SQL outside `modula_db`, and never write YAML to disk for state. Migrations live in `crates/modula_db/migrations/`, applied by `modula_db::open` via `sqlx::migrate!()` at startup.

7. **Spawn = pure Rust.** No bash launcher. `services::spawn` invokes the provider binary via `ProviderRuntime::build_command` with provider-specific env (`CLAUDE_CONFIG_DIR` + optional `MODULA_CLAUDE_MODEL` for claude; `OPENCODE_CONFIG_DIR` + optional `MODULA_PROVIDER_MODEL` for opencode; `CODEX_HOME` + optional `MODULA_PROVIDER_MODEL` for codex) plus common env (`MODULA_WORKSPACE`, `MODULA_ENGINE_SOCKET`, `MODULA_AGENT_NAME`, `MODULA_AGENT_EXTRA`, `MODULA_LOG_TS`, `MODULA_LOOP_ITER`, `MODULA_LOOP_TOTAL`) and `setsid` so the spawned process survives engine restarts. Spawned agents reach the engine through the `modula` CLI, which dials the local IPC socket at `MODULA_ENGINE_SOCKET` — never an HTTP API or loopback TCP.

8. **No prop-drilling for app-wide state.** Four contexts cover everything top-level: `WorkspaceContext`, `PipelineContext`, `SnapshotContext`, `SelectionContext`. All four are provided once by `RootLayout` (the layout route) and consumed via hooks.

9. **Rules of Hooks.** React `useMemo`/`useEffect` must run on every render — keep them ABOVE any early-return branch.

10. **Tauri is the only frontend↔engine surface.** The React app never opens a socket or talks gRPC directly. The Tauri Rust backend owns the generated gRPC client and exposes unary calls via `invoke` and streams via `Channel`; the webview calls those. No loopback TCP, no browser-facing engine origin, no CORS dependency.

11. **Workspace vs framework separation.** `~/.modula/db.sqlite` + `~/.modula/<workspace>/` (spec folders, logs, wiki, overview/workflow docs) is per-machine state. Agent prompts live in the `agents.prompt` column — NOT on disk. `apps/engine/templates/` holds embedded markdown: `wiki/`, `overview.md`, `workflow.md` are copied to each workspace at creation; `agents/<name>.md` are seeded into the `agents.prompt` column by `agents::seed_defaults`. Edit those .md files to tweak prompts — no Rust changes needed. Never put framework code under `~/.modula/`; never put per-machine markdown under `apps/engine/templates/`.

12. **Comments explain why, not what.** Across the codebase, a comment earns its place only by saying something the code can't — intent, a non-obvious constraint, a reason for an odd choice. Never restate what the syntax already shows or narrate the mechanics. Keep them short; default to no comment. In `apps/desktop/` (TSX + CSS) this is stricter: at most **one short line**, never a multi-line block.

13. **Dispatch is event-driven.** Every workspace event goes through the single publish path, `EventService::publish` (services inject it as `EventSink`): persist a row into `events`, then broadcast on the in-process `Bus` for gRPC watch streams — never one without the other. One central tokio task (`services::dispatcher`) ticks every ~5s, evaluates each agent's `rules` against unprocessed events, and spawns matching agents (one `agent_runs` row per spawn, `agent_processes` tracks live PIDs). The dispatcher never mutates task / variant / roadmap status — only agents do, via the API.

14. **Import the module, not the function.** Bring a helper function's parent module into scope (`use crate::services::branches;`) and call it qualified (`branches::branches_for_task(...)`) so call sites show it isn't local — per The Rust Book ch. 7. Types/enums/traits are still imported by name (`use ...::ProviderRuntime;`). Functions passed only as values (`.map(struct_to_json)`) may stay imported.

15. **Typed errors, converted at the boundary.** Fallible code returns `ApiResult<T>` (`core::error::ApiError`, a `thiserror` enum of domain variants — `BadRequest`/`NotFound`/`Forbidden`/`Conflict`/`Internal`). gRPC handlers map it to `tonic::Status` at the boundary via `grpc::error::to_status` (`BadRequest`→`InvalidArgument`, `NotFound`→`NotFound`, `Forbidden`→`PermissionDenied`, `Conflict`→`AlreadyExists`, `Internal`→`Internal`), preserving the human-readable detail. Underlying errors convert via `From` impls so `?` just works — never `.map_err` a generic conversion. A `From` impl may encode only mappings correct at *every* call site: e.g. `From<sqlx::Error>` maps UNIQUE→`Conflict` and everything else (incl. `RowNotFound`) to `Internal`. Site-specific outcomes are explicit at the query: a real not-found is `fetch_optional(...).ok_or_else(|| ApiError::NotFound(...))`, and a site needing a different mapping than the blanket `From` keeps an explicit `.map_err(|e| match e { ... })` (see `roadmap.rs` FK→`NotFound`). Don't add a global mapping (e.g. `RowNotFound`→`NotFound`) that would fire wrongly elsewhere.

16. **One title bar, one card.** `views/Titlebar.tsx` spans the window above everything and owns all window-level chrome — back/forward, the sidebar toggle, and the platform's window buttons; views never place their own. It and the sidebar sit flat on the `bg-chrome` base plate, and the content (header, outlet, and the right-hand diff/changes drawers) is a raised rounded card inset from the window's bottom-right. Only the title bar's two ends are platform-specific: macOS leaves a gutter for the overlay traffic lights, Windows and Linux run undecorated and fill the right end with app-drawn caption buttons (`components/WindowControls.tsx`). No platform gives us a native drag region, so `useTitlebarDrag` provides one over the whole strip. Tauri merges platform config with RFC 7386 semantics — arrays are *replaced*, not merged — so `src-tauri/tauri.windows.conf.json` and `tauri.linux.conf.json` each restate the whole `app.windows` entry: any size/background change in `tauri.conf.json` has to be mirrored in both.

## Where things live

### Engine (`apps/engine/src/`)

```
main.rs                  — clap subcommands (engine, status, install)
server.rs                — tonic gRPC server over local IPC; composes AppState
state.rs                 — AppState: composed *Service impls + Repositories bundle
                           + runtime fields (scheduler, loops, conv_runs, endpoint, Bus)
core/
  paths.rs               — MODULA_DIR
  validation.rs          — regex IDs
  error.rs               — ApiError (domain error; From<DomainError>; mapped to tonic::Status)
grpc/                    — thin gRPC service handlers; call services only, never repos
  error.rs               — to_status: ApiError → tonic::Status
  chunk.rs               — chunked-stream helpers
  health.rs workspace.rs config.rs task.rs roadmap.rs thread.rs label.rs
  provider.rs project.rs agent.rs event.rs run.rs conversation.rs
  snapshot.rs log.rs usage.rs diff.rs wiki.rs
services/                — *Service impls (owning pool + repos + runtime handles) + helpers
  tasks.rs threads.rs labels.rs agents.rs workspaces.rs projects.rs config.rs
  events/ runs.rs conversations.rs usage.rs                 — domain services
  providers/             — ProviderService: CRUD + ProviderRuntime hydration (service.rs)
  diffs.rs pr.rs processes.rs logs.rs snapshot.rs           — services owning cross-domain reads
  diff.rs branches.rs tools.rs wiki.rs mcp_config/          — agnostic fs/git helpers
  spawn.rs scheduler.rs loop_registry.rs                    — agent-launch runtime infra
  dispatcher/            — central event-driven dispatcher
    mod.rs               — tick loop, rule matching, spawn, reap
    expr.rs              — tiny rule expression evaluator (event.* ==, !=, and, or)
templates/               — workspace seed material, embedded via include_dir!
  agent/<name>/prompt.md wiki/SCHEMA.md wiki/index.md wiki/log.md
  overview.md workflow.md config.schema.md
```

The SQLite schema and repositories live in `crates/modula_db/` (migrations under
`crates/modula_db/migrations/`); domain models in `crates/modula_types/`.

### Desktop (`apps/desktop/`)

```
src/                     — React app (App.tsx, router.tsx, views/, components/, contexts/, …)
index.html
vite.config.ts           — dev server only; engine access is via Tauri invoke/Channel (no /api proxy)
package.json tsconfig.json tailwind.config.js postcss.config.js
src-tauri/               — Tauri Rust glue (crate `modula-desktop`)
  src/main.rs src/lib.rs — launchctl load/unload + tray icon
  Cargo.toml build.rs tauri.conf.json icons/
```

### Workspace state (per-machine, NOT in git)

`~/.modula/db.sqlite` — single global SQLite DB. All structured workspace state (workspaces, workspace_settings, pipeline_statuses, providers, projects, agents, agent_skills, tasks, variants, labels, task_labels, task_agent_settings, roadmap_rows, thread_entries, events, agent_runs, agent_processes, conversations).

`~/.modula/<workspace>/`:
- `specs/<task-uuid>/<variant-uuid>/phases.md` + `phase-N-plan.md` + `phase-N-task.md` — researcher's design + worker's checklist
- `logs/` — stream-json log files from agent runs
- `wiki/` — agent-maintained knowledge base
- `overview.md`, `workflow.md` — framework docs copied in on workspace creation

Agent prompts live in `agents.prompt` (DB), not on disk.

## Boundaries

- **This repo** owns engine + desktop + tests.
- **`~/.modula/`** owns workspace state (DB + per-workspace markdown). Per-machine; not in this repo's git history.
- Spawned agents are detached child processes (`setsid`); the engine never blocks on them.
- The engine is the only writer of the DB. Agents read and mutate through the `modula` CLI (gRPC over the local IPC socket at `MODULA_ENGINE_SOCKET`); every mutation publishes a row in `events` that the dispatcher routes back to the next agent.
