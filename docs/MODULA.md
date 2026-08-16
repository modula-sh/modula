# Building, Running & Configuring Modula

Operations reference for Modula: how to build it from source, run it for development, package the desktop app, run the tests, and configure it.

See also: [`CLI.md`](CLI.md) for the `modula` command reference, and [`overview.md`](../apps/engine/templates/overview.md) / [`workflow.md`](../apps/engine/templates/workflow.md) for the operating model.

## Requirements

| Requirement | Details |
|:------------|:--------|
| **OS** | macOS, Linux, and Windows. macOS is the primary target; Linux and Windows build and run via per-platform backends and are covered by CI. |
| **Rust** | Pinned via `rust-toolchain.toml` (channel `1.88`) |
| **Node** | Node 20+ with `pnpm` or `npm` for the desktop frontend |
| **Git** | 2.20+ (worktree support) |
| **At least one provider** | `claude`, `opencode`, or `codex` on `PATH` with a configured account |

## Build from source

```bash
git clone https://github.com/modula-sh/modula.git
cd modula
cargo build --workspace
```

## Run in dev mode

```bash
bash scripts/dev.sh          # macOS / Linux
pwsh scripts/dev.ps1         # Windows
```

Starts the engine over its local IPC socket (no TCP port), Vite on `127.0.0.1:9100`, and opens the Tauri native window. The frontend reaches the engine only through the Tauri backend's `invoke` bridge, so there is no browser-only mode.

## Packaged build

```bash
bash scripts/build.sh        # macOS / Linux
pwsh scripts/build.ps1       # Windows
```

Bundles the desktop app (`.app`/`.dmg`/installer) with the engine embedded and puts the `modula` CLI on your PATH. The app manages the engine for you, launching it on open and stopping it on **Quit**.

## Test

```bash
cargo test --workspace -- --test-threads=1
```

End-to-end tests cover workspace lifecycle, task CRUD, the dispatcher's event → run lifecycle, gRPC watch/snapshot streams, and the full agent workflow (PM → Researcher → Worker → Code-Reviewer → Reviewer → `ready_for_acceptance`) driven by `mock-claude`.

## Configuration

Workspace state lives in `~/.modula/db.sqlite` and `~/.modula/<workspace-id>/` (override the root with `MODULA_DIR`). Everything operational — providers, projects, agents, pipeline statuses, tracker scope — is editable from the dashboard or via the `modula` CLI.

| Env | Default | Description |
|:----|:--------|:------------|
| `MODULA_DIR` | `~/.modula/` | Workspace root |
| `MODULA_ENGINE_SOCKET` | `~/.modula/engine.sock` | Engine local IPC socket path (passed to spawned agents; the `modula` CLI resolves it) |
| `MODULA_DISPATCH_INTERVAL_SECS` | `5` | Dispatcher tick interval |
| `MODULA_FRONTEND_PORT` | `9100` | Vite dev port |

For the operating model — agents, events, state transitions, worktree convention — read [`overview.md`](../apps/engine/templates/overview.md). The pseudo-code form of each agent's behavior lives in [`workflow.md`](../apps/engine/templates/workflow.md).
