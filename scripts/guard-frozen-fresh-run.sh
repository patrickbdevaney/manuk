#!/usr/bin/env bash
# GUARD.R0.a.b: four tri-sweeps on the SAME 41-site frozen list (docs/sprint/guard-frozen-corpus.tsv), each
# running the M1 fidelity pass fresh (NO --m1-from). A site that fails M1 in a run keeps its row with an empty
# cov/shape, which scripts/guard-frozen-check.py scores as 0, so cov and shape are measured every run instead of
# being copied from one M1 file. Binaries are the current build (the tri-sweep binaries in target/).
#
# GUARD.R0.a.b.a: the second argument is the M1 per-site limit in seconds (default 30 = the GUARD.R0.a.b setting);
# the run's persisted file is recorded in $LIST (one line per run: run index, persisted tsv path).
#
# usage: scripts/guard-frozen-fresh-run.sh [RUNS=4] [M1_TIMEOUT=30]
set -uo pipefail
R=$(cd "$(dirname "$0")/.." && pwd)
RUNS=${1:-4}
M1TMO=${2:-30}
CORPUS=$R/docs/sprint/guard-frozen-corpus.tsv
LIST=${GUARD_RUN_LIST:-/tmp/guard-fresh-runs.txt}
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:?}
[ "$(wc -l < "$CORPUS")" -eq 41 ] || { echo "FAIL: frozen corpus is not 41 sites: $CORPUS"; exit 1; }
: > "$LIST"
for i in $(seq 1 "$RUNS"); do
  touch "$R/.git/sprint-working"
  echo "=== GUARD.R0.a.b run $i/$RUNS start $(date '+%F %T') chrome_ver=$MANUK_CHROME_PIN m1-timeout=${M1TMO}s"
  log="/tmp/guard-fresh-run$i.log"
  "$R/scripts/tri-sweep.sh" --corpus "$CORPUS" --m1-timeout "$M1TMO" --out "/tmp/guard-fresh-run$i" > "$log" 2>&1
  pf=$(sed -n 's/^  persisted per-site rows → \([^ ]*\).*/\1/p' "$log" | head -1)
  echo "run$i	$pf" >> "$LIST"
  tail -n 12 "$log"
  echo "=== run $i done $(date '+%F %T') persisted=$pf"
done
