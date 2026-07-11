# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 9** (about to run Tick 9). Ticks 1 (`1a717d0`), 2 (`91a22bb`), 3 (`7f1b35d`),
  4 (`d6022ff`), 5 (`c6925f7`), 6 (`7c4a1f6`), 7 (`861a66c`), 8 (`02595bc`) done + committed.
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

## Next action (Tick 9)

Pick: **L17 — AG2 task-intent AXTree pruning + AG3 dual (semantic+visual) targeting** (top UCB
~4.3; the agent-native differentiator, un-deferred now that human table stakes are solid). Pure
functions over the existing `engine/a11y` tree — HEADLESS-verifiable.

Surface already present: `engine/a11y` `A11yNode { node, role: Role, name, bbox: Option<Rect>,
z, children }`; `Role::is_interactive()`, `name_from_content()`; `A11yNode::{find, find_containing,
hit_test, to_viewport_lines}`; `Rect::{center, intersects}`. The agent crate (`agent/src/*`)
consumes it (traversal/triage/forms).

1. **AG2 — task-intent pruning.** A pure fn (new module, e.g. `agent/src/targeting.rs` or
   `engine/a11y`): `prune_for_task(tree: &A11yNode, task: &str) -> PrunedTree` (or a `Vec<&A11yNode>`
   of kept nodes). Keep: every interactive node (`role.is_interactive()`), any node whose `name`
   token-overlaps the task keywords, and the ancestor chain of each kept node (for context). Drop
   purely-decorative/hidden subtrees with no kept descendant. Emit a compact observation (reuse
   `to_viewport_lines`-style formatting) so the pruned tree is smaller than the full one — assert
   that reduction in a test.
2. **AG3 — dual targeting.** `resolve_target(tree, intent: &str, viewport: Rect) -> Option<Targeted>`
   combining a **semantic** score (role match + name token overlap / exact-match bonus) and a
   **visual** score (in-viewport, larger/centered bbox preferred, top-of-reading-order tiebreak).
   Return the best node + its click point (`bbox.center()`), plus the runner-up + a confidence
   margin so the caller can gate on ambiguity. Keep weights as named consts.
3. Verify HEADLESS: build a small synthetic `A11yNode` tree (buttons/links/headings with names +
   bboxes) and assert: (a) pruning keeps the interactive + task-matching nodes and their
   ancestors while dropping unrelated decorative nodes (and shrinks the node count); (b)
   `resolve_target("sign in", …)` picks the "Sign in" button over a same-text footer link by the
   visual score, and reports low confidence when two equally-good targets tie. Parity must stay
   72/72 (this is agent-layer; no render change).

Follow-ons: wire AG3 into the shell/agent action path (choose targets for `BrowserAction`); a
learned/weighted scorer; OCR/visual-text fallback when the AX name is empty.

## Then keep going

After Tick 9, run **Tick 10 = forced-highest-U** (§5): candidates L31 llama grounding U8
(EXTERNAL — may fail the verification gate; prefer the highest-U that stays HEADLESS, e.g. L16
Shadow DOM U7 or L34 service-worker-subset U8 if a headless slice exists). Then resume normal UCB.
Strong Tier-A still open: L06 password autofill (EXTERNAL keyring), L07 semantic history, L05
uploads, L09 DevTools (GUI), L13 off-thread external CSS/image, L15 inline SVG, L16 Shadow DOM.
Each tick: implement → verify (build + parity 72/72 + test) → disk hygiene → commit+push
(co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
