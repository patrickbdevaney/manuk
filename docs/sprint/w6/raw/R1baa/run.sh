#!/bin/bash
# usage: run.sh MODE ; starts the proxy, 3 agf-probe runs + one dom-eval dump, stops the proxy
cd /home/patrickd/manuk
python3 bench/agent-tasks/r1baa_proxy.py 8791 "$1" >/dev/null 2>&1 &
P=$!; sleep 2
for i in 1 2 3; do echo "$1 agf run$i: $(./target/release/agf-probe http://127.0.0.1:8791/ 2>/dev/null | wc -l) target lines"; done
echo "$1 dump: $(./target/release/dom-eval http://127.0.0.1:8791/ 3 docs/sprint/w6/raw/R1baa/dump.js 2>&1 | tail -c 500)"
kill $P
