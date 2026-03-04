#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_once() {
  echo "[sync] $(date -u +'%Y-%m-%d %H:%M:%S UTC') fetch --all --prune"
  git fetch --all --prune

  if git rev-parse --abbrev-ref --symbolic-full-name "@{u}" >/dev/null 2>&1; then
    echo "[sync] pull --ff-only (upstream)"
    git pull --ff-only
  else
    echo "[sync] no upstream; pulling origin main"
    git pull --ff-only origin main
  fi

  echo "[sync] regenerating LATEST_FINDINGS.md"
  python3 scripts/refresh_findings.py
}

if [[ "${1:-}" == "--watch" ]]; then
  interval="${2:-300}"
  if ! [[ "$interval" =~ ^[0-9]+$ ]]; then
    echo "Usage: $0 [--watch <seconds>]"
    exit 2
  fi
  echo "[sync] watch mode every ${interval}s"
  while true; do
    run_once
    sleep "$interval"
  done
else
  run_once
fi
