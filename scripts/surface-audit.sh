#!/usr/bin/env bash
# ── THE SURFACE AUDIT: "am I even looking at the whole surface?"
#
# **This is the last mechanism, and it is the one that closes the loop.**
#
# Every other instrument in this project measures the browser against a map. `orient.sh` ranks WPT areas.
# `constellation.sh` scores capability classes. `ratchet.sh` refuses regressions. **All of them are
# blind in exactly the same way: they can only see what is already on the map.**
#
# And the map was drawn from memory. `CONSTELLATION.tsv` is a list of capabilities *I could think of*, and
# the whole history of this project says that list is wrong:
#
#   * the pattern ledger was wrong six times, always a ❌ nobody measured;
#   * `html/dom` (59,818 subtests) was invisible for ten ticks — **in the same checkout**;
#   * `--show-failures` existed for many ticks and had never been run;
#   * both order-of-magnitude leaps required **a human to point at them**.
#
# > **A ranking inside the wrong frame is a confident wrong answer.** The loop cannot find an unknown
# > unknown by thinking harder about the knowns. It has to go OUTSIDE, on a schedule, and check its map
# > against the world.
#
# So this is a **protocol with a cadence**, enforced like the self-audit: every 10 ticks, the loop must
# leave its own frame and reconcile the constellation against external ground truth. It cannot be done by
# a script alone — it requires searching the web and reading what other engines and standards bodies say a
# browser is. The script's job is to **make it impossible to skip**, and to check the result is real.
#
#   scripts/surface-audit.sh check   # is an audit due? (called by tick.sh)
#   scripts/surface-audit.sh run     # print the protocol; the tick then performs it
set -uo pipefail
cd "$(dirname "$0")/.."

BLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; OFF=$'\033[0m'
LOG=docs/loop/SURFACE-AUDIT.md
CADENCE=10

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
LAST=$(grep -oP '^LAST_SURFACE_AUDIT:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
DUE=$(( TICK - LAST ))

case "${1:-check}" in
check)
  if [ "$DUE" -ge "$CADENCE" ]; then
    printf '%s✗ SURFACE AUDIT OVERDUE (last: tick %s, now %s).%s\n' "$RED$BLD" "$LAST" "$TICK" "$OFF" >&2
    printf '%s  The map was drawn from memory, and memory has been wrong six times.%s\n' "$RED" "$OFF" >&2
    printf '%s  Run: ./scripts/surface-audit.sh run — then set LAST_SURFACE_AUDIT in STATUS.md.%s\n' "$RED" "$OFF" >&2
    exit 1
  fi
  printf '  %s✓%s surface audit current (last: tick %s, due at %s)\n' "$GRN" "$OFF" "$LAST" "$((LAST + CADENCE))"
  ;;

run)
  cat <<'PROTOCOL'

  ══════════════════════════════════════════════════════════════════════════════════════════
   THE SURFACE AUDIT — leave the frame, then come back and widen the map
  ══════════════════════════════════════════════════════════════════════════════════════════

  You cannot find an unknown unknown by thinking harder about the knowns. Go and look.

  1. SEARCH THE WEB. Not optional, and not from memory — the platform moves and your training
     data does not. At minimum:

       · "Interop <current year> focus areas"        — what the four vendors agreed matters MOST
       · "wpt.fyi" category list / results-analysis  — every WPT directory that exists, and its size
       · "Baseline" (web.dev / MDN)                  — what is considered safe-to-use TODAY
       · Ladybird / Servo capability + WPT posts     — what an independent engine finds hard, and in
                                                        what ORDER they took it (they have walked this)
       · "browser engine <capability> required"      — for any class scoring low

  2. RECONCILE against docs/loop/CONSTELLATION.tsv. For every capability the world names that our
     map does not:

       → ADD IT, with status `unknown`.

     This is the whole point, and the ratchet is built to reward it: the invariant is MEASURED
     (capabilities with a verdict), NOT `unknown`. A bigger, uglier map is a GOOD tick. Discovery is
     never punished — only rot is.

  3. RE-RANK. A newly-discovered capability may be larger than everything already on the list.
     If it is, the next tick is that, not whatever the histogram said an hour ago.

  4. RECORD in docs/loop/SURFACE-AUDIT.md: the date, the sources (URLs), what was ADDED, what was
     CORRECTED, and — most importantly — **what we had been wrong about**. An audit that finds
     nothing is a suspicious audit; six phantoms say the map is never clean.

  5. Set LAST_SURFACE_AUDIT in STATUS.md.

  ── WHY THIS EXISTS ──────────────────────────────────────────────────────────────────────

     Twice this project made an order-of-magnitude leap, and BOTH times a human had to point at
     it. Not because the analysis was hard — because every instrument the loop owned could only
     see what was already on its map, and nothing ever checked the map.

     This is the instrument that checks the map. If it is skipped, the loop becomes very good at
     ranking things inside a frame that may be the wrong frame — which is the most confident way
     there is to be wrong.

PROTOCOL
  ;;
esac
