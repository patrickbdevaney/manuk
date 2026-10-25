#!/usr/bin/env bash
# ── TRI-BAND — 3-run band over tri-sweep persisted TSVs (R0.4.a.b).
# usage: tri-band.sh OUT.tsv RUN1.tsv RUN2.tsv RUN3.tsv
# Refuses (rc=1) unless every run has exactly 10 data rows, 20 fields per row, and chrome_ver == $MANUK_CHROME_PIN.
# Metrics per run, using the tri-sweep FULL headline rules (a non-OK row scores 0):
#   M1_SHAPE   mean SHAPE % over reachable sites (m1_status OK or LOW_SAMPLE)
#   M2_FULL    mean drive rate over all rows
#   A11Y_FULL  mean a11y F1 over all rows
#   REACHABLE  count of sites M1 reached (OK, LOW_SAMPLE or NO_STRUCT)
# Writes one row per metric: run1 run2 run3 min max spread.
set -uo pipefail
PIN=${MANUK_CHROME_PIN:?MANUK_CHROME_PIN unset}
OUT=$1; shift
[ "$#" -eq 3 ] || { echo "✗ need exactly 3 run files"; exit 2; }
for f in "$@"; do
  [ -f "$f" ] || { echo "✗ missing run file: $f"; exit 1; }
  n=$(tail -n +2 "$f" | wc -l)
  [ "$n" -eq 10 ] || { echo "✗ $f has $n data rows, need 10"; exit 1; }
  bad=$(tail -n +2 "$f" | awk -F'\t' -v p="$PIN" 'NF!=20 || $20!=p{c++}END{print c+0}')
  [ "$bad" -eq 0 ] || { echo "✗ $f: $bad rows with NF!=20 or chrome_ver!=$PIN"; exit 1; }
done

# file -> "M1_SHAPE M2_FULL A11Y_FULL REACHABLE"
run_metrics() {
  tail -n +2 "$1" | awk -F'\t' '
    { n++
      if ($8 == "OK") m2 += $9          # m2_tag, m2_rate
      if ($16 == "OK") f1 += $19        # a11y_tag, a11y_f1
      if ($3 == "OK" || $3 == "LOW_SAMPLE") { if ($5 != "") { s += $5; sn++ } ; reach++ }
      else if ($3 == "NO_STRUCT") reach++ }
    END { printf "%.2f %.2f %.2f %d\n", (sn ? s / sn : 0), m2 / n, f1 / n, reach + 0 }'
}

declare -a RUNVALS
i=0
for f in "$@"; do i=$((i + 1)); RUNVALS[$i]=$(run_metrics "$f"); done

{
  printf 'metric\trun1\trun2\trun3\tmin\tmax\tspread\n'
  k=0
  for name in M1_SHAPE M2_FULL A11Y_FULL REACHABLE; do
    k=$((k + 1)); vals=()
    for j in 1 2 3; do vals+=("$(echo "${RUNVALS[$j]}" | cut -d' ' -f$k)"); done
    mn=$(printf '%s\n' "${vals[@]}" | sort -g | head -1)
    mx=$(printf '%s\n' "${vals[@]}" | sort -g | tail -1)
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "${vals[0]}" "${vals[1]}" "${vals[2]}" "$mn" "$mx" \
      "$(awk -v a="$mx" -v b="$mn" 'BEGIN{printf "%.2f", a - b}')"
  done
} > "$OUT"
echo "▶ wrote $OUT"; cat "$OUT"
