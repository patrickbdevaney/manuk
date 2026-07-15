#!/usr/bin/env bash
# ── THE BACKGROUND GRIND SERVICE — keeps a headless agent working on the next tick, forever, independent
#    of any interactive window.
#
# The interactive window kept "handing back": harness wakeups (ScheduleWakeup / Monitor) only fire into a
# live, waiting session, and the idle gap between them reads as a stall. This service does NOT depend on
# that. It is a detached supervisor: it launches a headless `claude` that grinds ticks (self-continuing via
# the Stop hook), waits for it to exit, and immediately relaunches — until the budget in docs/loop/AUTOLOOP
# is spent. One instance only (flock). Progress is visible in `git log` and this service's log; the window
# is free.
#
#   start:   setsid nohup ./scripts/loop-forever.sh >/dev/null 2>&1 &
#   watch:   tail -f .git/manuk-loop-forever.log   (and: git log --oneline)
#   stop:    touch .git/manuk-loop-DISABLED   (pauses)   |   ./scripts/autoloop.sh set <=TICK   (ends)
#            or: kill the flock holder — pkill -f loop-forever.sh
set -uo pipefail
cd "$(dirname "$0")/.."

LOCK=.git/manuk-loop-forever.lock
LOG=.git/manuk-loop-forever.log
KILL=.git/manuk-loop-DISABLED
STATUS=STATUS.md
STORE=docs/loop/AUTOLOOP
WORKING=.git/manuk-working
HEARTBEAT=.git/manuk-loop-heartbeat

# Single instance: if another supervisor holds the lock, exit quietly.
exec 9>"$LOCK" || exit 1
flock -n 9 || { echo "$(date '+%F %T')  another loop-forever already holds the lock — exiting" >>"$LOG"; exit 0; }

say() { printf '%s  %s\n' "$(date '+%F %T')" "$1" >>"$LOG"; }
CLAUDE=$(command -v claude 2>/dev/null || echo "$HOME/.local/bin/claude")

PROMPT='Continue the autonomous Manuk tick loop NOW — you are a headless grind agent, there is no user to hand back to. Read STATUS.md, docs/loop/JOURNAL.md, docs/loop/CONSTITUTION-CHECK.md and CONSTITUTION.MD first (ground truth on disk). Then run as many ticks as you can this invocation: pick the top Pareto capability by FLIP RATE (how many subtests one fix turns green — see the wpt-horizon wiki; NOT raw failing count), implement it, gate it with a falsifiable check, capture the mechanism in docs/wiki, and land it via ./scripts/tick.sh. Touch .git/manuk-working at the top of every command so the watchdogs see you working. Honor THE RATCHET absolutely: a Bar 0 crash or any regression is never traded for a capability — revert instead. Do not stop; keep landing ticks until this process is killed or the budget is spent.'

say "=== loop-forever supervisor START (pid $$) ==="
NOPROG=0
while true; do
  if [ -f "$KILL" ]; then say "DISABLED (kill file present) — pausing 60s"; sleep 60; continue; fi

  TICK=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
  TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
  if [ "$TICK" -ge "$TARGET" ] 2>/dev/null; then
    say "budget spent (tick $TICK ≥ target $TARGET) — loop complete. Supervisor exiting."
    break
  fi

  # Keep the watchdogs (cron daemon) dormant across the brief relaunch gap.
  touch "$WORKING" "$HEARTBEAT" 2>/dev/null || true

  say "launching headless grind agent (at tick $TICK, target $TARGET, $((TARGET-TICK)) left)"
  # The headless agent self-continues via the Stop hook and lands ticks; when it exits we relaunch.
  START=$(date +%s)
  "$CLAUDE" --dangerously-skip-permissions --permission-mode bypassPermissions -p "$PROMPT" >>"$LOG" 2>&1 || true
  DUR=$(( $(date +%s) - START ))

  AFTER=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
  if [ "$AFTER" -gt "$TICK" ]; then
    say "agent exited — progress made (tick $TICK → $AFTER). Relaunching."
    NOPROG=0
  else
    NOPROG=$((NOPROG + 1))
    say "agent exited after ${DUR}s — NO tick landed (attempt $NOPROG)."
    # ── USAGE-EXHAUSTION PAUSE. A fast exit (< 180s) with no tick almost always means the weekly Claude
    # pool is spent (API rejects immediately) — NOT a code problem. Spinning would burn nothing useful and
    # spam relaunches. If a reset time is recorded and still in the future, SLEEP UNTIL THE RESET (+60s),
    # then resume with the fresh pool. See reference/usage/USAGE.md.
    RESET=$(cat .git/manuk-usage-reset 2>/dev/null || echo 0); NOW=$(date +%s)
    if [ "$DUR" -lt 180 ] && [ "$RESET" -gt "$NOW" ]; then
      WAIT=$(( RESET - NOW + 60 ))
      say "⏸ fast-fail (${DUR}s) + pool likely exhausted — sleeping ${WAIT}s until weekly reset $(date -d @"$RESET" '+%F %T'), then resuming"
      sleep "$WAIT"; NOPROG=0; continue
    fi
    # Otherwise it is a genuine no-progress condition (stuck/gated) — back off, never fully stop.
    if [ "$NOPROG" -ge 5 ]; then
      say "⚠ $NOPROG consecutive no-progress launches — backing off 300s (check the log; is a gate blocking?)"
      sleep 300
    fi
  fi
  sleep 8
done
say "=== loop-forever supervisor STOP ==="
