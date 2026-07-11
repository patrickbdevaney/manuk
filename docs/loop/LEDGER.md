# Manuk — LEDGER (scored work backlog)

_The selection source for [[CONSTITUTION]] §5 (UCB). V=value, C=cost, U=uncertainty (all 1-10),
T=times touched. `score = V/C + 1.5*sqrt(ln(1+TICKS)/(1+T)) + 1.0*(U/10)`. Every 5th tick force
the highest-U item. Update T/status/scores each tick; add items on reflection; mark dead ends
`superseded`. Verify class per §7: HEADLESS / GUI / EXTERNAL / MEASURE._

**TICKS = 9** (global tick counter; increment each tick). _Ticks done: 1 L14-slice, 2 L01
fetch/XHR, 3 L10 pushState, 4 L04 downloads, 5 L32 prerender, 6 L03 postMessage/opener, 7 L02
MutationObserver, 8 L11 responsive @media. Tick 9 pick: **L17 AG2 task-intent AXTree pruning +
AG3 dual (semantic+visual) targeting** — top UCB (~4.3), deferred through Ticks 2–8 by the
human-first ordering; table stakes now solid, so the agent-native differentiator is un-deferred.
NOTE: Tick 10 is the forced-highest-U tick._

## Tier A — absorb outstanding beneficial work already suggested (do first)

These come from the tier plan, the RESEARCH_V2 directive, IMPLEMENTATION.md follow-ons, and
STATE weak frontiers. High V, mostly known ⇒ high exploit ⇒ front-loaded.

| id | intent | axis | V | C | U | T | status | verify |
|----|--------|------|---|---|---|---|--------|--------|
| L01 | `fetch()` + `XMLHttpRequest` in page JS (SPA data loading) | JS/COMPAT | 10 | 5 | 4 | 1 | **done** (Tick 2) | HEADLESS |
| L02 | `MutationObserver` (SPAs mutate DOM post-load) | JS/COMPAT | 9 | 5 | 4 | 1 | **done** (Tick 7) | HEADLESS |
| L02b | `IntersectionObserver` + `ResizeObserver` + characterData oldValue nuance | JS/COMPAT | 6 | 6 | 5 | 0 | backlog (L02 follow-on) | HEADLESS |
| L03 | Cross-window `postMessage` + `window.opener` (finish OAuth popup return) | JS | 9 | 5 | 5 | 1 | **done** (Tick 6) | HEADLESS |
| L03b | `BroadcastChannel` + `MessageChannel`/`MessagePort` + full structured clone (Blob/Map/Set) + `window.name` targeting | JS | 4 | 6 | 5 | 0 | backlog (L03 follow-on) | HEADLESS |
| L04 | Downloads to disk (Content-Disposition, stream to file, manager entry) | NET/UI | 8 | 4 | 3 | 1 | **done** (Tick 4) | HEADLESS |
| L04b | `<a download>` attribute trigger + streaming-to-disk progress shelf UI + open/reveal | NET/UI | 4 | 5 | 3 | 0 | backlog (L04 follow-on) | GUI |
| L05 | File uploads (multipart from `type=file`) | NET/JS | 7 | 5 | 4 | 0 | backlog | HEADLESS |
| L06 | Wire password store + autofill (keyring-derived key, save/fill on forms) | UI/SECURITY | 8 | 6 | 5 | 0 | backlog | EXTERNAL |
| L07 | Wire semantic history index (record visits, query in omnibox) | UI/MEM | 7 | 5 | 4 | 0 | backlog | HEADLESS |
| L08 | Wire page-translate (menu item → agent translate backend) | UI/AGENT-IN | 6 | 5 | 5 | 0 | backlog | EXTERNAL |
| L09 | DevTools panel over BiDi (DOM tree, console, network) Ctrl+Shift+I | UI/AGENT-EXT | 8 | 8 | 5 | 0 | backlog | GUI |
| L10 | `history.pushState`/`popstate` SPA routing correctness end-to-end | JS/COMPAT | 8 | 4 | 4 | 1 | **done** (Tick 3) | HEADLESS |
| L10b | Same-document Back/Forward *button* → popstate w/ per-entry state restore (SessionHistory same-doc flag) | JS/UI | 5 | 4 | 4 | 0 | backlog (L10 follow-on) | HEADLESS |
| L11 | Responsive-skin correctness: `@media` + collapsed-menu on Wikipedia-class | RENDER | 8 | 6 | 6 | 1 | **done** (Tick 8) | HEADLESS |
| L11b | Container queries + matchMedia resize-listeners + full media-feature set (resolution/aspect-ratio/hover/pointer) + `@supports` | RENDER/JS | 5 | 6 | 5 | 0 | backlog (L11 follow-on) | HEADLESS |
| L12 | New window (2nd winit window) + duplicate/close semantics | UI | 5 | 7 | 5 | 0 | backlog | GUI |
| L13 | Off-thread the external-CSS/image fetch phase (R1 follow-on) | PERF | 6 | 5 | 4 | 0 | backlog | HEADLESS |
| L14 | Complete human fingerprint surface (screen, timezone, fonts, canvas/WebGL consistency) | FINGERPRINT | 7 | 6 | 7 | 0 | backlog | HEADLESS |
| L15 | Inline SVG rendering (P3) | RENDER | 6 | 7 | 6 | 0 | backlog | HEADLESS |
| L16 | Custom Elements + Shadow DOM basics (P4) | JS/RENDER | 6 | 8 | 7 | 0 | backlog | HEADLESS |
| L17 | AG2 task-intent AXTree pruning + AG3 dual (semantic+visual) targeting | AGENT-EXT | 6 | 4 | 5 | 0 | backlog | HEADLESS |
| L18 | Cookie partitioning + `SameSite` enforcement audit | NET/SECURITY | 5 | 4 | 4 | 0 | backlog | HEADLESS |
| L19 | Settings page / preferences surface | UI | 4 | 5 | 4 | 0 | backlog | GUI |
| L20 | PERF profile vs Chromium: nav + reflow + paint timings, publish numbers | PERF | 7 | 4 | 6 | 0 | backlog | MEASURE |
| L21 | Non-blocking async page-fetch (don't `block_on` the UI thread; spawn + deliver on completion) | PERF/JS | 6 | 5 | 5 | 0 | backlog (L01 follow-on) | HEADLESS |
| L22 | Real request/response fidelity for fetch: headers, `Request`/`Headers`/`Response` objects, credentials/cookies on XHR | JS/NET | 6 | 5 | 5 | 0 | backlog (L01 follow-on) | HEADLESS |
| L23 | `AbortController` / `signal` for fetch + XHR `abort()` wired to cancel the host request | JS | 4 | 4 | 4 | 0 | backlog (L01 follow-on) | HEADLESS |

## Tier B — new innovation surface (exploration; higher U)

| id | intent | axis | V | C | U | T | status | verify |
|----|--------|------|---|---|---|---|--------|--------|
| L30 | In-process automation tool surface hardening (stable selectors, wait-for, assertions) as the agent-native differentiator | AGENT-EXT | 9 | 6 | 7 | 0 | backlog | HEADLESS |
| L31 | llama.cpp agent: prompt→action grounding over the a11y tree, replayable | AGENT-IN | 8 | 7 | 8 | 0 | backlog | EXTERNAL |
| L32 | Speculative/predictive prerender of likely-next navigations | PERF | 6 | 7 | 8 | 1 | **done** (Tick 5) | HEADLESS |
| L32b | Idle (non-hover) prerender from ranked content links + surfaced prewarm hit-rate metric | PERF | 4 | 5 | 5 | 0 | backlog (L32 follow-on) | MEASURE |
| L33 | Memory: measure reflow-cache hit rate, then SoA-DOM only if it pays | MEM | 6 | 8 | 7 | 0 | backlog | MEASURE |
| L34 | Service worker / offline cache subset | NET/COMPAT | 6 | 9 | 8 | 0 | backlog | HEADLESS |

## Done (this loop's precursor session + ticks)

interactive-JS-keystone · persistent-cookies · spidermonkey-default · clicks→JS · fast-exit ·
clipboard+selection · tab-strip · hamburger-menu · suggestions/history-dropdown · scrollbar ·
position:sticky · grid-template-areas · UAX#14-linebreak · preconnect(R4) · AG5-latency ·
MEM3-binary-size · window.open→new-tab · **fetch()+XHR real Promises (L01, Tick 2)** ·
**history.pushState/replaceState/popstate + location (L10, Tick 3)** · **downloads to disk
(L04, Tick 4)** · **predictive prerender into bfcache (L32, Tick 5)** · **cross-window
postMessage + window.opener (L03, Tick 6)** · **MutationObserver (L02, Tick 7)** ·
**responsive @media + matchMedia (L11, Tick 8)**.

## Superseded / blocked

_(none yet)_
