#!/usr/bin/env bash
# Overwrite the NotImplemented plugin stubs with the real ones from
# ../modula-plugins. Run by dev-plugins.sh / build-plugins.sh, not directly.
#
# The tree is left dirty on purpose: this branch is never pushed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLUGINS="$ROOT/../modula-plugins"

if [[ ! -d "$PLUGINS" ]]; then
  echo "Need $PLUGINS (the private plugins repo)" >&2
  exit 1
fi
if [[ ! -d "$ROOT/../modula-shared" ]]; then
  echo "Need $ROOT/../modula-shared (the remote plugin depends on it)" >&2
  exit 1
fi

for src in "$PLUGINS"/*/; do
  name="$(basename "$src")"
  echo "[swap ] $name -> plugins/$name"
  rm -rf "${ROOT:?}/plugins/$name"
  cp -R "$src" "$ROOT/plugins/$name"
done
