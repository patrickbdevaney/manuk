#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a: a serial re-drive TSV (one row per site, written by the serial runner).
#   usage: tri-serial-gate.sh <tsv> <expected_rows> <chrome_ver>
# Green only if: exactly <expected_rows> data rows; every row has 7 non-empty tab fields; load1_before is a number;
# chrome_ver equals the pin on every row. Red (rc=1) otherwise, with the reason on stderr.
set -uo pipefail
F="$1"; N="$2"; PIN="$3"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F") - 1))
[ "$rows" -eq "$N" ] || { echo "red: rows=$rows, expected $N" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" 'NR>1 {
  if (NF!=7) { print NR": fields="NF; next }
  for (i=1;i<=7;i++) if ($i=="") { print NR": empty field "i; next }
  if ($5 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": load1_before not numeric ("$5")";
  if ($7 != pin) print NR": chrome_ver "$7" != pin";
}' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
echo "green: rows=$rows bad_rows=0 pin=$PIN"
