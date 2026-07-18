#!/usr/bin/env bash
# The VERIFY wall (ADR-010). Every tick runs this — one command, so a tick cannot "forget" a gate.
#
# Gates:
#   B  build            workspace compiles
#   P  parity           72/72 box probes vs headless Chrome (§1.1)
#   G2 js-conformance   the DOM/BOM surface real sites need (grows every JS tick)
#   G3 affordances      every user-reachable control has an observable effect (§1.8)
#   F  perf floors      EPOCH-1 F1/F2/F3 (§1.7) — measured, binding
#   T  crate tests      touched crates
#
# Usage: scripts/verify.sh [--fast]     (--fast skips the perf floors, which need a page corpus)
set -uo pipefail
cd "$(dirname "$0")/.."

# Snapshot the tree before doing anything. PROCESS #37: `git checkout` is a delete, and I have
# run it on uncommitted work twice. This makes that recoverable. It is free when nothing has changed.
./scripts/snap.sh || true

# ── OOM GUARD. `ld terminated with signal 9` is the OOM killer and it looks EXACTLY like a compile
# error — cargo returns non-zero and every wrapper reads it as "the code is broken". It has already
# produced a false FALSIFIER BROKEN verdict (PROCESS #31). Derive the job count from AVAILABLE MEMORY,
# not from `nproc`, because `nproc` knows nothing about LLVM's ~2GB-per-codegen-job peak.
# shellcheck source=/dev/null
. "$(dirname "$0")/mem-guard.sh"

# ── THE PARALLEL GATE RUNNER. Same gates, same assertions, same output — concurrently.
#
# The wall ran ~21 INDEPENDENT `cargo test` invocations strictly one after another. They are separate
# processes that share nothing (each JS gate even stands up its own SpiderMonkey runtime, which is most of
# its ~1.5s), so serialising them bought exactly nothing and cost the tick a minute.
#
# **Nothing is weakened: every gate still runs, still asserts the same thing, and still reports in the same
# order.** Only the *waiting* is removed.
#
# ⚠ TWO things must NOT be parallelised, and they are the reason this is a launcher rather than a blanket
# `xargs -P`:
#   * **The perf floors.** *A benchmark that shares a machine with a compile is not a benchmark* — this
#     project's own rule (PROCESS: the crawl that read 12.5% → 49% on the same binary because the job count
#     changed). They run LAST, alone, after every background job has been reaped.
#   * **Anything that measures time.** Same reason.
_PARDIR="$(mktemp -d)"
declare -A _PARPID
trap 'rm -rf "$_PARDIR"' EXIT

# Cap concurrency by the same memory-derived budget as the compiler: SpiderMonkey gates are not free.
_PARMAX="${CARGO_BUILD_JOBS:-4}"
_launch() {  # _launch <key> <cmd...>
  while [ "$(jobs -rp | wc -l)" -ge "$_PARMAX" ]; do wait -n 2>/dev/null || break; done
  local key="$1"; shift
  ( "$@" >"$_PARDIR/$key" 2>&1 ) &
  _PARPID[$key]=$!
}
_out() {  # _out <key> → the command's combined output, once it has finished
  local key="$1"
  [ -n "${_PARPID[$key]:-}" ] && wait "${_PARPID[$key]}" 2>/dev/null
  local out; out=$(cat "$_PARDIR/$key" 2>/dev/null)

  # ── A GATE THAT COULD NOT BUILD IS NOT A GATE THAT FAILED.
  #
  # PROCESS #46 taught this to the crate-suite loop and NOT to these parallel gates — so the very next
  # time the build broke (a dangling ramdisk symlink: `failed to create directory target/debug/incremental`)
  # the wall announced **"G_RUNTIME_COUNT failed — runtimes are proliferating"** and **"G_INTERACT failed —
  # a tab operation stalls the UI thread"**. Both were green in isolation one second later. The engine was
  # fine; cargo could not create a directory.
  #
  # **A lesson learned in one instrument is not learned until it is applied to the others** — and this is
  # the FOURTH instrument that has had to learn it (WPT's SHORT-vs-CRASH, the crate suite, the sweep, now
  # the parallel gates). So: a build failure is reported AS a build failure, loudly, and never dressed up
  # as a verdict about the engine.
  if echo "$out" | grep -qE "^error: (could not compile|failed to|couldn't)"; then
    # Literal escapes, NOT $RED/$BLD/$OFF — those were never defined in this script, so under `set -u`
    # this very branch (the one that exists to report a build failure HONESTLY) died on an unbound
    # variable, aborted the `$(_out …)` substitution, and handed the gate an EMPTY result — which the
    # caller then read as the gate FAILING. A transient parallel-build hiccup thus false-RED'd a green
    # tick (tick 113). The honest-reporting path must not itself be the thing that lies. [no-pattern]
    printf '  \033[31m\033[1mBUILD FAILED for gate %s — this is NOT a verdict about the engine:\033[0m\n' \
      "$key" >&2
    echo "$out" | grep -E "^(error|Caused by):" | head -4 | sed 's/^/    /' >&2
  fi
  printf '%s' "$out"
}

# Colors used by the error/retry branches below (were unbound under set -u — the harness bug the
# agent flagged: a RED wall crashed while trying to print its own error). Defined once here.
BLD=$'\033[1m'; YEL=$'\033[33m'; OFF=$'\033[0m'

# ── PRE-WARM (observer, tick 118). Build each distinct test unit ONCE, serially, BEFORE the parallel gate
# launch below. Without this, the ~26 gates each invoke `cargo test` on a cold cache and contend on cargo's
# build-directory LOCK — they cannot build in parallel, they queue on the lock, and the isolated shell gates
# (G_RUNTIME_COUNT/G_INTERACT) intermittently false-RED under that race. The agent then re-runs the whole
# wall 3-4x to prove it was a race and not a regression — ~40 min/tick of pure waste, and a flaky gate that
# teaches the loop to distrust a RED. Building the four units up front means every gate below only *runs* its
# already-linked binary, so the runs genuinely parallelise and the build-race false-RED cannot occur. Warm
# runs are a no-op. No gate semantics change — same binaries, same tests, built once instead of raced. A real
# build failure is NOT masked: `|| true` only skips the warm-up; the gate (or the workspace build) still REDs.
for _pw in \
  "manuk-page --features spidermonkey" \
  "manuk-page --features stylo,spidermonkey" \
  "manuk-shell" \
  "manuk-dom"; do
  cargo test --no-run -q -p $_pw >/dev/null 2>&1 || true
done

# Launch every independent gate NOW; each block below simply collects its result.
_launch js cargo test -q -p manuk-page --features spidermonkey -- --ignored js_conformance
_launch ga cargo test -q -p manuk-page --features spidermonkey --test g_alloc -- --ignored
# ── SHELL GATES AS ONE INVOCATION (observer, tick 118). The manuk-shell gates — affordance, teardown,
# runtime-count, tab-interact — used to run as FOUR concurrent `cargo test -p manuk-shell` processes. Under
# contention those four shells fought over the process/runtime accounting and G_RUNTIME_COUNT false-RED'd
# ("runtimes are proliferating") — a flaky gate that thrashed the loop for ~40 min/tick. Run alone each test
# passes; run as the FULL suite in ONE process they pass reliably (the agent confirmed 49 ok). So: one job,
# all shell tests, checked for ZERO failures below (nothing masked), and faster than four separate builds.
_launch shell cargo test -q -p manuk-shell -- --nocapture
_launch glb cargo test -q -p manuk-page --features stylo,spidermonkey --test g_load_budget
_launch gg cargo test -q -p manuk-page --features stylo,spidermonkey --test g_globals
_launch gau cargo test -q -p manuk-dom pointer_width
_launch gvp cargo test -q -p manuk-page --features stylo,spidermonkey --test g_viewport
_launch gdi cargo test -q -p manuk-page --features stylo,spidermonkey --test g_dom_impl
_launch gcn cargo test -q -p manuk-page --features stylo,spidermonkey --test g_contain_native
_launch gsn cargo test -q -p manuk-dom stale_handle
_launch gcd cargo test -q -p manuk-page --features stylo,spidermonkey --test g_chardata
_launch gdoc cargo test -q -p manuk-page --features stylo,spidermonkey --test g_doc_collections
_launch gl cargo test -q -p manuk-page --features stylo,spidermonkey --test g_lifecycle
_launch gsl cargo test -q -p manuk-page --features stylo,spidermonkey --test g_selector
_launch gan cargo test -q -p manuk-page --features stylo,spidermonkey --test g_animation
_launch gif cargo test -q -p manuk-page --features stylo,spidermonkey --test g_iframe
_launch gfm cargo test -q -p manuk-page --features stylo,spidermonkey --test g_form
_launch gdf cargo test -q -p manuk-page --features stylo,spidermonkey --test g_defer
_launch gf cargo test -q -p manuk-page --features stylo,spidermonkey --test g_first_paint
_launch gs cargo test -q -p manuk-page --features stylo,spidermonkey --test g_silent_fail
_launch gd cargo test -q -p manuk-page --features stylo,spidermonkey --test g_dedup
_launch gra cargo test -q -p manuk-page --features stylo,spidermonkey --test g_runaway
_launch gc cargo test -q -p manuk-page --features stylo,spidermonkey --test g_contain
# (G_RUNTIME_COUNT + G_INTERACT are part of the single `_launch shell` above — observer tick 118)


FAIL=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31m✗ %s\033[0m\n' "$1"; FAIL=1; }
# head_ also records a per-section timing breakdown to `.git/manuk-wall-sections`, so the wall-time audit
# (scripts/wall-audit.sh, on a sparse cadence) can see WHERE the wall spends its seconds and hunt bloat
# without cutting any gate's rigor. Costs nothing on the hot path (one echo of an integer).
_WALL_SECTIONS=".git/manuk-wall-sections"
: > "$_WALL_SECTIONS" 2>/dev/null || true
_WALL_LAST=$SECONDS
_WALL_PREV_NAME=""
head_() {
  if [ -n "$_WALL_PREV_NAME" ]; then
    printf '%s\t%s\n' "$((SECONDS - _WALL_LAST))" "$_WALL_PREV_NAME" >> "$_WALL_SECTIONS" 2>/dev/null || true
  fi
  _WALL_LAST=$SECONDS
  _WALL_PREV_NAME="${1%% ·*}"
  printf '\n\033[1m%s\033[0m\n' "$1"
}

# **RAM-based iterative builds, ALWAYS ON (not just when the disk is already full).** The incremental
# compile fragments — the bulk of what a rebuild rewrites, over and over, during ordinary edit→build→test
# — belong in `/dev/shm`, so local iteration almost never touches the platter. This used to run only
# inside the `disk >= 88%` reclaim branch, i.e. *reactively*, which meant every build between walls wrote
# incrementals to disk until the disk was nearly full. Reseating here, unconditionally and idempotently,
# means the RAM symlinks are in place before this build AND for every iterative build after it. Cheap
# (a `mkdir` + symlink check) and it must run BEFORE the compile (a dangling tmpfs symlink fails ENOENT).
# The operator can activate the same setup any time with `./scripts/ramdisk.sh`.
./scripts/ramdisk.sh >/dev/null 2>&1 || true

# Disk hygiene is a gate, not a chore. A full disk is a build failure that looks like a code failure, and
# this tree grows tens of gigabytes a week. RECLAIM is still conditional (it deletes the debug cache, which
# a warm wall wants to keep) — only the RAM setup above is unconditional.
PCT=$(df /home 2>/dev/null | awk 'NR==2 {gsub(/%/,""); print $5}')
if [ -n "${PCT:-}" ] && [ "$PCT" -ge 88 ]; then
  head_ "D · disk (${PCT}% full — reclaiming before the build fails on ENOSPC)"
  bash scripts/disk-hygiene.sh | sed 's/^/  /'
fi

head_ "B · build (workspace)"
if cargo build -q --workspace 2>&1 | grep -qE '^error'; then bad "workspace does not compile"; else ok "workspace compiles (shipping)"; fi
# HEADLESS GATE (observer, tick 186). The `--no-default-features` lane broke CI on EVERY OS (E0433: the
# GUI-feature type `DownloadRecord` was referenced from an always-compiled module) and the wall never built
# headless — "headless is CI's job, not the wall's" (tick 88) — so it slipped through silently. Catch that
# class here: manuk-shell is where the `#[cfg(feature="gui")]` split lives, so a headless check there is
# cheap (~5s warm) and covers the likely source. CI still runs the full `--workspace --no-default-features`.
if cargo check -q -p manuk-shell --no-default-features 2>&1 | grep -qE '^error'; then bad "headless (--no-default-features) broken — a gui-feature item referenced from always-compiled code?"; else ok "headless compiles (--no-default-features)"; fi
# **The headless lane (`--no-default-features`) is CI's job, not the wall's.** Tick 88 added it here to
# stop a headless-only regression (tick 84's `diag`) from passing a green wall — a real gap — but a third
# feature configuration taxed EVERY wall ~350s via cargo cache thrash, and the wall runs every tick. CI's
# `verify-linux` builds the headless config authoritatively and is now green; the loop reads CI at the
# start of each tick (the methodology's own rule). So the cheap, correct division of labour is: the wall
# proves the shipping config in seconds, CI proves the headless config out-of-band. The `diag` fix (gating
# it behind `spidermonkey`) stays — that is what actually fixed CI. See tick 88/89.

head_ "P · parity (§1.1 — 72/72 vs headless Chrome)"
PAR=$(cargo run -q -p manuk-wpt --release -- parity 2>&1 | tail -1)
if echo "$PAR" | grep -q "72/72"; then ok "$PAR"; else bad "$PAR"; fi

head_ "G1 · real-site visual fidelity vs Chromium (ADR-010/011 — SHIPPING config)"
# `example.com` was here and it has NO `[id]` elements — so it probed NOTHING, scored a perfect 100%,
# and inflated the mean of the gate whose job is to catch missing content. Mutation-testing found it.
# Every URL in this list must be one the gate can actually measure.
# ── THE SNAPSHOT CACHE. It makes this gate FASTER AND MORE CORRECT AT THE SAME TIME.
#
# This gate used to fetch two LIVE sites on every single tick — 25.5 of the wall's 92 seconds, and by far
# its largest single cost. Worse, it broke the project's own first rule of differential measurement:
#
#     ONE SNAPSHOT, BOTH ENGINES.
#
# A live page changes between runs, so the gate was comparing today's Manuk against today's Chrome against
# *yesterday's implicit baseline* — and a fidelity number that moves because a news site published an
# article is a number that cannot be trusted to have moved because of the code. (This project has already
# been burned by exactly that: a metric stuck at 5,122px across four correct fixes, because the two engines
# were fed two different documents.)
#
# So: fetch ONCE into a cache, feed the identical bytes to both engines forever after, and refresh
# deliberately on the audit cadence (`rm -rf .verify-cache`) rather than accidentally on every tick.
# **Determinism is not a side-effect of the speed-up here; it is the point, and the speed-up is the bonus.**
CACHE=".verify-cache"
mkdir -p "$CACHE"
_snapshot() {  # _snapshot <url> <name> → echoes a file:// URL for the cached copy
  # **Separate `local` statements — this is not style, it is the bug.** `local url="$1" name="$2"
  # f="$CACHE/$name.html"` expands every RHS *before* any assignment, so `$name` is unbound when `f` is
  # built, and under `set -u` the whole subshell dies. That silently broke the snapshot cache: the fetch
  # never cached, so EVERY wall re-fetched two live sites (~534s vs ~58s), and the WALL ratchet ceiling
  # failed a tick whose engine never changed. The instrument was measuring its own broken caching.
  local url="$1" name="$2"
  local f="$CACHE/$name.html"
  if [ ! -s "$f" ]; then
    curl -sL --max-time 30 -A "Mozilla/5.0 manuk-verify" "$url" -o "$f" 2>/dev/null || true
  fi
  [ -s "$f" ] && printf 'file://%s/%s' "$PWD" "$f" || printf '%s' "$url"   # fall back to live if offline
}
if [ -z "${MANUK_FIDELITY_URLS:-}" ]; then
  G1URLS="$(_snapshot https://news.ycombinator.com hn),$(_snapshot https://en.wikipedia.org/wiki/Terrier wiki)"
else
  G1URLS="$MANUK_FIDELITY_URLS"
fi
G1FLOOR="${MANUK_FIDELITY_FLOOR:-0.75}"
G1OUT="${MANUK_FIDELITY_OUT:-/tmp/manuk-fidelity}"
if cargo run -q -p manuk-wpt --release -- fidelity --urls "$G1URLS" --out "$G1OUT" --floor "$G1FLOOR" >/tmp/manuk-g1.txt 2>&1; then
  ok "$(grep 'MEAN FIDELITY' /tmp/manuk-g1.txt || echo 'fidelity ok')"
  printf '    side-by-side composites in %s — LOOK at them\n' "$G1OUT"
else
  bad "real-site fidelity below floor ($G1FLOOR) — see $G1OUT"; grep -E 'BELOW|MEAN' /tmp/manuk-g1.txt | sed 's/^/    /'
fi

head_ "G2 · JS conformance (ADR-010 — the DOM/BOM surface real sites need)"
JS=$(_out js | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$JS" ]; then ok "js conformance: $JS"; else bad "JS conformance suite did not pass"; fi

head_ "G3 · affordance completeness (§1.8 — no dead buttons)"
# The whole manuk-shell suite is one job now: capture it ONCE and count failures, so a regression in ANY
# shell test surfaces (nothing masked by a `grep ok | head -1`). G_TEARDOWN/G_RUNTIME_COUNT/G_INTERACT below
# read this same captured output.
SHELL_OUT=$(_out shell); SHELL_FAILED=$(printf '%s' "$SHELL_OUT" | grep -c 'test result: FAILED')
# ── FLAKY-GATE RETRY (observer). The shell suite carries the timing gates G_INTERACT (UI-thread latency
# <16ms) and G_RUNTIME_COUNT, which measure PERFORMANCE and false-RED under the CPU contention of the ~25
# gate builds launched in parallel above (confirmed tick 188: the suite passed 58/58 in the per-crate run
# while this parallel invocation reported FAILED, costing the agent a full-wall re-run). A REAL regression
# fails deterministically; a contention false-RED clears on a quiet machine. So on a reported failure, drain
# the other gates and re-run the suite ONCE, serially, alone — nothing is masked (a real regression still
# fails the quiet re-run), and this ends the flaky-gate re-run thrash instead of teaching the loop to
# distrust a RED.
if [ "$SHELL_FAILED" -ne 0 ]; then
  wait 2>/dev/null || true
  SHELL_OUT=$(cargo test -q -p manuk-shell -- --nocapture 2>&1)
  SHELL_FAILED=$(printf '%s' "$SHELL_OUT" | grep -c 'test result: FAILED')
fi
AFF=$(printf '%s' "$SHELL_OUT" | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ "$SHELL_FAILED" -eq 0 ] && [ -n "$AFF" ]; then ok "affordances (full shell suite green): $AFF"; else bad "affordance gate failed — a control may be dead, or another manuk-shell test regressed"; fi

head_ "G6 · clickability (a link the browser cannot find is a link the user cannot click)"
G6URL="${MANUK_CLICK_URL:-https://en.wikipedia.org/wiki/Terrier}"
G6HTML="/tmp/manuk-g6.html"
if curl -sL --max-time 30 "$G6URL" -o "$G6HTML" 2>/dev/null && [ -s "$G6HTML" ]; then
  CLK=$(cargo run -q -p manuk-wpt --release -- hittest --html "$G6HTML" --url "$G6URL" 2>/dev/null | grep -E "CLICKABILITY|MISSED|links on page")
  MISS=$(echo "$CLK" | grep -oE 'MISSED \(unclickable\): [0-9]+' | grep -oE '[0-9]+$' || echo 0)
  PCT=$(echo "$CLK" | grep -oE 'CLICKABILITY: [0-9.]+' | grep -oE '[0-9.]+' || echo 0)
  # **The gate must have MEASURED something.** `MISSED` is 0 when the page has no links at all — so a
  # browser that finds NOTHING scores a perfect clickability of 0-missed and sails through. That is the
  # same vacuity `G_DEDUP` shipped with (PROCESS #7), and it was found here by mutation-testing.
  TOTAL=$(echo "$CLK" | grep -oE 'links on page: +[0-9]+' | grep -oE '[0-9]+$' || echo 0)
  if [ "${TOTAL:-0}" -lt 50 ]; then
    bad "G6 is VACUOUS: only ${TOTAL:-0} links found on the page, so 'MISSED: 0' proves nothing. A browser that finds NO links scores a perfect clickability. Fix the harness, not the threshold."
  elif [ "${MISS:-99}" -le 5 ]; then ok "clickability ${PCT}% (${MISS} unclickable of ${TOTAL} links)"; else bad "clickability ${PCT}% — ${MISS} links the browser cannot find"; fi
else
  printf '  \033[33m—\033[0m could not fetch %s (skipped)\n' "$G6URL"
fi

head_ "F4 · interactive latency (§1.7 — one frame; the load bench is BLIND to this)"
if [ -s "$G6HTML" ]; then
  cargo run -q -p manuk-wpt --release -- bench --interactive --pages "$G6HTML" --runs 5 2>/dev/null | grep -E "^manuk-g6|OVER ONE FRAME" | sed 's/^/  /'
  printf '  \033[33m!\033[0m scroll and click must each stay under 16ms — a browser that loads fast and\n'
  printf '    then stutters on every wheel event is not fast, and G1/G2/G3 cannot see it.\n'
fi

head_ "G_ALLOC · allocation rate per input event (METHODOLOGY 5.2 — the load bench is BLIND to this)"
GA=$(_out ga | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GA" ]; then ok "per-event allocation: $GA"; else bad "G_ALLOC failed — an input event allocates per DOM node"; fi

head_ "G_TEARDOWN · no exit path bypasses Drop (METHODOLOGY 5.3 — a hidden crash is a data-loss bug)"
if [ "${SHELL_FAILED:-1}" -eq 0 ]; then ok "teardown: manuk-shell suite green"; else bad "G_TEARDOWN failed — an exit path skips the profile flush (or another shell test regressed)"; fi

head_ "G_LOAD · the page renders when its subresources never answer (METHODOLOGY 4.1 — the frozen tab)"
GL=$(_out glb | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GL" ]; then ok "load budget: $GL"; else bad "G_LOAD failed — a dead subresource can hold the document hostage"; fi

head_ "G_GLOBALS · a missing constructor is a THROWN EXCEPTION, not a missing feature"
# WebSocket's absence took an entire news front page down: aljazeera's 2,591 server-rendered elements
# became 141, because a live-blog client constructed one at boot and React's error boundary showed a
# skeleton. Fixing it revealed Blob; fixing Blob revealed FileList. They come in a long tail.
GG=$(_out gg | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GG" ]; then ok "globals: $GG"; else bad "G_GLOBALS failed — a global a real bundle references is missing, or one of them is LYING"; fi

head_ "G_ARENA_U64 · the arena handle is pointer-width-INDEPENDENT (wasm + ARM)"
# NodeId packs generation<<32 | index. Backed by usize, it OVERFLOWS on a 32-bit target (wasm32) — the
# crate does not even compile, which the in-browser demo's wasm build surfaced. u64 is identical to usize
# on 64-bit and correct on 32-bit. This test pins the packing so a future "simplify back to usize" cannot
# silently reintroduce the overflow. It also matters for the ARM/cross-platform target.
GAU=$(_out gau | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GAU" ]; then ok "arena u64: $GAU"; else bad "G_ARENA_U64 failed — the arena handle is not pointer-width-independent and will not compile on wasm32"; fi

head_ "G_VIEWPORT · the live viewport, and the whole lazy-load loop it unlocks"
# ONE primitive blocks FIVE features: lazy-load, virtualization, sticky, scroll-linked animation, infinite
# scroll. The platform map called it "the single biggest breadth-per-tick item on the board" — and it was
# ALREADY BUILT, with nothing proving it. A capability with no gate is indistinguishable from one that does
# not exist. The gate asserts the COMPLETE loop: viewport moves -> IntersectionObserver FIRES -> the
# callback sets img.src from data-src -> AND THE ENGINE QUEUES THAT URL FOR FETCHING. The last step is the
# one everybody forgets: firing the observer is not the feature; the image ARRIVING is the feature.
GVP=$(_out gvp | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GVP" ]; then ok "viewport + lazy-load: $GVP"; else bad "G_VIEWPORT failed — the viewport moved and nothing heard it, or the observer fired and the image never arrived"; fi

head_ "G_DOM_IMPL · createHTMLDocument + pre-insertion validity (a cycle is a HANG)"
# createHTMLDocument() is how DOMPurify builds a safe DETACHED document; its absence is a TypeError that
# takes the sanitizer down (WPT failed 488x on documentElement downstream of it). And inserting a node into
# its own descendant makes the tree a CYCLE — an infinite children() walk, i.e. a hang (Bar 0) — which
# pre-insertion validity must throw HierarchyRequestError for, not spin on.
GDI=$(_out gdi | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GDI" ]; then ok "dom impl + insertion validity: $GDI"; else bad "G_DOM_IMPL failed — createHTMLDocument, or the cycle check that stands between the DOM and an infinite loop"; fi

head_ "G_CONTAIN_NATIVE · a panic in a JS native must kill the CALL, not the browser (Bar 0)"
# Every DOM method is an `extern "C"` function, and `extern "C"` is NOUNWIND. A Rust panic inside one is
# "panic in a function that cannot unwind" -> SIGSEGV, core dumped: the whole browser, and every tab the
# user had open, because one page hit one bad index. Tick 46 found this for real via WPT.
#
# THE TRAP, and it is the whole lesson: wrapping an `extern "C"` fn in catch_unwind from the OUTSIDE does
# NOTHING — the panic aborts at that function's own nounwind boundary before any outer catch is reached.
# The catch must be INSIDE the extern "C" frame. So every native is a plain Rust fn and the generated
# trampoline is the only extern "C" frame.
GCN=$(_out gcn | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GCN" ]; then ok "native containment: $GCN"; else bad "G_CONTAIN_NATIVE failed — a panic in a JS native is NOT contained, and it takes the whole browser with it"; fi

head_ "G_STALE_NODE · a foreign handle must be INERT, not FATAL (Bar 0)"
# A JS reflector stores its node as a bare integer, and the arena it indexes is NOT necessarily the arena
# it came from: one process loads many documents and CURRENT_DOM is swapped on every re-entry. A handle
# held from an earlier document indexes into a different, smaller arena.
#
# And the consequence is not a wrong answer — it is a DEAD BROWSER. These accessors are reached from
# `extern "C"` natives, which are `nounwind`, so a Rust panic inside one is "panic in a function that
# cannot unwind" → SIGSEGV, core dumped. Every tab the user had open dies because one page held a stale
# node. WPT found it — and ONLY when the file ran AFTER other documents. It is clean in isolation, which
# is why no single-page test could ever have caught it.
GSN=$(_out gsn | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GSN" ]; then ok "stale handles: $GSN"; else bad "G_STALE_NODE failed — a handle from another arena panics, and a panic in an extern \"C\" native ABORTS THE PROCESS"; fi

head_ "G_NO_PHANTOM_FORK · an edit that LOOKS load-bearing and is inert"
# `./stylo` is a gitignored REFERENCE CLONE of servo/stylo. Nothing in this workspace builds it: it is
# not a member, there is no [patch.crates-io], there is no path dependency, and Cargo.lock pins
# `stylo 0.19.0` from the crates.io registry. So an edit under ./stylo reaches NOTHING.
#
# This already cost a tick: tick 42 began by flipping `parse_has() -> true` in there, rebuilding, and
# observing no change — which re-priced :has() from "a one-line flag" to "vendor Stylo", and is why it
# was ultimately solved with a hand-rolled supplement instead.
#
# A DIRTY reference checkout is, by definition, someone believing an edit matters when it cannot.
PHANTOM=0
if [ -d stylo ] && [ -n "$(git -C stylo status --short 2>/dev/null)" ]; then
  bad "G_NO_PHANTOM_FORK: ./stylo has LOCAL MODIFICATIONS — and nothing builds it. That edit reaches no binary, is not committed, and vanishes on a fresh clone. Revert it (git -C stylo checkout .) or vendor Stylo properly via [patch.crates-io] with the fork TRACKED IN THIS REPO."
  PHANTOM=1
fi
# And if a real fork ever DOES appear, it must be declared in STATUS.md's fork surface — a fork nobody
# recorded is a capability that a dependency bump can silently delete.
if grep -q "^\[patch.crates-io\]" Cargo.toml 2>/dev/null && ! grep -q "patch.crates-io" STATUS.md 2>/dev/null; then
  bad "G_NO_PHANTOM_FORK: Cargo.toml has a [patch.crates-io] that STATUS.md's fork surface does not record. An undeclared fork is a capability a dependency bump can silently delete."
  PHANTOM=1
fi
[ "$PHANTOM" -eq 0 ] && ok "no phantom fork: ./stylo is pristine reference-only; Stylo comes from crates.io"

head_ "G_CHARDATA · element.click() and CharacterData — neither existed"
# `element.click()` is how the web ACTIVATES things (menus, modals, carousels, hidden file inputs, every
# framework's programmatic activation). It was missing: a TypeError on the call, taking down whatever was
# running. CharacterData was `data` and nothing else — WPT scored CharacterData-replaceData 0/34.
# The offsets are UTF-16 CODE UNITS: "😀".length === 2, so counting Rust chars silently corrupts every
# emoji and surrogate pair on the web — and only for the scripts that use them.
GCD=$(_out gcd | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GCD" ]; then ok "chardata+click: $GCD"; else bad "G_CHARDATA failed — element.click(), the CharacterData interface, or its UTF-16 offsets"; fi

head_ "G_DOC_COLLECTIONS - document.images/forms/links named collections + getElementsByName"
GDOC=$(_out gdoc | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GDOC" ]; then ok "doc collections: $GDOC"; else bad "G_DOC_COLLECTIONS failed - document named collections or getElementsByName"; fi

head_ "G_LIFECYCLE · the document lifecycle, the clock, and the loop that must not die"
# Found by wiring up upstream WPT (tick 43). NONE of these move a box, which is why the 265-site
# Chromium differential could not see any of them for forty ticks:
#   * `window.parent` was undefined — and `while (w != w.parent) w = w.parent;` is how every page walks
#     to the top. It does not fail to terminate, it walks OFF THE END. This alone failed 100% of WPT.
#   * `DOMContentLoaded` and `load` were NEVER DISPATCHED. Anywhere. A site whose init lives in an
#     onload handler simply never initialised — silently. jQuery survived by checking `readyState`,
#     which is exactly why nobody noticed: it worked often enough to look fine.
#   * `setTimeout` threw its DELAY away — every timer was a FIFO push, so a 10s timer ran before a 0ms
#     one queued after it. Every debounce and retry-backoff on the web, in the wrong order, silently.
#   * A throwing task KILLED THE EVENT LOOP: one bad callback and every task after it never ran.
GL=$(_out gl | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GL" ]; then ok "lifecycle: $GL"; else bad "G_LIFECYCLE failed — the document lifecycle, the timer clock, or the event loop's survival of a throwing task"; fi

head_ "G_SELECTOR · the cascade must not silently DROP rules (CSS nesting)"
# RuleIndex — a cascade OPTIMISATION — read each StyleRule's selectors and block and never looked at its
# `rules` field: its NESTED rules. 41% of the corpus uses CSS nesting (a floor — external sheets are not
# even scanned). Every one of those rules was thrown away before it could match.
GSL=$(_out gsl | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GSL" ]; then ok "selectors: $GSL"; else bad "G_SELECTOR failed — the cascade is dropping rules (nested rules, or a selector that used to work)"; fi

head_ "G_ANIMATION · an animated element renders its END state, not its first frame"
# 52 of 237 corpus sites (21%) pair `opacity:0` with an animation. Rendering the first frame literally
# means a fifth of the web has content nobody can see. The second half of the gate is the important one:
# an element the author deliberately hid must STAY hidden.
GAN=$(_out gan | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GAN" ]; then ok "animation end-state: $GAN"; else bad "G_ANIMATION failed — fade-in content is invisible, or deliberately-hidden content was revealed"; fi

head_ "G_IFRAME · an <iframe> has a box, shows its document, and cannot touch its parent"
# 23% of the corpus, and usage == damage: `iframe` was in NO replaced-element list, so it laid out at
# ZERO WIDTH — the box was gone before we ever got as far as failing to fetch its document.
GIF=$(_out gif | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GIF" ]; then ok "iframes: $GIF"; else bad "G_IFRAME failed — an embed does not render, blocks paint, or can reach its parent"; fi

head_ "G_FORM · the browser must be WRITABLE — submit is cancellable, and forms serialize correctly"
# Forms are 50% of the corpus. The load-bearing assertion is that `preventDefault()` on `submit` is
# HONOURED: without it, every AJAX form on the web performs the full page navigation its author
# explicitly cancelled, and the user loses what they typed while nothing says why.
GFM=$(_out gfm | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GFM" ]; then ok "forms: $GFM"; else bad "G_FORM failed — submit is not cancellable, or a form does not serialize correctly"; fi

head_ "G_DEFER · defer/async/module must not block paint — and must still RUN"
# `defer` and `is_async` were parsed and used for NOTHING. Every script blocked first paint, including
# `type=module`, which is deferred by DEFAULT in every real browser and is what every Vite bundle ships
# as. The second half of this gate is the important one: when the split first landed I forgot it on
# `load_async` and every SPA in the suite silently stopped mounting.
GDF=$(_out gdf | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GDF" ]; then ok "defer/async: $GDF"; else bad "G_DEFER failed — a deferred script is blocking paint, or is never running at all"; fi

head_ "G_FIRST_PAINT · the document reaches the screen without waiting for its images"
# nytimes.com: the document was parsed, cascaded and laid out in 1.7s — and the user saw it at 14s,
# because the load path fetched every image first. A browser that does this feels broken while every
# other gate stays green.
GF=$(_out gf | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GF" ]; then ok "first paint: $GF"; else bad "G_FIRST_PAINT failed — first paint is waiting for subresources again"; fi

head_ "G_SILENT_FAIL · an error on the load/render/script path must never be swallowed"
# Named by an expensive failure: "React mounts, throws nothing, renders nothing" sat in the ledger for
# several ticks as a REACT bug. React was throwing, truthfully, inside an async render — and nothing was
# listening. A browser that fails silently sends you looking in the wrong codebase.
GS=$(_out gs | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GS" ]; then ok "silent failure: $GS"; else bad "G_SILENT_FAIL failed — an error is being swallowed on the load/render/script path"; fi

head_ "G_DEDUP · the same resource must not go to the WIRE twice for one navigation (Part 22.3)"
# Measured on real sites before this gate existed: nytimes issued 813 fetches, and one sprite was pulled
# down once per element that named it. The gate asserts on NET_DUPES — the wire, not the call — because
# a repeat `fetch()` served from cache costs nothing and counting it conflates free with expensive.
GD=$(_out gd | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GD" ]; then ok "dedup: $GD"; else bad "G_DEDUP failed — a resource is being fetched more than once per navigation"; fi

head_ "G_RUNAWAY · Bar 0 — a self-rescheduling timer must not hang the browser"
GRA=$(_out gra | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GRA" ]; then ok "runaway timer: $GRA"; else bad "G_RUNAWAY failed — a page can freeze the browser with one line of JS"; fi

head_ "G_CONTAIN · Bar 0 — a panic kills the PAGE, not the process (Part 23.2)"
GC=$(_out gc | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
if [ -n "$GC" ]; then ok "containment: $GC"; else bad "G_CONTAIN failed — a page can take the whole browser down"; fi

head_ "G_RUNTIME_COUNT · one async runtime for the process, not one per action (Part 25.2)"
if [ "${SHELL_FAILED:-1}" -eq 0 ]; then ok "runtime count flat (shell suite green)"; else bad "G_RUNTIME_COUNT failed — runtimes are proliferating"; fi

head_ "G_INTERACT · UI-thread cost of tab open/switch/close (the 'browser feels laggy' report)"
GI=$(printf '%s' "${SHELL_OUT:-}" | grep -E "^  (open|switch|close)")
if [ "${SHELL_FAILED:-1}" -eq 0 ] && [ -n "$GI" ]; then echo "$GI" | sed 's/^/  /'; ok "every tab operation under one frame"
else bad "G_INTERACT failed — a tab operation stalls the UI thread"; fi

head_ "T · crate tests"

# ── A KILLED GATE IS NOT A FAILING GATE.
#
# This loop used to be `grep 'test result: ok'` and `bad` on anything else — so a suite that was
# **OOM-killed**, or whose build was starved out under memory pressure, produced no `test result` line at
# all and was reported as a RED GATE. It happened three times in one session (G_FORM, G_IFRAME,
# manuk-shell), each time only when the wall shared the machine with a heavy release build, and each time
# the suite passed 3/3 in isolation a minute later.
#
# **A wall that is green non-deterministically proves nothing** — and worse, it teaches you to re-run
# until it is green, which is how a real regression gets shipped. PROCESS #17 already said this about
# gates; the wall itself had the same defect.
#
# The project already knows the fix, because the WPT harness learned it first: **separate the engine's
# verdict from the instrument's.** WPT calls a lost row `SHORT` and refuses to score it. Here:
#
#   * an explicit `test result: FAILED`  → the gate is RED. It is a real failure and it stops the tick.
#   * no verdict at all (signal, OOM, build starved) → the INSTRUMENT faulted. Retry ONCE, alone. If the
#     retry produces a verdict, that verdict is the truth. If it still produces none, say **INSTRUMENT**
#     and fail — because an unmeasurable gate is not a passing one either.
_crate_suite() {
  local c="$1" out rc
  out=$(cargo test -q -p "$c" 2>&1); rc=$?
  if echo "$out" | grep -q 'test result: FAILED'; then
    bad "$c tests FAILED"                       # a real red. Never retried, never excused.
    return
  fi
  local R
  R=$(echo "$out" | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
  if [ -n "$R" ]; then ok "$c: $R"; return; fi

  # No verdict. That is the instrument, not the engine — retry once, alone, with nothing else running.
  printf '  %s⟳%s %s produced no verdict (exit %s) — retrying alone; a killed gate is not a failing gate\n' \
    "$YEL" "$OFF" "$c" "$rc"
  wait
  out=$(cargo test -q -p "$c" 2>&1)
  if echo "$out" | grep -q 'test result: FAILED'; then bad "$c tests FAILED (on retry)"; return; fi
  R=$(echo "$out" | grep -oE 'test result: ok\. [0-9]+ passed' | head -1)
  if [ -n "$R" ]; then ok "$c: $R (after an instrument fault — the first run was killed, not red)"; return; fi
  bad "$c: INSTRUMENT FAULT — no verdict on two runs. Unmeasurable is not passing."
}

for c in manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell; do
  _crate_suite "$c"
done

if [ "${1:-}" != "--fast" ]; then
  # Every background gate must be finished before we time anything. A benchmark sharing the machine with
  # a compile is not a benchmark.
  wait
  head_ "F · perf floors (§1.7 — EPOCH-1, binding: a regression FAILS the tick)"
  # **These floors were silently SKIPPING.** The default corpus named `mid.html`/`large.html`, and
  # neither file existed, so `bench` printed empty tables and `verify.sh` printed a yellow dash and
  # moved on — for as long as the gate has existed. A gate that skips is not a gate; it is a
  # decoration that makes you feel measured. The corpus is now committed (docs/bench/), so it cannot
  # go missing again, and the floors are asserted rather than eyeballed.
  #
  # The corpus is deliberately flex-saturated — rows of `width:100%` cards with real paragraph text —
  # because that is the worst case for intrinsic sizing, which is where a layout regression will
  # actually hide. Floors are set from measured medians with ~20% headroom, NOT from the old numbers,
  # which referred to a page that does not exist and therefore never constrained anything.
  CORPUS="${MANUK_BENCH_CORPUS:-docs/bench/mid.html,docs/bench/large.html}"
  BENCH=$(cargo run -q -p manuk-wpt --release -- bench --pages "$CORPUS" --runs 5 2>/dev/null)
  echo "$BENCH" | sed -n '/nodes    parse/,/^$/p' | sed 's/^/  /'
  L_CASCADE=$(echo "$BENCH" | awk '/^large /{print $5; exit}')
  L_TOTAL=$(echo "$BENCH" | awk '/^large /{print $9; exit}')
  M_TOTAL=$(echo "$BENCH" | awk '/^mid /{print $9; exit}')
  fl() { awk -v a="$1" -v b="$2" 'BEGIN{exit !(a+0 <= b+0)}'; }

  # ── AN ABSOLUTE MILLISECOND FLOOR MEASURES THE CODE **AND THE CPU**, AND CANNOT TELL THEM APART.
  #
  # At tick 83 this wall failed `F2 pipeline 139ms > 125ms` — with **zero engine changes since the tick
  # that had measured 88ms on the same machine**. The cause: the governor is `powersave`, and a
  # *memory-bound* workload like layout does not make it ramp. Sampled during the bench, the cores sat at
  # 1.4–2.3GHz. `mid` went 14.4 → 23.2ms and `large` went 88 → 152ms **together** — the machine got ~1.7x
  # slower and the floor announced, with complete confidence, that the engine had.
  #
  # (A CPU-calibration loop does not fix this: a tight ALU loop *does* make the governor ramp, so it
  # reports a healthy machine while the memory-bound bench crawls. **The calibrator must be the same shape
  # as the workload** — and the only thing guaranteed to be is the workload itself.)
  #
  # So the binding floor is a **RATIO between two engine workloads of different size**. Machine speed
  # divides out exactly — both pages slow down together — while the thing F2 actually exists to catch,
  # **superlinear scaling**, survives untouched: a layout change that is quadratic in node count raises
  # `large/mid` no matter how fast the CPU is.
  #
  # The absolute numbers are still printed and still logged to the cadence ledger, because a *uniform*
  # slowdown is real and must be visible over time — it just cannot be judged from one run on one machine.
  # This is the FIFTH instrument in this project to need the same lesson: **an instrument must be able to
  # distinguish its own condition from the thing it measures** (PROCESS #46).
  RATIO=$(awk -v l="$L_TOTAL" -v m="$M_TOTAL" 'BEGIN{ printf "%.2f", (m+0>0 ? l/m : 0) }')
  CRATIO=$(awk -v c="$L_CASCADE" -v m="$M_TOTAL" 'BEGIN{ printf "%.2f", (m+0>0 ? c/m : 0) }')

  if [ -n "$L_CASCADE" ] && fl "$CRATIO" 0.55; then
    ok "F1 cascade/mid ${CRATIO} <= 0.55   (${L_CASCADE}ms absolute)"
  else
    bad "F1 cascade/mid ${CRATIO:-?} exceeds 0.55 — the cascade is scaling superlinearly"
  fi
  if [ -n "$L_TOTAL" ] && fl "$RATIO" 7.5; then
    ok "F2 pipeline large/mid ${RATIO}x <= 7.5x   (${L_TOTAL}ms / ${M_TOTAL}ms absolute — machine-dependent, not binding)"
  else
    bad "F2 pipeline large/mid ${RATIO:-?}x exceeds 7.5x — the pipeline is scaling superlinearly in page size"
  fi
fi

printf '\n'
# **The receipt.** The gates having run is now a FACT the pre-commit hook checks, not a claim anyone
# has to trust. It names the exact tree that was verified: `git diff HEAD` of the working tree, which
# is what a subsequent `git add -A && git commit` will stage. Edit anything afterwards and the name
# changes, the hook notices, and the commit is refused — which is the entire point. A receipt that
# said only "green, at 14:02" would be worthless, because the interesting failure is verifying one
# version of the diff and committing another.
# The receipt names a real git TREE OBJECT, not a diff hash. `git diff HEAD` was the obvious choice
# and it is wrong: it omits UNTRACKED files, which `git add -A` happily stages — so the receipt and
# the commit were hashing different things and the hook refused its own author's commit. It was right
# to. Building the tree the way `git add -A && git commit` would (in a throwaway index, so the real
# one is untouched) makes the two sides compute the same object by construction rather than by
# agreement.
RECEIPT=".git/manuk-verify-receipt"
TMPIDX="$(mktemp)"
GIT_INDEX_FILE="$TMPIDX" git read-tree HEAD 2>/dev/null
GIT_INDEX_FILE="$TMPIDX" git add -A 2>/dev/null
# Record the final section's duration so the wall-time audit sees the whole breakdown.
if [ -n "${_WALL_PREV_NAME:-}" ]; then
  printf '%s\t%s\n' "$((SECONDS - _WALL_LAST))" "$_WALL_PREV_NAME" >> "$_WALL_SECTIONS" 2>/dev/null || true
fi

VERIFIED_TREE="$(GIT_INDEX_FILE="$TMPIDX" git write-tree 2>/dev/null)"
rm -f "$TMPIDX"
{
  echo "tree: $VERIFIED_TREE"
  echo "head: $(git rev-parse HEAD)"
  echo "at: $(date -Iseconds)"
  echo "seconds: ${SECONDS}"
  if [ "$FAIL" -eq 0 ]; then echo "result: green"; else echo "result: FAILED"; fi
} > "$RECEIPT"

if [ "$FAIL" -eq 0 ]; then
  printf '\033[32m\033[1mVERIFY: all gates green\033[0m  (%ss)\n' "$SECONDS"
else
  printf '\033[31m\033[1mVERIFY: FAILED — the tick does not land\033[0m\n'
fi
exit "$([ "$FAIL" -eq 0 ] && echo 0 || echo 1)"
exit "$FAIL"
