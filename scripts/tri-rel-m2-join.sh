#!/usr/bin/env bash
# Side-by-side M2 drive rate per reachable site: debug (old default) vs release (new default).
# R0.4.a.c.a.b.a.a.a.a.a.a.b. Output columns (8):
#   url debug_rate release_rate debug_tag release_tag release_load1 debug_chrome_ver release_chrome_ver
#   usage: tri-rel-m2-join.sh <debug_sweep.tsv> <release_sweep.tsv> <release.load.tsv> > joined.tsv
# Rows = the sites the release sweep drove (m1 reachable). A site with no debug row prints debug fields as "-".
set -uo pipefail
DBG="$1"; REL="$2"; LOAD="$3"
awk -F'\t' '
  FILENAME==dbg { if (FNR>1) { dt[$2]=$8; dr[$2]=$9; dc[$2]=$20 } ; next }
  FILENAME==ld  { ll[$1]=$2; next }
  FNR>1 && $8!="SKIPPED" {
    d_tag=(($2 in dt)?dt[$2]:"-"); d_rate=(($2 in dr)?dr[$2]:"-"); d_chr=(($2 in dc)?dc[$2]:"-")
    l=(($2 in ll)?ll[$2]:"")
    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n", $2, d_rate, $9, d_tag, $8, l, d_chr, $20
  }' dbg="$DBG" ld="$LOAD" "$DBG" "$LOAD" "$REL"
