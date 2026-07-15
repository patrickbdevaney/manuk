#!/usr/bin/env bash
# ── THE AUTONOMOUS-LOOP STOP HOOK: re-prompt this agent to keep grinding ticks until the budget is spent.
#
# Wired as a Claude Code `Stop` hook (see .claude/settings.json). It fires every time the agent finishes a
# response. While the loop budget in `docs/loop/AUTOLOOP` has ticks remaining, it emits a `block` decision
# whose `reason` becomes the agent's next instruction — so the agent does not hand back; it is compelled to
# start (or finish) the next tick. When `TICK` reaches `LOOP_UNTIL_TICK`, it allows the stop and the loop
# ends with a report. The operator changes the budget any time by editing `docs/loop/AUTOLOOP` (or
# `./scripts/autoloop.sh set <K>`), and can end the loop immediately by setting the target at or below the
# current tick — which is the off-switch.
#
# SAFETY: bounded by the budget (never truly infinite); every tick still passes the full ratchet + wall +
# gates before it lands, so nothing unsafe ships no matter how long this runs; and a missing/broken budget
# file fails OPEN (allows the stop), never into a stuck re-prompt loop.
set -uo pipefail
cd "$(dirname "$0")/.."

STORE=docs/loop/AUTOLOOP
STATUS=STATUS.md

# Fail open: if we can't read the budget, let the agent stop rather than trap it in a re-prompt.
[ -f "$STORE" ] && [ -f "$STATUS" ] || exit 0

# ── LIVENESS HEARTBEAT. Touched on every fire — i.e. every agent turn. The cron resurrection daemon
# (scripts/loop-daemon.sh) reads this file's age to distinguish "a session is alive and grinding" (fresh)
# from "the session died — OOM, closed terminal" (stale). This is the signal that makes the outer loop
# safe: the daemon only relaunches when this goes stale, so it never spawns a second competing agent while
# one is still working.
touch .git/manuk-loop-heartbeat 2>/dev/null || true

# A hard OFF-SWITCH that does not require editing the budget: if this file exists, stop the loop now.
[ -f .git/manuk-loop-DISABLED ] && exit 0

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' "$STATUS" 2>/dev/null || echo 0)
TARGET=$(grep -oP '^LOOP_UNTIL_TICK=\K[0-9]+' "$STORE" 2>/dev/null || echo 0)
REMAIN=$(( TARGET - TICK ))

if [ "$REMAIN" -le 0 ]; then
  # Budget spent — allow the agent to stop. It will report the loop is complete.
  exit 0
fi

# Read the hook payload (Claude Code passes JSON on stdin). `stop_hook_active` is true when THIS stop was
# already triggered by a stop hook — we intentionally keep going (the budget is the bound), but we surface
# it so the agent knows it is inside the autonomous loop and not a fresh user turn.
PAYLOAD=$(cat 2>/dev/null || echo '{}')

# The continue-prompt. Kept blunt on purpose: do not stop, do not ask, finish or start the next tick, land
# it through tick.sh, capture wiki knowledge, respect the ratchet. The specific next lever is named so the
# agent never wastes a turn re-deciding the obvious.
REASON=$(cat <<EOF
AUTONOMOUS LOOP ACTIVE — ${REMAIN} tick(s) remaining (docs/loop/AUTOLOOP, target tick ${TARGET}; now at ${TICK}). Do NOT stop, do NOT hand back, do NOT ask for confirmation. Continue the tick/eval loop:

1. If a tick is in progress (uncommitted work, a running wall), finish and LAND it via ./scripts/tick.sh.
2. Otherwise start the next tick: pick the top Pareto mechanism (the teed-up next capability tick is the effective-stack-quota fix — set JS_SetNativeStackQuota from the real thread-stack bounds so deep reflection recursion throws instead of segfaulting — which then unblocks ARIA reflection and the ~35k-subtest reflection backlog; after that, CSS layout breadth: flexbox/grid). Implement it, gate it (with a falsifiable G_* where a capability), capture the durable mechanism in docs/wiki (enforced for engine ticks; retrieve with scripts/wiki-lookup.sh), and land it via ./scripts/tick.sh.
3. Honor every cadence the pre-flight enforces (self-audit, surface-audit, constitution-check, wall-audit) when it comes due.
4. A Bar 0 crash or any ratchet regression is never traded for a capability — revert rather than ship it.

Keep grinding across tick boundaries. Stop and report ONLY when TICK reaches ${TARGET} (the budget), or when the operator interrupts.
EOF
)

# Emit the block decision. `reason` is what the agent reads next.
python3 - "$REASON" <<'PY'
import json, sys
print(json.dumps({"decision": "block", "reason": sys.argv[1]}))
PY
