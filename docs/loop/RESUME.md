# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 8** (about to run Tick 8). Ticks 1 (`1a717d0`), 2 (`91a22bb`), 3 (`7f1b35d`),
  4 (`d6022ff`), 5 (`c6925f7`), 6 (`7c4a1f6`), 7 (`861a66c`) done + committed.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 41G free (86%); nuke
  `target/debug` only if free < 25G.
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

## Next action (Tick 8)

Pick: **L11 — responsive `@media` correctness** (UCB near-tie with the agentic L17; the user's
explicit "human-browser table stakes BEFORE agentic" ordering breaks it toward L11, which also
hits a known weak frontier — Wikipedia-class responsive layouts render only partially).

FIRST: establish the current state — `grep -rn "media\|@media\|MediaQuery\|media_query" engine/css/src`
to see how much `@media` the parser/cascade already handles (it may parse-and-drop, or ignore
entirely). The tick's shape depends on this:
1. Parse `@media` prelude conditions into a small evaluable form: `width`/`min-width`/`max-width`/
   `min-height`/`max-height` (px), `orientation`, `prefers-color-scheme`, and `and`/comma lists.
   Keep the evaluator pure + unit-testable (`matches(query, viewport_w, viewport_h) -> bool`).
2. In the cascade, include a media-block's rules only when its query matches the current
   viewport. The cascade already takes `viewport_width` (see `cascade_styles(..., viewport_width)`)
   — thread height too if needed. On resize/relayout the styles must be recomputed (the shell
   already re-cascades on width changes; verify the media set is re-evaluated there).
3. Make `window.matchMedia(q)` (currently a no-match stub in the prelude) evaluate the same way
   against the boot viewport, and ideally update on resize (a follow-on if costly).
4. Verify HEADLESS + MEASURE: a WPT-style parity probe or a unit/integration test where an element
   has different computed width/display under a narrow vs a wide viewport (e.g.
   `@media (max-width:600px){ .box{display:none} }`), asserting it applies only when narrow.
   Parity must stay 72/72 (add a probe page if useful).

Follow-ons: container queries; `matchMedia` change listeners on resize; the full media-feature
set (resolution, aspect-ratio, hover/pointer).

## Then keep going

After Tick 8, re-run §5 UCB (normal exploit/explore; **Tick 10** is the next forced-highest-U —
candidates incl. L16 Shadow DOM U7, L31 llama U8, L34 service worker U8). Strong Tier-A still
open: L17 AG2/AG3 agentic targeting (top UCB, deferred by the human-first ordering — revisit once
table stakes feel solid), L06 password autofill (EXTERNAL keyring), L07 semantic history, L05
uploads, L09 DevTools (GUI), L13 off-thread external CSS/image. Each tick: implement → verify
(build + parity 72/72 + test) → disk hygiene → commit+push (co-author line) → update
LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
