#!/bin/bash
# CPU.3: CONC Layer-1 cell (engine-proc-full, N=1, all 8 traces) per opt-level variant, interleaved rounds (rotated order).
# ROUNDS defaults to 3 (first pass, host load 15-22). CPU.3.a runs ROUNDS=5 with OUTDIR=docs/sprint/cpu3/quiet.
cd "$(dirname "$0")/../../.."
OUTDIR=${OUTDIR:-docs/sprint/cpu3}; R=${ROUNDS:-3}; mkdir -p "$OUTDIR"
V=(s 2 3 hot3)
for r in $(seq 0 $((R-1))); do
  for k in 0 1 2 3; do v=${V[$(( (k+r)%4 ))]}
    echo "load1=$(cut -d' ' -f1 /proc/loadavg) variant=$v round=$r" >> "$OUTDIR/conc.loads"
    MANUK_MCP_BIN=/home/patrickd/cpu3bin/manuk-mcp-$v python3 -I bench/conc/harness.py run --arms engine-proc-full --ns 1 --runs 1 --out "$OUTDIR/conc-$v.tsv" --seed cpu3-$r
  done
done
