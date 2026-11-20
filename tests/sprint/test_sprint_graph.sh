#!/usr/bin/env bash
# gate for W6.L1: sprint-graph.py rejects malformed rows (file untouched) and accepts a good one.
set -u
cd "$(dirname "$0")/../.."
T=$(mktemp -d); export GRAPH=$T/G.tsv
cp docs/sprint/GRAPH.tsv "$GRAPH"; before=$(md5sum <"$GRAPH"); fail=0
G="python3 scripts/sprint-graph.py"
ok()  { "$@" >/dev/null 2>&1 || { echo "FAIL (expected accept): $*"; fail=1; }; }
bad() { "$@" >/dev/null 2>&1 && { echo "FAIL (expected reject): $*"; fail=1; }; [ "$(md5sum <"$GRAPH")" = "$before" ] || { echo "FAIL: file changed by rejected edit"; fail=1; }; }
bad $G add T.1 x 1 sonnet - TODO t g n            # non-integer ring
bad $G add T.1 1 1 sonnet - TODO t g              # 8 fields
bad $G add T.1 1 1 opus - TODO t g n              # opus floor
bad $G add T.1 1 1 sonnet - WAT t g n             # bad status
bad $G add T.1 1 1 sonnet NOPE.NODE TODO t g n    # unknown dep
bad $G add W6.L1 1 1 sonnet - TODO t g n          # duplicate id
bad $G add "T.1" 1 1 sonnet - TODO "t	x" g n      # tab in field
bad $G set W6.L1 status WAT
bad $G set NOPE status DONE
ok  $G add T.1 1 1 sonnet W6.L1 TODO t g n
[ "$(tail -1 "$GRAPH" | awk -F'\t' '{print NF}')" = 9 ] || { echo "FAIL: appended row not 9 cols"; fail=1; }
ok  $G set T.1 status DONE
grep -q '^T.1	.*	DONE	' "$GRAPH" || { echo "FAIL: set did not apply"; fail=1; }
rm -rf "$T"; [ $fail = 0 ] && echo "sprint-graph tests: PASS" || { echo "sprint-graph tests: FAIL"; exit 1; }
