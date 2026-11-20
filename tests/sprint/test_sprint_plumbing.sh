#!/usr/bin/env bash
# gate for W6.L1: ledger row from a json result, pack <6k tokens for EVERY node, loop wiring present.
set -u; cd "$(dirname "$0")/../.."; fail=0; T=$(mktemp -d)
echo '{"type":"result","is_error":false,"duration_ms":5,"num_turns":3,"result":"r","session_id":"s","total_cost_usd":0.5,"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":7}}' >$T/j.json
python3 scripts/sprint-ledger.py $T/j.json N sonnet 1 0 pack $T/L.tsv >/dev/null
[ "$(tail -1 $T/L.tsv | cut -f10)" = 1110 ] || { echo "FAIL ledger input_total"; fail=1; }
[ "$(head -1 $T/L.tsv | awk -F'\t' '{print NF}')" = "$(tail -1 $T/L.tsv | awk -F'\t' '{print NF}')" ] || { echo "FAIL ledger cols"; fail=1; }
echo notjson >$T/bad; python3 scripts/sprint-ledger.py $T/bad N sonnet 1 1 full $T/L.tsv >/dev/null
[ "$(wc -l <$T/L.tsv)" = 3 ] || { echo "FAIL: unparseable output must still get a row"; fail=1; }
over=0; max=0
for id in $(cut -f1 docs/sprint/GRAPH.tsv | tail -n +2); do
  n=$(python3 scripts/sprint-pack.py "$id" 2>/dev/null | wc -c) || true
  python3 scripts/sprint-pack.py "$id" >/dev/null 2>&1 || over=$((over+1)); [ "$n" -gt "$max" ] && max=$n
done
[ $over = 0 ] || { echo "FAIL: $over packs over 6k tokens"; fail=1; }
echo "max pack: $max chars (~$((max/4)) tokens)"
grep -q 'sprint-pack.py' scripts/sprint-loop.sh && grep -q 'sprint-ledger.py' scripts/sprint-loop.sh && grep -q 'counted as an attempt' scripts/sprint-loop.sh || { echo "FAIL: loop not wired"; fail=1; }
bash -n scripts/sprint-loop.sh || fail=1
rm -rf $T; [ $fail = 0 ] && echo "sprint-plumbing tests: PASS" || { echo "sprint-plumbing tests: FAIL"; exit 1; }
