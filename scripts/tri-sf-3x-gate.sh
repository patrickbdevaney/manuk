#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a.a.a.a: salesforce.com drive-probe, 3 runs per binary (url binary rc tag wall_s load1_before rate chrome_ver).
#   usage: tri-sf-3x-gate.sh <tsv> <expected_rows> <chrome_ver>
# Green only if: expected row count; 8 non-empty fields; exactly expected/2 rows per binary (debug, release);
# every row rc=0 and tag=OK; load1_before < 4; rate numeric; chrome_ver == pin. Red (rc=1) otherwise, reason on stderr.
set -uo pipefail
F="$1"; N="$2"; PIN="$3"
[ -f "$F" ] || { echo "red: missing $F" >&2; exit 1; }
rows=$(($(wc -l < "$F") - 1))
[ "$rows" -eq "$N" ] || { echo "red: rows=$rows, expected $N" >&2; exit 1; }
bad=$(awk -F'\t' -v pin="$PIN" -v half="$((N/2))" 'NR>1 {
  if (NF!=8) { print NR": fields="NF; next }
  for (i=1;i<=8;i++) if ($i=="") { print NR": empty field "i }
  cnt[$2]++;
  if ($2!="debug" && $2!="release") print NR": binary "$2" unknown";
  if ($3!=0 || $4!="OK") print NR": not OK (rc="$3" tag="$4")";
  if ($6 !~ /^[0-9]+(\.[0-9]+)?$/ || $6+0 >= 4) print NR": load1_before "$6" not < 4";
  if ($7 !~ /^[0-9]+(\.[0-9]+)?$/) print NR": rate "$7" not numeric";
  if ($8 != pin) print NR": chrome_ver "$8" != pin";
}
END { if (cnt["debug"]!=half || cnt["release"]!=half) print "per-binary counts debug="cnt["debug"]+0" release="cnt["release"]+0" expected "half" each" }' "$F")
if [ -n "$bad" ]; then echo "red:" >&2; echo "$bad" >&2; exit 1; fi
echo "green: rows=$rows, $((N/2)) per binary, all OK at load1_before < 4, pin=$PIN"
