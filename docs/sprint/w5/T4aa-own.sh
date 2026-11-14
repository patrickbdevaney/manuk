#!/usr/bin/env bash
# W5.T4.a.a: own suite x3 for both arms of the diff-or-view rule, same binary, same day, same chrome_ver.
# rule-off (MANUK_DIFFVIEW_MUTATE=off) runs first, then rule-on; the comparison is T4aa-tokens.py OFF ON.
# Usage: MANUK_CHROME_PIN=... bash docs/sprint/w5/T4aa-own.sh   (log: ~/.cache/manuk-t4aa/driver.log)
set -u
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-t4aa/driver.log"
TR="$HOME/.cache/manuk-t4aa/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
FAILED=0
for arm in manuk-diffview-off manuk; do
  touch .git/sprint-working
  tag=$([ "$arm" = manuk ] && echo on || echo off)
  echo "$(date -Is) OWN $tag ($arm)" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$arm" --suite own --runs 3 \
    --out "$OUT/T4aa-own-$tag.tsv" --transcripts "$TR/$tag" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "OWN $tag rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
