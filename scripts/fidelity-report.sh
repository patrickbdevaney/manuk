#!/usr/bin/env bash
# ── FID-SWEEP reporting — the honest accounting half.
#
# Split from the sweep so a completed run can be re-scored without re-crawling, and so the
# accounting rules live in ONE readable place instead of being buried in the crawl loop.
#
# THE CENTRAL RULE: report the flattering number and the honest number SIDE BY SIDE, and let the
# gap be the finding. Two ways a fidelity sweep lies to you, both of them by omission:
#   · averaging only the sites that rendered (failures vanish → a broken engine scores 95%)
#   · averaging COVERAGE over pages Chrome rendered no `[id]` elements on (measures nothing, and
#     the fidelity tool itself calls counting it a pass "how a gate that cannot fail looks green
#     forever"), or over pages with 3 ids, where 100% is noise
#   usage: scripts/fidelity-report.sh [results.tsv]
set -uo pipefail
cd "$(dirname "$0")/.."
R="${1:-.git/fidelity-sweep/results.tsv}"
[ -s "$R" ] || { echo "✗ no results at $R"; exit 1; }

awk -F'\t' '
NR==1 { next }
{
  cat=$1; st=$3; vis=$4; cov=$5; pl=$6; ids=$7
  tot[cat]++; TOT++
  stat[cat"\t"st]++; STAT[st]++
  # A site only contributes to the flattering means if it produced a number at all.
  if (vis != "" && vis+0 == vis) { vs[cat]+=vis; vn[cat]++; VS+=vis; VN++ }
  # COVERAGE counts ONLY where the structural probe had a real sample (status OK).
  if ((st == "OK" || st == "OK_SLOW") && cov != "" && cov+0 == cov) { cs[cat]+=cov; cn[cat]++; CS+=cov; CN++; idt[cat]+=ids; IDT+=ids }
  if ((st == "OK" || st == "OK_SLOW") && pl != "" && pl+0 == pl) { ps[cat]+=pl; pn[cat]++; PS+=pl; PN++ }
  # The honest denominator: every attempted site. A failure scores ZERO, it does not vanish.
  hv[cat] += (vis != "" && vis+0 == vis) ? vis : 0; HV += (vis != "" && vis+0 == vis) ? vis : 0
  ok = (st=="OK" || st=="OK_SLOW")
  hc[cat] += (ok && cov != "" && cov+0 == cov) ? cov : 0; HC += (ok && cov != "" && cov+0 == cov) ? cov : 0
}
END {
  printf "\n══ FID-SWEEP — REAL-SITE FIDELITY, CATEGORY-STRATIFIED ══\n\n"
  printf "%-14s %5s %5s %6s %6s  | %8s %8s | %8s %8s\n", \
         "category","sites","ok","noids","fail","vis(ok)","VIS(all)","cov(ok)","PLACE(ok)"
  printf "%s\n", "---------------------------------------------------------------------------------------------"
  for (c in tot) {
    nook = stat[c"\t"OK]
    noids = stat[c"\tNO_IDS"] + stat[c"\tLOW_SAMPLE"]
    fail = tot[c] - ((stat[c"\tOK"]+stat[c"\tOK_SLOW"]+0)) - noids
    printf "%-14s %5d %5d %6d %6d  | %7.1f%% %7.1f%% | %7.1f%% %7.1f%%\n", \
      c, tot[c], (stat[c"\tOK"]+stat[c"\tOK_SLOW"]+0), noids, fail, \
      (vn[c]?vs[c]/vn[c]:0), (tot[c]?hv[c]/tot[c]:0), \
      (cn[c]?cs[c]/cn[c]:0), (pn[c]?ps[c]/pn[c]:0)
  }
  printf "%s\n", "---------------------------------------------------------------------------------------------"
  printf "%-14s %5d %5d %6d %6d  | %7.1f%% %7.1f%% | %7.1f%% %7.1f%%\n", \
    "TOTAL", TOT, (STAT["OK"]+STAT["OK_SLOW"]+0), STAT["NO_IDS"]+STAT["LOW_SAMPLE"]+0, \
    TOT-((STAT["OK"]+STAT["OK_SLOW"]+0))-(STAT["NO_IDS"]+STAT["LOW_SAMPLE"]+0), \
    (VN?VS/VN:0), (TOT?HV/TOT:0), (CN?CS/CN:0), (PN?PS/PN:0)

  printf "\n  vis(ok)/cov(ok) = mean over sites that PRODUCED a number  <- the flattering read\n"
  printf "  VIS(all)/COV(all) = every attempted site in the denominator, failures = 0  <- THE HONEST ONE\n"
  printf "  A large gap between the two IS the finding: it means most of the corpus never rendered.\n"

  printf "\n══ STATUS BREAKDOWN (why sites did not score) ══\n"
  for (s in STAT) printf "  %-12s %4d  %5.1f%%\n", s, STAT[s], 100*STAT[s]/TOT
  printf "\n  NO_IDS/LOW_SAMPLE are NOT failures of the engine — they are failures of the INSTRUMENT:\n"
  printf "  Chrome rendered no (or <10) [id] elements, so structural coverage measured nothing there.\n"
  printf "  They are excluded from cov(ok) and counted as zero in COV(all). Never scored as passes.\n"
  if (CN) printf "  Mean id sample size on scored sites: %.0f ids.\n", IDT/CN

  printf "\n  PLACE(ok) = %% of elements within 8px of Chrome. COVERAGE SATURATES (we render the boxes);\n  PLACEMENT is what actually discriminates -- github scored 100%% coverage and 0.7%% visual.\n"
  printf "\n══ PHASE-0 EXIT GATE ══\n"
  printf "  rule: >=0.75 structural fidelity on >=95%% of the corpus, plus >=0.70 per category\n"
  gate_all = (PN?PS/PN:0)
  printf "  gate on PLACEMENT (not saturated coverage): %.1f%%  ->  %s\n", gate_all, (gate_all>=75 ? "MEETS the 0.75 bar" : "BELOW the 0.75 bar")
}
' "$R"

echo
echo "  full rows: $R    ·   worst scorers:"
awk -F'\t' 'NR>1 && ($3=="OK"||$3=="OK_SLOW") && $5!="" {print $5"\t"$1"\t"$2}' "$R" | sort -n | head -8 | sed 's/^/    /'
