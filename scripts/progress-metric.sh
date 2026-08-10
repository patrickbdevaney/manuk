#!/usr/bin/env bash
# progress-metric.sh — a CONTINUOUS, drift-robust steering gauge for the fidelity sweep.
#
# WHY: the exit gate is a BINARY per-site conjunction — a site scores 1 iff shape>=0.75 AND
# jarring-clean, else 0. A site at shape 0.74 with one overlap scores the same as a blank page, so
# the gate CANNOT see incremental engine progress (+825 suite tests moved it by zero; an anchor moved
# 12.7 pts, the corpus moved zero). This tool leaves the EXIT BAR UNTOUCHED and adds the signal the loop
# was missing: a per-site score in [0,1] that is MONOTONIC — it rises whenever shape improves OR a jarring
# flag is cleared OR a new site starts rendering — so the loop can be STEERED by something that moves.
#
# It changes nothing about the engine, the sweep, or the pass/fail definition. It only READS the existing
# SWEEP-t*-rows.tsv files. Read-only, always exits 0 (a gauge cannot brick a tick).
#
# ⚠ THIS IS A PROGRESS/STEERING METRIC, NOT A NEW EXIT BAR. The daily-driver exit stays exactly as the
# certification defines it. Never report this number as "the milestone" and never move the exit to make it.
#
# Row schema (10 cols, tab-sep): name coverage shape h_overflow overlap reading_order dead_target
# shape_n reason instrument.  IN-SCOPE = reason is NOT an accepted exclusion (bot-wall/unreachable/
# probe-blocked/http-4xx-5xx/empty). SCORED (genuine) = shape!="-" AND reason=="". A site can be in-scope
# but unscored (timeout/shell/thin/divergence, or shape "-") — that is a scorability miss, scored as 0.
set -euo pipefail
cd "$(dirname "$0")/.."

NEW="${1:-$(ls -t docs/loop/SWEEP-t*-rows.tsv 2>/dev/null | sed -n 1p)}"
OLD="${2:-$(ls -t docs/loop/SWEEP-t*-rows.tsv 2>/dev/null | sed -n 2p)}"
[ -n "${NEW:-}" ] && [ -f "$NEW" ] || { echo "progress-metric: no SWEEP-t*-rows.tsv found"; exit 0; }

# W = jarring weight: flag DENSITY (offending nodes / total nodes) times W, as a soft penalty.
# W=4 → a page 25% of whose nodes carry a flag gets jarring_factor 0.5. Density normalises page size,
# so 376 inversions on a 2000-node page is not crushed to zero the way a raw count would be.
W=4

echo "── PROGRESS METRIC  ←  $(basename "$NEW")${OLD:+   (vs $(basename "$OLD"))}"
echo "   Continuous steering gauge. The EXIT BAR is unchanged; this only makes progress VISIBLE."
echo

awk -F'\t' -v W="$W" -v OLD="${OLD:-}" '
  function clamp01(x){ return x<0?0:(x>1?1:x) }
  function site_score(shape, ho, ov, ro, dt, n,    fd, jf){
    n = (n>0)?n:1
    fd = (ho+ov+ro+dt)/n            # flag density: offending nodes / total nodes
    jf = 1/(1 + fd*W)              # jarring factor in (0,1], =1 when clean
    return clamp01(shape)*jf
  }
  function excluded(reason){ return (reason ~ /bot-wall|unreachable|probe-blocked|^http-|empty-/) }
  BEGIN{ pass_shape_th=0.75 }

  # ---- pass 1: OLD file into arrays (if present) ----
  FNR==NR && OLD!="" {
    if($1~/^#/||$1=="") next
    if($3!="-" && $9==""){ o_shape[$1]=$3+0; o_score[$1]=site_score($3+0,$4,$5,$6,$7,$8+0) }
    next
  }
  # ---- pass 2 (or pass 1 if no OLD): NEW file ----
  {
    if($1~/^#/||$1=="") next
    inscope = !excluded($9)
    if(inscope) N++                                  # in-scope denominator
    if($3!="-" && $9==""){                            # SCORED (genuine measurement)
      M++                                            # scored count
      s=$3+0; sc=site_score(s,$4,$5,$6,$7,$8+0)
      sum_scored += sc                               # for mean-over-scored
      if(inscope) sum_corpus += sc                   # for corpus mean (unscored in-scope = 0)
      if(inscope){
        if(s>=pass_shape_th) shape_pass++
        jf_clean = ($4+$5+$6+$7==0)
        if(jf_clean) jarring_clean++
        if(s>=pass_shape_th && jf_clean) m1++
      }
      if(OLD!="" && ($1 in o_shape)){                # common set
        n++; dS=s-o_shape[$1]; dC=sc-o_score[$1]
        sumdS+=dS; sumdC+=dC
        if(dS>0.02)upS++; else if(dS<-0.02)dnS++
        if(dC>0.02)upC++; else if(dC<-0.02)dnC++
      }
    }
  }
  END{
    printf "  Population (in-scope excludes bot-wall/unreachable/http/empty/probe-blocked):\n"
    printf "    in-scope=%d  scored=%d  scorability=%.1f%%\n\n", N, M, (N? 100*M/N:0)
    printf "  Binary gates (count / in-scope):\n"
    printf "    shape-only  (shape>=0.75)        = %d/%d = %.1f%%   (the ledger f12 headline)\n", shape_pass,N,(N?100*shape_pass/N:0)
    printf "    jarring-clean                    = %d/%d = %.1f%%\n", jarring_clean,N,(N?100*jarring_clean/N:0)
    printf "    M1 conjunction (shape AND clean) = %d/%d = %.1f%%   (trap-free; the RENDER bar)\n\n", m1,N,(N?100*m1/N:0)
    printf "  Continuous fidelity in [0,1] (monotonic: rises on ANY shape gain / cleared flag / new render):\n"
    printf "    mean over SCORED sites           = %.4f   (quality of what renders)\n", (M? sum_scored/M:0)
    printf "  ⇒ CORPUS fidelity (unscored=0, /in-scope) = %.4f   ← single steering gauge (quality × scorability)\n", (N? sum_corpus/N:0)
    if(OLD!=""){
      printf "\n  Δ vs previous sweep (common set, n=%d sites scored in BOTH — drift-robust):\n", n
      printf "    mean Δshape       = %+.4f   (up>0.02: %d  down: %d)\n", (n?sumdS/n:0), upS, dnS
      printf "    mean Δsite_score  = %+.4f   (up>0.02: %d  down: %d)   ← the honest movement signal\n", (n?sumdC/n:0), upC, dnC
    }
    printf "\n  site_score = clamp01(shape) x 1/(1 + (Σflags/nodes)x%d).  Read-only; not an exit bar.\n", W
  }
' "${OLD:-$NEW}" "$NEW"
exit 0
