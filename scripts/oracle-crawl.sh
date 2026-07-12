#!/usr/bin/env bash
# **The oracle crawl** (METHODOLOGY Part 2; Part 21.2 item 2; G_HANG per Part 22.2).
#
# Twenty sites was an anecdote. Every class bug we ever found was found because *some site in the
# corpus happened to use that pattern* — which means the bugs we had NOT found were exactly the ones
# no corpus site happened to use. That is a structural blind spot, and no amount of staring at twenty
# sites closes it.
#
# ## Why a shell driver and not a loop inside the binary
#
# Three things change when the crawl goes from 20 sites to 265, and all three are about failure, not
# throughput:
#
#   1. **A hang eats the run.** In-process, one site that never returns takes the other 264 with it,
#      and the harness reports a smaller corpus *as if that were the corpus*. Every site here runs in
#      its own process under `timeout`. A timeout is a HARD, COUNTED, ATTRIBUTED failure — never a
#      skipped test, never silently absent. That is G_HANG, and a crawl is the only place it can
#      honestly be enforced.
#   2. **A crash loses everything.** Each site writes its own result file the moment it finishes, so
#      the crawl is resumable and a panic on site 200 costs one site, not two hundred.
#   3. **Serial is hours.** The engine is single-threaded by design (`Page`/`FontContext` are !Send),
#      so parallelism has to come from processes. It does.
#
# Snapshots are cached: a re-run diffs the SAME document, so a change in the numbers is attributable
# to the engine rather than to the web having moved under us. That is the one-snapshot rule, and it
# is enforced in `oracle.rs`, not merely intended.
#
#   scripts/oracle-crawl.sh [--jobs N] [--timeout SECONDS] [--fresh]
set -uo pipefail
cd "$(dirname "$0")/.."

JOBS="${MANUK_ORACLE_JOBS:-6}"
TIMEOUT="${MANUK_ORACLE_TIMEOUT:-90}"
OUT="${MANUK_ORACLE_OUT:-/tmp/manuk-oracle-run}"
SNAPS="/tmp/manuk-oracle-snapshots"
CORPUS="docs/bench/oracle-corpus.txt"

while [ $# -gt 0 ]; do
  case "$1" in
    --jobs) JOBS="$2"; shift 2 ;;
    --timeout) TIMEOUT="$2"; shift 2 ;;
    --fresh) rm -rf "$OUT"; shift ;;
    *) shift ;;
  esac
done

mkdir -p "$OUT" "$SNAPS"
BIN=target/release/manuk-wpt
[ -x "$BIN" ] || { echo "build first: cargo build --release -p manuk-wpt" >&2; exit 1; }

TOTAL=$(grep -cE '^[a-z_]+[[:space:]]+https' "$CORPUS")
echo "▶ oracle crawl: $TOTAL sites · $JOBS jobs · ${TIMEOUT}s watchdog per site"
echo "  results → $OUT   snapshots → $SNAPS (cached: a re-run diffs the SAME document)"
echo

one_site() {
  local class="$1" url="$2"
  local short; short=$(echo "$url" | sed 's|https\?://||;s|/.*||')
  # Already done? The crawl is resumable by construction.
  [ -s "$OUT/$short.jsonl" ] && return 0

  local start; start=$(date +%s)
  if timeout -k 5 "$TIMEOUT" "$BIN" oracle --urls "$url" --class "$class" --emit "$OUT" \
       --snapshots "$SNAPS" >/dev/null 2>"$OUT/$short.err"; then
    :
  else
    local rc=$? el=$(( $(date +%s) - start ))
    if [ "$rc" -eq 124 ] || [ "$rc" -eq 137 ]; then
      # **A hang is a hard failure, and it is RECORDED.** The one thing that must never happen is a
      # site vanishing from the run and the remaining sites being reported as "the corpus".
      printf '{"kind":"meta","site":"%s","class":"%s","status":"HANG","seconds":%s}\n' \
        "$short" "$class" "$el" > "$OUT/$short.jsonl"
      printf '  \033[31m⏱ HANG\033[0m  %-34s %ss (watchdog)\n' "$short" "$el"
      return 0
    fi
    printf '{"kind":"meta","site":"%s","class":"%s","status":"FAIL","rc":%s}\n' \
      "$short" "$class" "$rc" > "$OUT/$short.jsonl"
    printf '  \033[33m✗ fail\033[0m %-34s rc=%s\n' "$short" "$rc"
    return 0
  fi
  # `oracle` writes the file itself on success; if it did not, the site was discarded (bot wall,
  # error page, no-script fallback) and that must be recorded too, not silently dropped.
  if [ ! -s "$OUT/$short.jsonl" ]; then
    printf '{"kind":"meta","site":"%s","class":"%s","status":"DISCARDED"}\n' \
      "$short" "$class" > "$OUT/$short.jsonl"
    printf '  \033[90m− discard\033[0m %-32s (degraded oracle — never scored as our bug)\n' "$short"
  else
    local n; n=$(grep -c '"kind":"div"' "$OUT/$short.jsonl" 2>/dev/null || echo 0)
    printf '  \033[32m✓\033[0m %-36s %s divergence(s)\n' "$short" "$n"
  fi
}
export -f one_site
export OUT SNAPS BIN TIMEOUT

grep -E '^[a-z_]+[[:space:]]+https' "$CORPUS" \
  | awk '{print $1" "$2}' \
  | xargs -P "$JOBS" -I{} bash -c 'one_site $0' {} 2>/dev/null

echo
"$BIN" oracle-merge "$OUT"
