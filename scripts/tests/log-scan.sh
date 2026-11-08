#!/usr/bin/env bash
# log_scan_fixtures: dirty fixture lists each secret line (numbers only), clean fixture passes.
set -euo pipefail
d=$(dirname "$0")/log-scan
s=$(dirname "$0")/../log-scan.sh
rc=0; out=$("$s" "$d/dirty.log") || rc=$?
[ "$rc" = 1 ] || { echo "dirty: expected exit 1, got $rc" >&2; exit 1; }
want=$(grep -n 'DIRTY' "$d/dirty.log" | cut -d: -f1)
[ "$out" = "$want" ] || { echo "dirty: lines '$out' != '$want'" >&2; exit 1; }
"$s" "$d/clean.log" >/dev/null || { echo "clean: expected exit 0" >&2; exit 1; }
echo "log_scan_fixtures ok"
