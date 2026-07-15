#!/usr/bin/env bash
# ── THE CONSTITUTION CHECK: "is the loop still building toward the long horizon, or just the next number?"
#
# **This is the anchor between the tick loop and the vision.**
#
# `orient.sh` picks the next tick from the tree. `surface-audit.sh` checks the *map* against the world.
# Neither of them looks UP — at the governing document (`CONSTITUTION.MD`) that says what all of this is
# *for*: H0 Pareto parity → H1 hardening → H2 agentic surface → H3 appanages → H4 speciation, under eight
# invariants. A loop that optimises the local gradient forever can climb a hill that is not the mountain —
# tick 84 did exactly that, banking +721k encoding subtests that the constitution's Pareto discipline (I4)
# says are the *tail*, not the frontier. The number went up; the horizon did not get closer.
#
# > **A tick that advances the scoreboard but not the current horizon's exit gate is drift.** The
# > constitution is the only document that can see that, and nothing was reading it on a schedule.
#
# So this is a **protocol with a cadence**, enforced like the surface audit: every N ticks the loop must
# re-read the constitution, name the horizon it is in, and reconcile its recent work against that
# horizon's *exit gate* — correcting `PART VI` (the ground-truth reconciliation) where the tree has moved,
# and confirming the loop's north star still matches the constitution's Pareto discipline. It cannot be
# done by a script alone; the script's job is to make it **impossible to skip** and to check the result is
# real.
#
#   scripts/constitution-check.sh check   # is a check due? (called by tick.sh)
#   scripts/constitution-check.sh run     # print the protocol; the tick then performs it
set -uo pipefail
cd "$(dirname "$0")/.."

BLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; CYA=$'\033[36m'; OFF=$'\033[0m'
LOG=docs/loop/CONSTITUTION-CHECK.md
DOC=CONSTITUTION.MD
CADENCE=8

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
LAST=$(grep -oP '^LAST_CONSTITUTION_CHECK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
DUE=$(( TICK - LAST ))

case "${1:-check}" in
check)
  if [ ! -f "$DOC" ]; then
    printf '%s✗ CONSTITUTION.MD is missing — the loop has no long horizon to anchor to.%s\n' "$RED$BLD" "$OFF" >&2
    exit 1
  fi
  if [ "$DUE" -ge "$CADENCE" ]; then
    printf '%s✗ CONSTITUTION CHECK OVERDUE (last: tick %s, now %s).%s\n' "$RED$BLD" "$LAST" "$TICK" "$OFF" >&2
    printf '%s  The loop optimises the local gradient; only the constitution sees the mountain.%s\n' "$RED" "$OFF" >&2
    printf '%s  Run: ./scripts/constitution-check.sh run — then set LAST_CONSTITUTION_CHECK in STATUS.md.%s\n' "$RED" "$OFF" >&2
    exit 1
  fi
  printf '  %s✓%s constitution check current (last: tick %s, due at %s)\n' "$GRN" "$OFF" "$LAST" "$((LAST + CADENCE))"
  ;;

run)
  cat <<'PROTOCOL'

  ══════════════════════════════════════════════════════════════════════════════════════════
   THE CONSTITUTION CHECK — look UP, then correct the course
  ══════════════════════════════════════════════════════════════════════════════════════════

  The tick loop climbs the local gradient. This is the step that makes sure the hill is the mountain.

  1. RE-READ CONSTITUTION.MD — the WHOLE thing, not from memory. In particular:
       · PART I  (invariants)      — is any recent tick about to violate one? (esp. I4 Pareto discipline,
                                      I2 never-patch-deps, I3 semantic model lands in lockstep)
       · PART II (progression tree) — which HORIZON are we in? What is its EXIT GATE (binary conditions)?
       · PART VI (ground truth)     — this is the loop's own reconciliation. Has the tree moved past it?

  2. NAME THE HORIZON AND ITS GATE, out loud, in the log. For H0 that is: ~83% WPT across categories,
     oracle-verified across the four corpora, a daily-drivable shell, semantic-API coverage of every
     rendered construct. Then answer, honestly:

       → Did the last ~8 ticks move an EXIT-GATE condition, or only the scoreboard?
       → Is `orient`'s ranking (usage-weighted breadth, tail excluded — §VI.3) still the north star,
         or has a big-but-tail number crept back to the top?
       → Is any invariant being bent? (A capability tick that skips its semantic-model exposure bends I3.)

  3. CORRECT PART VI. The reconciliation drifts as the tree changes. Update what is now DONE, what is now
     the real blocker, and re-derive the direct path to the current horizon's gate. This is the same
     discipline as `orient` re-deriving from the tree — but one level up, from the constitution.

  4. IF THE LOOP HAS DRIFTED, say so and steer: the next tick is whatever the constitution says is closest
     to the current gate, not whatever the histogram ranked highest. Novelty never outranks the gate
     (PART III standing rule).

  5. RECORD in docs/loop/CONSTITUTION-CHECK.md: the date, the tick, the horizon, the gate, the honest
     answer to "gate or scoreboard?", what PART VI was corrected to, and the steer (if any). Then set
     LAST_CONSTITUTION_CHECK in STATUS.md.

  ── WHY THIS EXISTS ──────────────────────────────────────────────────────────────────────

     Tick 84 banked +721,000 encoding subtests — a real win, and the wrong hill. The suite total read
     47.6%; the Pareto-relevant breadth was 32.3%. The loop's own instruments could not see the
     difference, because none of them read the document that defines the frontier. This is the instrument
     that reads it. Skip it, and the loop becomes very good at climbing a hill that is not the mountain.

PROTOCOL
  ;;
esac
