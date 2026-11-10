#!/usr/bin/env bash
# RG.B REAL re-bench at HEAD: manuk x3 (102 judged tasks) + playwright x1 as the drift control.
# Same harness as W5.B1 (docs/sprint/w5/b1-run.sh): real_run.py, judge gpt-oss-120b (REAL_JUDGE_* in .env),
# one chrome_ver. Resume: real_run.py skips (arm, task, run) rows already in its TSV.
# Usage: MANUK_CHROME_PIN=145.0.7632.116 bash docs/sprint/rg/B-run.sh   (log: ~/.cache/manuk-rgb/driver.log)
set -u
FAILED=0
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
OUT=docs/sprint/rg
LOG="$HOME/.cache/manuk-rgb/driver.log"
TR="$HOME/.cache/manuk-rgb/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
# (arm run-index) pairs; playwright runs once, after manuk run 1, as the drift control
SCHEDULE="manuk:0 playwright:0 manuk:1 manuk:2"
for item in $SCHEDULE; do
  arm="${item%%:*}"; r="${item##*:}"
  touch .git/sprint-working
  echo "$(date -Is) REAL $arm run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/real_run.py run --arm "$arm" --runs 1 --run-base "$r" --judge --jobs "${REAL_JOBS:-2}" \
    --out "$OUT/B-real-$arm.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "REAL $arm run$((r+1)) rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
