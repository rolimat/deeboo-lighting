#!/usr/bin/env bash
# Scans a session log for secret shapes. Prints only line numbers, never the matched text.
# Exit 1 on any hit, 0 when clean. Device ids (20-24 chars) are not secret and are not matched;
# the app does not log them anyway. Redacted values ("[REDACTED]") do not match.
set -euo pipefail
[ $# -eq 1 ] && [ -f "$1" ] || { echo "usage: log-scan.sh <logfile>" >&2; exit 2; }
key='(local_?key|(access|refresh|qr)_?token|terminal_?id|user_?code|token)'
re="${key}[\"']?[[:space:]]*[:=][[:space:]]*[\"']?[A-Za-z0-9_-]{6,}"   # named secret fields (JSON, k=v, URL query)
re="${re}|tuyaSmart--qrLogin\\?token="                                  # QR text
re="${re}|[A-Za-z0-9]{32,}"                                             # bare token / hex run
hits=$(grep -nEi -e "$re" -- "$1" | cut -d: -f1) || true
[ -z "$hits" ] || { echo "$hits"; exit 1; }
