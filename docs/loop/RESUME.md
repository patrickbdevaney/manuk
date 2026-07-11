# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 15** (about to run Tick 15 — the forced-highest-U tick). Ticks 1–14 done + committed;
  latest: 12 (`034c275`), 13 (`64ba73a`), 14 (`e441564`).
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

## Next action (Tick 15 — FORCED-HIGHEST-U, filtered by traversal impact)

Pick: **L16 — Custom Elements + Shadow DOM basics** (U7, HEADLESS). §5 forces the highest-U item;
ADR-004 then filters by traversal-blocking capability. L34 (service worker) is nominally U8 but
C9 and *not* traversal-blocking — sites degrade gracefully without it. Unsupported **web
components** instead make content **simply not appear**, blocking whole classes of the modern web
(design systems, YouTube-class apps). So: highest-U among the traversal-blocking, honestly
verifiable set.

1. **Custom elements**: `customElements.define(name, ctor)` registry in the window prelude;
   upgrade matching elements already in the DOM and on later insert (reuse the Tick-7
   `record_mutation` hook — it already fires on every native DOM mutation). Call the lifecycle
   callbacks: `connectedCallback` / `disconnectedCallback` / `attributeChangedCallback` (+
   `observedAttributes`).
2. **Shadow DOM**: `element.attachShadow({mode})` → a shadow root the element's children render
   *instead of* its light-DOM children. The arena DOM needs a shadow-root notion; layout/paint must
   walk the shadow tree. NOTE: `Page::visible_text` already claims to respect shadow DOM + slot
   assignment (see its doc comment) — **read that first**; some of the model may already exist.
   Start with closed/open mode + a single default `<slot>`; named slots are a follow-on.
3. Verify **HEADLESS + VISUAL**: an interactive-test scenario where a custom element upgrades,
   `connectedCallback` fires, and its shadow content renders (assert via the DOM/a11y tree); then
   `manuk-wpt render` the same page and *look* at the PNG to confirm the shadow content actually
   paints. Parity must stay 72/72.

Follow-ons: named slots + slot reassignment; `::part`/`::slotted`; adopted stylesheets; scoped
style isolation in the cascade.

## Then keep going

Re-run §5 UCB (Tick 20 is the next forced-highest-U). With VISUAL + llama unblocked, **the whole
GUI/EXTERNAL backlog is fair game** — and ADR-004 says rank by traversal-blocking capability:
- **JS/DOM depth**: L02b (Intersection/ResizeObserver — virtualized feeds *need* these), L22
  fetch fidelity, L16 follow-ons.
- **Virtualized-feed performance** (the X/feed class): scroll + recycle + incremental relayout
  under a live feed — likely a new ledger item; pair with L20 (profile vs Chromium).
- **Session/auth durability**: L18 cookie partitioning (re-queued), L06 autofill, L22 credentials.
- **Visual fidelity**: real-page audits vs Chrome (example.com / HN / Wikipedia) using `render
  --chrome`; L43b radii/shadows; L15 inline SVG; L44 shell-chrome paint (unblocks GUI-chrome).
Each tick: implement → verify (build + parity 72/72 + test/screenshot) → disk hygiene →
commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.
