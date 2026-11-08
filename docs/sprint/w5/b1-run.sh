#!/usr/bin/env bash
# W5.B1 driver: 3 arms x (REAL 102 judged tasks + own 40 + external 10) x 3 runs, Haiku, one chrome_ver.
# Order: run 1 of every arm, then run 2 of every arm, then run 3, so live-site drift hits all arms alike.
# Resume: real_run.py skips (arm, task, run) rows already in its TSV; run_arm.py does NOT, so a crashed
# run_arm.py (arm, run) must have its rows removed from the TSV before this script is re-invoked.
# Usage: MANUK_CHROME_PIN=... bash docs/sprint/w5/b1-run.sh      (logs: ~/.cache/manuk-b1/driver.log)
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-b1/driver.log"
TR="$HOME/.cache/manuk-b1/transcripts"
mkdir -p "$OUT" "$(dirname "$LOG")" "$TR"
ARMS_REAL="manuk cdp-layer playwright"
ARMS_FULL="manuk cdp-layer playwright"
for r in 0 1 2; do
  for arm in $ARMS_REAL; do
    touch .git/sprint-working
    echo "$(date -Is) REAL $arm run$((r+1))" >> "$LOG"
    # REAL judge: gpt-oss-120b (default); MANUK_REAL_JUDGE=claude switches to the W5.J1 Haiku fallback (REAL_JOBS=1)
    python3 -I bench/agent-tasks/real_run.py run --arm "$arm" --runs 1 --run-base "$r" --judge --jobs "${REAL_JOBS:-2}" \
      --out "$OUT/B1-real-$arm.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
    [ $rc -eq 0 ] || echo "REAL $arm run$((r+1)) rc=$rc" >> "$LOG"
  done
  for arm in $ARMS_FULL; do
    touch .git/sprint-working
    # run_arm.py does not skip finished runs: a run with its 54 rows already present is not re-run
    if [ "$(awk -F'\t' -v r=$((r+1)) 'NR>1 && $8==r' "$OUT/B1-full-$arm.tsv" 2>/dev/null | wc -l)" -ge 54 ]; then continue; fi
    echo "$(date -Is) FULL $arm run$((r+1))" >> "$LOG"
    python3 -I bench/agent-tasks/run_arm.py --arm "$arm" --suite full --runs 1 --run-base "$r" \
      --out "$OUT/B1-full-$arm.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
    [ $rc -eq 0 ] || echo "FULL $arm run$((r+1)) rc=$rc" >> "$LOG"
  done
done
echo "$(date -Is) DONE" >> "$LOG"
