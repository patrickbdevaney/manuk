#!/usr/bin/env bash
# GUARD.R0.a.b: four tri-sweeps on the SAME 41-site frozen list (docs/sprint/guard-frozen-corpus.tsv), each
# running the M1 fidelity pass fresh (NO --m1-from). A site that fails M1 in a run keeps its row with an empty
# cov/shape, which scripts/guard-frozen-check.py scores as 0, so cov and shape are measured every run instead of
# being copied from one M1 file. Binaries are the current build (the tri-sweep binaries in target/).
#
# usage: scripts/guard-frozen-fresh-run.sh [RUNS=4]
set -uo pipefail
R=$(cd "$(dirname "$0")/.." && pwd)
RUNS=${1:-4}
CORPUS=$R/docs/sprint/guard-frozen-corpus.tsv
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:?}
[ "$(wc -l < "$CORPUS")" -eq 41 ] || { echo "FAIL: frozen corpus is not 41 sites: $CORPUS"; exit 1; }
for i in $(seq 1 "$RUNS"); do
  touch "$R/.git/sprint-working"
  echo "=== GUARD.R0.a.b run $i/$RUNS start $(date '+%F %T') chrome_ver=$MANUK_CHROME_PIN"
  "$R/scripts/tri-sweep.sh" --corpus "$CORPUS" --out "/tmp/guard-fresh-run$i" 2>&1 | tail -n 12
  echo "=== run $i done $(date '+%F %T')"
done
