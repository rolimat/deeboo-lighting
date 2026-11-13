#!/usr/bin/env bash
# Scans a session log for secret shapes. Prints only line numbers, never the matched text.
# Exit 1 on any hit, 0 when clean. Device ids (20-24 chars) are not secret and are not matched;
# the app does not log them anyway. Redacted values ("[REDACTED]") do not match.
set -euo pipefail
# Optional 2nd arg: the strip.json; its local_key is then searched as a fixed string (a bare 16-char
# key with punctuation has no shape to match: only this catches it).
[ $# -ge 1 ] && [ $# -le 2 ] && [ -f "$1" ] || { echo "usage: log-scan.sh <logfile> [strip.json]" >&2; exit 2; }
key='(local_?key|(access|refresh|qr)_?token|terminal_?id|user_?code|token)'
re="${key}[\"']?[[:space:]]*[:=][[:space:]]*[\"']?[^][:space:]\"',}][^[:space:],}]{5,}"   # named secret fields (JSON, k=v, URL query)
re="${re}|tuyaSmart--qrLogin\\?token="                                  # QR text
re="${re}|[A-Za-z0-9]{32,}"                                             # bare token / hex run
# Named values may hold any punctuation (keys do); trade-off: "token: some-word" style prose also hits.
# grep exits 1 for "no match" and 2+ for an error: only the first is clean (fail closed).
scan() { local rc=0; grep -n "$@" || rc=$?; [ "$rc" -le 1 ] || { echo "log-scan: grep failed ($rc)" >&2; exit 2; }; }
# "[REDACTED]" is the logger's own placeholder: blank it before matching so it is never a hit.
hits=$(sed 's/\[REDACTED\]//g' < "$1" | scan -Ei -e "$re" | cut -d: -f1)
if [ $# -eq 2 ]; then
  # The key as stored in the JSON (escaped: \" and \\) and as a log line would show it (raw).
  esc=$(sed -En 's/.*"local_key"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p' "$2" | head -n 1)
  raw=$(printf '%s' "$esc" | sed -e 's/\\"/"/g' -e 's/\\\\/\\/g')
  for k in "$esc" "$raw"; do
    [ -z "$k" ] || hits=$(printf '%s\n%s' "$hits" "$(scan -F -e "$k" -- "$1" | cut -d: -f1)" | sed '/^$/d' | sort -nu)
  done
fi
[ -z "$hits" ] || { echo "$hits"; exit 1; }
