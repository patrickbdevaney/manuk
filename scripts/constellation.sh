#!/usr/bin/env bash
# ── THE CONSTELLATION, SCORED — the near horizon, made countable.
#
# **WPT is the scoreboard. This is the goal.**
#
# A 90% WPT score with no video, no OAuth popup and no real iframe is not a daily driver — it is a very
# well-tested rendering library. So the near horizon needs a definition that is *qualitative in what it
# demands* and *mechanical in how it is checked*, or it drifts. The pattern ledger drifted six times
# because it was prose nobody executed.
#
# `docs/loop/CONSTELLATION.tsv` is the definition. This scores it.
#
# **The status vocabulary is deliberately harsh:**
#
#   gated    a named G_* gate asserts it. This is the only status that cannot rot.
#   works    measured working, with a receipt — but nothing stops it regressing tomorrow.
#   partial  works for the common case, and the limit is written down.
#   missing  MEASURED absent. An honest hole.
#   unknown  NOBODY HAS EVER LOOKED.
#
# > **`unknown` is a bug, not a state.** An absent measurement is not a negative measurement (PROCESS #19,
# > #20, #21, #35, #41 — five times). Every `unknown` is a probe somebody has not written, and it is
# > exactly the soil the six phantom ❌s grew in.
#
#   scripts/constellation.sh          # score it
#   scripts/constellation.sh --gaps   # just the work list: the biggest hole in each class
set -uo pipefail
cd "$(dirname "$0")/.."

TSV=docs/loop/CONSTELLATION.tsv
BLD=$'\033[1m'; GRN=$'\033[32m'; RED=$'\033[31m'; YEL=$'\033[33m'; CYA=$'\033[36m'; OFF=$'\033[0m'
[ -f "$TSV" ] || { echo "missing $TSV" >&2; exit 1; }

if [ "${1:-}" != "--gaps" ]; then
  printf '%s── THE CONSTELLATION — what "a browser you can actually use" means%s\n\n' "$BLD" "$OFF"
  printf '  %-10s %5s  %6s %6s %8s %8s %8s   %s\n' \
    "class" "caps" "gated" "works" "partial" "missing" "UNKNOWN" "gated coverage"
  printf '  %-10s %5s  %6s %6s %8s %8s %8s   %s\n' \
    "──────────" "─────" "──────" "──────" "────────" "────────" "────────" "──────────────"

  for c in doc app platform media cross; do
    N=$(awk -F'\t' -v c="$c" '$1==c' "$TSV" | wc -l)
    G=$(awk -F'\t' -v c="$c" '$1==c && $4=="gated"'   "$TSV" | wc -l)
    W=$(awk -F'\t' -v c="$c" '$1==c && $4=="works"'   "$TSV" | wc -l)
    P=$(awk -F'\t' -v c="$c" '$1==c && $4=="partial"' "$TSV" | wc -l)
    M=$(awk -F'\t' -v c="$c" '$1==c && $4=="missing"' "$TSV" | wc -l)
    U=$(awk -F'\t' -v c="$c" '$1==c && $4=="unknown"' "$TSV" | wc -l)
    PCT=$(awk -v g="$G" -v n="$N" 'BEGIN{printf "%.0f", (n>0?100*g/n:0)}')
    BAR=$(awk -v p="$PCT" 'BEGIN{ n=int(p/5); s=""; for(i=0;i<n;i++) s=s "█"; for(i=n;i<20;i++) s=s "·"; print s }')
    UC=""; [ "$U" -gt 0 ] && UC="$RED$BLD"
    printf '  %-10s %5s  %s%6s%s %6s %8s %8s   %s%8s%s   %s %s%%\n' \
      "$c" "$N" "$GRN" "$G" "$OFF" "$W" "$P" "$M" "$UC" "$U" "$OFF" "$BAR" "$PCT"
  done

  TU=$(awk -F'\t' '$4=="unknown"' "$TSV" | wc -l)
  printf '\n'
  if [ "$TU" -gt 0 ]; then
    printf '  %s✗ %s capabilities are UNKNOWN — nobody has ever looked.%s\n' "$RED$BLD" "$TU" "$OFF"
    printf '    %sAn absent measurement is not a negative measurement. Five PROCESS defects say so, and the\n' "$RED"
    printf '    pattern ledger grew six phantom ❌s in exactly this soil. Each one is a probe nobody wrote.%s\n' "$OFF"
  else
    printf '  %s✓ nothing is unknown. Every capability has been looked at.%s\n' "$GRN" "$OFF"
  fi
fi

printf '\n%s── THE BIGGEST HOLE IN EACH CLASS — the near horizon'"'"'s work list%s\n\n' "$BLD" "$OFF"
for c in doc app platform media cross; do
  printf '  %s%s%s\n' "$CYA$BLD" "$c" "$OFF"
  # UNKNOWN first — you cannot rank what you have not measured, so measuring it outranks fixing anything.
  awk -F'\t' -v c="$c" '$1==c && $4=="unknown" {printf "    ? %-46s %s\n", $2, $3}' "$TSV" | head -3
  awk -F'\t' -v c="$c" '$1==c && $4=="missing" {printf "    ✗ %-46s %s\n", $2, $3}' "$TSV" | head -3
done

cat <<'RULE'

  THE RULE (docs/loop/CONSTELLATION.md):

    ? outranks ✗.  You cannot rank a hole you have not measured, and "unknown" has produced a
      phantom ❌ six times. Probing an unknown is a legitimate tick and often a cheap one.

    The tick is chosen by whichever horizon is FURTHER BEHIND — the WPT mechanism with the most
    failing subtests, or the constellation class with the largest hole. Not by whichever is more
    interesting. The two are nearly orthogonal (measured, tick 70), so optimising one blind to
    the other is how ten ticks went into the wrong room.

    BORROW, DO NOT BUILD (media): symphonia, dav1d, openh264, cpal, ffmpeg behind a flag.
    A tick spent writing an H.264 decoder is a tick not spent on the browser. The engine's job
    is the plumbing — demux, buffer, sync, present — not the codec.
RULE
