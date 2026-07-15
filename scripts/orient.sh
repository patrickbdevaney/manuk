#!/usr/bin/env bash
# ── ORIENT: the loop looks at itself, and derives what to do next.
#
# **This is the first action of every tick, and it replaces remembering.**
#
# For eighty-two ticks the answer to *"what next?"* came from a document — the pattern ledger, the roadmap,
# the last journal entry. Those documents were **wrong six times**, and wrong about their own top priority
# twice. Ten ticks were spent on 9% of the ground that was already checked out, because nobody re-derived
# the picture; they inherited it.
#
# > **A plan is a memory. A measurement is a fact.** This script throws the memory away every tick and
# > rebuilds the picture from the tree: what is broken, what is biggest, what is at risk.
#
# It prints four things, in the order the loop must consider them:
#
#   1. **BAR 0** — anything crashing or hanging. Outranks every other consideration, always.
#   2. **THE RATCHET** — anything that would regress. A tick that trades is not allowed to land.
#   3. **THE WORK LIST** — every WPT area, ranked, and the *failure-message histogram* of the largest.
#      The top row of that histogram IS the next tick, if it is a mechanism.
#   4. **HEALTH** — the wall, the perf floors, duplicate downloads, compiler warnings. A browser that
#      passes tests while getting slower and fatter is not becoming a browser.
#
#   scripts/orient.sh              # full: re-sweeps WPT (slow, honest — do this for a capability tick)
#   scripts/orient.sh --cached     # reuse the last sweep (fast — for a second look inside one tick)
set -uo pipefail
cd "$(dirname "$0")/.."

BLD=$'\033[1m'; GRN=$'\033[32m'; RED=$'\033[31m'; YEL=$'\033[33m'; OFF=$'\033[0m'
: "${WPT_DIR:=$HOME/wpt}"; export WPT_DIR
AREAS=docs/loop/WPT-AREAS.tsv

head_() { printf '\n%s── %s%s\n' "$BLD" "$1" "$OFF"; }

# ═════════════════════════════════════════════════════════════════════════════════════════════
# **THE APERTURE COMES FIRST**, before any ranking, because a ranking inside the wrong frame is a
# confident wrong answer. Both of this project's order-of-magnitude leaps were aperture problems, and both
# times a human had to point at them. That is the failure this section exists to end.
./scripts/blindspot.sh | tail -n +1

# ═════════════════════════════════════════════════════════════════════════════════════════════
head_ "1. BAR 0 — a crash or a hang outranks every score on this page"
HANGS=$(grep -oP '^ORACLE_HANGS:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
printf '  oracle hangs: %s of 265 sites\n' "$HANGS"

# ═════════════════════════════════════════════════════════════════════════════════════════════
head_ "2. THE SWEEP — every area, because the one you are looking at is not the one that matters"
if [ "${1:-}" != "--cached" ] || [ ! -f "$AREAS" ]; then
  ./scripts/wpt-sweep.sh
else
  printf '  (cached — %s)\n' "$(stat -c %y "$AREAS" 2>/dev/null | cut -d. -f1)"
  column -t -s$'\t' "$AREAS" | sed 's/^/  /'
fi

# ═════════════════════════════════════════════════════════════════════════════════════════════
head_ "3. THE RATCHET — would anything go backwards?"
./scripts/ratchet.sh check || true

# ═════════════════════════════════════════════════════════════════════════════════════════════
# The work list. The area with the most FAILING subtests is where the ground is — not the area with the
# lowest percentage, which is a different question with a different answer (a 0%-passing area of 40 tests
# is a rounding error; a 21%-passing area of 60,000 is the whole map).
#
# **PARETO LENS (CONSTITUTION §I4 + §VI.3).** "Most failing subtests" is a Pareto TRAP when one area is a
# deep exotic tail. `encoding` has ~767k *failing* subtests — every one a per-codepoint legacy-CJK case the
# constitution says to DEGRADE, not chase — and left unchecked it would rank #1 forever and drag the loop
# off the parity frontier. So the tail areas are **excluded from the ranking** (not from the ratchet; they
# stay banked and must not regress). The score the loop optimises is *usage-weighted breadth*: an area used
# by every site outranks one used by the pre-2010 web, at equal failing mass. Flexbox at 5% beats encoding
# at 48%.
PARETO_TAIL='^(encoding)'   # areas that are Pareto-COMPLETE for H0 — deep tail, excluded from ranking
head_ "4. THE WORK LIST — the biggest MECHANISM by USAGE-WEIGHTED breadth (not raw subtest count)"
BIGGEST=$(awk -F'\t' -v tail="$PARETO_TAIL" 'NR>1 && $1!="TOTAL" && $1 !~ tail { fail=$3-$2; if (fail>max) { max=fail; a=$1 } } END{print a}' "$AREAS")
FAILING=$(awk -F'\t' -v a="$BIGGEST" '$1==a {print $3-$2}' "$AREAS")
BREADTH=$(awk -F'\t' -v tail="$PARETO_TAIL" 'NR>1 && $1!="TOTAL" && $1 !~ tail {p+=$2; t+=$3} END{printf "%d/%d = %.1f%%", p, t, (t>0?100*p/t:0)}' "$AREAS")
printf '  %sPareto breadth (encoding-tail excluded): %s%s  ← the H0 gauge, not the 47%% headline\n' "$YEL" "$BREADTH" "$OFF"
printf '  largest NON-TAIL area by failing subtests: %s%s%s (%s failing)\n' "$BLD" "$BIGGEST" "$OFF" "$FAILING"
printf '  histogramming its failure messages — this is the ranked work list:\n\n'

timeout 3000 cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt "$BIGGEST" --show-failures 2>&1 \
  | grep -oP '^         \K.*' \
  | sed -E 's/"[^"]*"/"X"/g; s/'"'"'[^'"'"']*'"'"'/'"'"'X'"'"'/g; s/\b[0-9]+\b/N/g' \
  | cut -c1-84 | sort | uniq -c | sort -rn | head -12 | sed 's/^/    /'

cat <<'RULE'

  THE RULE (docs/loop/GRIND.md + CONSTITUTION §VI):
    Take the top row that is a MECHANISM, not an instance. "Reflection" is a tick;
    "fix input.disabled" is four hundred ticks. Prefer a mechanism that also serves the
    NEAR horizon (a real page calls it) — that is where the cheapest tick lives.
    Rank by USAGE-WEIGHTED breadth, not raw subtest count: a mechanism every site needs
    (flexbox, grid, position) outranks a deep exotic tail with more failing subtests.
    OPEN THE APERTURE FIRST: ~8 sub-areas of hundreds are measured. css/* and html/*
    beyond html/dom are unmeasured — a ranking inside the wrong frame is confidently wrong.
    A stub is worse than an absence: if it cannot be done properly, skip it and SAY SO.
RULE

# ═════════════════════════════════════════════════════════════════════════════════════════════
# The OTHER horizon. WPT is the scoreboard; this is the goal. They are nearly orthogonal (measured, tick
# 70), so a loop that reads only one of them optimises blind — which is how ten ticks went into the wrong
# room. Both, in the same breath, every tick.
./scripts/constellation.sh

# ═════════════════════════════════════════════════════════════════════════════════════════════
head_ "5. HEALTH — a browser that passes tests while getting slower and fatter is not becoming a browser"
printf '  verify wall     : %ss (mark %ss)\n' \
  "$(grep -oP '^LAST_WALL_TIME:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo '?')" \
  "$(awk -F'\t' '$1=="WALL"{print $2}' docs/loop/RATCHET.tsv 2>/dev/null || echo '-')"
printf '  duplicate wire  : %s (must be 0 — the same URL downloaded twice is bandwidth, latency, money)\n' \
  "$(awk -F'\t' '$1=="TOTAL"{print $6}' "$AREAS" 2>/dev/null || echo '?')"
printf '  gates live      : %s\n' "$(ls engine/page/tests/g_*.rs shell/tests/g_*.rs 2>/dev/null | wc -l)"
printf '  capabilities    : %s asserted by G_CAPABILITY\n' \
  "$(grep -cE '^\s+\("' engine/page/tests/g_capability.rs 2>/dev/null || echo 0)"
printf '  engine LOC      : %s\n' \
  "$(find engine shell agent -name '*.rs' -not -path '*/target/*' 2>/dev/null | xargs cat 2>/dev/null | wc -l)"

cat <<'VERDICT'

  ── THE VERDICT — how this tick is chosen, mechanically:

     1. Is anything CRASHING or HANGING?              → that is the tick. Bar 0 outranks every score.
     2. Is the largest INVISIBLE area bigger than the → OPEN THE APERTURE. wpt-expand.sh, then re-map.
        largest mechanism inside the visible ones?       An area you cannot see cannot be ranked.
     3. Is a constellation class further behind than  → probe its UNKNOWNs (? outranks ✗), then fix its
        the WPT work list?                               biggest hole. WPT is the scoreboard, not the goal.
     4. Otherwise                                     → the top MECHANISM in the failure histogram.

     Then: probe first · implement · gate · falsify · the wall · tick.sh. Go to 1.
     Never ask. A question to the user is a tick that did not happen.
VERDICT
printf '\n%sORIENTED.%s\n' "$BLD$GRN" "$OFF"
