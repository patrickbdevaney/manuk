#!/usr/bin/env bash
# ── Run the upstream WPT suite and record the result as a DATA POINT the cadence ledger can trust.
#
# The far horizon is 50,000 WPT tests. There is no way to know whether we are moving toward it without
# measuring, and no way to measure honestly without saying **when** each measurement was taken.
#
# So this writes `.git/manuk-wpt-last` (pass, total), which `scripts/tick-log.sh` picks up and marks as
# **fresh** in the cadence ledger. A tick that did not run WPT carries the previous figure forward and it
# is marked as carried — never as measured.
#
# That distinction is the whole point. A number copied forward through forty ticks draws a beautiful flat
# line that never happened, and this project's methodology is that a number you cannot defend is not a
# result.
#
#   usage: scripts/wpt-run.sh [subset]     (default: dom)
set -uo pipefail
cd "$(dirname "$0")/.."

SUBSET="${1:-dom}"
: "${WPT_DIR:=$HOME/wpt}"
export WPT_DIR

if [ ! -d "$WPT_DIR/.git" ]; then
  echo "no WPT checkout at $WPT_DIR — run ./scripts/wpt-setup.sh" >&2
  exit 1
fi

echo "── WPT: $SUBSET (release, spidermonkey)"
OUT=$(cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt "$SUBSET" 2>&1)
echo "$OUT" | tail -14

# `FILES 458   subtests 1736/6418  =  27.0%`
LINE=$(echo "$OUT" | grep -oE 'subtests [0-9]+/[0-9]+' | tail -1)
PASS=$(echo "$LINE" | grep -oE '[0-9]+/' | tr -d '/')
TOTAL=$(echo "$LINE" | grep -oE '/[0-9]+' | tr -d '/')

if [ -z "${PASS:-}" ] || [ -z "${TOTAL:-}" ]; then
  echo "✗ could not parse a subtest count from the run — recording NOTHING rather than a guess" >&2
  exit 1
fi

# Bar 0 outranks the score. A run with hangs or crashes is not a conformance measurement, it is an
# incident — and recording its percentage as progress would be recording a fire as a renovation.
if echo "$OUT" | grep -qE 'HANG/CRASH [1-9]'; then
  echo "⚠ Bar 0: this run HUNG or CRASHED. The score is recorded, and the crash outranks it." >&2
fi

printf '%s %s\n' "$PASS" "$TOTAL" > .git/manuk-wpt-last
printf '✓ WPT %s: %s/%s = %.1f%% — recorded as a FRESH data point for this tick\n' \
  "$SUBSET" "$PASS" "$TOTAL" "$(awk -v p="$PASS" -v t="$TOTAL" 'BEGIN{print 100*p/t}')"
