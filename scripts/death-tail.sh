#!/usr/bin/env bash
# death-tail.sh — CROSSINGS-PER-TICK + the DEATH-TAIL calculation for M1 (render) and M2 (function proxy).
#
# WHY THIS EXISTS. "How many ticks to M1?" has two honest answers that bracket the truth:
#   • LINEAR      — extrapolate the recent crossing rate. Optimistic; assumes batches keep landing.
#   • DECAY       — if the crossing rate is itself falling (a DEATH TAIL), a geometric projection shows
#                   whether 95% is even REACHABLE, and if so, in how many ticks. Pessimistic; assumes the
#                   current between-batch deceleration is permanent.
# The DEATH-TAIL SIGNAL (memory session-871+/phase0): the tail becomes ROI-negative when marginal
# CROSSINGS-PER-TICK decay toward ~0 AND stop arriving in family-sized batches. This script computes the
# quantifiable half of that (the rate + its decay); the family-vs-singleton half needs the agent's per-site
# cause labels, which are not in the ledger — that limit is stated in the output, not hidden.
#
# It is a LEDGER + ALERT, never a gate (exit 0 always). Reads the observer-owned burndown
# docs/loop/FIDELITY-PROGRESS.tsv (f2 tick · f4 scored · f11 inscope · f13 corpus · f15 m1_pass_pct),
# honors the SAME-corpus + CONTAMINATION-skip rules the sibling scripts use, and writes a derived ledger
# docs/loop/DEATH-TAIL.tsv whose headline column is crossings_per_tick.
#
#   scripts/death-tail.sh            # write the derived ledger + print the M1/M2 death-tail analysis
#   scripts/death-tail.sh --oneline  # one line for ops-check / heartbeat
#   scripts/death-tail.sh --check    # quiet; emit DEATH-TAIL alert lines only
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
OUT=docs/loop/DEATH-TAIL.tsv
MODE="${1:-full}"
[ -f "$LEDGER" ] || { [ "$MODE" != "--check" ] && echo "death-tail: no $LEDGER yet"; exit 0; }

B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; C=$'\033[36m'; R=$'\033[31m'; O=$'\033[0m'
[ "$MODE" = "--check" ] && { B=; G=; Y=; C=; R=; O=; }

# ── Extract clean, same-corpus (crux) rows in tick order → (tick, inscope, m1pct, scored) ────────────────
# CLEAN = the row is NOT immediately preceded by a '# ... contaminat ...' ledger comment (t771, t820).
# CORPUS = f13 (missing⇒265). We instrument the CrUX burndown (the Phase-0 target corpus); if there are
# <2 crux rows we fall back to whatever the majority corpus is so the tool still says something on old data.
ROWS=$(awk -F'\t' '
  /^#/ { if(tolower($0) ~ /contaminat/) cflag=1; next }
  $1=="iso_sweep" || $1=="" { next }
  { c=(cflag?1:0); cflag=0
    corpus=($13==""?"265":$13)
    m1=($15!=""?$15:$12)                        # f15 m1_pass_pct, fall back to f12 (pre-jarring rows)
    if(c) next                                  # skip contaminated
    t=$2; gsub(/[^0-9]/,"",t)                   # sweep_tick is "t807" — strip the 't' to a bare number
    if(t=="") next
    printf "%s\t%s\t%s\t%s\t%s\n", t, ($11+0), (m1+0), ($4+0), corpus }' "$LEDGER")

# pick target corpus = crux if present, else the most common corpus among clean rows
CORPUS=$(printf '%s\n' "$ROWS" | awk -F'\t' '{c[$5]++} END{best="265";bn=0; for(k in c) if(c[k]>bn){bn=c[k];best=k} print (("crux" in c)?"crux":best)}')
CROWS=$(printf '%s\n' "$ROWS" | awk -F'\t' -v c="$CORPUS" '$5==c{print}')
NROWS=$(printf '%s\n' "$CROWS" | grep -c . )

if [ "${NROWS:-0}" -lt 3 ]; then
  [ "$MODE" = "--oneline" ] && { echo "DEATH-TAIL ▸ only ${NROWS} clean ${CORPUS} sweeps — need ≥3 for a rate; no projection yet"; exit 0; }
  [ "$MODE" = "--check" ] && exit 0
  echo "death-tail: only ${NROWS} clean ${CORPUS} sweep(s) in $LEDGER — need ≥3 for crossings-per-tick. Nothing to project."
  exit 0
fi

# ── Write the derived ledger (the crossings-per-tick COLUMN) + compute the analysis in one awk ───────────
# m1cnt = round(m1pct/100 * inscope) = sites over the M1 bar. crossings = Δm1cnt between consecutive clean
# same-corpus sweeps. cross_per_tick = crossings / Δtick. We also carry scored (M2 proxy: scorability).
ANALYSIS=$(printf '%s\n' "$CROWS" | awk -F'\t' -v OUT="$OUT" -v corpus="$CORPUS" '
  function rnd(x){ return int(x+0.5) }
  {
    tick[NR]=$1+0; ins[NR]=$2+0; pct[NR]=$3+0; scd[NR]=$4+0
    m1cnt[NR]=rnd($3/100*$2); n=NR
  }
  END{
    # newest inscope defines the target
    INSCOPE=ins[n]; TARGET=int(0.95*INSCOPE+0.999)
    CUR=m1cnt[n]; NEEDM1=TARGET-CUR; if(NEEDM1<0)NEEDM1=0
    CURSC=scd[n]; NEEDSC=TARGET-CURSC; if(NEEDSC<0)NEEDSC=0

    # ---- derived ledger with the crossings_per_tick column ----
    printf "# derived by scripts/death-tail.sh — crossings-per-tick + death-tail bracket. corpus=%s. clean rows only (t771/t820 contaminated skipped). Do not hand-edit.\n", corpus > OUT
    printf "from_tick\tto_tick\tdticks\tfrom_m1cnt\tto_m1cnt\tm1_crossings\tm1_cross_per_tick\tfrom_scored\tto_scored\tsc_cross_per_tick\tcorpus\n" >> OUT
    for(i=2;i<=n;i++){
      dt=tick[i]-tick[i-1]; if(dt<=0)dt=1
      dc=m1cnt[i]-m1cnt[i-1]; ds=scd[i]-scd[i-1]
      printf "%d\t%d\t%d\t%d\t%d\t%+d\t%+.3f\t%d\t%d\t%+.3f\t%s\n", tick[i-1],tick[i],dt,m1cnt[i-1],m1cnt[i],dc,dc/dt,scd[i-1],scd[i],ds/dt,corpus >> OUT
    }

    # ---- windowed rates: split the interval series into EARLY vs RECENT by tick span ----
    # recent window = trailing intervals covering ~the last 40 ticks (min 2 intervals); early = the rest.
    span=tick[n]-tick[1]
    rt=tick[n]-40; if(rt<tick[1]) rt=tick[1]
    # find split index s: first row with tick>=rt (recent window = rows s..n)
    s=1; for(i=1;i<=n;i++){ if(tick[i]>=rt){ s=i; break } }
    if(s>n-2) s=n-2; if(s<1) s=1                # ensure >=2 recent intervals if possible

    # cumulative rate over a window [a..b] in sites/tick, for M1 (m1cnt) and M2 proxy (scored)
    dt_rec=tick[n]-tick[s]; if(dt_rec<=0)dt_rec=1
    r_recent   = (m1cnt[n]-m1cnt[s])/dt_rec
    rs_recent  = (scd[n]-scd[s])/dt_rec
    dt_all=tick[n]-tick[1]; if(dt_all<=0)dt_all=1
    r_all      = (m1cnt[n]-m1cnt[1])/dt_all
    dt_early=tick[s]-tick[1]; if(dt_early<=0)dt_early=1
    r_early    = (m1cnt[s]-m1cnt[1])/dt_early

    # ---- LINEAR ETAs (the optimistic bracket) ----
    eta_lin_recent = (r_recent>0.0001) ? NEEDM1/r_recent : -1
    eta_lin_all    = (r_all>0.0001)    ? NEEDM1/r_all    : -1

    # ---- DECAY model (the death-tail bracket) ----
    # rho = how the rate multiplies from the early window to the recent window.
    rho = (r_early>0.0001 && r_recent>0.0001) ? r_recent/r_early : -1
    # W = tick-span of the recent window (the decay time-constant unit)
    W = dt_rec
    decay_verdict=""; eta_decay=-1; asym_cross=-1; asym_pct=-1
    if(rho>0 && rho<0.90 && r_recent>0.0001){
      # continuous decay rate(t)=r_recent*rho^(t/W). Max future crossings = r_recent*W/(-ln rho).
      k = -log(rho)                                   # >0
      asym_cross = r_recent*W/k                       # total additional sites reachable as t->inf
      asym_pct = 100*(CUR+asym_cross)/INSCOPE
      if(asym_cross < NEEDM1){
        decay_verdict="UNREACHABLE"                   # death-tail: converges below 95%
      } else {
        # solve NEEDM1 = asym_cross*(1 - rho^(T/W)) -> T
        frac = 1 - NEEDM1/asym_cross                  # = rho^(T/W), in (0,1)
        eta_decay = W*log(frac)/log(rho)              # ticks
        decay_verdict="REACHABLE"
      }
    } else if(rho>=0.90){
      decay_verdict="LINEAR"                          # no meaningful deceleration
    } else {
      decay_verdict="STALLED"                         # recent rate ~0 or negative
    }

    # ---- death-tail SIGNAL: recent rate low AND clearly decelerating ----
    sig = (r_recent < 0.10 && rho>0 && rho<0.6) ? 1 : 0
    if(r_recent<=0.0001) sig=1

    # ---- M2 proxy (scorability ceiling) linear ETA ----
    sc_pct = 100*CURSC/INSCOPE
    eta_sc = (rs_recent>0.0001) ? NEEDSC/rs_recent : -1

    # emit machine-readable line for bash to format
    printf "META\t%d\t%d\t%d\t%d\t%.4f\t%.4f\t%.4f\t%.3f\t%.1f\t%.1f\t%.1f\t%s\t%.1f\t%d\t%.2f\t%.1f\t%d\t%.1f\t%.1f\t%d\t%d\t%d\t%d\n",
      INSCOPE, TARGET, CUR, NEEDM1,
      r_recent, r_all, r_early, (rho<0?-1:rho),
      (eta_lin_recent<0?-1:eta_lin_recent), (eta_lin_all<0?-1:eta_lin_all),
      (eta_decay<0?-1:eta_decay), decay_verdict,
      (asym_cross<0?-1:asym_cross), (asym_pct<0?-1:int(asym_pct+0.5)),
      rho<0?-1:rho, W,
      CURSC, sc_pct, (eta_sc<0?-1:eta_sc), NEEDSC,
      sig, tick[s], tick[n]
  }')

META=$(printf '%s\n' "$ANALYSIS" | awk -F'\t' '$1=="META"')
read -r _ INSCOPE TARGET CUR NEEDM1 R_RECENT R_ALL R_EARLY RHO ETA_LIN_R ETA_LIN_A ETA_DECAY DVERDICT ASYM_CROSS ASYM_PCT RHO2 W CURSC SC_PCT ETA_SC NEEDSC SIG SPLIT_TICK LAST_TICK <<<"$META"

# ── M2 real-artefact check (auto-activate when FUNCTION-CERT.tsv exists) ──────────────────────────────────
M2_REAL=""; [ -f docs/loop/FUNCTION-CERT.tsv ] && M2_REAL=$(awk -F'\t' '/overall|OVERALL|pass_pct/{for(i=1;i<=NF;i++) if($i+0>0 && $i+0<=100) v=$i} END{print v+0}' docs/loop/FUNCTION-CERT.tsv 2>/dev/null)

fmt_ticks(){ awk -v v="$1" 'BEGIN{ if(v<0) printf "∞"; else printf "~%d", int(v+0.5) }'; }

# ── --check: alert only if the death-tail signal is ACTIVE ───────────────────────────────────────────────
if [ "$MODE" = "--check" ]; then
  if [ "${SIG:-0}" = 1 ]; then
    printf "DEATH-TAIL-SIGNAL: M1 crossings-per-tick recent=%.3f sites/tick, decay ρ=%.2f/window — %s. Verify family-vs-singleton in the latest SWEEP rows before treating 95%% as at-risk.\n" "$R_RECENT" "$RHO2" "$DVERDICT"
  fi
  exit 0
fi

# ── --oneline: for ops-check / heartbeat ─────────────────────────────────────────────────────────────────
if [ "$MODE" = "--oneline" ]; then
  DV="$DVERDICT"
  case "$DVERDICT" in
    UNREACHABLE) DTXT="decay→UNREACHABLE (asymptote ~${ASYM_PCT}%)";;
    REACHABLE)   DTXT="decay→$(fmt_ticks "$ETA_DECAY") ticks";;
    LINEAR)      DTXT="no decel";;
    *)           DTXT="STALLED (recent rate≈0)";;
  esac
  SIGTXT=""; [ "${SIG:-0}" = 1 ] && SIGTXT=" ⚠SIGNAL-ACTIVE"
  printf "DEATH-TAIL ▸ M1 cross/tick recent=%.3f (ρ=%.2f) → linear $(fmt_ticks "$ETA_LIN_R")t · %s%s  [M2 proxy: scorability %.0f%%%s]\n" \
    "$R_RECENT" "$RHO2" "$DTXT" "$SIGTXT" "$SC_PCT" "$([ -n "$M2_REAL" ] && echo " · CERT ${M2_REAL}%" || echo "")"
  exit 0
fi

# ── full report ──────────────────────────────────────────────────────────────────────────────────────────
echo "${B}══ DEATH-TAIL — crossings-per-tick + reachability (corpus=${CORPUS} · clean rows · window ${SPLIT_TICK}→${LAST_TICK}) ══${O}"
echo
echo "  ${B}M1 RENDER${O}  (shape≥0.75 ∧ jarring-clean)   now ${B}${CUR}${O}/${INSCOPE} sites · target ${TARGET} (95%) · ${B}need +${NEEDM1}${O}"
printf "    crossings-per-tick:  recent(≤40t)=%s%.3f%s  ·  all-clean=%.3f  ·  early=%.3f  sites/tick   (deceleration ρ=%s%.2f%s per window)\n" \
  "$B" "$R_RECENT" "$O" "$R_ALL" "$R_EARLY" "$B" "$RHO2" "$O"
echo
echo "    ${B}LINEAR bracket${O} (optimistic — assumes batches keep landing):"
printf "       at recent rate → %s ticks   ·   at all-clean rate → %s ticks\n" "$(fmt_ticks "$ETA_LIN_R")" "$(fmt_ticks "$ETA_LIN_A")"
echo "    ${B}DECAY bracket${O} (pessimistic — assumes the current deceleration is permanent):"
case "$DVERDICT" in
  UNREACHABLE)
    printf "       %s⛔ 95%% UNREACHABLE at this decay%s — the rate converges; asymptote ≈ %s%s%%%s of in-scope (+%.1f more sites, then it flattens).\n" \
      "$R" "$O" "$B" "$ASYM_PCT" "$O" "$ASYM_CROSS"
    echo   "          ⇒ a DEATH TAIL by this model: only a fresh family-sized batch (RTL, IO, tables, grid…) bends it back. If none lands, the 95% bar or the M1→M2 sequence needs an owner decision."
    ;;
  REACHABLE)
    printf "       reachable, but slowly → %s ticks (asymptote ≈ %s%% leaves headroom above 95%%).\n" "$(fmt_ticks "$ETA_DECAY")" "$ASYM_PCT"
    ;;
  LINEAR)
    echo   "       ρ≈1 — no meaningful deceleration yet; the linear bracket is the honest estimate."
    ;;
  *)
    echo   "       ${R}recent crossings-per-tick ≈ 0 or negative${O} — no forward motion to extrapolate; treat linear-∞ as the reading until the next batch lands."
    ;;
esac
echo
if [ "${SIG:-0}" = 1 ]; then
  echo "    ${Y}${B}⚠ DEATH-TAIL SIGNAL ACTIVE${O}${Y} — recent rate low AND decelerating.${O} This is the quantifiable half only."
  echo "      The other half (are the remaining crossings SINGLETONS or a FAMILY?) needs per-site cause labels not in the ledger:"
  echo "      check the newest docs/loop/SWEEP-t*-rows.tsv — if the sites nearest the bar share a cause, a batch is still available and the signal is a between-batch lull, not the true tail."
else
  echo "    ${G}✓ death-tail signal not active${O} — crossings still arriving fast enough / not yet clearly decelerating."
fi
echo
echo "  ${B}M2 FUNCTION${O}"
if [ -n "$M2_REAL" ]; then
  echo "    real cert artefact present: docs/loop/FUNCTION-CERT.tsv → ${B}${M2_REAL}%${O} (render∧function). Full death-tail calc on this leg is now live above the proxy."
else
  printf "    no FUNCTION-CERT.tsv yet → instrumenting the %sPROXY%s: scorability ceiling (boot/render rate = function precondition).\n" "$B" "$O"
  printf "       scorability now ${B}%.0f%%${O} of in-scope (need +%s sites to 95%%) · at recent rate → %s ticks\n" "$SC_PCT" "$NEEDSC" "$(fmt_ticks "$ETA_SC")"
  echo   "       ⚠ this is a PROXY, not M2: it measures whether a site BOOTS, not whether it FUNCTIONS. The true M2 (BiDi"
  echo   "         render∧function A/B, bidi/src/protocol.rs) has no ledger — it is deferred until scorability ≥85%, and"
  echo   "         this block auto-switches to the real calc the moment docs/loop/FUNCTION-CERT.tsv appears."
  awk "BEGIN{ if(($SC_PCT+0) < ($CUR*100/$INSCOPE + 5)) exit 0; else exit 1 }" 2>/dev/null \
    || echo "       note: scorability (${SC_PCT%.*}%) sits far above M1 (${CUR}/${INSCOPE}) — M1 is NOT ceiling-bound right now; it is shape/jarring-bound among sites that already boot."
fi
echo
echo "  derived ledger written: ${OUT}  (headline column: m1_cross_per_tick)"
echo "  last intervals:"
grep -vE '^#|^from_tick' "$OUT" 2>/dev/null | tail -6 | awk -F'\t' '{printf "    %s→%s  %2st  %+d sites  %+.3f/tick  (scored %s→%s)\n",$1,$2,$3,$6,$7,$8,$9}'
exit 0
