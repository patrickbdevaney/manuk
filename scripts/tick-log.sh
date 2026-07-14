#!/usr/bin/env bash
# ── THE CADENCE LEDGER: every tick, timestamped, measured, and described.
#
# **We have been measuring the browser and not the loop.**
#
# The project has two horizons — a daily-driver near horizon (doc/app/platform web) and a 50,000-test WPT
# far horizon — and no way to answer the only question that matters about either: **are we getting there,
# and how fast?** "Tick 69 landed" is not progress data. It is a receipt.
#
# So each landed tick appends one row of ground truth:
#
#   * **when** it landed, and **how long since the last one** — the real cycle time of
#     implement → debug → verify-wall → land. This is the loop's clock speed, and it is the denominator
#     of every rate we care about.
#   * **what it cost**: verify-wall seconds, files and lines changed.
#   * **what it bought**: the two horizon metrics, measured, not asserted —
#       - NEAR: capabilities asserted by `G_CAPABILITY`, ✅ rows in the capability ledger, live gates,
#         oracle hangs (Bar 0);
#       - FAR: WPT subtests passing / total.
#   * **the shape and the claim**: the tick's declared shape, and the journal headline in its own words.
#
# The point is the DERIVATIVE, not the row. A single tick's numbers say nothing; forty of them say how
# many ticks a capability costs, whether the wall is getting slower, whether WPT moves at all when we work
# on the near horizon (tick 64 says: it does not — and that is worth knowing), and how many ticks at this
# rate stand between here and parity.
#
# Appends to `docs/loop/CADENCE.tsv` (the data) and regenerates `docs/loop/CADENCE.md` (the reading).
# Called by `scripts/tick.sh` AFTER a successful push — a tick that did not land is not a tick.
set -uo pipefail
cd "$(dirname "$0")/.."

TSV=docs/loop/CADENCE.tsv
MD=docs/loop/CADENCE.md

TICK="${1:-$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)}"
SHA=$(git rev-parse --short HEAD)
# The COMMIT's own timestamp, not `now` — so a backfilled row and a live row mean the same thing, and a
# re-run of this script is idempotent rather than creative.
WHEN=$(git log -1 --format=%aI HEAD)
EPOCH=$(git log -1 --format=%at HEAD)

# ── What it cost.
STAT=$(git show --stat --format= HEAD | tail -1)
FILES=$(echo "$STAT" | grep -oP '\d+(?= files? changed)' || echo 0)
ADDED=$(echo "$STAT" | grep -oP '\d+(?= insertions?)' || echo 0)
DELETED=$(echo "$STAT" | grep -oP '\d+(?= deletions?)' || echo 0)
WALL=$(grep -oP '^LAST_WALL_TIME:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)

# ── The shape, and the tick's own words. The journal headline IS the qualitative impact statement — it is
#    written per tick, in terms of what changed for the browser, which is exactly what belongs here.
SHAPE=$(awk "/^## Tick ${TICK}[^0-9]/,0" docs/loop/JOURNAL.md 2>/dev/null \
        | grep -oiP 'TICK SHAPE:\s*\K[a-z0-9-]+' | head -1)
HEADLINE=$(grep -m1 "^## Tick ${TICK}[^0-9]" docs/loop/JOURNAL.md 2>/dev/null \
           | sed "s/^## Tick ${TICK} *[—-] *//" | cut -c1-160)

# ── NEAR HORIZON: what the browser can do, counted from the things that assert it.
CLAIMS=$(grep -cE '^\s+\("' engine/page/tests/g_capability.rs 2>/dev/null || echo 0)
PATTERNS=$(grep -c '✅' docs/loop/WEB-PATTERNS.md 2>/dev/null || echo 0)
GATES=$(ls engine/page/tests/g_*.rs shell/tests/g_*.rs 2>/dev/null | wc -l)
HANGS=$(grep -oP '^ORACLE_HANGS:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)

# ── FAR HORIZON: WPT. Recorded only when a run actually happened this tick — a stale number copied
#    forward is a fabricated data point, and this file's whole value is that it is not fabricated.
#    `scripts/wpt-run.sh` writes .git/manuk-wpt-last; absent ⇒ we carry the previous row's figure and
#    mark it as CARRIED, so a reader can never mistake it for a fresh measurement.
WPT_PASS=""; WPT_TOTAL=""; WPT_FRESH=0
if [ -f .git/manuk-wpt-last ]; then
  read -r WPT_PASS WPT_TOTAL < .git/manuk-wpt-last
  WPT_FRESH=1
fi
if [ -z "$WPT_PASS" ] && [ -f "$TSV" ]; then
  WPT_PASS=$(awk -F'\t' 'NR>1 && $13 != "" {p=$13} END{print p}' "$TSV")
  WPT_TOTAL=$(awk -F'\t' 'NR>1 && $14 != "" {t=$14} END{print t}' "$TSV")
fi

# ── Δ since the previous tick: the loop's clock speed.
#
# The previous tick is the highest tick STRICTLY BELOW this one — not simply the last row. Re-logging a
# tick that is already the final row would otherwise compare it against ITSELF and report Δ = 0, which is
# not a fast tick, it is a broken instrument. (It did exactly that on the first run.)
PREV_EPOCH=$(awk -F'\t' -v t="$TICK" 'NR>1 && $1+0 < t+0 && $3 != "" {e=$3} END{print e}' "$TSV" 2>/dev/null)
if [ -n "${PREV_EPOCH:-}" ] && [ "$PREV_EPOCH" -eq "$PREV_EPOCH" ] 2>/dev/null; then
  DELTA=$((EPOCH - PREV_EPOCH))
else
  DELTA=0
fi

if [ ! -f "$TSV" ]; then
  printf 'tick\tsha\tepoch\tiso\tdelta_s\tshape\twall_s\tfiles\tadded\tdeleted\tgates\tclaims\twpt_pass\twpt_total\twpt_fresh\tpatterns_ok\thangs\theadline\n' > "$TSV"
fi

# Idempotent: re-running for a tick already recorded replaces its row rather than duplicating it.
if grep -qP "^${TICK}\t" "$TSV" 2>/dev/null; then
  grep -vP "^${TICK}\t" "$TSV" > "$TSV.tmp" && mv "$TSV.tmp" "$TSV"
fi

printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$TICK" "$SHA" "$EPOCH" "$WHEN" "$DELTA" "${SHAPE:-unknown}" "$WALL" \
  "$FILES" "$ADDED" "$DELETED" "$GATES" "$CLAIMS" \
  "${WPT_PASS:-}" "${WPT_TOTAL:-}" "$WPT_FRESH" "$PATTERNS" "$HANGS" "$HEADLINE" >> "$TSV"

# Keep it sorted by tick so the file reads as a history rather than an append order.
{ head -1 "$TSV"; tail -n +2 "$TSV" | sort -n -k1,1; } > "$TSV.tmp" && mv "$TSV.tmp" "$TSV"

rm -f .git/manuk-wpt-last
python3 scripts/cadence-report.py > "$MD"
printf '  ✓ cadence: tick %s logged (Δ %s since previous)\n' "$TICK" \
  "$(awk -v d="$DELTA" 'BEGIN{ if (d<=0) print "—"; else if (d<3600) printf "%dm", d/60; else printf "%.1fh", d/3600 }')"
