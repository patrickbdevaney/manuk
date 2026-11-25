#!/bin/bash
# CONC.2.b gate: N=50 Chrome rows (3 runs/arm, caged=False, cage size recorded) and a reproducible, red-able cost verdict.
set -e; cd "$(dirname "$0")/../.."
G=docs/sprint/conc2/layer1-grid.tsv; C=docs/sprint/conc2/chrome-n50-uncaged.tsv
for a in chrome-mcp pw-proc pw-shared; do [ "$(awk -F'\t' -v a=$a 'NR>1&&$3==a&&$4==50&&$22=="False"&&$23>8' $C | wc -l)" -eq 3 ] || { echo "RED: $a needs 3 uncaged N=50 runs with cage_gb"; exit 1; }; done
python3 -I bench/conc/cost_n50.py $G $C | diff -q - docs/sprint/conc2/cost-n50.md >/dev/null || { echo "RED: cost-n50.md not reproducible"; exit 1; }
grep -q 'Cost win (engine-fork).*PASS' docs/sprint/conc2/cost-n50.md && grep -q 'Cost win (registered, N=50): PASS' docs/sprint/CONCURRENCY.md || { echo "RED: verdict missing"; exit 1; }
T=$(mktemp -d); awk -F'\t' 'BEGIN{OFS="\t"} NR>1&&$3=="pw-shared"{$19=$19*0.5} {print}' $C > $T/c.tsv
python3 -I bench/conc/cost_n50.py $G $T/c.tsv | grep -q 'Cost win (engine-fork).*FAIL' || { echo "RED: mutant not detected"; exit 1; }
echo GREEN
