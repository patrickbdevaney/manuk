#!/bin/bash
# CONC.2 / CONC.2.a gate: published verdict text is reproducible from the grid + bisection TSVs, and changes when the data is doctored (red-able).
set -e; cd "$(dirname "$0")/../.."
G=docs/sprint/conc2/layer1-grid.tsv; C=docs/sprint/conc2/capacity.tsv
python3 -I bench/conc/analyze.py $G $C | diff -q - docs/sprint/conc2/analysis.md >/dev/null || { echo "RED: analysis.md not reproducible from grid+capacity"; exit 1; }
[ "$(grep -c . $G)" -eq 85 ] && [ "$(grep -c . $G.infra)" -eq 24 ] || { echo "RED: grid/infra row counts"; exit 1; }
[ "$(grep -c . $C)" -eq 54 ] && [ "$(grep -c . $C.infra)" -eq 13 ] || { echo "RED: capacity/infra row counts"; exit 1; }
[ "$(awk -F'\t' 'NR>1 && $22!="True"' $C | wc -l)" -eq 0 ] || { echo "RED: an uncaged capacity row"; exit 1; }
grep -q 'Capacity win (engine-proc-full).*: PASS' docs/sprint/conc2/analysis.md && ! grep -q UNRESOLVED docs/sprint/conc2/analysis.md || { echo "RED: capacity verdicts unresolved"; exit 1; }
# mutant 1: drop engine-proc-full N=59 rows -> its lower bound falls to 56 (56/39 < 1.5x) and the PASS must go
T=$(mktemp -d)
awk -F"\t" '$3=="engine-proc-full" && $4==59 {next} {print}' $C > $T/c.tsv; cp $C.infra $T/c.tsv.infra
python3 -I bench/conc/analyze.py $G $T/c.tsv | grep -q 'Capacity win (engine-proc-full).*: PASS' && { echo "RED: mutant 1 not detected"; exit 1; } || true
python3 -I bench/conc/analyze.py $G $T/c.tsv | diff -q - docs/sprint/conc2/analysis.md >/dev/null && { echo "RED: mutant 1 output unchanged"; exit 1; } || true
# mutant 2 (ladder): drop engine-fork N=50 grid rows -> its ladder bracket must change
awk -F"\t" '$3=="engine-fork" && $4==50 {next} {print}' $G > $T/g.tsv; cp $G.infra $T/g.tsv.infra
python3 -I bench/conc/analyze.py $T/g.tsv | grep -q 'Capacity win (engine-fork): capacity in \[25,49\]' || { echo "RED: mutant 2 not detected"; exit 1; }
echo "GREEN"
