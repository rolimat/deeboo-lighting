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
# A bare punctuated key has no shape: found only through the strip file.
printf '{"id":"a","local_key":"xK#9pQ$2mL!7zR@4"}\n' > "$d/../strip.tmp.json"
trap 'rm -f "$d/../strip.tmp.json"' EXIT
printf 'ok line\nsaw xK#9pQ$2mL!7zR@4 here\n' > "$d/../bare.tmp.log"
trap 'rm -f "$d/../strip.tmp.json" "$d/../bare.tmp.log"' EXIT
"$s" "$d/../bare.tmp.log" >/dev/null || true
[ "$("$s" "$d/../bare.tmp.log" "$d/../strip.tmp.json" || true)" = "2" ] || { echo "bare key: line 2 expected" >&2; exit 1; }
# A key with a quote and a backslash, as JSON stores it (escaped) and as a log would show it (raw).
printf '{"id":"a","local_key":"ab\\"c\\\\d1234567890"}\n' > "$d/../q.tmp.json"
printf 'ok\nraw ab"c\\d1234567890 end\nescaped ab\\"c\\\\d1234567890 end\n' > "$d/../q.tmp.log"
[ "$("$s" "$d/../q.tmp.log" "$d/../q.tmp.json" || true)" = "$(printf '2\n3')" ] || { echo "quote/backslash key: lines 2,3 expected" >&2; exit 1; }
# Fail closed: a grep that errors (exit 2) must not read as a clean log.
mkdir "$d/../fakebin"; printf '#!/bin/sh\nexit 2\n' > "$d/../fakebin/grep"; chmod +x "$d/../fakebin/grep"
rc=0; PATH="$d/../fakebin:$PATH" "$s" "$d/clean.log" >/dev/null 2>&1 || rc=$?
rm -rf "$d/../fakebin"
[ "$rc" = 2 ] || { echo "grep error: expected exit 2, got $rc" >&2; exit 1; }
# The recipe must survive paths with spaces (the real strip file lives under "Application Support").
sp=$(mktemp -d)/"with space"; mkdir -p "$sp"
trap 'rm -f "$d/../strip.tmp.json" "$d/../bare.tmp.log" "$d/../q.tmp.json" "$d/../q.tmp.log"; rm -rf "$(dirname "$sp")"' EXIT
cp "$d/../bare.tmp.log" "$sp/s.log"; cp "$d/../strip.tmp.json" "$sp/strip.json"
if command -v just >/dev/null; then
  [ "$(just --justfile "$d/../../../justfile" --working-directory "$d/../../.." log-scan "$sp/s.log" "$sp/strip.json" || true)" = "2" ] || { echo "spaces: line 2 expected" >&2; exit 1; }
fi
echo "log_scan_fixtures ok"
