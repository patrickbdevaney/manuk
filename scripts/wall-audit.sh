#!/usr/bin/env bash
# ── WALL-TIME AUDIT: on a sparse cadence, hunt bloat in the per-tick wall — WITHOUT cutting rigor.
#
# The wall runs every single tick, so a second of needless cost is a second taxed on every future tick,
# forever. The ratchet's WALL invariant catches *regression* (the wall got slower); it does not catch
# *standing bloat* (the wall was never as lean as it could be). This is the proactive half: every N ticks,
# look at where the wall actually spends its seconds and ask whether any of it is removable **without
# dropping a gate or weakening an assertion.** Leanness is a feature; cutting coverage to buy it is not,
# and this audit refuses that trade by construction — it reports, it does not delete.
#
#   scripts/wall-audit.sh check   # is an audit due? (called by tick.sh)
#   scripts/wall-audit.sh run     # show the section breakdown + the rigor-preserving optimisation list
set -uo pipefail
cd "$(dirname "$0")/.."

BLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; CYA=$'\033[36m'; OFF=$'\033[0m'
SECTIONS=.git/manuk-wall-sections
RECEIPT=.git/manuk-verify-receipt
CADENCE=20

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
LAST=$(grep -oP '^LAST_WALL_AUDIT:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
DUE=$(( TICK - LAST ))

case "${1:-check}" in
check)
  if [ "$DUE" -ge "$CADENCE" ]; then
    printf '%s✗ WALL-TIME AUDIT OVERDUE (last: tick %s, now %s).%s\n' "$RED$BLD" "$LAST" "$TICK" "$OFF" >&2
    printf '%s  The wall taxes every tick; a sparse audit keeps it lean without cutting a gate.%s\n' "$RED" "$OFF" >&2
    printf '%s  Run: ./scripts/wall-audit.sh run — then set LAST_WALL_AUDIT in STATUS.md.%s\n' "$RED" "$OFF" >&2
    exit 1
  fi
  printf '  %s✓%s wall-time audit current (last: tick %s, due at %s)\n' "$GRN" "$OFF" "$LAST" "$((LAST + CADENCE))"
  ;;

run)
  TOTAL=$(grep -oP '^seconds:\s*\K[0-9]+' "$RECEIPT" 2>/dev/null || echo "?")
  printf '\n%s══ WALL-TIME AUDIT @ tick %s — total %ss ══%s\n' "$BLD" "$TICK" "$TOTAL" "$OFF"
  if [ -s "$SECTIONS" ]; then
    printf '\n%sWhere the seconds go (most expensive first):%s\n' "$BLD" "$OFF"
    sort -rn "$SECTIONS" | head -12 | awk -F'\t' -v t="$TOTAL" '
      { pct = (t+0>0) ? 100*$1/t : 0
        bar=""; n=int(pct/4); for(i=0;i<n;i++)bar=bar"█"
        printf "  %4ss  %-40s %s %.0f%%\n", $1, substr($2,1,40), bar, pct }'
  else
    printf '  %s(no section breakdown yet — run ./scripts/verify.sh once, then re-run this)%s\n' "$YEL" "$OFF"
  fi
  cat <<'PROTOCOL'

  ── THE AUDIT: for each of the biggest costs, ask ONLY rigor-preserving questions ──────────

  Coverage is sacred. A gate that goes red for a real bug STAYS. The only admissible optimisations are
  the ones that buy the same assertion for fewer seconds:

    1. REDUNDANCY — do two gates stand up a fresh SpiderMonkey runtime to assert overlapping things?
       ~1.5s of every JS gate is runtime startup. If two gates could share one binary/one runtime and
       still each fail independently for their own bug, that is free seconds. (cargo-nextest shares the
       test binary and parallelises harder than `cargo test` — the self-audit already names it.)
    2. PARALLELISM — is the slowest section actually parallel, or serialised by a false dependency? The
       gates are launched concurrently (CARGO_BUILD_JOBS); the perf floors are deliberately NOT (a
       benchmark sharing the machine is not a benchmark). Confirm nothing else got accidentally serial.
    3. CACHING — is anything recomputed that a prior step already produced (a build the gates rebuild,
       a fetch the cache should have served)? Incrementals already live in RAM; live fetches are snapshot-
       cached. Find the next one.
    4. SCOPE — does a gate build MORE than it asserts on (a whole-workspace build where one crate's test
       binary would do)? Narrower target = fewer seconds, same assertion.

  What is NOT admissible, ever: dropping a gate, widening a floor, sampling instead of covering, or
  moving a check "to CI" to make the local wall look fast (CI is out-of-band, not a rigor launder).

  Record in the journal what was found and what (if anything) was trimmed. An audit that finds the wall
  already lean is a fine result — say so. Then set LAST_WALL_AUDIT in STATUS.md.

PROTOCOL
  ;;
esac
