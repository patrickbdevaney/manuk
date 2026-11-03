#!/usr/bin/env bash
# Runner for R0.4.a.c.a.b.a.a.a.a.a.a.b.a: serial release drive-probe re-drive of the M2 rows that ran at host load1 >= 4.
# One site at a time; each drive starts only when 1-min load < 4 (waits up to 20 min, then records as-is so the gate
# turns red on it). Debug rate and debug tag/chrome_ver are copied from the original sweep row, so the output has the
# same 8 columns as tri-rel-m2-join.sh:
#   url debug_rate release_rate debug_tag release_tag release_load1 debug_chrome_ver release_chrome_ver
#   usage: tri-rel-m2-redrive.sh <src_joined.tsv> <out.tsv> <url>...
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
. scripts/tri-classify.sh
SRC="$1"; OUT="$2"; shift 2
PIN="${MANUK_CHROME_PIN:?MANUK_CHROME_PIN not set}"
DP="${DP:-target/release/drive-probe}"
M2TMO="${M2TMO:-90}"
[ -x "$DP" ] || { echo "red: missing $DP" >&2; exit 1; }
printf 'url\tdebug_rate\trelease_rate\tdebug_tag\trelease_tag\trelease_load1\tdebug_chrome_ver\trelease_chrome_ver\n' > "$OUT"
for url in "$@"; do
  touch .git/sprint-working
  waited=0
  while :; do
    l=$(awk '{print $1}' /proc/loadavg)
    awk -v l="$l" 'BEGIN{exit !(l<4)}' && break
    [ "$waited" -ge 1200 ] && { echo "  load $l still >= 4 after 20 min; recording as-is" >&2; break; }
    sleep 10; waited=$((waited+10)); touch .git/sprint-working
  done
  row=$(awk -F'\t' -v u="$url" '$1==u' "$SRC")
  [ -n "$row" ] || { echo "red: $url not in $SRC" >&2; exit 1; }
  drate=$(printf '%s' "$row" | cut -f2); dtag=$(printf '%s' "$row" | cut -f4); dchr=$(printf '%s' "$row" | cut -f7)
  ERR=$(mktemp)
  o=$(timeout "$M2TMO" nice -n 15 "$DP" "$url" 2>"$ERR"); rc=$?
  tag=$(classify_tag "$rc" "$o" "$(cat "$ERR")")
  rate=0
  [ "$tag" = OK ] && rate=$(parse_m2 "$o" | cut -d' ' -f1)
  rm -f "$ERR"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$url" "$drate" "${rate:-0}" "$dtag" "$tag" "$l" "$dchr" "$PIN" >> "$OUT"
  printf '  REL %-38s rate=%-6s %s load1=%s\n' "${url:0:38}" "${rate:-0}" "$tag" "$l"
done
