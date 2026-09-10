# Phase 0 Completion — Deep Research + Implementation Plan + Loop Steer

**Author:** observer · **Date:** 2026-09-10 · **Tick at authoring:** 1479 · **Loop state:** suspended (`.git/manuk-loop-DISABLED`)

Purpose: the decisive, evidence-grounded steer to finish **Phase 0** (a daily-driver,
agent-native browser) in the **fewest ticks**, with **step-change (super-linear) growth**
instead of the decimal-grind asymptote. This supersedes the "alternate A/B/C, no track
>5 ticks dark" nudges as the operative selection rule.

---

## 0. TL;DR — the one thing that changes

Phase 0's exit is a **function-gated conjunction**: `≥95% of representative sites RENDER (shape≥0.75 on ≥95% of nodes, jarring-clean) AND FUNCTION (their used capabilities pass A/B vs Chromium)` (`DAILY-DRIVER-CERTIFICATION.md:46`, `PHASE0-MEASUREMENT-SYSTEM.md:71`).

The **binding constraint** on that number today is the **SCORABILITY / FUNCTION ceiling**, not layout placement:
- `M1M2-PLATEAU-BREAKER-PLAN.md:14` — **~82% scorability ceiling: 24 of 135 in-scope sites never yield a scored tree.** M1 cannot arithmetically exceed ~82% until those sites become scorable, regardless of layout quality.
- Coverage is already **~87–99%** ("right boxes, wrong places", `PHASE0-ASSESSMENT:20`). The boxes exist; scorability and placement are the gaps.
- Function **leads** render — "M1 and M2 are two terms of one function-gated number" (`PHASE0-MEASUREMENT-SYSTEM.md:79,190`). A site with an app-halting JS exception is *unscorable*, so it fails render by construction.

**Consequence:** ~30 of the last ~35 ticks were Track-A layout porting — improving shape on sites that were *already scorable*, while the exit metric is capped by the ~18% that fail **function**. That is the mechanical reason "61 ticks moved the render headline by one site" (`session-1406-1407`). The growth was sub-linear because the loop was grinding a **non-binding** constraint.

**The steer:** stop selecting work by track rotation. Select it by **which gate is arithmetically capping the exit conjunction**, and attack it in **dependency order** (scorability → M2 function cert → a11y → render polish). Each unscorable site shares a cause with others (the project's own recurring "one expression = 94% of the error" finding, `session-1404-1405`, `session-1349-1350`), so each fix flips *multiple* sites — the step-change the loop needs.

---

## 1. The deep-research prompt (formulated, repo-grounded — reusable)

> **Context.** Manuk is a from-scratch, Rust, memory-safe, **agent-native** web browser
> (mozjs/SpiderMonkey for JS, Stylo for CSS cascade, Taffy for layout, Blitz/Servo as
> porting sources, plus large native areas). Thesis: a browser a human daily-drives *and*
> that exposes a **unified agent-driving surface** — the accessibility tree (role + name +
> state + click box, computed with no JS from the parsed DOM + solved layout) as the
> agent's perception channel *instead of* a screenshot, plus stable semantic element
> addressing and event actuation. It is built by an autonomous "grind" loop that lands one
> gated, WPT-ratcheted commit per tick (~1500 ticks in). **Phase 0 = daily-driver: ≥95% of
> a stratified real-web (CrUX) corpus both RENDER (shape≥0.75 on ≥95% of nodes,
> jarring-clean) and FUNCTION (used capabilities pass A/B vs Chromium via WebDriver BiDi);
> a11y tree role+name+state matches Chrome on ≥90% of nodes.** WPT pass-rate (~74%) is the
> per-tick *climb instrument*, never the certificate.
>
> **Question.** Given a from-scratch engine already at ~74% WPT / ~87–99% node coverage but
> a ~82% real-site scorability ceiling and shape (placement) as the weak axis: what is the
> **minimum-tick methodology** to reach the ≥95% render∧function daily-driver bar? Specifically:
> 1. How did Servo and Ladybird structure work to get **step-change** conformance gains
>    (whole-subsystem/algorithm porting, WPT drill-down specs→sections→tests, reftest
>    discipline) — and where did each hit its asymptote, and why?
> 2. For real-web *usability* (not test pass-rate), what makes sites **fail to render/function
>    at all** (script errors halting the DOM, missing boot-critical Web APIs, hydration), and
>    what is the highest-leverage order to eliminate those shared failure causes?
> 3. For the **agent-native** surface: what is the state of the art an agent needs to drive
>    the whole (legacy) web — accessibility tree vs DOM-pruning vs screenshots (cost/latency/
>    accuracy), WebDriver BiDi `input.performActions`, CDP, and WebMCP (`navigator.modelContext`)
>    — and what can a *browser* provide natively that bolt-on-to-Chrome stacks cannot?
> 4. What loop/process design produces **linear-or-super-linear** capability growth rather than
>    logarithmic asymptote (targeting the binding constraint, aperture-before-mechanism,
>    shared-cause fixes, avoiding per-assert grind)?

---

## 2. External findings (the research)

**Method — how the closest analogs actually grew:**
- **Ladybird** (from-scratch, nearest peer): >90% WPT, ~2.07M passing subtests by making WPT a **first-class CI metric on every commit** + daily full-suite runs + a custom multi-mode runner (Layout/Ref/Text/Screenshot). Big gains came from **implementing whole subsystems to spec**; the later curve flattens (matches manuk's observed Ladybird asymptote, `session-1403`).
- **Servo**: drives work by drilling the WPT dashboard **specs → sections → tests → subtests**, organized by **subsystem** (abspos, floats, positioning, box-model); **reftests** for layout, testharness for DOM. Layout-engine migration compared old-vs-new per focus-area to find "where it most needed catching up" — the **aperture** method (manuk's `session-1413`).
- **Interop 2024–25**: the conformance long tail concentrates in **Layout (flex/grid/subgrid) + fragmentation** — all whole-algorithm work (the super-linear class), not per-assert.

**The agent-native surface — the distinction, made concrete:**
- Accessibility tree is the **winning perception surface**, not screenshots: a UC-Berkeley/Michigan 2026 study found agent task success drops **78%→42%** on a degraded a11y tree; browser-use hits **89.1%** WebVoyager on a11y-tree input; a 0.6B model hits 88.1% F1 with DOM pruning. Screenshots cost ~**$0.40** and +2–3s/page vs ~**$0.04–0.06** and +0.2–0.5s for a structured tree.
- Actuation standard is **WebDriver BiDi** (`input.performActions`), W3C, cross-browser — **but BiDi has *no* first-class accessibility-tree surface** ("CDP's accessibility-tree access has no equivalent in BiDi today").
- **WebMCP** (`navigator.modelContext`) is the emerging semantic ideal (no DOM scraping, no CSS selectors, ~89% token savings) **but only works on sites that adopt it** — useless on the legacy web.
- **→ Manuk's opening (no incumbent ships this):** compute a **WebMCP-quality semantic agent surface for the entire legacy web, natively** — a11y tree (perceive) + stable semantic addressing (manuk's M2 ordinals / `@e1`-style refs) + BiDi-style actuation — so an agent drives *any* site at low latency / low token cost, no brittle selectors, no screenshot inference. This is Phase 2's ingress and it is *already* what M2 + a11y are building.

Sources: [Servo & WPT](https://servo.org/blog/2023/07/20/servo-web-platform-tests/) · [Ladybird 90% milestone](https://alternativeto.net/news/2025/10/ladybird-passes-the-apple-90-thresholf-on-web-platform-tests-marking-a-major-milestone/) · [WebDriver BiDi (Chrome)](https://developer.chrome.com/blog/webdriver-bidi) · [BiDi input module (MDN)](https://developer.mozilla.org/en-US/docs/Web/WebDriver/Reference/BiDi/Modules/input) · [Accessibility tree & AI agents](https://www.webyes.com/blogs/accessibility-tree-ai-agents/) · [Agent browser landscape 2026](https://zylos.ai/research/2026-04-05-browser-automation-ai-agents-2026-landscape/) · [Interop 2025 (WebKit)](https://webkit.org/blog/17808/interop-2025-review/)

---

## 3. Why growth has been sub-linear (the diagnosis)

1. **The exit is a conjunction, function-first — but work was selected by track rotation, not by the binding term.** With render capped at ~82% scorability, layout ticks (Track A) raise a term that is not the limiter. The exit number barely moves → looks like asymptote.
2. **Per-tick metric ≠ exit metric.** The loop climbs monotonic **WPT total** each tick, but reads the **exit** (CrUX render∧function, scorability) only on a ~6h sweep. So it can grind WPT for dozens of ticks with zero exit movement and not notice (`session-1406-1407`: 61 ticks, +1 site).
3. **A single stubbed primitive blocks a whole exit leg:** `script.evaluate` at `bidi/src/protocol.rs:481` is stubbed → the M2 BiDi A/B function cert can't be built → the entire FUNCTION half is unmeasured and ungated on real sites.
4. The doctrine is already right (port subsystems, aperture-before-mechanism, growth-class typing). The failure was **targeting**: applying good method to a non-binding constraint.

---

## 4. Implementation plan — dependency-ordered, step-change

Attack the conjunction in the order that removes the arithmetic cap first. Each step is a **capability/subsystem**, gated and ratcheted, never a decimal.

**P0 — Re-measure the binding constraint (cheap, FIRST; ~1–2 ticks).**
Enumerate the in-scope sites that fail the FUNCTION/scorability gate and **histogram the cause** (uncaught exception halting the DOM? missing boot-critical Web API? navigation/timeout? which JS symbol?). Do NOT trust the stale ~82%/24-site figure — confirm it current. Aperture-before-mechanism, applied to the *exit* metric. (Reuse the fast tri-sweep sub-corpus, `scripts/tri-sweep.sh`, extended to record manuk-scorability + first-failure cause, so this is minutes not a 6h sweep.)

**P1 — Break the scorability ceiling (the step-change; the main line).**
Fix the **shared** function/JS/DOM/event-loop defects that make in-scope sites unscorable. History says these cluster hard (one expression = most of the error). Target **scorability ≥95%**. Each shared fix flips *multiple* sites unscorable→scorable, so this is where super-linear growth lives — and it *raises M1 directly* (more sites can pass render) while tripping the ≥85% trigger that unblocks M2.

**P2 — Unblock and build the M2 FUNCTION cert (the differentiator).**
Implement `script.evaluate` (`bidi/protocol.rs:481`), then stand up the **WebDriver-BiDi A/B function harness** that diffs the site's used capabilities against Chromium (`DAILY-DRIVER-CERTIFICATION.md:110-118`). Bar: top-5 flows pass A/B on ≥85% of sites. This is Tier-6 / Phase-2's ingress and the thing "no incumbent publishes."

**P3 — a11y tree to the bar (the perception surface).**
Adopt the AccessKit bridge (`H0.7` "not started") and drive **role+name+state node-match ≥90%** vs Chrome, measured on the **real-site corpus** (not fixtures — `session-1404-1405`: fixtures hid the missing clause; the a11y-score F1 was ~79% on real sites). This is the highest-value axis for the agent thesis (the 78%→42% finding).

**P4 — Render polish to the 95%-of-nodes bar (Track A, now on scorable sites).**
Now that scorable sites dominate, push **shape≥0.75 on ≥95% of nodes** — the placement/SHAPE weak axis (~69%, `tri-sweep`). Continue the whole-algorithm ports in leverage order: writing-mode/logical geometry → table auto-layout → floats+BFC+clear → abspos/containing-block/stacking (already the Track-A order, now correctly sequenced *after* the cap is lifted).

**Throughout — close the feedback gap.** Run the fast tri-sweep sub-corpus (small, stratified, minutes) **every few ticks** for exit-metric feedback, so the loop never grinds 61 ticks blind again. The 6h full sweep stays the certification instrument.

---

## 5. Loop optimization (what changes in the harness)

1. **New selection rule (installed in `lever-board.sh`):** *each tick, work the term that is arithmetically capping the exit conjunction* — the binding constraint — in the P0→P4 dependency order above. Track rotation (A/B/C alternation) is **retired** as the selection rule; it optimized balance, not leverage.
2. **Exit feedback cadence:** wire the fast tri-sweep sub-corpus into the orient step (a cheap scorability+shape+a11y read) so exit movement is visible within a few ticks.
3. **Growth-class discipline (unchanged but sharpened):** a tick is on-mandate iff it moves the **binding** constraint via a shared-cause subsystem fix — never a decimal, never a non-binding subsystem.
4. **Instruments:** keep WPT as the per-tick climb proxy **only where it targets the binding constraint** (function/JS areas while scorability is the cap; layout areas in P4).

---

## 6. Open decisions for the owner (flagged, not decided)

1. **Bar reconciliation — 80% vs 95%.** `ROADMAP.md:42` says render on **≥80%** of corpus; the locked cert (`DAILY-DRIVER-CERTIFICATION.md`, `PHASE0-MEASUREMENT-SYSTEM.md`) says **≥95% render∧function**. That is a 10–15pt difference in the finish line. Recommendation: treat **≥95%** as the certification, but recognize a **≥85–90% render∧function** milestone as "an agent can daily-drive the real web" — which `ROADMAP.md:37` itself calls the true goal ("a lower, achievable bar than pixel-perfect Chrome"). Pick the number that ends Phase 0.
2. **Confirm the binding constraint is current** (P0) before committing the loop to P1 — the ~82% figure is from t1226.
3. **AccessKit adoption** (`H0.7` not started) — greenlight for P3.
4. **a11y metric:** the cert bar is **node-match ≥90%**; the tri-sweep's F1 (79.4%) is a related instrument, not the cert. Keep node-match as the authority.
