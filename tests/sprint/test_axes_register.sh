#!/usr/bin/env bash
# AXES.1 gate: check passes on the real tree, goes red on (a) a losing axis with no node, (b) a dead node id,
# (c) a doctored grid (stale AXES.md), (d) a too-short reason.
set -u
cd "$(dirname "$0")/../.."
S=scripts/axes-register.py; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT; fail=0
python3 $S check >/dev/null || { echo "FAIL: real tree red"; fail=1; }
red() { if "$@" >/dev/null 2>&1; then echo "FAIL: expected red: $DESC"; fail=1; else echo "ok red: $DESC"; fi; }
grep -v '^cpu-n1' docs/sprint/AXES-NODES.tsv > $T/n1.tsv
DESC="losing axis cpu-n1 without node"; AXES_NODES=$T/n1.tsv red python3 $S check
sed 's/^disk\tAXES.1.d/disk\tNOPE.9/' docs/sprint/AXES-NODES.tsv > $T/n2.tsv
DESC="dead node id"; AXES_NODES=$T/n2.tsv red python3 $S check
sed 's/^disk\tAXES.1.d\t.*/disk\t-\ttoo short/' docs/sprint/AXES-NODES.tsv > $T/n3.tsv
DESC="short reason"; AXES_NODES=$T/n3.tsv red python3 $S check
awk -F'\t' 'BEGIN{OFS="\t"} NR>1&&$3=="pw-shared"{$14=$14*9} {print}' docs/sprint/conc2/layer1-grid.tsv > $T/g.tsv
DESC="doctored grid"; AXES_GRID=$T/g.tsv red python3 $S check
# (e) a node that is DONE must not own an axis: close AXES.1.d in a copy of the graph
sed 's/^\(AXES.1.d\t[^\t]*\t[^\t]*\t[^\t]*\t[^\t]*\t\)TODO/\1DONE/' docs/sprint/GRAPH.tsv > $T/g2.tsv
DESC="owning node closed (DONE)"; AXES_GRAPH=$T/g2.tsv red python3 $S check
[ $fail = 0 ] && echo PASS
exit $fail
