#!/usr/bin/env bash
# Undo scripts/swap-plugins.sh. `git checkout` alone is not enough: the real
# plugins add files the stub does not track (build.rs, proto/, migrations/), and
# a leftover build.rs breaks the stub build.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$ROOT" checkout -- plugins/
git -C "$ROOT" clean -fdq plugins/
echo "[reset] plugins restored to their stubs"
