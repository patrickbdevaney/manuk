# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 7** (about to run Tick 7). Ticks 1 (`1a717d0`), 2 (`91a22bb`), 3 (`7f1b35d`),
  4 (`d6022ff`), 5 (`c6925f7`), 6 (`7c4a1f6`) done + committed.
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
  - The document URL reaches JS via `install(..., doc_url)` → `%URL%` in `WINDOW_PRELUDE`.
    Per-document window identity (id + opener) is seeded post-load via `PageContext::set_identity`.
  - Navigation returns `manuk_page::Loaded::{Document, Download}` from `fetch_document`; the shell
    branches in `NavEvent::{Fetched, Prewarmed}`. `build_page` builds a Page off fetched HTML
    (shared by finish_load + finish_prewarm); prewarmed pages live in the bfcache; `goto` checks
    it first for an instant click.

## Next action (Tick 7)

UCB pick: **L02 — `MutationObserver`** (top score ~4.4; the next SPA-compat lever — frameworks
mutate the DOM after a fetch and observe it; without the API their code throws at construction).
Design (emit records at the reflector mutation sites, deliver as a microtask):

1. `engine/js/src/dom_bindings.rs`: the DOM-mutating reflector methods (`setAttribute`/
   `removeAttribute`, the `textContent`/`innerHTML` setters, `appendChild`/`insertBefore`/
   `removeChild`/`replaceChild`) already run through native fns — have each emit a mutation
   record to a JS-side pending list: `__recordMutation(type, targetNid, attrName, oldValue,
   addedNids, removedNids)`. (Find the exact set with `grep -n "fn .*append\|set_attribute\|
   text_content\|remove_child" engine/js/src/dom_bindings.rs`.)
2. Prelude: a real `MutationObserver` class — `observe(targetNode, options)` records
   `{target, childList, attributes, characterData, subtree, attributeOldValue,
   characterDataOldValue, attributeFilter}`; `disconnect()`; `takeRecords()`. A microtask
   checkpoint (queue via the existing `queueMicrotask`) drains `__pendingMutations`, builds
   `MutationRecord`s, and dispatches the batched records to each observer whose target/subtree +
   options match. Reuse the node reflectors (`__nodes`) to turn nids back into nodes.
3. No host round-trip needed (mutations are same-document + synchronous), so this lives entirely
   in `engine/js` — but it must run inside `PageContext` (load + dispatch + resolve paths), which
   already drain microtasks via `run_deferred`. Ensure records queued during a dispatch are
   delivered before that call returns.
4. Verify HEADLESS: add a scenario to the page interactive test — a script observes a node, then
   a click handler does `el.setAttribute('data-x','1')` + `el.appendChild(...)`; after
   `dispatch_click`, assert the observer callback ran with records of the right `type`/`target`/
   `addedNodes`. Also test `attributeOldValue` + `subtree`. Parity must stay 72/72.

Follow-ons: `characterData` oldValue nuance; observer GC lifetime; `IntersectionObserver`/
`ResizeObserver` (separate ticks).

## Then keep going

After Tick 7, re-run §5 UCB (normal exploit/explore; **Tick 10** is the next forced-highest-U).
Strong Tier-A candidates still open: L11 responsive `@media`, L06 password autofill (EXTERNAL
keyring), L07 semantic history, L09 DevTools (GUI), L13 off-thread external CSS/image. Each tick:
implement → verify (build + parity 72/72 + test) → disk hygiene → commit+push (co-author line) →
update LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
