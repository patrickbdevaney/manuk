#!/bin/bash
# CPU.3.a: quiet-host remeasure. Each stage waits until the 1-min load (load1) is < 2 before it starts, then runs:
# 10 real pages x {s,2,3,hot3} x 5 interleaved rounds (pages.py) and CONC engine-proc-full x {s,2,3,hot3} x 5 rounds.
# Every row records load1; validate.py rejects the whole set if any row has load1 >= 2 or any run failed.
cd "$(dirname "$0")/../../.."
OUT=docs/sprint/cpu3/quiet; mkdir -p "$OUT"
waitq(){ until awk '{exit !($1<2)}' /proc/loadavg; do touch .git/sprint-working; sleep 30; done; }
waitq; python3 docs/sprint/cpu3/pages.py "$OUT/pages.tsv" s,2,3,hot3 5 && echo PAGES_DONE
waitq; OUTDIR="$OUT" ROUNDS=5 bash docs/sprint/cpu3/conc.sh && echo CONC_DONE
