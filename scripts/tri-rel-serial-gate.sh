#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a: serial release drive-probe re-drive TSV (url rc tag wall_s load1_before rate chrome_ver).
#   usage: tri-rel-serial-gate.sh <tsv> <expected_rows> <chrome_ver>
# Green only if: expected row count; 7 non-empty fields; every row rc=0 and tag=OK; load1_before < 4;
# chrome_ver == pin; rate is numeric. Red (rc=1) otherwise, reason on stderr.
set -uo pipefail
F="$1"; N="$2"; PIN="$3"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F") - 1))
[ "$rows" -eq "$N" ] || { echo "red: rows=$rows, expected $N" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" 'NR>1 {
  if (NF!=7) { print NR": fields="NF; next }
  for (i=1;i<=7;i++) if ($i=="") { print NR": empty field "i }
  if ($2!=0 || $3!="OK") print NR": not OK (rc="$2" tag="$3")";
  if ($5 !~ /^[0-9]+(\.[0-9]+)?$/ || $5+0 >= 4) print NR": load1_before "$5" not < 4";
  if ($6 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": rate "$6" not numeric";
  if ($7 != pin) print NR": chrome_ver "$7" != pin";
}' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
echo "green: rows=$rows all OK at load1_before < 4, pin=$PIN"
