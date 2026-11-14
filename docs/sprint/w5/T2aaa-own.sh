#!/usr/bin/env bash
# W5.T2.a.a.a: own suite x3 for both arms of the nav-cut, same binary, same day, same chrome_ver.
# base arm manuk (nav body kept) runs first, then manuk-navcut (MANUK_NAV_CUT=1).
# Compare: T4a-tokens.py BASE CUT
set -u
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-t2aaa/driver.log"
TR="$HOME/.cache/manuk-t2aaa/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
FAILED=0
for arm in manuk manuk-navcut; do
  touch .git/sprint-working
  tag=$([ "$arm" = manuk ] && echo off || echo cut)
  echo "$(date -Is) OWN $tag ($arm)" >> "$LOG"
  python3 -I bench/agent-tasks/run_arm.py --arm "$arm" --suite own --runs 3 \
    --out "$OUT/T2aaa-own-$tag.tsv" --transcripts "$TR/$tag" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "OWN $tag rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
