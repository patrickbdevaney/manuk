#!/usr/bin/env bash
# R3.1 gate: the same fixture under CPU stress and idle gives an identical a11y digest, 5/5, with
# --deterministic; the control arm (no flag) must DIFFER, otherwise the gate cannot go red.
# usage: scripts/r31-determinism-gate.sh [runs=5] [stress_procs=20]
set -u
cd "$(dirname "$0")/.."
touch .git/sprint-working
RUNS=${1:-5}; PROCS=${2:-20}
BIN=./target/release/a11y-dump
FIX=file://$PWD/tests/fixtures/determinism.html
digest() { "$BIN" "$@" "$FIX" 2>/dev/null | sha256sum | cut -c1-16; }
stress_on()  { for _ in $(seq "$PROCS"); do ( while :; do :; done ) & echo $! >> "$PIDS"; done; }
stress_off() { [ -f "$PIDS" ] && xargs kill < "$PIDS" 2>/dev/null; rm -f "$PIDS"; }
PIDS=$(mktemp); trap 'stress_off' EXIT
run_arm() { # $1 = flag ('' or --deterministic)
  local flag=$1 idle=() busy=()
  for _ in $(seq "$RUNS"); do idle+=("$(digest $flag)"); done
  stress_on
  for _ in $(seq "$RUNS"); do busy+=("$(digest $flag)"); done
  stress_off
  local u; u=$(printf '%s\n' "${idle[@]}" "${busy[@]}" | sort -u | wc -l)
  echo "arm=${flag:-wall-clock} idle=[${idle[*]}] stress=[${busy[*]}] distinct=$u"
  echo "$u" > "$PIDS.u"
}
run_arm --deterministic; DET=$(cat "$PIDS.u")
run_arm "";              CTL=$(cat "$PIDS.u")
rm -f "$PIDS.u"
if [ "$DET" = 1 ] && [ "$CTL" -gt 1 ]; then echo "PASS: deterministic 1 digest across $((RUNS*2)) runs; control $CTL"; exit 0; fi
echo "FAIL: deterministic distinct=$DET (want 1), control distinct=$CTL (want >1)"; exit 1
