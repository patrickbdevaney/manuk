#!/usr/bin/env bash
# RG.B one-judge-per-task fix: re-run only the (arm, task, run) rows the Haiku fallback judged (kept in
# B-judge-fallback-dropped-<arm>.tsv). real_run.py skips rows already present, so only the missing ones run.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"; cd "$ROOT" || exit 1
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
LOG="$HOME/.cache/manuk-rgb/driver.log"; TR="$HOME/.cache/manuk-rgb/transcripts"
M="dashdish-11,fly-unified-12,gocalendar-10,networkin-2,opendining-2,opendining-7,staynb-7,topwork-7,udriver-5"
P="fly-unified-10,gocalendar-10,gocalendar-9,udriver-5"
FAILED=0
for r in 0 1 2; do
  touch .git/sprint-working; echo "$(date -Is) REJUDGE manuk run$((r+1))" >> "$LOG"
  python3 -I bench/agent-tasks/real_run.py run --arm manuk --runs 1 --run-base "$r" --judge --jobs 2 --tasks "$M" \
    --out docs/sprint/rg/B-real-manuk.tsv --transcripts "$TR" >> "$LOG" 2>&1 || FAILED=1
done
touch .git/sprint-working; echo "$(date -Is) REJUDGE playwright run1" >> "$LOG"
python3 -I bench/agent-tasks/real_run.py run --arm playwright --runs 1 --run-base 0 --judge --jobs 2 --tasks "$P" \
  --out docs/sprint/rg/B-real-playwright.tsv --transcripts "$TR" >> "$LOG" 2>&1 || FAILED=1
echo "$(date -Is) REJUDGE DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
