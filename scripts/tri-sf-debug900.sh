#!/usr/bin/env bash
# Runner for R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a.a.b: salesforce.com drive-probe, DEBUG binary only, 3 runs,
# each capped at 900 s (the 3x debug runs in R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a.a all hit the 180 s budget).
# Same classification and row format as tri-sf-3x.sh; the 180 s cap is raised, nothing else changes.
#   usage: tri-sf-debug900.sh <out.tsv>
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
. scripts/tri-classify.sh
OUT="$1"
URL="https://salesforce.com/"
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
CAP=900
printf 'url\tbinary\trc\ttag\twall_s\tload1_before\trate\tchrome_ver\n' > "$OUT"
for n in 1 2 3; do
  b=debug; ERR="${OUT%.tsv}.$n.$b.err"
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
  o=$(timeout "$CAP" nice -n 15 "$DP" "$URL" 2>"$ERR"); rc=$?
  t1=$(date +%s.%N)
  printf '%s\n' "$o" > "${OUT%.tsv}.$n.$b.out"
  tag=$(classify_tag "$rc" "$o" "$(cat "$ERR")")
  rate=NA
  [ "$tag" = OK ] && rate=$(parse_m2 "$o" | awk '{print $1}')
  wall=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$URL" "$b" "$rc" "$tag" "$wall" "$l" "${rate:-NA}" "$PIN" >> "$OUT"
  echo "$b rc=$rc $tag ${wall}s load=$l rate=${rate:-NA}"
done
