# Modula CLI

The packaged build places the `modula` binary on your `PATH` (it's the same binary that bundles the engine). It exposes the engine/host subcommands plus a CRUD client over the running engine — the surface every spawned agent uses.

```bash
modula engine                    # start the gRPC engine over local IPC (no TCP)
modula status                    # health check + workspace list over the IPC socket
modula task list                 # CRUD client commands (see below)
modula install                   # register login-autostart (optional)
```

## Global flags

| Flag | Description |
|:-----|:------------|
| `--socket PATH` | Engine IPC socket/pipe path for both `engine` (serve) and the client commands. Falls back to `$MODULA_ENGINE_SOCKET`, then the default per-user runtime path (`~/.modula/engine.sock`). |
| `--workspace ID\|SLUG` (alias `--ws`) | Target workspace for the task / variant / comment / config commands, overriding `$MODULA_WORKSPACE`. |

## Host commands

### `modula engine`

Run the gRPC engine in the foreground over a local IPC socket (Unix-domain socket; named pipe on Windows). Opens **zero TCP listeners** by default. In dev, `scripts/dev.sh` runs this for you; in a packaged build the desktop app launches the bundled engine on open, so you rarely need to run this by hand.

| Flag | Description |
|:-----|:------------|
| `--grpc-tcp ADDR` | DEV ONLY, INSECURE: also serve gRPC over loopback TCP (e.g. `127.0.0.1:9101`). No auth/TLS; local development only. A non-loopback address is refused without `--grpc-tcp-allow-remote`. |

### `modula status`

Print engine health and the workspace list over the IPC endpoint (the gRPC `HealthService` check that doubles as the "is the engine up?" probe).

### `modula install`

Register the engine to start at login (launchd on macOS, systemd `--user` on Linux, a registry Run key on Windows). Optional — the desktop app launches the engine itself; use this only to also run it headless at login.

### `modula link-cli`

Symlink/shim this binary as `modula` on your `PATH`. `scripts/dev.sh` runs it each dev launch so the terminal `modula` tracks your latest build.

## CRUD client commands

These connect to the running engine over IPC and operate on workspace state. Reads print formatted plain text; writes take a single JSON-string body.

| Command | Purpose |
|:--------|:--------|
| `modula task list` / `get <id>` / `create <body>` / `patch <id> <body>` | Tasks. `patch` routes on body keys: `status`/`notes`/`depends_on` advance the roadmap pipeline status; other keys (`approved`, `title`, `description`, `max_variants`, `worktree`) edit the task row. |
| `modula roadmap list` | Roadmap rows in order (task, pipeline status, depends_on, notes). |
| `modula variant get <id>` / `create <task> <body>` / `patch <id> <body>` | Variant registration (`{"count":N}`) and transitions (`{"status":"…"}` or `{"action":"accept"\|"rework"}`). |
| `modula comment list <task>` / `create <task> <body>` | Task / variant thread entries. |
| `modula config get` | Workspace config (limits, pipeline, providers, projects, agents). |
| `modula workspace list` / `get <id>` / `create <body>` | Workspaces (not workspace-scoped). |
