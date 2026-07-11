# Manuk — STATE (living snapshot)

_Updated by the loop each tick that changes reality. Take-stock source of truth. See
[[CONSTITUTION]] for how this is used, [[LEDGER]] for the scored backlog._

## What Manuk is (2026-07-11)

~37k-LOC memory-safe Rust browser, 16 crates: `engine/{net,dom,html,css,layout,text,js,paint,
compositor,page,a11y}`, `shell`, `agent`, `bidi`, `store`, `tests/wpt`. Real Stylo cascade
(default) with a MinimalCascade panic-fallback; Taffy flex/grid over the arena DOM; swash text;
SpiderMonkey JS (mozjs, **now the shell default**); tiny-skia CPU paint + wgpu GPU present; winit
window; hyper/rustls net with persistent cookies + HTTP cache + adblock. Agent-native: stable
arena `NodeId` handles, typed `BrowserAction`, a11y-tree diffing, in-process automation.

Gate: `cargo run -q -p manuk-wpt --release -- parity` = **72/72**.

## Axis scores (0-100; rough, self-assessed; move only with evidence)

| Axis | Score | Notes |
|------|------|-------|
| RENDER | 73 | Stylo cascade, flex/grid, grid-template-areas, UAX#14 breaking, position:sticky, **responsive `@media` (rules apply by real viewport; matchMedia agrees)**. Complex responsive skins (Wikipedia Vector-2022) closer; container queries + collapsed-menu JS still partial. |
| JS | 73 | **Interactive**: persistent per-page context, real clicks fire listeners, preventDefault. **fetch()/XHR real Promises + host round-trip**; **pushState/popstate + real `location`**; **window.open + cross-window `postMessage`/`opener`**; **MutationObserver** (attributes/childList/characterData/subtree, microtask-batched). Missing: IntersectionObserver/ResizeObserver, many WebIDL APIs. |
| NET | 62 | hyper+rustls, HTTP cache (RFC-9111 subset), RFC-6265 cookies **persistent across sessions**, preconnect, adblock, page-fetch bodies, **downloads to disk (Content-Disposition/binary, de-dup, path-safe)**. Missing: HTTP/2 push nuance, service workers, async non-blocking page-fetch (currently block_on), streaming-to-disk. |
| UI | 58 | Tab strip (new/close/dup), hamburger menu, omnibox suggestions/history dropdown, scrollbar, find, zoom, text selection + clipboard; omnibox URL tracks SPA pushState routes; **downloads saved to disk + listed in the menu**. Missing: richer downloads shelf, settings page, richer a11y. |
| PERF | 58 | Off-thread nav, bfcache, preconnect, **predictive prerender (hover → full page built into bfcache → instant click)**, partial GPU upload, shaping caches. Not yet profiled against Chromium; cold-start ~73ms tiny page. |
| MEM | 55 | Tab hibernation (discard/restore). No SoA DOM yet (deferred, measure first). Binary 16.4 MB (Stylo+SM). |
| AGENT-IN | 40 | llama.cpp/GGUF in-browser agent panel (Ctrl+J), typed actions. Depends on local model. |
| AGENT-EXT | 45 | In-process typed `BrowserAction` + a11y targeting; AG5 measured ~12× lower per-command latency than CDP-over-socket. BiDi surface exists (no DevTools UI). |
| FINGERPRINT | 33 | Honest human UA + navigator + **boot window/screen metrics** (innerWidth/screen/dpr/matchMedia/rAF). Not yet complete (fonts, timezone, canvas/WebGL consistency, true window size). |
| COMPAT | 53 | Simple + table-driven sites faithful (example.com, HN). Boot-metric ReferenceErrors fixed; **fetch/XHR + pushState routing + postMessage/opener + MutationObserver** (SPA hydration, routing, OAuth-return, post-fetch DOM-watching all work). Remaining SPA gaps narrowing; responsive `@media` correctness (Wikipedia-class) still partial (next). |
| STABILITY | 55 | Parity green; fast-exit avoids mozjs teardown crash. GUI paths unverified headlessly. |
| SECURITY | 45 | Provenance-tagged agent observations + Action-Guard; adblock; no site sandbox/process isolation (deliberate in-process model). |

## Recently landed (this loop's precursor session + ticks)

Interactive JS keystone; persistent cookies; SpiderMonkey default; clicks→JS + fast-exit;
clipboard + text selection; tab strip; hamburger menu; suggestions/history dropdown; scrollbar;
position:sticky; grid-template-areas; UAX#14 line-breaking; speculative preconnect; AG5 latency
measurement; MEM3 binary-size measurement; boot window/screen metrics (Tick 1); **fetch()/XHR
real Promises + host round-trip (Tick 2)**; **history.pushState/popstate + location (Tick 3)**;
**downloads to disk (Tick 4)**; **predictive prerender into bfcache (Tick 5)**; **cross-window
postMessage + window.opener (Tick 6)**; **MutationObserver (Tick 7)**; **responsive @media +
matchMedia (Tick 8)**.

## Known weak frontiers (feed exploration)

Complex-SPA support (fetch/XHR/MutationObserver/observers, history routing); DevTools; downloads/
uploads; cross-window postMessage/opener; complete human fingerprint surface; profiled PERF vs
Chromium; the unwired subsystems (passwords/autofill, translate, semantic history) need callers +
external pieces; `@media`/responsive-skin correctness on Wikipedia-class layouts.
