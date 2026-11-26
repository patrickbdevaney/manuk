#!/usr/bin/env bash
# AXES.1.a gate: attrib.md is reproducible from the per-call trace, names the same tool the AXES node text names, and goes red on a doctored trace.
set -u; cd "$(dirname "$0")/../.."
C=docs/sprint/axes1a/calls.tsv; A=docs/sprint/axes1a/attrib.md; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT; fail=0
python3 -I bench/conc/attrib_p95.py $C | diff -q - $A >/dev/null || { echo "FAIL: attrib.md stale"; fail=1; }
# dominant tail op for engine arms must be nav and the node texts must say navigate
for n in 1 25; do
  sec=$(awk -v n="$n" '/^## engine-(fork|proc-full) N=/{s=($0 ~ " N="n":")} s' $A | grep -m1 -A8 '^| op ' | tail -n +3 | awk -F'|' '{split($6,a,"/"); if (a[1]+0>m){m=a[1]+0;op=$2}} END{print op}' | tr -d ' ')
  [ "$sec" = nav ] || { echo "FAIL: N=$n dominant tail op '$sec' != nav"; fail=1; }
  grep -P "^p95-n$n\tAXES.1.a\t.*navigate" docs/sprint/AXES-NODES.tsv >/dev/null || { echo "FAIL: node text p95-n$n does not name navigate"; fail=1; }
done
# mutant: make every manuk click 10 s -> the tail op must change and the output must differ
awk -F'\t' 'BEGIN{OFS="\t"} $2 ~ /^engine/ && $9=="click"{$10=10000} {print}' $C > $T/c.tsv
python3 -I bench/conc/attrib_p95.py $T/c.tsv | diff -q - $A >/dev/null && { echo "FAIL: doctored trace not detected"; fail=1; }
python3 -I bench/conc/attrib_p95.py $T/c.tsv | grep -A8 '^## engine-fork' | grep -E '^\| click' | grep -q '10000' || { echo "FAIL: mutant not visible"; fail=1; }
[ $fail = 0 ] && echo PASS; exit $fail
