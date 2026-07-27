#!/usr/bin/env bash
# fidelity-progress.sh — TRACK the Phase-0 fidelity basket over time so it cannot silently stall, and so a
# "number went up" reading is checked against the DENOMINATOR TRAP the journal documents repeatedly:
#   * "MEAN COVERAGE 85.2% -> 98.7% is aparat DROPPING OUT, not an improvement"  (coverage rose on a
#     SHRINKING denominator — a hard site left the scored set)
#   * "keirin coverage 98.0% -> 74.4% is the HONEST COST of a real layout"       (coverage FELL because we
#     attempted MORE boxes — a real gain that looks like a regression)
# So the two numbers the user asked to drive — SCORABILITY (how many sites we can measure at all) and
# COVERAGE (of those, how many boxes we render) — are IN TENSION, and neither is trustworthy alone. This
# records BOTH plus placement(shape)/visual/box-counts every sweep, and flags the trap instead of cheering.
#
# It is a LEDGER + TREND read, never a gate: fidelity has real run-to-run variance (~3.7pt shape on ONE
# unchanged tree — reliability-doctrine), so wiring a raw number into the ratchet would false-brick the
# loop. This ALERTS (via ops-check) and RECORDS; it never refuses a tick. Observer-owned (records the
# AGENT's manuk-wpt sweep output; does not compute fidelity itself).
#   usage: scripts/fidelity-progress.sh            # record-if-changed + print trend + verdict
#          scripts/fidelity-progress.sh --check    # quiet; emit ALERT lines only (for ops-check)
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
MODE="${1:-record}"

# Freshest sweep output among the known locations (full corpus preferred; fall back to the smaller sweep).
SRC=""; SRC_EPOCH=0
for f in .git/fidelity-full/results.tsv .git/fidelity-sweep/results.tsv; do
  [ -f "$f" ] || continue
  e=$(stat -c %Y "$f" 2>/dev/null || echo 0)
  [ "$e" -gt "$SRC_EPOCH" ] && { SRC="$f"; SRC_EPOCH="$e"; }
done
[ -z "$SRC" ] && { [ "$MODE" != "--check" ] && echo "fidelity-progress: no sweep results.tsv found"; exit 0; }

# Compute the basket from the freshest sweep. Columns (1-based): 3 status · 4 visual · 5 coverage ·
# 6 placement · 7 ids(matched) · 8 missing · 9 misplaced. Scorable == status OK.
read -r SCOR TOT COVM COVP PLM VISM MISS MISP < <(awk -F'\t' '
  NR>1{ tot++
    if($3=="OK"){ ok++; cv+=$5; pl+=$6; vs+=$4; ids+=$7; ms+=$8; mp+=$9 } }
  END{
    covp = (ids+ms>0)? 100*ids/(ids+ms) : 0
    if(ok>0) printf "%d %d %.1f %.1f %.1f %.1f %d %d", ok, tot, cv/ok, covp, pl/ok, vs/ok, ms, mp
    else     printf "0 %d 0 0 0 0 0 0", tot
  }' "$SRC")
SWEEP_ISO=$(date -d "@$SRC_EPOCH" '+%Y-%m-%dT%H:%M:%S' 2>/dev/null || echo "?")
AGE_DAYS=$(( ( $(date +%s) - SRC_EPOCH ) / 86400 ))

# ── prior row (for trend + trap detection) ────────────────────────────────────────────────────────────
PREV=""; [ -f "$LEDGER" ] && PREV=$(grep -vE '^(iso|#)' "$LEDGER" 2>/dev/null | tail -1)
p_scor=$(echo "$PREV" | cut -f2); p_covp=$(echo "$PREV" | cut -f5); p_pl=$(echo "$PREV" | cut -f6)
p_sweep=$(echo "$PREV" | cut -f1)

# ── record-if-changed: append only when THIS sweep (by its own timestamp) is new to the ledger ─────────
if [ "$MODE" != "--check" ]; then
  [ -f "$LEDGER" ] || printf 'iso_sweep\tscorable\ttotal\tcov_mean\tcov_pooled\tplace_mean\tvis_mean\tmissing\tmisplaced\tsource\n' > "$LEDGER"
  if [ "$SWEEP_ISO" != "$p_sweep" ]; then
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$SWEEP_ISO" "$SCOR" "$TOT" "$COVM" "$COVP" "$PLM" "$VISM" "$MISS" "$MISP" "$SRC" >> "$LEDGER"
    echo "recorded sweep $SWEEP_ISO"
  fi
fi

# ── verdict / alerts (shared by both modes) ────────────────────────────────────────────────────────────
alerts=""
[ "$AGE_DAYS" -ge 2 ] && alerts="${alerts}STALE-SWEEP: corpus sweep is ${AGE_DAYS}d old ($SWEEP_ISO) — re-run manuk-wpt fidelity so scorability/coverage trend stays live\n"
if [ -n "$PREV" ] && [ "$SWEEP_ISO" != "$p_sweep" ]; then
  awk "BEGIN{exit !($SCOR < $p_scor)}" && alerts="${alerts}SCORABILITY-REGRESSED: scorable ${p_scor} -> ${SCOR} (fewer sites measurable — investigate, not progress)\n"
  # denominator trap: coverage up while scorability down => the rise is composition, not rendering
  if awk "BEGIN{exit !($COVP > $p_covp && $SCOR < $p_scor)}"; then
    alerts="${alerts}DENOMINATOR-TRAP: cov_pooled ${p_covp}->${COVP} ROSE while scorable ${p_scor}->${SCOR} FELL — a hard site likely dropped out; the coverage gain is NOT real\n"
  fi
fi

if [ "$MODE" = "--check" ]; then
  [ -n "$alerts" ] && printf "%b" "$alerts"
  exit 0
fi

# ── human trend read ───────────────────────────────────────────────────────────────────────────────────
echo "── FIDELITY PROGRESS (source: $SRC, sweep ${SWEEP_ISO}, ${AGE_DAYS}d old) ─────────"
printf "  scorable=%s/%s   cov_pooled=%s%%   cov_mean=%s%%   placement(shape)=%s%%   visual=%s%%   missing=%s  misplaced=%s\n" \
  "$SCOR" "$TOT" "$COVP" "$COVM" "$PLM" "$VISM" "$MISS" "$MISP"
echo "  TARGET: scorable ↑ toward $TOT · cov_pooled → 95 · placement → 95 · visual → 95  (all four, per the Phase-0 certificate)"
if [ -n "$PREV" ] && [ "$SWEEP_ISO" != "$p_sweep" ]; then
  printf "  Δ vs %s: scorable %s→%s · cov_pooled %s→%s · placement %s→%s\n" "$p_sweep" "$p_scor" "$SCOR" "$p_covp" "$COVP" "$p_pl" "$PLM"
fi
if [ -n "$alerts" ]; then printf "  ⚠ %b" "$alerts"; else echo "  ✓ no trap/regression/staleness flag"; fi
echo "  last ${LEDGER} rows:"; tail -4 "$LEDGER" 2>/dev/null | sed 's/^/    /'
exit 0
