#!/usr/bin/env bash
# Samples CPU and RSS of the app process (WebKit helpers excluded) once per second and
# prints mean/max next to the spec 13.1 targets. Reports only; judging is the user's.
# DEEBOO_PERF_PID overrides the process lookup (used to smoke-test the script).
set -euo pipefail
secs=${1:-60}
[[ $secs =~ ^[0-9]+$ ]] && [ "$secs" -gt 0 ] || { echo "usage: perf.sh [SECONDS]" >&2; exit 2; }
pid=${DEEBOO_PERF_PID:-$(pgrep -x deeboo-lighting | head -n1 || true)}
if [ -z "$pid" ]; then
    app="target/release/bundle/macos/Deeboo Lighting.app"
    [ -d "$app" ] || app="/Applications/Deeboo Lighting.app"
    [ -d "$app" ] || { echo "no running app and no bundle: run just dmg first" >&2; exit 1; }
    open "$app"
    for _ in $(seq 1 20); do
        pid=$(pgrep -x deeboo-lighting | head -n1 || true)
        [ -n "$pid" ] && break
        sleep 0.5
    done
    [ -n "$pid" ] || { echo "app did not start" >&2; exit 1; }
fi
echo "sampling pid $pid for ${secs}s"
for _ in $(seq 1 "$secs"); do
    ps -o %cpu=,rss= -p "$pid" || { echo "process $pid exited" >&2; break; }
    sleep 1
done | awk '
    { c += $1; n++; if ($1 > mc) mc = $1; if ($2 > mr) mr = $2 }
    END {
        if (!n) { print "no samples" > "/dev/stderr"; exit 1 }
        printf "samples %d  mean CPU %.1f%%  max CPU %.1f%%  max RSS %.1f MB\n", n, c / n, mc, mr / 1024
        print "RSS counts the WebKit/AppKit library pages shared with the system (clean, evictable); judge memory by the footprint below"
        print "targets (spec 13.1): stopped ~0% CPU, < 30 MB; playing < 2% CPU, < 40 MB"
    }'
# phys_footprint = the memory this process owns (dirty + compressed), what Activity Monitor calls "Memory".
footprint "$pid" 2>/dev/null | awk '/phys_footprint/ { print }' || true
