#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a: debug-vs-release A/B TSV (url binary rc tag wall_s load1_before rate chrome_ver).
#   usage: tri-ab-gate.sh <tsv> <expected_rows> <chrome_ver>
# Green only if: expected row count; 8 non-empty fields; load1_before < 4; chrome_ver == pin; and at least one url
# has a different tag between its two binaries. Red (rc=1) otherwise.
set -uo pipefail
F="$1"; N="$2"; PIN="$3"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F") - 1))
[ "$rows" -eq "$N" ] || { echo "red: rows=$rows, expected $N" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" 'NR>1 {
  if (NF!=8) { print NR": fields="NF; next }
  for (i=1;i<=8;i++) if ($i=="") { print NR": empty field "i }
  if ($6 !~ /^[0-9]+(\.[0-9]+)?$/ || $6+0 >= 4) print NR": load1_before "$6" not < 4";
  if ($8 != pin) print NR": chrome_ver "$8" != pin";
}' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
diff=$(awk -F'\t' 'NR>1 { if ($1 in t && t[$1]!=$4) d++; t[$1]=$4 } END{print d+0}' "$F")
[ "$diff" -ge 1 ] || { echo "red: no site differs between binaries" >&2; exit 1; }
echo "green: rows=$rows sites_differing=$diff pin=$PIN"
