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

# ── CONTAMINATION HONOR ──────────────────────────────────────────────────────────────────────────────────
# A sweep can be CONTAMINATED (a --jobs>2 triage run whose wall-clock contention costs hard sites their
# SCORABILITY — timeout-150s/css-starved — while every site that DOES score is unchanged to <0.1pt). The
# agent annotates such a row with a preceding '# ... CONTAMINATED ...' ledger comment and says "read the last
# honest point." HONOR it: display the run, but SUPPRESS the burndown Δ/slope (it would read a false
# regression) and say so. Detect: does a '#...contaminat...' comment immediately precede THIS sweep's row?
CONTAM_SWEEP=0
[ -f "$LEDGER" ] && awk -F'\t' -v t="$TICK" '
  /^#/ { if(tolower($0) ~ /contaminat/) c=1; next }
  { if(c && $2==t){ found=1; exit } c=0 }
  END{ exit !found }' "$LEDGER" && CONTAM_SWEEP=1

# ── CORPUS DETECTION (the metric-swap guard at the corpus switch) ─────────────────────────────────────────
# When the driving corpus changes (265 curated → representative CrUX), the first sweep on the new corpus is a
# NEW BASELINE, NOT a slope point — diffing it against a different site set is the metric-swap lie. Detect the
# corpus of THIS sweep from its CONTENT (max site-name overlap with each corpus file, ground truth), tag the
# ledger row with it, and only diff SAME-CORPUS rows. Old rows without a tag default to '265' (all were).
_names=$(awk -F'\t' 'NR>1{print $1}' "$SRC" 2>/dev/null | sed 's#^https\?://##; s#/.*##; s/^www\.//' | grep -v '^$' | sort -u)
if [ -n "$_names" ]; then
  n265=$(grep -Ff <(printf '%s\n' "$_names") docs/bench/oracle-corpus.txt 2>/dev/null | wc -l)
  ncrux=$(grep -Ff <(printf '%s\n' "$_names") docs/bench/corpus-crux-trend.txt docs/bench/corpus-v2.tsv 2>/dev/null | wc -l)
  CORPUS=$([ "${ncrux:-0}" -gt "${n265:-0}" ] && echo crux || echo 265)
else CORPUS=265; fi

# Aggregate. The Phase-0 exit metric is shape>=0.75 on >=95% of the IN-SCOPE corpus — and the denominator
# is the crux (DAILY-DRIVER-CERTIFICATION.md §3): a site is EXCLUDED (reported separately, its rate watched
# and capped) ONLY when it is permanently unreachable by our own no-stealth policy or simply gone —
# bot-wall-4xx/429, probe-blocked, unreachable, http-404/503, empty-2xx. EVERYTHING else is IN-SCOPE and
# counts against us: scored sites, AND crashed/render-failed/shell-only/thin-overlap (those are OUR bugs,
# not the corpus's — a daily driver may not crash or render a shell on a reachable real site). This is the
# winnable, honest target: dividing by all 265 (incl. the 21% we will never stealth past) caps the ceiling
# at ~80% and makes 95% arithmetically impossible — that pessimism is a metric bug, not a result.
# The partition is driven by the reason STRING the instrument emits, so it self-corrects as the instrument
# gets more precise (a render-failed that turns out to be a soft bot-wall moves to EXCLUDED automatically).
# The existing 9 columns keep their meaning UNCHANGED (f6 = shape>=0.75 over the FIXED 265 denom, so the
# t706 row and all history stay diff-able — a column that silently changes meaning is the metric-swap lie).
# Three columns are ADDED: excluded(f10), inscope(f11), inscope_pass_pct(f12 = the winnable Phase-0 headline).
# A row < ~80% of the corpus is a partial; skip it.
# M1 = the FULL visual bar (DAILY-DRIVER-CERTIFICATION.md §4), not shape alone: shape>=0.75 AND the JARRING
# invariants clean — no horizontal-overflow (f4), no overlap (f5), reading-order preserved (f6), no dead
# click-target (f7), each a Chrome-divergence COUNT. TOL=2 forgives single-element measurement noise; a real
# jarring page (ikea: overlap 6, reading-order 22) still fails. inpass(f12)=shape-only (kept for continuity);
# m1pass(f15)=shape AND jarring = the TRUE M1 gate. jc/m1 divide by in-scope (a crashed site is not clean).
read -r SITES SCORED GE75 GE75PCT SHAPEM COVM EXCL INSCOPE INPASS JARCLEAN JCPCT M1CNT M1PCT < <(awk -F'\t' -v TOL=2 '
  NR==1{next}
  { sites++
    r=$9; gsub(/[ \t]/,"",r)
    if(r ~ /^bot-wall/ || r=="probe-blocked" || r=="unreachable" || r=="http-404" || r=="http-503" || r ~ /^empty-/){ excl++; next }
    inscope++
    if(r==""){ scored++; sh+=$3; cv+=$2
      shape_ok = ($3>=0.75)
      jar_ok = (($4+0)<=TOL && ($5+0)<=TOL && ($6+0)<=TOL && ($7+0)<=TOL)
      if(shape_ok) ge++
      if(jar_ok) jc++
      if(shape_ok && jar_ok) m1++ } }
  END{
    ge75pct = sites>0 ? 100*ge/sites : 0          # LEGACY fixed-265 denom (unchanged meaning)
    inpass  = inscope>0 ? 100*ge/inscope : 0       # shape-only, in-scope
    jcpct   = inscope>0 ? 100*jc/inscope : 0       # jarring-clean, in-scope
    m1pct   = inscope>0 ? 100*m1/inscope : 0       # THE M1 GATE: shape AND jarring, in-scope
    if(scored>0) printf "%d %d %d %.1f %.1f %.1f %d %d %.1f %d %.1f %d %.1f", sites, scored, ge, ge75pct, 100*sh/scored, 100*cv/scored, excl, inscope, inpass, jc, jcpct, m1, m1pct
    else         printf "%d 0 0 0 0 0 %d %d 0 0 0 0 0", sites, excl, inscope
  }' "$SRC")

# PREV = last COMPLETE ledger row for a DIFFERENT tick (skip a same-tick partial we are about to supersede),
# so Δ/alerts always compare against a real prior reading, never against a mid-write row for this same sweep.
# PREV = last COMPLETE row for a DIFFERENT tick AND the SAME corpus (f13; missing⇒265). Same-corpus only, so
# the slope never diffs a CrUX sweep against a 265 sweep (different site set = the metric-swap lie).
PREV=""; [ -f "$LEDGER" ] && PREV=$(grep -vE '^(iso|#)' "$LEDGER" 2>/dev/null | awk -F'\t' -v t="$TICK" -v c="$CORPUS" '{ rc=($13==""?"265":$13) } $2!=t && rc==c' | tail -1)
p_iso=$(echo "$PREV" | cut -f1); p_tick=$(echo "$PREV" | cut -f2); p_scored=$(echo "$PREV" | cut -f4); p_ge=$(echo "$PREV" | cut -f5); p_sh=$(echo "$PREV" | cut -f7); p_excl=$(echo "$PREV" | cut -f10); p_inscope=$(echo "$PREV" | cut -f11); p_inpass=$(echo "$PREV" | cut -f12)

# ── SETTLED / COMPLETE guard (the mid-write-partial fix) ──────────────────────────────────────────────────
# The instrument appends one row per site over a long, chunked run (each chunk under `timeout … manuk-wpt
# fidelity`). Reading it early banks a MID-WRITE PARTIAL — that is how an 8-site 0.0%% row landed in the
# ledger while a 265-site sweep was in flight. Bank ONLY when the sweep is finished:
#   LIVE     — a manuk-wpt fidelity/oracle process is running ⇒ still being written ⇒ NOT settled.
#   COMPLETE — row count ≥80%% of the expected corpus (max historical sites, floor 200), so a between-chunk
#              lull or a sweep that died early is never mistaken for a finished run.
# EXPECT is CORPUS-RELATIVE (a fixed 265 breaks when the driving corpus becomes the ~200-site CrUX trend or
# the 400-site full cert corpus): expected = the detected corpus's own size (for CrUX, whichever of the
# 200-trend / 400-full file the sweep's site count is closest to), so "complete" means ~all of ITS corpus.
if [ "$CORPUS" = "crux" ]; then
  et=$(grep -vcE '^#|^$' docs/bench/corpus-crux-trend.txt 2>/dev/null); [ -z "$et" ] && et=200
  ef=$(grep -vcE '^#' docs/bench/corpus-v2.tsv 2>/dev/null); [ -z "$ef" ] && ef=400
  dt=$(( SITES>et ? SITES-et : et-SITES )); df=$(( SITES>ef ? SITES-ef : ef-SITES ))
  EXPECT=$et; [ "$df" -lt "$dt" ] && EXPECT=$ef
else
  EXPECT=$(grep -vcE '^#|^$' docs/bench/oracle-corpus.txt 2>/dev/null); [ -z "$EXPECT" ] && EXPECT=265
fi
[ "${EXPECT:-0}" -lt 100 ] && EXPECT=200
MINROWS=$(( EXPECT * 80 / 100 ))
SWEEP_LIVE=0; { pgrep -f 'manuk-wpt fidelity' >/dev/null 2>&1 || pgrep -f 'manuk-wpt oracle' >/dev/null 2>&1; } && SWEEP_LIVE=1
SETTLED=1; { [ "$SWEEP_LIVE" = 1 ] || [ "${SITES:-0}" -lt "$MINROWS" ]; } && SETTLED=0

# ── record-if-new (by sweep tick) ──────────────────────────────────────────────────────────────────────
if [ "$MODE" != "--check" ]; then
  [ -f "$LEDGER" ] || printf '# rebuilt-instrument schema (t706+); shape=parent-relative, NOT the old absolute placement (6.4 pre-2026-07-28 row is a DIFFERENT metric, do not diff)\n# f12 inscope_pass_pct = shape-only (kept for continuity). f13 corpus. f14 jarring_clean_pct. f15 m1_pass_pct = THE M1 GATE (shape>=0.75 AND jarring-clean, DAILY-DRIVER-CERTIFICATION.md §4). Phase-0 M1 exit = f15 >= 95. slope only diffs SAME corpus.\niso_sweep\tsweep_tick\tsites\tscored\tshape_ge0.75\tshape_ge0.75_pct\tshape_mean\tcov_mean\tsource\texcluded\tinscope\tinscope_pass_pct\tcorpus\tjarring_clean_pct\tm1_pass_pct\n' > "$LEDGER"
  EXIST_SITES=$(awk -F'\t' -v t="$TICK" '$2==t{print $3+0; exit}' "$LEDGER")
  ROW=$(printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s' "$SWEEP_ISO" "$TICK" "$SITES" "$SCORED" "$GE75" "$GE75PCT" "$SHAPEM" "$COVM" "$SRC" "$EXCL" "$INSCOPE" "$INPASS" "$CORPUS" "$JCPCT" "$M1PCT")
  if [ "$SETTLED" != 1 ]; then
    echo "fidelity-progress: sweep $TICK NOT settled (live=$SWEEP_LIVE, $SITES/$EXPECT sites) — not banking a mid-write partial"
  elif [ -z "$EXIST_SITES" ]; then
    printf '%s\n' "$ROW" >> "$LEDGER"; echo "recorded sweep $TICK ($SWEEP_ISO, $SITES sites)"
  elif [ "${EXIST_SITES:-0}" -lt "$MINROWS" ]; then
    # a PARTIAL row for this tick was banked earlier (the mid-write bug) → replace it with the complete run
    awk -F'\t' -v t="$TICK" '($1 ~ /^#/ || $1=="iso_sweep" || $2!=t)' "$LEDGER" > "$LEDGER.tmp" && mv "$LEDGER.tmp" "$LEDGER"
    printf '%s\n' "$ROW" >> "$LEDGER"; echo "superseded partial $TICK ($EXIST_SITES sites) with complete sweep ($SITES sites)"
  fi
fi

# ── verdict / alerts ───────────────────────────────────────────────────────────────────────────────────
alerts=""
AGE_DAYS=$(( ( NOW_EPOCH - SRC_EPOCH ) / 86400 ))
[ "$AGE_DAYS" -ge 3 ] && [ "$SWEEP_LIVE" != 1 ] && alerts="${alerts}STALE-SWEEP: newest banked corpus sweep ($TICK) is ${AGE_DAYS}d old — agent should run a fresh full sweep so the trend stays live\n"
# Contaminated triage sweeps (--jobs>2) legitimately show fewer scored sites; the banner already explains it,
# so DO NOT fire the regression/trap alerts on them (they would false-alarm ops-check --check every heartbeat
# until the next clean sweep). The contamination is honored, not hidden — it is printed as its own banner.
if [ -n "$PREV" ] && [ "$SETTLED" = 1 ] && [ "$CONTAM_SWEEP" != 1 ]; then
  awk "BEGIN{exit !($SCORED < ${p_scored:-0})}" && alerts="${alerts}SCORABILITY-REGRESSED: scored ${p_scored} -> ${SCORED} (fewer sites measurable — investigate, not progress)\n"
  # denominator trap: shape_mean up while scored down => a hard site dropped out, not real improvement
  if awk "BEGIN{exit !($SHAPEM > ${p_sh:-0} && $SCORED < ${p_scored:-0})}"; then
    alerts="${alerts}DENOMINATOR-TRAP: shape_mean ${p_sh}->${SHAPEM} ROSE while scored ${p_scored}->${SCORED} FELL — likely a hard site dropped out; the gain is NOT real\n"
  fi
  # EXCLUDED must stay capped (cert §3): a rising exclusion rate hides real failures as "unreachable" and
  # shrinks the denominator the pass-rate is computed over — a silent way to fake progress.
  if [ -n "${p_excl:-}" ] && awk "BEGIN{exit !($EXCL > ${p_excl:-0} + 3)}"; then
    alerts="${alerts}EXCLUDED-RISING: excluded ${p_excl}->${EXCL} (+$((EXCL - p_excl))) — the fetch path or corpus is degrading, OR real failures are being reclassified as unreachable; audit the new exclusions\n"
  fi
fi

if [ "$MODE" = "--check" ]; then [ -n "$alerts" ] && printf "%b" "$alerts"; exit 0; fi

TARGET=$(awk "BEGIN{printf \"%d\", int(0.95*$INSCOPE + 0.999)}")
NEED=$(( TARGET - GE75 )); [ "$NEED" -lt 0 ] && NEED=0
echo "── FIDELITY PROGRESS (rebuilt instrument · sweep $TICK · corpus=${CORPUS} · $SWEEP_ISO · ${AGE_DAYS}d old) ─────────"
[ "$SETTLED" != 1 ] && printf "  ⏳ SWEEP IN PROGRESS: %s/%s sites so far (live=%s) — numbers below are a PARTIAL, NOT banked until complete\n" "$SITES" "$EXPECT" "$SWEEP_LIVE"
[ "$CONTAM_SWEEP" = 1 ] && printf "  🚫 THIS SWEEP (%s) IS FLAGGED CONTAMINATED in the ledger (a --jobs>2 triage run — scorability lost to wall-clock contention, geometry intact). DO NOT read it as a burndown point; the slope below is SUPPRESSED. The last HONEST point is the last non-contaminated ledger row.\n" "$TICK"
[ -z "$PREV" ] && [ "$SETTLED" = 1 ] && printf "  🆕 NEW %s-CORPUS BASELINE — no same-corpus prior sweep, so NO slope this time (a different site set is not diffable; the slope resumes at the next %s sweep)\n" "$CORPUS" "$CORPUS"
M1_NEED=$(( TARGET - ${M1CNT:-0} )); [ "$M1_NEED" -lt 0 ] && M1_NEED=0
# ── SCORABILITY CEILING (PHASE0-MEASUREMENT-SYSTEM.md §1,§3): the binding sub-constraint the shape burndown
# was blind to. A site that does not render (render-failed/shell-only/timeout/css-starved — mostly boot-halting
# JS throws) counts as an M1 FAIL and cannot pass until it first renders. So the HARD CAP on M1 is scored/inscope
# — even if every scored site passed perfectly, M1 = scored/inscope. This is the FUNCTION leg (M2) surfacing as
# M1's ceiling: killing boot throws raises this cap AND builds the function cert. Attack it FIRST. Break the
# unscored reasons into a worklist so the loop ranks throw-killers, not just shape primitives.
SC_CEIL=$(awk "BEGIN{printf \"%.1f\", ($INSCOPE>0?100*$SCORED/$INSCOPE:0)}")
UNSCORED=$(( INSCOPE - SCORED ))
read -r U_RENDER U_SHELL U_THIN U_TIMEOUT U_CSS U_OTHER < <(awk -F'\t' -v TOL=2 '
  NR==1{next}
  { r=$9; gsub(/[ \t]/,"",r)
    if(r ~ /^bot-wall/ || r=="probe-blocked" || r=="unreachable" || r=="http-404" || r=="http-503" || r ~ /^empty-/) next
    if(r=="") next                                     # scored — not in the ceiling
    if(r ~ /render-fail|crash|panic/) rf++
    else if(r ~ /shell/) sh++
    else if(r ~ /thin|overlap-too|too-few/) th++
    else if(r ~ /timeout|timed-out|slow/) to++
    else if(r ~ /css-star|no-css|css-fail/) cs++
    else ot++ }
  END{ printf "%d %d %d %d %d %d", rf+0, sh+0, th+0, to+0, cs+0, ot+0 }' "$SRC")
printf "  🧱 SCORABILITY CEILING: %s/%s in-scope sites RENDER = %s%% — M1 is CAPPED here (function leg; PHASE0-MEASUREMENT-SYSTEM.md)\n" \
  "$SCORED" "$INSCOPE" "$SC_CEIL"
if [ "$UNSCORED" -gt 0 ]; then
  printf "     %s unscored = M1 fails until they render → throw-killer worklist: render-fail %s · shell-only %s · thin-overlap %s(booted-but-thin) · timeout %s · css-starved %s · other %s\n" \
    "$UNSCORED" "$U_RENDER" "$U_SHELL" "$U_THIN" "$U_TIMEOUT" "$U_CSS" "$U_OTHER"
  # ⚠ thin-overlap is NOT (measurement) when coverage is low — t777 showed all 25 at cov<0.2 (booted but
  # rendered <20% of Chrome's DOM). It is the NEXT engine gap AFTER the boot throw: throw-killers stop the
  # crash, the page boots, then a deeper gap leaves it blank/thin. That is where scored-count actually moves.
  [ "${U_THIN:-0}" -gt 0 ] && printf "       ↳ thin-overlap = booted-but-thin (ENGINE, not measurement, when cov<0.5): the crash is fixed but the app didn't populate the DOM — the chain's NEXT throw/gap. THIS is the current binding constraint on scored-count.\n"
fi
printf "  ⭐ M1 GATE (shape>=0.75 AND jarring-clean · the TRUE visual bar): %s/%s = %s%%   TARGET 95%% = %s sites  →  NEED +%s\n" \
  "${M1CNT:-0}" "$INSCOPE" "${M1PCT:-0}" "$TARGET" "$M1_NEED"
printf "     ├─ shape>=0.75: %s/%s = %s%%      └─ jarring-clean (no overlap/h-overflow/reorder/dead-target, TOL2): %s/%s = %s%%\n" \
  "$GE75" "$INSCOPE" "$INPASS" "${JARCLEAN:-0}" "$INSCOPE" "${JCPCT:-0}"
printf "     EXCLUDED (bot-wall/unreachable, watched·capped): %s/%s = %s%%   ·   scored %s/%s   shape_mean=%s%%  cov_mean=%s%%\n" \
  "$EXCL" "$SITES" "$(awk "BEGIN{printf \"%.0f\",100*$EXCL/$SITES}")" "$SCORED" "$INSCOPE" "$SHAPEM" "$COVM"
if [ -n "$PREV" ] && [ "$SETTLED" = 1 ] && [ -n "${p_inpass:-}" ] && [ "$CONTAM_SWEEP" != 1 ]; then
  DPP=$(awk "BEGIN{printf \"%+.1f\", $INPASS - ${p_inpass:-0}}")
  printf "  Δ vs %s: IN-SCOPE PASS %s%%→%s%% (%s pts) · scored %s→%s · excluded %s→%s\n" \
    "$p_tick" "$p_inpass" "$INPASS" "$DPP" "$p_scored" "$SCORED" "${p_excl:-?}" "$EXCL"
  # burndown slope → sweeps-to-95% (the FINITENESS readout the plan hinges on)
  awk "BEGIN{ d=$INPASS-${p_inpass:-0}; if(d>0.05){ printf \"  BURNDOWN: +%.1f pts/sweep → ~%d more sweeps to 95%% at this rate\n\", d, int((95-$INPASS)/d + 0.999) } else if(d<=0){ print \"  BURNDOWN: flat/negative — the shape burndown is NOT moving; see PHASE0-RENDER-BURNDOWN.md\" } }"
fi
if [ -n "$alerts" ]; then printf "  ⚠ %b" "$alerts"; else echo "  ✓ no trap/regression/staleness/exclusion flag"; fi
echo "  last ${LEDGER} rows:"; tail -3 "$LEDGER" 2>/dev/null | sed 's/^/    /'
exit 0
