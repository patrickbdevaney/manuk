#!/usr/bin/env bash
# ── LEVER BOARD — the biggest-impact-first work list + progress tally. The observer owns this; the agent
# CONSULTS it to pick the next tick so it attacks the broadest-impact mechanism first, not a narrow win.
#
# Ranks reachable areas by FAILING subtests, split by FLIP-RATE class: HIGH-FLIP areas (DOM/CSSOM/API/
# selector/parsing surfaces) yield many green subtests per fix; LAYOUT areas (flex/grid/sizing/…) are a
# multi-assertion slog (one fix flips ~nothing) — big in raw count but low yield. Exhaust high-flip mass
# FIRST. The encoding tail is out of v1 scope (V1-SCOPE.md) and excluded from the reachable tally.
#   usage: scripts/lever-board.sh
set -uo pipefail
cd "$(dirname "$0")/.."
A=docs/loop/WPT-AREAS.tsv
B=$'\033[1m'; C=$'\033[36m'; G=$'\033[32m'; Y=$'\033[33m'; O=$'\033[0m'
TARGET=83   # daily-driver MVP DIAGNOSTIC: reachable (non-tail) pass-rate goal. Real bar = real-site drivability.

awk -F'\t' -v B="$B" -v C="$C" -v G="$G" -v Y="$Y" -v O="$O" -v TGT="$TARGET" '
  NR>1 && $1!="TOTAL" && $1!="encoding" {
    p+=$2; t+=$3
    fail=$3-$2
    if (fail>0) { af[$1]=fail; ap[$1]=$4 }
  }
  END {
    pct = t>0 ? 100*p/t : 0
    need = TGT/100*t - p
    printf "%s══ PROGRESS TALLY ══%s\n", B, O
    printf "  reachable (excl encoding tail): %s%d / %d = %.1f%%%s\n", G, p, t, pct, O
    printf "  daily-driver MVP diagnostic:    %d%% reachable  →  %s%d subtests to go%s\n", TGT, Y, (need>0?need:0), O
    printf "\n%s══ LEVER BOARD — attack top-down; HIGH-FLIP mass before LAYOUT slog ══%s\n", B, O
    printf "  %-22s %8s %7s  %s\n", "area", "failing", "pass%", "class"
    # sort areas by failing desc (simple insertion into an index array)
    n=0; for (k in af) { key[n++]=k }
    for (i=0;i<n;i++) for (j=i+1;j<n;j++) if (af[key[j]]>af[key[i]]) { tmp=key[i]; key[i]=key[j]; key[j]=tmp }
    for (i=0;i<n;i++) {
      a=key[i]
      cls = (a ~ /flex|grid|sizing|align|overflow|position|display|contain|multicol|break|masking|transform/) ? "LAYOUT-slog (low flip)" \
          : (a ~ /^dom$|html\/dom|selectors|domparsing|cssom|css-values|css-color|css-fonts|css-ui/) ? "HIGH-FLIP surface" : "mixed"
      col = (cls ~ /HIGH/) ? C : Y
      printf "  %s%-22s %8d %6.1f%%  %s%s\n", col, a, af[a], ap[a], cls, O
    }
  }' "$A"
printf "\n%sHOW TO USE (agent):%s take the top HIGH-FLIP row; run \`manuk-wpt wpt <area> --show-failures\` for its\n" "$B" "$O"
printf "  failure-message histogram — the top mechanism there IS the tick. Prefer a fix that flips THOUSANDS\n"
printf "  over a narrow +N API. Deprioritise LAYOUT-slog rows until the high-flip mass is exhausted.\n"
