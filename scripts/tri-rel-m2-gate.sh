#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.b: release drive-probe M2 re-run over the reachable sites, debug rate beside it.
#   usage: tri-rel-m2-gate.sh <joined.tsv> <expected_rows> <chrome_ver>   (joined.tsv from tri-rel-m2-join.sh)
# Green only if: the M2 default in scripts/tri-sweep.sh is target/release/drive-probe; expected row count;
# 8 non-empty fields; every release_tag OK; debug_rate and release_rate numeric; release_load1 < 4 (load read
# before that site's drive); debug_chrome_ver and release_chrome_ver == pin. Red (rc=1) otherwise, reason on stderr.
set -uo pipefail
R=/home/patrickd/manuk
F="$1"; N="$2"; PIN="$3"
grep -qF 'DP=${DP:-$R/target/release/drive-probe}' "$R/scripts/tri-sweep.sh" || { echo "red: tri-sweep.sh DP default is not release" >&2; exit 1; }
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F")))
[ "$rows" -eq "$N" ] || { echo "red: rows=$rows, expected $N" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" 'NR>=1 {
  if (NF!=8) { print NR": fields="NF; next }
  for (i=1;i<=8;i++) if ($i=="") { print NR": empty field "i }
  if ($2 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": debug_rate "$2" not numeric";
  if ($3 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": release_rate "$3" not numeric";
  if ($5 != "OK") print NR": release tag "$5" (url "$1")";
  if ($6 !~ /^[0-9]+(\.[0-9]+)?$/ || $6+0 >= 4) print NR": release_load1 "$6" not < 4 (url "$1")";
  if ($7 != pin || $8 != pin) print NR": chrome_ver "$7"/"$8" != pin";
}' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
echo "green: rows=$rows release M2 OK at load1 < 4, debug and release on pin=$PIN"
