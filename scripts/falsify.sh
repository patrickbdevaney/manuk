#!/usr/bin/env bash
# **FALSIFY — prove that each gate can actually go RED.** (METHODOLOGY Part 33; docs/loop/PROCESS.md)
#
# A gate is not "a test that passes". A gate is **a test that is known to fail when the thing it
# protects is broken.** Those are different claims and only one of them is worth anything.
#
# This exists because of a specific, recent, humiliating fact: the first version of `G_DEDUP` called
# `Page::load` — the synchronous path, which never fetches a subresource. `FETCHES` was therefore 0,
# `FETCH_DUPES` was 0, `assert_eq!(dupes, 0)` passed, and the gate reported everything was fine while
# **measuring nothing at all**. It would have been green through the entire duplicate-fetch storm it
# was written to catch. It was caught by an accounting check — by me squinting at "fetches 0" — and not
# by anything mechanical.
#
# `G_CONTAIN` has always been trustworthy for exactly one reason: it *deliberately panics a build* and
# asserts that the page dies and the process does not. **It proves itself.** Every gate should.
#
# ## How it works
#
# For each gate: apply a mutation that BREAKS the thing the gate protects, run the gate, and assert it
# goes RED. Then revert. A gate that stays GREEN under its own mutation is **vacuous** — and vacuous is
# worse than absent, because absent is honest and vacuous is trusted.
#
# ## Why it is not in the per-tick wall
#
# It builds a deliberately broken engine once per mutation. That is minutes, not seconds. It runs on the
# self-audit cadence — often enough that a gate cannot rot for long, rarely enough that it does not tax
# every tick. `scripts/self-audit.sh` fails if a gate has no falsifier declared here.
#
#   scripts/falsify.sh [gate...]      # default: all
set -uo pipefail
cd "$(dirname "$0")/.."

RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; BLD=$'\033[1m'; OFF=$'\033[0m'
PASS=0; FAIL=0
BACKUP="$(mktemp -d)"
trap 'restore_all; rm -rf "$BACKUP"' EXIT

# ── Mutation plumbing. Every mutation is applied to a real source file and reverted afterwards, so a
#    crash mid-run cannot leave the tree poisoned (see the EXIT trap).
# **The falsifier must be INCAPABLE of leaving a mutation behind.** It was not, and it poisoned the
# repository — which is the worst thing a safety tool can do.
#
# What happened: a falsify run was killed before its EXIT trap fired, leaving `MAX_TASKS_PER_DRAIN =
# u32::MAX` in `event_loop.rs`. The NEXT run then backed up that already-mutated file, mutated it again,
# and faithfully "restored" the corruption. The mutation was now committed-adjacent, in the tree, in a
# Bar 0 code path — and the next `verify.sh` hung on a genuinely broken engine, which looked for all the
# world like a real regression.
#
# Two rules now make that impossible:
#
#   1. **Refuse to start on a poisoned tree.** Every mutation carries a `MUTATION` marker. If a target
#      file already contains one, a previous run died and the tree is dirty — stop, and say so.
#   2. **Verify the restore.** After putting the file back, assert the marker is gone. A restore that
#      silently fails is the same bug one layer down.
MUTATED=()

poisoned() {  # a leftover marker means a previous run died mid-flight
  grep -l "MUTATION" "$@" 2>/dev/null | head -1
}

mutate() {  # mutate <file> <python-replacement-expression>
  local file="$1" pyexpr="$2"
  if grep -q "MUTATION" "$file" 2>/dev/null; then
    printf "%sREFUSING: %s already contains a MUTATION marker.%s\n" "$RED$BLD" "$file" "$OFF" >&2
    printf "%s  A previous falsify run died before restoring it. The tree is POISONED.%s\n" "$RED" "$OFF" >&2
    printf "%s  Restore it (git diff -- %s) before running again.%s\n" "$RED" "$file" "$OFF" >&2
    exit 2
  fi
  cp "$file" "$BACKUP/$(echo "$file" | tr / _)"
  MUTATED+=("$file")
  python3 - "$file" <<PYEOF
import sys
p = sys.argv[1]
s = open(p).read()
$pyexpr
open(p, 'w').write(s)
PYEOF
}

restore_all() {
  local f
  for f in "${MUTATED[@]:-}"; do
    [ -z "$f" ] && continue
    cp "$BACKUP/$(echo "$f" | tr / _)" "$f" 2>/dev/null
    # **Verify the restore.** A restore that silently fails is the original bug, one layer down.
    if grep -q "MUTATION" "$f" 2>/dev/null; then
      printf "%sFATAL: failed to restore %s — it still contains a MUTATION marker.%s\n" "$RED$BLD" "$f" "$OFF" >&2
      printf "%s  Fix it by hand NOW. A poisoned source tree looks exactly like a real regression.%s\n" "$RED" "$OFF" >&2
    fi
  done
  MUTATED=()
}

# ── The core assertion: with the mutation applied, the gate MUST fail.
#    A gate that survives the removal of the thing it guards is not guarding it.
# **A gate that HANGS under its mutation is RED, not stuck.**
#
# `G_RUNAWAY`'s mutation removes the task-drain ceiling, so a self-rescheduling timer loops forever —
# which is precisely the bug the gate exists to catch, and precisely what the gate then does: it hangs.
# Without a deadline here the falsifier hangs with it, and a tool that hangs is a tool nobody runs.
#
# So the run is bounded, and a timeout counts as RED — "the test never came back" is the most emphatic
# way a gate can tell you the browser no longer terminates.
FALSIFY_TIMEOUT="${MANUK_FALSIFY_TIMEOUT:-420}"

expect_red() {  # expect_red <gate-name> <command...>
  local name="$1"; shift
  printf "  %-18s " "$name"
  if timeout -k 5 "$FALSIFY_TIMEOUT" "$@" >/dev/null 2>&1; then
    printf "%sVACUOUS — it passed with the bug INSTALLED%s\n" "$RED$BLD" "$OFF"
    printf "                     %sThis gate does not test what it claims. Absent would be honest;%s\n" "$RED" "$OFF"
    printf "                     %sthis is worse, because it is trusted.%s\n" "$RED" "$OFF"
    FAIL=$((FAIL + 1))
  else
    local rc=$?
    if [ "$rc" -eq 124 ] || [ "$rc" -eq 137 ]; then
      printf "%s✓ goes red when broken%s (it HUNG — the bug, stated loudly)\n" "$GRN" "$OFF"
    else
      printf "%s✓ goes red when broken%s\n" "$GRN" "$OFF"
    fi
    PASS=$((PASS + 1))
  fi
  restore_all
}

echo "${BLD}FALSIFY — can each gate actually fail?${OFF}"
echo "  A gate that has never been proven to go red is not known to work."
echo

WANT="${*:-all}"
want() { [ "$WANT" = "all" ] || [[ " $WANT " == *" $1 "* ]]; }

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_DEDUP — break the URL-keyed image cache so the same sprite is fetched once per element naming it.
# This is the exact bug that shipped: nytimes pulled one image down for every element that mentioned it.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_DEDUP; then
  mutate engine/net/src/lib.rs '
s = s.replace("""    let gate = {
        let map = INFLIGHT.get_or_init(Default::default);""", """    let gate = {
        if true { return fetch_with_deadline(url, request_timeout()).await; }   // MUTATION
        let map = INFLIGHT.get_or_init(Default::default);""")
'
  expect_red G_DEDUP cargo test -q -p manuk-page --features stylo,spidermonkey --test g_dedup
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_LOAD — remove the page budget. A dead subresource must then hold the document hostage, and the
# gate must notice. If it does not, the "frozen tab" it was written for can come back unseen.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_LOAD; then
  # **Mutate the GUARANTEE, not one implementation of it.**
  #
  # The first version of this falsifier removed the outer `tokio::time::timeout` around
  # `finish_loading_inner` — and G_LOAD stayed green, so the falsifier reported the gate VACUOUS. It was
  # wrong. `finish_loading_inner` carries its own per-phase budget, so the guarantee survived; the outer
  # timeout is defence-in-depth, and removing one of two layers does not remove the promise.
  #
  # A weak mutation produces a FALSE "vacuous" verdict, which is a defect in the meta-gate itself — the
  # falsifier needed falsifying. So: disable the budget at its SOURCE. `load_budget()` feeds every layer,
  # and with it gone a dead subresource really can hold the document hostage, which is the thing G_LOAD
  # exists to prevent.
  mutate engine/page/src/lib.rs '
s = s.replace(
    "pub fn load_budget() -> std::time::Duration {",
    "pub fn load_budget() -> std::time::Duration {\n    #[allow(unreachable_code)]\n    return std::time::Duration::from_secs(3600);   // MUTATION: no budget at any layer",
    1)
'
  expect_red G_LOAD cargo test -q -p manuk-page --features stylo,spidermonkey --test g_load_budget
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_RUNAWAY — remove the task-drain ceiling. A self-rescheduling timer must then hang the engine.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_RUNAWAY; then
  mutate engine/js/src/event_loop.rs '
s = s.replace("MAX_TASKS_PER_DRAIN: u32 = 20_000", "MAX_TASKS_PER_DRAIN: u32 = u32::MAX;   // MUTATION\nconst _UNUSED_MUT: u32 = 0")
'
  expect_red G_RUNAWAY cargo test -q -p manuk-page --features stylo,spidermonkey --test g_runaway
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G2 — the JS conformance wall. Break `ownerDocument` back to the unrooted raw pointer that made React
# return one of our own MutationRecords after a GC. Scenario 14 allocates 60,000 objects specifically
# to force the collection that exposes it — so if this mutation does NOT go red, that scenario is not
# actually reaching the bug it was written for.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G2; then
  mutate engine/js/src/dom_bindings.rs '
s = s.replace("""    rooted!(in(cx) let global = CurrentGlobalOrNull(&wrap_cx(cx)));
    if !global.get().is_null() {
        rooted!(in(cx) let mut doc = UndefinedValue());
        if JS_GetProperty(&mut wrap_cx(cx), global.handle(), c"document".as_ptr(), doc.handle_mut())
            && doc.get().is_object()
        {
            *vp = doc.get();
            return true;
        }
    }
    *vp = NullValue();
    true
}""", """    *vp = NullValue();   // MUTATION: ownerDocument returns null
    let _ = cx;
    true
}""")
'
  expect_red G2 cargo test -q -p manuk-page --features spidermonkey -- --ignored js_conformance
fi

echo
if [ "$FAIL" -gt 0 ]; then
  echo "${RED}${BLD}FALSIFY: $FAIL gate(s) are VACUOUS — they pass with the bug installed.${OFF}"
  echo "A vacuous gate is worse than a missing one: it is trusted. Fix the GATE, not the threshold."
  exit 1
fi
echo "${GRN}${BLD}FALSIFY: all $PASS gate(s) proven falsifiable${OFF} — each one goes red when its bug is put back."
