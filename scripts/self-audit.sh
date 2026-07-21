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
SITES=$(grep -cE '^[a-z_]+[[:space:]]+https' docs/bench/oracle-corpus.txt 2>/dev/null || echo 0)
if [ "$SITES" -ge 200 ]; then
  ok "oracle crawl frame: $SITES sites (≥200)"
else
  bad "oracle crawl frame: only $SITES sites — Part 21.2 item 2 wants 200-500. The cluster ranking cannot BE the ledger while the corpus is an anecdote."
fi

# 3. SPA miner. The largest unmeasured unknown in the schedule.
# **This check used to count files in `tests/spa/` and call that "the miner ran".**
#
# `tests/spa/` contains `apps/`, `build.sh` and `README.md` — three entries — so the check reported
# "not run" for a miner that had been run against all eight frameworks and had already produced five
# real engine fixes. It was measuring directory entries, not the thing it claimed to measure. Exactly
# the failure this audit exists to catch, sitting inside the audit.
#
# What matters is not that the apps EXIST (their `dist/` is gitignored, so a fresh clone has nothing to
# run) but that **what the miner FOUND is captured in a gate**. G2 scenario 14 asserts the six
# primitives each framework needed — a use-after-GC in `ownerDocument`, `Node.prototype` accessors,
# `CharacterData.data`, the shadow root's nodeType, the fragment-insert contract, the DOM mixins. That
# gate is the durable output of the miner, and it runs every tick.
APPS=$(ls -1 tests/spa/apps 2>/dev/null | wc -l)
if [ "$APPS" -ge 8 ] && grep -q "framework primitive" engine/page/src/lib.rs 2>/dev/null; then
  ok "SPA Framework Exception Miner: $APPS apps, and its findings are asserted in G2 (scenario 14)"
elif [ "$APPS" -ge 8 ]; then
  bad "SPA apps exist ($APPS) but NOTHING ASSERTS what the miner found — the findings will rot. Add the gate."
else
  bad "SPA Framework Exception Miner: only $APPS starter apps (Part 21.2 item 3). This is a BINARY risk — additive IDL work, or a scheduling-fidelity subsystem — and you cannot plan around it while it is unmeasured."
fi

head_ "Gates (Parts 5, 19.5, 22) — standing up, or only written down?"
gate() {
  local name="$1" pattern="$2" where="$3"
  if grep -rqE "$pattern" $where 2>/dev/null; then ok "$name"; else bad "$name — PRESCRIBED BUT NOT BUILT"; fi
}
gate "G_ALLOC"           "g_alloc"            scripts/verify.sh
# tick 118 consolidated the four shell gates into ONE `_launch shell` invocation, so the old
# per-test patterns (g_teardown / tab_operations / runtime_instantiations) vanished from verify.sh
# while the gates themselves kept running — the audit then reported live gates as NOT BUILT.
# Detect the gate's verdict line, which must exist for the wall to judge it at all.
gate "G_TEARDOWN"        "G_TEARDOWN failed"  scripts/verify.sh
gate "G_LOAD"            "g_load_budget"      scripts/verify.sh
gate "G_INTERACT"        "G_INTERACT failed"  scripts/verify.sh
gate "F1/F2 perf floors" "F1 cascade"         scripts/verify.sh
gate "G_CONTAIN (Bar 0)" "g_contain"         scripts/verify.sh
gate "G_RUNTIME_COUNT"  "G_RUNTIME_COUNT failed" scripts/verify.sh
gate "G_SILENT_FAIL"     "g_silent_fail"      scripts/verify.sh
# G_HANG lives in the CRAWL, not the wall — a watchdog only means anything across many sites.
if grep -q "HANG" scripts/oracle-crawl.sh 2>/dev/null; then
  ok "G_HANG (per-site watchdog in the crawl: a timeout is a hard, COUNTED, ATTRIBUTED failure)"
else
  bad "G_HANG — PRESCRIBED BUT NOT BUILT"
fi
# ── RETIRED, with a reason. A prescribed gate that turns out to be inapplicable must be retired
#    EXPLICITLY, not built as theatre and not silently dropped. Both of these presumed a concurrency
#    architecture this engine does not have:
#
#    G_SPAWN ("work spawned per-action, not per-process") — subsumed by G_RUNTIME_COUNT, which is live
#      and asserts exactly one tokio runtime for the process. There is no second spawn-shaped thing to
#      guard: `handle.spawn` for a preload is a TASK on the one runtime, which is what tasks are for.
#
#    G_POOL_ISOLATION ("one page's work starving another's") — presumes a rayon/thread pool. There is
#      no rayon anywhere in the workspace (`grep -r rayon --include=Cargo.toml` → nothing). A gate on a
#      pool that does not exist would pass forever, tell nobody anything, and be counted as coverage.
#      That is the definition of a vacuous gate, and this project has now shipped one and does not
#      intend to ship another on purpose.
#
#    If either architecture arrives, so does its gate. Until then, saying so out loud beats an audit
#    that is green because I built something pointless to make it green.
ok "G_SPAWN — retired (subsumed by G_RUNTIME_COUNT: one runtime per process, asserted)"
ok "G_POOL_ISOLATION — retired (no rayon/thread pool exists; a gate on it would be vacuous by construction)"
gate "G_DEDUP"           "g_dedup"            scripts/verify.sh

head_ "Falsifiability (Part 33) — is each gate PROVEN to go red, or only known to go green?"
# A gate is not "a test that passes". A gate is a test that is KNOWN TO FAIL when the thing it protects
# is broken. Those are different claims and only one is worth anything.
#
# This check exists because `G_DEDUP` shipped VACUOUS: its first version called the synchronous load
# path, which never fetches, so it asserted `dupes == 0` against zero fetches and passed while measuring
# nothing at all. It would have been green through the entire storm it was written to catch — and it was
# caught by squinting at the output, not by anything mechanical.
if [ -x scripts/falsify.sh ]; then
  ok "scripts/falsify.sh exists — the gates are mutation-tested against themselves"
  # **The gate list is DERIVED from the wall, never carried as a copy.**
  #
  # This check used to hardcode `G_DEDUP G_LOAD G_RUNAWAY G2` — the four gates that existed when it was
  # written. Six more have shipped since (G_FIRST_PAINT, G_DEFER, G_FORM, G_IFRAME, G_ANIMATION,
  # G_SELECTOR) and it did not know about a single one of them. It was reporting "every gate is proven"
  # while checking 40% of them, and it would have kept saying so forever.
  #
  # That is the same defect as a test re-deriving the constant it checks (PROCESS #12) and the same as a
  # capability ledger whose ✅ was never tested (#19, #20, #21, #25). **A check that keeps its own copy of
  # the list it is checking will drift from reality, and it will do it silently.** Read the wall.
  GATES=$(grep -oE 'head_ "(G[A-Z_0-9]+) ' scripts/verify.sh | grep -oE 'G[A-Z_0-9]+' | sort -u)
  #
  # **SELF-PROVING gates need no mutation, and saying so is not an excuse — it is the point.**
  #
  # `G_CONTAIN` deliberately panics a build and asserts the PAGE dies while the process lives. It cannot
  # pass unless the thing it protects works, because the bug IS the test input. That is strictly stronger
  # than a mutation, and it is the standard every other gate is being held to.
  #
  # An exemption stays a NAMED, SHORT list. The moment it starts growing, it has stopped being a fact
  # about those gates and started being a place to put gates nobody wants to falsify.
  SELF_PROVING="G_CONTAIN"
  for g in $GATES; do
    if [[ " $SELF_PROVING " == *" $g "* ]]; then
      ok "  $g is SELF-PROVING (its test input IS the bug — stronger than a mutation)"
      continue
    fi
    # Gates whose falsifier is the build itself (parity/F-floors/crate suites) have no mutation; only
    # the named G_* gates in falsify.sh are claimed.
    if grep -q "want $g" scripts/falsify.sh 2>/dev/null; then
      ok "  $g declares how to break it"
    else
      bad "  $g has NO falsifier — nothing proves it can go red, so nothing proves it works"
    fi
  done
else
  bad "scripts/falsify.sh MISSING — every gate is trusted and none is proven. A gate that has never been shown to fail is not known to work (Part 33)."
fi

head_ "Process defects (docs/loop/PROCESS.md) — are they recorded, or re-learned?"
if [ -f docs/loop/PROCESS.md ]; then
  N=$(grep -cE '^\| [0-9]+ \|' docs/loop/PROCESS.md 2>/dev/null || echo 0)
  ok "process-defect ledger exists ($N recorded)"
  if grep -q "the mechanism that now prevents it\|The mechanism that now prevents it" docs/loop/PROCESS.md; then
    ok "  every defect names the MECHANISM that closes it, not just a lesson"
  else
    bad "  PROCESS.md records defects without naming the mechanism — a lesson you can recite while breaking it is a decoration"
  fi
else
  bad "docs/loop/PROCESS.md MISSING — process defects are being re-learned instead of closed. The process has produced more false conclusions than the engine has produced crashes."
fi

head_ "Enforcement — is compliance mechanical, or is it my memory? (Part 28)"
grep -q "TICK SHAPE" scripts/hooks/pre-commit && ok "tick-shape claim is CROSS-CHECKED against the cluster registry (28.2)" || bad "tick shape is a self-report, not a check"
grep -q "SELF-AUDIT OVERDUE" scripts/hooks/pre-commit && ok "self-audit cadence is enforced by the hook, not by memory (28.2)" || bad "audit cadence is a note someone has to remember"
[ -f docs/loop/CLUSTERS.md ] && ok "cluster registry exists ($(grep -cE '^C[0-9a-f]{4} ' docs/loop/CLUSTERS.md) clusters) — this IS the ledger" || bad "no cluster registry: the ledger is still judgement"
grep -q "Settled Decisions" STATUS.md && ok "Settled Decisions frozen in STATUS.md (29.2)" || bad "no Settled Decisions — closed questions are open to relitigation"
grep -q "^## Lessons" STATUS.md && ok "recurring lessons promoted out of the journal (29.1)" || bad "lessons still buried in an append-only journal a fresh session will not open"
[ -x scripts/status-update.sh ] && ok "STATUS.md is generated, not hand-narrated (28.3)" || bad "STATUS.md is hand-written prose"

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

head_ "Pattern ledger (WEB-PATTERNS.md) — is coverage tracked, or just claimed?"
if [ -f docs/loop/WEB-PATTERNS.md ]; then
  ok "pattern ledger exists ($(grep -cE '^\| ' docs/loop/WEB-PATTERNS.md) rows)"
  # A ledger nobody edits is a ledger nobody believes. It must move as capability moves.
  LEDGER_AGE=$(git log -1 --format=%ct -- docs/loop/WEB-PATTERNS.md 2>/dev/null || echo 0)
  ENGINE_AGE=$(git log -1 --format=%ct -- engine/ 2>/dev/null || echo 0)
  if [ "$ENGINE_AGE" -gt "$LEDGER_AGE" ] && [ $((ENGINE_AGE - LEDGER_AGE)) -gt 86400 ]; then
    bad "the engine has changed but WEB-PATTERNS.md has not been touched in over a day — coverage is drifting from reality"
  else
    ok "pattern ledger moves with the engine"
  fi
  grep -q "% of web" docs/loop/WEB-PATTERNS.md && ok "coverage estimates present, and marked as JUDGEMENTS the crawl corrects" || bad "no coverage estimate — the roadmap has no denominator"
else
  bad "no docs/loop/WEB-PATTERNS.md — we are tracking bugs instead of coverage, which is the unanswerable question"
fi

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
