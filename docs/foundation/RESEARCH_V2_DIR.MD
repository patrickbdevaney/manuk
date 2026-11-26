# RESEARCH v2 — Responsiveness, parity, and the agent spine

A parallel research sweep. Each axis below is an independent pass with its own
decision and success criterion — run them separately, fold findings into
IMPLEMENTATION.md as concrete seam-scoped items. Same rules as RESEARCH.md:
primary sources (engine source, specs, WPT, papers) over blog posts; every
finding resolves to reuse-crate / hand-roll-because / defer / out-of-scope.

## Standing constraints (carried from CLAUDE.md — do NOT let this sweep reopen them)

- **JS engine stays SpiderMonkey/mozjs.** Research into QuickJS/Hermes/deno_core
  as *replacements* is OUT — their footprint win comes from not being
  spec-complete JIT engines, which breaks the broad-compatibility mission. They
  may be referenced only as ideas for the *agent-ephemeral* path, never the
  human browser's engine.
- **No bot-evasion / TLS-fingerprint spoofing / anti-detection.** The attached
  research's Obscura/BrowserOxide/wreq/JA3-JA4 material is OUT for the human
  browser. Honest presentation only (Axis F of RESEARCH.md governs the bounded
  legitimate-request question). Do not research evasion.
- **No DRM/EME, no horizontal multi-node scaling, no Chromium-quirk parity**
  beyond WPT compliance. Settled non-goals.
- **Crypto and JIT/GC internals are never hand-rolled.** Reuse audited crates
  (Axis E2) / vendored SpiderMonkey (modification boundary) only.

---

## Axis R — Responsiveness & navigation architecture (HIGHEST PRIORITY)

The screenshots show link-click "unresponsive" prompts and slow back/forward.
Diagnosis: navigation runs the full pipeline synchronously on the UI thread,
and there is no back-forward cache. This is the top felt problem — prioritize
above the feature checklist.

**Scope note, non-negotiable:** "navigation" here means *every* trigger that
loads or re-shows a page — typed search + Enter, clicking a search-result
link, clicking any in-page link, Back, Forward, and Refresh. These are not
separate problems with separate priorities; they are one architectural gap
(synchronous on-thread pipeline, no bfcache) surfaced through six different
entry points. R1-R4 below must each be researched and later implemented
against *all six* triggers, not link-click alone — do not let Back/Forward or
Refresh get a lesser or later treatment than link navigation. The eventual
acceptance target (Axis 8, Measurement) should be a single latency number
measured across all six trigger types, not one per trigger.

**R1. Off-thread navigation.**
Decision: how to move `bytes→DOM→style→layout→paint` off the UI thread so the
chrome stays live (and can show progress / allow cancel) during a load.
Investigate: how Servo/Blink separate the main/compositor/loading threads; what
the minimal split is for Manuk's arena-DOM design (the arena being `Send` is an
asset here); message-passing vs shared-state tradeoffs; how this interacts with
the single-`Runtime`-per-process SpiderMonkey constraint (JS must stay on its
thread — what crosses the boundary).

**R2. Back-forward cache (bfcache).**
Decision: keep prior-page constructed state (DOM+layout+JS heap where safe) in
memory for instant back/forward instead of re-running the pipeline.
Investigate: Blink/WebKit bfcache eligibility rules (what makes a page
cacheable — no unload handlers, no open connections, etc.); memory budget vs
the hibernation model (bfcache'd pages are a *tier* — reconcile with the
compositor's existing tiering); eviction. This is the single biggest
back/forward-latency lever.

**R3. Input responsiveness during load.**
Decision: guarantee the chrome (omnibox, back/forward, tabs) never blocks on
page work.
Investigate: the "unresponsive" prompt's actual trigger (main-loop stall
threshold); coalescing/prioritizing input events ahead of layout work; how the
paint-coalescing already in `shell/gui.rs` extends to input during navigation.

**R4. Speculative/predictive loading.**
Decision: whether to preconnect/DNS-prefetch on link hover *and* on
search-bar typing (predictive preconnect to the likely search endpoint), and
how aggressively.
Investigate: privacy-safe preconnect policy (no cross-origin speculative
connections without opt-in); measured latency win vs Chrome's baseline.

## Axis P — Rendering fidelity ("perfect CSS for virtually all websites")

**Scope note:** the Wikipedia/DuckDuckGo screenshots are two illustrative
examples, not the scope boundary. P1-P5 target general layout/CSS
completeness, verified against WPT plus a diverse real-site sample — not
"make these two specific pages look right."

**P0. Diagnose before researching further — likely regression, not a gap.**
The screenshots show something more specific than "layout is incomplete":
Wikipedia renders with essentially no external stylesheet applied (raw
unstyled tables, default fonts) and a broken/blank box where an inline image
should be — **despite the spec explicitly listing
`fetch_and_apply_stylesheets / images / @font-face (concurrent) → re-layout`
as already part of the pipeline.** This reads as a regression in something
already claimed to work, not a missing feature. Before P1-P5 research
proceeds: instrument the `engine/net` subresource-fetch path for a real page
load (Wikipedia is a good repro case) and confirm whether stylesheet/image
fetches are (a) actually being issued, (b) succeeding, and (c) their results
actually reaching the cascade/paint stage. This is a bounded debugging pass,
not open research — do it first, since it may explain a meaningful fraction
of the "1990s look" without any new feature work at all.

**P1. Layout completeness.** Per-feature WPT-target research (each its own pass):
the remaining from-scratch layout — margin collapsing edge cases, complex
floats, table layout corners, abs/rel/fixed/sticky positioning, overflow/scroll
containers, writing-modes/bidi block flow. Which WPT subdir, current-vs-target
pass count, how Servo structures each. (Extends RESEARCH.md Axis D1.)

**P2. Line-breaking correctness.** Spec notes line-breaking is whitespace-based,
not UAX#14. Decision: adopt a UAX#14 line-breaking crate vs hand-roll.
Investigate: `xi-unicode`/`unicode-linebreak` maturity; the real-site rendering
difference (CJK, hyphenation) it fixes.

**P3. Inline SVG + MathML.** Spec folds these namespaces. Decision: whether
either is in-scope for the target site set, and cost. `resvg` already present
for standalone SVG — what's the gap to *inline* SVG layout.

**P4. Custom Elements + Shadow DOM + SSR/hydration mechanics.** (Carried from
RESEARCH.md Axis D amendment.) `<script type=module>`, dynamic `import()`,
History API `pushState`/`replaceState` — load-bearing for the JS frameworks the
user named (React/Next/Angular/Svelte). Confirm coverage; these are the
difference between "renders" and "interactive."

**P5. The `eval`-string binding round-trip.** Spec's honest gap: some DOM
bindings still go through `eval`'d JS strings. Decision: which hot-path bindings
to move to native reflectors first (correctness + latency). Ties to RESEARCH.md
D3.

## Axis U — User-facing feature parity (the checklist, leanly)

For each: leanest correct implementation, reuse-vs-hand-roll against the
"every hand-roll is maintenance surface" bar, and — critically — does Manuk's
existing engine surface (`a11y` tree, `observe`, `find_in_page`, session
history, `store`) already give the primitive?

**U1. Navigation chrome.** Back/forward/refresh wired to `SessionHistory`
(exists) + bfcache (Axis R2); omnibox quality (suggestions, search-template —
partly exists); a hamburger/settings menu model.
**U2. Bookmarks/favorites + Downloads + History.** All three are
local-store + UI surfaces over data the engine already sees. Design the store
schema once (shares infra with `store` and the session-restore work).
**U3. Keyboard shortcuts.** Ctrl-F (find-in-page exists), C/V/X/A (needs a
selection model over the fragment/text tree — does one exist? likely the
biggest new surface here), zoom (layout-reflow vs transform — decide).
**U4. Tabs/windows/tab-groups + new/duplicate/close tab + new window.**
Over the compositor's tiering; duplicate-tab = clone session state.
**U5. Password store + autofill.** `store` crate exists (audited crypto, no
hand-roll). Research the origin-matching/anti-phishing rules for autofill
correctness (a wrong-origin autofill is a real vuln) and BYO-inference-key
storage reuse.
**U6. Persistent cookies across sessions (weeks).** Spec's HTTP layer has no
persistent cookie jar yet. Decision: disk-backed, encrypted cookie store;
expiry/eviction; how it composes with `store`. This is what "stay logged in"
actually requires.
**U7. Google OAuth v3 popup flow.** The named hard case: login opens a
popup/new window, completes, and the *original* tab continues. Investigate:
`window.open` + `postMessage` cross-window communication, `window.opener`,
popup window lifecycle — this needs real multi-window JS context plumbing, not
just UI. Likely exposes gaps in the window/global model. Treat as its own
focused item; it's a good forcing function for cross-context correctness.
**U8. Zoom + translate-page.** Zoom decided in U3. Translate: local-model
(reuse the bundled llama.cpp/Gemma path) vs API; DOM text-node
extract/reinject preserving structure.
**U9. Downloads + uploads.** Download-to-FS (stream to disk); file upload
(`<input type=file>` — needs the file-picker + form-data plumbing; some
failures may be target-site JS, not Manuk — distinguish).

## Axis DT — Developer tools (Ctrl-Shift-I, "gecko/chromium parity")

**DT1. Protocol choice — the strategic fork.** Implement the DevTools surface
over **WebDriver-BiDi** (the `bidi` crate already exists, standards-track) vs a
bespoke inspector vs CDP. BiDi is the strategic pick: it's already in-repo, and
the *same* protocol that drives dev-tools can drive the *agent* — one surface,
two consumers (mirrors the whole project thesis). Investigate what the minimal
useful panels are (DOM inspector over the arena, console over the SpiderMonkey
runtime, network over `engine/net`, layout/box-model over the fragment tree)
and how much each reuses existing engine introspection.

## Axis AG — Agent spine (external tool API + in-browser agent)

Consolidates and extends RESEARCH.md Axes E6/H with the attached research's
genuinely-useful ideas (constraint-bound — the evasion material stays out).

**AG1. External automation API surface.** The `bidi` WebDriver-BiDi remote end
already exists as the standards-track drive interface. Decision: is BiDi
sufficient as *the* external agent API, or is a leaner in-process API also
warranted? Investigate what Vercel agent-browser / Playwright-BiDi expect, so
external harnesses point at Manuk with zero adaptation.
**AG2. Observation-reduction for LLM context.** From the attached FocusAgent /
LineRetriever papers: dynamic AXTree pruning driven by task intent, not static
truncation — directly serves "big-KV or small model of any size" (RESEARCH.md
H2). Manuk's `a11y` tree + semantic `diff` are the substrate. Decision: where
the pruning/retrieval layer sits (engine vs harness).
**AG3. Semantic vs visual grounding.** From AgentQL (semantic locators) vs
Skyvern/Tarsier (visual OCR grid). Manuk owns both channels (a11y tree +
screenshot) — decision: expose both targeting modes (RESEARCH.md H1), let the
model use whichever it's trained for. The Tarsier indexed-overlay pattern is
directly applicable to the screenshot channel.
**AG4. Prompt-injection defense.** From the WAAA paper + the confused-deputy
risk: page-derived observations are already provenance-tagged (`E6`
Action-Guard). Research the actual attack surface and whether the fence is
sufficient; this gates shipping the in-browser agent panel.
**AG5. In-process advantage, quantified.** The spec's differentiator: in-process
agent skips the CDP/WebDriver delta-serializer + dual-node-ID machinery. Decision:
measure this — the claim needs a number vs a CDP-over-socket baseline, per the
project's measured-not-asserted discipline.

## Axis MEM — Memory topology & lean footprint

**MEM1. Data-oriented DOM (SoA).** From the attached DOD research: struct-of-
arrays DOM for cache-friendly reflow + SIMD. Decision: is this worth a
reflow-hot-path refactor of the arena, or does the generational-`NodeId` arena
already capture most of the locality win? Investigate honestly — this is a large
refactor and may be premature; disposition could well be "defer, measure current
reflow cache behavior first."
**MEM2. Hibernation realized + bfcache reconciled.** The tiering is policy-only
per spec. Decision: realize freeze-JS + evict-tiles, and reconcile with Axis R2
(bfcache pages are a retained tier — one memory model, not two competing ones).
**MEM3. Binary-size measurement.** `spidermonkey-noicu` exists; measure the ICU
delta and the whole-binary breakdown (`cargo bloat`); dedupe the duplicate crate
resolutions the spec notes. All inside the modification boundary (build config
only).

## Axis SRC — Local engine-source repomapping (Chromium/Gecko/WebKit subdirs)

The user has Chromium/Gecko/WebKit source as repo subdirs. Use them as
**reference for how a specific algorithm is structured**, never as code to copy
(license + the from-scratch mandate). For each hard subproblem above, the
research step can include reading the corresponding real-engine source:

**SRC1. Targeted source reads, per open problem.** bfcache eligibility
(Blink `Document::CanBeCached`-style logic); float/table layout structure;
bfcache/threading boundaries; OAuth popup `window.opener` plumbing; DevTools
protocol surface. Decision each read feeds: a specific IMPLEMENTATION.md item's
design. Extract *approach and edge-case lists*, not code — the from-scratch,
WPT-verified mandate holds; the source is a spec-clarifier for
under-documented behavior, same role the CLAUDE.md methodology gives WPT.
**SRC2. What NOT to take.** Do not import Chromium's process model, its
quirk-compat cruft, its evasion-adjacent networking, or its IPC architecture —
Manuk's leaner in-process model is a deliberate divergence. The source is a
reference for *correctness*, not *architecture*.

---

## Output & sequencing

Per pass: findings (primary-source-backed, dated) → disposition per item →
specific IMPLEMENTATION.md edit → new WPT target where relevant.

Priority order: **Axis R first** (it's the felt problem), then P (fidelity) and
U (parity) in parallel, then DT/AG (the agent+devtools surface, which share
BiDi), then MEM and the SRC reads folded into whichever axis needs them. Do not
run this as one giant pass — it's ~8 axes; stage them the way RESEARCH.md's are
staged, single-agent, no subagent fan-out, capped searches per item, honest
deferral over forced-confident dispositions on the architecture-design items
(R1/R2, MEM1, DT1 especially).