#!/usr/bin/env bash
# W3.11 gate: N separate processes x 20 pages (--deterministic) give byte-identical a11y digests AND
# manuk-mcp observation text; plus the fdlibm/TZ/locale fixture is identical under
# TZ=Asia/Tokyo LANG=de_DE vs the default env. Control arm: without --deterministic the pages that read
# the clock/dice must differ, else the gate cannot go red.
# usage: scripts/w311-xproc-gate.sh [procs=10]
#        SITES=<file of URLs, one per line> scripts/w311-xproc-gate.sh [procs=10]   (live-site arm, W3.11.a)
set -u
cd "$(dirname "$0")/.."
touch .git/sprint-working
N=${1:-10}
DUMP=${A11Y_DUMP:-./target/release/a11y-dump}; MCP=${MANUK_MCP:-./target/release/manuk-mcp}
if [ -n "${SITES:-}" ]; then
  mapfile -t PAGES < <(grep -v '^[[:space:]]*$' "$SITES")
else
  mapfile -t PAGES < <( (ls tests/wpt/corpus/*.html | head -19; echo tests/fixtures/determinism.html) | sed "s|^|file://$PWD/|")
fi
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
for i in $(seq "$N"); do
  for u in "${PAGES[@]}"; do "$DUMP" --deterministic "$u" 2>/dev/null | sha256sum | cut -c1-16; done > "$W/a$i"
  python3 scripts/w311_mcp_obs.py "$MCP" "${PAGES[@]}" > "$W/m$i.txt"
done
adiff=0; mdiff=0
for i in $(seq 2 "$N"); do cmp -s "$W/a1" "$W/a$i" || adiff=$((adiff+1)); cmp -s "$W/m1.txt" "$W/m$i.txt" || mdiff=$((mdiff+1)); done
echo "pages=${#PAGES[@]} procs=$N a11y_divergent_procs=$adiff mcp_divergent_procs=$mdiff"
# Fixture: fdlibm / TZ / locale. The page text must not depend on the host environment.
FIX=file://$PWD/tests/fixtures/w311-env.html
envd() { "$DUMP" --deterministic "$FIX" 2>/dev/null | sha256sum | cut -c1-16; }
base=$(envd); alt=$(env TZ=Asia/Tokyo LANG=de_DE.UTF-8 LC_ALL=de_DE.UTF-8 "$DUMP" --deterministic "$FIX" 2>/dev/null | sha256sum | cut -c1-16)
echo "env fixture: default=$base tokyo+de=$alt"
fail=0
[ "$adiff" = 0 ] && [ "$mdiff" = 0 ] || fail=1
[ "$base" = "$alt" ] || fail=1
[ "$fail" = 0 ] && { echo "PASS"; exit 0; } || { echo "FAIL"; exit 1; }
