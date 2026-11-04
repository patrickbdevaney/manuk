#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a.a.a.b.a.a.b: 3 debug salesforce.com rows (tri-sf-debug900.sh output).
#   usage: tri-sf-debug900-gate.sh <tsv> <chrome_ver>
# Green only if: exactly 3 rows; 8 non-empty fields; every row binary=debug, rc=0, tag=OK, rate numeric,
# chrome_ver == pin. Red (rc=1) otherwise, reason on stderr.
set -uo pipefail
F="$1"; PIN="$2"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F") - 1))
[ "$rows" -eq 3 ] || { echo "red: rows=$rows, expected 3" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" 'NR>1 {
  if (NF!=8) { print NR": fields="NF; next }
  for (i=1;i<=8;i++) if ($i=="") { print NR": empty field "i }
  if ($2!="debug") print NR": binary "$2" not debug";
  if ($3!=0 || $4!="OK") print NR": not OK (rc="$3" tag="$4")";
  if ($7 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": rate "$7" not numeric";
  if ($8 != pin) print NR": chrome_ver "$8" != pin";
}' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
echo "green: rows=3, all debug OK with numeric rate, pin=$PIN"
