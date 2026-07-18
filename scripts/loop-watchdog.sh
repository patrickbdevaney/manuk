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

# List all descendants of a pid (recursive). `pgrep -P` matches by PARENT pid, never a command pattern,
# so this can't self-match the watchdog (unlike a `pgrep -f loop` scan).
descendants(){ local p=$1 c; for c in $(pgrep -P "$p" 2>/dev/null); do echo "$c"; descendants "$c"; done; }
# Signal a process tree, children first, so parents can't respawn a just-killed child.
killtree(){ local p=$1 sig=$2 c; for c in $(pgrep -P "$p" 2>/dev/null); do killtree "$c" "$sig"; done; kill "-$sig" "$p" 2>/dev/null || true; }

exec 8>"$LOCK" || exit 1
flock -n 8 || exit 0                                   # another watchdog run already in progress
[ -f .git/manuk-loop-DISABLED ] && exit 0              # hard off-switch
[ -f docs/loop/AUTOLOOP ] && [ -f STATUS.md ] || exit 0

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' docs/loop/AUTOLOOP 2>/dev/null || echo 0)
[ "$TICK" -ge "$TARGET" ] 2>/dev/null && exit 0        # budget spent — nothing to keep alive

# Precise /proc scan: is a loop-forever supervisor (bash running the script) alive? Collect their pids.
sup=0; SUP_PIDS=""
for d in /proc/[0-9]*; do
  [ "$(cat "$d/comm" 2>/dev/null)" = bash ] || continue
  tr '\0' ' ' < "$d/cmdline" 2>/dev/null | grep -q 'scripts/loop-forever.sh' || continue
  sup=$((sup+1)); SUP_PIDS="$SUP_PIDS ${d#/proc/}"
done

if [ "$sup" -ge 1 ]; then
  # A supervisor is alive — normally we stand down. But FIRST reap a HUNG agent: the supervisor only
  # relaunches on agent EXIT, so a stuck agent (infinite loop / wedged build / self-matching wait-loop)
  # blocks it forever while it still LOOKS alive — the exact "didn't move forward automatically" stall
  # nothing else recovers. The agent touches .git/manuk-working at the top of every command (~1–2min in a
  # healthy tick); if that flag is stale this long AND a `claude` agent is a live descendant of a
  # supervisor, the agent is hung → kill its tree so the supervisor relaunches a fresh one. Running from
  # cron every 2min, this is live within 2min of any code change here — no supervisor restart needed. The
  # descendant gate means a between-ticks gap or usage-limit pause (no claude descendant) is never touched.
  STALL="${MANUK_STALL_SECS:-1800}"
  AGE=$(( $(date +%s) - $(stat -c %Y .git/manuk-working 2>/dev/null || echo 0) ))
  if [ "$AGE" -ge "$STALL" ]; then
    for sp in $SUP_PIDS; do
      for c in $(descendants "$sp"); do
        [ "$(cat /proc/"$c"/comm 2>/dev/null)" = claude ] || continue
        say "⏰ HUNG-AGENT REAPER: working-flag stale ${AGE}s ≥ ${STALL}s — killing hung agent tree (claude $c under supervisor $sp)"
        killtree "$c" TERM; sleep 3; killtree "$c" KILL
        systemctl --user stop manuk-agent-tick.scope 2>/dev/null || true
      done
    done
  fi
  exit 0                                               # supervisor already running → do nothing else
fi

say "no supervisor alive (tick $TICK/$TARGET) — (re)starting loop-forever"
touch .git/manuk-working .git/manuk-loop-heartbeat
# Login shell so node/claude resolve; the supervisor's own flock guarantees single-instance.
setsid nohup bash -lc "$(pwd)/scripts/loop-forever.sh" >/dev/null 2>&1 &
say "launched supervisor via watchdog"
