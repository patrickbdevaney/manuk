# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 3** (about to run Tick 3). Ticks 1 (`1a717d0`) + 2 (`91a22bb`) done + committed.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 42G free (86%); nuke
  `target/debug` only if free < 25G.
- Key architecture note for future ticks: page JS runs on a **persistent** `PageContext`
  (`engine/js/src/dom_bindings.rs`) whose event loop is `event_loop::run_deferred` — it runs
  microtasks + timers but leaves `fetch`/XHR queued for the host. The host (shell
  `pump_fetches`) drains via `Page::take_fetches()`, performs I/O on `manuk-net`, and settles
  via `Page::resolve_fetch`. Reuse this host-queue + re-enter pattern for any new async surface
  (postMessage, MutationObserver callbacks, pushState) — don't add a parallel queue.

## Next action (Tick 3)

UCB pick: **L10 — `history.pushState`/`replaceState` + `popstate` SPA routing** (V/C = 2.0, top
score; the natural complement to the fetch just landed — SPAs pair data-load with client-side
URL routing). Design:

1. `engine/js/src/dom_bindings.rs` window prelude: define `history` with `pushState(state,
   title, url)`, `replaceState(...)`, `state`, `length`, `back()/forward()/go()`, and a
   `location` shim kept in sync (at least `href/pathname/search/hash`). `pushState`/`replaceState`
   update `location`, store the state, and queue an entry for the host (thread-local
   `PENDING_HISTORY` drained like window.open — the host updates the omnibox URL + its
   session/back-forward stack WITHOUT a network navigation). Fire no `popstate` on push (per
   spec); fire `popstate` when the host calls back on a real back/forward.
2. `engine/js/src/lib.rs`: `take_history_ops(ctx)` (drain queued pushes) + `fire_popstate(ctx,
   state_json)` (dispatch a `popstate` event into the persistent global, then `run_deferred`).
3. `engine/page`: `Page::take_history_ops()` + `Page::fire_popstate(state, fonts, w)` (relayout
   on mutation, mirroring `resolve_fetch`).
4. `shell/src/gui.rs`: after dispatch, drain history ops → update `self.url` + omnibox + the
   tab's back/forward stack (no reload). Wire the existing Back/Forward buttons so that when the
   target entry was a pushState entry (same document), call `page.fire_popstate(...)` instead of
   re-navigating.
5. Verify HEADLESS: interactive test — a click handler calls `history.pushState({p:1},'','/next')`;
   assert `location.pathname==='/next'` and the queued op reached `take_history_ops`; then
   `fire_popstate` and assert the page's `onpopstate` ran. Parity must stay 72/72.

Follow-ons already logged: L21 async non-blocking fetch, L22 request/response fidelity, L23
AbortController.

## Then keep going

After Tick 3, re-run §5 UCB over the LEDGER (Tick 5 forces the highest-U item). Strong Tier-A
candidates queued: L02 MutationObserver, L03 postMessage/opener, L04 downloads, L11 responsive
`@media`. Each tick: implement → verify (build + parity 72/72 + test) → disk hygiene →
commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
