#!/usr/bin/env bash
# AXES.1.b gate: lean/full N=5 p95 ratio computed from the per-call trace is <=1.5 in each of 3 runs, attrib.md is reproducible, and the check goes red on a doctored trace.
set -u; cd "$(dirname "$0")/../.."
C=docs/sprint/axes1b/calls.tsv; A=docs/sprint/axes1b/attrib.md; fail=0
ratios() { python3 -I - "$1" <<'P'
import csv,sys,collections
def pct(xs,q):
    xs=sorted(xs);return xs[min(len(xs)-1,int(round(q*(len(xs)-1))))]
d=collections.defaultdict(list)
for r in csv.reader(open(sys.argv[1]),delimiter='\t'):
    if r[2]=='5': d[(r[1],r[3])].append(float(r[9]))
for run in sorted({k[1] for k in d}):
    print(run, round(pct(d[('engine-proc-lean',run)],.95)/pct(d[('engine-proc-full',run)],.95),2))
P
}
[ "$(ratios $C | wc -l)" = 3 ] || { echo "FAIL: need 3 runs"; fail=1; }
ratios $C | awk '$2>1.5{bad=1} END{exit bad}' || { echo "FAIL: ratio > 1.5"; fail=1; }
python3 -I bench/conc/attrib_p95.py $C | diff -q - $A >/dev/null || { echo "FAIL: attrib.md stale"; fail=1; }
grep -qP '^p95-lean-n5\tAXES.1.b\t.*does not reproduce' docs/sprint/AXES-NODES.tsv || { echo "FAIL: reason missing"; fail=1; }
# mutant: lean arm nav calls 10x slower -> must go red
T=$(mktemp); trap 'rm -f $T' EXIT
awk -F'\t' 'BEGIN{OFS="\t"} $2=="engine-proc-lean"&&$9=="nav"{$10=$10*10} {print}' $C > $T
ratios $T | awk '$2>1.5{bad=1} END{exit bad}' && { echo "FAIL: mutant not detected"; fail=1; }
[ $fail = 0 ] && echo PASS; exit $fail
