#!/usr/bin/env bash
# **The Framework Exception Miner** (METHODOLOGY Part 9; Tier 0 item 3).
#
# This does not check that the apps render correctly. It checks **what they throw**.
#
# The insight it runs on is one this project already paid for once: the browser was naming its own bugs
# out loud and we were throwing the message away. Printing the JS exception turned "a page <script>
# threw; continuing" into `TypeError: a.protocol is undefined` and `document.scrollingElement is
# undefined` — two missing IDL properties that were killing the sidebar on every mdbook site. Every
# exception a framework raises on boot is a piece of substrate we do not have, named precisely, by the
# framework, for free.
#
# The apps are REAL framework output (`tests/spa/build.sh`), not hand-written toys. A toy exercises the
# IDL we already thought to implement, which is a tautology. A real bundle exercises what React/Vue/
# Svelte actually call when they mount, which is the only thing that measures anything.
set -uo pipefail
cd "$(dirname "$0")/.."

OUT="${MANUK_SPA_OUT:-/tmp/manuk-spa}"
PORT=8910
mkdir -p "$OUT"; rm -f "$OUT"/*.log 2>/dev/null
BIN=target/release/manuk-wpt

echo "▶ Framework Exception Miner — what do real framework bundles THROW on boot?"
echo

printf '%-14s %-8s %-9s %s\n' "app" "mounted" "throws" "first exception"
printf '%-14s %-8s %-9s %s\n' "──────────────" "───────" "──────" "───────────────"

for app in tests/spa/apps/*/; do
  name=$(basename "$app")
  [ -f "$app/dist/index.html" ] || continue

  python3 -m http.server $PORT --directory "$app/dist" >/dev/null 2>&1 &
  srv=$!
  sleep 0.4

  # Everything the page threw, plus whether the framework actually MOUNTED anything. "No exceptions"
  # is worthless on its own: a bundle that throws nothing because it never ran is a silent failure,
  # which is the exact category Part 22.1 exists to refuse.
  RUST_LOG=warn timeout 60 "$BIN" boxes --fetch "http://127.0.0.1:$PORT/" --width 1200 \
    > "$OUT/$name.boxes" 2> "$OUT/$name.log"
  kill $srv 2>/dev/null; wait $srv 2>/dev/null

  # `grep -c` prints its count AND exits non-zero on no-match, so `|| echo 0` appends a SECOND zero.
  throws=$(grep -cE "script threw|module failed|JS error" "$OUT/$name.log" 2>/dev/null); throws=${throws:-0}
  boxes=$(wc -l < "$OUT/$name.boxes" 2>/dev/null); boxes=${boxes:-0}
  # Did the framework put anything INSIDE its mount point? That is the difference between "hydrated"
  # and "we served an empty div and nobody complained".
  mounted=$(grep -cE "^(root|app)\b" "$OUT/$name.boxes" 2>/dev/null); mounted=${mounted:-0}
  first=$(grep -oE "(TypeError|ReferenceError|SyntaxError|RangeError)[^\"]*" "$OUT/$name.log" 2>/dev/null | head -1 | cut -c1-64)

  if [ "$throws" -gt 0 ]; then col='\033[31m'; else col='\033[32m'; fi
  printf "%-14s %-8s ${col}%-9s\033[0m %-6s %s\n" "$name" \
    "$([ "$mounted" -gt 0 ] && echo yes || echo NO)" "$throws" "${boxes}box" "${first:-—}"
done

echo
echo "──── EVERY DISTINCT EXCEPTION — this is the substrate list, named by the frameworks ────"
echo
grep -hoE "(TypeError|ReferenceError|SyntaxError|RangeError)[^\"]*" "$OUT"/*.log 2>/dev/null \
  | sed 's/[0-9a-f]\{8,\}/<hash>/g' | sort | uniq -c | sort -rn | head -25
echo
echo "Each line is a missing piece of substrate, named by the framework itself. This is a bounded,"
echo "enumerated list — which is the entire point of Tier 0 item 3, and why leaving it unmeasured was"
echo "indefensible: the question 'is the app web additive work or a subsystem' is BINARY and cheap."
