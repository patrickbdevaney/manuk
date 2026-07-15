#!/usr/bin/env bash
# ── THE TICK ORACLE — the background signal that chains ticks the instant the agent goes idle.
#
# Run as a PERSISTENT Monitor (Monitor tool, persistent:true). Each line it prints becomes a
# <task-notification> that re-invokes the agent the moment it is idle — so the loop does not wait on a
# fixed timer; it fires the next tick as soon as the previous one settles. This is the "oracle you keep
# listening to" that the operator asked for.
#
# **Spam-safe by construction — exactly ONE emit per idle→work cycle.** A Monitor that floods events is
# auto-stopped by the harness, which would silently break the chain. So the oracle emits once when the loop
# goes idle and then STAYS SILENT until it sees work resume (the agent touches `.git/manuk-working`), at
# which point it re-arms for the next idle period. Net: ~one notification per tick, never a flood.
#
# The liveness signal is `.git/manuk-working`, touched by the agent while it is actively working a tick
# (its tick commands touch it). Stale ⇒ the agent is idle and a tick is due. Fresh ⇒ work in progress,
# stay quiet.
#
# Stops itself when the budget is spent (prints a final line and exits) or when the kill file is present.
# Off-switches match the rest of the loop: `docs/loop/AUTOLOOP` budget, or `touch .git/manuk-loop-DISABLED`.
set -uo pipefail
cd "$(dirname "$0")/.."

WORKING=.git/manuk-working
KILL=.git/manuk-loop-DISABLED
STATUS=STATUS.md
STORE=docs/loop/AUTOLOOP

IDLE_AFTER=18     # seconds since the last `working` touch that count as "idle, tick due"
POLL=6            # how often to check (local file stat — cheap)

emitted=0         # 1 = already nudged for the current idle period (don't re-emit until work resumes)
n=0

while true; do
  TICK=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
  TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)

  # Budget spent → announce completion once and exit (ends the watch cleanly).
  if [ "$TICK" -ge "$TARGET" ] 2>/dev/null; then
    echo "TICK-ORACLE: LOOP COMPLETE — budget spent at tick $TICK (target $TARGET). Stopping the oracle."
    break
  fi

  # Hard off-switch: stay dormant (do not exit — operator may re-enable).
  if [ -f "$KILL" ]; then emitted=1; sleep "$POLL"; continue; fi

  now=$(date +%s)
  mt=$(stat -c %Y "$WORKING" 2>/dev/null || echo 0)
  age=$(( now - mt ))

  if [ "$age" -ge "$IDLE_AFTER" ]; then
    if [ "$emitted" -eq 0 ]; then
      n=$((n + 1))
      echo "TICK-ORACLE #$n: agent idle ${age}s — $((TARGET - TICK)) ticks left. Run the NEXT tick NOW: orient, implement the top Pareto mechanism, gate it, capture wiki, land via ./scripts/tick.sh. No handback. Touch .git/manuk-working while working."
      emitted=1
    fi
  else
    emitted=0   # work resumed (working is fresh) → re-arm for the next idle period
  fi
  sleep "$POLL"
done
