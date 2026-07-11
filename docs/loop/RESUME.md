# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 5** (about to run Tick 5 — the forced-highest-U tick). Ticks 1 (`1a717d0`),
  2 (`91a22bb`), 3 (`7f1b35d`), 4 (`d6022ff`) done + committed.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 42G free (86%); nuke
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
    branches in `NavEvent::Fetched`. Downloads policy is `manuk_net::downloads` (pure + tested).

## Next action (Tick 5 — forced highest-U)

§5 forces the highest-U item. Highest U overall is a three-way tie at U8 (L31 llama grounding,
L32 prerender, L34 service worker); L31 (needs a local GGUF model → EXTERNAL) and L34 (C9, very
costly) can't be cleanly HEADLESS-verified in one tick, so honoring the verification invariant
the pick is **L32 — speculative/predictive prerender of the likely-next navigation** (U8, PERF,
HEADLESS). Design:

1. A predictor: given the current page + its links (and optionally hover/pointer signal from the
   shell), score which link the user is most likely to click next. Start simple + measurable:
   the top-of-viewport primary link / highest-rank in-content link, or the link currently
   hovered. Keep the scoring in a small `manuk_page` (or shell) module so it is unit-testable.
2. Prewarm: on idle after load (or on hover), kick an **off-thread** `manuk_net::fetch` of the
   predicted URL into the HTTP cache (RFC-9111) — the machinery already exists (preconnect R4 +
   the cache). Optionally pre-build the `Page` into the bfcache keyed by URL so a click is an
   instant swap. Bound it (1 in-flight; cancel/replace on a new prediction; never prerender
   cross-origin POST/again-non-idempotent or `rel=nofollow`).
3. Shell: wire a hover/idle hook to call the predictor + prewarm; on the actual click, if the
   target was prewarmed, serve from cache/bfcache (measure the win).
4. Verify HEADLESS + MEASURE: unit-test the predictor (given a link set + signal → expected pick)
   and assert a prewarmed URL is served from cache (cache-hit path) rather than re-fetched.
   Publish the latency delta. Parity must stay 72/72.

Guardrails: only same-origin GET; respect adblock + `nofollow`/`noreferrer`; cap concurrency;
cancel stale predictions. Log what was prewarmed vs used (hit rate) so it can be tuned.

## Then keep going

After Tick 5, re-run §5 UCB (normal exploit/explore; Tick 10 is the next forced-highest-U).
Strong Tier-A candidates still open: L02 MutationObserver, L03 postMessage/opener, L11 responsive
`@media`, L06 password autofill, L07 semantic history, L09 DevTools. Each tick: implement →
verify (build + parity 72/72 + test) → disk hygiene → commit+push (co-author line) → update
LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
