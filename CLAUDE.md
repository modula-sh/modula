# Modula: dashboard repo (Rust)

One-binary desktop app for the Modula AI software factory.

- Structured state lives in one global SQLite DB at `~/.modula/db.sqlite`.
- Markdown artifacts (spec folders, logs, wiki) live under `~/.modula/<workspace>/`.
- The engine serves gRPC over a local IPC socket, not a TCP port. The desktop app and the `modula` CLI are both its clients.

## Read before editing

| Doc | Covers |
| --- | --- |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate layout, where things live, and the rules for edits. **Start here.** |
| [`docs/PLUGINS.md`](docs/PLUGINS.md) | The plugin seam and the open-source/proprietary split (`plugins/remote`, `../modula-plugins`). |
| [`docs/CLI.md`](docs/CLI.md) | The `modula` command reference. |
| [`docs/MODULA.md`](docs/MODULA.md) | Requirements, packaging, configuration. |

## Run and develop

```bash
bash scripts/dev.sh                          # engine + Vite + Tauri shell
./scripts/dev-plugins.sh                     # same, with the private plugins
./scripts/reset-plugins.sh                   # undo a plugin swap

cargo build --workspace
cargo test --workspace -- --test-threads=1   # single-threaded: each E2E test boots an engine
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml   # shell only
```

## Pass CI before committing

Run what CI gates on, or the PR fails.

- Rust: `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`.
- Frontend (`apps/desktop`): `pnpm exec biome check --write .`, then `pnpm build`.

### Write the commit message

- Use Conventional Commits: `<type>[optional scope]: <description>`. Types: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`, `perf`, `ci`.
- Write one line. Add a short body only when the diff doesn't show the *why*.
- No `Co-Authored-By` trailer.

```text
feat(conversations): derive chat title from first user message
fix(spawn): validate provider config_dir before launching child
```

### Write the pull request

State what changed and why, not a play-by-play. No co-author line.
