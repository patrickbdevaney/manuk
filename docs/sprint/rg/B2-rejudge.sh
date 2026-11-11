#!/usr/bin/env bash
# RG.2.B one-judge-per-task fix: re-run only the (arm, task, run) rows the Haiku fallback judged (kept in
# B2-judge-fallback-dropped-<arm>.tsv). real_run.py skips rows already present, so only the missing ones run.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"; cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
LOG="$HOME/.cache/manuk-rgb2/driver.log"; TR="$HOME/.cache/manuk-rgb2/transcripts"
# manuk run N is --run-base N-1; the fallback keys are listed per run
declare -A M=(
  [0]="opendining-2,topwork-7,udriver-5"
  [1]="dashdish-11,gocalendar-2,opendining-7,topwork-7,udriver-5"
  [2]="dashdish-11,gocalendar-6,networkin-4,staynb-7,udriver-5"
)
FAILED=0
for r in 0 1 2; do
  touch .git/sprint-working; echo "$(date -Is) REJUDGE manuk run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/real_run.py run --arm manuk --runs 1 --run-base "$r" --judge --jobs 2 --tasks "${M[$r]}" \
    --out docs/sprint/rg/B2-real-manuk.tsv --transcripts "$TR" >> "$LOG" 2>&1 || FAILED=1
done
touch .git/sprint-working; echo "$(date -Is) REJUDGE playwright run1" >> "$LOG"
python3 -I bench/agent-tasks/real_run.py run --arm playwright --runs 1 --run-base 0 --judge --jobs 2 --tasks "dashdish-11,gocalendar-10,gocalendar-9,topwork-9" \
  --out docs/sprint/rg/B2-real-playwright.tsv --transcripts "$TR" >> "$LOG" 2>&1 || FAILED=1
echo "$(date -Is) REJUDGE DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
