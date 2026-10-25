#!/usr/bin/env bash
# ── R0.4.a.c gate: a full-corpus tri-sweep file is complete and honest. ──
#   usage: tri-full-gate.sh <tri-sweep-*.tsv> <expected-rows> <chrome-pin>
# Red (rc=1) unless: the header + exactly <expected-rows> data rows; every row has 20 fields;
# m2_tag and a11y_tag are never empty; chrome_ver (field 20) equals the pin on every row.
set -uo pipefail
F="${1:?file}"; N="${2:?expected rows}"; PIN="${3:?chrome pin}"
[ -f "$F" ] || { echo "✗ missing: $F"; exit 1; }
DATA=$(($(wc -l < "$F") - 1))
BAD=$(awk -F'\t' -v pin="$PIN" 'NR>1 && (NF!=20 || $8=="" || $16=="" || $20!=pin){n++}END{print n+0}' "$F")
echo "rows=$DATA expected=$N bad_rows=$BAD  file=$F"
[ "$DATA" -eq "$N" ] && [ "$BAD" -eq 0 ] && { echo "✓ tri-full-gate green"; exit 0; }
echo "✗ tri-full-gate red"; exit 1
