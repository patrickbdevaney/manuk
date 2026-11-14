#!/usr/bin/env bash
# W5.T4.a.a.a.a: pre-registered SPA-route slice D01-D14 (bench/agent-tasks/suite-v1-diffview.json) x3, both arms of the
# diff-or-view rule, same binary/day/chrome_ver. rule-off (MANUK_DIFFVIEW_MUTATE=off) first, then rule-on.
set -u
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-t4aaaa/driver.log"
TR="$HOME/.cache/manuk-t4aaaa/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
FAILED=0
for arm in manuk-diffview-off manuk; do
  touch .git/sprint-working
  tag=$([ "$arm" = manuk ] && echo on || echo off)
  echo "$(date -Is) SPA $tag ($arm)" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$arm" --suite diffview --runs 3 \
    --out "$OUT/T4aaaa-spa-$tag.tsv" --transcripts "$TR/$tag" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "SPA $tag rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
