#!/usr/bin/env bash
# Modula — dev entrypoint.
#
# Builds + runs the engine (gRPC over the local IPC socket), then `tauri dev`
# via the repo-pinned v2 CLI (which starts Vite on 9100 itself and opens the
# native window).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/apps/desktop"

FRONTEND_PORT="${MODULA_FRONTEND_PORT:-9100}"
export MODULA_FRONTEND_PORT="$FRONTEND_PORT"

if command -v pnpm >/dev/null 2>&1; then
  JS=pnpm
elif command -v npm >/dev/null 2>&1; then
  JS=npm
else
  echo "Need pnpm or npm in PATH" >&2
  exit 1
fi
if [[ ! -d "$DESKTOP/node_modules" ]]; then
  echo "[setup   ] frontend: installing deps with $JS"
  ( cd "$DESKTOP" && $JS install )
fi

cleanup() {
  trap - INT TERM EXIT
  jobs -p | xargs -r kill 2>/dev/null || true
}
trap cleanup INT TERM EXIT

echo "[engine  ] building"
cargo build --release -p modula-engine
ENGINE="$ROOT/target/release/modula"

# Put the freshly built CLI on PATH every dev launch (production links itself
# only on update). Best-effort: a link failure must not block the engine.
echo "[cli     ] linking modula onto PATH"
"$ENGINE" link-cli || echo "[cli     ] warning: could not link modula onto PATH" >&2

echo "[engine  ] starting on ${MODULA_ENGINE_SOCKET:-$HOME/.modula/engine.sock}"
( exec "$ENGINE" engine ) &

echo "[tauri   ] starting native shell (Vite on ${FRONTEND_PORT} via beforeDevCommand)"
( cd "$DESKTOP" && MODULA_DEV=1 exec $JS exec tauri dev ) &

wait
