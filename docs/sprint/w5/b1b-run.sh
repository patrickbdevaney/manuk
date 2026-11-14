#!/usr/bin/env bash
# W5.B1.a.a driver: the manuk+preload arm (ENABLE_TOOL_SEARCH=false, the manuk binary) on the W5.B1 full slice
# (own 40 + external 14), 3 runs, Haiku, one chrome_ver. Writes B1-manuk-preload-full.tsv (162 rows).
# Like-for-like with playwright+preload (B1-preload-full.tsv): same setting, same slice, same driver.
# Resume: run_arm.py does NOT skip finished runs, so a run whose 54 rows are already present is skipped here,
# and a crashed partial run must have its rows removed from the TSV first.
# Usage: MANUK_CHROME_PIN=... bash docs/sprint/w5/b1b-run.sh      (logs: ~/.cache/manuk-b1/driver-manuk-preload.log)
set -u
FAILED=0
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
OUT=docs/sprint/w5
ARM="manuk+preload"
LOG="$HOME/.cache/manuk-b1/driver-manuk-preload.log"
TR="$HOME/.cache/manuk-b1/transcripts"
mkdir -p "$OUT" "$(dirname "$LOG")" "$TR"
# heartbeat for the watchdog while the (hours-long) driver runs
( while kill -0 $$ 2>/dev/null; do touch "$ROOT/.git/sprint-working"; sleep 60; done ) &
for r in 0 1 2; do
  touch .git/sprint-working
  if [ "$(awk -F'\t' -v r=$((r+1)) 'NR>1 && $8==r' "$OUT/B1-manuk-preload-full.tsv" 2>/dev/null | wc -l)" -ge 54 ]; then continue; fi
  echo "$(date -Is) FULL $ARM run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$ARM" --suite full --runs 1 --run-base "$r" \
    --out "$OUT/B1-manuk-preload-full.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "FULL $ARM run$((r+1)) rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
