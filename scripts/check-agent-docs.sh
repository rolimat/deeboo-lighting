#!/usr/bin/env bash
# Enforces the agent-docs token budget (progressive disclosure). See AGENTS.md.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
err() { echo "agent-docs: $*" >&2; fail=1; }

[ -f AGENTS.md ] || err "AGENTS.md is missing"
if [ -f AGENTS.md ] && [ "$(wc -l < AGENTS.md)" -gt 80 ]; then err "AGENTS.md is over 80 lines"; fi
if [ "$(readlink CLAUDE.md)" != AGENTS.md ]; then err "CLAUDE.md must be a symlink to AGENTS.md"; fi

shopt -s nullglob
skills=(.claude/skills/*/SKILL.md)
[ "${#skills[@]}" -gt 0 ] || err "no skills found in .claude/skills"

for f in "${skills[@]}"; do
  dir=$(basename "$(dirname "$f")")
  [ "$dir" != rust-engineer ] || continue # vendored unchanged from upstream, see its SOURCE.md
  [ "$(wc -l < "$f")" -le 100 ] || err "$f is over 100 lines"
  name=$(sed -n 's/^name: *//p' "$f" | head -n1)
  desc=$(sed -n 's/^description: *//p' "$f" | head -n1)
  [ -n "$name" ] || err "$f lacks name:"
  [ -n "$desc" ] || err "$f lacks description:"
  [ -z "$name" ] || [ "$name" = "$dir" ] || err "$f name '$name' differs from directory '$dir'"
  [ "${#desc}" -le 250 ] || err "$f description is over 250 chars (${#desc})"
  for ref in "$(dirname "$f")"/references/*.md; do
    grep -qF "references/$(basename "$ref")" "$f" || err "$ref is not linked from $f"
  done
done

[ "$fail" -eq 0 ] || exit 1
echo "agent docs OK"
