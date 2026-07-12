# Manuk — DECISIONS (append-only ADRs)

_Tradeoffs, constitution amendments, axis changes, north-star-drift rationale. Long-horizon
coherence lives here. Newest last. See [[CONSTITUTION]]._

## ADR-001 — Adopt the autonomous perpetual loop (2026-07-11)

Established `docs/loop/` as the deterministic, resumable operating system for indefinite
self-directed engineering toward a global maximum. Invariants: parity 72/72, build green,
memory-safety + no vendored-engine internal patches, honesty, coherence (commit+push), no local
axis regression. Absorb outstanding beneficial work (tiers, RESEARCH_V2, IMPLEMENTATION
follow-ons) into the LEDGER first, then drive new innovation via UCB explore/exploit.

## ADR-002 — SpiderMonkey is the shell default (2026-07-11)

Now that interactive JS works (persistent per-page context + click dispatch), a plain
`cargo build -p manuk-shell` should produce a browser that runs the real interactive web. The
parity harness (`manuk-wpt`) stays on its own lean features so the gate stays fast. Cost: heavier
default build; accepted — a browser without JS is not the product.

## ADR-003 — Disk hygiene is a loop responsibility (2026-07-11)

The loop prunes stale debug artifacts each tick and does periodic fuller cleans (keeping release
artifacts), never touching `~/.cargo` registry or vendored engine trees. Prevents an indefinite
loop from filling the disk.

## ADR-004 — Amended mission: maximal traversal + ambidextrous spine (2026-07-11)

**Amendment (user-issued).** Two clarifications that sharpen, not replace, the north star.

**1. Maximal traversal, earned by capability — not a checklist, not a disguise.**
The ambition is near-total traversal of the *real* internet: Chromium/Gecko-parity breadth across
the **kinds** of site the web is actually made of — real-time virtualized feeds (X), session-heavy
professional platforms (LinkedIn, Indeed), media-rich client apps (Instagram), complex
authenticated dashboards (cloud consoles, banking, account pages). Named sites are *representative
points in that space*, never a list to clear; the target is the whole space they stand in for.

Manuk earns that access the way any real browser does: **by being one.** It presents its own
genuine TLS handshake and its own genuine engine fingerprint. Chrome has a Chrome fingerprint,
Firefox a Firefox one, Safari a Safari one — **Manuk has a Manuk fingerprint, and it is earned,
not hidden.** This *strengthens* the existing honesty invariant (§1.4): the strategy for coverage
is closing the **capability gap** — full JS/DOM depth, full layout/CSS fidelity, real-time-feed-
grade performance — until *being genuinely Manuk* is sufficient anywhere a real browser is
welcome. **A fifth real browser, not a disguise wearing someone else's face.** Impersonation is
therefore not merely forbidden, it is off-strategy: it would substitute for the very capability
that is the point.

**Loop consequence.** Prioritize by *traversal-blocking capability*, not by site. Ask of each
candidate: "which class of the real web does this unblock?" This raises: JS/DOM depth (the
remaining WebIDL surface), layout/CSS fidelity (now VISUAL-verifiable), virtualized-feed
performance (scroll/recycle/incremental-relayout under a live feed), and session/auth durability
(cookies, storage partitioning, OAuth, long-lived logins).

**2. Ambidextrous spine — one engine; the split is who's driving, not which binary.**
A human drives the headful GUI. An agent drives **either** headless mode (no window; scale and
throughput) **or the same headful GUI, visibly and live**. Both are the identical engine and page
pipeline; they differ only in whether a window is presented and who issues the actions. This is
"shared core, diverge at consumption" made literal: one browser, two ways of holding the wheel.

**Loop consequence.** No agent-only or human-only fork of the page pipeline. The automation
surface (L30) and the shell must drive the *same* `Page`/action path — an agent action in headful
mode must go through the same code a human click does. Divergence is a defect.

## ADR-005 — EPOCH: the long-horizon systemic audit (2026-07-11)

**The flaw this fixes (with evidence).** Over Ticks 1–17 the loop drove capability hard and
quality not at all:

| axis | start | now | Δ |
|---|---|---|---|
| JS | 55 | 77 | **+22** |
| COMPAT | 42 | 57 | **+15** |
| RENDER | 70 | 82 | **+12** |
| PERF | 55 | 58 | +3 |
| MEM | 55 | 55 | **0** |
| STABILITY | 55 | 55 | **0** |

Capability **+49**; quality **+3**. Every tick added a feature and verified it *in isolation* —
build, parity, one test, one screenshot. **Nothing ever verified that the whole engine is fast,
lean, and does not hang.** The loop optimized exactly what it measured. The north star said
"production-grade, blazing fast, stable" and the machinery did nothing to make that true. That is
a hole in the *design*, not in any one tick.

**The rule.** Per-tick gates stay as they are (they are the right granularity for a feature). Add a
**rare, intensive, whole-system gate** — an **EPOCH** — at major-milestone cadence. An epoch is not
a feature tick: it is a dedicated arc that treats the browser as one machine and ruthlessly
optimizes it end to end (see CONSTITUTION §10).

**Trigger (a drift detector, not just a counter).** An epoch is DUE when either:
- **(a) cadence** — ≥ 20 ticks since the last epoch; or
- **(b) drift** — `(ΔRENDER + ΔJS + ΔCOMPAT) − (ΔPERF + ΔMEM + ΔSTABILITY) > 25` since the last
  epoch.

(b) is the important one: it fires precisely when features outrun the machine, which is the failure
mode above. By this rule an epoch is **due right now** (drift = 49 − 3 = 46).

**Why not just do perf work as ordinary ticks?** Because systemic optimization is not decomposable
into feature-sized units. Latency comes from the *interaction* of cascade, layout, paint, event
loop and I/O; an O(n²) only shows at scale; a hang only shows under a real session. A tick-sized
slice would measure the part and miss the machine. It must be a distinct, infrequent, deep arc —
"epochal step jumps," as the mission puts it — not a recurring tax.

**Output is binding.** An epoch publishes numbers and then **converts them into invariant floors**
(CONSTITUTION §1): once a latency/memory budget is measured and set, a later tick that regresses it
FAILS, exactly as a parity regression fails today. That is what stops the drift from re-accruing.

## ADR-006 — The PRODUCT STAR (k extensible points) + a bounded EPOCH (2026-07-11)

**What ADR-005 got wrong.** It framed the systemic gate as *speed and stability*. That is too
narrow. The qualities a feature tick cannot own are not only performance ones — they are every
property that is **emergent in the whole product**: whether the UI is beautiful and intuitive,
whether every button a user can reach actually *does* something, whether the thing is a
self-contained working browser rather than a pile of working features. No tick owns these, so no
tick defends them, so they rot silently. (Exactly as PERF/MEM/STABILITY rotted: +3 while capability
went +49.)

**The PRODUCT STAR.** The north star is not one point but **k points** — the dimensions of a
complete browser *product*. These are distinct from the capability axes in §3 (which measure what
the engine *can do*); the star measures what the product *is*.

| # | point | what it means | probe (how it is observed) |
|---|-------|---------------|-----------------------------|
| 1 | **RESPONSIVENESS** | input→feedback, nav→first-paint, frame pacing; never blocks | per-stage timings + interaction latencies, published |
| 2 | **EFFICIENCY** | CPU / memory / instruction cost; **algorithmic scaling** | timings vs page size — hunt superlinear |
| 3 | **RELIABILITY** | no panics, no hangs, no lost work; graceful degradation | panic/hang audit on input+network paths; soak run |
| 4 | **FIDELITY** | looks and behaves like Chromium/Gecko on the real web | parity gate + `render --chrome` visual diffs |
| 5 | **ERGONOMICS** | a person does the thing *without being taught*; keyboard-complete | task walkthrough against the standard affordance set |
| 6 | **AESTHETICS** | the chrome is coherent and beautiful, not a toy | screenshot the chrome and *look at it* |
| 7 | **COMPLETENESS** | self-contained working product — **no dead buttons, no unwired menu items, no user-reachable stubs** | enumerate every reachable affordance; assert each does something real |
| 8 | **COHERENCE** | one engine; headful/headless/agent share the page pipeline (ADR-004 spine) | grep for divergent/forked paths |
| 9 | **ACCESSIBILITY** | keyboard-only operable; a11y tree correct; contrast | a11y-tree assertions; keyboard traversal |
| 10 | **SECURITY & PRIVACY** | safe defaults, partitioning, no leaks | audit defaults + partition isolation |
| 11 | **IDENTITY & HONESTY** | genuine Manuk fingerprint; truthful reporting (ADR-004) | fingerprint surface review; verification-class honesty |
| 12 | **AGENT-DRIVABILITY** | the automation surface works end-to-end, ambidextrously | drive a real task headless *and* headful |

**The list is dynamic.** k is not fixed. A point may be **added or retired only by ADR**, and — this
is the load-bearing rule — **a point must ship with its probe.** *A point without a probe is a
slogan, not an axis.* If we cannot say how we would observe it, we do not add it.

**The EPOCH audits the whole star** (not just perf), and is **cost-bounded** so diligence never eats
velocity. The bound is the key design move:

> **Measurement is total and cheap. Remediation is bounded and prioritized.**

An epoch **measures every star point** (probes are chosen to be cheap and automatable) but **fixes
only the top violations** — those breaching a floor, worst-first, within the epoch's budget.
Everything else becomes an **ordinary ledger item with the measurement attached as evidence**. So
the epoch's *diligence* is total while its *repair* is bounded: feature velocity continues, and the
epoch cannot become a swamp. **An epoch always terminates.**

Cadence guards (so epochs stay rare):
- **Minimum interval:** no epoch within **12 ticks** of the last (prevents thrash).
- **Budget:** target ≤ **~15% of ticks**. If remediation would exceed budget, the epoch **ships the
  measurements + floors and hands the rest to the LEDGER** rather than overrunning.
- Trigger stays ADR-005's drift detector (≥20 ticks, or capability-minus-quality drift > 25),
  now computed over the **star**, not just three axes.

**Ratchet.** Every measured floor becomes an invariant (§1): a later tick that regresses it FAILS,
like a parity regression. That is what stops any star point from silently rotting again.

## ADR-007 — Comprehensiveness guarantee: severity classes + STAR DEBT (2026-07-11)

**The hole in ADR-006.** "Measure everything, fix only the worst, hand the rest to the LEDGER"
bounds the epoch — but as written, a deferred violation could sit in the backlog forever and the
epoch would be **theatre**. Boundedness must constrain *when* a thing is fixed, **never whether it
is measured or fixed**. Both must hold: comprehensive **and** non-disruptive.

**1. Coverage is never bounded.** Every star point is probed **every epoch, without exception**. No
point is ever skipped for budget. Comprehensiveness lives here, and it is cheap — probes are
designed to be automatable. *The budget may cut repair; it may never cut measurement.*

**2. Severity decides what may be deferred.** Not everything found is equal:

- **CRITICAL — fixed in-epoch, never deferred.** Panics; hangs / UI-thread blocks; **dead
  user-reachable affordances** (a button that does nothing); data loss; a security-default
  violation. These *are* what "self-contained working product" means — deferring one means shipping
  a broken browser. An epoch **cannot close** with an open CRITICAL.
- **MAJOR — a measured floor breach.** May be deferred out of the epoch, but only as **STAR DEBT**
  (below), never as ordinary backlog.
- **MINOR — suboptimal but within floor.** Ordinary LEDGER item, with the measurement attached as
  evidence.

**3. STAR DEBT — deferral with teeth.** A MAJOR deferred from an epoch becomes a `DEBT` item in the
LEDGER. Debt is *not* ordinary backlog:

- **Priority override:** debt outranks new capability work in selection.
- **Mandated paydown rate:** **at least one DEBT item must be retired every 3 ticks** while any
  debt is outstanding. A tick that ignores an available debt item when the rate is unmet **fails**.
- **No silent rollover:** the **next epoch cannot close** while prior STAR DEBT is outstanding —
  it must be paid, or explicitly re-justified in an ADR (a conscious, recorded decision, never
  drift).

**Net effect.** Coverage total; CRITICALs always fixed; MAJORs guaranteed to be fixed within a few
ticks (rate-enforced) rather than "someday"; MINORs tracked with evidence. The epoch stays bounded
in *duration* while the star stays comprehensively defended over *time*. That is how both
properties hold at once: **the epoch is a checkpoint, not the only place quality happens** — the
ratchet (floors that fail a tick) and the debt rate carry the guarantee between epochs.

## ADR-008 — EPOCH-1 closed: cascade rule index, dead-affordance fix, binding floors (2026-07-11)

Full report: [[EPOCH-1]]. 12/12 star points probed (coverage is never bounded, §10.3).

**Headline.** The cascade was **66% of the entire pipeline** on a 19k-node real page and
**superlinear** — per-node cost rose ×11.6 from 1.3k→18.7k nodes — because *every element was
matched against every rule* (O(nodes × rules), no selector index). Fixed with a **rule index**
(bucket each selector by its rightmost compound's key: id → class → tag → universal; an element
tests only rules it could possibly match), plus removal of a per-element allocate-and-sort that was
pure waste (the caller already sorts matched declarations).

Result: cascade **84.56 → 31.40 ms (2.69×)**, whole pipeline **127.97 → 76.44 ms (1.67×)**.
**Parity stayed 72/72** — the index only skips rules that provably cannot match, so computed styles
are byte-identical. This is a complexity fix, not a constant-factor one.

**CRITICAL found and fixed.** The COMPLETENESS probe caught a **dead affordance**: the "Downloads"
menu item only wrote to `tracing` — a user who clicked it saw **nothing**. Per ADR-007 a critical is
never deferred; an epoch cannot close over one. Replaced with a real Downloads panel. This is
exactly the class of bug no feature tick would ever have caught, and it validates adding
COMPLETENESS to the star.

**Ratchet (§1.7/§1.8).** The measured budgets are now **invariants**: F1 cascade ≤ 40 ms, F2
pipeline ≤ 95 ms, F3 mid-page ≤ 10 ms (19k / 1.3k node classes), plus **no dead affordances**. A
tick that regresses one now **fails**, like a parity regression. That is what stops the drift from
re-accruing.

**STAR DEBT (§1.9, ≥1 per 3 ticks).** DEBT-1 the 4 UI-thread `block_on`s (latent hangs); DEBT-2
residual cascade superlinearity; DEBT-3 shell-chrome headless paint — note DEBT-3 is a **probe
gap**: aesthetics/ergonomics are currently *unmeasurable*, and an unprobeable star point is a hole
in the guarantee, so it is debt rather than a feature.

**Cost.** EPOCH-1 was bounded as designed: total measurement, 2 fixes, 3 deferrals. Feature velocity
resumes immediately at Tick 18.

## ADR-009 — Star point 13: DURABILITY & UPDATABILITY (user-issued, long-horizon) (2026-07-11)

**Requirement.** At release maturity (not during rapid iteration), the **browser binary must be
decoupled from user data**. Shipping a new version must never disturb the user's profile:
bookmarks, history, cookies, sessions, passwords, downloads, settings. Updates are seamless; the
profile is durable across them, forward- and backward-compatible within a major line.

**Why it is a star point, not a feature.** It is an *emergent product property* — it cannot be
verified by any single feature tick, and it fails catastrophically and silently (a user upgrades
and loses their logins). It also constrains design *now*: every store we add (cookies.json, session,
bookmarks, history, password store) is a schema that will have to migrate. Deciding that late is
how browsers corrupt profiles.

**Star point 13 — DURABILITY & UPDATABILITY**
*Meaning:* user data outlives the binary. A version upgrade never destroys, locks, or silently
migrates-wrong a profile. The profile is a versioned, self-describing store, separate from the
executable, with explicit forward/backward-compat rules.
*Probe:* (i) every on-disk store carries a **schema version**; (ii) load an *older* profile with a
*newer* binary and assert no data loss; (iii) the binary writes **nothing** user-owned inside its
own install dir; (iv) a corrupt/partial store degrades gracefully (never a panic, never a wipe).

**Now vs later.** Do **not** build an updater during rapid iteration. But **do**, from here on:
version every persisted store as it is created or touched, keep all user data under the profile
dir (`$MANUK_STATE`/XDG), and never write user data next to the binary. The full seamless-update
mechanism is a release-epoch item. Cheap now; unaffordable to retrofit later.

## ADR-010 — STANDING GATES: JS interactivity, Chromium CSS/HTML fidelity, and affordance completeness are checked EVERY tick (2026-07-11)

**The problem.** JS interactivity and CSS/HTML parity-with-Chromium-on-real-sites are the two
biggest traversal-blocking axes (ADR-004) — they are not features to be "done in a tick", they are
**continuous obligations**. Today they are only checked *opportunistically*: the parity gate is 30
synthetic box-probe pages, and JS is verified by whatever scenario the current tick happened to add.
Nothing forces every tick to prove that a **real modern site** still renders like Chromium and that
the **DOM/BOM surface a real site needs** still works. The same is true of UI affordances — Tick 18
had to fix two dead buttons that a *user*, not the loop, discovered.

Anything not gated rots. (Proof: EPOCH-1's drift, and two dead affordances shipped.)

**The rule: these become STANDING GATES — run every tick, like the parity gate.** They are
invariants (§1), not backlog:

- **G1 — Real-site fidelity.** A corpus of **real, snapshotted modern pages** (not just synthetic
  probes) renders within tolerance of headless Chrome, every tick. Box parity *and* the visual
  screenshot pair are produced. Growing this corpus is how "renders any modern website like
  Chromium" becomes a measured claim instead of an aspiration.
- **G2 — JS conformance.** A named, growing suite asserting the DOM/BOM surface real sites actually
  use (events, fetch/XHR, history/location, postMessage, observers, custom elements/shadow DOM,
  timers, matchMedia…). Every tick runs it. Every new JS capability **adds a scenario** — the suite
  only grows.
- **G3 — Affordance completeness.** A machine-checked assertion that **every user-reachable control
  maps to a real action** (menu items, toolbar buttons, shortcuts). §1.8 was a rule a human had to
  remember; now it is a test. Dead affordances become impossible to ship, not merely forbidden.
- **G4 — Visual eyeball, on interval.** Headful/rendered screenshots of the corpus + Chrome
  references, produced for direct inspection (not only numeric box tolerance — a page can pass box
  parity and still look wrong: colours, shadows, fonts).

**Ergonomics parity** (Chromium/Gecko table stakes: standard keybindings, zoom controls, find bar,
bookmark star, back/forward/refresh behaviour) is folded into G3: an affordance that exists but does
not behave the way a user *already expects* is a defect, not a missing feature.

**One command.** All gates run via a single `verify` entry so a tick cannot "forget" one, and so the
epoch's floors, the parity gate, and these gates are one wall rather than four.

## ADR-011 — Gates MUST run the SHIPPING configuration (2026-07-11)

**The bug in the method.** `manuk-wpt` defaulted to `MinimalCascade`. The **shell ships Stylo**. So
the parity gate, the new G1 fidelity gate, and the EPOCH bench were all validating **a cascade no
user has ever seen**. A gate that does not test what ships is not a gate.

Caught by screenshot, exactly as the user predicted ("you will need to screenshot and analyze the
headful browser UI rather than side channels and WPT heuristics that don't match the actual user
experience"). Rendering Wikipedia under each cascade:

- **MinimalCascade** — essentially **unstyled**: the no-CSS fallback source order (a checkbox "Main
  menu", nav links stacked vertically, article content pushed off-screen). The "1990s look".
- **Stylo** — genuinely styled (typography, links, infobox) but with **broken layout**: overlapping
  elements, an unhidden language dropdown sitting on the infobox, a floating Tools panel.

Two completely different bug classes, and the gates were pointed at the wrong one.

**The rule.** Every gate builds the shipping configuration. `stylo` is now **default** for
`manuk-wpt`; a full-fidelity G1 run also enables `spidermonkey` (the modern web is JS-driven, and a
gate without JS measures a page no user loads). Parity is **72/72 under both**, so this cost nothing
and should have been done from the start.

**What it changed immediately.** Fidelity was *understated*: example.com 96.8→**99.2%**, HN
71.2→**78.6%**, Wikipedia 75.5→**81.0%**, mean 81.2→**86.3%** — while simultaneously **hiding** a
near-total Wikipedia layout failure that only the screenshot revealed. Both errors at once: the
numbers were too low *and* too kind.

**Generalization (the user's actual ask).** Real-site CSS/HTML and JS parity cannot be established
by box probes on synthetic pages. It requires: render the real page in the shipping config →
screenshot → compare to Chromium's screenshot of the same URL → **look at the composite**. G1 is
that loop, and it is now a standing gate with a ratcheting floor (start 0.75; raise as it improves).
The numeric score alone is insufficient and is documented as such: Wikipedia scored 75% while being
visibly, structurally broken. **The score gates; the eyeball diagnoses.**

## ADR-012 — The PARITY BENCHMARK SUITE: broad, honest, and lean (2026-07-11)

**The ask.** Prove real parity with Chromium/Gecko across *the internet*, not one reference site —
and cover the **interaction surface** (clicks, scroll, typing, form filling), not just static
rendering. It must be rigorous enough to be believed and cheap enough not to strangle the loop.

**1. The honest metric: COVERAGE.** This session proved a pixel score is a poor proxy — an entirely
absent sidebar moved Wikipedia's visual score by <1 point. So the benchmark reports **two** numbers,
and gates on the second:

- **VISUAL** — coarse block-grid agreement with Chromium's screenshot (blind to font AA, sensitive
  to layout/colour). Diagnostic.
- **COVERAGE (the gate)** — of every element **Chromium actually renders**, what fraction does Manuk
  render *at all*? Probed via `getBoundingClientRect` over every `[id]` in both engines. **A missing
  region cannot hide in this.** Placement drift is reported separately (`misplaced`), because on
  real pages it is dominated by font-metric differences — a fidelity concern, not a correctness one.

*First reading: HN 75.6% coverage (29 of 119 missing); Wikipedia 78.3% (1,402 of 6,461 missing).
Mean **77%**. That is the real distance to parity, and the number to drive to ~99%.*

**2. Breadth: a corpus spanning the traversal classes (ADR-004), not one site.** `docs/bench/corpus.txt`,
grouped by class — reference/content, link aggregator, docs, marketing/landing, app shell,
e-commerce, social feed, media-rich, dashboard. Named sites are *samples of a class*; the class is
what must pass.

**3. Leanness: two tiers, so diligence never strangles velocity.**
- **TICK tier** (every tick, seconds): 3 sites, one per dominant class. Catches regressions.
- **EPOCH tier** (epochs only, minutes): the full corpus. Produces the headline parity number and
  the per-class breakdown that says *which kind of web* we still fail.

**4. Interaction parity (G5 — new).** Rendering parity is half the claim. The other half is that the
browser *behaves* like Chromium under real use: click a link, type into a field, submit a form,
scroll, focus, select. This is scripted through the **in-process automation surface** already built
(Tick 12: durable `Selector`s, `Condition`s, `assert_that`) and mirrored in Chromium, comparing the
resulting page state. An interaction that works in Chromium and not in Manuk is a **CRITICAL**, the
same class as a dead affordance — because to a user it *is* one.

**5. Ratchet.** Mean COVERAGE becomes a binding floor (§1.7-style). It only goes up.

---

## ADR-013 — TWO BARS: functional breadth now, pixel precision later. Never conflate them.

**Status:** accepted (2026-07-11). Amends ADR-012's single metric.

**Context.** COVERAGE reached 99.7% and VISUAL ~90% on a **three-site** sample. Every judgement
about "what to fix next" was being drawn from that sample — which is not a benchmark, it is an
anecdote, and it will cheerfully report that a bug on one of those three sites is the most important
bug on the web. Meanwhile the thing a person actually wants is not a page that matches Chromium
within 8px; it is a page they can *read, click and use*.

**Decision.** Two bars, kept permanently distinct, both reported, never substituted for one another.

**BAR 1 — FUNCTIONAL BREADTH (the near-term target).** A site passes when a person can use it:
scripts run and handlers fire, forms fill and submit, scrolling works, links navigate, and the page
is *legible and correctly shaped* — right box model, right formatting context, right colours and
text. "Close" means it does not look broken to a human looking at it. It does **not** mean it
matches Chromium's geometry within a tolerance.

**BAR 2 — PIXEL PRECISION (the existing gate, unchanged, deferred).** The 8px placement tolerance
and the full box-for-box diff. Still the eventual Tier-1 completion target. Not lowered — *deferred*.

**What this changes.** Do not iterate placement to convergence. Once a fix takes a page from broken
to correctly-shaped, **stop** and move to the next broken site. Breadth of sites reaching Bar 1 beats
depth of one site reaching Bar 2 — that is the actual mechanism that shortens time-to-daily-driver.

**What this does NOT change.**
* **Behavioural correctness is untouched.** An interaction that works in Chromium and not in Manuk
  is still a CRITICAL (ADR-007). This defers visual *precision*, never working functionality.
* Pass 1 (scripting/interactivity) is unaffected — a handler either fires or it does not; there was
  never a pixel tolerance to relax.
* Bar 2 is not abandoned. It is the milestone after Bar 1, at the pace it actually takes.

**Reporting rule.** Every corpus and epoch report states BOTH, as two lines, always:

```
Functional-breadth (Bar 1):  X/Y sites usable
Pixel-precision   (Bar 2):  X/Y sites within tolerance
```

Reporting one as if it were the other is the failure this ADR exists to prevent.

---

## ADR-014 — Work the backlog in three IMPACT passes, and report by pass.

**Status:** accepted (2026-07-11).

**Context.** PARITY-LEDGER.md is ordered by priority, but a flat P0 list still invites picking the
easy item. Ease of implementation is not impact.

**Decision.** Three passes, and the sequencing criterion is *usability impact only*.

* **PASS 1 — breadth-securing.** Unlocks a whole CLASS of pages, not one page: IDL attribute
  reflection, `element.style`/`classList`/`dataset`, event-handler attributes/properties, live layout
  reads, dynamic `<script>`/`<link>` execution. Every server-rendered-button site; every SPA.
* **PASS 2 — usability-securing.** Between "renders and responds" and "someone could use this
  daily": real scrolling, text selection/editing, form submission, backgrounds/gradients,
  `::before`/`::after`, navigation basics. Test: *would a person hit this in their first hour?*
* **PASS 3 — everything else.** Deep spec completeness, edge-case selectors, rare values. Real gaps;
  they block nobody. **A Pass 3 item never jumps the queue for being easy.**

**Corollary — G5 and corpus breadth come before grinding any pass**, because they are what turn
"assumed high-impact" into "measured high-impact".

**Reporting rule.** Progress is reported **by pass**, not by ticks closed. A tick that moves three
sites from broken to Bar-1-usable is a bigger deal than one that moves a single site from Bar 1 to
Bar 2, and must not read the same.

---

## ADR-015 — Reference-source discovery: read the algorithm, never the code.

**Status:** accepted (2026-07-11).

**Context.** Every class bug this loop has found was found *experimentally* — render, stare, diagnose,
fix, re-render. That works, and it is slow. Blink and Gecko already contain the correct algorithm for
every ambiguous, edge-case-heavy thing we keep rediscovering: margin collapsing, line breaking,
float/BFC interaction, event dispatch order, IDL reflection semantics.

**Decision.** For any Pass 1/2 item whose correct behaviour is ambiguous or edge-case-heavy, **read
the corresponding Blink/Gecko source first** and extract the *algorithm and its edge-case list*.
Implement once, against that, rather than iterating against broken renders.

**Hard boundary — same tier as the SpiderMonkey/Stylo modification boundary, and non-negotiable:**
**never copy code verbatim from either reference tree, under any circumstance.** Licence
incompatibility and the from-scratch mandate both make this a line, not a preference. What is
extracted is the *algorithm*, the way one extracts an algorithm from a paper.

**Traceability.** Every implementation informed this way names the reference file/function in its
commit message — both as a citation and as an explicit record that it was pattern extraction and not
transplantation.

**Ceiling, stated so it is not extrapolated past its scope.** This compresses *discovery*. It does
not compress *implementation* — the Rust still has to be written — and it does nothing at all for the
external-integration problems (codec licensing, GPU drivers, DRM), which are not algorithm-discovery
problems and where reading someone else's approach substitutes for none of the work.
