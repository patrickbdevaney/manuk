#!/usr/bin/env bash
# AXES.1.c gate: check passes on the real tree; red on (a) a doctored manuk TSV (stale output), (b) a dropped run (sum mismatch/stale), (c) a stale output file.
set -u
cd "$(dirname "$0")/../.."
S=scripts/axes-pass102.py; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT; fail=0
python3 $S check >/dev/null || { echo "FAIL: real tree red"; fail=1; }
red() { if "$@" >/dev/null 2>&1; then echo "FAIL: expected red: $DESC"; fail=1; else echo "ok red: $DESC"; fi; }
MK=bench-queue/out/w6f1ab-manuk.tsv
awk -F'\t' 'BEGIN{OFS="\t"} NR>1&&$5=="udriver-7"&&$7==1{$9=1} {print}' $MK > $T/m1.tsv
DESC="doctored manuk pass (udriver-7 run1 flipped)"; P102_MK=$T/m1.tsv red python3 $S check
grep -v "	udriver-7	" $MK | head -n -1 > $T/m2.tsv
DESC="dropped rows"; P102_MK=$T/m2.tsv red python3 $S check
cp docs/sprint/axes1c/pass102.tsv $T/o.tsv; echo "x" >> $T/o.tsv; cp docs/sprint/axes1c/pass102.md $T/o.md
DESC="stale output"; P102_OUT=$T/o.tsv red python3 $S check
[ $fail = 0 ] && echo PASS
exit $fail
