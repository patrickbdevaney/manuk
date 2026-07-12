#!/usr/bin/env bash
# **The scheduled honesty check** (every 10 ticks; due at the tick shown by `--due`).
#
# The methodology got out of sync with reality once already: verify-wall compression and oracle
# breadth were prescribed in Parts 2 and 10 and simply had not been done, while tick-level backlog
# work carried on. Nobody noticed until a self-audit was explicitly requested. That is the failure
# this script exists to make impossible.
#
# It does NOT ask me whether I have been following the methodology. It checks the filesystem, the
# git history and the corpus for the *artifacts* each prescription would have produced if it had
# actually been executed, and it exits non-zero if any of them is missing. An audit you can pass by
# remembering things is not an audit.
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
bad()  { printf '  \033[31m✗ %s\033[0m\n' "$*" >&2; FAIL=$((FAIL+1)); }
ok()   { printf '  \033[32m✓ %s\033[0m\n' "$*"; }
head_(){ printf '\n\033[1m%s\033[0m\n' "$*"; }

TICK=$(grep -oP '^TICK:\s*\K[0-9]+' STATUS.md 2>/dev/null || echo 0)
if [ "${1:-}" = "--due" ]; then
  echo "$(( (TICK / 10 + 1) * 10 ))"
  exit 0
fi

echo "════ SELF-AUDIT @ tick $TICK — prescribed vs. actually built ════"

head_ "Tier 0 (Part 21) — these BLOCK backlog work"

# 1. The verify wall. The prescription is a NUMBER, so check the number, not the intent.
WALL=$(grep -oP '^seconds:\s*\K[0-9]+' .git/manuk-verify-receipt 2>/dev/null || echo "")
if [ -z "$WALL" ]; then
  bad "verify wall: never measured (no receipt). Run ./scripts/verify.sh."
elif [ "$WALL" -le 300 ]; then
  ok "verify wall: ${WALL}s ≤ 300s target"
else
  bad "verify wall: ${WALL}s EXCEEDS the 300s target — Part 21.2 item 1 has regressed. mold/lld, cargo-nextest, workspace-hack, risk-based gate scheduling."
fi

# 2. Oracle breadth. 20 sites is an anecdote; the prescription is 200-500.
SITES=$(grep -cE '^[a-z-]+\s+https?://' docs/bench/corpus.txt 2>/dev/null || echo 0)
if [ "$SITES" -ge 200 ]; then
  ok "oracle crawl frame: $SITES sites (≥200)"
else
  bad "oracle crawl frame: only $SITES sites — Part 21.2 item 2 wants 200-500. The cluster ranking cannot BE the ledger while the corpus is an anecdote."
fi

# 3. SPA miner. The largest unmeasured unknown in the schedule.
if [ -d "tests/spa" ] && [ "$(ls -1 tests/spa 2>/dev/null | wc -l)" -ge 10 ]; then
  ok "SPA starter apps: $(ls -1 tests/spa | wc -l) present"
else
  bad "SPA Framework Exception Miner: not run against 10 starter apps (Part 21.2 item 3). This is a BINARY risk — additive IDL work, or a scheduling-fidelity subsystem — and you cannot plan around it while it is unmeasured."
fi

head_ "Gates (Parts 5, 19.5, 22) — standing up, or only written down?"
gate() {
  local name="$1" pattern="$2" where="$3"
  if grep -rqE "$pattern" $where 2>/dev/null; then ok "$name"; else bad "$name — PRESCRIBED BUT NOT BUILT"; fi
}
gate "G_ALLOC"           "g_alloc"            scripts/verify.sh
gate "G_TEARDOWN"        "g_teardown"         scripts/verify.sh
gate "G_LOAD"            "g_load_budget"      scripts/verify.sh
gate "G_INTERACT"        "tab_operations"     scripts/verify.sh
gate "F1/F2 perf floors" "F1 cascade"         scripts/verify.sh
gate "G_SILENT_FAIL"     "g_silent_fail"      scripts/verify.sh
gate "G_HANG"            "g_hang"             scripts/verify.sh
gate "G_SPAWN"           "g_spawn"            scripts/verify.sh
gate "G_DEDUP"           "g_dedup"            scripts/verify.sh
gate "G_POOL_ISOLATION"  "g_pool"             scripts/verify.sh

head_ "Enforcement — is compliance mechanical, or is it my memory?"
[ "$(git config core.hooksPath)" = "scripts/hooks" ] && ok "pre-commit hook wired (core.hooksPath)" || bad "pre-commit hook NOT wired — compliance is back to being a claim"
[ -x scripts/hooks/pre-commit ] && ok "pre-commit hook executable" || bad "pre-commit hook missing/not executable"
grep -q "manuk-verify-receipt" scripts/verify.sh && ok "verify.sh writes a gate receipt" || bad "verify.sh writes no receipt — the hook cannot tell WHICH tree was verified"
grep -qs "STATUS.md" CLAUDE.MD CLAUDE.md && ok "CLAUDE.MD makes reading STATUS.md the first session action" || bad "CLAUDE.MD does not require the session-start STATUS.md read"

head_ "Journal (Part 7) — one entry per tick, no gaps"
MISSING=""
for t in $(seq $((TICK-4)) "$TICK"); do
  [ "$t" -lt 0 ] && continue
  grep -qE "^## Tick $t\b" docs/loop/JOURNAL.md 2>/dev/null || MISSING="$MISSING $t"
done
[ -z "$MISSING" ] && ok "journal entries present for the last 5 ticks" || bad "journal MISSING for tick(s):$MISSING"

head_ "Part 22 — runtime health (audited, or assumed?)"
grep -rq "duplicate" docs/loop/JOURNAL.md 2>/dev/null && ok "duplicate-work audit has been journaled at least once" \
  || bad "no call-graph leanness audit journaled (Part 22.3: duplicate fetches / tree renders / JS module execution)"

echo
if [ "$FAIL" -eq 0 ]; then
  printf '\033[32m\033[1mSELF-AUDIT: methodology and reality agree.\033[0m\n'
else
  printf '\033[31m\033[1mSELF-AUDIT: %s prescribed-but-not-executed item(s).\033[0m\n' "$FAIL"
  printf 'These are not backlog entries. Per Part 21.3, closing the gap between what the methodology\n'
  printf 'prescribes and what has actually been built OUTRANKS the ledger, and is what the next\n'
  printf 'session does.\n'
  exit 1
fi
