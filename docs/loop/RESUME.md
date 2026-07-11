# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 14** (about to run Tick 14). Ticks 1–13 done + committed; latest: 11 (`fc41bc9`),
  12 (`034c275`), 13 (`64ba73a`).
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

## Next action (Tick 14)

Pick: **L43 — `border-radius` + `box-shadow` paint** (the next visible "look like Chromium" gaps,
both plainly missing in the flex-card screenshot vs Chrome; now VISUAL-verifiable).

1. Confirm the CSS is parsed: `grep -rn "border.radius\|border_radius\|box.shadow\|box_shadow"
   engine/css/src`. `border-radius` likely parses to a `ComputedStyle` field (checkbox rendering
   mentioned it); `box-shadow` may need parsing (offset-x, offset-y, blur, spread, color, inset).
2. Paint (`engine/paint`): the CPU painter uses tiny-skia — it has `PathBuilder` for rounded
   rects. Add rounded-corner clipping/fill for background + border when `border-radius` > 0, and a
   blurred shadow rect for `box-shadow` (tiny-skia blur, or an approximate feathered rect).
   Thread the radii/shadow from `ComputedStyle`/`LayoutBox` into the paint DisplayList.
3. Verify VISUAL: `manuk-wpt render --inline '<div style="border-radius:12px;box-shadow:0 2px
   8px rgba(0,0,0,.3);width:100px;height:60px;background:#09e"></div>' --out P.png --chrome`, Read
   P.png + P.chrome.png — corners rounded, soft shadow present. Add a HEADLESS assertion where
   feasible (e.g. a corner pixel is background, not fill). Parity must stay 72/72.

Follow-ons: per-corner radii; `border-radius` on images/clipping; inset shadows; multiple shadows.

## Then keep going

Re-run §5 UCB. **Tick 15 is the next forced-highest-U.** The screenshot + llama unblock means the
GUI/EXTERNAL backlog is now fair game — grind visual fidelity (real-page audits vs Chrome:
example.com/HN/Wikipedia), L09 DevTools, L06 autofill, and the L44 shell-chrome headless-paint
path (to screenshot the tab strip/menus). Also still open: L18 cookie partitioning (re-queued),
L07 history, L13 off-thread CSS/image, L15 SVG, L16 Shadow DOM. Rotate human/agentic. Each tick:
implement → verify (build + parity 72/72 + test/screenshot) → disk hygiene → commit+push → update
docs → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
