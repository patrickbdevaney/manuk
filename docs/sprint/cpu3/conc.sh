#!/bin/bash
# CPU.3: CONC Layer-1 cell (engine-proc-full, N=1, all 8 traces) per opt-level variant, 3 interleaved rounds (rotated order).
cd "$(dirname "$0")/../../.."
V=(s 2 3 hot3)
for r in 0 1 2; do
  for k in 0 1 2 3; do v=${V[$(( (k+r)%4 ))]}
    echo "load1=$(cut -d' ' -f1 /proc/loadavg) variant=$v round=$r" >> docs/sprint/cpu3/conc.loads
    MANUK_MCP_BIN=/home/patrickd/cpu3bin/manuk-mcp-$v python3 -I bench/conc/harness.py run --arms engine-proc-full --ns 1 --runs 1 --out docs/sprint/cpu3/conc-$v.tsv --seed cpu3-$r
  done
done
