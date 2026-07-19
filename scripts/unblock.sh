#!/usr/bin/env bash
# ── UNBLOCK — diagnose a JAMMED loop and apply the known-safe remedy.
#
# WHY THIS EXISTS. Tick 235/236 sat unlanded for ~6 hours with the agent working perfectly the whole
# time. Two complete, tested media ticks (M4 audio, M5 video) ended up parked on wip/ branches while
# the supervisor escalated into a 600s backoff. Not one of those hours was spent on browser
# capability — every one went to a harness defect, and most of the defects were mine.
#
# The through-line: a HEALTHY agent can be blocked indefinitely by harness state it is forbidden to
# touch (scripts/ + cron are observer-owned), and nothing in the loop noticed that "alive and busy"
# is not "landing". agent-doctor.sh DETECTS that. This script ACTS on it.
#
# Every remedy here is safe to run repeatedly and never edits engine code.
#   usage: scripts/unblock.sh [--apply]      (default: diagnose only, change nothing)
set -uo pipefail
cd "$(dirname "$0")/.."
APPLY=0; [ "${1:-}" = "--apply" ] && APPLY=1
R=$'\033[31m'; G=$'\033[32m'; Y=$'\033[33m'; B=$'\033[1m'; O=$'\033[0m'
act(){ printf "  ${Y}→${O} %s\n" "$1"; }
did(){ printf "  ${G}✓${O} %s\n" "$1"; }

printf "${B}══ UNBLOCK ══${O}  (%s)\n\n" "$( [ "$APPLY" = 1 ] && echo APPLY || echo 'diagnose only — pass --apply to act' )"
JAMS=0

# ── 1. UNTRUSTED WALL BLOCKING THE RATCHET ─────────────────────────────────────────────────────
# The original deadlock: ratchet.sh judged STATUS.md's stored wall BEFORE verify.sh ran, and STATUS
# only refreshes from a GREEN receipt — so one bad reading refused every tick forever while the run
# that would replace it never executed. ratchet.sh now demotes an untrusted figure to advisory; this
# verifies that hasn't regressed, because it is the single highest-cost failure we have hit.
WALL=$(grep -oE 'LAST_WALL_TIME: *[0-9]+' STATUS.md 2>/dev/null | grep -oE '[0-9]+$')
MARK=$(awk -F'\t' '$1=="WALL"{print $2}' docs/loop/RATCHET.tsv 2>/dev/null | head -1)
RES=$(grep -oE '^result: *\w+' .git/manuk-verify-receipt 2>/dev/null | awk '{print $2}')
if [ -n "${WALL:-}" ] && [ -n "${MARK:-}" ] && [ "$WALL" -gt $(( MARK * 13 / 10 )) ]; then
  if [ "$RES" = "green" ]; then
    printf "  ${R}✗${O} wall %ss exceeds ceiling %ss from a GREEN receipt — this is a REAL slow wall.\n" "$WALL" "$(( MARK * 13 / 10 ))"
    act "do NOT raise the mark to make it pass. Find what got slow: sort -rn .git/manuk-wall-sections"
    JAMS=$((JAMS+1))
  else
    if ./scripts/ratchet.sh check >/dev/null 2>&1; then
      did "wall ${WALL}s is stale/untrusted (receipt: ${RES:-absent}) and correctly ADVISORY — not blocking"
    else
      printf "  ${R}✗${O} ratchet still refuses on an UNTRUSTED wall — the deadlock breaker regressed\n"
      JAMS=$((JAMS+1))
    fi
  fi
fi

# ── 2. FINISHED WORK PARKED ON wip/ BRANCHES ───────────────────────────────────────────────────
# Parking beats losing work, and the agent was right to do it. But parked ticks are invisible to
# every progress metric, so a loop can look "stalled" while carrying finished, tested capability.
WIPS=$(git branch --list 'wip/*' 2>/dev/null | tr -d ' *')
if [ -n "$WIPS" ]; then
  printf "  ${Y}⚠${O} parked branches holding UNLANDED work:\n"
  for b in $WIPS; do
    printf "      %s — %s\n" "$b" "$(git log -1 --format='%s' "$b" 2>/dev/null | cut -c1-64)"
  done
  act "these are NOT work to redo. Once the gates are green: git cherry-pick <branch>, then ./scripts/tick.sh"
  JAMS=$((JAMS+1))
fi

# ── 3. SUPERVISOR BACKOFF ──────────────────────────────────────────────────────────────────────
# After 3 no-progress launches loop-forever.sh backs off 600s. That is correct when the agent is
# genuinely stuck, and actively harmful once the observer has FIXED the harness cause: the loop
# sits idle waiting out a penalty for a problem that no longer exists.
if tail -40 .git/manuk-loop-forever.log 2>/dev/null | grep -q 'backing off'; then
  printf "  ${Y}⚠${O} supervisor is in no-progress BACKOFF\n"
  JAMS=$((JAMS+1))
  if [ "$APPLY" = 1 ]; then
    # Killing the sleeping supervisor is safe: the cron watchdog relaunches it within ~2min with a
    # reset counter, and the flock guarantees only one lives. Never kill it while an agent is live —
    # the agent inherits fd 9 and would orphan, blocking the next supervisor.
    if ps -eo pid=,cmd= | grep -F 'claude --model claude-opus' | grep -qv 'grep -F'; then
      act "agent is LIVE — not touching the supervisor (killing it now would orphan the agent on the flock)"
    else
      for p in $(ps -eo pid=,cmd= | grep -F 'loop-forever.sh' | grep -v 'grep -F' | awk '{print $1}'); do
        kill -TERM "$p" 2>/dev/null || true
      done
      did "supervisor terminated — the cron watchdog will relaunch it with a fresh counter (<=2min)"
    fi
  else
    act "re-run with --apply to end the backoff early (safe only when no agent is live)"
  fi
fi

# ── 4. STALE WORKING FLAG ──────────────────────────────────────────────────────────────────────
AGE=$(( $(date +%s) - $(stat -c %Y .git/manuk-working 2>/dev/null || echo 0) ))
if [ "$AGE" -gt 1800 ]; then
  printf "  ${Y}⚠${O} working flag stale ${AGE}s — the stall-reaper should be killing the agent tree\n"
  JAMS=$((JAMS+1))
fi

printf "\n${B}══ %s ══${O}\n" "$( [ "$JAMS" -eq 0 ] && printf "NO JAM — the loop can land" || printf "%d JAM(S)" "$JAMS" )"
printf "  Full picture: scripts/agent-doctor.sh\n"
exit 0
