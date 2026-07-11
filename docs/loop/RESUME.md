# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 18.** Ticks 1–17 done + committed; latest: 15 (`8f76665`), 16 (`e7cd623`),
  17 (`6e27574`).
- **>>> EPOCH-1 IS DUE (CONSTITUTION §10 / ADR-005). It is the next action, not a tick. <<<**
- **Mission amended (ADR-004):** maximal traversal earned by **capability** — a fifth real browser
  with its own genuine fingerprint (impersonation is *off-strategy*, not merely forbidden); named
  sites are representative points, **not a checklist**. **Ambidextrous spine:** one engine — a
  human drives the headful GUI, an agent drives headless *or* the same headful GUI; **no forked
  page pipeline**. Rank candidates by *traversal-blocking capability*.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 41G free (86%); nuke
  `target/debug` only if free < 25G.
- **NEW verification powers (Tick 13, see [[CONSTITUTION]] §7):** VISUAL via `manuk-wpt render`
  (paint a page to PNG headlessly, then Read it; `--chrome` for a reference). EXTERNAL via
  `llama-server` + the Qwen GGUF under `/home/patrickd`. GUI/EXTERNAL items are no longer blocked —
  grind the visual/model-deferred backlog.
- Key architecture notes for future ticks:
  - Page JS runs on a **persistent** `PageContext` (`engine/js/src/dom_bindings.rs`) whose event
    loop is `event_loop::run_deferred` — microtasks + timers run, but `fetch`/XHR stay queued for
    the host. Host (shell `pump_fetches`) drains via `Page::take_fetches()`, performs I/O on
    `manuk-net`, settles via `Page::resolve_fetch`.
  - **Host-queue + re-enter is the universal pattern.** A native pushes to a thread-local; the
    host drains (`take_*`) after dispatch/load and calls back into a `PageContext` method that
    evals a delivery shim + `run_deferred`. Used by: `fetch`, `window.open`
    (`take_pending_window_opens`), `history` (`take_pending_history`/`handle_history_ops`),
    `postMessage` (`take_messages`/`pump_messages` → `deliver_message`). **Reuse it — never add a
    parallel queue** (that mistake cost a rework in Tick 2).
  - Window events (popstate, message, load) fire via the window-level registry
    `__fireWindowEvent(type, ev)` in the prelude (added Tick 3) — reuse it for any window event.
  - Same-document JS surfaces need no host round-trip and live entirely in engine/js, but must
    deliver via a microtask so they run before the enclosing dispatch/load/fetch call returns
    (all drain microtasks via `run_deferred`). `MutationObserver` (Tick 7) is the model: native
    mutation methods call `record_mutation` → `__recordMutation`; `queueMicrotask` delivers.
  - The document URL reaches JS via `install(..., doc_url)` → `%URL%` in `WINDOW_PRELUDE`.
    Per-document window identity (id + opener) is seeded post-load via `PageContext::set_identity`.
  - Navigation returns `manuk_page::Loaded::{Document, Download}` from `fetch_document`; the shell
    branches in `NavEvent::{Fetched, Prewarmed}`. `build_page` builds a Page off fetched HTML
    (shared by finish_load + finish_prewarm); prewarmed pages live in the bfcache; `goto` checks
    it first for an instant click.

## Next action — **EPOCH-1** (systemic audit; NOT a feature tick)

**Why now.** §10.1's drift detector has fired: over Ticks 1–17, capability rose **+49**
(JS +22, COMPAT +15, RENDER +12) while quality rose **+3** (PERF +3, MEM 0, STABILITY 0).
Threshold is 25; drift is **46**. Per-tick gates verify a *feature in isolation* and are
structurally incapable of proving the whole engine is fast, lean and hang-free. The loop
optimized what it measured; EPOCH-1 makes it measure the machine.

**This is an arc, not one tick** (§10.4). Several consecutive ticks is normal. Do not slice it
into "a bit of perf each tick" — that dilution is exactly what the gate exists to prevent.

Run §10.2 in order. **Measure first — do not optimize anything before there is a number.**

1. **Profile the hot paths.** Build a `manuk-wpt bench` (or `shell --measure`) that reports, on
   real pages (example.com / HN / Wikipedia / a heavy SPA): nav→first-paint, click→paint,
   key→caret, scroll frame time, and the per-stage breakdown (parse, cascade, layout, paint,
   display-list build, JS dispatch). **Publish the numbers next to Chromium's on the same pages.**
   Existing seeds: `AG5` latency harness (agent/src/bin/ag5-latency.rs), the `MEM3` binary-size
   measurement, `cold-start ~73ms` in STATE.
2. **Algorithmic complexity audit.** Hunt superlinear behaviour that only shows at scale. Known
   suspects already visible in the code: `cascade_via_stylo` runs a **full second MinimalCascade
   over the whole document** every cascade (for `vertical_align` + the shadow-tree fallback);
   `measure_intrinsic` re-lays-out subtrees per taffy probe (memoized — verify the hit rate);
   `DisplayList::build` rebuilds the entire list per frame; `styles` is a `HashMap` looked up in
   inner loops; `collect_positioned` / `flat_children` walk the tree repeatedly per layout.
3. **Latency budgets → invariants.** Set a budget per interaction, then write them into §1 so a
   later tick that regresses one **fails** like a parity regression. This is the ratchet that stops
   drift re-accruing.
4. **No hangs, no panics.** Every `block_on` on the UI thread is a latent hang — several are
   already logged as follow-ons (L21 async page-fetch; `pump_fetches`; `build_page`'s
   `fetch_and_apply_stylesheets`). Audit `unwrap`/`expect`/index/slice on input- and network-driven
   paths; bound every loop. (Tick 15 already removed one real layout panic — there will be more.)
5. **Memory.** Steady-state + per-tab; growth across repeated navigation.
6. **Stability soak.** Long realistic session (many navs, clicks, scrolls, tabs): zero panics, zero
   hangs, bounded memory.

**Binding output (§10.3):** a MEASURE report with real numbers, new invariant floors in §1, and an
ADR. *An epoch that produces no numbers has not happened.*

## After EPOCH-1

Reset the epoch tracker in [[LEDGER]] (record the axis snapshot). Then resume normal UCB ticks,
ranked by traversal-blocking capability (ADR-004). Queued and now well-motivated by the amended
mission's "media-rich client apps" class: **L50 CSS animations/transitions**, **L51 video/audio**,
**L52 canvas 2D**, **L53 iframes** — plus L02b Intersection/ResizeObserver (virtualized feeds),
L47 (HN nav wrap), L18 cookie partitioning, L15 SVG, L44 shell-chrome paint.
