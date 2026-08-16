#!/usr/bin/env bash
# Modula — full local release build.
#
# Builds the engine, stages it as the desktop sidecar for the host target, then
# bundles the desktop app (.app / .dmg / installer) with the engine embedded.
# The installed app self-launches the bundled engine on open and places the
# `modula` CLI on PATH itself — there is no separate install step.
#
#   bash scripts/build.sh            engine sidecar + desktop bundle
#   bash scripts/build.sh --open     also reveal the bundle dir when done (macOS)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/apps/desktop"
SIDECAR_DIR="$DESKTOP/src-tauri/binaries"

DO_OPEN=0
for arg in "$@"; do
  case "$arg" in
    --open) DO_OPEN=1 ;;
    -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown arg: $arg" >&2; exit 2 ;;
  esac
done

# tauri.conf.json's beforeBuildCommand is hardcoded to `pnpm build`, so pnpm is
# required for the bundle regardless of what else is installed.
if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is required for the desktop bundle (tauri beforeBuildCommand uses it). Install it: https://pnpm.io/installation" >&2
  exit 1
fi

TRIPLE="$(rustc --print host-tuple)"
EXT=""; [[ "$TRIPLE" == *windows* ]] && EXT=".exe"

echo "[engine  ] building release binary"
cargo build --release -p modula-engine

echo "[sidecar ] staging engine -> binaries/modula-$TRIPLE$EXT"
mkdir -p "$SIDECAR_DIR"
cp "$ROOT/target/release/modula$EXT" "$SIDECAR_DIR/modula-$TRIPLE$EXT"

if [[ ! -d "$DESKTOP/node_modules" ]]; then
  echo "[frontend] installing deps with pnpm"
  ( cd "$DESKTOP" && pnpm install )
fi

echo "[bundle  ] tauri build (engine bundled as sidecar)"
# Updater artifacts need TAURI_SIGNING_PRIVATE_KEY (CI-only); skip them locally.
( cd "$DESKTOP" && pnpm tauri build --config src-tauri/tauri.bundle.conf.json --config '{"bundle":{"createUpdaterArtifacts":false}}' )

BUNDLE_DIR="$ROOT/target/release/bundle"
echo "[bundle  ] artifacts under: $BUNDLE_DIR"
[[ -d "$BUNDLE_DIR" ]] && find "$BUNDLE_DIR" -maxdepth 2 \( -name '*.app' -o -name '*.dmg' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.exe' \) -print 2>/dev/null || true

if [[ "$DO_OPEN" == "1" && "$(uname)" == "Darwin" && -d "$BUNDLE_DIR" ]]; then
  open "$BUNDLE_DIR"
fi

echo "[done    ] install the app from the bundle dir; it launches the engine on open."
