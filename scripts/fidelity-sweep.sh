#!/usr/bin/env bash
# ── FID-SWEEP — broad, category-stratified real-site fidelity against Chromium.
#
# WHY THIS EXISTS. The per-tick G1 gate in verify.sh scores TWO static doc pages (Hacker News +
# a Wikipedia article). That is our easiest class, and `docs/bench/oracle-corpus.txt` warns in its
# own header: "the bugs we had NOT found were exactly the ones no corpus site happened to use."
# Capability% cannot see feature-present-but-site-broken. This sweep produces the number that
# actually decides whether Phase 0 is on track.
#
# ⚠ THIS IS DELIBERATELY **OFF** THE PER-TICK PATH. Each site costs a Manuk render + a live
# Chromium screenshot + network. Wiring 265 of those into verify.sh would blow the WALL ratchet
# and brick every tick. Run it manually or from a slow cron; bank the headline into RATCHET.tsv.
#
# ── PROBITY RULES (this instrument must not be able to flatter us) ──────────────────────────────
# 1. A site that FAILS TO FETCH / TIMES OUT / CRASHES is **not** absent from the average — it is
#    reported. Silently averaging only the sites that worked is how a broken engine scores 95%.
# 2. COVERAGE is only meaningful when Chrome actually rendered `[id]` elements. Pages with none
#    (or very few) are recorded as UNPROBEABLE / low-sample, **never** counted as a pass. The
#    fidelity tool itself says: "Counting it as a pass is how a gate that cannot fail looks green
#    forever." We record the id SAMPLE SIZE next to every score so a 100% built on 3 ids is
#    visible as such.
# 3. Both the flattering mean (over sites that rendered) and the honest mean (failures counted as
#    zero) are printed. If they diverge, the gap IS the finding.
#
#   usage: scripts/fidelity-sweep.sh [--limit N] [--category C] [--jobs J] [--timeout S] [--out DIR]
set -uo pipefail
cd "$(dirname "$0")/.."

LIMIT=0; CATEGORY=""; JOBS=3; TMO=180   # 90s was too tight: heavy news sites time out in CHROMIUM, not in us
OUT="${MANUK_SWEEP_OUT:-.git/fidelity-sweep}"
while [ $# -gt 0 ]; do
  case "$1" in
    --limit)    LIMIT="$2"; shift 2 ;;
    --category) CATEGORY="$2"; shift 2 ;;
    --jobs)     JOBS="$2"; shift 2 ;;
    --timeout)  TMO="$2"; shift 2 ;;
    --out)      OUT="$2"; shift 2 ;;
    *) echo "unknown flag: $1"; exit 2 ;;
  esac
done

BIN=target/release/manuk-wpt
[ -x "$BIN" ] || { echo "✗ $BIN not built — run: cargo build --release -p manuk-wpt"; exit 1; }
command -v google-chrome >/dev/null || { echo "✗ google-chrome not found — the oracle is required"; exit 1; }

CORPUS=docs/bench/oracle-corpus.txt
mkdir -p "$OUT/shots"
RESULTS="$OUT/results.tsv"
printf 'category\turl\tstatus\tvisual\tcoverage\tplacement\tids\tmissing\tmisplaced\tmdx\tmdy\tmanuk_ms\tchrome_ms\tfirst_divergence\n' > "$RESULTS"

# ── select the corpus rows ─────────────────────────────────────────────────────────────────────
sel() {
  # ── STRATIFIED SAMPLING. `--limit N` used to take the FIRST N rows, and the corpus is grouped by
  # category, so `--limit 9` sampled nine NEWS sites — the heaviest class (paywalls, consent walls,
  # ad tech) — and called it a corpus-wide read. Round-robin across categories so any limit stays
  # representative instead of worst-case.
  grep -vE '^#|^$' "$CORPUS" \
    | awk -v c="$CATEGORY" 'NF>=2 && (c=="" || $1==c) {n[$1]++; print n[$1]"\t"$1"\t"$2}' \
    | sort -k1,1n -k2,2 | cut -f2-
}
TOTAL=$(sel | wc -l)
[ "$LIMIT" -gt 0 ] && TOTAL=$LIMIT
echo "▶ FID-SWEEP: $TOTAL sites${CATEGORY:+ (category=$CATEGORY)} · jobs=$JOBS · timeout=${TMO}s · out=$OUT"
echo "  (each site = Manuk render + live Chromium screenshot; this is SLOW by design and OFF the tick path)"

# ── one site ───────────────────────────────────────────────────────────────────────────────────
# Per-site invocation, not a batched --urls list: a hang or crash on one site must not take the
# whole batch down with it, and `timeout` only isolates cleanly at process granularity.
run_one() {
  local cat="$1" url="$2" name log
  name=$(printf '%s' "$url" | sed -E 's#^https?://##; s#/.*$##')
  log=$(mktemp)
  timeout "$TMO" nice -n 15 "$BIN" fidelity --urls "$url" --out "$OUT/shots" >"$log" 2>&1
  local rc=$?

  local visual cov ids missing misplaced mms cms status place mdx mdy fdiv
  visual=$(grep -oE 'MEAN VISUAL: *[0-9.]+' "$log" | grep -oE '[0-9.]+$' | head -1)
  # `structural: 97.1% (241 ids, 3 missing, 4 misplaced)` — ids is the SAMPLE SIZE, and it is the
  # difference between a real 97% and a 97% computed over two elements.
  local sline; sline=$(grep -oE 'structural: [^(]*\([0-9]+ ids, [0-9]+ missing, [0-9]+ misplaced\)' "$log" | head -1)
  cov=$(printf '%s' "$sline" | grep -oE 'structural: *[0-9.]+' | grep -oE '[0-9.]+$')
  ids=$(printf '%s' "$sline" | grep -oE '\([0-9]+ ids' | grep -oE '[0-9]+')
  missing=$(printf '%s' "$sline" | grep -oE '[0-9]+ missing' | grep -oE '[0-9]+')
  misplaced=$(printf '%s' "$sline" | grep -oE '[0-9]+ misplaced' | grep -oE '[0-9]+')
  # ── PLACEMENT is the metric that actually discriminates. COVERAGE saturates: the pilot found
  # github.com at 100% coverage / 0 missing / 49-of-50 MISPLACED and a 0.7% visual score. We render
  # every element Chrome renders and put them in the wrong place, so a coverage gate reads green on
  # a page that looks nothing like the reference. fidelity.rs says so itself: "the placement half of
  # the honest number, now that COVERAGE is near-saturated". Capture it, and capture the median
  # offset + first divergence, because those distinguish ONE systematic drift (a single bug) from N
  # independently misplaced boxes (many bugs) — the difference between a tractable fix and a slog.
  local pline; pline=$(grep -oE 'PLACEMENT: [0-9.]+% within [0-9]+px \| median offset dx=-?[0-9]+ dy=-?[0-9]+' "$log" | head -1)
  place=$(printf '%s' "$pline" | grep -oE 'PLACEMENT: *[0-9.]+' | grep -oE '[0-9.]+$')
  mdx=$(printf '%s' "$pline" | grep -oE 'dx=-?[0-9]+' | grep -oE '\-?[0-9]+')
  mdy=$(printf '%s' "$pline" | grep -oE 'dy=-?[0-9]+' | grep -oE '\-?[0-9]+')
  fdiv=$(grep -oE 'FIRST DIVERGENCE: .*' "$log" | head -1 | cut -c1-120 | tr '\t' ' ')
  mms=$(grep -oE 'manuk [0-9]+ms' "$log" | grep -oE '[0-9]+' | head -1)
  cms=$(grep -oE 'chromium [0-9]+ms' "$log" | grep -oE '[0-9]+' | head -1)

  # ── DATA-FIRST CLASSIFICATION (fixed after the first pilot) ────────────────────────────────
  # The first cut tested failure conditions BEFORE asking whether the run produced numbers, so
  # reuters came back FETCH_FAIL *carrying vis=65.3* (the grep matched a SUBRESOURCE fetch, not the
  # document) and cnn came back TIMEOUT *carrying vis=45.0 cov=78.9 ids=19* (it scored, then ran
  # past the clock). Both had rendered and been measured. Mis-scoring a success as a failure biases
  # the sweep AGAINST the engine — the mirror image of the flattering-average bug this instrument
  # exists to prevent, and just as dishonest. So: if a real number came back, the site SCORED.
  # A crash is never excused this way — a segfault is ours and stays a failure whatever it printed.
  if   grep -qiE 'segmentation fault|SIGSEGV|panicked' "$log"; then status=CRASH
  elif [ -n "${visual:-}" ]; then
    if   [ -z "${ids:-}" ] || [ "${ids:-0}" -eq 0 ]; then status=NO_IDS       # unprobeable, NOT a pass
    elif [ "${ids:-0}" -lt 10 ];                     then status=LOW_SAMPLE   # thin sample
    elif [ $rc -eq 124 ];                            then status=OK_SLOW      # scored, but over the clock
    else                                                  status=OK
    fi
  elif [ $rc -eq 124 ];                      then status=TIMEOUT
  elif grep -q 'fetch failed' "$log";        then status=FETCH_FAIL
  elif grep -q 'chrome:' "$log";             then status=CHROME_FAIL
  elif grep -q 'manuk render failed' "$log"; then status=RENDER_FAIL
  elif [ $rc -ne 0 ];                        then status=ERROR
  else                                            status=NO_DATA
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$cat" "$url" "$status" "${visual:-}" "${cov:-}" "${place:-}" "${ids:-0}" \
    "${missing:-}" "${misplaced:-}" "${mdx:-}" "${mdy:-}" "${mms:-}" "${cms:-}" "${fdiv:-}" >> "$RESULTS"
  printf '  %-11s %-42s %-11s vis=%-6s cov=%-6s place=%-6s ids=%s\n' \
    "$cat" "${url:0:42}" "$status" "${visual:-–}" "${cov:-–}" "${place:-–}" "${ids:-0}"
  rm -f "$log"
}

# ── drive with bounded concurrency ─────────────────────────────────────────────────────────────
n=0
while IFS=$'\t' read -r cat url; do
  [ "$LIMIT" -gt 0 ] && [ "$n" -ge "$LIMIT" ] && break
  n=$((n+1))
  while [ "$(jobs -rp | wc -l)" -ge "$JOBS" ]; do wait -n 2>/dev/null || sleep 0.3; done
  run_one "$cat" "$url" &
done < <(sel)
wait

# ── honest accounting ──────────────────────────────────────────────────────────────────────────
echo
./scripts/fidelity-report.sh "$RESULTS"
