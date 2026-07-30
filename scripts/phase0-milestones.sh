#!/usr/bin/env bash
# ── PHASE-0 COMPLETION MILESTONES — the ordered, mechanical state machine to Phase-0 done.
#
# WHY THIS EXISTS. The path to Phase-0 was DECIDED by the owner, but a decision that lives only in prose
# drifts. This makes the sequence DURABLE + SELF-REPORTING: it computes which milestone we are in from
# observable signals (not memory), reports distance + slope, and is surfaced every observer heartbeat
# (ops-check) so it survives agent relaunches AND observer context resets. It NEVER gates a tick (exits 0).
#
# ⚠ REVISED 2026-07-30 (owner decision): drive the REPRESENTATIVE CrUX corpus FROM THE START, not "v1 then
# v2". This COLLAPSES the old 3 milestones (v1-render → v1-function → v2-recert) into TWO — because the
# corpus is representative from day one, there is no separate "re-cert on v2" step. Two ORDERED gates:
#   M1  RENDER    — shape>=0.75 on >=95% of the IN-SCOPE representative CrUX corpus (FIDELITY-PROGRESS f12).
#                   Driven on the ~200-site CrUX trend sample (docs/bench/corpus-crux-trend.txt); certified
#                   on the full 400 (docs/bench/corpus-v2.tsv). Throw-class FUNCTION killers are flagged
#                   during these sweeps (a touched API that throws) — a cheap early signal for M2.
#   M2  FUNCTION  — per-site render∧function >=95% on the representative corpus (the BiDi function leg,
#                   cert §4 Layer C-function; artefact docs/loop/FUNCTION-CERT.tsv). Built AFTER M1.
#   → then the v1.0.0 release trigger (memory: v1-release-trigger) and Phase 1.
#
#   scripts/phase0-milestones.sh            # full rollup
#   scripts/phase0-milestones.sh --oneline  # one line for ops-check / heartbeat
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LEDGER=docs/loop/FIDELITY-PROGRESS.tsv
B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; C=$'\033[36m'; R=$'\033[31m'; O=$'\033[0m'
MODE="${1:-full}"

# ── M1 signal: last COMPLETE fidelity row (skip mid-write partials: sites >= 80% of the max sites seen) ───
#    Also lift the row's CORPUS tag (f13; missing⇒265) so we can say WHICH corpus the render number is on.
# Every BANKED ledger row is already a settled+complete sweep (fidelity-progress guards that at write time,
# corpus-relative), so DO NOT re-apply a site-count threshold here — a fixed 265-based one wrongly excludes
# the 200-site CrUX sweeps. M1 = the LAST row; PREV = the most recent SAME-CORPUS row before it (slope only
# within a corpus — never diff CrUX against the 265).
# M1_PASS is the FULL gate = m1_pass_pct (f15, shape AND jarring); old rows (pre-jarring) fall back to shape
# (f12). Also lift shape (f12) and jarring-clean (f14) as the two components for display.
read -r M1_TICK M1_PASS M1_INSCOPE M1_SHAPEPCT M1_JARPCT M1_PREVPASS M1_CORPUS < <(awk -F'\t' '
  $1 ~ /^#/ || $1=="iso_sweep" || $1=="" {next}
  { rows[NR]=$0 }
  END{
    last=""; lastNR=0
    for(i=1;i<=NR;i++){ if(i in rows){ last=rows[i]; lastNR=i } }
    if(last==""){ printf "none 0 0 0 0 NA 265"; exit }
    split(last,L,"\t"); lc=(L[13]==""?"265":L[13])
    m1=(L[15]!=""?L[15]:L[12]); shp=L[12]; jar=(L[14]!=""?L[14]:"NA")
    prev=""
    for(i=1;i<lastNR;i++){ if(!(i in rows)) continue; split(rows[i],f,"\t"); if((f[13]==""?"265":f[13])==lc) prev=rows[i] }
    pp="NA"; if(prev!=""){ split(prev,P,"\t"); pp=(P[15]!=""?P[15]:P[12]) }
    printf "%s %s %s %s %s %s %s", L[2], m1, L[11], shp, jar, pp, lc
  }' "$LEDGER" 2>/dev/null)

# ── M2 done-signal (auto-detect the agent's artefact) ────────────────────────────────────────────────────
m2_pass=""; [ -f docs/loop/FUNCTION-CERT.tsv ] && m2_pass=$(awk -F'\t' '/overall|OVERALL|pass_pct/{for(i=1;i<=NF;i++) if($i+0>0 && $i+0<=100) v=$i} END{print v+0}' docs/loop/FUNCTION-CERT.tsv 2>/dev/null)

ge() { awk "BEGIN{exit !(($1+0) >= ($2+0))}"; }
M1_DONE=0; ge "${M1_PASS:-0}" 95 && M1_DONE=1
M2_DONE=0; [ -n "$m2_pass" ] && ge "${m2_pass:-0}" 95 && M2_DONE=1
if   [ "$M1_DONE" != 1 ]; then CUR="M1"
elif [ "$M2_DONE" != 1 ]; then CUR="M2"
else CUR="DONE"; fi

TARGET=$(awk "BEGIN{printf \"%d\", int(0.95*${M1_INSCOPE:-200}+0.999)}")
M1CNT=$(awk "BEGIN{printf \"%d\", (${M1_PASS:-0}+0)/100*(${M1_INSCOPE:-200}+0)+0.5}")
NEED=$(( TARGET - M1CNT )); [ "$NEED" -lt 0 ] && NEED=0
SLOPE_TXT="(need a 2nd same-corpus complete sweep for a slope)"
if [ "${M1_PREVPASS:-NA}" != "NA" ]; then
  SLOPE_TXT=$(awk "BEGIN{d=${M1_PASS:-0}-${M1_PREVPASS:-0}; if(d>0.05) printf \"+%.1f pts/sweep → ~%d sweeps to 95%%\", d, int((95-${M1_PASS:-0})/d+0.999); else printf \"%+.1f pts/sweep (FLAT/NEG — see PHASE0-RENDER-BURNDOWN.md)\", d}")
fi
# CrUX transition flag: the render target is the representative corpus; while the number is still on the 265,
# it is a PROXY that recalibrates (likely lower) at the first CrUX sweep.
CORPUS_NOTE=""; [ "${M1_CORPUS:-265}" = "265" ] && CORPUS_NOTE=" [still on 265 proxy — recalibrates at first CrUX sweep]"

_st(){ [ "$1" = 1 ] && printf "%s✓done%s" "$G" "$O" || { [ "$2" = "$CUR" ] && printf "%s◀ CURRENT%s" "$Y$B" "$O" || printf "%s· pending%s" "$C" "$O"; }; }

if [ "$MODE" = "--oneline" ]; then
  case "$CUR" in
    M1) printf "PHASE-0 ▸ M1 RENDER(corpus=%s) shape+jarring %s%%→95%% (need +%s, %s) [shape %s%% · jarring-clean %s%%]%s  [M2 fn pending]\n" "${M1_CORPUS}" "${M1_PASS:-?}" "$NEED" "$SLOPE_TXT" "${M1_SHAPEPCT:-?}" "${M1_JARPCT:-?}" "$CORPUS_NOTE" ;;
    M2) printf "PHASE-0 ▸ M1 RENDER done ✓ ▸ M2 FUNCTION leg %s  → then v1.0.0\n" "${m2_pass:-not-started}" ;;
    DONE) printf "PHASE-0 ▸ BOTH MILESTONES MET → fire v1.0.0 release trigger (memory v1-release-trigger)\n" ;;
  esac
  exit 0
fi

echo "${B}══ PHASE-0 COMPLETION — 2 ordered milestones · representative CrUX from the start (owner 2026-07-30) ══${O}"
printf "  %sM1%s RENDER  shape>=0.75 AND jarring-clean on >=95%% of in-scope representative CrUX (the TRUE visual bar)   %s\n" "$B" "$O" "$(_st $M1_DONE M1)"
printf "       now %s%s%%%s on corpus=%s  (%s/%s in-scope · target %s · need +%s)   %s%s\n" "$B" "${M1_PASS:-?}" "$O" "${M1_CORPUS}" "${M1CNT:-?}" "${M1_INSCOPE:-?}" "$TARGET" "$NEED" "$SLOPE_TXT" "$CORPUS_NOTE"
printf "       ├─ shape>=0.75: %s%%   └─ jarring-clean (no overlap/h-overflow/reorder/dead-target): %s%%  (both must clear 95%%)\n" "${M1_SHAPEPCT:-?}" "${M1_JARPCT:-?}"
printf "       drive: docs/bench/corpus-crux-trend.txt (~200, fast) · cert: docs/bench/corpus-v2.tsv (400) · last sweep %s\n" "${M1_TICK:-none}"
printf "  %sM2%s FUNCTION  per-site render∧function >=95%% on the representative corpus   %s\n" "$B" "$O" "$(_st $M2_DONE M2)"
printf "       %s  (artefact docs/loop/FUNCTION-CERT.tsv — BiDi leg, build AFTER M1; throw-class killers flagged during M1 sweeps as an early signal)\n" "$([ -n "$m2_pass" ] && echo "now ${m2_pass}%" || echo "not started — leg unbuilt")"
printf "  %s→%s then v1.0.0 release trigger (memory: v1-release-trigger) + Phase 1\n" "$B" "$O"
echo "  plan: docs/loop/PHASE0-RENDER-BURNDOWN.md (M1) · PHASE0-BOUNDED-REMAINDER.md (M2 worklist) · DAILY-DRIVER-CERTIFICATION.md (authority)"
exit 0
