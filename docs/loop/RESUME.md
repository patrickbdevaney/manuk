# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 16** (about to run Tick 16). Ticks 1–15 done + committed; latest: 13 (`64ba73a`),
  14 (`e441564`), 15 (`8f76665`).
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

## Next action (Tick 16)

Pick: **L45 — block-in-inline** (found while VISUAL-verifying Tick 15; pre-existing and NOT
shadow-specific). A block-level box inside an inline element **loses its box entirely** — the text
still flows, but the block's background / padding / border simply vanish. Repro:

    cargo run -q -p manuk-wpt --release -- render --inline \
      '<span><div style="background:#fd0;padding:6px">block in inline</div></span>' \
      --out /tmp/bii.png --chrome
    # Manuk: bare text, no yellow box. Chrome: a yellow padded block.

Straightforwardly traversal-blocking per ADR-004: block-in-inline is everywhere in real markup
(a `<div>` inside an `<a>`/`<span>`/custom element), and losing the box means losing the visual.
It is also *why* an inline shadow host with block shadow content renders bare text (Tick 15).

Design — CSS2 §9.2.1.1 **anonymous block boxes**: when an inline box contains a block-level child,
the inline is *split* around it and the whole run is wrapped in anonymous block boxes:
`inline-before | block | inline-after`. In `engine/layout`:
1. Find where inline content is gathered (`collect_inline_group` / `collect_inline_node`, ~line
   2040+) — a block-level child encountered while building inline items is currently swallowed.
2. When building a block's children, if an inline-level run contains a block-level descendant,
   emit: anonymous block(inline run before) → the block child (laid out as a block) → anonymous
   block(inline run after). The existing anonymous-box machinery around line 1720 (`kids.extend
   (new_boxes)`, `BoxContent::Inline(std::mem::take(frags))`) is the seam.
3. Preserve the inline's own styles on the split parts (an inline's background/border applies to
   each fragment) — a simplification is acceptable if documented.
4. Verify **VISUAL** (render + `--chrome`: the yellow padded block appears, matching Chrome) and
   **HEADLESS** (a layout test: the inner block's box has the right width/height/background, and
   text before/after it stays on separate lines). Parity MUST stay 72/72 — this touches the core
   inline/block seam, so run it early and often.

## Then keep going

Re-run §5 UCB (Tick 20 = next forced-highest-U). Rank by **traversal-blocking capability**
(ADR-004):
- **JS/DOM depth**: L02b Intersection/ResizeObserver (virtualized feeds *need* these), L16b named
  slots + a scoped flat-tree walk in Stylo, L22 fetch fidelity.
- **Virtualized-feed performance** (the X/feed class): scroll + recycle + incremental relayout
  under a live feed; pair with L20 (profile vs Chromium).
- **Session/auth durability**: L18 cookie partitioning, L06 autofill.
- **Visual fidelity**: real-page audits vs Chrome (`render --chrome` on example.com / HN /
  Wikipedia); L43b radii/shadows; L15 inline SVG; L44 shell-chrome paint (unblocks GUI-chrome).
Each tick: implement → verify (build + parity 72/72 + test/screenshot) → disk hygiene →
commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.
