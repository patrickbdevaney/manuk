#!/usr/bin/env bash
# ── THE LOOP BUDGET: how many ticks to run before handing back.
#
# **This is the operator's dial, and it lives on disk, not in a context window.**
#
# The standing directive is "loop autonomously across ticks with no handback." But "no handback" needs a
# floor — an operator who said "1000 ticks" three sessions ago should not have to hold that number in
# their head, and the loop should not have to hold it in a conversation that gets summarised and
# compacted. So the target is a **fact on disk** (`docs/loop/AUTOLOOP`), read mechanically at the start of
# every tick. The operator sets it once, updates it whenever, and the loop obeys it without being retold.
#
# Semantics: the loop runs while `TICK < LOOP_UNTIL_TICK`. When it reaches the target, the loop STOPS and
# reports — the ONE legitimate handback, because the operator asked for exactly this many. Everything else
# (a question, a "should I continue?") is still forbidden; this is the only stop that is *by design*.
#
#   scripts/autoloop.sh check       # continue? exit 0 = yes (with remaining), exit 1 = budget reached
#   scripts/autoloop.sh remaining   # print ticks remaining
#   scripts/autoloop.sh set <K>     # set the target to (current tick + K)
#   scripts/autoloop.sh status      # human-readable summary
set -uo pipefail
cd "$(dirname "$0")/.."

BLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; OFF=$'\033[0m'
STORE=docs/loop/AUTOLOOP

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
REMAIN=$(( TARGET - TICK ))

case "${1:-status}" in
check)
  if [ ! -f "$STORE" ]; then
    printf '%s⚠ no loop budget set (%s missing) — treating as run-forever.%s\n' "$YEL" "$STORE" "$OFF"
    exit 0
  fi
  if [ "$TICK" -ge "$TARGET" ]; then
    printf '%s╔══════════════════════════════════════════════════════════════════════════╗%s\n' "$RED$BLD" "$OFF"
    printf '%s║  LOOP BUDGET REACHED — tick %s of target %s. STOP AND REPORT.            %s\n' "$RED$BLD" "$TICK" "$TARGET" "$OFF"
    printf '%s║  This is the one handback that is BY DESIGN: the operator asked for exactly %s\n' "$RED$BLD" "$OFF"
    printf '%s║  this many ticks. To continue: ./scripts/autoloop.sh set <K>  (or edit %s).%s\n' "$RED$BLD" "$STORE" "$OFF"
    printf '%s╚══════════════════════════════════════════════════════════════════════════╝%s\n' "$RED$BLD" "$OFF"
    exit 1
  fi
  printf '  %s✓%s loop budget: %s%s ticks remaining%s (tick %s → target %s)\n' \
    "$GRN" "$OFF" "$BLD" "$REMAIN" "$OFF" "$TICK" "$TARGET"
  exit 0
  ;;

remaining)
  echo "$REMAIN"
  ;;

set)
  K="${2:-}"
  case "$K" in
    ''|*[!0-9]*) printf '%susage: autoloop.sh set <positive integer K>%s\n' "$RED" "$OFF" >&2; exit 2 ;;
  esac
  NEW=$(( TICK + K ))
  # Preserve the header comment; rewrite only the value line.
  if [ -f "$STORE" ]; then
    sed -i "s/^LOOP_UNTIL_TICK=.*/LOOP_UNTIL_TICK=${NEW}/" "$STORE"
  else
    printf 'LOOP_UNTIL_TICK=%s\n' "$NEW" > "$STORE"
  fi
  printf '  %s✓%s loop budget set: %s more ticks (tick %s → target %s)\n' "$GRN" "$OFF" "$K" "$TICK" "$NEW"
  ;;

status|*)
  if [ "$TICK" -ge "$TARGET" ]; then
    printf '%sLOOP BUDGET: REACHED%s (tick %s of %s) — the loop stops and reports.\n' "$RED$BLD" "$OFF" "$TICK" "$TARGET"
  else
    printf '%sLOOP BUDGET:%s %s ticks remaining (tick %s → target %s).\n' "$BLD" "$OFF" "$REMAIN" "$TICK" "$TARGET"
  fi
  ;;
esac
