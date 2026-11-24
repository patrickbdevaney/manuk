#!/bin/bash
# CONC.2 gate: the published verdict text is reproducible from the grid TSV, and turns different when the data is doctored (red-able).
set -e; cd "$(dirname "$0")/../.."
G=docs/sprint/conc2/layer1-grid.tsv
python3 -I bench/conc/analyze.py $G | diff -q - docs/sprint/conc2/analysis.md >/dev/null || { echo "RED: analysis.md not reproducible from grid"; exit 1; }
[ "$(grep -c . $G)" -eq 85 ] && [ "$(grep -c . $G.infra)" -eq 24 ] || { echo "RED: grid/infra row counts"; exit 1; }
# mutant: drop engine-fork N=50 rows (as if OOM-killed) -> its capacity bracket must change
T=$(mktemp -d); awk -F"\t" '$3=="engine-fork" && $4==50 {next} {print}' $G > $T/g.tsv; cp $G.infra $T/g.tsv.infra
python3 -I bench/conc/analyze.py $T/g.tsv | grep -q 'Capacity win (engine-fork): capacity in \[25,49\]' || { echo "RED: mutant not detected"; exit 1; }
echo "GREEN"
