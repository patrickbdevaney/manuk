# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 12** (about to run Tick 12). Ticks 1–11 done + committed; latest: 9 (`6524b11`),
  10 (`2db6920`), 11 (`fc41bc9`).
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

## Next action (Tick 12)

Pick: **L30 — in-process automation-surface hardening** (top raw-UCB ~4.6; the agent-native
differentiator, composing directly with Ticks 9–10 targeting+grounding; the user's latest
directive explicitly invites "innovations"). Pure, HEADLESS-verifiable functions over the a11y
tree + observation stream — a reliable driving surface an external agent/test can depend on.

Build in `agent/` (new module, e.g. `automation.rs`), composing `targeting`/`grounding`:
1. **Stable selectors.** A `Selector` that references an element by durable attributes rather
   than a fragile index/path: `{ role, name, nth }` (+ maybe an ancestor role for scoping).
   `resolve(selector, tree) -> Option<NodeId>` — deterministic, and stable across unrelated DOM
   mutations (re-resolve by role+name, not position). Unit-test that it still resolves after
   sibling insert/removal.
2. **Wait-for conditions.** A `Condition` enum evaluated against an `A11yNode` snapshot:
   `Visible(Selector)`, `Gone(Selector)`, `TextPresent(String)`, `UrlMatches(String)`,
   `CountAtLeast(Selector, n)`. `evaluate(cond, tree, url) -> bool`. The agent loop polls it
   between observations (no timers here — the caller drives ticks); provide a
   `wait(cond, snapshots: impl Iterator<Item=&A11yNode>) -> Outcome` that returns Met/Timeout
   over a bounded snapshot budget.
3. **Assertions.** The same `Condition`s as pass/fail checks: `assert_that(cond, tree, url) ->
   AssertResult { passed, detail }` — the primitive a test/automation script uses to verify page
   state. Compose with `grounding::ground_action` so an automation step is
   act → wait(post-condition) → assert.
4. Verify HEADLESS: unit tests over synthetic trees — selector resolves the intended node and
   survives a sibling mutation; each Condition true/false case; wait returns Met when a later
   snapshot satisfies it and Timeout when none do; assert reports the failing detail. Parity 72/72
   (agent-layer only).

Follow-ons: wire the automation surface into a scriptable session/BiDi command; retries with
backoff; a `Selector` by test-id attribute (`data-testid`) when present.

## Then keep going

After Tick 12, re-run §5 UCB. **Tick 15 is the next forced-highest-U.** Tier-A still open:
L06 password autofill (EXTERNAL keyring), L07 semantic history, L09 DevTools (GUI), L13 off-thread
external CSS/image, L15 inline SVG, L16 Shadow DOM, L18 cookie partitioning; Tier-B: L33 SoA-DOM
measure, L34 service worker. Rotate human/agentic to keep axis balance. Each tick: implement →
verify (build + parity 72/72 + test) → disk hygiene → commit+push (co-author line) → update
LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
