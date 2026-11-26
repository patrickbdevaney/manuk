#!/bin/bash
# CONC.5 gate: CONC.2 data untouched, CONC.5 tables reproducible from its TSVs, binary sha named, doc carries both runs; red-able via mutations.
set -e; cd "$(dirname "$0")/../.."
D=docs/sprint/conc5; G=$D/layer1-grid.tsv; C=$D/capacity.tsv
git diff --quiet HEAD -- docs/sprint/conc2 || { echo "RED: conc2 files edited"; exit 1; }
head -1 $G | awk -F'\t' '{exit NF==26?0:1}' || { echo "RED: grid not 26 columns"; exit 1; }
[ "$(awk -F'\t' 'FNR>1 && $1!="145.0.7632.116"' $G $C $D/chrome-n50-uncaged.tsv | wc -l)" -eq 0 ] || { echo "RED: mixed chrome_ver"; exit 1; }
python3 -I bench/conc/analyze.py $G $C | diff -q - $D/analysis.md >/dev/null || { echo "RED: analysis.md not reproducible"; exit 1; }
python3 -I bench/conc/compare.py docs/sprint/conc2/layer1-grid.tsv $G | diff -q - $D/compare.md >/dev/null || { echo "RED: compare.md not reproducible"; exit 1; }
python3 -I bench/conc/cost_n50.py $G $D/chrome-n50-uncaged.tsv | diff -q - $D/cost-n50.md >/dev/null || { echo "RED: cost-n50.md not reproducible"; exit 1; }
grep -q 3005846f33b39ebfb73efeb4fbabcc196b6581eb2a3fa403dea5ff56b300bdc8 docs/sprint/CONCURRENCY.md || { echo "RED: binary sha not in doc"; exit 1; }
grep -q '^## CONC.5' docs/sprint/CONCURRENCY.md && grep -q '^## Verdicts, as pre-registered' docs/sprint/CONCURRENCY.md || { echo "RED: doc sections"; exit 1; }
# doc side-by-side block is exactly compare.md
awk '/^### Side by side \(CONC.2/{f=1;next} f' docs/sprint/CONCURRENCY.md | sed '1{/^$/d}' | diff -q - <(cat $D/compare.md) >/dev/null || { echo "RED: doc table != compare.md"; exit 1; }
grep -q 'Capacity win (engine-proc-full).*: PASS' $D/analysis.md || { echo "RED: no capacity verdict"; exit 1; }
# mutant: drop all engine-proc-full N=100 grid rows -> compare output must change
T=$(mktemp -d); awk -F'\t' '$3=="engine-proc-full" && $4==100 {next} {print}' $G > $T/g.tsv
python3 -I bench/conc/compare.py docs/sprint/conc2/layer1-grid.tsv $T/g.tsv | diff -q - $D/compare.md >/dev/null && { echo "RED: mutant not detected"; exit 1; } || true
echo GREEN
