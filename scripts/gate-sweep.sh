#!/usr/bin/env bash
# ── GATE-SWEEP — run the FULL gate set, off the tick path.
#
# WHY: verify.sh's per-tick wall runs ~19 of the 176 gate files in engine/page/tests/ (see
# docs/loop/GATE-COVERAGE.md). Every other gate is a ratchet tooth nothing bites on — g_capability
# sat RED for ~100 ticks unseen. This sweep runs EVERY gate test target and reports the reds, so a
# silent regression in an unwatched gate surfaces within a day instead of never.
#
# Rules of engagement:
#   - OFF the tick path. Never called by verify.sh/tick.sh; never blocks a landing.
#   - Refuses to run while a build is live (would contend with the agent AND poison the warm cache).
#   - Uses the SAME feature set as verify's warm build (stylo,spidermonkey for manuk-page) so it
#     cannot feature-thrash cargo's fingerprints (see memory: headless-gate feature thrash).
#   - Cap memory from the caller: scripts/observer-run.sh --mem 10G -- scripts/gate-sweep.sh
#     (and CARGO_BUILD_JOBS is set explicitly here — observer-run's cgroup is invisible to cargo).
#
#   usage: scripts/gate-sweep.sh            # full sweep, writes .git/manuk-gate-sweep + prints reds
set -uo pipefail
cd "$(dirname "$0")/.."

if pgrep -x rustc >/dev/null 2>&1 || pgrep -x cargo >/dev/null 2>&1; then
  echo "✗ a compiler is live — refusing to contend with the agent's build. Re-run on a quiet box."
  exit 1
fi

OUT=.git/manuk-gate-sweep
: > "$OUT"
START=$SECONDS
echo "▶ FULL gate sweep (every test target; features match the warm build) — off the tick path"

run_pkg() { # pkg [feature-args...]
  local pkg="$1"; shift
  echo "── $pkg $* ──" | tee -a "$OUT"
  # --no-fail-fast: one red gate must not hide the others. test-threads=1: JS gates SIGSEGV when
  # sharing a process (see memory: one #[test] per JS gate).
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" cargo test -p "$pkg" "$@" --no-fail-fast -- --test-threads=1 2>&1 \
    | tee -a "$OUT" \
    | grep -E '^(test result:|failures:$)|FAILED|panicked at' \
    | sed 's/^/  /' | tail -40
}

run_pkg manuk-page --features stylo,spidermonkey
run_pkg manuk-media

echo
PASS=$(grep -c '^test result: ok' "$OUT" 2>/dev/null || true)
FAIL_TARGETS=$(grep -c '^test result: FAILED' "$OUT" 2>/dev/null || true)
RED_TESTS=$(grep -oP '^test \S+ \.\.\. FAILED' "$OUT" 2>/dev/null | sort -u)
printf '══ SWEEP DONE in %ss — %s green targets, %s RED targets ══\n' "$((SECONDS-START))" "${PASS:-0}" "${FAIL_TARGETS:-0}"
if [ -n "$RED_TESTS" ]; then
  echo "RED gates (regressions the per-tick wall cannot see — journal these, agent fixes them as ticks):"
  echo "$RED_TESTS" | sed 's/^/  ✗ /'
else
  echo "No red gates — the unwatched 85% are actually green today."
fi
echo "full log: $OUT"
