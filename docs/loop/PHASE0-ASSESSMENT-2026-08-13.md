# Phase-0 Progress Assessment — 2026-08-13

_Observer assessment, written at tick ~1228. Honest, data-grounded snapshot of how close the engine is to
the owner-locked daily-driver definition: **≥95% of representative real (CrUX) sites both RENDER acceptably
AND FUNCTION on the capabilities they use.** This separates the WPT *steering* climb from the actual
daily-driver *exit* bar, because as of this date they have diverged._

## Headline

| Axis | Where we are | Exit bar | Status |
|------|-------------|----------|--------|
| **M1 — real-site render** (shape≥0.75 AND jarring-clean, in-scope CrUX) | **~29%** (t1226 f12 = 28.8%) | ≥95% | **binding constraint; ~flat this session** |
| **M2 — real-site function** | **uncertified** (primitives broad; no real-site A/B cert yet) | ≥95% render∧function | **not measured on real sites** |
| WPT spec-conformance (*steering only, not the bar*) | **73.92%** (92778/125519 active-areas) | — | climbed 66→74% this session |

## M1 — visual rendering / placement: ~29%, plateaued

- Latest real-site sweep (t1226): **in-scope pass 28.8%** vs a 95% exit. Recent sweeps have oscillated
  ~29–33% (28.9 → 32.6 → 31.6 → 29.1 → 30.0 → 28.8), i.e. **roughly flat over the last ~80 ticks.**
- **Coverage is ~87% — we draw the right boxes, in the wrong places.** The gap is *placement geometry*,
  not missing content: fully-covered anchor pages score low (news.ycombinator 0.72, wikipedia 0.52,
  a11yproject 0.44, blog.rust-lang 0.63). The error concentrates in ~4–5 mechanism families
  (dy-accumulation / width-launder-into-dy / `display` mis-computation / overflow / grid-flex-sizing).
- **~22% of in-scope sites (≈29 of 130) do not render at all** — throw-class boot-blockers (a touched API
  that throws/hangs aborts boot before any render). This is the "larger half" and has been under-worked;
  it is also the *scorability ceiling* that caps M1 arithmetically.

## M2 — interactivity / function: broad primitives, no real-site certification

- ~400 capability primitives present/gated (CONSTELLATION.tsv), and the WPT function surface
  (DOM, CSSOM, selectors, iframe, JS preemption, …) climbed strongly this session. Hand-built capability
  fixtures pass.
- **But function is proven only on fixtures, not the corpus.** Per DAILY-DRIVER-CERTIFICATION.md: "daily-driver
  is fundamentally about function, and it is not currently certified against real sites at all." The missing
  Phase-0 leg is a stratified, functional, real-site smoke suite A/B-diffed against Chromium — deferred until
  the scorability ceiling clears (~85%), because a site that won't boot can't be A/B'd.

## The key tension (why the two numbers diverged)

This session's engine gains were real but landed on the **spec/function axis** (WPT 66→74%: reflection,
selectors, cssom, iframe identity, JS preemption) — the *fast, high-flip* veins. **Real-site render stayed
flat at ~29%** because placement geometry is the *byte-exact layout slog* where one fix flips ~nothing on
WPT. This is the known "DOM is the fast WPT climb, layout is the M1 slog" model, playing out.

## Steer set 2026-08-13 (owner directive)

Prioritize the two levers that actually move M1 render, over the DOM/spec veins that climb WPT% but leave
the render number flat:

1. **Throw-class render-blockers first** — get the ~22% non-rendering sites to boot + populate DOM. Raises
   scorability (the M1 cap) and builds the M2 function cert for free.
2. **Then placement geometry** — the ~4–5 mechanism families above; PORT whole algorithms from
   `blitz/` (Taffy+Stylo, our exact stack) and `servo/`, not per-assertion reverse-engineering.

WPT-total stays the monotonic progress ledger; the change is *which vein within it* the loop works —
render-moving areas, not pure high-flip DOM. Exit bar unchanged. Wired into `scripts/lever-board.sh`.

## Bottom line

Not close to a rendering daily driver: ~29% of real sites render acceptably (plateaued), against 95%.
Function is ahead on primitives but unmeasured on real sites. The WPT 74% is encouraging steering progress,
not the finish line — placement geometry and the throw-class render-blockers are the binding constraints,
and they are now the steer.
