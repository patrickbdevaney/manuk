#!/usr/bin/env bash
# ── LEVER PIVOT — auto-detect when the loop is GRINDING one engine horizon with diminishing daily-driver
# return, and surface a pivot to the current biggest frontier. Systematizes the manual pivot the observer did
# at tick 193 (CSS-paint polish -> OAuth/hydration/CORS/dialog/CJK/media). NOT a gate: advisory only, printed
# by lever-board.sh so the agent sees it when picking the next tick.
#
# STALE := the last K landed feat-ticks CLUSTER in one engine subsystem (the "grinding incremental little
# changes in one horizon" signal). When stale, print the biggest constellation holes (constellation.sh
# --gaps), unknowns first (? outranks X — a probe is cheap and either flips a cell green or reveals a real
# hole), so the agent rotates to a NEW domain instead of mining a diminishing tail. Phase-0 readiness
# (docs/loop/PHASE0-PROGRESS.tsv) tunes the message: FLAT => pivot now; still-climbing => a productive deep
# push, pivot only once the wins shrink.
#
#   scripts/lever-pivot.sh          # print the verdict (stale + pivot frontier, or "not stale")
set -uo pipefail
cd "$(dirname "$0")/.."
K="${LEVER_PIVOT_WINDOW:-5}"
LEDGER=docs/loop/PHASE0-PROGRESS.tsv
B=$'\033[1m'; R=$'\033[31m'; Y=$'\033[33m'; G=$'\033[32m'; C=$'\033[36m'; O=$'\033[0m'

# While the Phase-0 FINISH LINE is locked (explicit user directive: work the 5 levers top-down), clustering on
# those levers' subsystems is EXPECTED, not stale — defer the pivot to the finish line. The readiness meter also
# lags here (a landed lever reads 'missing' until surface-audit reconciles), which would otherwise false-trigger
# staleness. The pivot resumes its steering role once Phase 0 is declared complete.
if [ -f .git/manuk-phase0-finish-line ] && [ ! -f .git/manuk-phase0-complete ]; then
  printf '%s\U0001F3C1 Phase-0 FINISH LINE active%s — the 5 locked levers (fetch-streaming, a11y-states, WebSocket, scroll-anchoring, forced-reflow) ARE the mandate; work them top-down. Clustering on them is expected, not stale. Pivot deferred until Phase 0 is declared complete.\n' "$B" "$O"
  exit 0
fi

# Dominant engine/shell subsystem touched by one commit (top-2 path segment, e.g. engine/css, shell).
area_of() { git show --name-only --format= "$1" 2>/dev/null \
  | grep -E '^(engine/[a-z0-9_]+|shell)/' \
  | sed -E 's#^(engine/[a-z0-9_]+|shell)/.*#\1#' \
  | sort | uniq -c | sort -rn | awk 'NR==1{print $2}'; }

# The last K landed feat-ticks.
shas=$(git log -80 --format='%H %s' 2>/dev/null | grep -iE 'feat.*tick [0-9]+' | head -"$K" | awk '{print $1}')
n=$(printf '%s\n' "$shas" | grep -c .)
[ "$n" -lt "$K" ] && { printf '%s· lever-pivot: only %s feat-ticks on record (<%s) — no staleness verdict yet%s\n' "$C" "$n" "$K" "$O"; exit 0; }

# Count the dominant area across the window.
domarea=""; domn=0
while read -r a c; do [ "$c" -gt "$domn" ] && { domn=$c; domarea=$a; }; done < <(
  for s in $shas; do a=$(area_of "$s"); echo "${a:-misc}"; done | sort | uniq -c | awk '{print $2, $1}')
need=$(( (K*6 + 9) / 10 ))          # >= ceil(0.6*K) of the window in one subsystem = clustered
clustered=0; [ "$domn" -ge "$need" ] && clustered=1

# Phase-0 readiness delta over the window (context, not a hard trigger).
delta="?"; flat=1
if [ -f "$LEDGER" ]; then
  # DATA rows only (a real tick number in col 1) — the header's col-10 is the string "ready_pct".
  d=$(grep -E '^[0-9]' "$LEDGER" | tail -"$K" | awk -F'\t' 'NR==1{f=$10} {l=$10} END{if(NR>=2) printf "%d", l-f}')
  if [ -n "$d" ]; then delta=$d; [ "$d" -gt 0 ] && flat=0; fi
fi

if [ "$clustered" -ne 1 ]; then
  printf '%s✓ lever not stale%s — last %s ticks span multiple subsystems (dominant %s ×%s). Continue the mandate.\n' \
    "$G" "$O" "$K" "${domarea:-mixed}" "$domn"
  exit 0
fi

# Clustered → advise a pivot. Tune by readiness trend.
if [ "$flat" -eq 1 ]; then
  printf '%s⚠ HORIZON STALE — PIVOT DOMAINS.%s last %s ticks clustered in %s%s%s and Phase-0 readiness is FLAT (Δ%s%%).\n' \
    "$R$B" "$O" "$K" "$B" "$domarea" "$O" "$delta"
  printf '  A tick that moves no constellation cell is a diminishing-return TAIL tick. Rotate to a NEW domain.\n'
else
  printf '%s◐ horizon narrowing%s — last %s ticks clustered in %s%s%s, but readiness is still climbing (Δ+%s%%).\n' \
    "$Y$B" "$O" "$K" "$B" "$domarea" "$O" "$delta"
  printf '  A productive deep push — keep going while the wins hold, but line up the next domain now.\n'
fi
printf '  %sBiggest daily-driver holes right now%s (%s? outranks ✗%s — probe an unknown first, it is a cheap tick):\n' "$B" "$O" "$Y" "$O"
./scripts/constellation.sh --gaps 2>/dev/null \
  | sed -n '/BIGGEST HOLE/,/THE RULE/p' \
  | grep -E '^\s+(\?|✗)|^  [a-z]+$' | head -22 | sed 's/^/    /'
printf '  %sfull list: scripts/constellation.sh --gaps  ·  readiness trend: docs/loop/PHASE0-PROGRESS.tsv%s\n' "$C" "$O"
