#!/usr/bin/env bash
# throw-killer.sh — the scorability worklist generator (ClusterFuzz-style signature bucketing).
#
# WHY: M1 is arithmetically CAPPED at the scorability rate (~82%): a site that never yields a scored
# fidelity tree cannot pass no matter how good layout is. fidelity-progress.sh prints the bucket COUNTS;
# this lists the SITES per bucket, largest bucket first, so the grind opens real pages and drains the
# single root cause that reclaims the most sites per tick. Each cleared throw both raises the ceiling
# AND becomes an M2 function candidate for free (function-first is owner-locked).
#
# INPUT: a per-site SWEEP-t*-rows.tsv. IN-SCOPE UNSCORED = a row that is NOT an accepted exclusion
# (bot-wall / unreachable / probe-blocked / http-4xx-5xx / empty) and either has shape "-" (timeout) or
# carries a shell/divergence/thin/render/starved reason despite a shape number. Read-only, exits 0.
set -euo pipefail
cd "$(dirname "$0")/.."
ROWS="${1:-$(ls -t docs/loop/SWEEP-t*-rows.tsv 2>/dev/null | head -1)}"
[ -n "${ROWS:-}" ] && [ -f "$ROWS" ] || { echo "throw-killer: no SWEEP-t*-rows.tsv found"; exit 0; }

echo "── THROW-KILLER scorability worklist  ←  $ROWS"
echo "   Each in-scope site here does NOT yield a scored tree, so it fails M1 by construction and caps"
echo "   the whole gate. Draining the largest bucket is the biggest single scorability jump."
echo "   (Excluded bot-wall/unreachable/probe-blocked/http/empty are omitted — accepted, capped.)"
echo
awk -F'\t' '
  $1 ~ /^#/ || $1=="" { next }
  $9 ~ /bot-wall|unreachable|probe-blocked|^http-|empty-/ { next }        # accepted exclusions
  ($3=="-") || ($9 ~ /module|shell|thin|render-fail|divergence|starved/) {
    r=$9; if(r=="") r="(blank/unknown)";
    if      (r ~ /timeout/)      b="timeout";
    else if (r ~ /module|shell/) b="shell-only";
    else if (r ~ /thin/)         b="thin";
    else if (r ~ /render-fail/)  b="render-fail";
    else if (r ~ /divergence/)   b="tree-divergence";
    else if (r ~ /starved/)      b="css-starved";
    else                         b="other";
    cnt[b]++; sites[b]=sites[b] sprintf("      %-28s %s\n", $1, r);
  }
  END {
    n=0; for(b in cnt) order[n++]=b;
    for(i=0;i<n;i++) for(j=i+1;j<n;j++) if(cnt[order[j]]>cnt[order[i]]){t=order[i];order[i]=order[j];order[j]=t}
    tot=0;
    for(i=0;i<n;i++){
      printf "  %-16s %d site(s)%s\n", order[i], cnt[order[i]], (i==0?"   <- largest bucket, drain first":"");
      printf "%s", sites[order[i]]; tot+=cnt[order[i]];
    }
    printf "\n  total in-scope unscored: %d\n", tot;
    print  "  DISCIPLINE: gate each fix with an IN-PAGE probe asserting injected content reached the tree —";
    print  "  a schedule/order bug is invisible to any final-frame diff. Stub the FULL shape, never half-install.";
  }
' "$ROWS"
exit 0
