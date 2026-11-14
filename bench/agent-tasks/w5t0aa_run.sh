#!/usr/bin/env bash
# W5.T0.a.a driver: the manuk arm on the 12 largest-thinking-gap tasks (W5.T0.a), 3 runs each, twice:
#   nocap = unchanged navigate observation; cap = MANUK_NAV_CAP_BYTES set to that task's Playwright observation size.
# Runs interleave arms within each run number so live-site drift hits both alike. Rows carry chrome_ver.
# Resume: run_arm.py does NOT skip finished rows; a crashed (arm, task, run) must be removed from its TSV first.
set -euo pipefail
ROOT=/home/patrickd/manuk
cd "$ROOT"
: "${MANUK_CHROME_PIN:?set MANUK_CHROME_PIN}"
export MANUK_MCP_BIN="$ROOT/target/release/manuk-mcp"
OUT=docs/sprint/w5
CACHE="$HOME/.cache/manuk-t0aa"
# task:cap (cap = round(obs_prev_pw) from docs/sprint/w5/T0a-thinking.tsv)
PAIRS="L12:1326 X05:2596 L03:1149 X06:1730 X02:4745 L07:21900 X01:5292 L09:846 L17:861 X03:5706 L13:5024 X08:1451"
for run in 1 2 3; do
  for pair in $PAIRS; do
    task=${pair%%:*}; cap=${pair##*:}
    for arm in nocap cap; do
      touch .git/sprint-working
      if [ "$arm" = cap ]; then export MANUK_NAV_CAP_BYTES="$cap"; else unset MANUK_NAV_CAP_BYTES; fi
      echo "== $arm $task run$run cap=${cap}" >&2
      python3 -I bench/agent-tasks/run_arm.py --arm manuk --suite full --runs 1 --run-base $((run - 1)) \
        --tasks "$task" --out "$OUT/T0a-a-$arm.tsv" --transcripts "$CACHE/$arm" --timeout 300
    done
  done
done
unset MANUK_NAV_CAP_BYTES
echo "DONE" >&2
