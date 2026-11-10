#!/usr/bin/env bash
# The dev-only editor and popover mocks must not reach production builds (run after `npm --prefix ui run build`).
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d ui/dist ] || { echo "ui/dist missing: build the UI first" >&2; exit 1; }
if grep -rqE "__DEEBOO_(EDITOR|POPOVER)_MOCK_HARNESS__" ui/dist; then
  echo "dev mock harness found in ui/dist: it must be tree-shaken out of production builds" >&2
  exit 1
fi
