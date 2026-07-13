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

expect_red() {  # expect_red <gate-name> <cargo-test-command...>
  local name="$1"; shift
  printf "  %-18s " "$name"

  # **A mutation that does not COMPILE is not a red gate — it is a broken falsifier.**
  #
  # `cargo test` returns non-zero for a compile error exactly as it does for a failing assertion, so a
  # typo in a mutation reads as "✓ goes red when broken" and the gate is certified by nothing at all.
  # I wrote precisely that bug: a mutation calling a function that does not exist would have "proven"
  # G_FIRST_PAINT falsifiable while testing nothing.
  #
  # So: build first, and treat a build failure as an ERROR IN THE FALSIFIER, loudly, rather than as
  # evidence about the gate. The tool that certifies the gates cannot itself be uncertified.
  if ! cargo build -q -p manuk-page --features stylo,spidermonkey --tests >/dev/null 2>&1; then
    printf "%sFALSIFIER BROKEN — the mutation does not COMPILE%s\n" "$RED$BLD" "$OFF"
    printf "                     %sThis proves nothing about the gate. A compile error and a failing%s\n" "$RED" "$OFF"
    printf "                     %sassertion are the same exit code, and that is a trap. Fix the MUTATION.%s\n" "$RED" "$OFF"
    FAIL=$((FAIL + 1))
    restore_all
    return
  fi

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
# G_FIRST_PAINT — put the image fetch back ON the paint path, which is where it used to be. The page
# must then wait for twenty black holes before it can be painted, and the gate must notice.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_FIRST_PAINT; then
  mutate engine/page/src/lib.rs '
s = s.replace(
    "            let images: HashMap<String, manuk_paint::DecodedImage> = HashMap::new();",
    "            let images: HashMap<String, manuk_paint::DecodedImage> =   // MUTATION: images back on the paint path\n                fetch_images_owned(&dom, &final_url, &std::collections::HashSet::new(), &std::collections::HashSet::new()).await.0;",
    1)
'
  expect_red G_FIRST_PAINT cargo test -q -p manuk-page --features stylo,spidermonkey --test g_first_paint
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_DEFER — make every script block paint again, which is what it did before tick 32.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_DEFER; then
  mutate engine/js/src/dom_bindings.rs '
s = s.replace(
    "            blocks_paint =\n                !is_module && el.attr(\"defer\").is_none() && el.attr(\"async\").is_none();",
    "            blocks_paint = true;   // MUTATION: every script blocks paint again",
    1)
'
  expect_red G_DEFER cargo test -q -p manuk-page --features stylo,spidermonkey --test g_defer
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_FORM — ignore preventDefault() on submit, which is what we did before tick 34. Every AJAX form on
# the web then performs the navigation its author cancelled.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_FORM; then
  mutate engine/page/src/lib.rs '
s = s.replace(
    "        // The handler may have re-rendered the page (that is the entire point of intercepting submit).\n        self.relayout(fonts, viewport_width);\n        proceed",
    "        self.relayout(fonts, viewport_width);\n        let _ = proceed;\n        true   // MUTATION: navigate anyway, ignoring preventDefault()",
    1)
'
  expect_red G_FORM cargo test -q -p manuk-page --features stylo,spidermonkey --test g_form
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_IFRAME — take `iframe` back out of the replaced-element list, which is where it was NOT.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_IFRAME; then
  mutate engine/css/src/stylo_engine.rs '
s = s.replace(
    "        if tag == \"iframe\" {\n            if s.width == crate::Dim::Auto {\n                s.width = crate::Dim::Px(300.0);\n            }",
    "        if false {   // MUTATION: no iframe default size\n            if s.width == crate::Dim::Auto {\n                s.width = crate::Dim::Px(300.0);\n            }",
    1)
'
  expect_red G_IFRAME cargo test -q -p manuk-page --features stylo,spidermonkey --test g_iframe
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_ANIMATION — render the animation's FIRST frame again, which is what made a fifth of the web invisible.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_ANIMATION; then
  mutate engine/css/src/stylo_map.rs '
s = s.replace(
    "    if s.has_animation && s.opacity == 0.0 {\n        s.opacity = 1.0;\n    }",
    "    if false {   // MUTATION: render the animation first frame, hiding the content\n        s.opacity = 1.0;\n    }",
    1)
'
  expect_red G_ANIMATION cargo test -q -p manuk-page --features stylo,spidermonkey --test g_animation
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_LIFECYCLE — throw the setTimeout DELAY away again. That is what the engine did for 40 ticks: every
# timer was a FIFO push, so `setTimeout(f, 10000)` ran BEFORE a `setTimeout(g, 0)` queued after it —
# and testharness's own 10s harness timeout fired before the tests it was guarding, so 100% of WPT
# reported TIMEOUT. It never errors; it just happens in the wrong order, silently, on every debounce
# and retry-backoff on the web.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_LIFECYCLE; then
  mutate engine/js/src/event_loop.rs '
s = s.replace(
    "__tasks.push({ f: fn, w: __now + ms, s: ++__seq });",
    "__tasks.push({ f: fn, w: __now, s: ++__seq });   // MUTATION: throw the DELAY away",
    1)
'
  expect_red G_LIFECYCLE cargo test -q -p manuk-page --features stylo,spidermonkey --test g_lifecycle
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_LIFECYCLE (load half) — stop firing the `load` event. For forty ticks this engine dispatched
# NEITHER `DOMContentLoaded` NOR `load`, anywhere, and every site whose init lived in an onload
# handler simply never initialised — in silence, with nothing in any log to say so.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_LIFECYCLE_LOAD; then
  mutate engine/page/src/lib.rs '
s = s.replace(
    "        page.fire_lifecycle(\"load\");",
    "        // MUTATION: never fire load on the sync path",
    1)
'
  expect_red G_LIFECYCLE_LOAD cargo test -q -p manuk-page --features stylo,spidermonkey --test g_lifecycle
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_SELECTOR — stop descending into nested rules again, which is what dropped 41% of the web's CSS.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_SELECTOR; then
  # Stop descending into nested rules — which dropped 41% of the web's CSS.
  mutate engine/css/src/stylo_engine.rs '
s = s.replace(
    "                        self.add_rules(&nested.0, guard, device, order);",
    "                        let _ = &nested;   // MUTATION: stop descending into nested rules",
    1)
'
  expect_red G_SELECTOR cargo test -q -p manuk-page --features stylo,spidermonkey --test g_selector
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_SELECTOR (:has half) — disable the supplement pass. Stylo DISCARDS `:has()` rules at parse, so
# without our own pass they do not exist at all. 13% of the corpus.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_HAS; then
  mutate engine/css/src/stylo_engine.rs '
s = s.replace(
    "    let has_sheets: Vec<&Stylesheet> = sheets.iter().filter(|sh| sh.has_relative_rules()).collect();",
    "    let has_sheets: Vec<&Stylesheet> = Vec::new();   // MUTATION: no :has() supplement\n    let _ = sheets;",
    1)
'
  expect_red G_HAS cargo test -q -p manuk-page --features stylo,spidermonkey --test g_selector
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_SILENT_FAIL — unregister the promise-rejection tracker. Every async framework error then goes
# nowhere, which is exactly how "React mounts, throws nothing, renders nothing" happened.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_SILENT_FAIL; then
  mutate engine/js/src/dom_bindings.rs '
s = s.replace(
    "        mozjs::jsapi::SetPromiseRejectionTrackerCallback(\n            raw_cx,\n            Some(promise_rejection_tracker),\n            std::ptr::null_mut(),\n        );",
    "        // MUTATION: nothing listens for unhandled rejections\n        let _ = promise_rejection_tracker as *const ();",
    1)
'
  expect_red G_SILENT_FAIL cargo test -q -p manuk-page --features stylo,spidermonkey --test g_silent_fail
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_RUNTIME_COUNT — build a second async runtime. One per process is the promise (Part 25.2); the
# shell was building two, and that is what this gate exists to keep at one.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_RUNTIME_COUNT; then
  mutate engine/net/src/lib.rs '
s = s.replace(
    "pub fn runtime() -> &\x27static tokio::runtime::Runtime {",
    "pub fn runtime() -> &\x27static tokio::runtime::Runtime {\n    // MUTATION: count a fresh runtime on every call\n    RUNTIME_INSTANTIATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);",
    1)
'
  expect_red G_RUNTIME_COUNT cargo test -q -p manuk-shell runtime_instantiations
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_ALLOC — remove the "ask first" guard, so every scroll rebuilds the rect map for a page that is not
# listening. Sixty times a second, on the UI thread. That was the wheel-event freeze.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_ALLOC; then
  mutate engine/page/src/lib.rs '
s = s.replace(
    "        if !manuk_js::wants_view_events(ctx) {\n            return;\n        }",
    "        // MUTATION: do the work even when nobody is listening\n        let _ = manuk_js::wants_view_events(ctx);",
    1)
'
  expect_red G_ALLOC cargo test -q -p manuk-page --features spidermonkey --test g_alloc -- --ignored
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_TEARDOWN — put a `process::exit` back in the shell. It skips every destructor, so the profile
# (cookies, localStorage, session) is never flushed: a data-loss bug wearing a crash-fix disguise.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_TEARDOWN; then
  # **Mutate a file the gate actually SCANS.** The first version of this falsifier put the
  # `process::exit` in `shell/src/tab.rs` — and the gate reported VACUOUS, because it deliberately scans
  # only the SHIPPING exit paths (`src/main.rs`, `src/gui.rs`). The gate's scope was right and the
  # mutation was wrong: a weak mutation produces a false "vacuous" verdict, which is PROCESS #9, again.
  mutate shell/src/gui.rs '
s = s.replace(
    "use ",
    "#[allow(dead_code)]\nfn _mutation_exit() { std::process::exit(0); }   // MUTATION: skips the profile flush\nuse ",
    1)
'
  expect_red G_TEARDOWN cargo test -q -p manuk-shell --test g_teardown
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G6 — clickability. Break HIT-TESTING, which is what the gate actually guards. A link the browser
# cannot find under the cursor is a link the user cannot click, and no box-comparing gate can see it.
#
# My first mutation emptied `Page::links()` and the falsifier reported VACUOUS — because `hittest`
# enumerates links from the DOM, not from that function. The mutation was wrong. **But the check that
# proved it wrong also found a REAL hole:** with zero links, `MISSED` is 0 and the gate passes. A browser
# that finds NOTHING scored perfectly. `verify.sh` now refuses fewer than 50 links as vacuous.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G6; then
  mutate engine/a11y/src/lib.rs '
s = s.replace(
    "    pub fn hit_test(&self, x: f32, y: f32) -> Option<&A11yNode> {",
    "    pub fn hit_test(&self, x: f32, y: f32) -> Option<&A11yNode> {\n        if true { let _ = (x, y); return None; }   // MUTATION: nothing is clickable\n        #[allow(unreachable_code)]",
    1)
'
  expect_red G6 bash -c '
    curl -sL "${MANUK_CLICK_URL:-https://en.wikipedia.org/wiki/Terrier}" -o /tmp/manuk-g6f.html || exit 1
    M=$(cargo run -q -p manuk-wpt --release -- hittest --html /tmp/manuk-g6f.html \
          --url "${MANUK_CLICK_URL:-https://en.wikipedia.org/wiki/Terrier}" 2>/dev/null \
        | grep -oE "MISSED \(unclickable\): [0-9]+" | grep -oE "[0-9]+$")
    [ "${M:-99}" -le 5 ]'
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_INTERACT — stall the UI thread inside the tab-open path itself. "The browser feels laggy" is a
# report that no box-comparing gate can ever produce, which is exactly why this gate exists.
#
# The FIRST version of this mutation added a `_mutation_stall()` function that nothing called, and the
# falsifier duly reported the gate VACUOUS. It was not: the mutation was. A weak mutation produces a
# false verdict — PROCESS #9, for the third time. Mutate the path the gate actually drives.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_INTERACT; then
  mutate shell/src/tab.rs '
s = s.replace(
    "    pub fn open(&mut self, url: impl Into<String>) -> TabId {",
    "    pub fn open(&mut self, url: impl Into<String>) -> TabId {\n        std::thread::sleep(std::time::Duration::from_millis(50));   // MUTATION: stall the UI thread",
    1)
'
  expect_red G_INTERACT cargo test -q -p manuk-shell tab_operations
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G3 — affordances. Give a control an EMPTY effect: a button in the UI that does nothing when pressed.
# "No dead buttons" is the promise, and a dead button is invisible to every other gate here.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G3; then
  mutate shell/src/chrome.rs '
import re
s = re.sub(
    r"const AFFORDANCES: &\[\(&str, &str\)\] = &\[\n",
    "const AFFORDANCES: &[(&str, &str)] = &[\n        (\"mutation-dead-button\", \"\"),   // MUTATION: a control that does nothing\n",
    s, count=1)
'
  expect_red G3 cargo test -q -p manuk-shell affordance
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G1 — real-site fidelity vs Chromium. Its floor is applied to the **STRUCTURAL** score: *what fraction
# of what Chrome renders do we render AT ALL*. So the mutation must make us render FEWER elements.
#
# My first attempt painted every page on a black canvas — and the falsifier reported VACUOUS. It was not:
# a black background changes no element's existence, so the structural score stayed at 100%. The gate's
# floor was right and the mutation was aimed at the wrong property. Third time a weak mutation has
# produced a false "vacuous" verdict (PROCESS #9), and the third time it was the mutation that was wrong.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G1; then
  mutate engine/layout/src/lib.rs '
s = s.replace(
    "    pub fn node_rects(&self, dom: &Dom) -> std::collections::HashMap<NodeId, Rect> {",
    "    pub fn node_rects(&self, dom: &Dom) -> std::collections::HashMap<NodeId, Rect> {\n        if true { let _ = dom; return Default::default(); }   // MUTATION: the page renders nothing\n        #[allow(unreachable_code)]",
    1)
'
  expect_red G1 cargo run -q -p manuk-wpt --release -- fidelity --urls "https://news.ycombinator.com" --out /tmp/manuk-fid-f --floor 0.75
fi

# ─────────────────────────────────────────────────────────────────────────────────────────────────
# G_GLOBALS — take WebSocket back out. That single absence turned a 2,591-element news front page into
# a 141-element skeleton, because the constructor threw inside React's render.
# ─────────────────────────────────────────────────────────────────────────────────────────────────
if want G_GLOBALS; then
  mutate engine/js/src/event_loop.rs '
s = s.replace(
    "      if (typeof globalThis.WebSocket === \x27undefined\x27) {",
    "      if (false) {   // MUTATION: WebSocket does not exist",
    1)
'
  expect_red G_GLOBALS cargo test -q -p manuk-page --features stylo,spidermonkey --test g_globals
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
