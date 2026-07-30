#!/usr/bin/env bash
# ── PHASE-0 COMPLETION MILESTONES — the ordered, mechanical state machine to Phase-0 done.
#
# WHY THIS EXISTS. The path to Phase-0 was DECIDED by the owner (2026-07-29) as three ORDERED milestones,
# but a decision that lives only in prose drifts — this repo's board history is full of once-authoritative
# blocks that went stale. This makes the sequence DURABLE and SELF-REPORTING: it computes which milestone
# we are in from observable signals (not memory), reports the distance to its gate and the slope toward it,
# and is surfaced every observer heartbeat (ops-check) so it survives agent relaunches AND observer context
# resets. It NEVER gates a tick (always exits 0). Truth computed, not asserted.
#
# THE THREE ORDERED GATES (each must clear before the next is worked; this ORDER is the locked decision):
#   M1  v1 RENDER      — shape>=0.75 on >=95% of the IN-SCOPE 265 corpus   (FIDELITY-PROGRESS.tsv f12 >= 95)
#   M2  v1 FUNCTION    — per-site render∧function >=95% on the 265 corpus  (the BiDi function leg, cert §4)
#   M3  v2 RE-CERT     — render∧function >=95% on the REPRESENTATIVE CrUX/Tranco corpus-v2 (docs/bench/corpus-v2.tsv)
#   → then the v1.0.0 release trigger (memory: v1-release-trigger) and Phase 1.
#
# THE ADVANCE CONTRACT (how a milestone auto-detects as started/done — the agent produces these artefacts):
#   M2 done-signal: docs/loop/FUNCTION-CERT.tsv exists with an overall pass >= 95 (see its own header for schema).
#   M3 done-signal: a corpus-v2 sweep — docs/loop/SWEEP-v2-t<NNN>-rows.tsv (or a row tagged corpus=v2) at >=95.
# Until those artefacts exist, the milestone reports "not started" — which is honest, not a failure.
#
#   scripts/phase0-milestones.sh            # full rollup
#   scripts/phase0-milestones.sh --oneline  # one line for ops-check / heartbeat
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; C=$'\033[36m'; R=$'\033[31m'; O=$'\033[0m'
MODE="${1:-full}"

# ── M1 signal: last COMPLETE fidelity row (skip mid-write partials: sites >= 80% of the max sites seen) ───
read -r M1_TICK M1_PASS M1_INSCOPE M1_GE75 M1_PREVPASS < <(awk -F'\t' '
  $1 ~ /^#/ || $1=="iso_sweep" || $1=="" {next}
  { if($3+0>maxs) maxs=$3+0; rows[NR]=$0 }
  END{
    thr = (maxs>200?maxs:265)*0.8
    last=""; prev=""
    for(i=1;i<=NR;i++){ if(!(i in rows)) continue; split(rows[i],f,"\t"); if(f[3]+0>=thr){ prev=last; last=rows[i] } }
    if(last!=""){ split(last,L,"\t"); pp=""; if(prev!=""){ split(prev,P,"\t"); pp=P[12] }
      printf "%s %s %s %s %s", L[2], L[12], L[11], L[5], (pp==""?"NA":pp) }
    else printf "none 0 0 0 NA"
  }' "$LEDGER" 2>/dev/null)

# ── M2 / M3 done-signals (auto-detect the agent's artefacts) ─────────────────────────────────────────────
m2_pass=""; [ -f docs/loop/FUNCTION-CERT.tsv ] && m2_pass=$(awk -F'\t' '/overall|OVERALL|pass_pct/{for(i=1;i<=NF;i++) if($i+0>0 && $i+0<=100) v=$i} END{print v+0}' docs/loop/FUNCTION-CERT.tsv 2>/dev/null)
m3_src=$(ls -t docs/loop/SWEEP-v2-t*-rows.tsv 2>/dev/null | head -1)
m3_pass=""; [ -n "$m3_src" ] && m3_pass=$(awk -F'\t' 'NR==1{next}{s++; r=$9; gsub(/[ \t]/,"",r); if(r=="") {sc++; if($3>=0.75) g++}} END{if(s>0) printf "%.1f", 100*g/(s - (s-sc<0?0:0))}' "$m3_src" 2>/dev/null)

# thresholds
ge() { awk "BEGIN{exit !(($1+0) >= ($2+0))}"; }
M1_DONE=0; ge "${M1_PASS:-0}" 95 && M1_DONE=1
M2_DONE=0; [ -n "$m2_pass" ] && ge "${m2_pass:-0}" 95 && M2_DONE=1
M3_DONE=0; [ -n "$m3_pass" ] && ge "${m3_pass:-0}" 95 && M3_DONE=1

# current milestone = first unmet, in order
if   [ "$M1_DONE" != 1 ]; then CUR="M1"
elif [ "$M2_DONE" != 1 ]; then CUR="M2"
elif [ "$M3_DONE" != 1 ]; then CUR="M3"
else CUR="DONE"; fi

# M1 arithmetic
TARGET=$(awk "BEGIN{printf \"%d\", int(0.95*${M1_INSCOPE:-209}+0.999)}")
NEED=$(( TARGET - ${M1_GE75:-0} )); [ "$NEED" -lt 0 ] && NEED=0
SLOPE_TXT="(need a 2nd complete sweep for a slope)"
if [ "${M1_PREVPASS:-NA}" != "NA" ]; then
  SLOPE_TXT=$(awk "BEGIN{d=${M1_PASS:-0}-${M1_PREVPASS:-0}; if(d>0.05) printf \"+%.1f pts/sweep → ~%d sweeps to 95%%\", d, int((95-${M1_PASS:-0})/d+0.999); else printf \"%+.1f pts/sweep (FLAT/NEG — see PHASE0-RENDER-BURNDOWN.md)\", d}")
fi

_st(){ [ "$1" = 1 ] && printf "%s✓done%s" "$G" "$O" || { [ "$2" = "$CUR" ] && printf "%s◀ CURRENT%s" "$Y$B" "$O" || printf "%s· pending%s" "$C" "$O"; }; }

if [ "$MODE" = "--oneline" ]; then
  case "$CUR" in
    M1) printf "PHASE-0 ▸ M1 v1-render %s%%→95%% (need +%s, %s)  [M2 fn, M3 v2 pending]\n" "${M1_PASS:-?}" "$NEED" "$SLOPE_TXT" ;;
    M2) printf "PHASE-0 ▸ M1 render DONE ✓ ▸ M2 v1-FUNCTION leg %s  [M3 v2 pending]\n" "${m2_pass:-not-started}" ;;
    M3) printf "PHASE-0 ▸ M1✓ M2✓ ▸ M3 v2 re-cert %s  → then v1.0.0\n" "${m3_pass:-not-started}" ;;
    DONE) printf "PHASE-0 ▸ ALL 3 MILESTONES MET → fire v1.0.0 release trigger (see memory v1-release-trigger)\n" ;;
  esac
  exit 0
fi

echo "${B}══ PHASE-0 COMPLETION — 3 ordered milestones (owner-locked 2026-07-29) ══${O}"
printf "  %sM1%s v1 RENDER  shape>=0.75 on >=95%% of in-scope 265   %s\n" "$B" "$O" "$(_st $M1_DONE M1)"
printf "       now %s%s%%%s  (%s/%s in-scope · target %s · need +%s)   %s\n" "$B" "${M1_PASS:-?}" "$O" "${M1_GE75:-?}" "${M1_INSCOPE:-?}" "$TARGET" "$NEED" "$SLOPE_TXT"
printf "       source: last complete sweep %s · ledger %s\n" "${M1_TICK:-none}" "$LEDGER"
printf "  %sM2%s v1 FUNCTION  per-site render∧function >=95%% on 265   %s\n" "$B" "$O" "$(_st $M2_DONE M2)"
printf "       %s  (artefact: docs/loop/FUNCTION-CERT.tsv — the BiDi function leg, cert §4; build AFTER M1)\n" "$([ -n "$m2_pass" ] && echo "now ${m2_pass}%" || echo "not started — leg unbuilt")"
printf "  %sM3%s v2 RE-CERT  render∧function >=95%% on CrUX/Tranco corpus-v2   %s\n" "$B" "$O" "$(_st $M3_DONE M3)"
printf "       %s  (corpus docs/bench/corpus-v2.tsv EXISTS; artefact: docs/loop/SWEEP-v2-t<NNN>-rows.tsv; build AFTER M2)\n" "$([ -n "$m3_pass" ] && echo "now ${m3_pass}%" || echo "not started — corpus-v2 not yet swept")"
printf "  %s→%s then v1.0.0 release trigger (memory: v1-release-trigger) + Phase 1\n" "$B" "$O"
echo "  plan: docs/loop/PHASE0-RENDER-BURNDOWN.md (M1) · PHASE0-BOUNDED-REMAINDER.md (M2 worklist) · DAILY-DRIVER-CERTIFICATION.md (authority)"
exit 0
