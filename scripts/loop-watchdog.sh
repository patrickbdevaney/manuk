#!/usr/bin/env bash
# ── THE WATCHDOG — ensure exactly one loop-forever supervisor is alive. Run by crond every 2 minutes, so it
# survives what an interactive session cannot: an OOM that kills the whole loop tree, or a reboot. crond is a
# system daemon outside our cgroup, so it is never a victim of the loop's own memory spikes.
#
# CANNOT CASCADE — three guards: (1) this watchdog holds its own flock, so two watchdog runs never overlap;
# (2) it starts a supervisor ONLY when a precise /proc scan finds zero running; (3) the supervisor itself
# holds loop-forever.lock, so even a race can't produce two. It never deletes a lock file (that was the old
# cascade bug — deleting a held flock lets a duplicate acquire a fresh one).
#
# OFF: touch .git/manuk-loop-DISABLED  |  ./scripts/autoloop.sh set <=TICK  |  crontab -e (remove the line)
set -uo pipefail
cd "$(dirname "$0")/.."
LOG=.git/manuk-loop-watchdog.log
LOCK=.git/manuk-watchdog.lock
say(){ printf '%s  %s\n' "$(date '+%F %T')" "$1" >>"$LOG"; }

exec 8>"$LOCK" || exit 1
flock -n 8 || exit 0                                   # another watchdog run already in progress
[ -f .git/manuk-loop-DISABLED ] && exit 0              # hard off-switch
[ -f docs/loop/AUTOLOOP ] && [ -f STATUS.md ] || exit 0

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' docs/loop/AUTOLOOP 2>/dev/null || echo 0)
[ "$TICK" -ge "$TARGET" ] 2>/dev/null && exit 0        # budget spent — nothing to keep alive

# Precise /proc scan: is a loop-forever supervisor (bash running the script) alive?
sup=0
for d in /proc/[0-9]*; do
  [ "$(cat "$d/comm" 2>/dev/null)" = bash ] || continue
  tr '\0' ' ' < "$d/cmdline" 2>/dev/null | grep -q 'scripts/loop-forever.sh' && sup=$((sup+1))
done
[ "$sup" -ge 1 ] && exit 0                             # already running → do nothing

say "no supervisor alive (tick $TICK/$TARGET) — (re)starting loop-forever"
touch .git/manuk-working .git/manuk-loop-heartbeat
# Login shell so node/claude resolve; the supervisor's own flock guarantees single-instance.
setsid nohup bash -lc "$(pwd)/scripts/loop-forever.sh" >/dev/null 2>&1 &
say "launched supervisor via watchdog"
