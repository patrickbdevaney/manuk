#!/usr/bin/env bash
# GUARD.R0.a: four tri-sweeps on ONE frozen site list (docs/sprint/guard-frozen-m1.tsv, the 41-site set F),
# every run importing the same M1 reachability rows (--m1-from), so the self-agreement check tests the
# M2 / a11y instrument on fixed sites rather than the live web's reachability.
# Binaries are the PRE-sprint engine (c28d82bd, built in a worktree); the caller sets BIN_ROOT to that worktree.
#
# usage: BIN_ROOT=/home/patrickd/manuk-guard-pre scripts/guard-frozen-run.sh [RUNS=4]
set -uo pipefail
R=/home/patrickd/manuk
BIN_ROOT=${BIN_ROOT:?set BIN_ROOT to the c28d82bd worktree}
RUNS=${1:-4}
M1=$R/docs/sprint/guard-frozen-m1.tsv
export MW=$BIN_ROOT/target/release/manuk-wpt DP=$BIN_ROOT/target/debug/drive-probe AX=$BIN_ROOT/target/debug/a11y-score
export MANUK_CHROME_PIN=${MANUK_CHROME_PIN:?}
for i in $(seq 1 "$RUNS"); do
  touch "$R/.git/sprint-working"
  echo "=== GUARD.R0.a run $i/$RUNS start $(date '+%F %T') chrome_ver=$MANUK_CHROME_PIN"
  "$R/scripts/tri-sweep.sh" --m1-from "$M1" --out "/tmp/guard-frozen-run$i" 2>&1 | tail -n 8
  echo "=== run $i done $(date '+%F %T')"
done
