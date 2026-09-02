# Modula — Dashboard repo (Rust)

One-binary desktop app for the Modula AI software factory. Structured state lives
in a single global SQLite DB at `~/.modula/db.sqlite`; markdown artifacts (spec
folders, logs, wiki) live under `~/.modula/<workspace>/`.

The engine serves gRPC over a local IPC socket — no TCP port. The desktop app and
the `modula` CLI are both clients of it.

## Read before editing

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — crate layout, where things
  live, and the rules that govern edits. **Start here.**
- [`docs/PLUGINS.md`](docs/PLUGINS.md) — the plugin seam and the
  open-source/proprietary split (`plugins/remote`, `../modula-plugins`).
- [`docs/CLI.md`](docs/CLI.md) — the `modula` command reference.
- [`docs/MODULA.md`](docs/MODULA.md) — requirements, packaging, configuration.

## Run / dev

```bash
bash scripts/dev.sh                          # engine + Vite + Tauri shell
./scripts/dev-plugins.sh                     # same, with the private plugins
./scripts/reset-plugins.sh                   # undo a plugin swap

cargo build --workspace
cargo test --workspace -- --test-threads=1   # single-threaded: each E2E test boots an engine
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml   # shell only
```

## Before committing

Run what CI gates on, or the PR will fail:

- Rust: `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`
- Frontend (`apps/desktop`): `pnpm exec biome check --write .`, then `pnpm build`

### Commit messages

- Conventional Commits: `<type>[optional scope]: <description>` (`feat`, `fix`,
  `docs`, `refactor`, `chore`, `test`, `perf`, `ci`).
- One line; add a short body only when the *why* isn't obvious from the diff.
- No `Co-Authored-By` trailer.

```
feat(conversations): derive chat title from first user message
fix(spawn): validate provider config_dir before launching child
```

### Pull requests

Concise: what changed and why, not a play-by-play. No co-author line.
