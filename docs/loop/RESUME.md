# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 13** (about to run Tick 13). Ticks 1–12 done + committed; latest: 10 (`2db6920`),
  11 (`fc41bc9`), 12 (`034c275`).
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

## Next action (Tick 13)

Pick: **L18 — cookie partitioning + `SameSite` enforcement audit** (rotate back to human/security
after the agentic L30; advances the under-developed SECURITY axis 45; self-contained + HEADLESS in
`engine/net`).

FIRST survey what exists: `engine/net/src/cookies.rs` (RFC-6265 `CookieJar`, `SameSite` enum,
serde, persistence) and `engine/net/src/storage.rs` (there's already a partitioned store —
`jar_mut(profile, container, top_level)`, `RequestContext::{navigation, subresource}`,
`is_same_site` — see the earlier grep). The tick likely = **wire + audit + test**, not build from
scratch:
1. Confirm/þfix that cross-site subresource requests use a **partitioned** jar keyed by the
   top-level site (so a tracker's cookie on site A isn't sent on site B). If `storage.rs` already
   partitions, ensure the actual `send_once`/`cookie_jar()` path (lib.rs) routes through it with
   the request's top-level context rather than the single global jar.
2. `SameSite` enforcement on send: `Strict` cookies omitted on cross-site navigations;
   `Lax` sent only on top-level GET navigations, omitted on cross-site subresource/POST;
   `None` requires Secure. Implement `should_send(cookie, request_ctx)` purely and test the
   matrix.
3. Verify HEADLESS: unit tests — a `Strict`/`Lax`/`None` cookie set on site A is/ isn't attached
   for (same-site nav, cross-site nav, cross-site subresource, cross-site POST); partition
   isolation (cookie on A not visible to B's jar). Parity 72/72 (net-layer only).

Follow-ons: `__Host-`/`__Secure-` prefixes; partition-key persistence; CHIPS (`Partitioned`
attribute).

## Then keep going

After Tick 13, re-run §5 UCB. **Tick 15 is the next forced-highest-U.** Tier-A still open:
L06 password autofill (EXTERNAL keyring), L07 semantic history, L09 DevTools (GUI), L13 off-thread
external CSS/image, L15 inline SVG, L16 Shadow DOM; Tier-B: L33 SoA-DOM measure, L34 service
worker. Rotate human/agentic to keep axis balance. Each tick: implement → verify (build + parity
72/72 + test) → disk hygiene → commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME
→ next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
