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

# **The job count is part of the measurement, not a knob for making it finish sooner.**
#
# The watchdog below wraps the WHOLE oracle process — ours *and Chromium's*. Raise the concurrency and
# you raise Chromium's startup contention, the watchdog fires on slowness you manufactured, and every
# site it kills is recorded as a hang **and attributed to us**. Measured, on the same binary, the same
# corpus and the same hour:
#
#      4 jobs  →  11 hangs /  88 sites   (12.5%)
#     12 jobs  →  22 hangs /  45 sites   (49.0%)
#
# The second number is not a worse browser. It is a busier machine. A hang count is only a measurement
# *relative to a baseline taken the same way*, so re-running at a different width is not a faster
# measurement — it is a different one, and it is not comparable to anything.
#
# 6 is the number every recorded baseline was taken at. Change it only if you are re-baselining
# everything, and say so out loud when you do.
JOBS="${MANUK_ORACLE_JOBS:-6}"
# **The process watchdog is a BACKSTOP, not the Bar 0 metric.** See `one_site`: it wraps our render
# AND Chromium's AND the diff, so it cannot attribute the time it kills. Chromium takes 30-60s on a
# news front page (measured: bloomberg 60.6s, vox 59.7s, cnn 59.2s, economist 53.9s — against our
# 15.5s, 7.6s, 29.5s, 15.1s on the same documents). At a 90s budget the watchdog was firing on
# CHROMIUM'S time and recording the result as OUR hang. It is generous now, and the hang number comes
# from `manuk_ms` instead.
TIMEOUT="${MANUK_ORACLE_TIMEOUT:-240}"
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

# **Every record is stamped with the run it came from.**
#
# This bit, and it was invisible: `export -f one_site` means the xargs CHILDREN carry the function
# definition they were forked with. Killing the driver does not kill them. So a previous crawl's workers
# — with the previous crawl's watchdog and the previous crawl's HANG-vs-TIMEOUT semantics — happily kept
# writing into the results directory of the NEW run, and the merged output was two different experiments
# wearing one name. It was only caught because the labels happened to differ ("HANG" from the old script,
# "TIMEOUT" from the new one) and the totals did not add up.
#
# A stamp makes it checkable instead of lucky: if a results directory contains more than one RUN_ID, it
# is not a measurement and `status-update.sh` refuses to report it.
RUN_ID="$(date +%s)-$$"
export RUN_ID
BIN=target/release/manuk-wpt
[ -x "$BIN" ] || { echo "build first: cargo build --release -p manuk-wpt" >&2; exit 1; }

# Refuse to start while another crawl's workers are alive. They would write THEIR results — with THEIR
# watchdog and THEIR semantics — into this run's directory, and the output would be two experiments
# wearing one name. `pkill` on the driver does not reach them: they are xargs children holding an
# exported copy of `one_site`.
if pgrep -f "$BIN oracle" >/dev/null 2>&1; then
  echo "REFUSING: oracle workers from a previous crawl are still alive." >&2
  echo "  They will write into this run's results directory and silently contaminate it." >&2
  echo "  Kill them first:  pkill -9 -f '$BIN oracle'" >&2
  exit 1
fi

TOTAL=$(grep -cE '^[a-z_]+[[:space:]]+https' "$CORPUS")
echo "▶ oracle crawl: $TOTAL sites · $JOBS jobs · ${TIMEOUT}s watchdog per site"
if [ "$JOBS" -ne 6 ]; then
  printf '\033[33m  ⚠ %s jobs, not the baseline 6 — the hang count from this run is NOT comparable to any\033[0m\n' "$JOBS"
  printf '\033[33m    recorded number. The watchdog wraps Chromium too; concurrency you add shows up as hangs\033[0m\n'
  printf '\033[33m    attributed to us. (Measured: 4 jobs 12.5%%, 12 jobs 49.0%% — same binary, same hour.)\033[0m\n'
fi
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
      # **`TIMEOUT`, not `HANG` — and the distinction is the whole point.**
      #
      # This watchdog wraps the entire oracle process: our render, Chromium's render, and the diff. When
      # it fires we know the PROCESS took too long. We do NOT know whose time that was, and on this
      # corpus it is usually Chromium's. Calling it a hang and attributing it to us is precisely the
      # error Lesson 4 exists to prevent — *an oracle must never be able to charge its own slowness to
      # your account* — and it is the error that produced the "73/265 sites hang" headline that drove
      # the schedule for several ticks.
      #
      # It is still a hard, counted, attributed-to-NOBODY failure. It is never a skipped test. But the
      # Bar 0 number is computed from `manuk_ms`, which is OUR clock, and it is computed in
      # `status-update.sh`.
      printf '{"kind":"meta","site":"%s","class":"%s","status":"TIMEOUT","seconds":%s,"run":"%s"}\n' \
        "$short" "$class" "$el" "$RUN_ID" > "$OUT/$short.jsonl"
      printf '  \033[31m⏱ TIMEOUT\033[0m %-32s %ss (process — engine NOT attributed)\n' "$short" "$el"
      return 0
    fi
    printf '{"kind":"meta","site":"%s","class":"%s","status":"FAIL","rc":%s,"run":"%s"}\n' \
      "$short" "$class" "$rc" "$RUN_ID" > "$OUT/$short.jsonl"
    printf '  \033[33m✗ fail\033[0m %-34s rc=%s\n' "$short" "$rc"
    return 0
  fi
  # `oracle` writes the file itself on success; if it did not, the site was discarded (bot wall,
  # error page, no-script fallback) and that must be recorded too, not silently dropped.
  if [ ! -s "$OUT/$short.jsonl" ]; then
    printf '{"kind":"meta","site":"%s","class":"%s","status":"DISCARDED","run":"%s"}\n' \
      "$short" "$class" "$RUN_ID" > "$OUT/$short.jsonl"
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
