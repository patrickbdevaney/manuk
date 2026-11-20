#!/usr/bin/env bash
# gate for W6.L1.a.a.a: scripts/ledger-first-turn.py goes green only when live stream-json rows show pack's turn-1
# prompt is lower than full's, and goes red on a live row without first_turn_input, a pack-not-lower result, or too few
# rows. Fixtures are written through sprint-ledger.py (the real writer), stream-json style and json style.
set -u; cd "$(dirname "$0")/../.."; fail=0; T=$(mktemp -d); CMP=scripts/ledger-first-turn.py
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:-145.0.7632.116}

# srow <ledger> <node> <mode> <turn1-cache-read> : one stream-json attempt (assistant turn-1 usage + result event)
srow() {
  printf '%s\n' \
    "{\"type\":\"system\",\"subtype\":\"init\"}" \
    "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":3,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":$4,\"output_tokens\":1}}}" \
    "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":5,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":$((4 * $4)),\"output_tokens\":1}}}" \
    "{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"num_turns\":2,\"result\":\"r\",\"session_id\":\"s\",\"total_cost_usd\":0.1,\"usage\":{\"input_tokens\":8,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":$((5 * $4)),\"output_tokens\":2}}" \
    >"$T/s.jsonl"
  python3 scripts/sprint-ledger.py "$T/s.jsonl" "$2" haiku 0 0 "$3" "$1" >/dev/null
}
# jrow <ledger> <node> <mode> : one json-era attempt (single result object, no assistant events): no first_turn columns
jrow() {
  echo "{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"num_turns\":1,\"result\":\"r\",\"session_id\":\"s\",\"total_cost_usd\":0.1,\"usage\":{\"input_tokens\":1,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":1000,\"output_tokens\":1}}" >"$T/j.json"
  python3 scripts/sprint-ledger.py "$T/j.json" "$2" haiku 0 0 "$3" "$1" >/dev/null
}
expect() { # expect <name> <want-rc> <ledger>
  python3 $CMP "$3" --since "2000-01-01 00:00:00" >"$T/out" 2>&1; local rc=$?
  if [ "$rc" = "$2" ]; then echo "ok   $1 -> rc=$rc"; else echo "FAIL $1: want rc=$2 got rc=$rc"; cat "$T/out"; fail=1; fi
}
# case <name> <pack-cache-read> : 3 full attempts (turn-1 prompt 1000) then 3 pack attempts
case_run() {
  local d=$T/$1; mkdir -p "$d"; local L=$d/LEDGER.tsv
  for i in 1 2 3; do srow "$L" "F$i" full 1000; done
  for i in 1 2 3; do srow "$L" "P$i" pack "$2"; done
  echo "$L"
}
expect pass 0 "$(case_run pass 400)"
expect pack_first_turn_higher_red 1 "$(case_run pack_higher 1500)"
expect pack_equal_red 1 "$(case_run pack_equal 1000)"

# a live row whose attempt wrote no turn-1 usage (json era) is the falsifier: red even when the means would pass
L=$(case_run json_row 400); jrow "$L" J1 pack
expect live_row_lacks_first_turn_red 1 "$L"

# only 2 pack rows with first_turn_input: incomplete, never a pass
d=$T/short; mkdir -p "$d"; L=$d/LEDGER.tsv
for i in 1 2 3; do srow "$L" "F$i" full 1000; done
for i in 1 2; do srow "$L" "P$i" pack 400; done
expect short_pack_incomplete 2 "$L"

# no ledger at all: incomplete
expect no_ledger 2 "$T/none/LEDGER.tsv"

[ "$fail" = 0 ] && echo "PASS test_ledger_first_turn" || { echo "FAILED test_ledger_first_turn"; exit 1; }
