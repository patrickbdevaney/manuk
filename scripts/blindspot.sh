#!/usr/bin/env bash
# ── THE BLIND SPOT: what am I not looking at?
#
# **This is the step the methodology was missing, and it is the one that produces the leaps.**
#
# Twice in one session the loop made an order-of-magnitude jump, and **both times a human had to point at
# it.** Not because the analysis was hard — because the loop was optimising inside a frame it never
# questioned:
#
#   * ten ticks were spent on `dom/` (6,484 subtests) while `html/dom` (59,818) sat un-measured **in the
#     same checkout**;
#   * `--show-failures` had existed for many ticks and had never been run, so the work was chosen by
#     guessing instead of by counting.
#
# Both were the same defect: **the loop ranked items inside its aperture and never ranked the aperture.**
#
# > **An area you have not checked out scores zero and is invisible. A test you do not run cannot fail.
# > Absence of measurement is not evidence of coverage — it is the absence of evidence.**
#
# So this script asks the only question that can generate a step-change on its own: *what exists upstream
# that I am not measuring, and how big is it?* The sparse WPT clone carries the **full tree index** even
# for paths it has not materialised — so this costs nothing and can be run every tick, forever.
#
# THE RULE (enforced in GRIND.md): **rank apertures before mechanisms.** If the largest UNMEASURED area is
# bigger than the largest mechanism inside the measured ones, the tick is not to write engine code — it is
# to `scripts/wpt-expand.sh <area>` and look.
set -uo pipefail
cd "$(dirname "$0")/.."

: "${WPT_DIR:=$HOME/wpt}"
BLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; OFF=$'\033[0m'
[ -d "$WPT_DIR/.git" ] || { echo "no WPT checkout at $WPT_DIR" >&2; exit 1; }

# What we currently measure, from the sweep's own area list.
MEASURED=$(git -C "$WPT_DIR" sparse-checkout list 2>/dev/null | tr '\n' ' ')

printf '%s── THE BLIND SPOT — every WPT area upstream, and whether we can even see it%s\n\n' "$BLD" "$OFF"
printf '  %-26s %8s  %s\n' "area" "tests" "status"
printf '  %-26s %8s  %s\n' "──────────────────────────" "────────" "──────"

# The FULL upstream tree — visible in the index even for paths that were never materialised. Grouped one
# level deep for `css/` and `html/` (their subdirectories are separate specs and separate engines' worth
# of work), and top-level everywhere else.
git -C "$WPT_DIR" ls-tree -r --name-only HEAD 2>/dev/null \
  | grep -E '\.(html|xhtml|htm|window\.js|any\.js|worker\.js)$' \
  | grep -vE '^(resources|common|tools|docs|infrastructure|conformance-checkers|\.)' \
  | awk -F/ '{ if (NF>1) print ($1=="css" || $1=="html" ? $1"/"$2 : $1) }' \
  | sort | uniq -c | sort -rn \
  | head -30 \
  | while read -r n area; do
      SEEN=no
      for m in $MEASURED; do
        case "$area" in "$m"|"$m"/*) SEEN=yes ;; esac
        case "$m" in "$area"/*) SEEN=yes ;; esac
      done
      if [ "$SEEN" = yes ]; then
        printf '  %-26s %8s  %s✓ measured%s\n' "$area" "$n" "$GRN" "$OFF"
      elif [ "$n" -ge 800 ]; then
        printf '  %-26s %8s  %s✗ INVISIBLE — larger than most mechanisms we chase%s\n' "$area" "$n" "$RED$BLD" "$OFF"
      else
        printf '  %-26s %8s  %s· not checked out%s\n' "$area" "$n" "$YEL" "$OFF"
      fi
    done

TOTAL=$(git -C "$WPT_DIR" ls-tree -r --name-only HEAD 2>/dev/null \
  | grep -cE '\.(html|xhtml|htm|window\.js|any\.js|worker\.js)$')
SEEN_N=$(git -C "$WPT_DIR" ls-files 2>/dev/null | grep -cE '\.(html|xhtml|htm|window\.js|any\.js|worker\.js)$' || echo 0)

printf '\n  %sWe measure %s of %s upstream test files (%s%%).%s\n' \
  "$BLD" "$SEEN_N" "$TOTAL" \
  "$(awk -v a="$SEEN_N" -v b="$TOTAL" 'BEGIN{printf "%.1f", (b>0?100*a/b:0)}')" "$OFF"

cat <<'RULE'

  THE RULE — rank APERTURES before MECHANISMS:

    An area you have not checked out scores zero and is INVISIBLE. It cannot appear in any
    ranking, so no amount of careful work inside the aperture will ever find it. Both of this
    project's order-of-magnitude leaps were aperture problems, and both times a human had to
    point at them. That is the failure this check exists to end.

    If the largest INVISIBLE area is bigger than the largest mechanism inside the measured
    ones, THE TICK IS TO OPEN THE APERTURE:

        scripts/wpt-expand.sh <area>     # add it to the sparse checkout
        scripts/wpt-sweep.sh             # measure it
        scripts/orient.sh                # re-derive the work list, which has now changed

    Opening an aperture is not overhead. It is the only move that can change what "biggest" means.
RULE
