#!/usr/bin/env bash
set -euo pipefail
# Get the directory of the actual script (handles symlinks)
if command -v realpath >/dev/null 2>&1; then
  SCRIPT_DIR="$(dirname "$(realpath "$0")")"
else
  SCRIPT_DIR="$(dirname "$(readlink "$0" || echo "$0")")"
fi
NODE_BIN="$SCRIPT_DIR/node"
exec "$NODE_BIN" "$SCRIPT_DIR/index.js" "$@"
