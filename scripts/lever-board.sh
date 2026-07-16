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
printf "\n%s══ DAILY-DRIVER PRIORITY LEVERS — research-informed; these OUTRANK the raw WPT-mass board above ══%s\n" "$B" "$O"
printf "  (full map + per-site deps + forecast method: %sdocs/wiki/lever-map.md%s)\n" "$C" "$O"
printf "  The board above ranks by WPT MASS and has a KNOWN BLIND SPOT: it hides cheap, boot-critical APIs whose\n"
printf "  absence BLANK-SCREENS whole SPAs. Weigh by daily-driver UNLOCK, not raw subtest count:\n"
printf "  %s⚡ BOOT-CRITICAL — do FIRST despite low WPT mass (cheap, high-gating):%s\n" "$G" "$O"
printf "     • IntersectionObserver/ResizeObserver  — x.com + Instagram feeds are DEAD without IO   (S, 402 WPT)\n"
printf "     • History / pushState                  — every SPA nav degrades to a full page reload  (S, ~1k WPT)\n"
printf "     • Proxy/Reflect + microtask/MessageChannel — Vue3 reactivity silently dead w/o Proxy    (S-M)\n"
printf "     • Fetch + ReadableStream body          — gates Next.js RSC navigation                  (M, ~19k WPT)\n"
printf "  %s⚡ STEP-FUNCTION — largest raw levers, fully daily-driver:%s\n" "$C" "$O"
printf "     • Flex/Grid intrinsic sizing (min/max-content; Taffy #204) — M-L, ~28k WPT, every site\n"
printf "     • Web fonts (@font-face / WOFF2)                           — M, 8.8k WPT, layout-metric fidelity\n"
printf "     • Shadow DOM + Custom Elements                             — L, 20k WPT, YouTube / web-components\n"
printf "  %sDEFER%s: MSE/media (bind GStreamer, don't build codecs), compositor scrolling, CSS containment.\n" "$Y" "$O"
printf "  %sDown-weight the near-done giants%s: dom (~99.8%%) + html/dom (~97.8%%) tail is LOW value — stop grinding it.\n" "$Y" "$O"
printf "  %sNEXT-3 (highest EV)%s: IntersectionObserver  ->  CSS intrinsic sizing  ->  Fetch streaming.\n" "$G" "$O"
printf "  %sFORECAST before building%s: cluster the failing subtests by ERROR-SIGNATURE — the biggest same-signature\n" "$B" "$O"
printf "     cluster means one fix flips them all (how tick-113 landed +10,249). See docs/wiki/lever-map.md #forecast.\n"
printf "\n%sHOW TO USE (agent):%s FIRST weigh the PRIORITY LEVERS above (daily-driver unlock + boot-critical SPA APIs).\n" "$B" "$O"
printf "  Then within the raw board, run \`manuk-wpt wpt <area> --show-failures\` for its failure histogram — the top\n"
printf "  mechanism there IS the tick. Prefer a fix that flips THOUSANDS or unblocks a boot-critical SPA API over a\n"
printf "  narrow +N. The raw board is a MASS view; the priority levers correct its daily-driver blind spot.\n"
