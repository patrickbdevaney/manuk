#!/usr/bin/env bash
# F1 cascade/mid spread under load (node W4.P0.c.a.a.a.a).
# Mirrors scripts/verify.sh's F1 computation exactly: same bench command and corpus, same field
# parse ($5 = large cascade, $9 = TOTAL of the mid and large rows), same ratio, and the 0.55 floor
# read from verify.sh itself (verify.sh is NOT edited by this node).
#
# usage: f1-spread.sh <label> <runs> [busy-workers]
#   busy-workers > 0 starts that many CPU-spinning `yes` processes for the duration (loaded condition);
#   they are killed on exit.
#   QUIET_WAIT_S (default 0) = wait up to this many seconds for 1-min load < QUIET_MAX_LOAD (default 2.0)
#   before the first run, so a "quiet" label is only given when the machine actually was quiet.
set -u
cd "$(dirname "$0")/../../.." || exit 1
LABEL=$1; RUNS=$2; WORKERS=${3:-0}
BIN=target/release/manuk-wpt
CORPUS=docs/bench/mid.html,docs/bench/large.html
OUT=docs/sprint/w4p0caaaa/F1-spread.tsv
CHROME_VER=${MANUK_CHROME_PIN:-145.0.7632.116}
FLOOR=$(grep -o 'fl "\$CRATIO" [0-9.]*' scripts/verify.sh | grep -o '[0-9.]*$')
[ -n "$FLOOR" ] || { echo "could not read F1 floor from verify.sh" >&2; exit 1; }
[ -x "$BIN" ] || { echo "missing $BIN (build manuk-wpt --release first)" >&2; exit 1; }
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT
touch .git/sprint-working

if [ "${QUIET_WAIT_S:-0}" -gt 0 ] && [ "$WORKERS" -eq 0 ]; then
  waited=0
  while :; do
    l1=$(cut -d' ' -f1 /proc/loadavg)
    awk -v a="$l1" -v b="${QUIET_MAX_LOAD:-2.0}" 'BEGIN{exit !(a+0 < b+0)}' && break
    [ "$waited" -ge "$QUIET_WAIT_S" ] && break
    sleep 15; waited=$((waited+15)); touch .git/sprint-working
  done
  echo "# quiet wait: waited ${waited}s, load1 now $(cut -d' ' -f1 /proc/loadavg)" >&2
fi

for ((w=0; w<WORKERS; w++)); do yes > /dev/null & PIDS+=($!); done
[ "$WORKERS" -gt 0 ] && sleep 3

for ((i=1; i<=RUNS; i++)); do
  touch .git/sprint-working
  read -r L1 L5 L15 RQ _ < /proc/loadavg
  TXT=$($BIN bench --pages "$CORPUS" --runs 5 2>/dev/null)
  C=$(echo "$TXT" | awk '/^large /{print $5; exit}')
  LT=$(echo "$TXT" | awk '/^large /{print $9; exit}')
  MT=$(echo "$TXT" | awk '/^mid /{print $9; exit}')
  CR=$(awk -v c="$C" -v m="$MT" 'BEGIN{ printf "%.2f", (m+0>0 ? c/m : 0) }')
  V=$(awk -v a="$CR" -v b="$FLOOR" 'BEGIN{ print (a+0 <= b+0) ? "PASS" : "EXCEEDS" }')
  printf '%s\t%d\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\tchrome_ver=%s\n' \
    "$LABEL" "$i" "$L1" "$L5" "$L15" "$RQ" "$C" "$MT" "$LT" "$CR" "$V" "$CHROME_VER" | tee -a "$OUT"
done
