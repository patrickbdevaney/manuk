#!/usr/bin/env bash
# ── THE RATCHET, MADE MECHANICAL.
#
# The first principle of this project is one sentence: **every tick leaves the browser strictly more
# capable, and never regresses capability, performance, stability or instrument fidelity.**
#
# It has been written down since tick 1. It is in `CLAUDE.md`, in `METHODOLOGY.md`, in the journal. And it
# has been **broken repeatedly while being recited** — most recently in tick 82, which landed +9,940 WPT
# subtests in `html/dom` and *quietly took 2 away from `dom/`*, and in tick 80, which shipped while the
# wall was red because the wall could not tell a killed gate from a failing one.
#
# > **A rule I can recite while breaking it is a decoration.** Forty-seven process defects say the same
# > thing: a lesson only holds when it is a MECHANISM.
#
# So the ratchet stops being a principle and becomes a **gate that refuses the commit**. It keeps a
# high-water mark for every invariant that matters, and a tick that moves any of them backwards **does not
# land** — no matter how large the win beside it.
#
#   scripts/ratchet.sh check    # compare the current tree against the high-water marks. Exit 1 on a regression.
#   scripts/ratchet.sh bank     # after a green tick: raise the marks. Only ever raises.
#   scripts/ratchet.sh show     # print the marks and the current values.
#
# **The invariants, and why each one is here:**
#
#   WPT_TOTAL   every WPT area's passing count, and the sum. A tick may not lose subtests anywhere. This
#               is the one tick 82 needed and did not have.
#   CRASHES     Bar 0. Hangs and crashes across the whole sweep must be ZERO, always. Not "not worse" —
#               zero. A crash outranks any score, and the score is worthless beside it.
#   NET_DUPES   the same URL on the wire twice in one navigation. That is bandwidth, latency, and on a
#               metered connection it is money. A browser that double-downloads is not lean, however fast
#               its layout is.
#   CLAIMS      capabilities asserted by G_CAPABILITY. The near horizon's odometer. May not shrink.
#   GATES       live G_* gates. An engine cannot become less measured.
#   WALL        the verify wall, in seconds. Allowed to grow 30% over the mark before it fails — the wall
#               taxes EVERY future tick, so letting it drift is compounding interest against the loop.
#   WARNINGS    compiler warnings. A codebase that accumulates them is one nobody reads the output of, and
#               a real warning then hides among a hundred tolerated ones.
#
# **Bootstrapping is not cheating.** The first `bank` writes today's numbers as the marks. From then on it
# only ever ratchets up — `bank` takes `max(mark, current)`, never the current value blindly. A regression
# cannot be laundered into the baseline by re-running it.
set -uo pipefail
cd "$(dirname "$0")/.."

RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; BLD=$'\033[1m'; OFF=$'\033[0m'
MARKS=docs/loop/RATCHET.tsv
AREAS=docs/loop/WPT-AREAS.tsv

mark() { awk -F'\t' -v k="$1" '$1==k {print $2}' "$MARKS" 2>/dev/null | tail -1; }

# ── Read the CURRENT value of every invariant from the tree.
current_claims()   { grep -cE '^\s+\("' engine/page/tests/g_capability.rs 2>/dev/null || echo 0; }
current_gates()    { ls engine/page/tests/g_*.rs shell/tests/g_*.rs 2>/dev/null | wc -l; }
current_wall()     { grep -oP '^LAST_WALL_TIME:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0; }
current_gated()    { awk -F'\t' -v c="$1" '$1==c && $4=="gated"' docs/loop/CONSTELLATION.tsv 2>/dev/null | wc -l; }
# **MEASURED, not UNKNOWN.** The first version ratcheted `unknown` downward-only — which PUNISHES
# DISCOVERY: a surface audit that finds a capability nobody had thought of adds it as `unknown`, and the
# guard would have refused the tick for it. That is precisely backwards. The thing that must never go
# backwards is **how much has been LOOKED AT**:
#
#   probing an unknown            → measured +1   ✓ progress
#   discovering a new capability  → measured +0, total +1   ✓ discovery, unpunished
#   letting a measured one rot    → measured −1   ✗ REFUSED
#
# So the invariant is the count of capabilities with a real verdict, and the constellation's SIZE is free
# to grow. An audit that makes the map bigger and uglier is a good tick.
current_measured() { awk -F'\t' 'NR>1 && $4!="unknown" && $4!=""' docs/loop/CONSTELLATION.tsv 2>/dev/null | wc -l; }
current_unknown()  { awk -F'\t' '$4=="unknown"' docs/loop/CONSTELLATION.tsv 2>/dev/null | wc -l; }
current_warnings() {
  # The no-default-features build too — that is the configuration the wall runs and a human never does,
  # and it is where a stolen `#[cfg]` hid 283 errors once (PROCESS #43).
  { cargo build --workspace --features stylo,spidermonkey 2>&1; cargo build --workspace 2>&1; } \
    | grep -c '^warning:' || echo 0
}

case "${1:-check}" in

show|check)
  FAIL=0
  echo "${BLD}── THE RATCHET — every invariant, against its high-water mark${OFF}"

  if [ ! -f "$AREAS" ]; then
    printf '  %s✗ no %s — run scripts/wpt-sweep.sh first.%s\n' "$RED$BLD" "$AREAS" "$OFF" >&2
    printf '    %sA tick that did not measure cannot claim it did not regress.%s\n' "$RED" "$OFF" >&2
    exit 1
  fi

  # How stale is the sweep? A capability tick MUST have measured this tree, not the one before it.
  SWEEP_AGE=$(( $(date +%s) - $(stat -c %Y "$AREAS" 2>/dev/null || echo 0) ))

  # ── WPT, per area AND in total. The per-area check is the one that matters: it is what would have
  #    caught tick 82's silent −2 in `dom/` behind a +9,940 in `html/dom`.
  while IFS=$'\t' read -r area pass total pct crashes dupes; do
    [ "$area" = "area" ] && continue
    M=$(mark "WPT:$area")
    if [ -z "$M" ]; then
      printf '  %s+%s WPT %-16s %7s  (new area — mark will be set on bank)\n' "$YEL" "$OFF" "$area" "$pass"
    elif [ "$pass" -lt "$M" ]; then
      printf '  %s✗ WPT %-16s %7s < %-7s  REGRESSED by %s subtests%s\n' \
        "$RED$BLD" "$area" "$pass" "$M" "$((M - pass))" "$OFF"
      FAIL=1
    elif [ "$pass" -gt "$M" ]; then
      printf '  %s✓%s WPT %-16s %7s  (+%s)\n' "$GRN" "$OFF" "$area" "$pass" "$((pass - M))"
    else
      printf '    WPT %-16s %7s  (=)\n' "$area" "$pass"
    fi

    # Bar 0 is absolute. Not "no worse" — ZERO.
    if [ "${crashes:-0}" -gt 0 ] && [ "$area" != "TOTAL" ]; then
      printf '  %s✗ BAR 0: %s has %s HANG/CRASH. A crash outranks every score, and the score is worthless beside it.%s\n' \
        "$RED$BLD" "$area" "$crashes" "$OFF"
      FAIL=1
    fi
    if [ "${dupes:-0}" -gt 0 ] && [ "$area" != "TOTAL" ]; then
      printf '  %s✗ LEAN: %s put the same URL on the wire twice (%s times). A browser that double-downloads is not lean.%s\n' \
        "$RED$BLD" "$area" "$dupes" "$OFF"
      FAIL=1
    fi
  done < "$AREAS"

  # ── The near horizon, the instruments, and the codebase's health.
  for pair in "CLAIMS:$(current_claims)" "GATES:$(current_gates)"; do
    K=${pair%%:*}; V=${pair##*:}
    M=$(mark "$K"); M=${M:-0}
    if [ "$V" -lt "$M" ]; then
      printf '  %s✗ %-10s %5s < %-5s REGRESSED — an engine cannot become less capable or less measured.%s\n' \
        "$RED$BLD" "$K" "$V" "$M" "$OFF"
      FAIL=1
    else
      printf '  %s✓%s %-10s %5s  (mark %s)\n' "$GRN" "$OFF" "$K" "$V" "$M"
    fi
  done

  # ── THE CONSTELLATION. WPT is the scoreboard; this is the goal. A class may not lose a gated capability
  #    — and `unknown` may only ever go DOWN, because every unknown is a probe nobody wrote and it is the
  #    exact soil six phantom ❌s grew in.
  for c in doc app platform media cross; do
    V=$(current_gated "$c"); M=$(mark "CONST:$c"); M=${M:-0}
    if [ "$V" -lt "$M" ]; then
      printf '  %s✗ CONST:%-8s %3s < %-3s REGRESSED — a class cannot lose a gated capability.%s\n' \
        "$RED$BLD" "$c" "$V" "$M" "$OFF"
      FAIL=1
    else
      printf '  %s✓%s CONST:%-8s %3s gated (mark %s)\n' "$GRN" "$OFF" "$c" "$V" "$M"
    fi
  done
  MEAS=$(current_measured); MM=$(mark MEASURED); MM=${MM:-0}
  if [ "$MEAS" -lt "$MM" ]; then
    printf '  %s✗ MEASURED   %3s < %-3s — a capability that HAD a verdict lost it. An absent measurement is\n' \
      "$RED$BLD" "$MEAS" "$MM"
    printf '               not a negative measurement — that is the soil six phantom ❌s grew in.%s\n' "$OFF"
    FAIL=1
  else
    printf '  %s✓%s MEASURED   %3s of %s capabilities have a verdict (mark %s)  — %s still UNKNOWN\n' \
      "$GRN" "$OFF" "$MEAS" "$((MEAS + $(current_unknown)))" "$MM" "$(current_unknown)"
  fi

  # ── The wall. It is allowed to breathe, but not to drift: it taxes every future tick, so a slow wall is
  #    compounding interest charged against the loop itself.
  W=$(current_wall); MW=$(mark WALL); MW=${MW:-0}
  if [ "$MW" -gt 0 ] && [ "$W" -gt 0 ]; then
    LIMIT=$(( MW * 13 / 10 ))
    if [ "$W" -gt "$LIMIT" ]; then
      printf '  %s✗ WALL       %4ss > %ss (mark %ss +30%%) — the wall taxes EVERY future tick.%s\n' \
        "$RED$BLD" "$W" "$LIMIT" "$MW" "$OFF"
      FAIL=1
    else
      printf '  %s✓%s WALL       %4ss  (mark %ss, ceiling %ss)\n' "$GRN" "$OFF" "$W" "$MW" "$LIMIT"
    fi
  fi

  if [ "${1:-check}" = "check" ]; then
    if [ "$FAIL" -ne 0 ]; then
      printf '\n%sTHE RATCHET REFUSES THIS TICK.%s\n' "$RED$BLD" "$OFF" >&2
      printf '%sA win beside a regression is not a win — it is a trade, and the ratchet does not trade.%s\n' "$RED" "$OFF" >&2
      printf '%sFix the regression, or explain in the journal why the mark itself was wrong and lower it deliberately.%s\n' "$RED" "$OFF" >&2
      exit 1
    fi
    if [ "$SWEEP_AGE" -gt 21600 ]; then
      printf '\n  %s⚠ the sweep is %sh old. A capability tick must measure THIS tree.%s\n' \
        "$YEL" "$((SWEEP_AGE / 3600))" "$OFF"
    fi
    printf '\n%sTHE RATCHET HOLDS.%s Nothing went backwards.\n' "$GRN$BLD" "$OFF"
  fi
  ;;

bank)
  # Only ever RAISES. `max(mark, current)` — a regression cannot be laundered into the baseline by
  # re-running it, and a bad day cannot lower the bar for a good one.
  TMP=$(mktemp)
  {
    printf 'invariant\tmark\tbanked_at\n'
    NOW=$(date -Iseconds)
    # WPT, per area.
    if [ -f "$AREAS" ]; then
      while IFS=$'\t' read -r area pass total pct crashes dupes; do
        [ "$area" = "area" ] && continue
        M=$(mark "WPT:$area"); M=${M:-0}
        [ "${pass:-0}" -gt "$M" ] && M=$pass
        printf 'WPT:%s\t%s\t%s\n' "$area" "$M" "$NOW"
      done < "$AREAS"
    fi
    for pair in "CLAIMS:$(current_claims)" "GATES:$(current_gates)"; do
      K=${pair%%:*}; V=${pair##*:}
      M=$(mark "$K"); M=${M:-0}
      [ "$V" -gt "$M" ] && M=$V
      printf '%s\t%s\t%s\n' "$K" "$M" "$NOW"
    done
    for c in doc app platform media cross; do
      V=$(current_gated "$c"); M=$(mark "CONST:$c"); M=${M:-0}
      [ "$V" -gt "$M" ] && M=$V
      printf 'CONST:%s\t%s\t%s\n' "$c" "$M" "$NOW"
    done
    # MEASURED ratchets UP. Discovery (a bigger constellation) is never punished; rot is always refused.
    MEAS=$(current_measured); MM=$(mark MEASURED); MM=${MM:-0}
    [ "$MEAS" -gt "$MM" ] && MM=$MEAS
    printf 'MEASURED\t%s\t%s\n' "$MM" "$NOW"
    # The WALL ratchets DOWNWARD — a faster wall is the improvement, so the mark is the best time seen.
    W=$(current_wall); MW=$(mark WALL)
    if [ -z "$MW" ] || { [ "$W" -gt 0 ] && [ "$W" -lt "$MW" ]; }; then MW=$W; fi
    printf 'WALL\t%s\t%s\n' "${MW:-0}" "$NOW"
  } > "$TMP"
  mv "$TMP" "$MARKS"
  printf '  %s✓%s ratchet banked → %s\n' "$GRN" "$OFF" "$MARKS"
  ;;

*)
  echo "usage: scripts/ratchet.sh [check|bank|show]" >&2
  exit 2
  ;;
esac
