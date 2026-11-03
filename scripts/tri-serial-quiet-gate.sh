#!/usr/bin/env bash
# Gate for R0.4.a.c.a.b.a.a.a: the quiet-host serial re-drive TSV (same columns as tri-serial-gate.sh).
#   usage: tri-serial-quiet-gate.sh <tsv> <expected_rows> <chrome_ver>
# Green only if tri-serial-gate.sh is green AND every row has load1_before < 4.
# Red (rc=1) otherwise, with the reason on stderr.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
F="$1"; N="$2"; PIN="$3"
"$HERE/tri-serial-gate.sh" "$F" "$N" "$PIN" || exit 1
busy=$(awk -F'\t' 'NR>1 && $5 >= 4 { print NR": load1_before="$5" >= 4" }' "$F")
if [ -n "$busy" ]; then echo "red: host not quiet:" >&2; echo "$busy" >&2; exit 1; fi
echo "green: all rows load1_before < 4"
