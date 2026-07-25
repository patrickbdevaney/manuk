#!/usr/bin/env bash
# build-corpus.sh — build a statistically-representative, two-stratum, reproducible website corpus.
#
# WHY (docs/loop/DAILY-DRIVER-CERTIFICATION.md §3): the old 265-site corpus is a CONVENIENCE sample
# (hand-curated, biased toward "sites that are easy to load"), so it cannot support the claim "X% of the
# web works". This builds a REPRESENTATIVE sample from a real traffic-weighted frame (Chrome UX Report,
# via the zakird/crux-top-lists mirror — page-view weighted, monthly, origin-level, exactly what HTTP
# Archive crawls), in TWO STRATA:
#   * HEAD  — weighted toward the top tiers → "covers what people actually visit" (CrUX: top 1k≈50%,
#             10k≈70%, 100k≈87% of all page loads).
#   * TAIL  — UNIFORM random from the deep tail → "handles the real web's diversity" (breakage lives in
#             the low-traffic millions; traffic-weighting alone STARVES this and hides the diversity that
#             breaks a from-scratch engine).
# Deterministic (seeded) so the corpus is reproducible; provenance is stamped in the header.
#
# It writes a NEW artifact (docs/bench/corpus-v2.tsv) and does NOT touch the live oracle-corpus.txt, so
# the running loop is undisturbed. The instrument migration to it is a steered agent task.
#
# Output columns (TSV):  stratum <TAB> rank <TAB> url         (category is tagged later by the on-page probe)
set -euo pipefail
cd "$(dirname "$0")/.."

SEED="${MANUK_CORPUS_SEED:-20260725}"        # change only deliberately — it changes which sites are drawn
HEAD_TARGET="${MANUK_CORPUS_HEAD:-250}"      # ~250 head + ~150 tail = ~400 → ±5% at 95% CI (Cochran, p=0.5)
TAIL_TARGET="${MANUK_CORPUS_TAIL:-150}"
FRAME_URL="https://raw.githubusercontent.com/zakird/crux-top-lists/main/data/global/current.csv.gz"
CACHE="/tmp/manuk-crux-frame.csv"
OUT="docs/bench/corpus-v2.tsv"

# A minimal, DOCUMENTED adult/gambling exclusion (a bounded named exception, DAILY-DRIVER-CERTIFICATION §3):
# these are a real traffic fraction but run standard SPA/video/payment stacks already represented by
# mainstream sites, so excluding them costs no unique web-platform capability while keeping the corpus
# work-appropriate. The excluded COUNT is reported so the bias is visible and bounded.
DENY='xhamster|xvideos|pornhub|xnxx|porn|redtube|onlyfans|chaturbate|\.xxx|[^a-z]sex|camsoda|stripchat|hentai|bet365|casino|betano|1xbet|1win|melbet|parimatch|betway|stake\.|bovada|\.bet|bet\.br|slot|jackpot|bajaj.*bet|satta'

echo "── fetching the CrUX frame (page-view-weighted, top 1M origins)"
if [ ! -s "$CACHE" ] || [ "${MANUK_CORPUS_REFRESH:-0}" = "1" ]; then
  curl -sS -m 120 "$FRAME_URL" | gunzip > "$CACHE"
fi
TOTAL=$(( $(wc -l < "$CACHE") - 1 ))
FRAME_DATE=$(date -u +%Y-%m-%d 2>/dev/null || echo unknown)   # the mirror is "current"; stamp fetch date
echo "   frame rows: $TOTAL"

# Tranco list-id, for a stable citable cross-reference (provenance only; sampling is off CrUX).
TRANCO_ID=$(curl -sS -m 15 "https://tranco-list.eu/top-1m-id" 2>/dev/null | tr -dc 'A-Za-z0-9' | head -c 12 || echo "unavailable")

# ── the deterministic two-stratum sampler (awk: a stable per-origin hash key → sort → take N per bucket).
# HEAD buckets weighted toward the top by page-view share; TAIL uniform.
awk -v seed="$SEED" -v head="$HEAD_TARGET" -v tail="$TAIL_TARGET" -v deny="$DENY" -F, '
  function key(s,   i,k){ k=seed%2147483647; for(i=1;i<=length(s);i++) k=(k*131 + (i*7) + index("abcdefghijklmnopqrstuvwxyz0123456789.:/-_", substr(tolower(s),i,1))) % 2147483647; return k }
  NR==1 { next }                                            # header
  {
    o=$1; r=$2+0; sub(/\r$/,"",o)
    if (o ~ deny) { excl++; next }
    # HEAD strata: page-view-share weighting → sample MORE from the top buckets.
    #   bucket  target-weight   (of the HEAD total)
    if      (r<=1000)    { b="h1";   w=0.32 }
    else if (r<=10000)   { b="h2";   w=0.24 }
    else if (r<=100000)  { b="h3";   w=0.24 }
    else if (r<=1000000) { b="t1";   w=0.20 }              # TAIL: uniform over the 100k–1M band
    else next
    k=key(o); print b"\t"k"\t"r"\t"o                        # bucket, sortkey, rank, origin
    if(b=="h1")wh1=0.32; if(b=="h2")wh2=0.24; if(b=="h3")wh3=0.24
  }
  END { print "EXCLUDED\t"excl > "/dev/stderr" }
' "$CACHE" > /tmp/manuk-corpus-pool.tsv 2> /tmp/manuk-corpus-excl.txt

EXCL=$(awk -F'\t' '/^EXCLUDED/{print $2}' /tmp/manuk-corpus-excl.txt 2>/dev/null || echo 0)

# per-bucket target counts (HEAD weighted, TAIL uniform)
h1=$(awk -v h="$HEAD_TARGET" 'BEGIN{printf "%d", h*0.32}')
h2=$(awk -v h="$HEAD_TARGET" 'BEGIN{printf "%d", h*0.24}')
h3=$(awk -v h="$HEAD_TARGET" 'BEGIN{printf "%d", h*0.24 + h*0.20}')   # h3 gets the top-100k remainder
t1="$TAIL_TARGET"

take() { # bucket-label target  → deterministic top-N by sortkey. `awk NR<=n` (not `head`) reads the
         # whole stream, so it never closes the pipe early — `head` would SIGPIPE `sort`, and pipefail
         # would turn that into a mid-script abort (the bug that produced a truncated 140-site corpus).
  awk -F'\t' -v b="$1" '$1==b' /tmp/manuk-corpus-pool.tsv | sort -t"$(printf '\t')" -k2,2n | awk -v n="$2" 'NR<=n'
}

{
  echo "# MANUK REPRESENTATIVE CORPUS v2 — docs/loop/DAILY-DRIVER-CERTIFICATION.md §3"
  echo "# frame: CrUX top-1M (zakird/crux-top-lists mirror) · fetched: $FRAME_DATE · seed: $SEED · tranco-id: $TRANCO_ID"
  echo "# strata: HEAD (traffic-weighted, ranks<=100k) + TAIL (uniform, 100k-1M). Two claims, reported separately."
  echo "# sizes: head_target=$HEAD_TARGET tail_target=$TAIL_TARGET · frame_rows=$TOTAL · adult/gambling excluded=$EXCL (bounded named exception)"
  echo "# DENOMINATOR IS FIXED: a timeout/crash/bot-wall is a COUNTED outcome (FAIL/EXCLUDED with reason), never a silent drop."
  echo "# columns: stratum <TAB> rank <TAB> url   (category tagged later by the on-page capability probe)"
  take h1 "$h1" | awk -F'\t' '{o=$4; sub(/^https?:\/\//,"",o); sub(/\/$/,"",o); print "HEAD\t"$3"\thttps://"o"/"}'
  take h2 "$h2" | awk -F'\t' '{o=$4; sub(/^https?:\/\//,"",o); sub(/\/$/,"",o); print "HEAD\t"$3"\thttps://"o"/"}'
  take h3 "$h3" | awk -F'\t' '{o=$4; sub(/^https?:\/\//,"",o); sub(/\/$/,"",o); print "HEAD\t"$3"\thttps://"o"/"}'
  take t1 "$t1" | awk -F'\t' '{o=$4; sub(/^https?:\/\//,"",o); sub(/\/$/,"",o); print "TAIL\t"$3"\thttps://"o"/"}'
} > "$OUT"

N=$(grep -vcE '^\s*#' "$OUT")
HN=$(grep -c '^HEAD' "$OUT"); TN=$(grep -c '^TAIL' "$OUT")
echo "── wrote $OUT : $N sites ($HN head + $TN tail), $EXCL adult/gambling excluded, seed $SEED"
echo "   this is a NEW artifact; the live oracle-corpus.txt is untouched. Instrument migration is a steered agent task."
