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

## Next action — **Tick 18** (EPOCH-1 is CLOSED; back to fast feature velocity)

**EPOCH-1 closed** (report: [[EPOCH-1]], ADR-008). Cascade 2.69× faster, pipeline 1.67× faster,
one dead button killed, floors F1–F3 + no-dead-affordances now binding (§1.7/§1.8).

**Debt rule is live (§1.9):** ≥1 STAR DEBT item must be retired **every 3 ticks**, and debt
**outranks new capability work**. Three are outstanding (LEDGER):

- **DEBT-1** — kill the 4 UI-thread `block_on`s (RELIABILITY: each is a latent hang).
  *Highest value:* these are the "app not responding" bugs waiting to happen. `gui.rs:754,755`
  (page load + stylesheets), `1403` (fetch pump), `1800` (agent panel). Pattern already exists:
  `start_fetch` spawns off-thread and wakes the loop with a `NavEvent` — do the same for these
  (spawn → `NavEvent::…` → apply on the UI thread).
- **DEBT-2** — residual cascade superlinearity (still 4.3× worse/node at 19k vs 1.3k nodes).
- **DEBT-3** — shell-chrome headless paint (a **probe gap**: AESTHETICS/ERGONOMICS are currently
  unmeasurable, so the star guarantee has a hole).

**Tick 18 = DEBT-1** (pay debt first; it is also the single biggest RELIABILITY win).

## Then keep going

Fast feature ticks resume, ranked by traversal-blocking capability (ADR-004), interleaved with the
debt rate (≥1 per 3 ticks). Queued: **L50 CSS animations/transitions**, **L51 video/audio**,
**L52 canvas 2D**, **L53 iframes** (the media-rich class the amended mission names), L02b
Intersection/ResizeObserver (virtualized feeds), L47 HN nav wrap, L18 cookie partitioning, L15 SVG.
Next epoch: earliest **Tick 30** (min interval 12), or sooner if drift > 25.
