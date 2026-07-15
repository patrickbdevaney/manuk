#!/usr/bin/env bash
# ── THE RESURRECTION DAEMON: the outer loop that survives what the Stop hook cannot — a dead session.
#
# The Stop hook (scripts/loop-continue.sh) keeps a LIVE agent grinding tick after tick with zero latency.
# But if that session dies — OOM (which has happened here), a closed terminal, a host reboot — nothing is
# left to fire the hook, and the loop silently stops. This script is the backstop. Run by cron on a sparse
# interval, it asks one question: "is a manuk loop actually alive right now, and is there budget left?" If
# the session died and budget remains, it relaunches a fresh headless `claude` in the repo — which then
# self-continues via the Stop hook exactly as before. The cron only has to START one; the hook keeps it up.
#
# Two independent liveness signals, so it never spawns a SECOND agent onto the same repo:
#   1. the heartbeat file — touched by the Stop hook every agent turn (covers the interactive session too)
#   2. the launched-PID file — covers a fresh headless session in the gap before its first Stop fires
# If either says "alive", the daemon exits. Only when BOTH are cold does it resurrect.
#
# OFF-SWITCHES (either one stops all resurrection):
#   • set the budget at/below the current tick:  ./scripts/autoloop.sh set <=TICK
#   • touch the kill file:                        touch .git/manuk-loop-DISABLED
#
#   usage:  scripts/loop-daemon.sh              # the cron entrypoint
#           LOOP_DAEMON_DRY_RUN=1 scripts/loop-daemon.sh   # print the decision, launch nothing
set -uo pipefail
cd "$(dirname "$0")/.."
REPO=$(pwd)

HEARTBEAT=.git/manuk-loop-heartbeat
PIDFILE=.git/manuk-loop.pid
KILL=.git/manuk-loop-DISABLED
LOG=.git/manuk-loop-daemon.log
STORE=docs/loop/AUTOLOOP
STATUS=STATUS.md

STALE_MIN=40          # heartbeat older than this ⇒ stalled (dead OR stuck-idle). LONGER than any ScheduleWakeup sleep (~30m max) so a legit self-driving sleep is never mistaken for a stall.
MIN_GAP_MIN=20        # never relaunch within this many minutes of the last launch (crash-loop guard)

now=$(date +%s)
say() { printf '%s  %s\n' "$(date '+%F %T')" "$1" >> "$LOG"; }

# ── Guards, cheapest first ────────────────────────────────────────────────────────────────────────
[ -f "$STORE" ] && [ -f "$STATUS" ] || { say "no budget/status file — not in a loop repo, skipping"; exit 0; }

if [ -f "$KILL" ]; then say "OFF: $KILL present — resurrection disabled"; exit 0; fi

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
if [ "$(( TARGET - TICK ))" -le 0 ]; then say "budget spent (tick $TICK ≥ target $TARGET) — loop complete, nothing to resurrect"; exit 0; fi

# ── THE ONE LIVENESS SIGNAL: heartbeat freshness. Earlier this stood down whenever the session PROCESS was
# alive — but a session that goes idle WITHOUT a pending self-wakeup is alive yet stuck, and that exact case
# ("the window handed it back without moving forward") was invisible to the old guard. So the rule is now:
# an actively-grinding session touches the heartbeat every turn; if the heartbeat is fresh, stand down; if
# it is STALE past the threshold, the loop is stalled — dead OR stuck-idle — and we resurrect. STALE_MIN is
# set LONGER than any ScheduleWakeup sleep (max ~30m), so a legitimately sleeping self-driving session is
# never mistaken for stalled.
SESSION_PIDFILE=.git/manuk-loop-session.pid
hb_age_min=$(( (now - $(stat -c %Y "$HEARTBEAT" 2>/dev/null || echo 0)) / 60 ))
if [ -f "$HEARTBEAT" ] && [ "$hb_age_min" -lt "$STALE_MIN" ]; then
  say "alive: heartbeat ${hb_age_min}m old (< ${STALE_MIN}m) — a session is grinding, stand down"; exit 0
fi

# A freshly-launched headless agent we own, still within its warmup, also counts as alive.
if [ -f "$PIDFILE" ]; then
  pid=$(cat "$PIDFILE" 2>/dev/null || echo "")
  last=$(( (now - $(stat -c %Y "$PIDFILE" 2>/dev/null || echo 0)) / 60 ))
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && [ "$last" -lt "$MIN_GAP_MIN" ]; then
    say "alive: headless PID $pid launched ${last}m ago (warming up) — stand down"; exit 0
  fi
fi

# ── STALLED. Heartbeat is stale past the threshold and budget remains. A prior session may be alive but
# STUCK (idle, not touching the heartbeat) — kill it so the fresh agent does not compete, then resurrect.
for pf in "$SESSION_PIDFILE" "$PIDFILE"; do
  [ -f "$pf" ] || continue
  p=$(cat "$pf" 2>/dev/null || echo "")
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then
    say "STUCK: PID $p alive but heartbeat ${hb_age_min}m stale — killing the stalled session before resurrect"
    kill "$p" 2>/dev/null; sleep 2; kill -9 "$p" 2>/dev/null || true
  fi
done

# ── RESURRECT. Launch a fresh, detached, headless agent to drive the loop. ──────
PROMPT="Resume the autonomous Manuk engineering loop. Read STATUS.md, docs/loop/JOURNAL.md, and CONSTITUTION.MD first (the loop's ground truth is on disk). Continue the tick/eval loop toward the budget in docs/loop/AUTOLOOP with NO handback: pick the top Pareto capability by FLIP RATE (not raw failing count — see wpt-horizon wiki), implement it, gate it, capture wiki knowledge, land it via ./scripts/tick.sh, and touch .git/manuk-working while working. CRITICAL: end EVERY turn by re-arming a ScheduleWakeup (600s) so the session never goes dormant — that lapse is what stalled the loop before. Honor the ratchet — a Bar 0 crash is never traded for a capability."

if [ "${LOOP_DAEMON_DRY_RUN:-0}" = "1" ]; then
  say "DRY_RUN: would resurrect (tick $TICK/$TARGET). Command: claude --dangerously-skip-permissions -p \"…\""
  printf 'DRY_RUN: would launch a fresh headless claude to resume the loop at tick %s (target %s).\n' "$TICK" "$TARGET"
  exit 0
fi

touch "$HEARTBEAT"   # claim liveness immediately so the next cron tick doesn't double-spawn before first Stop
CLAUDE=$(command -v claude || echo "$HOME/.local/bin/claude")
say "RESURRECT: session dead, budget left (tick $TICK/$TARGET) — launching fresh headless agent"
setsid nohup "$CLAUDE" --dangerously-skip-permissions --permission-mode bypassPermissions \
  -p "$PROMPT" >> "$LOG" 2>&1 &
echo $! > "$PIDFILE"
say "launched headless PID $(cat "$PIDFILE") in $REPO"
