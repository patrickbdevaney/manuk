#!/usr/bin/env bash
# fidelity-progress.sh — TRACK the Phase-0 fidelity basket over time from the AGENT's banked corpus sweeps,
# so it cannot silently stall and a "number went up" reading is checked against the DENOMINATOR TRAP.
#
# ⚠ REBUILT-INSTRUMENT SCHEMA (t531-537 + the t706 sweep-repair). The instrument changed its output at the
# t531-537 rebuild; the OLD reader here (and scripts/fidelity-sweep.sh) grepped a dead vocabulary
# ("PLACEMENT: X% within Npx", "ids") the instrument stopped printing, so the ledger sat on ONE stale row
# (2026-07-20, place_mean 6.4 = the OLD instrument's ABSOLUTE placement) and never saw a real sweep. The
# agent (tick 706, d7946743) diagnosed this and banks honest rows at docs/loop/SWEEP-t<NNN>-rows.tsv. This
# reader now reads THOSE. The new per-site columns are:
#   1 name · 2 coverage(0-1) · 3 shape(0-1, parent-relative) · 4 h_overflow · 5 overlap · 6 reading_order
#   7 dead_target · 8 shape_n · 9 reason(empty ⇒ SCORED; non-empty ⇒ unscored, e.g. bot-wall/crashed) · 10 instrument
# ⚠ shape here is PARENT-RELATIVE SHAPE, NOT the old absolute placement — the two are NOT differenceable
# ("6.4 -> 43.0 would be a metric swap dressed as progress" — the agent). The trend only compares rebuilt-
# instrument sweeps to each other.
#
# The Phase-0 exit metric is shape>=0.75 on >=95% of the corpus, FIXED DENOMINATOR (unscored count as fails,
# never dropped — that is how a broken engine scores 95%). LEDGER + ALERTS only, never a gate (fidelity has
# real run-to-run variance; a raw ratchet would false-brick). Observer-owned reader of the AGENT's output.
#   usage: scripts/fidelity-progress.sh            # record-if-new + print trend + verdict
#          scripts/fidelity-progress.sh --check    # quiet; emit ALERT lines only (for ops-check)
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
MODE="${1:-record}"

# Newest banked corpus sweep with the REBUILT schema (header has a 'shape' and 'reason' column). Skip old
# pre-rebuild banks / pilots that do not carry the new columns.
SRC=""; SRC_EPOCH=0
for f in docs/loop/SWEEP-t*-rows.tsv; do
  [ -f "$f" ] || continue
  head -1 "$f" | grep -qiE 'shape' && head -1 "$f" | grep -qiE 'reason' || continue
  e=$(stat -c %Y "$f" 2>/dev/null || echo 0)
  [ "$e" -gt "$SRC_EPOCH" ] && { SRC="$f"; SRC_EPOCH="$e"; }
done
[ -z "$SRC" ] && { [ "$MODE" != "--check" ] && echo "fidelity-progress: no rebuilt-schema SWEEP-t*-rows.tsv found"; exit 0; }
TICK=$(printf '%s' "$SRC" | grep -oE 't[0-9]+' | head -1)
SWEEP_ISO=$(date -d "@$SRC_EPOCH" '+%Y-%m-%dT%H:%M:%S' 2>/dev/null || echo "?")
NOW_EPOCH=$(date +%s 2>/dev/null || echo "$SRC_EPOCH")

# Aggregate. SCORED = reason empty. shape/coverage means over SCORED. shape>=0.75 % over the FIXED
# denominator (all sites) — the honest Phase-0 metric. A row < ~80% of the corpus is a partial; skip it.
read -r SITES SCORED GE75 GE75PCT SHAPEM COVM < <(awk -F'\t' '
  NR==1{next}
  { sites++
    r=$9; gsub(/[ \t]/,"",r)
    if(r==""){ scored++; sh+=$3; cv+=$2; if($3>=0.75) ge++ } }
  END{
    ge75pct = sites>0 ? 100*ge/sites : 0
    if(scored>0) printf "%d %d %d %.1f %.1f %.1f", sites, scored, ge, ge75pct, 100*sh/scored, 100*cv/scored
    else         printf "%d 0 0 0 0 0", sites
  }' "$SRC")

PREV=""; [ -f "$LEDGER" ] && PREV=$(grep -vE '^(iso|#)' "$LEDGER" 2>/dev/null | tail -1)
p_iso=$(echo "$PREV" | cut -f1); p_tick=$(echo "$PREV" | cut -f2); p_scored=$(echo "$PREV" | cut -f4); p_ge=$(echo "$PREV" | cut -f5); p_sh=$(echo "$PREV" | cut -f7)

# ── record-if-new (by sweep tick) ──────────────────────────────────────────────────────────────────────
if [ "$MODE" != "--check" ]; then
  [ -f "$LEDGER" ] || printf '# rebuilt-instrument schema (t706+); shape=parent-relative, NOT the old absolute placement (6.4 pre-2026-07-28 row is a DIFFERENT metric, do not diff)\niso_sweep\tsweep_tick\tsites\tscored\tshape_ge0.75\tshape_ge0.75_pct\tshape_mean\tcov_mean\tsource\n' > "$LEDGER"
  if [ "$TICK" != "$p_tick" ]; then
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$SWEEP_ISO" "$TICK" "$SITES" "$SCORED" "$GE75" "$GE75PCT" "$SHAPEM" "$COVM" "$SRC" >> "$LEDGER"
    echo "recorded sweep $TICK ($SWEEP_ISO)"
  fi
fi

# ── verdict / alerts ───────────────────────────────────────────────────────────────────────────────────
alerts=""
AGE_DAYS=$(( ( NOW_EPOCH - SRC_EPOCH ) / 86400 ))
[ "$AGE_DAYS" -ge 3 ] && alerts="${alerts}STALE-SWEEP: newest banked corpus sweep ($TICK) is ${AGE_DAYS}d old — agent should run a fresh full sweep so the trend stays live\n"
if [ -n "$PREV" ] && [ "$TICK" != "$p_tick" ]; then
  awk "BEGIN{exit !($SCORED < ${p_scored:-0})}" && alerts="${alerts}SCORABILITY-REGRESSED: scored ${p_scored} -> ${SCORED} (fewer sites measurable — investigate, not progress)\n"
  # denominator trap: shape_mean up while scored down => a hard site dropped out, not real improvement
  if awk "BEGIN{exit !($SHAPEM > ${p_sh:-0} && $SCORED < ${p_scored:-0})}"; then
    alerts="${alerts}DENOMINATOR-TRAP: shape_mean ${p_sh}->${SHAPEM} ROSE while scored ${p_scored}->${SCORED} FELL — likely a hard site dropped out; the gain is NOT real\n"
  fi
fi

if [ "$MODE" = "--check" ]; then [ -n "$alerts" ] && printf "%b" "$alerts"; exit 0; fi

echo "── FIDELITY PROGRESS (rebuilt instrument · sweep $TICK · $SWEEP_ISO · ${AGE_DAYS}d old) ─────────"
printf "  sites=%s  scored=%s  shape>=0.75 on %s (%s%% of corpus, FIXED denom)   shape_mean=%s%%  cov_mean=%s%%\n" \
  "$SITES" "$SCORED" "$GE75" "$GE75PCT" "$SHAPEM" "$COVM"
echo "  TARGET (Phase-0 exit): shape>=0.75 on >=95%% of the corpus · scored up toward $SITES · shape_mean/cov_mean up"
if [ -n "$PREV" ] && [ "$TICK" != "$p_tick" ]; then
  printf "  Δ vs %s: scored %s→%s · shape>=0.75%% %s→%s · shape_mean %s→%s\n" "$p_tick" "$p_scored" "$SCORED" "$p_ge" "$GE75PCT" "$p_sh" "$SHAPEM"
fi
if [ -n "$alerts" ]; then printf "  ⚠ %b" "$alerts"; else echo "  ✓ no trap/regression/staleness flag"; fi
echo "  last ${LEDGER} rows:"; tail -3 "$LEDGER" 2>/dev/null | sed 's/^/    /'
exit 0
