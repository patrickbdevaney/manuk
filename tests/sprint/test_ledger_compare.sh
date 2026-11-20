#!/usr/bin/env bash
# gate for W6.L1.a: scripts/ledger-compare.py goes green only when pack is cheaper AND lands at least as often,
# and goes red (or incomplete) on broken input. Fixtures are written through sprint-ledger.py (the real writer).
set -u; cd "$(dirname "$0")/../.."; fail=0; T=$(mktemp -d); CMP=scripts/ledger-compare.py
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:-145.0.7632.116}

# row <ledger> <node> <mode> <input_tokens> : one attempt, written by the real ledger writer
row() {
  echo "{\"type\":\"result\",\"is_error\":false,\"duration_ms\":5,\"num_turns\":1,\"result\":\"r\",\"session_id\":\"s\",\"total_cost_usd\":0.1,\"usage\":{\"input_tokens\":$4,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0,\"output_tokens\":1}}" >"$T/j.json"
  python3 scripts/sprint-ledger.py "$T/j.json" "$2" haiku 0 0 "$3" "$1" >/dev/null
}
# done <log> <node> : the loop's own success line, written right after the ledger row
done_line() { echo "$(date '+%F %T')  ✓ node $2 DONE on haiku (rc=0)" >>"$1"; }
expect() { # expect <name> <want-rc> <ledger> <log>
  python3 $CMP "$3" "$4" >"$T/out" 2>&1; local rc=$?
  if [ "$rc" = "$2" ]; then echo "ok   $1 -> rc=$rc"; else echo "FAIL $1: want rc=$2 got rc=$rc"; cat "$T/out"; fail=1; fi
}
# case <name> <pack-tokens> <pack-DONE-count>: 5 full attempts (1000 tokens, all DONE) then 5 pack attempts
case_run() {
  local d=$T/$1; mkdir -p "$d"; local L=$d/LEDGER.tsv G=$d/loop.log
  for i in 1 2 3 4 5; do row "$L" "F$i" full 1000; done
  for i in 1 2 3 4 5; do row "$L" "P$i" pack "$2"; done
  for i in 1 2 3 4 5; do done_line "$G" "F$i"; done
  for i in $(seq 1 "$3"); do done_line "$G" "P$i"; done
  echo "$d $L $G"
}
read -r d L G <<<"$(case_run pass 400 5)";            expect pass 0 "$L" "$G"
read -r d L G <<<"$(case_run pack_higher 1500 5)";    expect pack_higher_tokens_red 1 "$L" "$G"
read -r d L G <<<"$(case_run pack_lands_less 400 3)"; expect pack_fewer_done_red 1 "$L" "$G"

# 4 of 5 pack attempts recorded: incomplete, never a pass
d=$T/short; mkdir -p "$d"; L=$d/LEDGER.tsv; G=$d/loop.log
for i in 1 2 3 4 5; do row "$L" "F$i" full 1000; done
for i in 1 2 3 4; do row "$L" "P$i" pack 400; done
for i in 1 2 3 4 5; do done_line "$G" "F$i"; done; for i in 1 2 3 4; do done_line "$G" "P$i"; done
expect short_pack_incomplete 2 "$L" "$G"

# an attempt whose result could not be parsed has no usage numbers: incomplete, never a pass
d=$T/nousage; mkdir -p "$d"; L=$d/LEDGER.tsv; G=$d/loop.log
for i in 1 2 3 4 5; do row "$L" "F$i" full 1000; done
for i in 1 2 3 4; do row "$L" "P$i" pack 400; done
echo notjson >"$T/bad.json"; python3 scripts/sprint-ledger.py "$T/bad.json" P5 haiku 0 1 pack "$L" >/dev/null
for i in 1 2 3 4 5; do done_line "$G" "F$i"; done; for i in 1 2 3 4 5; do done_line "$G" "P$i"; done
expect unparseable_row_incomplete 2 "$L" "$G"

expect no_ledger_incomplete 2 "$T/none/LEDGER.tsv" "$T/none/loop.log"

# control: a copy of the script with the DONE check removed must pass the lands-less case, which shows that
# check is the one that turns that case red
sed 's/lands = pd \/ pn >= fd \/ fn/lands = True/' $CMP >"$T/mut.py"
python3 "$T/mut.py" "$T/pack_lands_less/LEDGER.tsv" "$T/pack_lands_less/loop.log" >/dev/null 2>&1; rc=$?
if [ "$rc" = 0 ]; then echo "ok   control: mutant without the DONE check passes pack_lands_less"; else echo "FAIL control: mutant rc=$rc"; fail=1; fi

rm -rf "$T"
[ $fail = 0 ] && echo "ledger-compare tests: PASS" || { echo "ledger-compare tests: FAIL"; exit 1; }
