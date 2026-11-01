#!/usr/bin/env bash
# W4.P0.d gate: on live quotes.toscrape.com/scroll, scrolling manuk-mcp to the end must load the 2nd
# quote batch (11th quote, "This life is what you make it", in extract; the author name is in batch 1 so is no test) in 3/3 runs. SCROLL_DY=0 is the mutation
# (a scroll that never moves, so no scroll trigger fires) and must go red. Needs target/release/manuk-mcp.
set -u
cd "$(dirname "$0")/.."
dy="${SCROLL_DY:-100000}"; want="${EXPECT:-This life is what you make it}"; ok=0
for r in 1 2 3; do
  touch .git/sprint-working 2>/dev/null
  python3 -I scripts/w4p0d-scroll.py https://quotes.toscrape.com/scroll "$dy" "$want" | tail -1 | grep -q PASS && ok=$((ok+1))
done
echo "scroll dy=$dy: $ok/3 runs show '$want'"
[ "$ok" = 3 ] && echo PASS || { echo FAIL; exit 1; }
