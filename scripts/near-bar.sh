#!/usr/bin/env bash
# near-bar.sh — the marginal-crossing ranker. THE plateau-breaker instrument.
#
# WHY: M1 sat at 15-18% for ~65 ticks while shape_mean rose — the signature of ranking on tag-MASS
# (CLUSTERS.md hit-count), which lifts the band and crosses ZERO sites. This ranks the opposite: sites
# that are ONE fix away from crossing the M1 bar (shape>=0.75 AND jarring-clean), so a tick converts a
# SITE instead of nudging an average. Rank the loop's work from here, not from tag frequency.
#
# INPUT: a per-site SWEEP-t*-rows.tsv (10 cols: name coverage shape h_overflow overlap reading_order
# dead_target shape_n reason instrument). A row is a REAL fidelity measurement IFF shape!="-" AND
# reason=="" — a scored row that still carries a reason (oracle-module-shell / thin-overlap / ...) is a
# shell the instrument treats as UNSCORED; that is throw-killer.sh's job, not this one.
#
# Read-only, always exits 0 (a ledger cannot brick a tick). Upgrades automatically: when the grind lands
# the mechanism columns (11+, displaced|mis-sized:axis~mag), each near-bar row shows {that key}.
set -euo pipefail
cd "$(dirname "$0")/.."
ROWS="${1:-$(ls -t docs/loop/SWEEP-t*-rows.tsv 2>/dev/null | head -1)}"
[ -n "${ROWS:-}" ] && [ -f "$ROWS" ] || { echo "near-bar: no SWEEP-t*-rows.tsv found"; exit 0; }

TAB=$(printf '\t')
gen() {
  awk -F'\t' '
    $1 ~ /^#/ || $1=="" { next }
    $3 == "-" || $9 != "" { next }                       # genuine fidelity measurements only
    { shape=$3+0; jf=$4+$5+$6+$7; sb=(shape<0.75)?1:0; blk=sb+jf; gap=0.75-shape;
      lbl=""; if(sb) lbl=sprintf("shape %.3f (+%.3f to bar)", shape, gap);
      if($4){lbl=lbl (lbl?", ":"") "h-overflow"}  if($5){lbl=lbl (lbl?", ":"") "overlap"}
      if($6){lbl=lbl (lbl?", ":"") "reading-order"}  if($7){lbl=lbl (lbl?", ":"") "dead-target"}
      mech=(NF>=11 && $11!="")?("  {" $11 "}"):"";
      if(blk>=1 && blk<=2) printf "%d\t%.4f\t%s\t%d\t%s%s\n", blk,(sb?gap:0.999),$1,$8,lbl,mech
    }' "$ROWS" | sort -t"$TAB" -k1,1n -k2,2n
}
pass=$(awk -F'\t' '$1!~/^#/ && $1!="" && $3!="-" && $9=="" {s=$3+0; if(s>=0.75 && ($4+$5+$6+$7)==0) p++} END{print p+0}' "$ROWS")

echo "── NEAR-BAR marginal-crossing worklist  ←  $ROWS"
echo "   M1 bar = shape>=0.75 AND jarring-clean.  'blockers' = independent fixes to cross; attack blockers=1 first."
echo
echo "  ONE FIX FROM M1 (blockers=1) — highest crossing yield:"
gen | awk -F'\t' '$1=="1"{printf "    %-30s n=%-5s need: %s\n",$3,$4,$5}'
echo
echo "  TWO FIXES FROM M1 (blockers=2) — second tier:"
gen | awk -F'\t' '$1=="2"{printf "    %-30s n=%-5s need: %s\n",$3,$4,$5}'
n1=$(gen | awk -F'\t' '$1=="1"' | wc -l | tr -d ' ')
n2=$(gen | awk -F'\t' '$1=="2"' | wc -l | tr -d ' ')
echo
echo "  summary: M1-passing=$pass · one-fix=$n1 · two-fix=$n2   →   clearing the $n1 one-fix sites is the fastest M1 gain."
echo "  DISCIPLINE: a shape fix is shared-measurement — prove each claimed crossing with scripts/old-binary-control.sh"
echo "  (same-hour, same-snapshot) before banking it; a single +0.02 is below the ~3.7pt live-site noise floor."
exit 0
