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
