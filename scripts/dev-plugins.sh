#!/usr/bin/env bash
# dev.sh with the proprietary plugins swapped in. Leaves the tree dirty.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/swap-plugins.sh"
exec "$ROOT/scripts/dev.sh" "$@"
