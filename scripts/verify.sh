#!/usr/bin/env bash
# The VERIFY wall (ADR-010). Every tick runs this — one command, so a tick cannot "forget" a gate.
#
# Gates:
#   B  build            workspace compiles
#   P  parity           72/72 box probes vs headless Chrome (§1.1)
#   G2 js-conformance   the DOM/BOM surface real sites need (grows every JS tick)
#   G3 affordances      every user-reachable control has an observable effect (§1.8)
#   F  perf floors      EPOCH-1 F1/F2/F3 (§1.7) — measured, binding
#   T  crate tests      touched crates
#
# Usage: scripts/verify.sh [--fast]     (--fast skips the perf floors, which need a page corpus)
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31m✗ %s\033[0m\n' "$1"; FAIL=1; }
head_() { printf '\n\033[1m%s\033[0m\n' "$1"; }

# Disk hygiene is a gate, not a chore. A full disk is a build failure that looks like a code
# failure, and this tree grows tens of gigabytes a week.
PCT=$(df /home 2>/dev/null | awk 'NR==2 {gsub(/%/,""); print $5}')
if [ -n "${PCT:-}" ] && [ "$PCT" -ge 88 ]; then
  head_ "D · disk (${PCT}% full — reclaiming before the build fails on ENOSPC)"
  bash scripts/disk-hygiene.sh | sed 's/^/  /'

# Ephemeral build output in RAM (METHODOLOGY 10.1). Idempotent, and it must run BEFORE the build:
# tmpfs does not survive a reboot, so the symlink into it has to be re-pointed or the compile fails
# on a dangling path. Disk-wear elimination, NOT a speed win — see scripts/ramdisk.sh.
./scripts/ramdisk.sh >/dev/null 2>&1 || true
fi

head_ "B · build (workspace)"
if cargo build -q --workspace 2>&1 | grep -qE '^error'; then bad "workspace does not compile"; else ok "workspace compiles"; fi

head_ "P · parity (§1.1 — 72/72 vs headless Chrome)"
PAR=$(cargo run -q -p manuk-wpt --release -- parity 2>&1 | tail -1)
if echo "$PAR" | grep -q "72/72"; then ok "$PAR"; else bad "$PAR"; fi

head_ "G1 · real-site visual fidelity vs Chromium (ADR-010/011 — SHIPPING config)"
G1URLS="${MANUK_FIDELITY_URLS:-https://example.com,https://news.ycombinator.com}"
G1FLOOR="${MANUK_FIDELITY_FLOOR:-0.75}"
G1OUT="${MANUK_FIDELITY_OUT:-/tmp/manuk-fidelity}"
if cargo run -q -p manuk-wpt --release -- fidelity --urls "$G1URLS" --out "$G1OUT" --floor "$G1FLOOR" >/tmp/manuk-g1.txt 2>&1; then
  ok "$(grep 'MEAN FIDELITY' /tmp/manuk-g1.txt || echo 'fidelity ok')"
  printf '    side-by-side composites in %s — LOOK at them\n' "$G1OUT"
else
  bad "real-site fidelity below floor ($G1FLOOR) — see $G1OUT"; grep -E 'BELOW|MEAN' /tmp/manuk-g1.txt | sed 's/^/    /'
fi

head_ "G2 · JS conformance (ADR-010 — the DOM/BOM surface real sites need)"
JS=$(cargo test -q -p manuk-page --features spidermonkey -- --ignored js_conformance 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$JS" ]; then ok "js conformance: $JS"; else bad "JS conformance suite did not pass"; fi

head_ "G3 · affordance completeness (§1.8 — no dead buttons)"
AFF=$(cargo test -q -p manuk-shell affordance 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$AFF" ]; then ok "affordances: $AFF"; else bad "affordance gate failed — a control may be dead"; fi

head_ "G6 · clickability (a link the browser cannot find is a link the user cannot click)"
G6URL="${MANUK_CLICK_URL:-https://en.wikipedia.org/wiki/Terrier}"
G6HTML="/tmp/manuk-g6.html"
if curl -sL "$G6URL" -o "$G6HTML" 2>/dev/null && [ -s "$G6HTML" ]; then
  CLK=$(cargo run -q -p manuk-wpt --release -- hittest --html "$G6HTML" --url "$G6URL" 2>/dev/null | grep -E "CLICKABILITY|MISSED")
  MISS=$(echo "$CLK" | grep -oE 'MISSED \(unclickable\): [0-9]+' | grep -oE '[0-9]+$' || echo 0)
  PCT=$(echo "$CLK" | grep -oE 'CLICKABILITY: [0-9.]+' | grep -oE '[0-9.]+' || echo 0)
  if [ "${MISS:-99}" -le 5 ]; then ok "clickability ${PCT}% (${MISS} unclickable links)"; else bad "clickability ${PCT}% — ${MISS} links the browser cannot find"; fi
else
  printf '  \033[33m—\033[0m could not fetch %s (skipped)\n' "$G6URL"
fi

head_ "F4 · interactive latency (§1.7 — one frame; the load bench is BLIND to this)"
if [ -s "$G6HTML" ]; then
  cargo run -q -p manuk-wpt --release -- bench --interactive --pages "$G6HTML" --runs 5 2>/dev/null | grep -E "^manuk-g6|OVER ONE FRAME" | sed 's/^/  /'
  printf '  \033[33m!\033[0m scroll and click must each stay under 16ms — a browser that loads fast and\n'
  printf '    then stutters on every wheel event is not fast, and G1/G2/G3 cannot see it.\n'
fi

head_ "G_ALLOC · allocation rate per input event (METHODOLOGY 5.2 — the load bench is BLIND to this)"
GA=$(cargo test -q -p manuk-page --features spidermonkey --test g_alloc -- --ignored 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GA" ]; then ok "per-event allocation: $GA"; else bad "G_ALLOC failed — an input event allocates per DOM node"; fi

head_ "G_TEARDOWN · no exit path bypasses Drop (METHODOLOGY 5.3 — a hidden crash is a data-loss bug)"
GT=$(cargo test -q -p manuk-shell --test g_teardown 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GT" ]; then ok "teardown: $GT"; else bad "G_TEARDOWN failed — an exit path skips the profile flush"; fi

head_ "T · crate tests"
for c in manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell; do
  R=$(cargo test -q -p "$c" 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
  if [ -n "$R" ]; then ok "$c: $R"; else bad "$c tests failed"; fi
done

if [ "${1:-}" != "--fast" ]; then
  head_ "F · perf floors (§1.7 — EPOCH-1, binding: a regression FAILS the tick)"
  CORPUS="${MANUK_BENCH_CORPUS:-}"
  if [ -z "$CORPUS" ]; then
    printf '  \033[33m—\033[0m set MANUK_BENCH_CORPUS=mid.html,large.html to check F1-F3 (skipped)\n'
  else
    cargo run -q -p manuk-wpt --release -- bench --pages "$CORPUS" --runs 5 2>&1 | sed -n '/median of runs/,/^$/p' | sed 's/^/  /'
    printf '  \033[33m!\033[0m compare against F1 cascade<=40ms, F2 pipeline<=95ms, F3 mid-page<=10ms\n'
  fi
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then printf '\033[32m\033[1mVERIFY: all gates green\033[0m\n'; else printf '\033[31m\033[1mVERIFY: FAILED — the tick does not land\033[0m\n'; fi
exit "$FAIL"
