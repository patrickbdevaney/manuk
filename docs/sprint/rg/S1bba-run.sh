#!/usr/bin/env bash
# RG.S1.b.b.a: gocalendar-1 x3 under manuk at HEAD, live judge (REAL_JUDGE_* read from .env inside real_judge.py).
# Same harness as B-run.sh: real_run.py, one run per index, no TZ override (the TZ question is RG.S1.b.b.b).
# Usage: MANUK_CHROME_PIN=145.0.7632.116 bash docs/sprint/rg/S1bba-run.sh   (log: ~/.cache/manuk-rgb/driver-S1bba.log)
set -u
FAILED=0
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
OUT=docs/sprint/rg/S1bba-gocalendar1-manuk.tsv
LOG="$HOME/.cache/manuk-rgb/driver-S1bba.log"
TR="$HOME/.cache/manuk-rgb/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
for r in 0 1 2; do
  touch .git/sprint-working
  echo "$(date -Is) S1bba manuk gocalendar-1 run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/real_run.py run --arm manuk --runs 1 --run-base "$r" --judge --jobs 1 \
    --tasks gocalendar-1 --out "$OUT" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "S1bba run$((r+1)) rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
