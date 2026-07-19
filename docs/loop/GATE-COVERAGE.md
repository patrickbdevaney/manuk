# GATE COVERAGE — the wall runs 19 of 104 page gates (measured, tick 239)

> **Observer-actionable.** `scripts/` is observer-owned; this file is the agent's measurement and the
> exact change it implies. Nothing here was applied to the harness.

## The finding

`engine/page/tests/` holds **104** gate files. `scripts/verify.sh` names **19** of them in `_launch`
lines. There is no glob, no sweep, and no package-wide `cargo test -p manuk-page` in the wall — the
only package-wide invocation (`verify.sh:96-104`) is a **pre-warm with `--no-run`**, which builds the
binaries and never executes them. So **85 page gates do not run in the verify wall.**

This is not a style point. `docs/loop/CONSTELLATION.tsv` marks rows `gated` and names gates in that
85 — `g_mse`, `g_media_buffered`, `g_canvas_text`, `g_canvas_image`, `g_hydration`, `g_oauth_redirect`,
`g_crypto`, `g_a11y_state`, `g_scroll`, `g_popover`, `g_capability`. Those rows claim a ratchet tooth
that nothing bites on. A regression in any of them lands green.

## What the measurement actually found (the part that matters)

Full sweep: `cargo test -q -p manuk-page --features stylo,spidermonkey --no-fail-fast`
**98 passed, 2 failed** across 86 targets. So no capability had been silently lost — but two were red:

1. **`g_capability` — RED, and it is the gate that guards the ledger.** Its whole purpose is catching
   stale `WEB-PATTERNS.md` claims, and it had itself gone stale. The claim
   `createDocumentType('')` throws `InvalidCharacterError` is the **pre-2020 QName rule**; the DOM spec
   now uses "valid doctype name" (reject only ASCII whitespace, U+0000, `>`). WPT
   (`dom/nodes/DOMImplementation-createDocumentType.html`) expects a doctype back for `''`, `1foo`,
   `@foo`, `:foo`, `foo:`, `a.b:c`, and throws for exactly two names (`edi:>`, `edi:a `). Tick 135
   relaxed the engine correctly and left the claim and the code comment describing the old rule.
   **Fixed in tick 239 — the claim moved, not the engine.** Red for ~100 ticks, unseen.
2. **`manuk-page --lib :: hard_wall_detection_and_honest_interstitial` — RED.** Pre-existing and
   already known (session 195-196 recorded that verify.sh misses it). Still missed.

**The generalisation, which is this project's own lesson #1 wearing a new hat:** a gate that is not
*invoked* is indistinguishable from a gate that passes. `falsify.sh` mutation-tests the gates that
run; nothing tests whether a gate runs at all. The 85 were written, proven to go red at authoring
time, committed — and then never asked again.

## The change this implies (observer's call, not applied)

Adding 85 `_launch` lines is the wrong shape — it hard-codes the same list that just went stale, and
some of these should NOT be in the per-tick wall:

- **Network-dependent / live** (would make the wall flaky or slow, likely deliberate exclusions):
  `g_eventsource`, `g_eventsource_reconnect`, `g_fetch_stream`, `g_fetch_stream_incremental`,
  `g_oauth_popup`, `g_oauth_redirect`, `g_scroll_anchor_live`, `g_websocket`, `g_websocket_live`,
  `g_xhr_progress`, `webfont_live`.
- **Everything else** is a plain in-process gate with no external dependency.

The shape that cannot go stale again is a **sweep with an explicit deny-list**, so a newly added gate
is watched BY DEFAULT and skipping one is a deliberate, named act:

```sh
# one invocation, all page gates, minus a NAMED exclusion set
cargo test -q -p manuk-page --features stylo,spidermonkey --no-fail-fast \
  $(printf -- '--test %s ' $(ls engine/page/tests/*.rs | xargs -n1 basename | sed 's/.rs$//' \
      | grep -vE '^(g_eventsource|g_eventsource_reconnect|g_fetch_stream|g_fetch_stream_incremental|g_oauth_popup|g_oauth_redirect|g_scroll_anchor_live|g_websocket|g_websocket_live|g_xhr_progress|webfont_live)$'))
```

⚠ **Wall cost is unmeasured and is the reason this is not a one-line yes.** The 86-target sweep above
took minutes of wall on a warm tree; the wall ceiling is 93s. The honest options are (a) run the sweep
OFF the per-tick path like FID-SWEEP, banking a pass/fail into `RATCHET.tsv`, or (b) add the cheap
subset to the wall and sweep the rest nightly. **(a) is the safer default** — it makes the 85 watched
without putting the tick loop's throughput at risk, which is exactly the trade FID-SWEEP already made.

## Residue

The same question has not been asked of the other crates: `manuk-shell`, `manuk-dom`, `manuk-css`,
`manuk-layout`, `manuk-paint`, `manuk-net`, `manuk-agent` run package-wide (`verify.sh:100-101`, `T ·
crate tests`), so their tests DO all run. `manuk-media`'s two gates (`audio_decode`, `video_decode`,
ticks 235/236) are `required-features` tests and appear in **no** wall invocation — they are watched by
nothing at all.
