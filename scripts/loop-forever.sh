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

# ── OOM DILIGENCE #1: protect THIS supervisor from the OOM killer. It is a tiny bash loop; it must survive
# so it can relaunch a fresh agent after an OOM. -900 makes the kernel pick almost anything else first.
echo -900 > /proc/self/oom_score_adj 2>/dev/null || true

# systemd-run --user needs a session bus; when launched from cron/nohup those env vars are absent, so set
# them explicitly (this is why the cgroup cap silently fell back to uncontained before).
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=${XDG_RUNTIME_DIR}/bus}"

# ── OOM DILIGENCE #2: run the agent + ALL its child builds/sweeps/parity inside a MEMORY-CAPPED cgroup, so
# a memory spike OOM-kills only the agent's tree — never this supervisor, never the OS, never the operator's
# other windows. The machine has ~31G; cap the agent at 22G RAM + 6G swap, leaving ~9G for everything else.
# MemoryHigh throttles allocation before the hard MemoryMax kill. If systemd --user is unavailable, fall
# back to a direct (uncontained) launch so the loop never fully stalls — better uncontained than stopped.
launch_agent() {
  if systemd-run --user --scope --quiet \
        --setenv=CARGO_BUILD_JOBS=12 --setenv=WPT_DIR="$HOME/wpt" \
        -p MemoryMax=22G -p MemorySwapMax=6G -p MemoryHigh=19G -p OOMPolicy=kill \
        "$CLAUDE" --model "${MANUK_AGENT_MODEL:-claude-opus-4-8}" --dangerously-skip-permissions --permission-mode bypassPermissions -p "$PROMPT" >>"$LOG" 2>&1
  then return 0; fi
  # Fallback: systemd-run failed — launch directly with just the CARGO cap (still reduces build memory).
  say "⚠ systemd-run unavailable — launching agent UNCONTAINED (CARGO_BUILD_JOBS cap only)"
  CARGO_BUILD_JOBS=12 WPT_DIR="$HOME/wpt" \
    "$CLAUDE" --model "${MANUK_AGENT_MODEL:-claude-opus-4-8}" --dangerously-skip-permissions --permission-mode bypassPermissions -p "$PROMPT" >>"$LOG" 2>&1 || true
}

PROMPT='Continue the autonomous Manuk tick loop NOW — you are a headless grind agent, there is no user to hand back to. Read STATUS.md, docs/loop/JOURNAL.md, docs/loop/CONSTITUTION-CHECK.md and CONSTITUTION.MD first (ground truth on disk). Then run as many ticks as you can this invocation: run scripts/lever-board.sh FIRST and OBEY its PHASE MANDATE — this phase builds DAILY-DRIVER CAPABILITY, not raw WPT-flip count. html/dom is reasonably done (~93%); do NOT grind html/dom or dom for +N flips. Pick EITHER a CSS-LAYOUT tick (flexbox/grid/sizing/position/values/overflow — flex/grid INTRINSIC SIZING, min/max-content propagation via Taffy #204, is the core lever) OR a MEDIA tick (build the MediaSource/SourceBuffer JS surface and BIND a media framework GStreamer/FFmpeg — do NOT hand-write codecs). For layout, run `manuk-wpt wpt css/css-flexbox --show-failures` (or css-grid / css-sizing) to get the histogram and implement the top mechanism; a layout or media fix that makes real pages render/play correctly BEATS a bigger html/dom +N. Gate it with a falsifiable check (for media, a sample stream buffers and plays — not a WPT count), capture the mechanism in docs/wiki, and land it via ./scripts/tick.sh. Touch .git/manuk-working at the top of every command so the watchdogs see you working. SCOPE — CRITICAL: the loop HARNESS (this service, scripts/tick.sh, scripts/verify.sh, scripts/ramdisk.sh, scripts/wpt-sweep.sh, the watchdog, cgroups) is DONE and OWNED BY THE OBSERVER. Do NOT edit, 'fix', or optimize ANY scripts/ or harness file — not even if the wall is slow or something seems broken. If a harness problem blocks you, write one line in docs/loop/JOURNAL.md and CONTINUE with browser work; the observer handles all infrastructure. Every tick must be PURE BROWSER CAPABILITY per CONSTITUTION.MD PART VII / docs/loop/V1-SCOPE.md (rendering parity vs the real internet + the agentic surface). ATOMICITY: start each tick from a clean tree — if leftover WIP from a crashed tick will not cleanly complete, `git checkout -- .` back to the last commit and redo the tick fresh; never build on top of inconsistent partial state. Honor THE RATCHET absolutely: a Bar 0 crash or any regression is never traded for a capability — revert instead. Do not stop; keep landing ticks until this process is killed or the budget is spent.'

say "=== loop-forever supervisor START (pid $$) ==="
NOPROG=0
while true; do
  if [ -f "$KILL" ]; then say "DISABLED (kill file present) — pausing 60s"; sleep 60; continue; fi

  # STATUS.md's TICK field goes stale (the agent numbers ticks from git, not this field), so read the tick
  # number from git — otherwise the progress check below false-fails and the supervisor backs off forever.
  TICK=$(git log --oneline -40 2>/dev/null | grep -oiE 'tick [0-9]+' | head -1 | grep -oE '[0-9]+' || echo 0)
  TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
  if [ "$TICK" -ge "$TARGET" ] 2>/dev/null; then
    say "budget spent (tick $TICK ≥ target $TARGET) — loop complete. Supervisor exiting."
    break
  fi

  # Keep the watchdogs (cron daemon) dormant across the brief relaunch gap.
  touch "$WORKING" "$HEARTBEAT" 2>/dev/null || true

  say "launching headless grind agent (at tick $TICK, target $TARGET, $((TARGET-TICK)) left)"
  # The headless agent self-continues via the Stop hook and lands ticks; when it exits we relaunch.
  # PROGRESS = a NEW git commit landed. git HEAD is the source of truth; STATUS.md's TICK field is stale, so
  # comparing it before/after always read "no progress" and forced a 600s backoff even while ticks landed.
  BEFORE=$(git rev-parse HEAD 2>/dev/null || echo none)
  START=$(date +%s)
  launch_agent   # memory-capped cgroup (falls back to uncontained if systemd --user is down)
  DUR=$(( $(date +%s) - START ))

  AFTER=$(git rev-parse HEAD 2>/dev/null || echo none)
  if [ "$AFTER" != "$BEFORE" ]; then
    say "agent exited — progress made (git ${BEFORE:0:8} -> ${AFTER:0:8}, now at tick $(git log --oneline -40 2>/dev/null|grep -oiE 'tick [0-9]+'|head -1)). Relaunching."
    NOPROG=0
  else
    NOPROG=$((NOPROG + 1))
    say "agent exited after ${DUR}s — NO tick landed (attempt $NOPROG)."
    # ── USAGE-EXHAUSTION PAUSE. A fast exit (<180s) with no tick that printed "session/usage limit" means the
    # Claude pool is spent — NOT a code problem. Churning 600s relaunches burns pool on doomed retries. PARSE
    # the real reset clock time from the agent's OWN output ("resets H:MMam (TZ)") and SLEEP UNTIL IT (+60s);
    # the old code trusted .git/manuk-usage-reset, which nothing updates, so it never engaged and just churned.
    if [ "$DUR" -lt 180 ] && tail -25 "$LOG" | grep -qiE 'session limit|usage limit|rate limit'; then
      RT=$(tail -25 "$LOG" | grep -oiE 'resets [0-9]{1,2}:[0-9]{2} ?(am|pm)?' | tail -1 | sed -E 's/^resets +//I'); NOW=$(date +%s)
      TGT=""; [ -n "$RT" ] && TGT=$(date -d "$RT" +%s 2>/dev/null)
      if [ -n "$TGT" ]; then
        [ "$TGT" -le "$NOW" ] && TGT=$((TGT + 86400))          # clock time already passed today → next occurrence
        DELTA=$(( TGT - NOW ))
        if [ "$DELTA" -gt 21600 ]; then                        # >6h out for a 5h session window = message is STALE
          say "⏸ session-limit message stale (reset already passed) — retrying now"; sleep 15; NOPROG=0; continue
        fi
        WAIT=$(( DELTA + 60 ))
        say "⏸ session limit — sleeping ${WAIT}s until reset '$RT' ($(date -d @"$TGT" '+%F %T')), then resuming"
        echo "$TGT" > .git/manuk-usage-reset; sleep "$WAIT"; NOPROG=0; continue
      fi
      say "⏸ session limit but reset time unparseable — pausing 900s (not churning)"; sleep 900; NOPROG=0; continue
    fi
    # Otherwise it is a genuine no-progress condition (stuck/gated) — back off, never fully stop.
    if [ "$NOPROG" -ge 3 ]; then
      say "⚠ $NOPROG consecutive no-progress launches — backing off 600s (check the log; is a gate blocking?)"
      sleep 600
    fi
  fi
  sleep 8
done
say "=== loop-forever supervisor STOP ==="
