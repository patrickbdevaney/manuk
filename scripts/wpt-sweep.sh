#!/usr/bin/env bash
# ── THE SWEEP: measure EVERY WPT area, not the one you happen to be looking at.
#
# **Tick 82 exists because this did not.** Ten ticks were spent on `dom/` — 6,484 subtests, worked
# carefully and correctly. Sitting in the same checkout, never once measured, was `html/dom`: **59,818
# subtests**, nine times larger, whose failures were 80% a single mechanism. The loop optimised the room it
# was standing in and never opened the other doors.
#
# And the inverse hazard is just as real: tick 82 landed +9,940 subtests in `html/dom` and **quietly
# regressed `dom/` by 2**. A tick that improves what you are looking at and breaks what you are not is
# indistinguishable from progress, unless you measure both.
#
# So: every area, every time. The result is a machine-readable table (`docs/loop/WPT-AREAS.tsv`) that
# `ratchet.sh` compares against the high-water marks and `orient.sh` turns into the tick's work list.
#
#   scripts/wpt-sweep.sh            # all areas
#   scripts/wpt-sweep.sh --quick    # skip the slowest (html/dom) — for a non-capability tick only
set -uo pipefail
cd "$(dirname "$0")/.."

: "${WPT_DIR:=$HOME/wpt}"
export WPT_DIR
[ -d "$WPT_DIR/.git" ] || { echo "no WPT checkout at $WPT_DIR — run ./scripts/wpt-setup.sh" >&2; exit 1; }

OUT=docs/loop/WPT-AREAS.tsv
TMP=$(mktemp)

# Every area that is CHECKED OUT. An area that is not checked out scores zero and is invisible — which is
# its own kind of blindness, so `wpt-setup.sh` decides what exists and this measures all of it.
# The measured surface. **CSS breadth opened (tick 87)** — flexbox/grid/selectors were the only css
# subtrees ranked, so the loop was ranking inside a frame that excluded most of CSS. The subtrees below
# were already checked out and simply never measured; adding them turns unknown breadth into a ranked,
# ratchet-protected work-list (CONSTITUTION §VI.4 step 1). Order roughly by Pareto usage.
AREAS=(dom html/dom
       css/selectors css/css-flexbox css/css-grid css/css-sizing css/css-fonts
       css/css-text css/css-overflow css/css-transforms css/css-ui css/css-backgrounds
       css/css-values css/css-position css/css-display css/css-color
       cssom domparsing url encoding)
# **APERTURE, tick 103/104 (surface-audit #3):** css-values/position/display/color were invisible zeros —
# checked out and swept now, turning unknown breadth into a ranked, ratchet-protected work-list. NOT yet
# swept: **html/semantics** (checked out, 8,879 failing — the biggest single mass on the board) because it
# has **2 real per-page crashes** (crash even in isolation, not the recoverable ACCUM class) that are a
# NEW Bar-0 to fix before it can join the sweep without failing the ratchet. See docs/loop/SURFACE-AUDIT.md.
if [ "${1:-}" = "--quick" ]; then
  AREAS=(dom css/selectors css/css-flexbox css/css-grid cssom domparsing url)
fi

printf 'area\tpass\ttotal\tpct\tcrashes\tdupes\n' > "$TMP"
TOT_P=0; TOT_T=0; TOT_C=0; TOT_D=0

# **Per-area batch size — a memory bound, not a speed knob.** A single `encoding` file can create
# 190,000+ testharness subtests (Big5 decode across every variant), each a live JS object with its own
# reflector; forty such files in one child process outruns the GC and the child is OOM-killed mid-batch —
# which is not a wrong number, it is *no* number, and it took the terminal down before this was bounded.
# Fewer files per child caps the peak: a child that exits after a handful of files hands its whole heap
# back to the OS. Areas whose files are ordinary keep the fast default.
batch_for() {
  case "$1" in
    encoding|encoding/*) echo 4 ;;
    # Heavy layout areas retain a lot of per-file memory (full runtime + DOM + grid/flex layout tree), and
    # a 40-file process accumulates enough to be killed — a batch-SIZE artifact, not an engine crash: the
    # SAME pass count (css-grid 150) measures crash-free at a smaller batch. Right-size it, exactly as
    # encoding already is. (Confirmed on css-grid at tick 96; add siblings here if they surface the same.)
    css/css-grid|css/css-grid/*) echo 10 ;;
    *) echo 40 ;;
  esac
}

for a in "${AREAS[@]}"; do
  RAW=$(timeout 3000 cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt "$a" --batch "$(batch_for "$a")" 2>&1)
  LINE=$(echo "$RAW" | grep -oE 'subtests [0-9]+/[0-9]+' | tail -1)
  P=$(echo "$LINE" | grep -oE '[0-9]+/' | tr -d '/')
  T=$(echo "$LINE" | grep -oE '/[0-9]+' | tr -d '/')
  # Bar 0 and resource hygiene, per area. A crash outranks the score; a duplicate wire request is the
  # browser downloading the same bytes twice, which is bandwidth, latency, and on a metered link, money.
  C=$(echo "$RAW" | grep -oP 'HANG/CRASH \K[0-9]+' | tail -1)
  D=$(echo "$RAW" | grep -c 'DUPLICATE NETWORK REQUEST')
  P=${P:-0}; T=${T:-0}; C=${C:-0}; D=${D:-0}

  # ── A ZERO IS NOT A SCORE. It is the instrument failing to look.
  #
  # The very first run of this sweep produced 0/0 for every area and reported SUCCESS — the build had
  # died on a dangling ramdisk symlink and `grep` simply found no subtest line. Had `ratchet.sh bank` run
  # against that, it would have banked a high-water mark of **zero for everything**, and the guard whose
  # entire job is to refuse regressions would have been unable to detect any regression, forever.
  #
  # That is the third time this class of defect has appeared (PROCESS #46: the wall could not tell a
  # KILLED gate from a FAILING one; before that, WPT's own SHORT-vs-CRASH). **A lesson learned in one
  # instrument is not learned until it is applied to the others** — so it is applied here, at the moment
  # the instrument is born rather than after it lies to us:
  #
  #   an area with a KNOWN NON-ZERO test count that reports 0 total is an INSTRUMENT FAULT, and the sweep
  #   ABORTS rather than write a number that would be mistaken for a measurement.
  # How many test FILES did the harness even find? That is the difference between "the instrument broke"
  # and "there is nothing here", and they are NOT the same event:
  #
  #   * FILES > 0 but 0 subtests  -> the harness ran and produced NO VERDICT. An instrument fault. Abort,
  #     because writing it would bank a high-water mark of zero and make the ratchet vacuous forever.
  #   * FILES == 0                -> the area holds no runnable testharness files (cssom, for us). That is
  #     a fact about the corpus, not a failure. Record it as empty and move on.
  #
  # Conflating the two is the same defect as PROCESS #46 (the wall could not tell a KILLED gate from a
  # FAILING one), and as WPT's own SHORT-vs-CRASH before it. A lesson learned in one instrument is not
  # learned until it is applied to the others -- and this is the third instrument it has needed.
  NFILES=$(echo "$RAW" | grep -oE 'FILES +[0-9]+' | grep -oE '[0-9]+' | tail -1); NFILES=${NFILES:-0}
  if [ "$NFILES" -eq 0 ]; then
    printf '  %-16s %7s  (no runnable testharness files -- empty, not broken)\n' "$a" "-"
    printf '%s\t0\t0\t0.0\t0\t0\n' "$a" >> "$TMP"
    continue
  fi
  if [ "$T" -eq 0 ]; then
    printf '\n  INSTRUMENT FAULT: %s found %s files and produced NO VERDICT.\n' "$a" "$NFILES" >&2
    printf '    That is the harness failing to run, not a score. Writing it would bank a high-water\n' >&2
    printf '    mark of zero and make the ratchet vacuous forever.\n' >&2
    echo "$RAW" | tail -5 | sed 's/^/      /' >&2
    exit 1
  fi
  PCT=$(awk -v p="$P" -v t="$T" 'BEGIN{ printf (t>0 ? "%.1f" : "0.0"), (t>0 ? 100*p/t : 0) }')
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$a" "$P" "$T" "$PCT" "$C" "$D" >> "$TMP"
  printf '  %-16s %7s/%-7s %5s%%  crashes=%s dupes=%s\n' "$a" "$P" "$T" "$PCT" "$C" "$D"
  TOT_P=$((TOT_P + P)); TOT_T=$((TOT_T + T)); TOT_C=$((TOT_C + C)); TOT_D=$((TOT_D + D))
done

TOT_PCT=$(awk -v p="$TOT_P" -v t="$TOT_T" 'BEGIN{ printf (t>0 ? "%.2f" : "0.00"), (t>0 ? 100*p/t : 0) }')
printf 'TOTAL\t%s\t%s\t%s\t%s\t%s\n' "$TOT_P" "$TOT_T" "$TOT_PCT" "$TOT_C" "$TOT_D" >> "$TMP"
mv "$TMP" "$OUT"

printf '\n  TOTAL  %s/%s = %s%%   crashes=%s   duplicate wire requests=%s\n' \
  "$TOT_P" "$TOT_T" "$TOT_PCT" "$TOT_C" "$TOT_D"
[ "${1:-}" = "--quick" ] && printf '  (QUICK sweep — html/dom SKIPPED. Not valid for a capability tick.)\n'
printf '  → %s\n' "$OUT"
