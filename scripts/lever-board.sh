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

# ── WORKFLOW WARNING (observer, tick 134). This has HUNG the loop 3× and needed a manual kill each time:
R=$'\033[31m'
printf "%s⚠ NEVER poll with a pgrep-of-your-own-pattern wait loop%s — e.g. \`while pgrep -f \"cargo test X\"; do sleep; done\`\n" "$R$B" "$O"
printf "  or \`until ! pgrep -f \"scripts/tick.sh\"; do ...; done\`. The pattern string is in the WAIT LOOP'S OWN cmdline, so\n"
printf "  pgrep always matches itself, never returns empty, and the loop spins FOREVER even after the real job finished.\n"
printf "  Instead: run cargo/tick.sh/tests in the FOREGROUND and let them block. If the harness backgrounds a long command,\n"
printf "  wait on its OUTPUT-FILE CONTENT (e.g. \`grep -q 'VERIFY:' out\`), never on pgrep of a string your wait command contains.\n\n"

# ── PHASE MANDATE (observer, tick 138) — DAILY-DRIVER CAPABILITY, not raw WPT flip. ENFORCED this phase.
printf "%s🎯 PHASE MANDATE — build DAILY-DRIVER CAPABILITY, not WPT-flip count%s\n" "$R$B" "$O"
printf "  html/dom is reasonably done (~93%%). The daily-driver bottleneck is now RENDERING and MEDIA. Pick your tick\n"
printf "  from these two areas IN PRIORITY ORDER until each is 'good enough' to render/play real sites:\n"
printf "    %s(1) CSS LAYOUT%s — flexbox (~6%%), grid (~5%%), sizing (~12%%), position/values/overflow/transforms. Real sites\n" "$G$B" "$O"
printf "        render MISLAID-OUT without it. Flex/Grid INTRINSIC SIZING (min/max-content; Taffy #204) is the core lever.\n"
printf "    %s(2) MSE / MEDIA pipeline%s — currently ABSENT; video sites are dead. Build the MediaSource/SourceBuffer JS\n" "$G$B" "$O"
printf "        surface and BIND a media framework (GStreamer/FFmpeg) — do NOT hand-write codecs (lever-map.md §M).\n"
printf "  These are LOW WPT-flip but HIGH daily-driver capability — that is the POINT. A html/dom-flip tick does NOT\n"
printf "  advance the mandate; prefer a CSS-layout or media tick even at a lower +N. Goal: a good-enough render+media\n"
printf "  foundation for the agentic driving surface (with agent fallbacks where WPT coverage is thin), then a security\n"
printf "  sweep. Only after CSS layout + MSE are 'good enough' do we return to general WPT flip. See docs/wiki/lever-map.md.\n\n"

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
    printf "\n%s══ LEVER BOARD — PHASE: pick CSS-LAYOUT / media rows first (★), regardless of flip-rate ══%s\n", B, O
    printf "  %-22s %8s %7s  %s\n", "area", "failing", "pass%", "class"
    # sort areas by failing desc (simple insertion into an index array)
    n=0; for (k in af) { key[n++]=k }
    for (i=0;i<n;i++) for (j=i+1;j<n;j++) if (af[key[j]]>af[key[i]]) { tmp=key[i]; key[i]=key[j]; key[j]=tmp }
    for (i=0;i<n;i++) {
      a=key[i]
      cls = (a ~ /flex|grid|sizing|align|overflow|position|display|contain|multicol|break|masking|transform|css-values|css-color|css-backgrounds|css-ui/) ? "★ CSS-LAYOUT — DAILY-DRIVER (build now)" \
          : (a ~ /media|video|audio/) ? "★ MEDIA — DAILY-DRIVER (build now)" \
          : (a ~ /^dom$|html\/dom|selectors|domparsing|cssom|css-fonts/) ? "html/dom (reasonably done — deprioritise)" : "mixed"
      col = (cls ~ /DAILY-DRIVER/) ? G : Y
      printf "  %s%-22s %8d %6.1f%%  %s%s\n", col, a, af[a], ap[a], cls, O
    }
  }' "$A"
printf "\n%s══ DAILY-DRIVER PRIORITY LEVERS — research-informed; weigh ALONGSIDE the raw board (undervalued by raw count) ══%s\n" "$B" "$O"
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
printf "  %sMSE/MEDIA is now a PHASE PRIORITY (not deferred)%s — bind GStreamer/FFmpeg, don't hand-write codecs.\n" "$G" "$O"
printf "  %sDEFER (only these)%s: compositor-threaded scrolling, CSS containment/content-visibility (low value, high cost).\n" "$Y" "$O"
printf "  %sWeight by MANUK'S OWN pass-rate (the TALLY above), never Chrome's%s: an area near 100%% HERE is a done tail,\n" "$Y" "$O"
printf "     but an area still far from 100%% here (e.g. dom) has real high-flip gaps — keep grinding those. The research's\n"
printf "     'giant is done' notes are CHROME's numbers; MANUK's measured failing subtests are ground truth and win.\n"
printf "  %sNEXT this phase%s: CSS flex/grid INTRINSIC SIZING (min/max-content, Taffy #204)  ->  more CSS layout  ->  MSE media pipeline.\n" "$G" "$O"
printf "  %sFORECAST before building%s: cluster the failing subtests by ERROR-SIGNATURE — the biggest same-signature\n" "$B" "$O"
printf "     cluster means one fix flips them all (how tick-113 landed +10,249). See docs/wiki/lever-map.md #forecast.\n"
printf "\n%sHOW TO USE (agent):%s Obey the PHASE MANDATE at the top — pick a CSS-LAYOUT (★) or MEDIA tick, NOT html/dom flip.\n" "$B" "$O"
printf "  Run \`manuk-wpt wpt css/css-flexbox --show-failures\` (or css-grid/css-sizing) for the failure histogram; the top\n"
printf "  mechanism there IS the tick. A layout fix that makes real pages render correctly beats a bigger html/dom +N.\n"
printf "  For media: there is little WPT to chase — build the MediaSource/SourceBuffer surface + a GStreamer/FFmpeg bind,\n"
printf "  and gate it with a falsifiable capability test (a sample stream buffers + plays), not a WPT count.\n"
