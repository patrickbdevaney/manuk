# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 11** (about to run Tick 11). Ticks 1–10 done + committed; latest: 8 (`02595bc`),
  9 (`6524b11`), 10 (`2db6920`).
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

## Next action (Tick 11)

Pick: **L05 — file uploads** (rotating back to human table stakes after two agentic ticks 9–10;
UCB tops at the agentic L30 ~4.6 but the user's human-first ordering keeps L30 queued; L05 is the
top human item ~4.2 — a concrete "run any website" gap, with a HEADLESS-verifiable core mirroring
Tick 4 downloads).

1. `engine/net` (or a small module): a pure `multipart` encoder — given fields
   `[(name, value)]` and files `[(field, filename, content_type, bytes)]`, produce the
   `multipart/form-data` body + the `Content-Type: multipart/form-data; boundary=…` header.
   Deterministic boundary (pass it in / derive from a counter — NO `Math.random`/time in tests).
   Unit-test the exact wire bytes (CRLFs, `Content-Disposition: form-data; name=…; filename=…`,
   part `Content-Type`, trailing `--boundary--`).
2. Form submission path: when a `<form enctype="multipart/form-data">` (or containing a
   `<input type=file>`) is submitted, encode with the above and POST via `manuk_net::request`.
   Find the current form-submit path (`grep -rn "enctype\|multipart\|fn submit\|application/x-www-form-urlencoded" engine shell agent`)
   and branch on enctype (urlencoded stays the default).
3. Shell: a file picker is GUI (can't headlessly verify) — wire `<input type=file>` click to a
   picker (winit/rfd or a stub), store the chosen path on the input, include it on submit. Keep
   the GUI part thin; the encoder is the verified core.
4. Verify HEADLESS: unit-test the multipart encoder's exact bytes for a field + a small file;
   optionally an integration test that a multipart submit builds the right request. Parity 72/72.

Follow-ons: multiple files per input; drag-drop; large-file streaming; progress.

## Then keep going

After Tick 11, re-run §5 UCB. **Tick 15 is the next forced-highest-U.** The **agentic L30**
(in-process automation-surface hardening — stable selectors, wait-for conditions, assertions;
composes directly with Ticks 9–10 targeting+grounding) is the top raw-UCB item — take it on the
next agentic rotation. Other Tier-A open: L06 password autofill (EXTERNAL keyring), L07 semantic
history, L09 DevTools (GUI), L13 off-thread external CSS/image, L15 inline SVG, L16 Shadow DOM,
L18 cookie partitioning. Each tick: implement → verify (build + parity 72/72 + test) → disk
hygiene → commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
