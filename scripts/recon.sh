#!/usr/bin/env bash
# ── RECON: fan out across EVERY area cheaply, and see the whole map before committing a tick to any of it.
#
# **This is the aperture-widening step, and it is what the loop could not do for itself.**
#
# A full sweep of every WPT area is expensive — tens of minutes — so the loop never ran one, and therefore
# never *saw* the areas it was not already in. That is how ten ticks went into `dom/` (6,484 subtests) while
# `html/dom` (59,818) sat un-measured in the same checkout, and how `html/canvas` stayed invisible even
# after we built a canvas rasterizer.
#
# The fix is not to run the expensive thing more often. It is to run a **cheap, unbiased sample** of
# everything, every tick:
#
#   * take the first N test files of each area (N=60 by default — enough to estimate a pass rate);
#   * measure the sample's pass rate;
#   * multiply by the area's KNOWN full test-file count to estimate **how many subtests are failing there**.
#
# **Rank by estimated FAILING subtests, not by percentage.** A 0%-passing area of 40 tests is a rounding
# error. A 21%-passing area of 60,000 is the whole map. Percentage answers *"how are we doing here?"*;
# failing-count answers *"where is the ground?"* — and only the second one chooses a tick.
#
# The estimate is a *scouting* number and is labelled as one. It never enters the ratchet, never enters the
# cadence ledger, and is never reported as a result. It exists to decide **where to point the expensive
# instrument**, which is exactly the job it is honest enough to do.
#
#   scripts/recon.sh            # sample every area, rank by estimated failing subtests
#   scripts/recon.sh 120        # a bigger sample (slower, tighter estimate)
set -uo pipefail
cd "$(dirname "$0")/.."

: "${WPT_DIR:=$HOME/wpt}"; export WPT_DIR
N="${1:-60}"
BLD=$'\033[1m'; OFF=$'\033[0m'; GRN=$'\033[32m'; YEL=$'\033[33m'
OUT=docs/loop/RECON.tsv
TMP=$(mktemp)

# Every area that is materialised and has real tests. Discovered, not hardcoded — an area added by
# `wpt-expand.sh` appears here on the next run with no edit, which is the point: the map maintains itself.
mapfile -t AREAS < <(
  cd "$WPT_DIR" && find . -mindepth 1 -maxdepth 2 -type d \
    -not -path './.git*' -not -path './resources*' -not -path './common*' \
    -not -path './tools*' -not -path './docs*' -not -path './infrastructure*' \
    2>/dev/null | sed 's|^\./||' | while read -r d; do
      # `css/*` and `html/*` are separate specs; everything else groups at the top level.
      case "$d" in
        css|html) continue ;;
        css/*|html/*) : ;;
        */*) continue ;;
      esac
      n=$(find "$d" -maxdepth 3 -name '*.html' 2>/dev/null | wc -l)
      [ "$n" -ge 40 ] && echo "$d $n"
    done | sort -k2 -rn
)

printf '%s── RECON — a cheap sample of every area, ranked by ESTIMATED FAILING SUBTESTS%s\n' "$BLD" "$OFF"
printf '   (sample of %s files per area. These are SCOUTING numbers: they choose where to point the\n' "$N"
printf '    expensive instrument, and they never enter the ratchet or the cadence ledger.)\n\n'
printf '  %-24s %7s %8s %9s  %s\n' "area" "files" "sample" "est.fail" "verdict"
printf '  %-24s %7s %8s %9s  %s\n' "────────────────────────" "───────" "────────" "─────────" "───────"

printf 'area\tfiles\tsample_pass\tsample_total\test_failing\n' > "$TMP"

for row in "${AREAS[@]}"; do
  a=${row%% *}; files=${row##* }
  J=$(mktemp)
  RAW=$(timeout 900 cargo run -q -p manuk-wpt --release --features spidermonkey -- \
        wpt "$a" --limit "$N" --json "$J" 2>&1)
  LINE=$(echo "$RAW" | grep -oE 'subtests [0-9]+/[0-9]+' | tail -1)
  SP=$(echo "$LINE" | grep -oE '^subtests [0-9]+' | grep -oE '[0-9]+'); SP=${SP:-0}
  ST=$(echo "$LINE" | grep -oE '/[0-9]+' | tr -d '/'); ST=${ST:-0}
  if [ "$ST" -eq 0 ]; then
    rm -f "$J"; printf '  %-24s %7s %8s %9s  (no verdict — skipped)\n' "$a" "$files" "-" "-"; continue
  fi

  # ── ESTIMATE WITH THE MEDIAN, NOT THE MEAN. The first version multiplied the sample's MEAN
  # subtests-per-file by the area's file count — and `encoding` came out at **810,520 estimated failing
  # subtests**, because a single generated file in the sample holds a test per Unicode codepoint. One
  # outlier moved the estimate by two orders of magnitude and put a degenerate area at the top of the map.
  #
  # The median is immune to that, and the whole job of this number is to CHOOSE WHERE TO LOOK — an
  # estimator that a single file can hijack chooses wrong, confidently. (`css/selectors` was likewise
  # estimated at 15,664 failing when the real, fully-measured figure is 1,313.)
  MED=$(python3 -c "
import json,sys,statistics
rows=[json.loads(l) for l in open('$J') if l.strip()]
t=[r['total'] for r in rows if r.get('total',0)>0]
print(statistics.median(t) if t else 0)
" 2>/dev/null || echo 0)
  EST=$(awk -v med="$MED" -v sp="$SP" -v st="$ST" -v f="$files" \
    'BEGIN{ fail = 1 - (sp/st); printf "%d", med * f * fail }')
  PCT=$(awk -v p="$SP" -v t="$ST" 'BEGIN{printf "%.0f", 100*p/t}')
  rm -f "$J"
  printf '%s\t%s\t%s\t%s\t%s\n' "$a" "$files" "$SP" "$ST" "$EST" >> "$TMP"

  V=""
  [ "$EST" -ge 5000 ] && V="${GRN}${BLD}◄ THE GROUND IS HERE${OFF}"
  [ "$EST" -ge 1000 ] && [ -z "$V" ] && V="${YEL}worth a tick${OFF}"
  printf '  %-24s %7s %6s%%  %9s  %b\n' "$a" "$files" "$PCT" "$EST" "$V"
done

{ head -1 "$TMP"; tail -n +2 "$TMP" | sort -t$'\t' -k5 -rn; } > "$OUT"
rm -f "$TMP"

printf '\n  %sRanked map → %s%s\n' "$BLD" "$OUT" "$OFF"
printf '  Next: take the top area, run `wpt <area> --show-failures`, histogram the messages,\n'
printf '  and fix the largest MECHANISM. (docs/loop/GRIND.md)\n'
