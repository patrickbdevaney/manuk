#!/usr/bin/env bash
# wpt-leverage.sh — the SOLID-GROUND steering instrument the live-CrUX sweep can't be.
#
# WHY: the M1 fidelity sweep measures ~135 LIVE sites — a moving target. Its denominator drifts
# (in-scope 132↔135), scorability churns (78%↔84% with box load/timeouts), and a "pass" can un-pass
# when a live site changes under us. You cannot climb monotonically against a metric whose numerator
# AND denominator both move. Every browser that shipped (Servo 30→62→79%, Ladybird 79%) measures the
# same way instead: a FROZEN, deterministic WPT suite where a pass is permanent and the total is fixed.
#
# We already HAVE it: docs/loop/WPT-AREAS.tsv (written by the grind's own runner) is a stable
# per-area pass/total — solid ground. This tool ranks the remaining WPT failures by
#   LEVERAGE = (failing tests in the area) x (how much of the real web uses that area)
# so the loop works the primitives that cover the MOST of the internet FIRST, not the long tail.
# Real-web weights are from the HTTP Archive Web Almanac (CSS chapter, page-usage %).
#
# This is a STEERING/COVERAGE gauge, NOT the exit bar. Phase-0 still certifies against the CrUX
# M1/M2 daily-driver definition (owner-locked). WPT is how we CLIMB fast on solid ground; M1/M2 is
# how we CERTIFY done. They correlate because the same primitives (flex/grid/sizing/dom) gate both.
# Read-only; always exits 0.
set -euo pipefail
cd "$(dirname "$0")/.."
AREAS="${1:-docs/loop/WPT-AREAS.tsv}"
[ -f "$AREAS" ] || { echo "wpt-leverage: no $AREAS (run the WPT area sweep first)"; exit 0; }

echo "── WPT LEVERAGE  ←  $(basename "$AREAS")   (solid-ground steering; NOT the exit bar)"
echo "   LEVERAGE = usage x winnable-tests x room-to-grow x FLIP-RATE.  Work top rows first — densest M1/M2 per tick."
echo

awk -F'\t' '
  # Real-web usage weight per WPT area (0..1), ~= fraction of pages exercising it.
  # Source: HTTP Archive Web Almanac CSS chapter (display 83%, width/height ~75%, flex ~75%, grid ~45%,
  # position ~60%, transforms ~35%, backgrounds ~70%, fonts ~80%, color ~77%, var/calc ~40%). dom/html/
  # selectors are universal. Tune as Almanac updates; the ranking is robust to small changes.
  function weight(a){
    if(a=="dom")             return 1.00
    if(a=="html/dom")        return 1.00
    if(a=="css/selectors")   return 0.95
    if(a=="css/css-sizing")  return 0.90   # width/height/min/max/fit-content — universal
    if(a=="css/css-text")    return 0.88
    if(a=="css/css-display") return 0.85
    if(a=="css/css-fonts")   return 0.80
    if(a=="css/css-color")   return 0.77
    if(a=="css/css-flexbox") return 0.75   # display:flex on ~3/4 of pages
    if(a=="css/css-backgrounds") return 0.70
    if(a=="css/css-values")  return 0.55   # var()/calc()
    if(a=="css/css-position")return 0.60
    if(a=="css/css-overflow")return 0.50
    if(a=="css/css-grid")    return 0.45   # growing fast
    if(a=="css/css-transforms") return 0.35
    if(a=="css/css-ui")      return 0.30
    if(a=="cssom")           return 0.60
    if(a=="domparsing")      return 0.35
    if(a=="encoding")        return 0.05   # 1.1M tests but charset edge cases rarely block rendering
    if(a=="url")             return 0.15
    return 0.40                            # default for unlisted areas
  }
  # FLIP-RATE class: how many WPT subtests one fix turns green. HIGH-flip = a generic MECHANISM
  # (DOM/API/selector/parsing/CSSOM) flips thousands of testharness assertions at once (precedent:
  # attribute reflection +9,940). LOW-flip = LAYOUT reftests are byte/fuzzy-exact CONJUNCTIONS — a page
  # needs flex AND sizing AND floats AND inline all correct, so one fix flips ~nothing on WPT even while
  # it genuinely improves real-site M1. This is WHY the M1 visual number plateaus and the WPT number
  # climbs on DOM: layout is a slog. Steer M2/coverage by flip-rate; steer M1 by real-site frequency.
  function flip(a){
    if(a=="dom"||a=="html/dom"||a=="cssom"||a=="domparsing"||a=="css/selectors"||a=="css/css-values"||a=="url"||a=="encoding") return 1.00
    if(a=="css/css-fonts"||a=="css/css-text"||a=="css/css-color") return 0.50   # mixed reftest+testharness
    return 0.35                            # layout reftest conjunctions — real M1 value, low WPT flip
  }
  NR==1 || $1=="" || $1=="TOTAL" { if($1=="TOTAL"){tp=$2;tt=$3} next }
  {
    area=$1; pass=$2+0; total=$3+0; pct=$4+0
    if(total<=0) next
    fail=total-pass; w=weight(area)
    # LEVERAGE = usage-weight x winnable-tests(capped) x room-to-grow.
    #  - cap failing at 4000: past a few thousand it is one "big vein", raw size stops being urgency.
    #  - headroom=(1-pass%): an area at 93% is the death-TAIL (little left); at 6% it is the BODY.
    #    This is what deprioritises near-done areas and the giant-but-mostly-passing ones.
    capfail = (fail>4000)?4000:fail
    headroom = 1 - pct/100
    fl = flip(area)
    lev = w * capfail * headroom * fl
    cls = (fl>=1.0)?"HIGH":((fl>=0.5)?"mixed":"LAYOUT")
    rows[++n]=sprintf("%12.0f\t%s\t%d/%d\t%.1f%%\t%s\tfail=%d", lev, area, pass, total, pct, cls, fail)
    sumfail+=fail
  }
  END{
    # sort rows by leverage desc (simple insertion; n is small)
    for(i=1;i<=n;i++) for(j=i+1;j<=n;j++){ split(rows[i],A,"\t"); split(rows[j],B,"\t"); if(B[1]+0>A[1]+0){t=rows[i];rows[i]=rows[j];rows[j]=t} }
    printf "  SOLID-GROUND TOTAL: %d/%d = %.2f%% WPT subtests passing (monotonic; the number only rises when the engine improves)\n", tp, tt, (tt?100*tp/tt:0)
    printf "  Remaining in-scope failures across areas: %d\n\n", sumfail
    printf "  %-8s  %-20s  %-12s  %-7s  %-7s  %s\n", "LEVERAGE","AREA","PASS/TOTAL","PCT","FLIP","FAILING"
    printf "  %s\n", "────────────────────────────────────────────────────────────────────────────────"
    for(i=1;i<=n && i<=12;i++){ split(rows[i],A,"\t"); printf "  %-8.0f  %-20s  %-12s  %-7s  %-7s  %s\n", A[1], A[2], A[3], A[4], A[5], A[6] }
    printf "\n  READING: LEVERAGE = usage x winnable-tests(cap4000) x room-to-grow x FLIP-RATE.\n"
    printf "   • HIGH-flip rows (DOM/API/selector/parsing) = the FAST WPT/M2 climb — one MECHANISM flips thousands\n"
    printf "     (precedent: attribute reflection +9,940). Exhaust these first; mine blitz/packages/blitz-dom + servo/ for the mechanism.\n"
    printf "   • LAYOUT rows (flex/grid/sizing) = the M1 VISUAL slog — low WPT flip but real-site value. Accelerate by\n"
    printf "     porting whole algorithms from blitz/ (Taffy+Stylo, our EXACT stack) & servo/, NOT reverse-engineering per-assertion.\n"
    printf "  SOLID GROUND: climb the WPT total above (monotonic). STEERING only — certify Phase-0 against CrUX M1/M2, unchanged.\n"
  }
' "$AREAS"
exit 0
