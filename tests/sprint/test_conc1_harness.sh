#!/usr/bin/env bash
# CONC.1 gate: harness self-test is green, and each of two mutants turns it red.
#   CONC1_MUTATE=share-proc  drops engine-proc's isolation (all agents share one process)
#   CONC1_MUTATE=skip-failed drops failed traces from the books
# Needs MANUK_CHROME_PIN and a built manuk-mcp (MANUK_MCP_BIN, else target/release/manuk-mcp).
set -u; cd "$(dirname "$0")/../.."
: "${MANUK_CHROME_PIN:?}"
H="python3 -I bench/conc/harness.py"; fail=0
run() { systemd-run --user --scope --quiet -p MemoryMax=8G -p MemorySwapMax=0 $H selftest --arm engine-proc-full --n 3 2>&1 | tail -1; }
out=$(run); echo "clean: $out"; case "$out" in "SELFTEST GREEN") ;; *) echo "FAIL: clean self-test not green"; fail=1;; esac
for m in share-proc skip-failed; do
  out=$(CONC1_MUTATE=$m run); echo "mutant $m: $out"
  case "$out" in "SELFTEST RED"*) ;; *) echo "FAIL: mutant $m did not turn the self-test red"; fail=1;; esac
done
[ $fail = 0 ] && echo "PASS conc1 harness gate" || exit 1
