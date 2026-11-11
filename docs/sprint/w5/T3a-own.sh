#!/usr/bin/env bash
# W5.T3.a: own suite x3 for the manuk arm, before vs after W5.T3 (same chrome_ver, same day, same harness).
# Usage: MANUK_CHROME_PIN=... bash docs/sprint/w5/T3a-own.sh   (log: ~/.cache/manuk-t3a/driver.log)
set -u
: "${MANUK_CHROME_PIN:?refusing: MANUK_CHROME_PIN unset}"
: "${T3A_BEFORE:?set T3A_BEFORE to the pre-W5.T3 manuk-mcp binary}"
: "${T3A_AFTER:?set T3A_AFTER to the post-W5.T3 manuk-mcp binary}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1
OUT=docs/sprint/w5
LOG="$HOME/.cache/manuk-t3a/driver.log"
TR="$HOME/.cache/manuk-t3a/transcripts"
mkdir -p "$(dirname "$LOG")" "$TR"
FAILED=0
for arm in before after; do
  touch .git/sprint-working
  bin_var="T3A_$(echo "$arm" | tr a-z A-Z)"
  echo "$(date -Is) OWN $arm" >> "$LOG"
  MANUK_MCP_BIN="${!bin_var}" python3 -I bench/agent-tasks/run_arm.py --arm manuk --suite own --runs 3 \
    --out "$OUT/T3a-own-$arm.tsv" --transcripts "$TR" >> "$LOG" 2>&1; rc=$?
  [ $rc -eq 0 ] || { echo "OWN $arm rc=$rc" >> "$LOG"; FAILED=1; }
done
echo "$(date -Is) DONE failed=$FAILED" >> "$LOG"
exit "$FAILED"
