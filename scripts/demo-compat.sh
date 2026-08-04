#!/usr/bin/env bash
# Emit demo/www/compat.json — the engine's OWN Phase-0 progress, read straight from the burndown ledger.
#
# The demo's front page shows a live compat panel (M1 render bar, M2 function proxy). Hard-coding those
# numbers guarantees they rot: the engine gains a point of fidelity every few ticks and the hosted page
# would keep claiming a months-old figure. So the panel reads THIS file, and this file is regenerated on
# every deploy from the last honest row of docs/loop/FIDELITY-PROGRESS.tsv — the same source the
# fidelity-progress and death-tail instruments read. A number the visitor sees is a number the project
# is actually standing behind today, not whenever the demo was last hand-edited.
#
# HONESTY, mechanically: a burndown row is skipped when the line immediately above it is a comment that
# says "contaminated" (t771's --jobs8 denominator collapse, t820's mozjs-teardown crash-fill were both
# tagged that way). Reading a contaminated row would publish a number the ledger itself disowns.
set -euo pipefail
cd "$(dirname "$0")/.."

LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
OUT=demo/www/compat.json

if [ ! -f "$LEDGER" ]; then
  echo "demo-compat: $LEDGER missing — leaving $OUT as-is" >&2
  exit 0
fi

# Last non-comment, non-contaminated data row → the fields the panel needs.
#   $2 tick  $4 scored  $7 shape_mean  $11 inscope  $12 shape-only%  $14 jarring-clean%  $15 M1%
JSON=$(awk -F'\t' '
  /^#/   { if (tolower($0) ~ /contaminat/) cflag=1; next }
  $1=="iso_sweep" || $1=="" { next }
  { if (cflag) { cflag=0; next }            # this data row was flagged by the comment above it
    tick=$2; scored=$4+0; shmean=$7+0; inscope=$11+0; shapep=$12+0; jarp=$14+0; m1=$15+0
  }
  END {
    if (inscope <= 0) { print ""; exit }
    scoredp = 100.0*scored/inscope
    # shape_mean is stored 0-100 in the ledger; the panel wants the 0-1 fidelity fraction.
    shmean_frac = shmean/100.0
    printf "{\n"
    printf "  \"tick\": \"%s\",\n", tick
    printf "  \"m1_pct\": %.1f,\n", m1
    printf "  \"shape_pct\": %.1f,\n", shapep
    printf "  \"jarring_pct\": %.1f,\n", jarp
    printf "  \"scored_pct\": %.0f,\n", scoredp
    printf "  \"shape_mean\": %.2f,\n", shmean_frac
    printf "  \"target_pct\": 95,\n"
    printf "  \"inscope\": %d,\n", inscope
    printf "  \"corpus\": \"representative CrUX sample\"\n"
    printf "}\n"
  }' "$LEDGER")

if [ -z "$JSON" ]; then
  echo "demo-compat: no usable ledger row — leaving $OUT as-is" >&2
  exit 0
fi

printf '%s\n' "$JSON" > "$OUT"
echo "── compat.json ← $(grep -o '"tick": "[^"]*"' "$OUT") · M1 $(grep -o '"m1_pct": [0-9.]*' "$OUT" | grep -o '[0-9.]*')%"
