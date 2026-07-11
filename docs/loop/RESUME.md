# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 4** (about to run Tick 4). Ticks 1 (`1a717d0`), 2 (`91a22bb`), 3 (`7f1b35d`) done +
  committed.
- Working tree: clean, on `main`, pushed. Parity 72/72. Disk: 42G free (86%); nuke
  `target/debug` only if free < 25G.
- Key architecture notes for future ticks:
  - Page JS runs on a **persistent** `PageContext` (`engine/js/src/dom_bindings.rs`) whose event
    loop is `event_loop::run_deferred` — microtasks + timers run, but `fetch`/XHR stay queued for
    the host. Host (shell `pump_fetches`) drains via `Page::take_fetches()`, performs I/O on
    `manuk-net`, settles via `Page::resolve_fetch`.
  - Same host-queue pattern for `window.open` (`take_pending_window_opens`) and `history`
    (`take_pending_history` / `handle_history_ops`). **Reuse it for any new host-visible surface
    (downloads, postMessage) — never add a parallel queue** (that mistake cost a rework in Tick 2).
  - The document URL reaches JS via `install(..., doc_url)` → `%URL%` in `WINDOW_PRELUDE`.

## Next action (Tick 4)

UCB pick: **L04 — downloads to disk** (V/C=2.0; self-contained, low-risk, a concrete item from
the original diligence needs list, cleanly HEADLESS-verifiable). Design:

1. Detect a download: a navigation/click whose response has `Content-Disposition: attachment`
   (or a hyperlink with the `download` attribute, or a MIME the engine won't render). The
   response already flows through `shell` `finish_load` / the click path — branch there instead
   of rendering.
2. `engine/net`: expose enough of the response (headers + body bytes + suggested filename from
   `Content-Disposition` / URL basename) to write to disk. A `download_dir()` helper mirroring
   `cookie_store_path()` (`$XDG_DOWNLOAD_DIR` / `~/Downloads`), with filename de-duplication
   (`name (1).ext`).
3. `shell`: a `Download { url, path, total, received, done }` record + a `downloads: Vec<..>`
   field; write the body to `download_dir()/filename`; add a downloads entry to the hamburger
   menu (and/or a small shelf). Keep it synchronous (block_on) for now; streaming-to-disk with
   progress is a follow-on.
4. Verify HEADLESS: a unit test that, given a canned `Response` with `Content-Disposition:
   attachment; filename="report.pdf"` + body bytes, `download_dir()` + the filename resolver
   produce the right path and the bytes are written (to a temp dir via `MANUK_STATE`/env).
   Parity must stay 72/72.

Follow-ons to log: streaming-to-disk with a progress bar; a real downloads shelf UI; open/reveal.

## Then keep going

After Tick 4, re-run §5 UCB over the LEDGER. **Tick 5 is the forced-highest-U tick** (candidates:
L14 fingerprint U7, L16 Shadow DOM U7, L31 llama grounding U8, L32 prerender U8, L33 SoA U7,
L34 service worker U8). Strong normal Tier-A candidates: L02 MutationObserver, L03 postMessage/
opener, L11 responsive `@media`. Each tick: implement → verify (build + parity 72/72 + test) →
disk hygiene → commit+push (co-author line) → update LEDGER/STATE/JOURNAL/RESUME → next.

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md docs/loop/JOURNAL.md
```
