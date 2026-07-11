# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 2** (about to run Tick 2). Tick 1 done + committed (`1a717d0`).
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 42G free (86%); nuke
  `target/debug` only if free < 25G.

## Next action (Tick 2)

UCB pick: **L01 — `fetch()` + `XMLHttpRequest` in page JS** (V/C = 2.0, highest exploit; the
biggest COMPAT lever — SPAs load data with it). Design (host-queue + re-enter, mirroring the
`window.open` pattern already in `dom_bindings.rs`):

1. `engine/js/src/dom_bindings.rs`: native `__fetch(url, method, body)` assigns an id, pushes
   `{id,url,method,body}` to a thread-local `PENDING_FETCHES`, returns the id. Add
   `pub fn take_pending_fetches() -> Vec<(u64,String,String,String)>`. In the window prelude add
   a `fetch` shim (returns a Promise stored by id in `__fetchResolvers`) + a minimal
   `XMLHttpRequest` shim + `__resolveFetch(id, status, bodyLiteral)` that resolves the stored
   Promise with a Response-like object (`ok/status/text()/json()`).
2. `engine/js/src/lib.rs`: `take_fetches()` (+ non-`_sm` stub) and
   `fetch_resolve(ctx, id, status, body)` → `PageContext` evals `__resolveFetch(...)` + drains
   the event loop.
3. `engine/page`: `Page::resolve_fetch(id, status, body, fonts, w)` → `manuk_js::fetch_resolve`
   + relayout if DOM changed.
4. `shell/src/gui.rs`: after `perform_page_click`'s dispatch + `handle_window_opens`, add
   `pump_fetches()`: loop (bounded ~8 rounds) draining `manuk_js::take_fetches()`, performing
   each via `manuk_net` on `self.rt` (block_on; async non-blocking is a follow-on item), then
   `page.resolve_fetch(...)`. Also pump after `finish_load`.
5. Verify HEADLESS: interactive test — a script that `fetch('/data').then(r=>r.text()).then(t=>
   document.body.textContent=t)`; the host resolves it; assert the DOM updated. Needs the shell
   to perform the request, so the test may live in `shell` or drive resolve_fetch directly with
   a canned body. Parity must stay 72/72.

Follow-ons to log after: non-blocking async fetch (don't stall the UI thread); real
Request/Response/Headers fidelity; `AbortController`.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
