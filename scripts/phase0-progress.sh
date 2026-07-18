#!/usr/bin/env bash
# ── PHASE-0 (DAILY-DRIVER) PROGRESS METER — the near horizon rolled up to one number that moves per tick.
#
# NOT A GATE. It never blocks a tick (always exits 0). It reads docs/loop/CONSTELLATION.tsv — the 106-capability
# daily-driver checklist that decomposes V1-SCOPE.md's four components (rendering parity · agentic surface ·
# reasonable security · reasonable performance) — and reports how much is CONFIRMED WORKING, per class and
# overall, plus a timestamped ledger row per landed tick so the TREND is visible, not just the snapshot.
#
# The status vocabulary (constellation.sh): gated (a G_* gate asserts it — cannot rot) · works (measured, no
# gate) · partial (common case only) · missing (honest hole) · unknown (nobody has looked — a bug, not a state).
#
# Readiness credit is deliberately weighted toward the un-rottable statuses:
#     gated = 1.0    works = 1.0    partial = 0.5    unknown = 0    missing = 0
# So "readiness %" = (gated + works + 0.5·partial) / total. Two companion numbers keep it honest:
#   gate-locked %  = gated / total       (the floor nothing can regress)
#   measured %     = (total − unknown)/total   (how much has even been looked at — 'unknown' is unmeasured, not broken)
#
#   scripts/phase0-progress.sh            # print the rollup
#   scripts/phase0-progress.sh --ledger   # append a row to docs/loop/PHASE0-PROGRESS.tsv (tick.sh calls this)
set -uo pipefail
cd "$(dirname "$0")/.."
TSV=docs/loop/CONSTELLATION.tsv
LEDGER=docs/loop/PHASE0-PROGRESS.tsv
BLD=$'\033[1m'; GRN=$'\033[32m'; YEL=$'\033[33m'; CYA=$'\033[36m'; OFF=$'\033[0m'
[ -f "$TSV" ] || { echo "missing $TSV (phase-0 meter is informational — not failing the tick)" >&2; exit 0; }

# Overall counts (status is column 4; row 1 is the header).
read -r N G W P M U <<EOF
$(awk -F'\t' 'NR>1 && $1!="" { n++
  if($4=="gated")g++; else if($4=="works")w++; else if($4=="partial")p++
  else if($4=="missing")m++; else if($4=="unknown")u++ }
  END{printf "%d %d %d %d %d %d", n+0,g+0,w+0,p+0,m+0,u+0}' "$TSV")
EOF
WORKING=$(awk -v g="$G" -v w="$W" -v p="$P" 'BEGIN{printf "%.1f", g+w+0.5*p}')
READY=$(awk    -v x="$WORKING" -v n="$N" 'BEGIN{printf "%.0f",(n>0?100*x/n:0)}')
GATEDPCT=$(awk -v g="$G" -v n="$N" 'BEGIN{printf "%.0f",(n>0?100*g/n:0)}')
MEASPCT=$(awk  -v u="$U" -v n="$N" 'BEGIN{printf "%.0f",(n>0?100*(n-u)/n:0)}')
TICK=$(git log --oneline -40 2>/dev/null | grep -oiE 'tick [0-9]+' | head -1 | grep -oE '[0-9]+' || echo 0)

if [ "${1:-}" = "--ledger" ]; then
  NOW=$(date -Iseconds 2>/dev/null || echo "?")
  [ -f "$LEDGER" ] || printf 'tick\tiso\tcaps\tgated\tworks\tpartial\tmissing\tunknown\tworking\tready_pct\tgated_pct\tmeasured_pct\n' > "$LEDGER"
  printf '%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s\n' \
    "$TICK" "$NOW" "$N" "$G" "$W" "$P" "$M" "$U" "$WORKING" "$READY" "$GATEDPCT" "$MEASPCT" >> "$LEDGER"
  # Trend vs the previous ledger row (delta in readiness points).
  DELTA=$(awk -F'\t' 'NR>1{r[NR]=$10} END{ if(NR>=3){d=r[NR]-r[NR-1]; printf "%+d", d} else print "·"}' "$LEDGER")
  printf '  %s✓%s PHASE-0 readiness %s%s%%%s (gate-locked %s%%, measured %s%%) trend %s — logged\n' \
    "$GRN" "$OFF" "$BLD" "$READY" "$OFF" "$GATEDPCT" "$MEASPCT" "$DELTA"
  exit 0
fi

# ── Human-readable rollup.
printf '%s── PHASE 0 · DAILY-DRIVER READINESS%s   (V1-SCOPE.md — measure, not gate)\n\n' "$BLD" "$OFF"
BARN=$(awk -v p="$READY" 'BEGIN{n=int(p/5); s=""; for(i=0;i<n;i++)s=s "█"; for(i=n;i<20;i++)s=s "·"; print s}')
printf '  readiness  %s %s%s%%%s   (gate-locked %s%%  ·  measured %s%%  ·  %s never looked at)\n\n' \
  "$BARN" "$BLD" "$READY" "$OFF" "$GATEDPCT" "$MEASPCT" "$U"
printf '  %-10s %5s  %6s %6s %8s %8s %8s   %s\n' "class" "caps" "gated" "works" "partial" "missing" "unknown" "working"
printf '  %-10s %5s  %6s %6s %8s %8s %8s   %s\n' "──────────" "─────" "──────" "──────" "────────" "────────" "───────" "───────"
for c in doc app platform media cross; do
  cn=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c' "$TSV" | wc -l)
  cg=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c && $4=="gated"'   "$TSV" | wc -l)
  cw=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c && $4=="works"'   "$TSV" | wc -l)
  cp=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c && $4=="partial"' "$TSV" | wc -l)
  cm=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c && $4=="missing"' "$TSV" | wc -l)
  cu=$(awk -F'\t' -v c="$c" 'NR>1 && $1==c && $4=="unknown"' "$TSV" | wc -l)
  cwork=$(awk -v g="$cg" -v w="$cw" -v p="$cp" -v n="$cn" 'BEGIN{printf "%.0f",(n>0?100*(g+w+0.5*p)/n:0)}')
  printf '  %-10s %5s  %s%6s%s %6s %8s %8s %s%7s%s   %s%%\n' \
    "$c" "$cn" "$GRN" "$cg" "$OFF" "$cw" "$cp" "$cm" "$YEL" "$cu" "$OFF" "$cwork"
done
printf '\n  overall    %5s  %6s %6s %8s %8s %8s   %s%% working\n' "$N" "$G" "$W" "$P" "$M" "$READY"
if [ -f "$LEDGER" ] && [ "$(wc -l < "$LEDGER")" -gt 2 ]; then
  printf '\n%s  recent trend (tick · readiness%%):%s ' "$CYA" "$OFF"
  awk -F'\t' 'NR>1{printf "%s→%s%% ", $1, $10}' "$LEDGER" | tr ' ' '\n' | tail -6 | tr '\n' ' '; printf '\n'
fi
printf '\n  the biggest lever is not always the lowest %%: %s37 unknowns%s are unmeasured, not broken —\n' "$YEL" "$OFF"
printf '  probing one is a cheap tick that can flip it green. Full detail: scripts/constellation.sh\n'
exit 0
