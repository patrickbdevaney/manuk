# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 6** (about to run Tick 6). Ticks 1 (`1a717d0`), 2 (`91a22bb`), 3 (`7f1b35d`),
  4 (`d6022ff`), 5 (`c6925f7`) done + committed.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 41G free (86%); nuke
  `target/debug` only if free < 25G.
- Key architecture notes for future ticks:
  - Page JS runs on a **persistent** `PageContext` (`engine/js/src/dom_bindings.rs`) whose event
    loop is `event_loop::run_deferred` — microtasks + timers run, but `fetch`/XHR stay queued for
    the host. Host (shell `pump_fetches`) drains via `Page::take_fetches()`, performs I/O on
    `manuk-net`, settles via `Page::resolve_fetch`.
  - Same host-queue pattern for `window.open` (`take_pending_window_opens`) and `history`
    (`take_pending_history` / `handle_history_ops`). **Reuse it for any new host-visible surface
    (postMessage, MutationObserver delivery) — never add a parallel queue** (that mistake cost a
    rework in Tick 2).
  - The document URL reaches JS via `install(..., doc_url)` → `%URL%` in `WINDOW_PRELUDE`.
  - Navigation returns `manuk_page::Loaded::{Document, Download}` from `fetch_document`; the shell
    branches in `NavEvent::{Fetched, Prewarmed}`. Downloads policy = `manuk_net::downloads`;
    prerender predictor = `shell/prerender.rs` (pure). `build_page` builds a Page off fetched HTML
    (shared by finish_load + finish_prewarm); prewarmed pages live in the bfcache; `goto` checks
    it first for an instant click.

## Next action (Tick 6)

UCB pick: **L03 — cross-window `postMessage` + `window.opener`** (top score ~4.4; completes the
OAuth-popup story `window.open` began — an explicit needs-list item — and is HEADLESS-verifiable).
Design (route messages between the two tabs' PageContexts through the host — reuse the host-queue
pattern, do NOT add a parallel queue):

1. `window.open` already queues the URL and the host opens a new tab with its own `PageContext`.
   Give the opener a stable handle to the opened tab and vice-versa (an `opener` tab-id link in
   the shell's `Browser`/tab model), plus a JS `window.opener` shim and the returned popup handle
   carrying a target tab-id.
2. `postMessage(msg, targetOrigin)` on a window handle (opener or popup): the JS shim serializes
   `msg` (structured-clone-lite: JSON) and calls a native `__postMessage(targetTabId, json,
   origin)` that queues `(from_tab, to_tab, json, origin)` to a thread-local drained by the host
   (`take_pending_messages`). The host routes it to the destination tab's `PageContext` via a new
   `PageContext::deliver_message(data_json, origin, source_ref)` that fires a `message`
   `MessageEvent` (`{data, origin, source}`) through the window event registry (`__fireWindowEvent`
   — already built in Tick 3), then `run_deferred`.
3. `engine/js` + `engine/page` wrappers (`take_messages`, `deliver_message`) + shell pump
   (`pump_messages`, called alongside `pump_fetches`/`handle_history_ops` after dispatch + load).
   Respect `targetOrigin` ('*' or an exact origin match).
4. Verify HEADLESS: drive two `PageContext`s directly — one registers `onmessage`, the other
   `deliver_message`s a payload; assert the handler ran with the right `data`/`origin`. (A single
   context can also self-post to test the MessageEvent shape.) Parity must stay 72/72.

Follow-ons: `BroadcastChannel`; `MessageChannel`/`MessagePort`; full structured clone (Blob/Map/
Set); `window.name` targeting.

## Then keep going

After Tick 6, re-run §5 UCB (normal exploit/explore; **Tick 10** is the next forced-highest-U).
Strong Tier-A candidates still open: L02 MutationObserver, L11 responsive `@media`, L06 password
autofill (EXTERNAL keyring), L07 semantic history, L09 DevTools (GUI). Each tick: implement →
verify (build + parity 72/72 + test) → disk hygiene → commit+push (co-author line) → update
LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
