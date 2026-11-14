#!/usr/bin/env bash
# W5.T4.a.a.a: external slice X01-X14 (pre-registered in suite-v1-external.json, unchanged) x3 for both arms of the
# diff-or-view rule, same binary, same day, same chrome_ver. rule-off (MANUK_DIFFVIEW_MUTATE=off) runs first, then rule-on.
# Compare: T4a-tokens.py OFF ON. Fallback hits: grep -l 'full view instead of a diff' over the transcripts.
set -u
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-t4aaa/driver.log"
TR="$HOME/.cache/manuk-t4aaa/transcripts"
TASKS=X01,X02,X03,X04,X05,X06,X07,X08,X09,X10,X11,X12,X13,X14
mkdir -p "$(dirname "$LOG")" "$TR"
FAILED=0
for arm in manuk-diffview-off manuk; do
  touch .git/sprint-working
  tag=$([ "$arm" = manuk ] && echo on || echo off)
  echo "$(date -Is) EXT $tag ($arm)" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$arm" --suite full --tasks "$TASKS" --runs 3 \
    --out "$OUT/T4aaa-ext-$tag.tsv" --transcripts "$TR/$tag" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "EXT $tag rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
