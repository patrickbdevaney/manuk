#!/usr/bin/env bash
# W4.P0.c.a.a.a.a.a.a — sample /proc/loadavg every INTERVAL seconds for DURATION seconds.
# Usage: sample-load.sh <out.tsv> [duration_s=86700] [interval_s=300]
# Default: 290 samples at t=0,300,...,86700 s (spans 24 h 05 min). Rows are appended, header written once.
# Each row carries chrome_ver=$MANUK_CHROME_PIN (pinned in docs/sprint/SUCCESS.md; default below).
set -u
OUT=${1:?usage: sample-load.sh <out.tsv> [duration_s] [interval_s]}
DUR=${2:-86700}
INT=${3:-300}
PIN=${MANUK_CHROME_PIN:-145.0.7632.116}
if [ ! -s "$OUT" ]; then
  printf 'utc\tepoch\tload1\tload5\tload15\tchrome_ver\n' > "$OUT"
fi
start=$(date +%s)
k=0
while :; do
  now=$(date +%s)
  read -r l1 l5 l15 _ < /proc/loadavg
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$now" "$l1" "$l5" "$l15" "$PIN" >> "$OUT"
  if [ $((now - start)) -ge "$DUR" ]; then
    break
  fi
  k=$((k + 1))
  wait=$((start + k * INT - $(date +%s)))
  if [ "$wait" -gt 0 ]; then
    sleep "$wait"
  fi
done
