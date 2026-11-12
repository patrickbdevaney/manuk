#!/usr/bin/env bash
# W5.B1.a driver: the playwright+preload arm (ENABLE_TOOL_SEARCH=false) on the same two slices as W5.B1,
# 3 runs, Haiku, one chrome_ver. Writes B1-preload-real.tsv (102 judged tasks) and B1-preload-full.tsv (54 tasks).
# Resume: real_run.py skips (arm, task, run) rows already in its TSV; run_arm.py does NOT, so a run whose
# 54 rows are already present is skipped here and a crashed partial run must have its rows removed first.
# Judge: the W5.J1 Haiku fallback (MANUK_REAL_JUDGE=claude), one judge for every row, as in W5.B1.
# Usage: MANUK_CHROME_PIN=... bash docs/sprint/w5/b1a-run.sh      (logs: ~/.cache/manuk-b1/driver-preload.log)
set -u
FAILED=0
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
OUT=docs/sprint/w5
ARM="playwright+preload"
LOG="$HOME/.cache/manuk-b1/driver-preload.log"
TR="$HOME/.cache/manuk-b1/transcripts"
mkdir -p "$OUT" "$(dirname "$LOG")" "$TR"
export MANUK_REAL_JUDGE=claude REAL_JOBS="${REAL_JOBS:-1}"
# heartbeat for the watchdog while the (hours-long) driver runs
( while kill -0 $$ 2>/dev/null; do touch "$ROOT/.git/sprint-working"; sleep 60; done ) &
for r in 0 1 2; do
  touch .git/sprint-working
  echo "$(date -Is) REAL $ARM run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/real_run.py run --arm "$ARM" --runs 1 --run-base "$r" --judge --jobs "$REAL_JOBS" \
    --out "$OUT/B1-preload-real.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "REAL $ARM run$((r+1)) rc=$rc" >> "$LOG"; FAILED=1; }
  touch .git/sprint-working
  if [ "$(awk -F'\t' -v r=$((r+1)) 'NR>1 && $8==r' "$OUT/B1-preload-full.tsv" 2>/dev/null | wc -l)" -ge 54 ]; then continue; fi
  echo "$(date -Is) FULL $ARM run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$ARM" --suite full --runs 1 --run-base "$r" \
    --out "$OUT/B1-preload-full.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "FULL $ARM run$((r+1)) rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
