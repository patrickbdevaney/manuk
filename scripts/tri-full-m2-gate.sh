#!/usr/bin/env bash
# ── R0.4.a.c.a gate: every row of TRI-FULL-M2.md cites a line that exists verbatim in the evidence file,
# and the table lists exactly the sweep's non-OK, non-SKIPPED M2 sites. usage: tri-full-m2-gate.sh [md] [evidence] [sweep.tsv]
set -uo pipefail
MD="${1:-docs/sprint/TRI-FULL-M2.md}"; EV="${2:-docs/sprint/TRI-FULL-M2-evidence.txt}"; SW="${3:-docs/bench/tri-sweep-20261025-054452.tsv}"
bad=0; n=0
while IFS='|' read -r _ url _ _ cite _; do
  url=$(echo "$url"|xargs); cite=$(echo "$cite"|xargs); n=$((n+1))
  grep -qxF -- "$cite" "$EV" || { echo "✗ cite not verbatim in evidence: $url :: $cite"; bad=$((bad+1)); }
  awk -F'\t' -v u="$url" '$2==u&&($8=="TIMEOUT"||$8=="CRASH"){f=1}END{exit !f}' "$SW" || { echo "✗ not a non-OK M2 site: $url"; bad=$((bad+1)); }
done < <(grep -E '^\| https?://' "$MD")
want=$(awk -F'\t' '$8=="TIMEOUT"||$8=="CRASH"' "$SW" | wc -l)
echo "rows=$n expected=$want bad=$bad"
[ "$n" -eq "$want" ] && [ "$bad" -eq 0 ] && { echo "✓ tri-full-m2-gate green"; exit 0; }
echo "✗ tri-full-m2-gate red"; exit 1
