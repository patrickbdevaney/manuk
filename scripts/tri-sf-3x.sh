#!/usr/bin/env bash
# Runner for R0.4.a.c.a.b.a.a.a.a.a.a: salesforce.com drive-probe 3x per binary (debug, release), interleaved
# D R D R D R, one run at a time, each started only when 1-min load < 4 (waits up to 20 min, then records as-is
# so the gate turns red on it). Writes a TSV with columns:
#   url binary rc tag wall_s load1_before rate chrome_ver
#   usage: tri-sf-3x.sh <out.tsv>
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
. scripts/tri-classify.sh
OUT="$1"
URL="https://salesforce.com/"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
printf 'url\tbinary\trc\ttag\twall_s\tload1_before\trate\tchrome_ver\n' > "$OUT"
ERR=$(mktemp)
for b in debug release debug release debug release; do
  touch .git/sprint-working
  DP="target/$b/drive-probe"
  waited=0
  while :; do
    l=$(awk '{print $1}' /proc/loadavg)
    awk -v l="$l" 'BEGIN{exit !(l<4)}' && break
    [ "$waited" -ge 1200 ] && break
    sleep 10; waited=$((waited+10)); touch .git/sprint-working
  done
  t0=$(date +%s.%N)
  o=$(timeout 180 nice -n 15 "$DP" "$URL" 2>"$ERR"); rc=$?
  t1=$(date +%s.%N)
  tag=$(classify_tag "$rc" "$o" "$(cat "$ERR")")
  rate=NA
  [ "$tag" = OK ] && rate=$(parse_m2 "$o" | awk '{print $1}')
  wall=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$URL" "$b" "$rc" "$tag" "$wall" "$l" "${rate:-NA}" "$PIN" >> "$OUT"
  echo "$b rc=$rc $tag ${wall}s load=$l rate=${rate:-NA}"
done
rm -f "$ERR"
