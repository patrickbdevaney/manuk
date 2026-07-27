#!/usr/bin/env bash
# fidelity-sweep-cron.sh — keep the FULL-CORPUS fidelity trend LIVE by re-running the sweep on a safe,
# quiet-gated cadence, so docs/loop/FIDELITY-PROGRESS.tsv keeps filling and the perpetual pursuit of
# scorability/coverage/placement/visual -> 95% never goes blind on the corpus (the HEAD-20 subset the
# agent runs each tick is NOT the corpus). ops-check records the new row automatically once this writes.
#
# CONTENTION IS THE WHOLE RISK (a full sweep is ~90 min of render): this NEVER runs uncapped and NEVER
# runs on a busy box. It is fail-safe — every gate that is not satisfied SKIPS with a logged reason and
# exits 0. It writes ONLY the sweep's own out-dir + its log. The agent is never touched.
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
LOG=.git/manuk-fidelity-sweep.log
LOCK=.git/manuk-fidelity-sweep.lock
OUTDIR=.git/fidelity-full
now(){ date -u +'%Y-%m-%dT%H:%M:%S' 2>/dev/null || echo '?'; }
log(){ printf '%s %s\n' "$(now)" "$*" >> "$LOG" 2>/dev/null; [ -t 1 ] && printf '  %s\n' "$*"; }

# ── single instance: never overlap a still-running sweep ────────────────────────────────────────────────
exec 9>"$LOCK" 2>/dev/null || { log "SKIP: cannot open lock"; exit 0; }
flock -n 9 || { log "SKIP: a sweep is already running (lock held)"; exit 0; }

# ── quiet gate: a heavy sweep must not land on top of the agent's build/verify/tick ─────────────────────
if pgrep -f 'bash ./scripts/(verify|tick)\.sh' >/dev/null 2>&1; then log "SKIP: verify/tick in progress"; exit 0; fi
if pgrep -x 'manuk-wpt' >/dev/null 2>&1;                    then log "SKIP: a manuk-wpt run is already live"; exit 0; fi
LOAD1=$(cut -d' ' -f1 /proc/loadavg 2>/dev/null || echo 99)
if awk "BEGIN{exit !(${LOAD1:-99} > 3.0)}"; then log "SKIP: load ${LOAD1} > 3.0 (box busy)"; exit 0; fi
# disk headroom: a sweep writes screenshots; never run into the wall-bank-guard zone
DP=$(df --output=pcent /home 2>/dev/null | tail -1 | tr -dc '0-9' || echo 0)
if [ "${DP:-0}" -ge 92 ]; then log "SKIP: /home ${DP}% (>=92) — no room for sweep artifacts"; exit 0; fi

BIN=target/release/manuk-wpt
[ -x "$BIN" ] || { log "SKIP: $BIN not built (agent owns the build; not building from here)"; exit 0; }

# ── run it: capped (observer-run.sh, the OOM guard) + niced-per-site (fidelity-sweep.sh) + gentle jobs ──
log "START full-corpus sweep (load ${LOAD1}, disk ${DP}%) -> $OUTDIR"
START=$(date +%s 2>/dev/null || echo 0)
if scripts/observer-run.sh --mem 6G -- bash scripts/fidelity-sweep.sh --out "$OUTDIR" --jobs 2 >>"$LOG" 2>&1; then
  DUR=$(( $(date +%s 2>/dev/null || echo 0) - START ))
  log "DONE full-corpus sweep in ${DUR}s — ops-check will record the new FIDELITY-PROGRESS row within 15m"
  bash scripts/fidelity-progress.sh >/dev/null 2>&1 || true   # record immediately too
else
  log "ENDED non-zero (partial or capped-out) — a partial results.tsv is still recorded honestly"
fi
exit 0
