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

STALE_MIN=25          # heartbeat older than this ⇒ the session is presumed dead
MIN_GAP_MIN=20        # never relaunch within this many minutes of the last launch (crash-loop guard)

now=$(date +%s)
say() { printf '%s  %s\n' "$(date '+%F %T')" "$1" >> "$LOG"; }

# ── Guards, cheapest first ────────────────────────────────────────────────────────────────────────
[ -f "$STORE" ] && [ -f "$STATUS" ] || { say "no budget/status file — not in a loop repo, skipping"; exit 0; }

if [ -f "$KILL" ]; then say "OFF: $KILL present — resurrection disabled"; exit 0; fi

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
if [ "$(( TARGET - TICK ))" -le 0 ]; then say "budget spent (tick $TICK ≥ target $TARGET) — loop complete, nothing to resurrect"; exit 0; fi

# ── Liveness #0: the ACTIVE DRIVING SESSION's PID is alive. A session driven by ScheduleWakeup (the /loop
# self-continue) SLEEPS between wakeups — during that sleep it stops touching the heartbeat, so heartbeat
# age alone would look "dead" and the daemon would wrongly launch a COMPETITOR. The process, however, is
# still alive. So a session that is driving the loop writes its own PID here; while it lives, stand down.
SESSION_PIDFILE=.git/manuk-loop-session.pid
if [ -f "$SESSION_PIDFILE" ]; then
  spid=$(cat "$SESSION_PIDFILE" 2>/dev/null || echo "")
  if [ -n "$spid" ] && kill -0 "$spid" 2>/dev/null; then
    say "alive: driving session PID $spid still running (may be asleep between ScheduleWakeup ticks) — stand down"; exit 0
  fi
fi

# ── Liveness #1: a headless session we launched is still running ───────────────────────────────────
if [ -f "$PIDFILE" ]; then
  pid=$(cat "$PIDFILE" 2>/dev/null || echo "")
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    say "alive: headless PID $pid still running — nothing to do"; exit 0
  fi
fi

# ── Liveness #2: any session (interactive OR headless) touched the heartbeat recently ──────────────
if [ -f "$HEARTBEAT" ]; then
  age_min=$(( (now - $(stat -c %Y "$HEARTBEAT" 2>/dev/null || echo 0)) / 60 ))
  if [ "$age_min" -lt "$STALE_MIN" ]; then
    say "alive: heartbeat ${age_min}m old (< ${STALE_MIN}m) — a session is grinding, nothing to do"; exit 0
  fi
fi

# ── Crash-loop guard: don't relaunch on top of a launch we just made ───────────────────────────────
if [ -f "$PIDFILE" ]; then
  last=$(( (now - $(stat -c %Y "$PIDFILE" 2>/dev/null || echo 0)) / 60 ))
  if [ "$last" -lt "$MIN_GAP_MIN" ]; then
    say "backoff: last launch ${last}m ago (< ${MIN_GAP_MIN}m) and already dead — waiting a cycle"; exit 0
  fi
fi

# ── RESURRECT. The session died and budget remains. Launch a fresh, detached, headless agent. ──────
PROMPT="Resume the autonomous Manuk engineering loop. Read STATUS.md, docs/loop/JOURNAL.md, and CONSTITUTION.MD first (the loop's ground truth is on disk). Then continue the tick/eval loop toward the budget in docs/loop/AUTOLOOP with NO handback: pick the top Pareto capability (the teed-up next tick is the effective-stack-quota fix), implement it, gate it, capture wiki knowledge, and land it via ./scripts/tick.sh. The Stop hook will keep you going across tick boundaries. Honor the ratchet — a Bar 0 crash is never traded for a capability."

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
