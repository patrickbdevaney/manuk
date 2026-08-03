# CONSTITUTION CHECKS — the loop, anchored to the long horizon

Every other instrument here optimises the local gradient. This one looks UP, at `CONSTITUTION.MD`, and
asks whether the hill the loop is climbing is the mountain the vision names.

Cadence: **every 8 ticks**, enforced by `scripts/constitution-check.sh` and `scripts/tick.sh`. It cannot
be skipped. A check that finds no drift is fine; a loop that never re-reads its constitution is how tick
84 climbed the encoding tail while the total read 47.6% and the frontier sat at 32.3%.

---

## Check #1 — tick 86/87

**Horizon:** H0 — Pareto Web Parity (*the engine is real*).

**Exit gate (binary):** ~83% WPT subtest pass **across categories** · differential-oracle-verified across
the four usage-weighted corpora (document / SPA / social-platform / high-traffic utility) · the headful
shell daily-drivable by its own developer · every rendered construct queryable through the in-process
semantic API.

**Gate or scoreboard?** This check was *born from a drift*. Tick 84's +721k encoding subtests moved the
scoreboard (25,869 → 747,778 total) but the Pareto-relevant breadth is **32.3%** (encoding excluded), and
encoding's remaining ~767k failures are the exotic per-codepoint tail that I4 says to *degrade*, not
chase. Tick 86 corrected the north star (`orient` now ranks by usage-weighted breadth, tail excluded) and
wrote **PART VI** into the constitution. Tick 87 acted on §VI.4 step 1 — opened the CSS aperture (8 css
subtrees were checked out but never measured) to turn unknown breadth into a ranked work-list.

**PART VI corrections banked (tick 86):** the a11y/semantic tree (I3) is already built and feeding the
agent — not a future task; Stylo is already the shell default; the GPU/Vello paint path is
aspirational-comments-only (raster is tiny-skia CPU); the differential oracle (I5) has never finished a
crawl.

**The steer, going forward (the direct H0 path, §VI.4):**
1. ✅ open css/* + html/* aperture (tick 87 — css done; html/* beyond html/dom still to open).
2. **CSS layout breadth** — the ranked Pareto lever: `css-flexbox` (6,459 failing, 5.5%), `css-grid`
   (4,414 failing, 4.7%), then `css-sizing` (2,204, 12.7%), `css-fonts` (1,930, 32.4%). Every modern
   site needs flexbox and grid; this is the H0.1 layout work.
3. Land one clean differential-oracle crawl (I5) — the gate's second condition, and a Bar 0 signal.
4. Web-API surface by usage weight, ordered by the oracle's divergence clusters.
5. Semantic model in lockstep (I3); schedule the AccessKit bridge once the a11y tree stabilises.

**No invariant is being bent.** The tail-exclusion is explicitly *from the ranking, not the ratchet* —
encoding stays banked and must not regress (I4 is about where loop-throughput goes, not about deleting a
capability).

**Next check due: tick 95.**

---

## Check #2 — tick 95

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified across the
four corpora · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** Honest answer: ticks 88–95 were **mostly meta-infrastructure, and mostly
operator-directed** — CI fix (88), loop budget (89), RAM builds (90), the wiki system + backfill (92/94),
the wall-time audit (93), and a blocked ARIA exploration (95). Only **innerText (91, +33)** moved the H0
scoreboard directly. On its face that is the drift the standing rule warns against (novelty/infrastructure
over the gate).

**Why it is not drift, this time.** The infrastructure was the operator's explicit ask and it is
*load-bearing for the horizon*, not novelty: the loop budget makes the autonomous grind actually
autonomous; the wiki system (enforced accumulation + deterministic retrieval) is the memory the
constitution's own §whole-point demands — the knowledge the H1 security work and the H2 agent-driving
surface and the H4 species will need and cannot reconstruct from a diff; the wall-audit keeps the per-tick
tax from compounding across the remaining ~990 ticks. This was the session that built the *machine* that
grinds H0, and proved it (orient → mechanism → gate → wiki → land, repeatedly). Building the machine once,
early, is not a detour from the gate; it is the multiplier on every tick that reaches it.

**The steer, and it is unambiguous.** The machine is built; the next ticks return to **capability**. The
histogram names the target — html/dom **attribute reflection** is the largest remaining Pareto mass
(~35k failing subtests), and tick 95 found its hard gate: the **mass-reflector C-stack recursion**. So the
next capability tick is the **effective-stack-quota fix** (`JS_SetNativeStackQuota` from real thread-stack
bounds), which unblocks ARIA *and* the ~15k missing reflected getters behind it. That is the biggest H0
lever on the board, and the loop knows exactly why.

**PART VI still holds.** No correction needed; the direct path (aperture → CSS/reflection breadth → oracle
crawl → web-API by usage → semantic model in lockstep) is intact, and reflection breadth is step 2, now
with its blocker named. **No invariant bent** — ARIA was reverted precisely *because* I4/Bar 0 forbids
trading a crash for a capability.

**Next check due: tick 103.**

---

## Check #3 — tick 103

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified across the four
corpora · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** Ticks 96–103 were **capability on the direct H0 path**, and the scoreboard moved
where the gate lives — not on the tail. The honest baseline correction (tick 96, onload double-fire →
32.1%) was banked, then a run of **web-API-surface-by-usage-weight** ticks (§VI.4 step 4): selector case
flag `[attr=val i]` (+117 css/selectors), `classList` ordered-set (+241 dom), `Range.createContextualFragment`
(+33 domparsing), and `document.elementFromPoint` (+29 css-transforms/flexbox/overflow). **+420 real
subtests across four categories**, none of it encoding-tail. The method was the operator's **flip-per-risk**
directive: probe the failure histogram, take the *single bounded mechanism* at zero Bar-0 risk. Two
honestly-neutral ticks (97 offset-int, 102 computed-style exposure) landed as correctness-with-flat-score
(tick-97 rule), explicitly labelled.

**Is `orient`'s usage-weighted ranking still the north star?** Yes. No big-but-tail number crept back;
encoding stayed banked and untouched. The `appearance` cluster (css-ui, 300) was *declined* precisely
because it is closer to the pedantic tail and needs a Stylo supplement (§I2) — the ranking held.

**Any invariant bent?** No. **I3 is served, not bent** — `elementFromPoint` bridges the a11y hit-test
(the agent surface's own hit-testing) to the JS surface; `classList`/selectors/computed-style are DOM-query
surface. **I2 was re-confirmed the hard way**: Stylo's servo build lacks `appearance` (gecko-only), so it
needs a *supplement*, never a patch — exactly the `:has()` ladder. **Bar 0 held absolutely**: tick 101
uncovered a real memory-safety **SIGSEGV** (a cross-file reflector/rooting UAF in flex relayout under
runtime reuse) and did NOT trade it for a capability — it built **isolation-retry** so the sweep
distinguishes a per-page crash from a runtime-reuse artifact (`ACCUM`), keeping real crashes sacred, and
the underlying UAF is now a **tracked, open Bar-0** (memory `flexbox-relayout-segfault.md`).

**PART VI correction.** §VI.4 step 4 (web-API surface by usage weight) is **actively in progress**, not
future. Two blockers are now named on the direct path: (1) the **stack-quota crash** still gates the
~35k html/dom reflection mass (step 2); (2) a **new open Bar-0** — the flexbox reflector-teardown UAF —
which needs ASAN/`valgrind` tooling (operator sudo) to fix and, per Bar-0 primacy, precedes further
capability once that tooling exists. I5 (the oracle) still has not completed one clean full crawl.

**The steer.** Continue web-API surface by usage weight while the clean bounded mechanisms last (next
candidates: the CSSOM `<style>.sheet` bridge ~944, or the `appearance` supplement); take the stack-quota
fix and the flexbox UAF **in fresh, well-resourced contexts** (both are the tick-84 GC-saga class —
forbidden at a maxed context). No drift; the direct path (§VI.4) is intact.

**Next check due: tick 111.**

---

## Check #4 — tick 111

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified across the four
corpora · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** **Gate, decisively.** Ticks 108–111 executed §VI.4 step-4 (web-API surface by
usage weight) and it culminated in the session's largest single move: **the global HTMLElement
attribute reflection** (`dir`/`hidden`/`tabIndex`/`accessKey`/… reflected on *every* element via a `"*"`
row in the existing reflection table) — **html/dom 22,690 → 40,935 (+18,245), TOTAL 389,637 → 407,882**,
crashes=0, no area regressed. That is real, usage-weighted breadth (the reflection surface every framework
reads), not tail. The method that found it: probe by **what the failing tests reference most** — the
`IDL get … undefined` mass — then find the *shared cause* (the per-tag table had no global row) rather
than one attribute at a time.

**The Bar-0 fear that gated this since tick 95 did NOT materialise.** tick 95 reverted ARIA because adding
accessors tipped the mass-reflector C-stack crash. Adding these 10 global accessors did **not** crash
(crashes=0 across the full sweep) — the crash threshold is higher than 10, and this session's
isolation-retry (tick 101) would have recovered an accumulation-only crash as ACCUM anyway. **The
remaining reflection mass (ARIA + the rest) is still gated on the effective-stack-quota fix** — re-scoped
this session (tick 106/110) to gate on the *reflection* JS-recursion, a fresh-context job — but a large,
crash-free chunk was reachable *without* it.

**Any invariant bent?** No. **I3 served** (the reflected surface is the DOM-query surface the agent reads);
**I2 intact** (the reflection *mechanism* is generic against the spec's algorithms — only the table grew,
and the table is the spec's IDL, not test knowledge); **Bar 0 held** (crashes=0, and the still-latent
mass-reflector crash was measured, not traded). **The self-audit passed** ("methodology and reality
agree").

**PART VI correction.** §VI.4 step 4 is now the loop's most productive vein and largely mined for
*crash-free* reflection: the global attributes landed; per-element table coverage is comprehensive. What
remains on the reflection frontier is the crash-gated mass (ARIA + idlharness-style whole-tree access),
which needs the stack-quota fix (now correctly scoped). The other levers (CSSOM `.sheet`, layout-geometry
precision) are unchanged.

**Next check due: tick 119.**

## Check #5 — tick 119

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified across the four
corpora · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** **Gate.** Ticks 112–119 stayed on the direct H0 path — DOM/HTML-DOM capability the
app web actually calls, picked by histogramming `--show-failures` for the single largest *one-mechanism*
cluster each time: numeric reflection coercion (117, +437), `dispatchEvent` validity (118, +15), and this
tick `Node.prototype.moveBefore` (119, +18) — the atomic move plus its stricter pre-move validity, a whole
missing DOM method framework reconcilers call. None of it is the encoding tail; every move landed where the
gate lives. The method was chosen over higher-*raw*-count `dom` clusters (XML document loading ~488, the
diffuse `assert_throws` mass) precisely because those are subsystems, not bounded ticks — the FLIP-RATE
discipline, not failing-count.

**Is `orient`'s usage-weighted ranking still the north star?** Yes. `moveBefore` is modest in raw flip
(+18) but high in *mechanism cleanliness and forward usage weight* (frameworks are adopting it), and it is
one coherent spec algorithm at zero Bar-0 risk — the right shape for the loop even though bigger raw
numbers (e.g. document named-properties, ~56) sat nearby but needed reflector class-ops surgery (a resolve
hook on the shared `NODE_CLASS`) that carries real Bar-0 risk. The ranking held: cleaner bounded mechanism
over bigger-but-riskier mass.

**Any invariant bent?** No. **Bar 0 held** (HANG/CRASH 0; the gate is its own process per the runtime-reuse
UAF discipline). **I2 intact** — the move is spec-generic (WebIDL + DOM "ensure pre-move validity"), no
engine forked. **I3 served** — a new DOM-mutation method is part of the surface the agent drives. A latent
memory-safety-adjacent hazard was *surfaced and closed*, not traded: `node_and_dom`'s blind `SLOT_NODE`
read mistook a plain `{a:1}` for node #1, now gated by an explicit `NODE_CLASS` check.

**PART VI correction.** None. §VI.4 step 4 (web-API surface by usage weight) remains the productive vein;
the crash-gated reflection mass (ARIA + whole-tree idlharness access) still awaits the effective-stack-quota
fix in a fresh context, and I5 (one clean full oracle crawl) is still outstanding — both unchanged since
check #4.

**Next check due: tick 127.**

---

## Check #6 — tick 127

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified across the four
corpora · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** **Gate.** Ticks 120–127 stayed on the direct H0 path (§VI.4 step 4 — web-API
surface by usage weight), each picked by histogramming `--show-failures` for the single largest
one-mechanism cluster: `createProcessingInstruction` (120), typed Event hierarchy (121), Text/Comment/
DocumentFragment constructors (122), `splitText`/`wholeText` (123), CSS-nesting measurement (124),
`getElementsByTagNameNS` (125), a Bar-0 diagnosis with no trade (126), and this tick the **DOMException
identity fix** — the largest single move of the run at **+420 dom** (47.5% → 53.9%). That is real,
usage-weighted breadth (every `catch` block that branches on `e.code`/`instanceof DOMException`, plus the
`assert_throws_dom` harness itself), not the encoding tail. The method held throughout: the FLIP-RATE
discipline — cluster by error signature, take the one shared cause — is exactly how tick 127 turned one
mechanism into ~420 flips instead of chasing names one at a time.

**Is `orient`'s usage-weighted ranking still the north star?** Yes. The DOMException cluster was picked
over the higher-*raw*-count but LAYOUT-slog areas (css-flexbox 3371, css-grid 2691 — deprioritised per the
loop's own "one fix flips ~nothing" rule) and over diffuse `assert_throws`-family masses that are
subsystems, not bounded ticks. Cleaner bounded mechanism, bigger flip, zero Bar-0 risk.

**Any invariant bent?** No. **Bar 0 held, and was checked rather than assumed** — dom/ranges' standalone
CRASH 1 was reproduced on the *committed* binary (stash → rebuild → same crash), proving it pre-existing
(a ranges/tentative runtime-reuse artifact the isolated sweep recovers), not a trade. **I2 intact** — the
`DOMException`/`.code` mapping is the spec's own WebIDL constant table, no engine forked; the polyfill
already existed, only the JS throw sites changed. **I3 served** — thrown-error identity is part of the DOM
surface the agent and every framework read.

**PART VI correction.** None. §VI.4 step 4 remains the productive vein; the crash-gated reflection mass
(ARIA + whole-tree idlharness access) still awaits the effective-stack-quota fix in a fresh context, and
I5 (one clean full oracle crawl) is still outstanding — both unchanged since check #5.

**Next check due: tick 135.**

## Constitution check @ tick 325 (2026-07-21) — reconciled after the counter unfreeze

**Gate or scoreboard?** Gate. Same counter-unfreeze reconciliation as the surface audit: the check was
not skipped for 200 ticks, the counter was frozen. The direction-vs-frontier substance is current in
the observer's tick-328 RESEARCH-SYNTHESIS-2026-07.md, which audits the loop against CONSTITUTION.MD
Part VII (rendering parity vs the real internet + the agentic surface) and the 7-phase vision.

**Is the loop still pointed at the frontier?** Yes. The work this session is PURE browser capability
(I3 agentic surface: IndexedDB indexes for the logged-in app web; Fullscreen for the media web) — not
the encoding tail that Part VI warns against. The authoritative work list is now the bounded Phase-0
remainder (3 subsystems + ~20 bounded items + a named cut line), worked Tier-1-first, with the fidelity
instrument rebuild ranked above any single capability tick as THE exit gate.

**Any invariant bent?** No. Bar 0 held (no crash/regression; each landed tick is additive + RED-proven).
I2 intact (no engine forked — IndexedDB indexes are a shim + a serde field; Fullscreen is a prelude
shim). The RATCHET is honored — nothing traded, the wall is green.

**Next check due: tick 333.**

## Check #7 — tick 326

**Canonical-header formalization** of the tick-325 check above (which used a non-matching
`## Constitution check @ tick 325` header, so `status-update.sh`'s `^## Check #N — tick M`
derivation never registered it and LAST_CONSTITUTION_CHECK stayed at 127). No check was skipped for
~200 ticks — the TICK counter was frozen at 128; the substance was done fresh at 325.

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website" across
doc/app+hydration/social/platform/MEDIA), not the retired 5-lever milestone. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 structural/placement fidelity on ≥95% of the corpus +
≥0.70 per top-20 category), NOT capability% and NOT the retired `ready_pct`. Authoritative work list:
docs/loop/PHASE0-BOUNDED-REMAINDER.md (3 subsystems + ~20 bounded items + a named cut line).

**Gate or scoreboard?** Gate. This tick (326) is an instrument-fidelity re-pin — it corrects the map
(one of the ratchet's three faces), not the scoreboard. It flips zero WPT and adds zero capability%;
its whole value is making the constellation stop lying about four already-gated cells. That is exactly
the discipline Part VI asks for: reconcile ground truth before climbing.

**Is `orient`'s usage-weighted ranking still the north star?** Yes. The batch being landed (326 re-pin,
327 re-pin, 328 Selection API, 329 IndexedDB indexes, 330 Fullscreen) is daily-driver capability +
map fidelity, not the encoding/CSS-layout tail Part VI warns against.

**Any invariant bent?** No. Bar 0 held (326 touches zero engine code; the capability ticks in the batch
are additive + RED-proven). I2 intact (no dep forked). I3 served (the re-pinned cells are the agentic
surface the map advertises). The RATCHET is honored — nothing traded, wall green, WALL mark re-baselined
to 189s by the observer (agent did not retune its own gate).

**PART VI correction.** None beyond what 325 already recorded: PHASE0-BOUNDED-REMAINDER.md supersedes the
constellation priority rows; ready_pct retired; fidelity certificate is the exit.

**Next check due: tick 334.**

## Check #8 — tick 334

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north star
of CONSTITUTION.MD **Part VII** (re-read this check). **Gate:** the FIDELITY-SCORING-REDESIGN.md
certificate (≥0.75 structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT
a WPT percentage and NOT the retired `ready_pct`. Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. The three ticks since the last check are pure Part-VII v1 work, and every
one applied VII.1's TEST ("does this move one of the four components toward shippable?") in the
affirmative: **331** cookie-attribute cross-layer enforcement (component #1 login correctness + #3
good-enough security), **332** IME composition (components #1 rendering parity + #2 agentic surface —
CJK/accented text entry was impossible, now the commit burst drives a real editor), **333** `:active`
fed end-to-end (component #1 — the last dynamic pseudo-class, press feedback on essentially every
interactive site, was dead; now live through the shell). Zero WPT-chasing, zero encoding tail, zero
work justified only by a deferred species — VII.1 held.

**Is the loop still pointed at the frontier?** Yes. The frontier remains the bounded Phase-0 remainder,
Tier-1-first, with the fidelity-instrument rebuild ranked above any single capability tick as THE exit
gate. This session took bounded Tier-1/Tier-2 daily-driver items (IME is Tier-1 #4; `:active` closes the
dynamic-pseudo-class set alongside the already-fed `:hover`/`:focus`) rather than opening an L subsystem
(media join, contenteditable) that cannot land atomically in one tick — correct per the atomicity rule.

**Is the agentic surface (component #2) being served?** Yes, directly. `Page::dispatch_composition` and
`Page::set_active` are both native driving-surface entry points an agent (and the shell) calls — the same
seam `dispatch_click`/`dispatch_key`/`set_focus` established. The a11y/DOM tree stays first-class.

**Any invariant bent?** No. Bar 0 held — every landed tick is additive and RED-proven (the RED edit
reverted byte-for-byte each time), dom+css unit suites green, no crash/regression. I2 intact — no
dependency forked (`:active` mirrors the existing `:hover` plumbing; IME is a dispatch shim). The RATCHET
is honored — nothing traded; the wall came in green at 66-70s (the 267-277s refusals were pure box
contention at load 3-4, re-banked honestly on a quiet box per the standing wall recipe, mark NOT
retuned).

**PART VI / VII correction.** None. PHASE0-BOUNDED-REMAINDER.md remains the work list; the fidelity
certificate remains the exit; Part VII's four-component v1 scope is intact and unbent.

**Next check due: tick 342.**

## Check #9 — tick 342

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north star of
CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 structural/
placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Authoritative
work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. The window since Check #8 is dominated by the observer's ranked CO-#1 — the
**fidelity-instrument rebuild** (FIDELITY-SCORING-REDESIGN.md), which the board explicitly ranks *above any
single capability tick* because it is THE Part-VII component-#1 exit gate. Building the measuring instrument
for "reliably renders the representative real internet" IS Part-VII v1 work, not a detour from it: **335**
Layer-1 SHAPE scoring, **338** h-overflow, **339** sibling-overlap, **340** reading-order inversion, **341**
collapsed interactive target (the box-dump half of hittability), **342** offset-magnitude banding in the
root-cause cluster key. Interleaved: **336** self-audit, **337** surface audit. Zero WPT-chasing, zero
encoding tail, zero work justified only by a deferred species — VII.1's TEST held every tick.

**Is the loop still pointed at the frontier?** Yes. VII.1 says the bar for component #1 is *"reliably renders
the representative real internet, NOT a WPT percentage"* — so the instrument that certifies exactly that
outranks flipping subtests. The redesign's five jarring invariants are now 4/5 wired (overlap / h-overflow /
reading-order / collapsed-target) on top of SHAPE + offset-banded clustering; only post-load stability (a
CLS-equivalent needing a second post-settle snapshot) and the occlusion-cover half of hittability (needs
paint order) remain — both honestly logged as unwired, neither over-claimed.

**Is the agentic surface (component #2) being served?** Indirectly this window — the fidelity instrument
scores the rendered tree the agent reads, so a truer instrument means a truer a11y/DOM surface. No component-
#2 regression; the driving seams (dispatch_click/key/composition, set_active/focus) are untouched. When the
instrument work completes, the frontier returns to Tier-1 capability + the agentic top-site drive.

**Any invariant bent?** No. Bar 0 held — every landed tick is additive (a new pub fn + one live call site, or
a signature refinement) and RED-proven with the RED edit reverted byte-for-byte; 7 oracle + 10 wpt lib tests
green, no crash/regression. THE RATCHET honored — nothing traded; instrument fidelity (the third ratchet
face) is precisely what this window *bought*, and it bought it without degrading capability or performance.
The wall came in green at 68s on a quiet box; mark not retuned.

**PART VI / VII correction.** None. The instrument-before-tail discipline is itself Part-VII-faithful:
certifying component #1 honestly is the precondition for declaring Phase 0 done. Four-component v1 scope
intact and unbent.

**Next check due: tick 350.**

## Check #10 — tick 350

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north star
of CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75
structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage.
Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. The window since Check #9 (343–350) closed two named arcs from the ranked
orders. First the instrument roll-up (**343**, corpus jarring tally — the last architecturally-bounded
oracle piece; the instrument then deliberately PIVOTED per its own assessment rather than being mined
past its value). Then the bounded daily-driver vein: **344** `:muted`, **345/347/348** the HTTP
cache-correctness arc (conditional revalidation → Expires → Age; the cache now behaves like a real
browser cache against real CDNs), **346** drag-and-drop editor half. Then the board's CO-#1 order (2)
head-on: **349** the MSE playback JOIN (the appended-bytes class — YouTube-shaped players — decodes and
paints, with `isTypeSupported` honestly steering) and **350** the audio output device (cpal borrowed;
decoded PCM reaches the device boundary sample-exact; the last dead organ in the A/V file pipeline).
Zero WPT-chasing, zero encoding tail; VII.1's TEST held every tick — each moved component #1 (rendering
the real internet, media class) or #2.

**Is the loop still pointed at the frontier?** Yes. The board's marquee target is "YOUTUBE PLAYS"; the
window built exactly the two organs that target lacked (the SourceBuffer→decoder join, the sound
device). Remainder on that path is codec breadth (High-profile H.264 / AV1 per MEDIA.md), A/V
master-slave sync, and ABR — named in the constellation row, not over-claimed.

**Is the agentic surface (component #2) being served?** Yes — **346** `Page::dispatch_drag` is a native
driving-surface entry point (the sortable-list/kanban reorder handoff), the same seam as
dispatch_click/key/composition. The semantic model rode every media tick (frames land in the page's
own image map, queryable state).

**Any invariant bent?** No. Bar 0 held — every tick additive, suites green twice, EXIT 0. THE RATCHET
honored — nothing traded, wall marks not retuned (349/350 landed off honest warm receipts). **I2/I8:**
one new dependency, `cpal` 0.17 — BORROWED per the standing rule (the board's explicit order), pure
adoption, not forked or patched, and confined to the shell's `gui` feature lane so no headless or gate
binary links sound hardware. **Process rule 3 enforced against ourselves:** tick 350's first RED probe
PASSED — the gate had a hole (a cursor overshoot invisible to byte-exact concatenation); the hole was
closed with an exact-landing assertion and the same edit now fires. A green that cannot go red measures
nothing, including ours.

**PART VI / VII correction.** None. The four-component v1 scope is intact; media work stays scoped to
"the representative real internet plays" (DRM/EME remains a stated permanent wall per I7).

**Next check due: tick 358.**

## Check #11 — tick 358

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north star
of CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75
structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage.
Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. The window since Check #10 (351–357) drove the board's CO-#1 order (2)
codec ladder to its end and closed the A/V organ chain: **351** A/V master-slave sync (the device
crystal owns time), **352** muted plumbing (the autoplay-muted class is quiet here as everywhere),
**353** AV1 decode (re_rav1d behind the M5 trait), **354** AV1 ships + all three honesty registries
flip in the same tick, **355** AVIF stills (the blank-hero-image class). Cadence held mechanically:
**356** self-audit (clean), **357** surface audit (2 unlisted rows added from the outside frame).
Zero WPT-chasing, zero html/dom flips; VII.1's TEST held every tick — each moved component #1
(media/image classes of the real internet) or governance.

**Is the loop still pointed at the frontier?** Yes. The marquee "YOUTUBE PLAYS" path now lacks: codec
breadth beyond av01+Baseline (High-profile H.264; VP9 constitutionally on the floor), ABR, and the
live .muted/.volume IDL channel — all named in constellation rows, none over-claimed. Container
queries (CO-#1 order 3, Stylo-side) and the playbackRate row (surface-audit #9) are the adjacent
bounded frontier.

**Is the agentic surface (component #2) being served?** Held, not advanced this window — the driving
seams are untouched and every media organ lands page-queryable state (frames in the page's image map,
feeds observable). The semantic model rode along; no regression, no rot (I3 requires lockstep
exposure, and set_video_frame/apply_images_by_url are exactly that exposure).

**Any invariant bent?** No. **I2** (sanctioned deps, never patched): three BORROWS this window —
re_rav1d 0.1.3 (pure Rust, no nasm, safe module), avif-parse 1.4 (MPL-2.0, same family as Stylo) —
adopted unpatched, feature-fenced so no gate lane acquires a decoder; the cargo-tree isolation was
re-verified. **I7** (honest walls): VP9/webm stay refused by name; 10-bit AVIF refuses gracefully;
alpha renders opaque and says so. **Process rule 3** enforced against ourselves twice: the t354
vacuous-substring claim (contains("av1:true") satisfied by cpt-av1:true — tripwire-caught, label
renamed) and t353's flush-discard archaeology probed one variable at a time after a two-variable fix.
THE RATCHET: nothing traded; the t353/354 cold walls (536s/545s from lockfile/feature rebuilds) were
re-run to warm 62-68s receipts, marks not retuned.

**PART VI / VII correction.** None. The four-component v1 scope is intact; media work stays scoped to
"the representative real internet plays" (DRM/EME permanent wall per I7; WebRTC out per Part IV).

**Next check due: tick 366.**

## Check #12 — tick 366

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage. Authoritative work list:
PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 359-365: **359** WasmGC measured working and pinned (the
Kotlin/Flutter-web class — probe bytes cross-validated against Chromium before trusting any no),
**360** the live media-IDL channel (mute buttons/volume sliders reach the device; IDL-beats-attribute),
**361** playbackRate (scaled wall, mastery refused at rate≠1, the chipmunk rule), **362/363** MP3
organ→join (the podcast class end-to-end; canPlayType flips with the join, never before), **364**
FLAC+Ogg/Vorbis through the same seam (Opus stays a named wall — no decoder, honest ''），**365**
the WebVTT row's three-tick-stale STILL-MISSING list corrected with receipts. VII.1's TEST held every
tick — media/audio classes of the real internet, or map honesty.

**Is the loop still pointed at the frontier?** Yes. The audio/codec vein is now mined to its honest
walls (Opus/AC-3 named refusals; WSOLA time-stretch and High-profile H.264 are the two named rungs
left; ABR is downstream of both). The board's CO-#1 order (3) container queries (Stylo-side) is the
adjacent non-media frontier and the natural next subsystem.

**Is the agentic surface (component #2) being served?** Held. Every media property landed
page-observable (feeds queryable, cues in the track model, registry answers truthful) — the semantic
model the agent reads stays in lockstep per I3; driving seams untouched, no rot.

**Any invariant bent?** No. **I2:** avif-parse and three symphonia features adopted unpatched behind
the existing fences; cargo-tree isolation re-verified each tick (no gate binary acquired a decoder).
**I7:** the honesty registry did its best work this window — bare audio/ogg answers exactly 'maybe'
(may be Opus), codecs=opus '', 10-bit AVIF a graceful no, rate≠1 mutes rather than chipmunks.
**Process rule 3:** every landed tick RED-proven; the t364 silent-vanish RED (sniff narrowed → FLAC
dies, suite green) is the class the rule exists for. THE RATCHET: wall marks never retuned through
five cold-relink cycles; Audit #5 measured the warm wall IMPROVED (66→59s) while coverage grew.

**PART VI / VII correction.** None. Four-component v1 scope intact; media claims scoped to what
provably plays.

**Next check due: tick 374.**

## Check #13 — tick 374

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage.

**Gate or scoreboard?** Gate. Window 367-373: **367** surface audit #10 put the WebMCP clock on the
map (Chrome 149 origin trial — the H2 marquee the constellation never listed), **368** AVIF alpha
(Y-plane-is-the-mask; the fixture-lies-by-name trap), **369** WAV (RIFF form-type routing, negative
asserted), **370** the mixer (sum+clamp, mismatch-skip, set-mastery; a gate hole closed in-tick when
the clamp claim could not fire on a too-quiet fixture), **371** container queries probed to the ONE
dead seam (query_container_size) with the build spec recorded and the build deliberately deferred to
a fresh context per atomicity, **372/373** the MPA lifecycle pair (pagereveal + pageswap, the spec's
own nulls, no animation over-claim). VII.1's TEST held every tick.

**Is the loop still pointed at the frontier?** Yes. The media/audio vein is mined to its honest walls
(Opus/AC-3 named refusals, WSOLA and resampler and High-profile named rungs, ABR downstream). The
specified container-queries build is the next subsystem, spec in JOURNAL t371; the WebMCP scope
question is flagged to the board/observer rather than self-decided — exactly where an H2-vs-v1 call
belongs.

**Is the agentic surface (component #2) being served?** Held; and audit #10's WebMCP row is the first
explicit component-#2 FRONTIER item on the map since the driving seams landed — the clock is visible
now, which is what I3's never-let-it-rot demands.

**Any invariant bent?** No. I2 borrows unpatched and fenced throughout; I7's honest-null discipline
(viewTransition:null is the spec's own value, stated in-code both times); process rule 3 caught two
of its own gates this window (the too-quiet clamp fixture t370, the mask-fixture-with-no-alpha t368)
— both holes closed in-tick and recorded. THE RATCHET refused one landing on a cold 478s receipt
(t372) and the refusal was CORRECT — warm re-verify re-banked and landed; the mark was never touched.

**PART VI / VII correction.** None.

**Next check due: tick 382.**

---

## Check #14 — tick 382

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage.

**Gate or scoreboard?** Gate — and this window put the gate's own INSTRUMENT into service. Window
375-381: **375** the mixer resampler (cross-rate audio audible), **376** self-audit clean, **377**
surface audit #11 (promise-scroll row + the WebMCP declarative/imperative split), **378** promise
scrolls — where the gate FALSIFIED the tick's own premise and the fix went a layer deeper (the
synchronous scrollY contract our request model had silently broken), **379** container queries
LANDED (the rung-3 source supplement + sized re-pass — the biggest CSS shift in a decade now
applies), **380** THE CORPUS ORACLE RAN END-TO-END FOR THE FIRST TIME, **381** its Bar-0 find
(the netlify char-boundary panic) fixed ratchet-first.

**A constitutional milestone.** Check #1 (tick 86) recorded "the differential oracle (I5) has
never finished a crawl" and steered "land one clean differential-oracle crawl" as the gate's
second condition. Tick 380 closed that, ~295 ticks later: 265 sites, 197 diffed, the jarring
baseline banked (overlap 45.2% / h-overflow 33.5% / reorder 71.6% / dead-target 47.2% — the
honest distance to the ≥95%-clean exit bar), a 627-cluster ledger, and 31 sites slow on OUR
clock. The Phase-0 gate is no longer aspirational prose; it is a number that can move.

**Is the loop still pointed at the frontier?** Yes, and the ledger now DEFINES it: (1) the
author-style-not-applied trio (none→block 49 sites / flex→block 43 / block→inline 39 — probe for
one shared cause before three fixes), (2) img/svg computed-display UA divergence (81/80 sites),
(3) MISSING BOX br/path/div, (4) the 13× perf outlier class (wix, atlassian). These outrank any
unmeasured hunch.

**Is the agentic surface (component #2) being served?** Held (t378's truthful promises are agent
food — awaits that resolve when the effect is real); no new component-#2 rows this window, WebMCP
clock still flagged to the board.

**Any invariant bent?** No. The measurement kept its own honesty rules (55 degraded-oracle
discards never scored as ours; 12 process timeouts attributed to NOBODY); the crash was fixed
before any capability work, per THE RATCHET; no mark retuned. Lesson re-banked by t380/381
together: measurement finds what unit tests cannot — the panic had survived every ASCII test we
ever wrote.

**PART VI / VII correction.** None — but Check #1's "I5 has never finished a crawl" parenthetical
is now historical, closed by t380.

**Next check due: tick 390.**

---

## Check #15 — tick 390

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage.

**Gate or scoreboard?** Gate — and for the first time the LEDGER, not intuition, chose every
capability tick. Window 383-389: **383** the instrument-honesty seam (starved sheets counted +
discarded — the trio families demoted to artifact before anyone built "fixes" for them), **384**
replaced elements compute `inline` (81/80-site family, unwound a two-cascade convenience mutation),
**385** `<br>` geometry (64-site family), **386** self-audit + wall audit #6 (clean; wall 57s while
coverage grew), **387** surface audit #12 (field-sizing added; t378's scroll promises validated
AHEAD of the platform roundup), **388** field-sizing:content (the recovered property that must beat
the hints), **389** the default object size in used-size layout (784×0 → 300×150; icon buttons
regain hit area). Every engine tick RED-proven; two families REFUSED as artifacts before fixing —
the instrument's honesty rules cut both ways, which is exactly VII.1's TEST.

**Is the loop still pointed at the frontier?** Yes — and the frontier is now explicitly the
ledger's residue: JS-tree divergence on hydrating sites (washingtonpost's missing-div mass — a
named subsystem), SVG internal geometry, the 13× perf outlier class (wix/atlassian), viewBox
intrinsic ratio. A re-crawl after this window's fixes will re-rank honestly (starved runs now
self-discard).

**Is the agentic surface (component #2) being served?** Held — t385/389 are directly agent-food
(br line-ends measurable; icon buttons hittable); focusgroup/aria-actions flagged in audit #12 as
component-#2 watches.

**Any invariant bent?** No. The t384 change honored the two-cascades rule (both mutated together);
t389 honored the t153 lesson (used-size layout, never UA defaults); no mark touched; the netlify
crash fix preceded all capability work in the prior window and nothing regressed it.

**PART VI / VII correction.** None.

**Next check due: tick 398.**

---

## Check #16 — tick 398

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage.

**Gate or scoreboard?** Gate. Window 391-397: **391** the svg default-size model corrected against
MEASURED Chrome (our own t389 pin was wrong — the gate refused to lock a recalled model in),
**392** the honest re-crawl (all four jarring invariants moved down; the discard rule priced its
own coverage cost), **393/394** the SVG-internals spec and its paint half landed same-day (inline
vectors visible — a borrow, not a build), **395** the none→block family traced to path-pairing
(tree drift wearing style drift's face), **396/397** self-audit clean + a quiet surface week.

**Is the loop still pointed at the frontier?** Yes — and the frontier has CHANGED SHAPE: three
consecutive instrument-honesty finds (starved fetches t383, coverage price t392, path pairing
t395) make **selector-path keying** (redesign item a) the highest-leverage single item on the
board — it un-pollutes every display-diff family at once. Engine-side, the ledger's real residue
stands: SVG child geometry (spec ready), hydration-depth JS, the 13× perf outliers.

**Is the agentic surface (component #2) being served?** Held — t394's visible icons are also
hittable-target food; no regressions.

**Any invariant bent?** No. t391 is the RATCHET working on ourselves (a wrong pin corrected the
tick after it landed, evidence first); the nih.gov segfault is banked with evidence and routed to
its prescribed ASAN context, not chased or traded.

**PART VI / VII correction.** None.

**Next check due: tick 406.**

---

## Check #17 — tick 406

**Horizon:** Phase 0 — the FULL daily-driver checklist, CONSTITUTION.MD **Part VII**. **Gate:** the
FIDELITY-SCORING-REDESIGN.md certificate, NOT a WPT percentage.

**Gate or scoreboard?** Gate — and this window closed the instrument arc and cashed it. Window
399-405: **399** the keying spec (predictions written before the run), **400** stackTraceLimit
probed to an honest no, **401** selector-path keying LANDED (RED both ways, okta's 316-phantom
display family collapsed, MISSING rose honestly, baseline reset declared), **402/403/404** the
named-error harvest — the re-keyed instrument named three organs on okta's console and the loop
converted each into a RED-proven gated capability within one tick (document.location accessor,
getPropertyValue totality, currentScript lifetime) — I5 working exactly as written: the oracle
discovers, the engine follows. **405** self-audit clean.

**Is the loop still pointed at the frontier?** Yes. The redesign's items (a)-(d) are ALL now
landed in the oracle command; the re-keyed 265-site crawl is running off-path and its ledger
becomes the new priority ranking (pre-401 numbers are not comparable — the baseline-reset rule
is being honored, not spliced). Engine residue unchanged and named: SVG child geometry (spec
ready), hydration-depth JS tree drift, the 13× perf outliers.

**Is the agentic surface (component #2) being served?** Held — t402-404 are load-time
capability (auth widgets mount, chunk loaders bootstrap): pages an agent could not previously
observe now exist to observe. No semantic-model exposure lagged.

**Any invariant bent?** No. The G_GLOBALS re-pin (currentScript null→element) corrected a claim
that asserted the STUB's behavior against spec+Chrome — the t391 precedent, evidence first, not
a gate retune: the old claim asserted the exact bug the tick fixed. I2 untouched (prelude-side
fixes; no vendored source patched). The wall marks were never retuned (346s/499s first-runs
re-run warm to 57-70s per the standing recipe).

**PART VI / VII correction.** None.

**Next check due: tick 414.**

---

## Check #18 — tick 415

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north
star of CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75
structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT
percentage. Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 407-414 was one coherent vein — re-probe a near-done rendering
feature for the ONE dropped, silent-fail variant, RED-prove it, gate it, land it atomically: **407**
surface audit #12, **408** scroll-snap horizontal (real carousels report full x-geometry + x-axis
snap gated), **409** `content: attr()` resolves in the shipping Stylo generated-content path, **410**
map honesty (recorded the 408/409 flips before they rot), **411** list ordinals follow the HTML
ordinal-value algorithm (reversed + value-continuation, not the sibling index), **412**
text-transform capitalize titlecases the first LETTER past leading punctuation/digits, **413**
white-space pre-wrap PRESERVES spaces (it had shared pre-line's collapse path), **414** text-align
start/end resolve against direction (the RTL web was left-aligning its body text). Zero WPT-chasing,
zero encoding tail; each tick moved component #1 (rendering the representative real internet) with a
RED edit reverted byte-for-byte.

**Is the loop still pointed at the frontier?** Mostly — with an honest caveat I am recording rather
than smoothing over. The vein is atomic and RED-proven, which is exactly why it beats the L-sized
subsystems that cannot land in one tick (media playback JOIN, contenteditable, software WebGL —
PHASE0-BOUNDED-REMAINDER Tier 1). But two of the eight (t412 capitalize edge, arguably t411's value
edge) sit near the *cosmetic* boundary Part VI warns against, where "one fix flips ~nothing a user
feels." The steer for the next window: keep mining the bounded-gap vein, but apply VII.1's TEST hard
— each tick must fix a **visibly-broken site class**, not a pedantic conformance edge. The immediate
next lever (text-indent — unimplemented: only a code comment references it) qualifies squarely: it
drives both first-line indentation AND the ubiquitous image-replacement idiom
(`text-indent:-9999px`/`100%` on logos + icon buttons), where unhandled = duplicate text bleeding
over the background image site-wide. The higher marquee frontier remains the Tier-1 JARRING
subsystems (YouTube-plays media join first), which need a decompose-before-starting context, not an
atomic tick.

**Is the agentic surface (component #2) being served?** Held. The driving seams
(dispatch_click/key/composition, set_active/focus) are untouched and un-rotted; this window was
component-#1 rendering correctness, and every fix lands page-observable (getComputedStyle arms added
where relevant, e.g. t414's text-align). No semantic-model exposure lagged.

**Any invariant bent?** No. Bar 0 held — every landed tick additive and RED-proven, dom/css/layout
unit suites green (t413 86/86, t414 39/39 + 86/86), no crash/regression. I2 intact — no dependency
forked; the fixes are in our own cascade/layout/paint code against the specs' own algorithms
(HTML ordinal-value, CSS Text titlecasing, CSS logical-to-physical resolution). I3 served (query
surface kept in lockstep). THE RATCHET honored — nothing traded; the wall's cold 511s reading is
harness contention (observer-owned), re-runs warm, mark not retuned.

**PART VI / VII correction.** None. The four-component v1 scope is intact; PHASE0-BOUNDED-REMAINDER.md
remains the work list and the fidelity certificate remains the exit.

**Next check due: tick 423.**

## Check #19 — tick 423

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north
star of CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75
structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT
percentage. Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 416-423 shifted the vein one level: from re-probing near-done
RENDERING features to **probing a "works"-marked capability for hidden BINARY corruption / a stubbed
sub-path, RED-proving the exact silent-fail, and fixing it**. **416** text-indent (first-line indent
+ the `-9999px` image-replacement idiom), **417** -webkit-line-clamp (N-line + …), **418** Intl
measure-and-pin (ICU-backed, de-DE RED-prover), **419** Element.checkVisibility() measure-and-pin,
**420** IndexedDB getAllRecords() (Interop-2026, store+index, the key≠primaryKey RED-prover), **421**
structuredClone preserves binary types (a Uint8Array was degrading to `{0:..}` — silent corruption
also via postMessage), **422** Blob from binary parts holds BYTES not `String(part)` +
readAsArrayBuffer un-stubbed, **423** canvas ImageData ctor + real putImageData pixel write (was an
honest no-op that discarded every filter/histogram/editor edit). Each moved component #1 with a RED
edit reverted byte-for-byte; 420-423 are a coherent sub-vein (the platform's binary-data seams were
lossy behind a "works" label).

**Is the loop still pointed at the frontier?** Yes, and this window is squarely inside VII.1's TEST
(fix a *visibly-broken site class*, not a conformance edge): structuredClone/Blob/ImageData
corruption silently breaks uploads, image processing, decoded-media handling, and worker messaging —
whole classes of app, not cosmetics. The measure-first discipline (Process Rule 2) paid repeatedly:
a batch probe found ~15 modern JS/DOM built-ins ALREADY working (pinned Intl, checkVisibility) and
isolated the three real corruption gaps (421/422/423) instead of rebuilding what worked. The higher
marquee frontier is unchanged: the Tier-1 JARRING subsystems (YouTube-plays media JOIN first,
contenteditable, software WebGL) need a decompose-before-starting context, not an atomic tick.

**Is the agentic surface (component #2) being served?** Held. The driving seams
(dispatch_click/key/composition, set_active/focus) are untouched and un-rotted. This window was
component-#1 correctness at the JS-platform/canvas layer; every fix lands page-observable and each
capability got its gate (the semantic surface — getComputedStyle, IDB shim, canvas 2D — stayed in
lockstep, so I3 is served).

**Any invariant bent?** No. Bar 0 held — every landed tick (420/421/422) additive and RED-proven,
related regression suites green (t422: 9 blob/form/fetch/xhr/clipboard gates; t423: 7 canvas gates).
I2 intact — no dependency forked; fixes are in our own shims/native canvas against the specs' own
algorithms (structured-clone, Blob byte semantics, HTML putImageData raw-blit). I3 served. THE RATCHET
honored — nothing traded; the wall's repeated cold ~500s readings are harness contention (an
observer 5.8h oracle crawl + 96-99% swap, both observer-owned), the SAME tree warms to 57-66s on a
quiet window and lands, mark not retuned.

**PART VI / VII correction.** None. The four-component v1 scope is intact; PHASE0-BOUNDED-REMAINDER.md
remains the work list and the fidelity certificate remains the exit.

**Next check due: tick 431.**

## Check #20 — tick 431

**Horizon:** Phase 0 — the FULL daily-driver checklist ("runs almost every website"), the v1 north
star of CONSTITUTION.MD **Part VII**. **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75
structural/placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT
percentage. Authoritative work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 424-431 continued and then broadened the 420-427 vein: **probe a
capability the map marks "works" (or untested) for a HIDDEN corruption / stub / wrong-storage / snapshot
/ two-engines-disagree bug, RED-prove the exact silent-fail, fix it.** **424** TextDecoder honoured its
label (windows-1252 + utf-16, distinct from the HTTP encoding_rs path), **425** parsed `<template>
.content` held its children (the accessor read the wrong storage field), **426** `url.searchParams` went
live (a dead snapshot silently fetched the original URL), **427** computed CSS custom properties reached
getComputedStyle (design tokens), **428** surface audit #16 + Temporal measure-and-pin, **429** `:open`
in the querySelector engine (the STYLE cascade already had it — two-engines split), **430** `event
.getModifierState` (keyboard-shortcut libs), **431** `element.scrollTo`/`scrollBy` (programmatic scroll).
Each fixed a VISIBLY-broken class per VII.1 — silent binary corruption, mojibake, a framework rendering
nothing, an un-paginating URL, a blank theme, a dead shortcut, a no-op scroll — not a conformance edge.

**Is the loop still pointed at the frontier?** Yes. These are app-web table stakes (uploads, image
processing, theming, forms, disclosure widgets, shortcuts, scrolling), not the CSS-layout tail. The
measure-first discipline (Process Rule 2) kept paying: broad behavioral probes found ~40 modern APIs
ALREADY working (Temporal, the whole crypto/encoding/event surface) and isolated the ~11 real gaps to
fix, so zero effort was spent rebuilding what worked. The clean-bounded vein is now largely MINED — the
remaining probed gaps are subsystems (form.elements named access, custom-element reactions) that need a
decompose-before-starting context, not an atomic tick. The marquee frontier is unchanged: the Tier-1
JARRING subsystems (YouTube-plays media JOIN, contenteditable, software WebGL) per PHASE0-BOUNDED-REMAINDER.

**Is the agentic surface (component #2) being served?** Held. The driving seams
(dispatch_click/key/composition, set_active/focus, and now scrollTo which routes through the same
PENDING_ELEM_SCROLLS host channel) are untouched/un-rotted; every fix lands page-observable with its gate.
I3 served — the semantic surface (getComputedStyle custom props, the selector engine, the event surface)
stayed in lockstep.

**Any invariant bent?** No. Bar 0 held — every landed tick (424-430) additive and RED-proven, related
regression suites green each time (blob/form/fetch/xhr, canvas, dom/html crates, url, event, scroll,
css). I2 intact — no dependency forked; fixes are in our own shims/selector-engine/CSSOM plumbing against
the specs' own algorithms; the `:open` cascade side was ALREADY Stylo's (we only taught the second
engine). THE RATCHET honored — nothing traded. **The one honest scar:** the wall/landing tax turned
severe this window — an observer oracle crawl (~8.5h, nice-19) + swap 90-99% degraded the box below the
shell's `tab_operations_stay_far_under_one_frame` timing floor (a jitter-sensitive relative-cost guard),
so it false-RED'd nearly every verify and t427/t430 each took ~5 quiet-window retries to land. Documented
in JOURNAL + memory; harness/infra is observer-owned (no scripts/ edits, no swap-cycle). Not a regression
— the mark was not retuned; the SAME tree lands at 57-73s on a quiet box.

**PART VI / VII correction.** None. The four-component v1 scope is intact; PHASE0-BOUNDED-REMAINDER.md
remains the work list and the fidelity certificate remains the exit.

**Next check due: tick 439.**

## Check #21 — tick 439

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2: "reliably renders and runs the representative real internet," and an agent that drives the
DOM/a11y tree as first-class queryable state). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate
(≥0.75 placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Work
list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 432-439 ran a single coherent vein — the **legacy DOM-collection /
form-object surface every non-framework page still uses**, built as live platform objects in
`collections_js.rs`: **432** CSSOM array-like + `!important` round-trip, **433** `form.elements`
(HTMLFormControlsCollection + RadioNodeList), **434** `control.labels`/`label.control`, **435** the
`<table>` READ DOM (`table.rows` in logical order, `tr.cells`/indices), **436** the `<table>` WRITE DOM
(`insertRow`/`insertCell`/section builders), **437** `element.form` (the form owner), **438** the `<select>`
WRITE API, **439** `option.text` + `Option()` defaultSelected. Each fixed a VISIBLY-broken class per VII.1:
a form library that can't enumerate controls, a data-grid that can't read or build rows, a control that
can't find its form, a dropdown builder whose `select.add` threw — and t438's marquee, `select.remove(0)`
**silently DETACHING THE WHOLE SELECT** (a data-corruption bug dressed as a working method). These are
app-web table stakes, not the CSS-layout tail.

**Is the loop still pointed at the frontier?** Yes, with a caveat now worth stating. The measure-first
discipline (Process Rule 2) kept paying — every tick RED-proved the exact silent-fail before touching code,
and the read-side of each surface (options/selectedIndex/datalist.options/fieldset.elements) was found
ALREADY WORKING, so effort went only to the genuine write-side gaps. BUT: the clean-bounded DOM-collection
vein is now deep-mined. After the select/option follow-ons the remaining probed ore is thin —
`select.options.length` setter (a real HTMLOptionsCollection object), custom-element reactions (an L-sized
subsystem, not an atomic tick). The marquee frontier is unchanged and still bigger than anything in this
vein: the **Tier-1 JARRING subsystems** (YouTube-plays media JOIN, contenteditable+IME, software WebGL) per
PHASE0-BOUNDED-REMAINDER. The vein was correct to mine (real corruption bugs, cheap, high site-coverage) —
but the next context should weigh a JARRING subsystem against yet more collection follow-ons.

**Is the agentic surface (component #2) served?** Yes — directly. `form.elements`, `element.form`,
`table.rows`/`cells`, `select.add/remove`, `option.text` ARE the queryable+actuable DOM state an agent
reads and writes to drive a form or a data grid. Every fix landed page-observable with its gate; I3 held
(the semantic model — the collections, the form-owner graph, the option label — lands in lockstep with the
capability).

**Any invariant bent?** No. Bar 0 held — every tick 432-439 additive and RED-proven, sibling suites green
each time (form/collections/table/element-internals/reflect/CSSOM). I2 intact — no dependency forked; all
work is our own shims against the specs' own algorithms. THE RATCHET honored — nothing traded; t438's
`remove` override was written specifically to NOT regress `div.remove()` (gated as a must-not-break
invariant). **The one honest scar, unchanged from Check #20:** the flaky shell `tab_operations_stay_far_
under_one_frame` timing gate still false-REDs ~half of verify runs under the observer's concurrent
`manuk-wpt`/chrome oracle load (box at load ~4); each landing took 2-4 quiet-window verify retries. The
mark is NOT retuned — the same tree lands at 58-69s when a run catches a quiet slot. Harness/infra is
observer-owned (no scripts/ edits).

**PART VI / VII correction.** None. The four-component v1 scope is intact; PHASE0-BOUNDED-REMAINDER.md
remains the work list and the fidelity certificate remains the exit. Steer for the next context recorded
above: consider a Tier-1 JARRING subsystem over further DOM-collection follow-ons — the bounded vein has
given most of what it holds.

**Next check due: tick 447.**

## Check #22 — tick 447

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2: "reliably renders and runs the representative real internet," plus an agent driving the
DOM/a11y tree as first-class queryable+actuable state). **Gate:** the FIDELITY-SCORING-REDESIGN.md
certificate (≥0.75 placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT
percentage. Work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 440-447 continued to close **the JS-facing read/write value surface of
the DOM that every non-framework page and hand-written handler depends on** — the same VII.1 "visibly-broken
class" discipline: **440** textarea.value, **441** select.length, **442** valueAsNumber/stepUp/stepDown,
**443** valueAsDate for date/month/time, **444** progress.position + output.value, **445** `.text` for
a/script/title, **446** datetime-local + week typed values (closing the typed-input surface), **447**
`<a>`/`<area>` URL-decomposition SETTERS (`link.search=`/`a.hash=` were silent no-ops — the canonical
analytics-tag and in-page-nav idioms changed nothing). Each RED-proved a silent-fail/dead-setter before
touching code; each is app-web table stakes, not the CSS-layout tail.

**Is the loop still pointed at the frontier?** Yes — but Check #21's caveat is now firm and must steer the
NEXT context. The bounded "silent-fail on a near-done value/URL accessor" vein has been mined hard for ~15
ticks (432-447) and is genuinely thinning: t446/t447 were the last two documented follow-ons (the typed-input
tail and the anchor write-side). The measure-first re-probe kept catching stale-pessimism — this window alone
found dataset, classList variants, insertAdjacent*/before/after/replaceWith/prepend/append/replaceChildren,
toggleAttribute/getAttributeNames, closest/matches, splitText/wholeText, the FULL Intl surface, URL/
URLSearchParams, and every anchor GETTER **already working** — so real effort went only to genuine gaps. That
same probe density is now the signal the vein is near-exhausted: four consecutive probe batches this session
returned all-green before one (anchor setters) hit RED. **The steer for the next context is unchanged from #21
and now more urgent: weigh a Tier-1 JARRING subsystem (media playback JOIN → YouTube plays / contenteditable+
IME / software-WebGL) or the FIDELITY-INSTRUMENT REBUILD (the actual exit gate, agent-editable manuk-wpt Rust)
over yet another bounded value-accessor follow-on.** The bounded vein still has a few teeth (custom-element
reactions is L-sized, not atomic) but the marginal site-coverage per tick is falling.

**Is the agentic surface (component #2) served?** Directly. `a.search=`/`a.hash=`/`a.pathname=` ARE how an
agent (or the page's own script) rewrites a link's target before following it — queryable AND actuable URL
state, landing page-observable in lockstep with its gate (I3 held). datetime-local/week valueAsNumber is the
same for typed form state.

**Any invariant bent?** No. Bar 0 held — every tick 440-447 additive and RED-proven, sibling suites green each
time. I2 intact (our own shims against the specs' own algorithms + the real `url` crate for anchor setters —
no forked dependency). THE RATCHET honored — the anchor setter is tag-guarded to `<a>`/`<area>` so it can
never grow a spurious `href` on a plain element (a must-not-regress written into the fix). **The one honest
scar, unchanged:** the flaky shell timing gates (affordance/G_TEARDOWN/G_RUNTIME_COUNT/G_INTERACT) still
false-RED under the observer's concurrent oracle load + swap-98%; t446 took 3 verify retries to catch a quiet
slot. The mark is NOT retuned — the same tree lands at 60s warm. Harness/infra is observer-owned (no scripts/
edits).

**PART VI / VII correction.** None to the four-component v1 scope. PHASE0-BOUNDED-REMAINDER.md remains the
work list; the fidelity certificate remains the exit. The correction that IS due is a loop-direction one,
recorded above: the bounded value-accessor vein has paid out most of what it holds and the next context should
pivot to a Tier-1 subsystem or the fidelity instrument.

**Next check due: tick 455.**

## Check #23 — tick 455

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2: "reliably renders and runs the representative real internet," plus an agent driving the
DOM/a11y tree as first-class queryable+actuable state). **Gate:** the FIDELITY-SCORING-REDESIGN.md
certificate (≥0.75 placement fidelity on ≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT
percentage. Work list: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Window 448-454 ran one coherent vein — **the interaction / focusability /
form-state SELECTOR surface an agent and every non-framework page drive**, each RED-proving a silent-fail
before touching code: **448/449** pointer-events:none transparent to both the JS elementFromPoint and the
agent a11y hit-test, **450** the HTML `inert` attribute (reflection + subtree-walk into the a11y hittable
path), **451** inert blocks focus, **452** disabled blocks focus (shared `set_focus` sink), **453**
`:disabled`/`:enabled` honour `<fieldset disabled>` in BOTH selector engines, **454** `:read-only`/
`:read-write` match in the querySelector engine agreeing with the cascade. Each fixed a VISIBLY-broken
class per VII.1 — a modal focus-trap defeated, a greyed control that Tab-focuses, a bulk-disabled section
rendered un-greyed, a form library that queries the wrong fields — not a conformance edge.

**Is the loop still pointed at the frontier?** At the vein's edge — and this check formalizes the PIVOT
that Checks #21/#22 twice flagged. The two-engines-disagree SELECTOR thread is now closed for the
static-resolvable form pseudos (`:open` t429, `:disabled` t453, `:read-only`/`:read-write` t454 all agree
across cascade + querySelector), and the interaction/focus vein (inert/disabled/pointer-events) is
mined out. The measure-first re-probe kept paying (t454 found the cascade half already worked; only the
querySelector half was broken) but the marginal site-coverage per tick is now low and the remaining edges
are conformance-only (display:none focus) or shell-spanning (autofocus is a two-model split between
`set_focus` and the shell's `focused_input`, not atomic). **The steer, now acted on, not just recorded:**
the next context pivots to a **Tier-1 JARRING subsystem** per PHASE0-BOUNDED-REMAINDER — media playback
JOIN → YouTube (items 1+2, the marquee), or contenteditable+IME (item 3+4) — each needing a
decompose-before-starting context rather than a rushed atomic tick. `user-select` the PROPERTY is a real
remaining `?` but is NOT atomic (crates.io Stylo fences it behind `servo_pref="layout.unimplemented"`,
~65 props; the `./stylo` checkout builds nothing) — a blast-radius pref flip or a manuk-side supplement,
weighed fresh.

**Is the agentic surface (component #2) served?** Directly, and this window was among the most component-2
-dense in recent memory: pointer-events/inert hit-test transparency (t448/449/450) is literally the agent's
occlusion-aware hit_test seeing what a user's cursor would; `set_focus` refusing inert/disabled targets
(t451/452) is the agent's focus-grounding path; `:read-only`/`:read-write`/`:disabled` querying is the
queryable state an agent reads to know which controls are actuable. Every fix landed page-observable with
its gate; I3 held.

**Any invariant bent?** No. Bar 0 held — every tick 448-454 additive and RED-proven both ways, sibling
suites green each landing (css/selector/focus/pointer/inert gates). I2 intact — no dependency forked; all
work is our own selector engines + page focus sink against the specs' own algorithms; the cascade side of
each pseudo was ALREADY Stylo's (we only taught the querySelector engine to agree). THE RATCHET honored —
nothing traded; t454's cold 561s wall was pure contention (observer oracle load + relink), re-banked to a
warm 80s green receipt on a quiet slot, mark NOT retuned (the standing wall recipe, unchanged since #20).

**PART VI / VII correction.** None to the four-component v1 scope. PHASE0-BOUNDED-REMAINDER.md remains the
work list; the fidelity certificate remains the exit. The loop-direction correction — flagged in #21 and
#22, now executed — is the pivot OFF the bounded selector/interaction vein and ONTO a Tier-1 subsystem.

**Next check due: tick 463.**

## Check #24 — tick 463

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on
≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-ROADMAP-ANCHOR.md
(the observer's verified 85-150-tick bound from t460).

**Gate or scoreboard?** Gate. Window 456-463 executed the Check #23 pivot — OFF the mined-out
selector/interaction vein — but chose the **DAILY-DRIVER COMPLETENESS** face of Phase-0 (CO-#1 items D/E/F)
over the two big Tier-1 subsystems, because both marquees (media H.264-High, contenteditable EDITING) are
explicitly decompose-first / NON-atomic and the anchor prices them as 7-20-tick efforts. Each tick RED-proved
a VISIBLY-broken class first: **456/457** contenteditable query+selector agreement (the rich-editing entry,
per the anchor), **458** completeness identity (deviceMemory + canonical platform — the LinkedIn/Cloudflare
degraded-path tell), **459** `select.options.length` live accessor (the clear-the-dropdown idiom; 4th
dead-expando-getter-snapshot instance), **460** custom-element `attributeChangedCallback` on live
setAttribute (reactive web components froze at boot), **461/462** clipboard image read+write
(paste-a-screenshot / copy-image — both binary directions), **463** `document.execCommand('copy')` (the
legacy copy-button path). Not conformance edges — each is a class of real site that silently misbehaved.

**Is the loop still pointed at the frontier?** Yes, but the ATOMIC daily-driver vein is now visibly
thinning. The measure-first re-probe kept paying its rent (the recurring stale-doc lesson held HARD: probing
the 10 constellation `?` unknowns + DAILY-DRIVER-EDGES "missing" rows found details / createObjectURL /
scroll-anchor / conic-gradients / canvas-fillText / visibilityState ALL already built). What remains among
the `?` unknowns is NON-atomic residue: `ic`/`ric` + `ch`/`ex` are the StubFontMetrics subsystem,
`contrast-color()`/`user-select` are Stylo servo-pref fences, ESM module-graph is a subsystem, per-element
`zoom` is layout-math (against the "avoid the CSS-layout tail" steer). The genuine remaining frontier is the
anchor's named build items — every one a 2-20-tick SUBSYSTEM (rich-editing, WebGL, WebAuthn, vault, bidi,
animation-timeline, widgets, effects, MathML, multicol, print) — none an atomic tick. **The steer:** the
next context takes ONE of those as a decompose-first subsystem, or runs the FID-SWEEP exit instrument; do
NOT keep force-fitting subsystem bricks into atomic ticks past the point the clean vein is dry.

**Is the agentic surface (component #2) served?** Yes, indirectly this window — clipboard read/write and
execCommand copy are the agent's ability to move data in/out of a page, and `attributeChangedCallback`/
`select.options` liveness are queryable/actuable control state. Less component-2-dense than window 448-454
(which was hit-test/focus-grounding); this window leaned component-1 (completeness so real logged-in apps
take their normal path, not the degraded/"unknown client" one). I3 held — every fix landed page-observable
with its RED-proven gate.

**Any invariant bent?** No. Bar 0 held — every tick 458-463 additive and RED-proven both ways, sibling
suites green each landing. I2 intact — no dependency forked; the one borrow-shaped decision (base64 for the
clipboard binary bridge) reused the existing `b64`/`atob` transport rather than adding a crate. THE RATCHET
honored — nothing traded. The wall recurred as the session's main friction: the G_INTERACT tab-timing gate
false-REDs under the observer's tri-oracle sweep contention (Chrome+Firefox, load 6-7), costing t461 three
tick.sh retries and parking t463 complete-in-tree until a lull; the mark was NOT retuned (standing recipe:
warm re-run on a quiet slot). Harness-owned; reported, not fixed.

**PART VI / VII correction.** None to the four-component v1 scope. PHASE0-ROADMAP-ANCHOR.md is the new
authoritative bound + ledger (supersedes the free-standing BOUNDED-REMAINDER list); the fidelity certificate
remains the exit. The only correction is tempo: acknowledge the atomic-completeness vein is drying and stop
mining it a tick or two past dry — pivot to a decompose-first subsystem or the exit instrument.

**Next check due: tick 471.**

---

## Check #25 — tick 471

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on
≥95% of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-ROADMAP-ANCHOR.md.

**Gate or scoreboard?** Gate. Ticks 464-470 confirmed the Check #24 diagnosis to the letter — the atomic
vein was NOT merely thinning, it dried: 464-466 mined the last cheap Stylo `servo_pref` CSS computed-value
flips (user-select / color-scheme / contrast-color), 469 already had to fall back to `MinimalCascade`
RECOVERY for scrollbar-color/width (no pref helps a `engine="gecko"` prop), and 467/468/470 spent three
ticks bringing the `<details>` disclosure surface to genuine COMPLETENESS (accordion exclusivity, script-set
`.open`, `beforetoggle`-before-`toggle` on both paths). All real classes, all RED-proven — but unmistakably
the "a tick or two past dry" tempo #24 warned against. **Tick 471 executes the #24 steer:** it takes ONE of
the anchor's decompose-first subsystems — **contenteditable EDITING** — and lands its FIRST atomic brick:
`document.execCommand('insertText', …)` actually inserts text at the caret inside the editing host and fires
the `beforeinput`→(mutate DOM)→`input` (`inputType:'insertText'`) pair, with a cancelled `beforeinput`
vetoing the insert (no mutation, no `input`). Built ENTIRELY on the already-won substrate (Selection/Range,
`insertData`, `new Event`+`dispatchEvent`, the `isContentEditable` query surface from t456) — I2's "publish,
do not rebuild" — so the brick is small even though the subsystem is 7-20 ticks.

**Is the loop still pointed at the frontier?** Yes — and now correctly OFF the atomic-completeness treadmill
and ONTO a marquee subsystem, which is exactly what #23 and #24 both prescribed. The right follow-on bricks
are pre-identified and each is atomic against this same substrate: the DEFAULT typed-character action
(`dispatch_key` printable → insertText path) so a plain contenteditable accepts keystrokes; `insertParagraph`
(Enter → block split); `deleteContentBackward` (Backspace); `insertFromPaste` plaintext. Formatting
(`bold`/`italic` → wrapping `<b>`/`<i>`) and multi-node selection deletion are later, larger bricks —
honestly still `false` and `queryCommandSupported`-false, so a page feature-detects the truth.

**Is the agentic surface (component #2) served?** Directly. `insertText` is the mechanism by which the AGENT
(not just a page script) will fill an editable region — the write half of "observe control state AND change
it." It complements the existing typed-value path for form controls (`dispatch_composition` writes `.value`);
this writes the DOM of a contenteditable, the class of editor `.value` cannot reach. I3 held — page-observable
(DOM text + fired events), RED-proven both ways.

**Any invariant bent?** No. Bar 0 held — additive branch inside the existing `execCommand` shim, no signature
or storage change; the seven neighbor gates (exec_command_copy, contenteditable query/pseudo, ime_composition,
selection, range, set_range_text) stay green. I2 intact — ZERO new dependencies; reused Selection/Range/
CharacterData wholesale. THE RATCHET honored — nothing traded.

**PART VI / VII correction.** None. The four-component v1 scope and the fidelity-certificate exit stand. The
only tempo note: the subsystem pivot is now genuinely underway — keep landing contenteditable-EDITING bricks
against the shared substrate rather than reverting to atomic-completeness scavenging.

**Next check due: tick 479.**

## Check #26 — tick 479

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on ≥95%
of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-ROADMAP-ANCHOR.md.

**Gate or scoreboard?** Gate. Ticks 471-478 executed the Check #24/#25 steer to the letter: the
**contenteditable EDITING** subsystem (the anchor's decompose-first ledger item, "IN PROGRESS 10-20") went
from its first brick to eight — insertText (471), typed-char (472), Backspace (473), Delete (474),
insertLineBreak (475), cut (476), **KeyModifiers plumbing (477)**, Ctrl+X/C keyboard routing (478) — and
479 adds Shift+Enter→insertLineBreak. Every brick RED-proven, every one built on already-won substrate
(Selection/Range/insertData/`new Event`), ZERO new dependency (I2 "publish, do not rebuild"). This is a real
marquee subsystem being mined at a healthy tempo, NOT atomic-completeness scavenging.

**The one inflection worth naming:** t477 was the first CROSS-CUTTING brick of the arc — a Rust signature
change (`KeyModifiers` threaded through `dispatch_key`), the kind #25 flagged as "needs fresh context." It
landed atomically by keeping the public 5-arg `Page::dispatch_key` STABLE (delegating with a default) and
adding a 6-arg `dispatch_key_mods`, so all ~15 existing callers + both GUI sites compiled unchanged. That is
the correct pattern for the remaining cross-cutting bricks, and it paid off immediately: t478 (keyboard
cut/copy) and t479 (Shift+Enter) are both DIRECT consequences of the modifier substrate — the subsystem is
now compounding on itself rather than bolting on isolated features.

**Is the loop still pointed at the frontier?** Yes. The ANCHOR (observer, t461) still lists rich-editing as
the current IN-PROGRESS ledger item; the CO-#1 board's #1 (fidelity-instrument rebuild) is what the OBSERVER
is executing right now (the live oracle re-key crawl), so the agent's capability lane and the observer's
instrument lane are complementary, not in conflict. No drift.

**Is the agentic surface (component #2) served?** Directly and increasingly. The modifier substrate means the
AGENT can now dispatch real chords (Ctrl/Cmd/Shift+key) that pages react to — command palettes (Cmd+K),
keyboard cut/copy, Shift+Enter newlines. This is the "observe control state AND change it" write-half
extending from form-control `.value` (dispatch_composition) to rich editors AND to keyboard-driven app UIs.

**Any invariant bent?** No. Bar 0 held across the arc — additive branches inside the `dispatch_key` default
action + `execCommand` shim; the cross-cutting t477 signature change was contained (stable public API). THE
RATCHET honored: the only refusal this window (t478, WALL 476s>245s) was pure crawl-contention, NOT a
regression — diagnosed (22G RAM free, swap-99% stale, hung 30h observer crawl leaving idle Chrome), cleared
by ONE clean detached verify (213s), landed. No mark retuned. I2 intact — zero new deps across all 9 bricks.

**PART VI / VII correction.** None. The four-component v1 scope and the fidelity-certificate exit stand.
Tempo note: the contenteditable-EDITING subsystem is ~9 bricks in of the anchor's 10-20 estimate; the
remaining bricks (Ctrl+V/insertFromPaste, Enter→insertParagraph block-split, cross-block boundary merge,
formatting-command wrapping) are the harder tail — keep mining them against the shared substrate, and when
the subsystem saturates, the anchor's next ledger item (WebAuthn/vault/bidi/animations/…) is the pivot.

**Next check due: tick 487.**

## Check #27 — tick 487

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on ≥95%
of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Ticks 485-487 executed the Check #26 steer — off the saturated contenteditable
vein onto ANCHOR's next ledger items. t485 WebAuthn detection surface (Tier-1 item 6, first brick). t486
`navigator.userActivation` (a probe→build off the constellation-unknowns lane): gesture-gated features
(autoplay/fullscreen/popup/clipboard) read `.isActive` inside a click handler — absent → synchronous TypeError
→ dead button. Built as live getters over real gesture state flipped in `__dispatchEvent`, discriminated by a
private `__actgesture` marker (NOT `isTrusted`, which engine gestures wrongly read false because they carry a
supplied object). RED-proven three ways; +1 gate; every brick on already-won substrate, zero new dep (I2).

**The inflection worth naming — a vein transition, measured not guessed.** t486/t487 batch-probed ~40 JS
platform surfaces across two sweeps. The result is a HARD finding: the clean-bounded JS-surface vein is MINED
OUT. Already-built (re-confirming stale-pessimism a seventh time): connection, scheduler.postTask/yield, locks,
permissions, wakeLock, mediaSession, storage, clipboard, CSS.supports, structuredClone, reportError,
queueMicrotask, sendBeacon, PerformanceObserver, crypto.randomUUID/getRandomValues, visualViewport,
AbortSignal.timeout/any, ResizeObserver, IntersectionObserver, Object.hasOwn, Array.at, performance.*,
matchMedia.addEventListener. The only remaining JS gaps — navigator.share/canShare, vibrate, cpuPerformance,
CSS.registerProperty — are either honest-absent (matching desktop-Linux Chrome; feature-detect cleanly) or
present-but-inert TRAPS (registerProperty without cascade integration is worse than absent). So the honest next
frontier is NOT more surface probing; it is the sized SUBSYSTEMS in PHASE0-BOUNDED-REMAINDER.md.

**Is the loop still pointed at the frontier?** Yes, and this check SHARPENS the aim. The measured next lever is
Tier-2 item 23 (ch/ex real font metrics) — confirmed a live STUB this tick (`StubFontMetrics::query_font_metrics`
returns `FontMetrics::default()`, so 1ch=1ex=0.5em for every font; monospace `Nch` code blocks/terminals render
~20% too narrow). It moves the REAL gate (placement fidelity), but it is correctly a 2-3 tick cross-crate
subsystem, NOT an atomic tick: the `FontMetricsProvider` lives in the `Device` that Stylo shares across rayon
parallel-cascade threads, so the metrics oracle must be a `Send+Sync` handle threaded through every
`make_device` site (a thread-local would silently return defaults on worker threads — a correctness bug), and
`ex` additionally needs a new x-height query in manuk-text (LineMetrics exposes only ascent/descent/gap). This
is exactly the "decompose before starting" class the anchor names; forcing it into one tick would trade
correctness for a tick line, which the RATCHET refuses.

**Is the agentic surface (component #2) served?** Yes, materially. t486's activation state is tripped by
`dispatch_click`, so an agent driving a page now produces the same `navigator.userActivation` read-signal a real
user's gesture would — gesture-gated actions the agent initiates (play, share, fullscreen) are honoured rather
than silently gated off. The write-half (dispatch) now feeds the read-half (userActivation) pages check.

**Any invariant bent?** No. Bar 0 held — additive prelude getters + a contained `__dispatchEvent` bracket
(set-after-`type`, restore-at-single-return, save/restore for nesting); 11 neighbor gates green. THE RATCHET
honored: this check REFUSES to open ch/ex as a squeezed atomic tick precisely to avoid a parallel-cascade
correctness trade. I2 intact — zero new deps.

**PART VI / VII correction.** None. The four-component v1 scope and the fidelity-certificate exit stand. Tempo
note: the JS-surface probe lane is closed (measured, not assumed); the loop's next phase is subsystem work —
ch/ex font metrics, the fidelity-instrument rebuild, media codec breadth, password-vault UX, bidi reordering —
each decomposed before starting. Pick one, plan it, mine it brick-by-brick against the shared substrate.

**Next check due: tick 495.**

## Check #28 — tick 495

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on ≥95%
of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Ticks 489-494 mined the last CLEAN ATOMIC bricks from the JS-surface /
DOM-method / rendering-UA vein, each measured-missing-first (probe→build) and each a real daily-driver
capability, not a WPT-flip: t489 the global `[hidden]` attribute (rendered as `display:block` — every
script-toggled panel painted permanently); t490 `inputMode`/`enterKeyHint` (keyed to a nonexistent tag
`"undefinedelement"` → reflected on nothing); t491 `dialog.requestClose()` (close-with-veto); t493
`<img>.currentSrc` (the URL we actually load — read by lazy-load/lightbox/analytics on every image); t494
`document.activeElement` defaulting to `<body>` not `null` (a null crashes the ubiquitous
`activeElement.blur()` idiom). Two honest measurement ticks (487, 492) bracket them.

**The inflection, measured not guessed.** t492's second sweep pinned it: the clean-bounded atomic vein is
now MINED OUT one level below t487's JS-platform finding — at the DOM-method and CSS-property layers too.
Form-constraint-validation is fully built; the CSS gaps (`accent-color`/`touch-action`/`overscroll-behavior`/
`text-decoration-*`/`text-wrap`) are servo-DROPS (absent from the built `properties.rs`, and the @container
source-supplement trick does not rescue a dropped *property*); `getHTML`/Typed-OM/Highlight/`img.complete`+
`naturalWidth`/CSSOM-`.sheet` are each SUBSYSTEMS, not bricks. So the honest next frontier is unchanged from
Check #27 and now doubly-confirmed: the sized subsystems in PHASE0-BOUNDED-REMAINDER.md, led by ch/ex real
font metrics (the Send+Sync FontMetricsProvider threaded through `make_device` + an x-height query in
manuk-text — decompose-before-starting, NOT a squeezed atomic tick).

**Is the loop still pointed at the frontier?** Yes. Every tick this window moved a capability a real site
depends on (the agentic surface too: t494's activeElement fallback keeps an agent's focus reads non-null).
Zero drift — no single-site ticks, no html/dom flip-grinding, no CSS-layout-tail. The self-audit (this same
tick) found and remediated one journal continuity gap (t486's entry was missing; backfilled from its commit).

**Any invariant bent?** No. Bar 0 held every tick; each capability banked a RED-proven gate; I2 intact (zero
new deps — every brick was prelude JS, a native getter, or a UA-sheet rule on already-won substrate). THE
RATCHET held: t494's `manuk-shell` false-RED (a load-sensitive timing flake under observer cron-contention,
proven green on a direct run) was NOT traded for the capability — re-run, not re-baseline.

**PART VI / VII correction.** None. The four-component v1 scope and the fidelity-certificate exit stand. The
only tempo note: the atomic surface lane is closed (measured twice now); the loop's remaining Phase-0 work is
subsystem work, each decomposed before starting.

**Next check due: tick 503.**

## Check #29 — tick 503

**Horizon:** Phase 0 — daily-driver rendering parity + the agentic surface (CONSTITUTION.MD **Part VII**,
components 1 & 2). **Gate:** the FIDELITY-SCORING-REDESIGN.md certificate (≥0.75 placement fidelity on ≥95%
of the corpus + ≥0.70 per top-20 category), NOT a WPT percentage. Anchor: PHASE0-BOUNDED-REMAINDER.md.

**Gate or scoreboard?** Gate. Check #28 named the frontier — "ch/ex real font metrics… decompose-before-
starting, NOT a squeezed atomic tick" — and this window DELIVERED it: t499 `ch` (the `0`-glyph advance),
t500 `ex` (OS/2 sxHeight), t502 `cap` (OS/2 sCapHeight, which had been resolving to **0px** — its fallback
is ascent, which the provider left unset). All three are real daily-driver fidelity fixes: `ch` alone was
under-sizing every `max-width:65ch` article column by ~17% and overflowing monospace layouts. t501 was a
measurement tick that resolved 4 stale-unknown constellation cells with evidence (UNKNOWN 12→8).

**The decomposition was LIGHTER than Check #28 forecast — and that is the finding.** #28 anticipated "a
Send+Sync FontMetricsProvider threaded through `make_device`" — a signature-threading subsystem. The actual
shape avoided all of it: a thread-local `FontContext` in manuk-text behind three free functions
(`zero_advance_px`/`x_height_px`/`cap_height_px`) let the provider stay a trivial `Sync` unit struct that
just calls them, so NO viewport/Device signature changed. The seam is one new workspace-internal, `--features
stylo`-gated dep (manuk-css→manuk-text; no cycle, no new external crate → I2 intact). The lesson banked: a
subsystem's honest size is only known after you find the seam — decompose-first surfaced a 3-brick tick, not
the forecast multi-tick thread-through.

**Is the loop still pointed at the frontier?** Yes, and it just cleared the #1 named lever. Zero drift — no
single-site ticks, no html/dom flip-grinding, no CSS-layout-tail. `ic` was deliberately left at its `1em`
spec fallback (correct, and not cleanly falsifiable — pinned measured-partial, not faked green). The
remaining font-metrics work is the one hard, non-atomic follow-up: threading the page's own FontContext for
webfont-exact units (the thread-local carries system+generic faces only). The next frontier moves to the
other PHASE0-BOUNDED-REMAINDER subsystems (intrinsic sizing, MSE playback-join, ESM module-graph), each to
be decomposed the same way before starting.

**Any invariant bent?** No. Bar 0 held every tick; each capability banked a RED-proven gate (`ch` box:80→96,
`ex` 800→~832, `cap` 0→~1150); nothing regressed (each unfilled metric kept its spec fallback). THE RATCHET
held under pressure: EVERY engine tick this window first false-RED'd on the load-sensitive `manuk-shell` gate
cluster (affordance/G_TEARDOWN/G_RUNTIME_COUNT/G_INTERACT — flaky under the prewarm load-tail, green on the
quiet re-run) and NOT ONCE was that traded for the capability or the mark re-baselined — re-run on the quiet
box, exactly as the standing rule requires.

**PART VI / VII correction.** None. The four-component v1 scope and the fidelity-certificate exit stand.
Tempo note: the font-metrics lever named across Checks #27–28 is now closed; the loop returns to the
subsystem list, decompose-first each time.

**Next check due: tick 511.**

## Check #30 — tick 511

The due constitution re-read (cadence every 8; last 503). Re-read CONSTITUTION.MD Part VII against the
loop's actual direction over ticks 504-510 — a 7-tick, ALL-measurement arc (2 probe batches resolving
unknowns, 1 self-audit, 1 ESM decompose-first, 1 surface-audit, 1 lh probe, 1 final probe batch) with
ZERO capability BUILDS. That pattern is exactly what a drift-check must interrogate, so this check does.

**Is that drift? No — but it is at its limit, and the next tick must be a BUILD.** The measurement arc
was the correct response to a MEASURED fact: the atomic-capability vein is exhausted. This session
verified ~15 features the board/map called missing or unknown were ALREADY built+gated (fillText,
Fullscreen, cookie-attributes, userAgentData, visibilityState, getAllRecords, lh unit, :user-invalid
parse, image-rendering parse) or are genuine multi-tick SUBSYSTEMS (content-visibility layout, ESM
import-graph, MSE playback-join). When no atomic capability remains, resolving the stale-pessimistic
unknown backlog with evidence (instrument fidelity — a first-class ratchet face) and decompose-first-ing
the subsystems IS frontier work, and it served Part VII component 1 by making the map tell the truth
about what "reliably renders the real internet" already covers. Net: constellation UNKNOWN 8→3, 2 real
capabilities newly GATED (esmmodule, lhunit), the map VALIDATED clean against Interop 2026, the ESM
subsystem decomposed to landable bricks, all four cadences (self/surface/wall/const) current.

**The hazard now named: the INVERSE of the documented measurement-bias.** The board once warned ~85% of
ticks went to visible capability and 0 to seams; this window is the mirror — 100% measurement, 0 builds.
Continuing to measure would become measurement-AVOIDANCE of the hard subsystem work. So the ruling: the
cheap-probe vein is CLOSED (the 3 remaining unknowns — 100-tab RSS, test262, hidden=until-found — are
benchmark-runs/subsystems, not probes), and the next capability tick MUST be a subsystem BUILD BRICK —
ESM import-graph B1 (the decomposition is ready, GC-rooting hazard flagged) or MSE playback-join — on a
fresh quiet box, not another probe.

**Any invariant bent?** No. Bar 0 held all 7 ticks; every gated capability was RED-proven (behavioural
probes with a demonstrated wrong-output); nothing regressed; the MEASURED count-mark only rose or held.
THE RATCHET held against ~6 manuk-shell/G_INTERACT false-REDs this window (all re-run on the quiet box,
NEVER re-baselined). Zero drift toward deferred scope — no WPT-grinding, no security research, no later-
species work. Part VII four-component scope stands; no correction.

**Next check due: tick 519.**

## Check #31 — tick 519

The due constitution re-read (cadence every 8; last #30 at 511). Re-read CONSTITUTION.MD Parts I/VI/VII
against the loop's direction over ticks 511-518.

**HORIZON: H0 / v1 (Part VII). GATE: "reliably renders and runs the representative real internet"
(usage-weighted, real-sites-moved — GUI + headless) + a complete agentic surface. Explicitly NOT a WPT %.**

**Gate or scoreboard? GATE — decisively, and exactly as #30 ordered.** Check #30 ruled "the next capability
tick MUST be a subsystem BUILD BRICK — ESM import-graph B1." The loop did precisely that: t512 B1 (GC-rooted
registry + import.meta.url), t513 B2 (resolve hook), t514 B3 (population walk), t515 B3b-i (page runner
consumer), t516 B3b-ii (async producer on load_async), t517 B3b-iii (shell producer + unify seam). The
result is a CLASS UNLOCK, not a scoreboard bump: a real multi-file `import {x} from './y.js'` graph now
resolves on BOTH real page paths — the GUI window (prefetch_document→from_prefetched) and the
headless/AGENT path (fetch_streaming_page→load_async). That serves Part VII component 1 (native-ESM /
Vite-dev / no-bundler apps are a large usage-weighted slice of the modern web — real-sites-moved, not a
subtest count) AND component 2 (the agent renders through the same load_async path, so it too resolves
graphs). t518 (surface audit #25) then reconciled the map: constellation `ESM module-graph loading`
partial→gated, and confirmed the map is otherwise strikingly clean against Interop 2026 (every focus area
already has a verdict).

**Orient's north star intact?** Yes — usage-weighted breadth, not tail. `type=module` import graphs are
ubiquitous (every Vite/Rollup/esbuild output); this is the opposite of the encoding-tail trap VI.3 warns
against. No big-but-tail number crept to the top.

**Any invariant bent?** No. I1 (crate structure) untouched. I2 (never patch deps) — nothing vendored was
patched; all ESM work is in our own manuk-js/manuk-page. I3 (semantic model in lockstep) — the loader is a
mechanism whose JS-visible behaviour (imports resolve → modules evaluate → DOM mutates) IS the semantic
exposure, gated behaviourally (g_esm_page_graph/g_esm_prefetched_graph, both RED-proven); no rendered
construct went un-exposed. I4 (Pareto) — high-usage class. Bar 0 held every tick (a fetch miss is
loud-but-safe, never a crash). No drift toward deferred scope (no per-origin isolation, no later-species
work, no 83%-WPT grind). THE RATCHET held against the recurring manuk-shell false-RED (cold-build load
spike; re-run on the quiet box lands it — NEVER re-baselined; harness-owned).

**PART VI correction (recorded here, per the #28-30 pattern — not by editing the tick-86 document):** VI.2's
"genuinely not done" frame is now one subsystem lighter — ESM import-graph loading is DONE. The standing
ground truth from #30 holds and sharpens: the atomic-capability vein is exhausted and the frontier is the
PHASE0-BOUNDED-REMAINDER subsystems; one of them (ESM import graphs) has now landed end-to-end. The
remaining bounded-remainder mass is the real blocker to the Part-VII component-1 gate.

**STEER: 520 MUST BUILD — no third consecutive meta tick.** 518 (surface) and 519 (const) are the two
cadence-meta ticks; the #30 discipline ("the next tick MUST build, not another probe") applies again.
Closest-to-gate candidates, re-run the board to pick: (a) the biggest remaining capability GAP — media
playback-join (Part VII component 1; a decompose-first subsystem per MEDIA.md, the #30 pattern), or (b) a
bounded ESM follow-up that widens the just-unlocked class — import maps (bare-specifier resolution unlocks
CDN-pinned no-bundler apps) or dynamic `import()` (the lazy hook). test262 is high-value but it is
MEASUREMENT (instrument fidelity) — valuable, yet after two meta ticks the loop must BUILD, so test262 is
sequenced behind the next build brick, not ahead of it.

**Next check due: tick 527.**

## Check #32 — tick 527

Re-read CONSTITUTION.MD Part VII (V1 scope) and the RATCHET/north-star framing. Since #31 the loop
landed a coherent **media playback-model arc** (t521 running clock + timeupdate/ended, t522
currentTime-as-seek, t523 played-union, t524 durationchange), a **self-audit** (t525), and closed a real
constellation "missing" row (**t526 ToggleEvent.source**, the popover invoker).

**Gate or scoreboard?** GATE, every tick. Each landed as a qualitative capability step banked behind a
gate proven to go RED (g_media_playback_clock / g_media_seek / g_media_played / g_media_durationchange /
g_toggle_event_source), not a subtest-count bump. The media arc is the JS-visible half of Part VII
component 1 (real video sites): `<video>` now has a running clock, a real seek, a watched-spans union
and a durationchange — the exact event/property set every progress bar, %-watched beacon, scrub bar,
transcript, autoplay-next and resume marker binds. This is usage-weighted breadth (ubiquitous on the
video web), the opposite of the encoding-tail trap VI.3 warns against.

**North star intact?** Yes. Capability MATCHED toward Chrome (the events/properties a player reads now
exist and are honest); performance/stability EXCEEDED (Bar 0 held every tick — the clock is inert until
the host drives it, a stale id is a silent no-op, a seek is clamped, over-scroll is safe; all additive,
nothing regressed). No timing divergence introduced into the oracle.

**Any invariant bent?** No. I1 (crate structure) untouched. I2 (never patch deps) — all work is in our
own manuk-js/manuk-page; nothing vendored patched. I3 (semantic model in lockstep) — every mechanism's
JS-visible behaviour IS the exposure, gated behaviourally and RED-proven. I4 (Pareto) — high-usage
classes. THE RATCHET held against the recurring `manuk-shell tests FAILED` false-RED, which struck THREE
times this session (t523/t525/t526, always under the gate-phase load spike, gate+parity+perf all green) —
each landed on a WARM re-run on the quiet box, NEVER re-baselined, NEVER touched scripts/ (harness-owned,
per the charter and the self-audit's wall finding).

**The honest boundary, recorded (per the #28-31 pattern):** the media JS-surface is now SATURATED, and
the media FRONTIER beyond it is genuinely XL and NOT bounded — (a) the shell frame-loop DRIVER calling
`__mediaAdvance` (GUI, false-RED risk), (b) codec breadth / WebM-VP9 (the CUT line — no usable Rust VP9
decoder; AV1+H.264 covers the practical web). And the atomic + bounded-capability vein is CONFIRMED mined
out this session by re-probe (IndexedDB indexes / cookie attributes / fetch body+headers / WebAuthn all
already built; PHASE0-BOUNDED-REMAINDER.md is stale). So VI.2's "genuinely not done" frame is another
subsystem lighter.

**STEER: the next capability frontier is a DELIBERATE PIVOT, not more small bricks.** The board's CO-#1 is
the **fidelity instrument rebuild** (agent-editable manuk-wpt Rust probe per FIDELITY-SCORING-REDESIGN.md
— THE Phase-0 EXIT gate, which now outranks another single-capability tick; but it is a subsystem that
LIES if half-built, so it needs a decompose-first session, the #30 pattern). Sequenced alternatives:
**test262** (measurement, permitted after this run of build bricks) or the two buildable remaining
unknowns (name-only container queries; the `<dialog>` ToggleEvent.source / command-invoker follow-up).
Do NOT resume mining atomic members — the vein is out.

**Next check due: tick 535.**

## Check #33 — tick 535

Re-read CONSTITUTION.MD Part VII (V1 scope) + the RATCHET's three faces (capability / performance /
**instrument fidelity**). Since #32 the loop did exactly what #32 steered: opened the deliberate PIVOT to
the **fidelity-instrument rebuild** (board CO-#1, THE Phase-0 EXIT gate) as a decompose-first arc, after
finishing the Surface-Audit-#26 reconciliation. Window 528-534: **528** surface audit #26, **529/530**
measure+pin (Promise.withResolvers / Set methods / scheduler.postTask — all already in SpiderMonkey, now
gated; rVFC measured-missing), then the rebuild bricks — **531** SHAPE primitive, **532** selector-path
producer wires SHAPE into the G1 report (the enabling §3a fix — 39% of the corpus was unmeasurable on `[id]`
keys), **533** the first Layer-2 jarring invariant (horizontal overflow) in the G1 probe, **534** self-audit
(clean).

**Gate or scoreboard?** GATE — and this window builds the gate's own INSTRUMENT, which is the third RATCHET
face, not the scoreboard. VII.1's TEST for component #1 is *"reliably renders the representative real
internet, NOT a WPT percentage"* — so the instrument that certifies exactly that outranks flipping subtests
(the #30/#32 ranking, unchanged). Zero WPT-chasing, zero encoding tail; each brick moved the exit
instrument's honesty (SHAPE replaces the misleading absolute PLACEMENT that charged one 23px offset N times;
path keys unlock the React/Tailwind majority; h-overflow is the first of §2's five jarring invariants — "the
actual Phase-0 bar").

**North star intact?** Yes. This is pure instrument-fidelity work bought WITHOUT degrading capability or
performance (every brick additive + RED-proven, the RED edit reverted byte-for-byte; the wall stayed green).
Decompose-first discipline held: brick 1 landed SHAPE as a tested primitive but left it UNWIRED and SAID SO
(a half-built instrument LIES — the #30 lesson); brick 2 wired it only once the producer emitted real
ancestry; the gate FLOOR still gates on COVERAGE and the flip to SHAPE is explicitly deferred to a
recalibrating sweep. "The primitive is proven now, the number is claimed later" every brick.

**Any invariant bent?** No. **I2** (never patch deps) — all work is in the agent-editable manuk-wpt harness
(chrome.rs / fidelity.rs / oracle.rs / main.rs); nothing vendored patched, no engine capability src touched.
**I1** (crate structure) untouched. **ONE-DEFINITION discipline honored**: the Manuk `sig_of`/`path_of` were
extracted from the oracle's local closures into shared free functions used by BOTH the oracle and the G1
probe, and `jarring_h_overflow` was refactored to delegate to a shared `h_overflow_boxes` core — so the exit
gate and the oracle can never drift on what a key or an invariant means. **Bar 0 held**; the recurring
`manuk-shell tests FAILED` false-RED struck once more (t532, under the gate-phase load spike, teardown+all
gates green) and landed on a WARM re-run, mark NEVER retuned, scripts/ NEVER touched. t533 came in at a slow
247s but PASSED — a slow-but-green wall lands; not a false-RED, not re-baselined.

**PART VI / VII correction.** None. The four-component v1 scope is intact; the fidelity certificate remains
the exit. **STEER (unchanged from #32, now mid-execution):** continue the rebuild decompose-first — brick 4
is the producer enrichment (emit tag+display, key into `Seen`) that unlocks the remaining three Layer-2
invariants (overlap / reading-order / collapsed-target) to reuse the oracle fns directly; then root-cause
clustering (§3b); then the gate-floor flip (coverage→SHAPE) once a broad path-keyed sweep recalibrates the
0.75 bar. Do NOT resume mining atomic members (vein confirmed out at #32) and do NOT grind the CSS-layout tail.

**Next check due: tick 543.**

## Check #34 — tick 543

Re-read CONSTITUTION.MD Part I (invariants) + Part VII (V1 scope) + the RATCHET's three faces. Window
535-543 CLOSED the fidelity-instrument rebuild's CORE: **536** collapsed-target invariant, **537** brick 4b
(producer enriched to `Seen`, all four Layer-2 jarring invariants wired DIRECT on the oracle fns), **540**
brick 5 §3b root-cause clustering (`run_fidelity_cmd` pools per-page `oracle::diff_page` divergences and
calls `oracle::cluster` VERBATIM — the exit gate now reports DISTINCT CAUSES not failing sites, sharing ONE
definition with the differential crawl). Bricks 1–5 of FIDELITY-SCORING-REDESIGN.md are DONE. Interleaved
measure-and-pin: **538** surface audit #27, **539** light-dark/CSS-math parse probes, **541** name-only
@container (→gated), **542** hidden=until-found (→partial), **543** light-dark RESOLUTION (measured absent —
always the light arm).

**Horizon + gate.** H0 / Phase-0. The EXIT GATE is VII.1's test for component #1: *reliably renders the
representative real internet — the fidelity CERTIFICATE, NOT a WPT percentage.*

**Gate or scoreboard?** GATE. The rebuild bricks moved the exit INSTRUMENT itself (the third RATCHET face) —
§3b clustering is the last piece that lets the certificate distinguish saturation from amplification ("40
sites, 1 bug"). The probes are the board's ranked cheapest-highest-yield measure-and-pin (`?`/`partial`
outranks `X`); they keep the MAP honest (a wrong `unknown` steers the loop blind) without pretending to move
the gate. Zero WPT-chasing, zero encoding tail across the whole window — orient's usage-weighted breadth
(§VI.3) is still the north star; no big-but-tail number crept back to the top.

**Any invariant bent?** No — and one was actively HONORED under temptation. **I2 (never patch deps):** the
window MEASURED four capabilities absent that live behind the servo-Stylo cfg fence — content-visibility
(t542 `contentvis:no`) and light-dark RESOLUTION (t543 `lightdarkresolve:no`, always the light arm because
the cascaded color-scheme is not consulted). The correct response to each was a pinned measured-absent
RECEIPT, NOT patching vendored Stylo to force it — exactly what I2 demands. The whole `missing` residue
(subgrid, @scope, @starting-style, text-wrap:balance, anchor-positioning, scroll-driven-animations) is the
same fence; grinding it would mean patching deps, so it stays honestly unmet. **I3 (semantic model in
lockstep):** all window work is measurement — no capability landed without exposure. **I4 (Pareto):** every
probe is usage-weighted (dark mode, accordions/FAQ, component-library container scoping), not tail. **Bar 0
held** every tick.

**Part VI correction.** The fidelity-instrument rebuild is no longer "in progress" — its CORE (bricks 1–5)
is COMPLETE. The ONE remaining rebuild item is the coverage→SHAPE gate-floor flip, and it is BLOCKED on a
broad path-keyed SHAPE-headline sweep to recalibrate the 0.75 bar — a calibration the observer owns and that
must NOT be self-served by retuning the floor to land a tick (the honest-answer-is-not-a-fixed-answer rule).
So the real blocker for the Phase-0 EXIT CERTIFICATE is now that calibration sweep, not any agent-editable
instrument code. Separately, the bounded probe/measure vein is genuinely MINED OUT: the 2 remaining
`unknown` cells are subsystems (test262 harness, 100-tab RSS MEM-HARNESS), and the `missing` rows are XL
subsystems (WebGL/WebRTC/WebCodecs/WebTransport/MathML), out-of-scope (EME, JPEG-XL), or the servo-drop
fence above.

**STEER.** The instrument is built; the certificate awaits the observer's broad sweep. Do NOT manufacture
marginal ticks to keep a cadence — novelty never outranks the gate (Part III standing rule). The honest next
moves, in order: (1) if the observer banks a broad path-keyed SHAPE headline, DO the coverage→SHAPE
gate-floor flip (the certificate's last piece). (2) Otherwise advance an XL subsystem as a DECOMPOSE-FIRST
arc from the board — media playback join / the pivot list — never a half-built lump. (3) Hold for
calibration is acceptable over drift. RE-PROBE stale-pessimistic before building anything marked missing
(the rule paid AGAIN this window: IndexedDB/Service-Worker/Cache/scroll-snap were all already gated).

**Next check due: tick 551.**

## Check #35 — tick 551

**HORIZON: H0 — Pareto Web Parity.** EXIT GATE (all binary): ~83% WPT across categories ·
differential-oracle-verified viability across all four usage-weighted corpora (document, SPA/framework,
social/platform, high-traffic utility) · the headful shell daily-drivable by its own developer · every
rendered construct queryable through the in-process semantic API.

**GATE OR SCOREBOARD? — GATE, and this is the first window in a long time where that is unambiguous.**
The last eight ticks (544–551) did almost nothing to the scoreboard: `WPT:TOTAL` is flat at 422,865 across
every one of them, and the readiness meter went 80 → 81 → 80. What moved is **exit-gate condition #2**,
the one the constitution words as *"differential-oracle-verified viability across the four corpora"* — and
it moved by being **measured for the first time on the rebuilt instrument** rather than by being improved:

- t546 ran **test262** for the first time ever: 94.14% of 87,009 executed subtests, 81.41% honest.
  That closed one of the two remaining `unknown` cells Check #34 named as blocking.
- t547 made the **exit certificate computable by the instrument that measures it** (the jarring counts were
  being printed and discarded, so the certificate could only be reached by a human summing 265 stanzas).
- t549 ran the **first corpus certificate**: every term below bar — shape ≥0.75 on 5.6% of sites,
  h-overflow 77.8%, overlap 59.3%, reading-order 46.3%, dead-target 75.9%.
- t550 then found that the sub-5%-coverage "class failure" t549 reported was **substantially the
  instrument**: with the class signature off the path key, gov.uk goes 0.0% → 82.8% and stripe 0.1% →
  43.1%, while three healthy sites are byte-identical.

**CORRECTION TO CHECK #34, and it is the important entry in this check.** #34 concluded *"the instrument
is built; the certificate awaits the observer's broad sweep"* and steered toward holding for a calibration
the observer owns. That was **wrong in a specific, instructive way**: it treated "the instrument is built"
as a property that could be established without running it broadly. Two ticks of running it broadly found
(a) the certificate could be **passed vacuously** (`0/0 = 1.0`, seven sites reporting `SHAPE 100.0% (0
scored)`, one of them with all 418 probed elements missing) and (b) the coverage key was **reporting healthy
pages as 0%**. Neither is a calibration question and neither needed the observer. **An instrument is not
built until it has been run at scale against reality** — and #34's "hold for calibration is acceptable over
drift" was, in hindsight, a way of not looking. The steer that actually paid was the observer's STEP-1
("run the exit verification NOW"), which #34 had subordinated.

**PART VI CORRECTIONS** (the reconciliation has drifted; re-derived from the tree):
- **VI.2 / I5 — "the differential oracle never completed a full crawl"** is stale twice over. `STATUS.md`
  reports `ORACLE_CRAWLED: 265 sites, 392 clusters`, and as of t549 the *fidelity* instrument has also swept
  a stratified 72-site sample end-to-end with a computed certificate. The honest current statement is not
  "not operational" but **"operational, and its keying was measurably wrong until t550"**.
- **VI.2 / H0.1 — `css-flexbox 5.5%` / `css-grid 4.7%`** are stale: the board reads flexbox 6.2%, grid 5.3%
  today. Still the weak spot in raw terms, but the standing steer (and the observer's) is explicitly NOT to
  grind the CSS-layout tail — and t549's sweep supports that empirically: the corpus failures cluster on
  `missing box: <div>` and 16–128px height deltas, not on flex/grid conformance.
- **NEW, and it belongs in VI.2:** the H0 gate's condition #2 now has a *number* with a *bar*, and both
  live in `docs/loop/PHASE0-ROADMAP-ANCHOR.md §6`. Before t547 that condition was unfalsifiable in practice.

**INVARIANTS — is any being bent?** I4 (Pareto): no; the last eight ticks avoided the encoding/CSS tail
entirely. I2 (never patch deps): no; the t546 SpiderMonkey work installs **embedder host hooks** (job queue,
`SetHostCleanupFinalizationRegistryCallback`) through public API — the opposite of patching. I3 (semantic
model in lockstep): **watch this one.** Six of the last eight ticks were instrument/measurement work with no
semantic-API surface, which is legitimate for measurement but means the I3 habit went unexercised for a
window; the next capability tick must not skip it.

**STEER.** Stay on the observer's STEP 1 — it is the gate, and running it is what has produced every real
finding this window. In order: **(1) RE-SWEEP on the corrected keying** — t549's line is the record of what
was measured, not a baseline, and the re-sweep is the first actuals line that may legitimately be
differenced. **(2) nytimes.com as a NAMED single-site investigation** — it did *not* move under the keying
fix (2,381 of 2,382 still missing, 27s vs Chromium 6s, load budget exhausted AND the 20,000-task ceiling
hit; three facts that probably share one cause), and it is the largest *real* failure the sweep found.
**(3) the crawl-side sig correction** (`run_oracle_cmd` keys still carry sigs — the same defect, unfixed,
in the instrument the constitution calls its primary discovery mechanism). Then STEP 1(c) 100-tab RSS and
Audit #28's three CSS probes. **Do NOT** start an XL subsystem while the exit instrument is still returning
corrections about itself — that is the ordering error #34 made in reverse.

**Next check due: tick 559.**

## Check #36 — tick 559

**HORIZON: H0 — Pareto Web Parity.** EXIT GATE (all binary): ~83% WPT across categories ·
differential-oracle-verified viability across all four usage-weighted corpora · the headful shell
daily-drivable by its own developer · every rendered construct queryable through the in-process semantic API.

**GATE OR SCOREBOARD? — GATE, and this window finally converted measurement into a CAPABILITY fix.**
Check #35 (t551) answered GATE on the grounds that condition #2 had become falsifiable. The eight ticks
since (552–559) did something better: they used it. `WPT:TOTAL` is *still* flat at 422,865 — the scoreboard
has not moved in sixteen ticks — while the instrument found, and the engine fixed, **the largest text-fidelity
defect in the repo**: no named font family had ever resolved, because `fontdb::Family::Name` is
case-sensitive and we lowercased before querying (t557), and then because `intern_family` discarded the case
one line later (t558). Measured on the committed probe against live Chromium: **SHAPE 36.4% → 90.9%**.

**THE PROCESS RESULT WORTH RECORDING, because it is the loop working as designed.** Nine ticks, one arc:
t549 swept and got a number · t550 found the coverage key was reporting healthy pages as 0% · t551 re-swept
and isolated placement as the real gap · t552 caught its own power-of-two inference as a printer artifact ·
t553 printed an instance and falsified its own lead · t554 split the confounded signature and the lead came
back · t555 sampled three instances and two independent signals named text metrics · t556 measured
SELECTION vs COMPUTATION on a committed probe · t557 and t558 fixed it in two halves. **Six of those nine
ticks corrected something the loop itself had just concluded**, and every correction was cheap because each
one ended in a RED-proven assertion rather than a note. The alternative — acting on t551's reading — would
have been one expensive tick in the box-model subsystem, which is not where the bug was.

**INVARIANTS.** I4 (Pareto): held — the arc chased what 13–18 corpus sites showed, not a WPT directory.
I2 (never patch deps): held — the `fontdb` defect was fixed by passing it the string it documents wanting,
not by forking it. I3 (semantic model in lockstep): **still the one to watch, and #35's warning was not
acted on.** t557/t558 are engine capability with no semantic-API surface, which is defensible for a text
metric (there is nothing new to query) — but surface audit #29 then found `<search>` and `CloseWatcher`
unmapped, both of which ARE agentic surface, and that is the second signal in two checks that the agentic
half of I3 is drifting out of habit. **Steer below acts on it.**

**PART VI CORRECTION.** VI.2's H0.8 line — *"text shaping / i18n to Pareto depth"* — was reading as a
future item. It is now demonstrably a **present, load-bearing, and partly-broken** subsystem: shaping runs
through swash, bidi and CJK are measured working, `ch`/`ex`/`cap` read real metrics (t499–502), and the
single largest fidelity defect found this year lived in its *face selection*, not its shaping. Correct the
map: text is not a depth question to be scheduled, it is a correctness surface already in the critical path,
and the fidelity sweep is what makes its defects visible.

**STEER, in order.** (1) **Re-sweep the corpus** — the font fix is the first change since t551 that should
move the SHAPE baseline, and t551 is differenceable, so this is the first honest before/after the instrument
has ever been able to report. Running now. (2) **Take the two agentic rows from Audit #29 together**
(`<search>`'s implicit `role=search` and `CloseWatcher` as an overlay-dismissal actuator) as ONE tick that
exercises I3 rather than two CSS-shaped probes — that discharges the drift #35 flagged and #36 is repeating.
(3) The t556 cascade-origin bug (author `* { margin:0 }` losing to the UA `body { margin:8px }` — every CSS
reset on the web hits it, so its blast radius is far wider than 8px suggests). (4) nytimes.com as a named
single-site investigation. (5) The crawl-side `.SIG` correction — `run_oracle_cmd` still carries the defect
t550 fixed in the exit gate, in the instrument the constitution calls its primary discovery mechanism.
**Do NOT** open an XL subsystem while the exit instrument is still producing capability fixes at this rate;
the marginal value of another measurement brick is currently higher than a new organ.

**Next check due: tick 567.**

## Check #37 — tick 567

**HORIZON: H0 — Pareto Web Parity.** EXIT GATE (all binary): ~83% WPT across categories ·
differential-oracle-verified viability across all four usage-weighted corpora · the headful shell
daily-drivable by its own developer · every rendered construct queryable through the in-process semantic API.

**GATE OR SCOREBOARD? — GATE, and the eight ticks since #36 are the clearest case yet.** `WPT:TOTAL` is *still*
flat at 422,865 — **twenty-four ticks without a scoreboard move** — while t560–t566 produced: `@font-face`
shadowing, the diff carrying the computed font, **`@import` chains fetched at all** (an unfetched import had
been silently deleting whole stylesheets, which is a large class of the real web), and a root cause isolated in
implied grid track sizing. Two of those are capability, two are the instrument earning its keep. Condition #2 is
the one moving, and it is moving because the instrument keeps getting sharper.

**THE PROCESS RESULT THIS WINDOW, and it is a correction to my own method.** Six leads died on contact with a
measurement across t551–t566, which #36 praised. t565/t566 showed the failure mode of that same discipline:
**`grid-template-areas.html` scored a perfect 100% and produced a CONFIDENT WRONG CONCLUSION** ("grid placement
is not the defect"), because it set `grid-template-columns` alongside the areas and the bug was one declaration
away. **An over-specified probe hides the bug it was written to find** — and a passing probe carries far more
authority than a cluster ranking, so this error is more expensive than the five before it. The rule to carry:
**when a probe PASSES, ask what it holds fixed that the real page does not.** That belongs beside
`parity-methodology`'s "when a metric won't move, suspect the metric".

**INVARIANTS.** I4 (Pareto): held — everything this window was corpus-driven, and t566 explicitly declined to
read the `css-grid 5.3%` WPT number as a mandate (it would have sent a tick at working code). I2: held —
`@import` fetching is embedder work Stylo already expects (`AllowImportRules::Yes` parses the rule; the bytes
were always ours to supply). **I3 (semantic model in lockstep): NOW OVERDUE, third check running.** #35 flagged
it, #36 flagged it and steered to discharge it, and #37 finds it still undone: Audit #29's `<search>` (implicit
`role=search`) and `CloseWatcher` (an overlay-dismissal actuator) remain unmapped, and every tick since has
been rendering-side. **This is no longer drift, it is a queue that keeps being deprioritised by more legible
work** — which is exactly the failure mode I3 exists to prevent.

**PART VI CORRECTION.** VI.2's `H0.6 networking completion` line reads as *"cache, cookies with SameSite
partitioning, HTTP/2/3"* — a list of protocol features. t564 found the gap that actually mattered was neither:
**we never fetched `@import`ed stylesheets**, a CSS-delivery hole in the *resource* layer that deleted whole
sheets and surfaced as a font bug three subsystems away. Correct the map: networking completion is not only
protocol depth, it is **resource-graph completeness** — every URL the CSS/HTML/JS graph names must actually be
fetched, and the honest way to find the holes is the corpus, not the spec's table of contents.

**STEER — and I3 goes FIRST this time, ahead of the hotter lead, precisely because it has been deferred three
checks running.** (1) **`<search>` + `CloseWatcher` as ONE agentic tick** (implicit landmark role in the a11y
tree that `manuk-agent` consumes; the dismissal actuator). Non-negotiable this window. (2) **implied/`auto` grid
track sizing** — the t566 root cause, RED proof committed
(`tests/wpt/probes/grid-implied-tracks.html`, 88/133 → ~289/291), Taffy-side and worth a fresh context. (3) the
same-face `{Open Sans/13}` metric delta (variable-font variant vs hinting). (4) the t556 cascade-origin bug
(author `* { margin:0 }` losing to the UA `body` margin). (5) the crawl-side `.SIG` correction. **Do NOT** open
an XL subsystem: the instrument is still returning root causes at a tick apiece, and that is the best rate this
loop has ever had.

**Next check due: tick 575.**

## Check #38 — tick 575

**HORIZON: H0 — Pareto Web Parity.** EXIT GATE (all binary): ~83% WPT across categories ·
differential-oracle-verified viability across all four usage-weighted corpora · the headful shell
daily-drivable by its own developer · every rendered construct queryable through the in-process semantic API.

**GATE OR SCOREBOARD? — GATE, and for the first window in a long time nothing is outstanding.** `WPT:TOTAL`
is *still* flat — thirty-two ticks now — while t567–t574 produced: the `<search>` landmark and `CloseWatcher`
(**I3 discharged**, after Checks #35/#36/#37 each flagged it), `justify-content: normal` (one enum conflation
that had disabled grid track stretching in *every grid we have ever laid out*), `repeat(auto-fill)` resolved
by the wrong subsystem, the **100-tab RSS benchmark run for the first time** (STEP 1(c) of the observer's exit
verification, and it found eviction returning 0% to the kernel), and two quadratic cascade loops. STEP 1 of
the exit verification is now COMPLETE — test262 (t546), the fresh instrument sweep (t531–540), and the RSS
benchmark (t571) have all been run. Condition #2 keeps moving because the instruments keep getting sharper.

**INVARIANTS.** I4 (Pareto): held — every tick this window was corpus- or instrument-driven. **I3: DISCHARGED
and closed**, three checks after it was first flagged; the residue (`AccessKit` platform bridge) is VI.1's
named gap, not drift. I2 (never patch dependencies): held, and this tick is the interesting case — it changes
the `Origin` we *declare* to Stylo and adds a sort term to **our own** matcher; nothing in Stylo is patched,
and the fork surface in `STATUS.md` stays empty.

**PART VI CORRECTION — and it is a correction to the discovery MODEL, not just to a row.**

- **VI.2's `I5` row is ~200 ticks stale.** It reads *"the differential oracle has never completed a full
  crawl (`ORACLE_CRAWLED: 0` of 265)"*. It has crawled 265 sites, 392 clusters, and has been rebuilt
  (t531–t540) into a SHAPE-scoring, selector-path-keyed, root-cause-clustering instrument. Replace the row:
  the oracle is **operational and is the loop's highest-yield input**.
- **VI.2's `H0.1` row is stale in its numbers** (`css-flexbox 5.5%`, `css-grid 4.7%`) and, more importantly,
  in its *frame*. Those are WPT area percentages, and this tick is the counter-example that matters:
  **the largest-blast-radius CSS defect on the board — every CSS reset on the web silently not applying —
  was found as a side-observation of a FONT probe (t556), and no WPT area ranking would ever have surfaced
  it.** A ranking over area percentages measures where the *tests* are, not where the *web* is.
- **So VI.4's ordering is corrected at item 2.** "CSS layout breadth, in usage order, ranked by WPT area" is
  demoted; **"whatever a real page, run for real, is measured to be doing wrong"** is promoted. That is not a
  new principle — it *is* I5, and VI.4 had quietly re-expressed I5's job as a spec table of contents. The
  loop's own record over the last ~30 ticks is unambiguous: the wins came from running test262, running the
  RSS benchmark, running the corpus sweep, and running one probe page against live Chromium.

**AN INSTRUMENT FINDING THAT OUTRANKS THIS TICK'S CAPABILITY, and it was found by accident.**
Because tick 575 touches the cascade, it ran the WHOLE `manuk-page` suite under the shipping feature set
instead of the wall's subset: **279 test binaries green, 3 red, all three verified pre-existing.** One is a
`CSS.supports()` lie (`view-transition-name` answers **yes** for a property we do not implement — the
`layout.unimplemented` pref's own comment claims it "changes nothing we read", and `CSS.supports()` reads it).
One is a real rendering failure on a page we author (the honest interstitial renders without its `<h1>`).
One — `g_exec_command_copy` asserting `queryCommandSupported('bold') === false` — was written at tick 463 as
an honest "no" and has been **stale-red since tick 481**, when bold landed: ninety-four ticks.

**The structural half is the part that matters to this document.** `verify.sh` launches ~19 named page-gate
binaries of ~280 that exist, and runs the crate suites without `--features stylo`. So *"the wall is green"*
and *"the tree is green"* are different statements, and the gap is wide enough to hide a ninety-four-tick
staleness. The H0 exit gate's third condition is *"the headful shell daily-drivable by its own developer"* —
a claim that cannot be made from a wall that watches 7% of the assertions the repo contains. Test selection
lives in `scripts/`, observer-owned per Part VII, so this is **reported, not fixed**; the loop-side half is
to stop treating a green wall as a green tree.

**STEER, in order.** (1) **Clear the three unwatched reds**, cheapest first — the `qs-bold` assertion follows
its capability (one line, plus an audit of every other honest-"no" written before its capability arrived); the
interstitial's missing `<h1>`; then `CSS.supports()` vs `layout.unimplemented` per property. Where a fix needs
watching, gate it from `manuk-page` (agent-editable) rather than reaching into `scripts/`.
(2) **The `apply_has_rules` per-element hoist** — the third instance of
t572/t573's *work-that-depends-only-on-the-stylesheet done per element* defect; `:has()` is 505 ms of the
2,570 ms cascade. (3) The same-face `{Open Sans/13}` metric delta. (4) The crawl-side `.SIG` correction —
`run_oracle_cmd` still carries the defect t550 fixed in the exit gate, in the instrument this document calls
its primary discovery mechanism. (5) A fresh corpus sweep, since t569's grid-stretch fix and this tick's
cascade-origin fix are both page-wide geometry changes and the sweep is differenceable. **Do NOT** open an XL
subsystem: the instrument is still returning root causes at roughly a tick apiece.

**Next check due: tick 583.**

## Check #39 — tick 583

**HORIZON: H0 — Pareto Web Parity.** But the EXIT GATE has been **replaced** since #38, and that is the
whole content of this check. The observer's tick-581 orders make `docs/loop/DAILY-DRIVER-CERTIFICATION.md`
the authority, superseding the tick-543 exit-verification orders: the certificate is now
**daily-driver-pass(site) = renders(site) ∧ functions(site)** over a 400-site CrUX-sampled corpus with a
**fixed denominator**, and it composes a RENDER layer (SHAPE + the four jarring invariants) with a new
FUNCTION layer (the capabilities each site actually touches, exercised per-site via BiDi).

**GATE OR SCOREBOARD? — GATE, and this window is the sharpest instance yet, because the loop's own numbers
were the thing found wanting.** The observer's stated reason for the redesign is that *"the fidelity RIG
can flatter itself (six false numbers in one session)"* and that the 265-site corpus is *"a convenience
sample with timeouts dropped"*. That is not a scoreboard complaint — it says the exit gate itself was
unsound. Ticks 574–582 land on the right side of it: t575 (cascade origin), t576 (`@supports` honesty),
t577/578 (machine-facing text), t580 (`:has()` cost), t582 (responsive images) are all Layer-C-render or
honest-failure work, which the orders explicitly say **remain valid and must not stop**.

**THE PART THAT MATTERS MOST, and it is a correction to my own recent ticks.** t578's audit found the map
had never read its own gate corpus; t581 drove that to zero. Both were *instrument-fidelity* work and both
were right — but the observer's #1 is a level deeper and I had not asked it: **is each certificate term
capable of going red at all?** The map being complete says nothing about whether the number it feeds is
falsifiable. `falsify.sh` exists because `G_LOAD` — a Bar 0 gate — had never tested what it was named for;
the certificate is now the single claim the Phase-0 exit rests on and had never been asked the same
question. This tick answers it for all six terms.

**INVARIANTS.** I4 (Pareto): held, and strengthened — the new corpus is explicitly usage-weighted (CrUX
HEAD traffic-weighted + TAIL uniform), which is I4 expressed as a sampling frame rather than a
disposition. I5 (the oracle is the discovery engine): held, and this tick is I5 *turned on itself* —
the discovery engine is now subject to the same falsification discipline it applies to the browser. I3:
discharged at t574 and not re-opened. I2: held; nothing vendored, nothing patched.

**PART VI CORRECTION.** §VI.4's ordering is now downstream of the certification redesign and should say so:
its item 3 (*"get the oracle to finish one clean crawl"*) is not merely done, it has been **superseded** —
a clean crawl of a convenience sample is exactly the reading the redesign rejects. Replace it with the
observer's strict order: **falsify the certificate → adopt the fixed-denominator corpus → compose FUNCTION
→ reconciliation gate.** The rest of VI.4 (usage-weighted breadth, semantic model in lockstep) is unchanged
and correct.

**A SECOND FINDING, and it is the same class as the certificate's.** Getting the falsification to run
required fixing **two pre-existing compile errors** in `manuk-wpt`'s own test build: `Seen` gained a `font`
field at t563 and one test constructor was never updated, and a prose formula in a `///` comment was being
compiled as a doctest. **`cargo test -p manuk-wpt` had not compiled for ~20 ticks** — in the crate that
holds the fidelity instrument, the oracle and the bench. This is the `paint-enum-field-breaks-wpt-bench`
lesson repeating, and it belongs beside t578's finding: the wall does not build this crate's tests either,
so *"the wall is green"* and *"the instrument compiles"* were also different statements.

**STEER — the observer's order, unchanged, with the residue named.** (1) ✅ **falsify the certificate** —
done this tick for all six RENDER terms; **the FUNCTION terms do not exist yet and cannot be falsified
until they do**, so this guard must be re-run as each is added, and the gate asserts a term COUNT so a
seventh term cannot appear unfalsified. (2) **adopt `docs/bench/corpus-v2.tsv`** with the fixed
denominator — a timeout/crash/bot-wall is a COUNTED FAIL, never a silent drop. (3) **compose FUNCTION**,
starting with the throw-class killers (IndexedDB, the observer trio). (4) **the reconciliation gate**
(`sampled == scored + FAIL + EXCLUDED`). **Do NOT** stop the render-fidelity fixes; they move Layer C of
the new certificate and the orders say so explicitly.

**Next check due: tick 591.**

## Check #40 — tick 591

**HORIZON: H0 — Pareto Web Parity**, under the tick-581 certification redesign
(`docs/loop/DAILY-DRIVER-CERTIFICATION.md`): **daily-driver-pass(site) = renders(site) ∧ functions(site)**
over a fixed-denominator 400-site corpus.

**GATE OR SCOREBOARD? — GATE, and the window is unusually legible because the observer's CO-#1 was a
numbered list.** Items (1) falsify the certificate, (2) fixed-denominator corpus, (3) compose FUNCTION,
(4) reconciliation: **all four landed** (t583–t586), each verify-gated and RED-proven. Then t587–t591 went
back to capability on the strength of what the instruments found. `WPT:TOTAL` remains flat and remains
irrelevant, exactly as #37 and #38 argued.

**THE INVARIANT THAT MOVED THIS WINDOW IS I5, AND IT MOVED IN AN UNUSUAL DIRECTION.** I5 names the
differential oracle as the discovery engine. This window it was **turned on itself** — t583 proved every
certificate term can go red; t584 made a dropped site impossible rather than discouraged — and then, twice,
**the instrument found engine defects that no conformance test would have**: t586's capability probe could
not wrap `localStorage` (→ t587: assignment silently discarded), and t591's `filter` probe found
`CSS.supports` answering **yes** for a property we do not render. **Building the measuring tool out of the
same primitives the web uses is what made both visible.** That belongs in the constitution's reading of I5:
the oracle is not only a corpus differ, it is anything that exercises the engine the way a page does.

**PART VI CORRECTION — §VI.3's north star needs a second clause.** VI.3 says: optimise *usage-weighted
breadth*, not subtest count. Correct, and this window proved it incomplete. t588 pulled the Blink use
counters and found the map ranking by standards-roadmap; t590 then took the resulting **#1 by usage
(`appearance`, 60.5% of page loads)**, measured it, and found it a **no-op for this engine** — our form
controls are UA CSS, not native widgets, so the effect authors want is already achieved by the cascade.
**A tick that "implemented appearance" would have been theatre, and the usage number would have justified
it.** So VI.3 becomes:

> **Rank by usage-weighted breadth — then price each candidate by measuring it IN THIS ENGINE.** A use
> counter measures what pages *ask for*; what it costs *us* depends on what we would otherwise do.
> *A capability's name is not its shape* (#588) and *a capability's usage is not its impact* (#590).

**INVARIANTS.** I4 (Pareto): held and sharpened by the above. I2: held — nothing vendored or patched; the
`UNRENDERED_LONGHANDS` list added this tick is our own denylist, not a Stylo edit. I3: discharged at t574
and not reopened; the FUNCTION leg (t585/t586) is arguably I3 work in the certificate's clothing.

**A DEFECT CLASS THIS CHECK WANTS ON RECORD, because it has now recurred at three widths.** t576 fixed
`@supports` lying for the 35 `layout.unimplemented` properties. t591 found the identical lie for properties
Stylo parses **natively, behind no pref at all** — `filter` (51.9% of page loads), `clip-path`,
`mix-blend-mode`, `writing-mode`. The 2026-07 pattern is: **a fix scoped to the shape the bug presented in
is one category too narrow.** t578 (three text-assembly consumers, not one), t581 (seven gate directories,
not one), t588's own standing rule (which had the blind spot it was written to cure) are the same shape.
**When a defect is found, ask what class it is an instance of before scoping the fix** — and the cheap
version of that question is *"what else reads this / is in this state, and does it have the same problem?"*

**STEER.** The observer's CO-#1 list is complete, so the next ordering comes from t588's usage ranking as
corrected by t590's pricing rule: (1) **`filter`** — 51.9%, and unlike `appearance` its impact **does**
transfer, because there is no cascade-level workaround for a blur; a real paint subsystem and the first in
a while. (2) `font-display` / `unicode-range` — cheap, and adjacent to the t557/t558 font arc that was this
project's largest fidelity win. (3) `clip-path` and `mix-blend-mode`, each **measured first** per VI.3's new
clause. (4) The `interpolate-size` SIGSEGV, re-priced from exotic to one-page-load-in-twelve. Standing:
the certificate's FUNCTION terms are falsified as they are added, which `G_CERT_FALSIFIABLE`'s term-count
assertion enforces mechanically.

**Next check due: tick 599.**

## Check #41 — tick 599

**Horizon:** H0 — the Phase-0 daily-driver certificate (`docs/loop/DAILY-DRIVER-CERTIFICATION.md`,
the authority since t581).

**Is the hill the mountain?** This window (t592-599) spent six ticks on ONE bundle — the
visual-effects properties and their CSSOM half — plus the due surface audit and this check. That
concentration is worth interrogating, because "six ticks on CSS" is exactly the shape of the
death-tail PART VI warns about.

**It holds, and the reason is measurable rather than aesthetic.** Every property in the bundle was
selected by the Blink use counters, not by what was tractable: `filter` 51.9%, `clip-path` 43.8%,
`backdrop-filter` 34.3%, `mix-blend-mode` 12.9%. All four were in the state §VI.3's second clause
exists to catch — *parsed, computed, and never read* — which is the specific failure that makes a
capability invisible to a capability count while being visible to every user. And the arc converged:
each successive property cost less because the first one paid for the offscreen group. That is the
opposite of a tail.

**§VI.3 gains a third clause, from t598's audit.** The rule so far reads *rank by usage-weighted
breadth, then price each candidate by measuring it IN THIS ENGINE*. The audit found the loop's own
board still steering at three CO-#1 items that were **already built** (MEDIA's MSE/demux/playback/
captions, OAuth's redirect/popup/postMessage, canvas `fillText` — confirmed by running its gate).
So: **before taking a named priority, verify it is still unmet.** A stale priority is not a small
waste; it is a tick spent re-deriving something the ratchet already banked, and the loop cannot see
it because the board reads like an instruction rather than a claim. The self-audit checks whether the
loop obeys its process; nothing was checking whether the process's *inputs* were still true.

**I5, and the direction it moved this window.** t591 recorded that turning the instrument on itself
found engine defects no conformance test would. That continued and sharpened: t597's probe of 95
properties found **86** returning `undefined` where t596 had fixed four, and t598's audit produced
**two wrong numbers of its own** (27, then 18, against a truth of 2) before the third pass. Both are
the same lesson from opposite ends — **measure the population before you claim the fix, and suspect
the instrument before the subject.** The loop is now generating this correction itself rather than
being handed it, which is the healthiest form of I5 available.

**PART VII / V1-SCOPE — one drift, corrected this tick.** Two SECURITY rows were carried as
`unknown`: `X-Frame-Options`/`frame-ancestors` and Subresource Integrity. For a security control
`unknown` is the worst available status — nobody can rely on it and nobody is alarmed — and one of
them (`frame-ancestors`) was documented as unimplemented *in the source's own header* while the map
said unknown. Both are now measured. SRI is **built and gated** (t599); XFO is honestly `missing`
with its enforcement point named. **A security capability may be absent, but it may not be unknown.**

**No drift found in the north star.** Chromium remains the ceiling on capability and the floor on
everything else; nothing this window traded performance or honesty for a feature, and three of the
six capability ticks *removed* a lie (`@supports` answering yes about `filter`, `clip-path`,
`backdrop-filter`) rather than adding a rendering.

## Check #42 — tick 607

**Horizon:** H0 — the Phase-0 daily-driver certificate (`docs/loop/DAILY-DRIVER-CERTIFICATION.md`,
the authority since t581). Its gate is `daily-driver-pass(site) = renders(site) ∧ functions(site)`
over the fixed-denominator corpus-v2, **not** a WPT percentage (PART VII.1).

**Gate or scoreboard?** t600-607 is eight ticks: anti-framing (600), the map-honesty probe (601),
unknowns→0 (602), foreign-content namespace (603), self-audit + `getBBox()` (604), `isolation`
measured-and-**declined** (605), THE PILOT (606), error documents (607). **Gate, and unusually
cleanly** — 606 produced the exit-gate number itself, 607 removed a defect that number surfaced, and
605 is the rarest shape on this board: a capability *priced and refused*, which only a real gate
lets you do without guilt.

**THE FINDING OF THIS WINDOW, AND IT IS ABOUT WHERE BUGS LIVE.** The certificate was built to measure
*rendering*. Its first live run found a defect **one layer below rendering**: five of twenty HEAD
sites answered `403` with a real body and the engine refused to fetch them, so a quarter of the
representative head was invisible to the exit measurement *for a reason that had nothing to do with
layout*. I5 says the differential oracle is the discovery engine; the sharpened version is
**an instrument finds bugs in everything it must traverse to measure, not only in what it measures.**
t606's *other* unrequested finding — 10 of 14 sites tripped OURS-IS-SLOW — is the same shape again,
and the observer's t602 board steer has already promoted it: **performance is now a fidelity input**,
because a page painted incomplete at the load budget scores as a layout failure. That is a genuine
correction to how this loop reads its own number, and it came from the instrument, not from a plan.

**§VI.3 GAINS A FOURTH CLAUSE, AND IT IS EARNED BY THREE INSTANCES.** The rule so far: rank by
usage-weighted breadth → price by measuring IN THIS ENGINE (t590) → before taking a named priority,
verify it is still unmet (t598). Add: **when a defect is found, ask whether the rule it violates is
implemented MORE THAN ONCE — and whether the copies agree.** Booked three times now, each time with
the *correct* implementation already present somewhere in the tree: two cascades (Stylo `UA_CSS` vs
`apply_ua_defaults`, and the live one went stale); two structural probes (`[id]` vs selector-path,
and the stale one kept 39% of the corpus unmeasurable); and now two navigation paths — GET `bail!`ing
on `status >= 400` while POST carried the correct rule **in a comment eight hundred lines away**
(*"a 4xx/5xx still has a body worth showing … matching a real browser"*). This is strictly cheaper
than t591's "grep for the CLASS, not the symptom", because the answer is usually already written
down: the work is reconciliation, not derivation.

**INVARIANTS.** I2: held — nothing vendored or patched. I4 (Pareto): held; t605 is the proof, since
declining `isolation` on measured impact is exactly what I4 asks for and is the hardest form of it to
actually do. I3: t607 is a network/navigation tick with no new rendered construct, so no semantic-model
exposure was owed. **I5: strengthened, per the finding above.**

**PART VI IS CORRECTED, and one row was years stale.** VI.2 still reads *"the differential oracle has
**never completed a full crawl** (`ORACLE_CRAWLED: 0` of 265)… the constitution's primary discovery
mechanism is not operational."* That has been false since roughly t380: 265/265 crawled, 392 clusters,
and the mechanism has since been *replaced* by a better one (the corpus-v2 certificate, t581+). The
real H0 blocker is no longer "get the oracle to finish" and it is no longer capability count either —
**it is MEASURABILITY**: 6 of 20 HEAD sites unfetchable (t607 closes the dominant class) and 9 of 14
unscoreable, against which p̂ = 0/5 cannot size anything. VI.2's flexbox/grid percentages are also
stale as a *steer*, though not as facts — PART VII.1 already demoted WPT from gate to diagnostic, so
they no longer name the blocker.

**STEER — and the next tick is already measured rather than guessed.** t607's live confirmation run
handed over the mechanism behind the OURS-IS-SLOW finding: `mangago.me` returned **200** and took
**174 seconds**, pulling ~hundreds of images that each timed out at the 8.0s subresource deadline
with **nothing bounding the aggregate**, plus `event loop hit its task ceiling` at 20,000 tasks. The
per-request deadline works; there is no total load budget above it. That single number is both a
Bar-0-class hang on our own clock **and** the fidelity input the board just promoted, which makes it
the unambiguous next tick. After it: the pilot's two remaining measurability classes — the
202-with-empty-body site (imdb) and the three connection failures — then re-run the pilot before
sizing any full sweep.

**No drift found in the north star.** Chromium remains the ceiling on capability and the floor on
everything else. ⚠ One thing to watch, named here so it cannot become folklore: **the load-budget work
now has a motive to make a number go up**, and "fast because we never loaded the images" is the exact
trap PART VII/the North Star names. Any load-budget tick must move fidelity and latency *together*,
or it is measuring what we stopped doing.

**Next check due: tick 615.**

---

## Check #43 — tick 615

**Horizon:** H0 — the Phase-0 daily-driver certificate (`docs/loop/DAILY-DRIVER-CERTIFICATION.md`,
the authority since t581). Gate: `daily-driver-pass(site) = renders(site) ∧ functions(site)` over the
fixed-denominator corpus-v2 — **not** a WPT percentage (PART VII.1).

**Gate or scoreboard?** t608-615 is eight ticks: map-reconcile self-test (608), interface objects
(609), the drain's clock bound (610), the certificate's fetch reasons (611), `innerText`'s setter
(612), XHR as an EventTarget (613), the oracle's shell (614), and this check plus
`HTMLScriptElement.supports` (615). **Gate.** Five of the eight came from *running the certificate and
reading what it said*, which is the shape §VI.4 asks for, and three of those (612/613/615) are the
same chain being peeled one rung at a time on one real site.

**§VI.3'S FOURTH CLAUSE IS NOT A PATTERN ANY MORE, IT IS THE DOMINANT DEFECT.** Check #42 added it on
three instances — *when a defect is found, ask whether the rule it violates is implemented MORE THAN
ONCE, and whether the copies agree.* This window fired it **five more times, consecutively**:

| tick | one rule | two implementations | the one that was wrong |
|---|---|---|---|
| 610 | the event-loop drain is bounded | `run_deferred` · `run_with_fetcher` | the second had **no bound at all** |
| 611 | a site that cannot be reached is a counted row | 4 separate `continue`s in one loop | three of the four dropped it silently |
| 612 | rendered text is settable | `outerText` · `innerText` | `innerText` had **no setter** |
| 613 | an XHR event is dispatched | streaming path · buffered path | buffered never fired `loadend` |
| 614 | a placement ratio needs a real sample | `certificate()` · the printed MEAN | the MEAN averaged in vacuous rows |

**Eight consecutive ticks, eight instances.** That is no longer "a thing to check"; it is the modal
bug in this codebase, and it has a common cause worth naming: **this engine grew by adding a second
path beside a working one** — a streaming delivery beside a buffered one, a prelude object beside a
Rust binding, a certificate function beside a report function — and the second path is written by
someone who has just read the *behaviour* rather than the *rule*. The remedy that actually worked five
times running is mechanical rather than attentional: **when you fix one, grep for the other and route
both through one function**, so there is no longer a place to write the rule down twice. t613 is the
clean form — six open-coded dispatch sites collapsed into one `__xhrFire`.

**AND A NEW CLAUSE, EARNED THREE TIMES IN ONE SESSION — §VI.3.5: BEFORE BELIEVING A MEASUREMENT,
ESTABLISH WHETHER IT IS A PROPERTY OF THE ENGINE OR OF THE INSTRUMENT.**

1. **t610 (the wall).** Four consistent readings of 544-752s against a mark of 189 produced a written
   request that the observer *re-baseline the ratchet mark*. All four were one artifact: an
   `engine/js` edit forces a ~10-minute release LTO relink **inside the gate phase**. Disproof cost
   one command (`cargo build -p manuk-wpt --release` → 0.45s warm); the same wall then ran **64s**.
   **A false regression report aimed at a RATCHET MARK is worse than one aimed at code, because the
   remedy it requests is permanently loosening the thing that catches regressions.**
2. **t613 (the getter-only list).** t612 published a residue list of "spec-settable properties that
   throw here", derived by **grepping** a registration macro. Probing found `nodeValue` already
   writable from a setter on another prototype. The list described the grep, not the engine.
3. **t614 (the shell).** `comix.to` scored `coverage 66.7%` — over **three elements**, because the
   oracle's own `file://` reference cannot hydrate a JS-built page (28 elements vs ~2643 live). The
   number measured the instrument's reach, not the engine's.

Each was one command away from disproof, and in each case the *reflex* was to believe the number
because the number was large, or consistent, or came from code I had just written. The clause:
**a measurement is a claim about a SYSTEM — engine plus harness plus corpus — and the attribution is
part of the result.** This is Lesson 4 in STATUS ("every number has a harness") generalised from
*timing* to *every* metric, which is where it was always heading.

**INVARIANTS.** I2: held — nothing vendored or patched; 612/613/615 are all our own JS/binding code.
I4 (Pareto): held, and this window is unusually disciplined about it — t613 *measured* `.classList =`,
`document.body =` and `.selectionDirection =` at **zero corpus usage** and declined to build them, and
found `.style =`'s 15 apparent hits were all React props objects. Declining on measured impact is the
same shape as t605's `isolation`. I3: no new rendered construct, so no semantic-model exposure owed.
I5: **strengthened again** — every capability tick this window (612/613/615) was found by the
certificate traversing a real site, not by a plan; check #42's *"an instrument finds bugs in
everything it must traverse"* is now the loop's main discovery channel.

**PART VII / V1-SCOPE: held.** Every tick is rendering parity or the agentic surface. The three
measurement ticks (611/614 and half of 615) are the certificate itself, which PART VII.1 names as the
gate, not as tooling.

**⚠ STANDING ITEM FOR THE OBSERVER, UNCHANGED SINCE t611:** `manuk-wpt`'s tests are **not in the
wall** — `verify.sh`'s `_crate_suite` lists seven crates and `manuk-wpt` is not one, with no `_launch`
line either. `G_CERT_FALSIFIABLE`, whose own doc comment reads *"this is the proof, re-run on every
wall"*, has never run on one; nor has the vacuous-pass guard, nor `G_UNMEASURABLE_REASON`. They are
green (`cargo test -p manuk-wpt` → 54 passed in **14s**) and the RATCHET's `GATES` count has been
counting them as coverage. This is `[[gates-not-in-the-wall]]` applied to the certificate itself.
`scripts/` is observer-owned; one line closes it.

**Next check due: tick 623.**

---

## Check #44 — tick 623

**Horizon:** H0 — the Phase-0 daily-driver certificate (`docs/loop/DAILY-DRIVER-CERTIFICATION.md`).
Gate: `daily-driver-pass(site) = renders(site) ∧ functions(site)` over the fixed-denominator
corpus-v2 — not a WPT percentage (PART VII.1).

**Gate or scoreboard?** t616-623 is eight ticks: error subresources (616), module base URLs (617),
surface audit #34 (618), web fonts (619), dynamic `import()` parked (620), measure-and-pin (621), a
correction (622), and this check. **Gate** — and the window's shape is unusual and worth naming:
**four of the eight produced no capability at all.** One audit, one parked negative result, one
pinning pass and one retraction. That is not drift; it is what happens when an instrument is finally
pointed at the map, and PART VII.1's *"the certificate IS the gate"* makes those ticks the main line
rather than overhead.

**THE WINDOW'S HEADLINE IS t617, AND IT IS THE ONE CAPABILITY RESULT THAT MATTERS.** `www.welt.de`
went **COVERAGE 0.0% → 94.9%**, blank white page → rendered front page, because an external
`<script type=module>` resolved its relative imports against the DOCUMENT instead of against itself.
That is the shape of **every** Vite/Rollup/esbuild build, and it was invisible to every test whose
modules sit beside the document — which is every tutorial, example and dev server. **A rule whose two
cases coincide in the common configuration will be written for whichever case the author had in front
of them, and the comment will state the general rule correctly while the code implements the special
one.** Both halves of that bug sat under comments stating the rule correctly.

**§VI.3.5 IS EARNED AGAIN AND NEEDS A SECOND CLAUSE.** Check #43 added *"before believing a
measurement, establish whether it is a property of the ENGINE or of the INSTRUMENT."* This window
fired it **four more times**: audit #34's "262 unreferenced gate files" (truth: 6) and its dangling
`G_CAP_TOUCH_PROBE` (which exists), t619's web-font blast radius (predicted broad, measured zero
sites), and t622's `ResizeObserver` (published as an inert stub; **it fires**).

The second clause, from t622 and cheap to apply:

> **Before publishing an ABSENCE, name the code path that WOULD deliver it and show that path ran.**
> If you cannot name it, you have measured your harness, not the capability.

**And the asymmetry underneath it is the real finding: a negative result feels like it needs no
confirmation.** `fired:0` was accepted at face value; a surprising *positive* would have been re-run
twice. Every false claim this window — the wall at t610, the getter-only list at t613, the oracle
shell at t614, RO at t622 — was a NEGATIVE accepted cheaply. **The loop is systematically
under-sceptical of bad news about itself**, which is the opposite of the failure mode this project
was built expecting, and worth more attention than any single row.

**§VI.3's fourth clause (one rule, N implementations) ran to NINE consecutive ticks** before pausing —
610 through 617, plus t616's variant where **all six copies were wrong the same way**, which teaches
that agreement between copies is not evidence the rule is present.

**INVARIANTS.** I2: held — nothing vendored or patched. I4 (Pareto): held, and unusually well
evidenced: t613 measured `.classList =`/`document.body =`/`.selectionDirection =` at **zero** corpus
usage and declined to build them; t619 measured its own fix's blast radius at **zero sites** and said
so in the commit rather than claiming the class. I3: no new rendered construct owed exposure. I5:
strengthened — every capability tick here came from the certificate traversing a real site.

**PART VII / V1-SCOPE: held.** No stealth/fingerprint work was taken even though **5 of 20 HEAD sites
bot-wall us**, which is the single largest measured blocker on the corpus and explicitly out of scope.

**⚠ STANDING FOR THE OBSERVER, unchanged since t611 and now with a second item:**
1. `manuk-wpt`'s tests are **not in the wall** (`_crate_suite` lists seven crates; `manuk-wpt` is not
   one). `G_CERT_FALSIFIABLE`, whose doc comment says *"re-run on every wall"*, has never run on one.
   54 tests, **14s**.
2. `map-reconcile.sh` scans only `engine/page/tests/`, so a real gate under `tests/wpt/tests/` reads
   as dangling. That single false positive is the whole of the remaining drift and is what blocks
   `--strict`.

**Next check due: tick 631.**

---

## Check #45 — tick 631

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate: `daily-driver-pass(site) = renders(site)
∧ functions(site)` over the fixed-denominator corpus-v2, not a WPT percentage (PART VII.1).

**Gate or scoreboard?** t624-631 is eight ticks: dynamic `import()` (624), the sweep's subprocess
deadline (625), the certificate re-measured (626), the oracle's font field (627), surface audit #35
(628), the SVG measurement (629), `<path>` bbox (630), this check. **Gate** — and for the first time
this session the window contains a *complete* exit measurement rather than a partial one, which is
what t625 existed to make possible.

**THE NUMBER, and it is the one PART VII.1 says we are scored on:**
```text
  sites 20 · scored 6 · shape >=0.75 on 0 (bar 95%) · COVERAGE 85.2% · SHAPE 40.0% · VISUAL 66.0%
```

**AND THE BINDING CONSTRAINT HAS CHANGED CLASS, WHICH IS THIS CHECK'S FINDING.** t606 named it
measurability. That is now largely closed — **13 of 14 unscored sites carry a named cause**. What
replaces it is not a measurement problem and not a rendering problem:

> **5 of 20 HEAD sites bot-wall us, and `V1-SCOPE` puts bot-detection explicitly OUT OF SCOPE.**

The achievable ceiling on this corpus is **15/20, not 20/20**. That is not an argument to change the
denominator — a fixed denominator is exactly what stops the hard sites vanishing — but the constitution
should say plainly that a quarter of the representative HEAD is unreachable **by a decision this
project made deliberately**, or it reads forever as unfinished work. **PART VII should carry that
sentence.** After it, what remains is **placement, not presence**: coverage 85.2% against shape 40.0%,
shape ≥0.75 on *zero* of the six scoreable sites.

**§VI.3.5 GAINS A THIRD CLAUSE, and it is the inverse of a rule we already had.** t630 found
`<path>`'s `0×0` bbox was a **deliberate** honest refusal — the gate's own doc said *"on purpose"* —
made when no exact answer was available. It stayed after one became available, and for 26 ticks the
commonest SVG element reported no geometry *because a correct decision was never re-priced*.

> **An honest "we cannot know" must be re-priced when the ability to know arrives.**
> `[[honest-answer-is-not-a-fixed-answer]]` covers the case where a "no" becomes a lie once the
> CAPABILITY lands. This is the mirror: a "no" becomes a ceiling once the ANSWER becomes computable.
> Both rot; the second rots invisibly, because it is documented as intentional.

**THE SESSION'S ASYMMETRY, now with a fourth instance and a count.** Check #44 named it: *a negative
result feels like it needs no confirmation.* Final tally across t610-631 — **four false absences**
(the "bricked" wall, ResizeObserver, scroll anchoring + forced reflow, and `<path>`'s outlived
refusal) against **zero false presences of my own making**. Every one was a *negative* accepted at a
price a positive would never have been sold at. That is not a run of bad luck; it is a systematic
mis-calibration, and it is now measured three separate ways (check #44, audit #35, this).

**A FOURTH PROBE-DISCIPLINE CLAUSE, cheap and earned at t630:** a NEGATIVE assertion needs a RED probe
that produces **a different wrong answer**, not a differently-shaped absence. The first probe for the
arc refusal guessed `(0,0)` — indistinguishable from "no answer" — and the gate stayed green.

**INVARIANTS.** I2: held. I4 (Pareto): held and repeatedly evidenced — t625's deadline set generously
because *a deadline is not a latency budget*; t630 declining to bound arcs rather than guessing. I3: no
new rendered construct owed exposure. I5: strengthened — every capability tick in this window
(624/630) descended from the certificate traversing a real site.

**PART VII / V1-SCOPE: held**, and now load-bearing: the largest single measured blocker on the corpus
is one the scope forbids touching, and it was not touched.

**⚠ STANDING FOR THE OBSERVER — one closed, two open:**
1. ✅ `map-reconcile.sh` now searches `tests/` (fixed by the observer at `f9882e98`). Drift is 0.
2. `manuk-wpt`'s tests are **still not in the wall** — 54 tests, **14s**, and `G_CERT_FALSIFIABLE`'s own
   doc comment says *"re-run on every wall"*.
3. **NEW (wall audit #18):** the wall is **67s warm and 512-716s on any tick touching `engine/`** — a
   51MB `manuk-wpt` under `lto = true, codegen-units = 1`, relinked inside the gate phase. A
   per-package `lto = "thin"` for the harness changes no assertion and drops no gate.

**Next check due: tick 639.**

## Check #46 — tick 639

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate: `daily-driver-pass(site) =
renders(site) ∧ functions(site)` over fixed-denominator corpus-v2 (PART VII.1).

**The window:** t632 (the load budget moves coverage and shape in opposite directions), t633 (WebM
demux), t634 (AV1-in-WebM), t635 (`mediaCapabilities.decodingInfo`), t636 (ESM top-level await,
measure-and-pin), t637 (the `ic` fix, declined), t638 (surface audit #36), t639 (this check).

**Gate or scoreboard? — SCOREBOARD, and naming it is this check's job.** Three of the four capability
ticks are media, which is the right *area*: PART VII.1 names depth on "the handful of sites where
people actually spend time," and WebM + AV1 + the rendition-scan API is the YouTube story. But
PART VII.1 does not rank by area. It ranks by **"real sites moved per fix, verified against the
oracle corpus."**

> **Every piece of evidence in t633-635 is a FIXTURE.** `bear-vp9-opus.webm`, `bear-av1-480x360.webm`,
> six contentType strings. Not one of the three ticks measured a real site, before or after. The
> claim *"this unblocks YouTube"* is **plausible**, and plausible is exactly the currency PART VII.1
> forbids ranking in.

This is not an argument that the ticks were wrong — a demuxer needs a fixture, and a fixture-gated
capability is a real ratchet tooth. It is an argument that **the window bought an unmeasured amount**,
and that the next media tick owes a corpus number rather than another rung. The honest form: I know
these capabilities *exist*; I do not know that any site renders differently because of them.

**§VII.1 EARNS A PROCEDURAL CLAUSE.** The rule already says rank by real-sites-moved. What the last
eight ticks show is that the rule is satisfiable *in intent* while producing no site measurement at
all, because each individual tick has a good local reason not to run the sweep (it costs ~45min, the
capability is obviously present, the fixture is more precise). Three good local reasons compound into
a class of work with no corpus evidence behind it.

> **A capability arc must produce ONE real-site measurement before it produces its fourth rung.**
> Not per tick — per arc. Otherwise "ranked by real sites moved" degrades into "ranked by which area
> real sites are in," which is a different and much weaker rule wearing the same words.

**THE WINDOW'S ACTUAL YIELD WAS INSTRUMENT WORK, and that is worth stating because it was not
planned.** Five separate findings, none about the engine:

| tick | the instrument was wrong |
|---|---|
| 634 | the wall's RED matched a **remembered false-RED** exactly; re-running the crate is the only reason a hole did not land |
| 635 | per-answer assertions **cannot see a second implementation**; only a runtime agreement check can |
| 636 | the TLA probe printed `-` — and so did the **no-await control**, so the harness was wrong, not the engine |
| 637 | the `ic` fix's gate **passes before the fix**, which makes the fix a claim rather than a win |
| 638 | eleven map rows were cited in a dialect **no instrument could read**; validated claims 259 → 271 |

That is five, in eight ticks, against ~three engine capabilities. §VI.3's *"suspect the instrument
before the subject"* has stopped being a caution and started being the base rate. **Meta-instrument
#3 (accounting reconciliation) predicted exactly this**: 8 of 30 process defects were caught by a
number that did not add up rather than by any gate, and audit #36 is that mechanism finding twelve
more.

**§VI.3.5 GAINS A FOURTH CLAUSE, and it is the constructive form of the previous three.** t622 gave
*name the code path that would deliver an absence*. t630 gave *a negative assertion needs a RED probe
producing a different wrong answer*. t633 gave *a positive assertion needs one too*. All three are
expensive enough to skip. t636 found the cheap version that subsumes them:

> **RUN THE CONTROL — re-run the same measurement with the feature under test REMOVED.** If it still
> fails, you were measuring the harness. Naming a code path is analysis and gets deferred; deleting
> the feature from your own fixture takes thirty seconds and cannot be rationalised away.

**NO SCOPE DRIFT AGAINST PART VII.1 OR PART IV.** Nothing in the window was justified by a post-v1
goal. VP9 stayed deliberately undecoded (observer t235). WebRTC answers an honest `false` as a
declared non-goal. EME was not touched. The `ic` fix was **declined rather than shipped**, which is
PART VII.1's scope discipline operating on a change I had already written — the first time in this
session's record that the rule cost something real.

**STANDING, UNMOVED:** t626's binding constraint (5 of 20 HEAD sites bot-wall us; the ceiling on that
corpus is 15/20 by a deliberate decision) and t632's (our own 17-28s latency against Chromium's 6s
contaminates the SHAPE half, and the load budget cannot be tuned to fix it — only to choose which
half to flatter). Neither moved this window, and neither was worked on.

## Check #47 — tick 648

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate:
`daily-driver-pass(site) = renders(site) ∧ functions(site)` over fixed-denominator corpus-v2
(PART VII.1).

**The window:** t640 (the media arc's owed real-code measurement), t641 (EME interfaces exist and
grant nothing), t642 (jQuery totally dead — `document.nodeType` was 8), t643 (DOMPurify returned the
empty string), t644 (XPath subset; htmx boots), t645 (the last four `?` cells), t646 (`playbackRate`
reaches the sound), t647 (SVG client rect), t648 (Opus re-taken → an unverified MSRV).

**Gate or scoreboard? — GATE, and it is the direct answer to check #46.** #46's finding was that
three media ticks were evidenced entirely by fixtures and that *"this unblocks YouTube"* was
plausible rather than measured. This window ran **real shipped third-party code** — 20 bundles across
t640/t642/t643/t644, ~4MB of production JavaScript — and every capability claim in it is backed by
what that code actually did. PART VII.1 ranks by *real sites moved per fix*; jQuery, DOMPurify and
htmx are not sites, but they are much closer to sites than a fixture is, and each was **silently,
totally dead** rather than degraded.

**AND THAT IS EXACTLY WHY THE CERTIFICATE IS NOW THE STALE NUMBER, WHICH IS THIS CHECK'S FINDING.**

> **Three libraries went from silently dead to working, and nobody has looked at what that did to
> the corpus.** jQuery alone is on a large fraction of the web. DOMPurify blanking every sanitized
> body is a *rendering* defect on any site with user content. The certificate was last measured at
> **t626 — 22 ticks ago** — and every fidelity number carried since then predates all three fixes.

This is #46's complaint one level up, and it survived #46's own remedy. That remedy — *"a capability
arc must produce ONE real-site measurement before its fourth rung"* — was **satisfied** here
(t640 for media, real bundles throughout t642-644), and the corpus number still did not move,
because the clause binds an *arc* and the certificate is not an arc. So §VII.1 gains its second
procedural clause, aimed at the gap the first one left:

> **A fix whose blast radius is argued in terms of "a large fraction of the web" owes a CORPUS
> measurement, not just a library one.** Per-library evidence establishes that a thing was broken and
> is now fixed. It says nothing about how many pages changed — and "a large fraction of the web" is a
> claim about pages.

**The instrument note, because it is falsifiable and dated.** The real-library method went **3-for-3**
on high-blast-radius finds (t640 shaka, t642 jQuery, t643 DOMPurify) and then **0-for-7** on its
fourth tier (marked, handlebars, mustache, moment, preact, d3, quill — all clean). An instrument
whose yield falls to zero is not broken; it is **saturated on the population it samples**. The
correct response is to change the population (server-rendered frameworks, consent/analytics tags,
Google Maps, Angular) or change the instrument — not to run a fifth tier of the same tier.

**NO SCOPE DRIFT AGAINST PART IV, AND ONE DECISION EXPLICITLY DECLINED.** The EME interfaces landed
on PART IV's own *"documented, degraded gracefully"* and grant nothing — Widevine, PlayReady and
Clear Key all refused. XPath refused rather than stubbed, on the reasoning that *"define the
interface, refuse the capability" is honest only where refusal is a valid answer*. And t648 **declined
to raise `rust-version`**, because replacing an unverified 1.80 with an unverified 1.85 is the same
defect with a newer number. That is PART VII.1's scope discipline operating three times, twice
against changes already in my hands.

**MEASUREMENT/CAPABILITY BALANCE:** five capability, four measurement. #46 noted the window before it
was inverted from the historical ~85% capability bias; this one is close to even, which is the shape
to hold.

**STANDING, UNMOVED AND UNWORKED:** t626's binding constraint (5 of 20 HEAD sites bot-wall us; the
ceiling on that corpus is 15/20 by deliberate decision) and t632's (our own 17-28s latency against
Chromium's 6s contaminates the SHAPE half). Neither moved. The wall (734s) remains harness-owned and
was reported at every audit rather than carried silently.

---

## Check #48 — tick 656

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate:
`daily-driver-pass(site) = renders(site) ∧ functions(site)` over fixed-denominator corpus-v2
(PART VII.1).

**The window:** t649–t656 — t650/651 (the cert scored a blank white page `ok`; a parked reproducer had
rotted), t652 (an injected script ran and no `load` was ever fired), t653 (a sweep re-opened the row it
had just closed), t654 (the stylesheet phase discarded nine fully-downloaded sheets on the load
deadline), t655 (`join_all` is one future — one stalled host discarded every image that arrived),
t656 (an image's intrinsic size erased by every re-cascade; `784×0`).

**Gate or scoreboard? — GATE on the mechanism, and DRIFTING on the measurement. That is this check's
finding, and it is check #47's finding again, one window later and worse.**

#47 said, in bold: *"the certificate is now the stale number."* It named a 22-tick gap and added a
procedural clause — *a fix argued in terms of "a large fraction of the web" owes a CORPUS
measurement.* The clause was correct and **it has now been violated three ticks running**, by me,
with the debt written into each tick's own NEXT and then not paid:

> **t654, t655 and t656 are three independent placement root causes, each measured on one page, landed
> against a SHAPE number nobody has re-measured.** t654 changed what a budget-exhausted site renders.
> t655 changed what it renders it *with*. t656 changed where every picture on it sits. The last real
> corpus reading is older than all three, and the last certificate is older still.

Each tick is individually well-founded — RED-proven gate, control run, named mechanism — and the *set*
is exactly the shape PART VII.1 exists to prevent: a run of plausible-blast-radius fixes with no
page-level number behind them. The per-tick evidence is honest; the aggregate claim ("placement
fidelity is improving") is currently **unmeasured**, and I have written "the sweep is owed" three
times instead of running it once. **Writing the debt down is not paying it.** A NEXT list is not an
instrument.

**No new clause.** #47's is the right rule and adding a fourth one would be the drift wearing process
as a costume. What is missing is not a rule; it is one sweep.

**THE STEER, and it is unambiguous: the next tick is the HEAD-20 SWEEP.** Not another mechanism, even
a well-surfaced one — and t656's own NEXT list has two live candidates (a failing ESM-scanner unit
test that is not in the wall; the two `join_all` fan-outs t655 deliberately left). Both wait. The
binding constraint named at t602 was never *"find more placement bugs"*; it was the **64%
unscored/failed rate** and the fact that perf contaminates SHAPE — and three ticks of load-deadline
work have gone straight at exactly that, which makes the sweep the measurement that says whether any
of it generalised. If it did not, that is the more valuable finding, and it is one I currently cannot
get.

**PART VI/VII still hold; no invariant bent.** The ratchet was honoured — t656 found a pre-existing
failing unit test while running the page suite and did **not** absorb it into the tick (the function
and its test are byte-identical to HEAD, 3,000 lines from the nearest hunk); it is recorded as its own
thread, along with the observation that it is **not in the wall**. t655 took its `COMMIT_RESERVE` out
of the load budget rather than adding it, explicitly refusing to buy a capability by extending a Bar 0
promise. t656 declined to smuggle in the natural-size finding that t655's control surfaced, and made
it its own tick with its own gate.

**MEASUREMENT/CAPABILITY BALANCE:** three capability (654/655/656), four reliability/measurement
(650/651/652/653) — even on its face, which is why the drift is *not* visible in the balance ratio.
The reliability ticks were about the **instrument**, and the capability ticks then ran ahead of the
instrument's last reading. Balance was the wrong thing to count.

**STANDING, UNMOVED:** t626's bot-wall ceiling (5 of 20 HEAD sites; 15/20 by deliberate decision) and
t632's latency contamination — the latter is precisely what t654/655 attacked, and the sweep is what
will say whether it moved. The wall is harness-owned; t655 ran at 252s warm and is reported, not
carried silently.

**Next check due: tick 664.**

---

## Check #49 — tick 664

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate:
`daily-driver-pass(site) = renders(site) ∧ functions(site)` over fixed-denominator corpus-v2
(PART VII.1).

**The window:** t657–t663 — the HEAD-20 sweep three ticks owed (657), `unreachable` split into four
causes and the h2 header limit fixed (658), surface audit #38 (659), the drain measurement (660) and
its retraction (661), located script errors (662), the CSSOM surface probe (663).

**Gate or scoreboard? — GATE, and check #48's steer was obeyed within one tick.** #48 said, in bold,
that the next tick was the HEAD-20 sweep and not another mechanism. t657 ran it. What it found is the
substance of this check, and it is not flattering:

> **Three placement mechanisms (t654/655/656) moved the corpus by less than one site's noise.** The
> one byte-reproducible site moved by nothing at all. And the run-to-run spread of a live site's SHAPE
> is **3.7 points on an unchanged tree** — five times the "regression" t657 was one paragraph from
> reporting.

**THE CORRECTION THIS CHECK MAKES, AND IT IS ABOUT TARGETING, NOT EFFORT.** Every one of those three
ticks was well-founded in isolation: a measured defect, a RED-proven gate, a named mechanism. They were
aimed by *what the last tick's log happened to show* — a load-deadline thread that ran three ticks deep
because each fix surfaced the next. Then t658 asked the certificate what the corpus was actually made
of, spent one `curl` per host, found `playhop.com` was **ours** (our h2 client refusing a 16 KiB
response header block), and moved the certificate: **scored 4 → 5, unreachable 4 → 3.** One tick, one
transport setting, the first corpus-level movement of the session.

> **A thread that keeps producing defects is not the same as a thread that is worth pulling.** The
> load-deadline arc produced three real fixes and no measurable corpus movement; a fifteen-minute
> diagnostic pass over the unscored rows produced one. **Rank by what the DENOMINATOR is made of, not
> by what the last fix uncovered.** The certificate says 15 of 20 sites are unscored and only 2 of
> those are timing — and three consecutive ticks had gone at timing.

**PART VII.1 gains no new clause, and that is deliberate.** #47 added one and #48 found it violated
three ticks running. The gap is not a missing rule; it is that a NEXT list is written by the tick that
just ended, and therefore always argues from the last thing seen. The mechanism that actually
corrected it was **running the measurement** — so the standing answer is the one already in the file:
*a fix argued as "a large fraction of the web" owes a corpus measurement*, and this window paid it.

**TWO RETRACTIONS IN SEVEN TICKS, AND BOTH ARE THE INSTRUMENT WORKING.** t661 retracted t660's
published claim (the hang guard's five firings were five *harness page-loads*, not five rounds of one
navigation) and reverted the change built on it **entirely** rather than keeping it as harmless. t657
retracted a 0.7-point regression before publishing it. Neither was caught by a reviewer or a gate on
the subject — both were caught by **a control run on an unchanged tree**, which is now the third
distinct thing that discipline has saved this session (t654's false regression was the first).
*Measurement/capability balance is the wrong thing to count; what matters is whether the capability
ticks ran ahead of the instrument's last reading, and for t654-656 they did.*

**NO INVARIANT BENT.** t658's 256 KiB is Chrome's announced value, taken because the North Star says
Chromium is the ceiling to MATCH, not tuned until one site worked. t655's commit reserve came **out**
of the load budget rather than extending a Bar 0 bound. t663 declined to make `.sheet` return `null`,
because for an applied `<style>` that is a lie that reads as honest. t656's pre-existing failing unit
test was reported, not absorbed. The one gate written this window that could not go red was **deleted**
(t661), on the project's own precedent for `G_SPAWN`/`G_POOL_ISOLATION`.

**THE STEER.** The next capability tick is the **CSSOM `.sheet` bridge**, and for the first time it is
aimed by evidence rather than by WPT mass: t662's located stack (`insertRules` → `getTag` →
`this.sheet`) names it as what blanks `agoda`, and t663's probe maps the surface exactly — `.sheet`
`undefined` (not the spec's `null`, so every standard guard passes and the page dies one line later),
`document.styleSheets` `undefined`, and `typeof CSSStyleSheet === "function"` **already true**, which
is the false-presence shape this project's reliability doctrine names. **It is all-or-nothing**: a
half-built `.sheet` returning an object without a working `insertRule` gets a CSS-in-JS runtime *past*
its guard and fails worse than today, which is the `IndexedDB` lesson (*keep ABSENT until done*). Its
falsifiable bar is **agoda renders**, not a subtest count.

**STANDING, UNMOVED:** t626's bot-wall ceiling (5 of 20 HEAD sites; 15/20 by deliberate decision) —
now measured rather than assumed, since t658 checked every unreachable host by hand and three of four
were genuinely not ours. `manuk-wpt`'s own tests are still not in the wall (seventh report), and
`scan_static_import_specifiers` still fails on `main` (sixth report). Both harness-owned or
wall-scoped; reported, not touched.

**Next check due: tick 672.**

---

## Check #50 — tick 672

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate:
`daily-driver-pass(site) = renders(site) ∧ functions(site)` over fixed-denominator corpus-v2.

**The window:** t665–t672 — the CSSOM `.sheet` bridge (665), the page-level 39.9s-vs-12s measurement
(666), the per-navigation drain bound (667), its non-effect on agoda (668), a wrong timeline inference
(669), the per-phase ledger that ended the argument (670), the settle-loop budget check that took
agoda 43.6s → 19.0s (671), and this re-sweep.

**Gate or scoreboard? — GATE, and the window's shape is the finding.** #49's steer was the CSSOM
bridge, aimed for the first time by *evidence* (a located stack) rather than WPT mass. It landed, and
then the same evidence-first discipline ran a six-tick perf arc to a real 2.3× on a HEAD site. Both
were driven by instruments this session built: t662's located error report and t670's per-phase
ledger.

**AND THE CERTIFICATE DID NOT MOVE. Both terms, unchanged: `scored 5`, `shape ≥0.75 on 0`.**

That is the third time this session a genuine capability win has failed to appear in the certificate,
and the pattern is now clear enough to name:

> **agoda's 2.3× speedup bought a better FAILURE, not a scored row.** It moved `render-failed` →
> `thin-overlap-5`: *the oracle built the page and we did not.* The clock was never going to fix a
> coverage gap. Likewise t654–656's three placement fixes moved the corpus by less than one site's
> noise, and t658's h2 fix moved it by exactly one site — the only corpus movement of the entire
> session, from a fifteen-minute diagnostic pass.

**The correction, and it is #49's one level deeper.** #49 said to rank by what the denominator is made
of rather than by what the last fix uncovered. This window obeyed that at the level of *phases* — the
per-phase ledger is exactly "make the denominator report itself" — and still spent six ticks inside
one site. The missing question is not *what is slow*, it is **what would make an UNSCORED row
scoreable**, and for 15 of 20 rows the answer is not performance at all: 5 bot-walls (a decided
ceiling), 3 unreachable, 2 timeouts, 1 empty-202, 1 probe-blocked, 2 shell-only (a measurement
architecture limit), 1 thin-overlap.

**A NEW STANDING HAZARD, MEASURED THIS TICK.** `keirin.jp` read **0.048 against a ~0.40 population** in
this sweep; three controls on the same tree minutes later read 0.400 / 0.351 / 0.402. I was one
paragraph from reporting a 35-point regression against my own previous tick. **The certificate takes
ONE reading per site**, and for a high-variance site that is a draw, not a measurement. t657's spread
instrument caught it the moment two sweeps were concatenated. Five of eight scored sites are
effectively deterministic (Δ ≤ 0.3 pts) and two are not — so the fix is *per-site repetition where the
spread demands it*, not a blanket triple-run.

**NO INVARIANT BENT.** t671's budget check takes nothing the budget was not already discarding (past
the deadline the image/mask/background phases are skipped outright). t667's residual — three fixed
drain sites it does not bound — is written into its own gate rather than left to be discovered.
t665's bridge stated its scope (`<style>` only) and refused `null` for `<link>.sheet`, because for an
applied sheet that is a lie that reads as honest. And t661's retraction stood: the change it reverted
came back at t667 **only** once t666 produced the page-level evidence and t669–670 located it.

**THE STEER.** Two things, in order. **(1) Make the sweep repeat the sites its own spread block says
are unstable** — a certificate computed from single draws on a high-variance population is the
delusion this whole redesign exists to prevent, and it is now measured rather than suspected.
**(2) Then attack the unscored rows by category, not by site** — the reachable set is `thin-overlap`
(ours, a coverage gap) and `shell-only` (a `file://`-origin limit in the probe itself); the bot-walls
are a decided ceiling and the dead hosts are not ours. **Do not start another single-site arc without
first naming which unscored CATEGORY it converts.**

**Next check due: tick 680.**

## Check #51 — tick 680

**Horizon:** H0 — the Phase-0 daily-driver certificate. Gate:
`daily-driver-pass(site) = renders(site) ∧ functions(site)` over the fixed-denominator corpus-v2.

**The window:** t673–t680 — the per-site repeat + median collapse (673), the probe deferred to `load`
(674), `reportError` made to report (675), the owed HEAD-20 sweep + the instrument-version column
(676), named access on the Window object (677), the navigation phase ledger that reconciles (678),
inline scripts named + surface audit #40 (679), and the virtual-clock horizon (680).

**Gate or scoreboard? — GATE, and this window finally obeyed #50's steer in the way #50 meant it.**
#50 said: *do not start another single-site arc without first naming which unscored CATEGORY it
converts.* Every capability tick in this window did exactly that. t674 attacked `shell-only` and moved
naukri to `thin-overlap`. t677 attacked `thin-overlap` and named its cause on playhop —
`window.__appData__ is undefined`, HTML §7.3.3 absent entirely. t680 attacked the *timing* half of the
same category and took playhop's Bar 0 hang-guard trips from **6 to 0**.

**AND THE CERTIFICATE STILL SHOWS `scored 5 · shape ≥0.75 on 0`** — but for the first time this
session the reason is *not* "the win was in the wrong place":

> **The last sweep predates four of these eight ticks.** t676's sweep ran on the t674 tree. t677
> (named access), t679 (attribution) and t680 (the clock horizon) have never been measured on the
> corpus at all, and t680 is the first change in five whose mechanism could plausibly move a scored
> term: it removed every drain give-up on the one site it was measured against.

**That is the steer, and it is a measurement, not a capability:** *the corpus is four ticks stale, and
the loop is again in the state #50 warned about — deriving what the certificate would say instead of
asking it.* **RE-RUN HEAD-20 as the next tick.** Three of the five scored rows (`keirin`, `ikea`,
`welt.de`) have never been read with a bounded clock.

**A CORRECTION TO PART VI, and it is the second entry in VI.2 that the tree has moved past.**
VI.2 lists *"**I5** the differential oracle is the discovery engine — ⚠ never completed a full crawl
(`ORACLE_CRAWLED: 0` of 265)"*. That is stale twice over: the crawl completes (265 sites, 392
clusters), and the discovery engine of this window was **not the crawl** — it was the engine's own log,
read three times in four ticks:

| tick | the line | what it named |
|---|---|---|
| 677 | `TypeError: … window.__appData__ is undefined at inline.js:1:155` | HTML §7.3.3 absent — playhop's whole app |
| 679 | `inline.js:1:155` itself | one source name for every inline script on every page |
| 680 | `spinning=queued=1 due_now=0 next_in_ms=86400000 vclock_ms=13822876800000` | our own unbounded virtual clock |

**PART VI should record that the primary discovery mechanism is now the INSTRUMENTED LOG, and that its
yield comes from attribution rather than volume.** Each of those three was found because the previous
tick had made the log one degree more specific. That is a compounding instrument, and it is cheaper per
finding than any sweep this project runs.

**A hazard promoted to a standing rule, because it fired twice in this window.** ⚠ **`most likely` in a
log message is a confession.** t680's hang guard said *"the page is not converging (a self-rescheduling
timer, most likely)"* while holding the entire pending task list; the guess was wrong and it had blamed
the page for six ticks. The general form: **if a message speculates about state the process is holding,
print the state.** The same shape produced t675 (an error reported without its address) and t679 (an
address with no file).

**NO INVARIANT BENT — and one nearly was.** t680's horizon is bounded *by a measured constraint*
(`testharness.js`'s 10s harness timeout) rather than by a number that felt right, and the gate asserts
both sides so a horizon of zero — a bound bought by not running the page, which would bend I4 — cannot
pass. ⚠ **Two gates in this window could not fail on their first draft** (t675's whole-log disjunction,
t680's flag read from inside a 5000ms report that `(due, seq)` ordering always runs first). Both were
caught by running the mutation instead of trusting the green. The falsify discipline is doing real work
and the honest reading is that it had to.

**THE STEER.** **(1) RE-RUN HEAD-20** — the corpus is four ticks stale and the loop is deriving again.
**(2) THEN the per-call load budget**, which is measured, named, and still unfixed: 12.7s + 12.0s under
a stated 12s ceiling, with `initial images+masks` (6.2s, an *enhancement*) running BEFORE
`dynamic scripts` (which builds the DOM). The naive scope fix is a capability loss and the real tick is
the PRIORITY inversion; it is blocked on the nine gate files that call `load_async` without
`finish_loading`, so size it as a full tick and do not smuggle it.

**Next check due: tick 688.**

## Check #52 — tick 688

**Horizon:** H0 — the Phase-0 daily-driver certificate, and as of the observer's t684 USER DIRECTIVE the
gate is stated as a number: **drive COVERAGE and SHAPE to 95%**, attacking `docs/loop/CLUSTERS.md`
top-down, one shared root cause per tick, naming the cluster each tick shrinks.

**The window:** t681–t688 — the owed HEAD-20 sweep (681), the retraction of its own collapse rule (682),
the per-navigation negative cache (683), naukri's 89,905px body diagnosed to four eliminations
(684/685/686), the repeat plan retired where it measures nothing (687), and this.

**Gate or scoreboard? — HONESTLY: NEITHER, for four of these eight ticks, and the board said so before
I did.** The observer's t684/t685 blocks name the exact failure: *"the last ~20 ticks were floor-gates +
instrument-hardening — real work, but they do NOT move the bar"*, and *"the geometry NEAR-MISS lever is
UNTOUCHED after 2 post-steer ticks (both perf/measurement)."* By t687 it was four. The steer was right and
I was not obeying it.

⚠⚠ **AND THE REASON IS RECORDED, BECAUSE IT IS STRUCTURAL, NOT A LAPSE OF WILL.** My in-context launch
prompt says verbatim *"Do NOT grind the CSS-layout tail — it is in diminishing returns"* and lists
media/OAuth/canvas as CO-#1 — all long done. The observer found and fixed both that prompt and the board's
own tick-159 anti-layout block, but **a launch-prompt fix only lands on the next relaunch**, so the running
agent keeps the stale instruction in context and it OUTRANKS a board steer. The board now carries the
correction explicitly (*"this tick-159 anti-CSS-layout steer is DEAD… layout is NO LONGER the tail to
avoid; it is the MAIN LINE"*). **Standing rule for me, not just the observer: when the board and the launch
prompt conflict, the board is newer — and re-read the board's TOP block each tick rather than grepping it
for familiar markers, which is precisely how I missed three consecutive new blocks.**

**PART VI stands as corrected at #51** (the instrumented log is the discovery engine). This window is more
evidence for it: every finding in t683–t686 came from a log line or a per-site diag line.

### THE MEASUREMENT THIS CHECK EXISTS TO PRODUCE — the board's own hypothesis, tested

The mandate's item (2) hypothesises *"ONE shared constant (font-metrics / line-height / margin /
border-box rounding) likely snaps MANY boxes into 8px tolerance at once."* The sweep already computes the
per-site median delta, and across the scored HEAD-20 rows it says:

```text
  site           dx    dy    dw    dh    absolute PLACEMENT
  comix.to        0     0     0     7    100.0%
  desitales2      0    91     0     3      1.2%
  www.welt.de     0  3077     0     0      2.8%
  www.agoda.com   1    14     1     1      3.1%
  keirin.jp       2   206     0     1      0.3%
  www.ikea.com    0   145     0     0      9.9%
  playhop.com     0     0    10     7     14.3%
```

⚠⚠ **`dx` is 0–2 everywhere and `dw`/`dh` are 0 on the WORST sites. The dominant term is `dy` — 91, 145,
206, 3077 — a pure VERTICAL displacement of correctly-sized boxes.** A box of the right size at the wrong
`y` is not a per-box metric error: **something ABOVE it has the wrong height**, and everything below
inherits the shift. The two sites that DO show a text-metric term (`comix` dh=7, `playhop` dw=10 dh=7) are
the two with the HIGHEST placement scores — so the metric term is the residual, not the lever.

**So the board's hypothesis is measurably not the dominant cause, and the FIRST-DIVERGENCE instrument
already points at the real one:**

```text
  keirin.jp    after …/nav:1/…/a:1/img:1  → …/nav:1/div:2   off by dy=70
  desitales2   after …/nav:3/div:1/div:2  → …/div:5/div:2   off by dy=-73
  welt.de      after …/p:2                → …/article:1     off by dy=285
  agoda        after body/div:23/div:5    → body/div:23/div:9 off by dy=631
```

keirin's divergence begins **immediately after an `<img>`** — and `Cc4e6 geometry: <img>` is a 67-site
cluster, with a memory of an `<img>` laying out **784×0**. An image whose height is wrong shifts every
sibling after it, on 67 sites.

**THE STEER, and it is one tick, not a category:** the next tick is **the height of a box above the
content, starting with `<img>`** — take one site's first divergence, find the element whose height is
wrong, and name the cluster (`Cc4e6 <img>` 67 sites, `C7eb9 <body>` 93 sites, `C01ca <div>` 111 sites) whose
site-count must shrink in the following sweep. No more measurement ticks until a geometry cluster moves;
the measurement needed to aim is now done and written down here.

**Next check due: tick 696.**

## Check #53 — tick 696

**Date:** 2026-07-27. **Horizon:** H0 — Pareto Web Parity. **Gate:** the four binary conditions of §H0,
scoped by PART VII to the v1 system and operationalised by `docs/loop/DAILY-DRIVER-CERTIFICATION.md`:
Bar-0 clean + jarring invariants ≥95% + **shape ≥0.75 on ≥95% of sites** + interactivity ≥95%, named
exceptions only.

### GATE OR SCOREBOARD? — gate, and the previous check's steer is why

Check #52 ended with a hard steer: *"no more measurement ticks until a geometry cluster moves"*, and it
named the lever from the first-divergence data — **the height of a box above the content, starting with
`<img>`**. That steer landed:

```text
  t691  a broken <img> is 16x16 in Chrome (was 784x0) + every line box starts with a STRUT
        ikea coverage 97.1% -> 100%   keirin dy 206 -> 161   welt dy 3077 -> 2957
  t695  the half-leading belongs to each INLINE BOX, not to the line
        22 of 22 probed boxes match Chrome exactly on the mixed text+atomic fixture
```

Two consecutive gate-condition ticks on the `shape` term, both Chrome-measured, both RED-proven. The
steer worked because it named **one element and one number**, not a category. That is the reusable part.

### THE CORRECTION TO PART VI — I5's discovery engine is reading a stale instrument

**VI.2's `I5` row needs its third correction, and this one is about TRUST rather than completeness.** The
row currently says the crawl completes (265 sites, 392 clusters) and that the instrumented log has become
the depth instrument. Both still hold. What is newly false is the unstated assumption underneath the
board's own mandate: **that `docs/loop/CLUSTERS.md` describes the engine as it is now.** Measured this
tick, re-running the oracle serially on three of the top `MISSING BOX: <div>` contributors:

```text
  site           stale crawl (Jul 22)   fresh serial re-run   
  theverge.com        944 missing              0 missing
  vox.com             872 missing              0 missing
  wix.com            1905 missing           3394 missing   (90% of the page)
```

Two of the three are **gone**, and the third is far worse than recorded. So the registry's ranking — the
document the board calls *"the priority ledger… no judgement required"* — is a five-day-old reading whose
top rows do not survive re-measurement. **Every number has a harness, and the harness is part of the
number** (Lesson 4, sixth occurrence). A convenience-ordered ledger read as ground truth is precisely the
Pareto trap I4 exists to prevent, one level up: it does not rank the wrong *area*, it ranks a *phantom*.

**And the label itself merges three subsystems.** `MISSING BOX` is computed from the absence of a layout
rect, so one ranked row contains: *not in the DOM at all* · *in the DOM with no computed style* · *styled
and generated no box*. Chasing `C3833` this tick, the answer on its worst site was the first — script
deletes 4917 elements inside the `load` event and never rebuilds them. A coverage or geometry tick aimed
at that row would have moved nothing, and the row gave no way to know that in advance.

### THE STEER

1. **RE-CRAWL BEFORE RANKING AGAIN.** The registry is stale and is currently steering the loop. A full
   corpus sweep is owed before any further tick claims a cluster's mass. This is not a measurement
   detour — it is the instrument I5 designates as the discovery engine, currently disagreeing with itself.
2. **SPLIT `MISSING BOX` BY CAUSE, not by tag.** `manuk-wpt boxes --why ID` (landed this tick) answers
   *DOM / style / box* in one command. A cluster row that names the symptom's layer cannot rank causes,
   and ranking is the only thing the registry is for.
3. **The `shape` term keeps the alternating slot** (board refinement t685): the last two geometry ticks
   both delivered, and `dy` is still the dominant term. Continue there while (1) runs.

**No invariant is being bent.** I3 is not implicated — this tick added a JS-surface capability with no new
rendered construct, so there is no semantic-model exposure owed. I4 holds: the tick was chosen off the
usage-weighted cluster ranking, and the finding is that the ranking needs re-measuring, which is I4
enforcing itself.

**Next check due: tick 704.**

---

## Check #54 — tick 704

**Horizon:** H0 — Pareto Web Parity. **Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate —
render × function per site, `shape ≥ 0.75` on ≥95% of a fixed-denominator corpus.

**Gate or scoreboard?** Gate, and for the first time in five ticks by a route that was *written down
in advance*. Ticks 700–703 were four consecutive measurement ticks on one residual (the §10.8.1
inline-block baseline), each correctly refusing to land a patch that traded a control site's SHAPE
for a large win elsewhere. That is the ratchet working. It was also, by t703, a loop: four
hypotheses, one cause, two no-ops and one regression, and a fifth hypothesis queued.

**The drift this check catches is not in the constitution — it is in the SEARCH.** t703's own PATTERN
line said it: *"when consecutive hypotheses about the same residual keep missing, stop generating
hypotheses and go read the failing case."* Reading the failing case this tick did not confirm the
fifth hypothesis; it surfaced a **completely different and larger mechanism** sitting in the same
cluster report — every SVG `<path>` on the page reporting `0×22`, a number from the wrong formatting
model entirely. That is `Ccd7f`: 34 sites, 1,658 hits, and it had a **build spec written at tick 393
that named this exact gap and was never built** (`docs/wiki/box-layout.md`, *"geometry mapping is the
other half"* — the paint half landed at t394 and the sentence sat there for 310 ticks).

**So the reusable finding is about the BACKLOG, not the bug:** a deferred half of a landed feature is
invisible to every ranking instrument the loop has. `CLUSTERS.md` ranked its *symptom* at #3 for
months without connecting it to the spec that already said how to fix it, and no audit reads
`box-layout.md` for the word "remaining". **A build spec whose second half is unbuilt is an
UNTRIAGED TICK with excellent prose** — the same shape as [[a bug described accurately in a comment
is an untriaged bug]] from t633-649, one level up.

**On I4 (usage-weighted Pareto):** honoured. The tick was chosen off the oracle's own ranked ledger,
attacked the third-ranked mechanism family, and the deliverable is measured against the site the
project keeps *because* it is byte-reproducible.

**On PART VII / V1-SCOPE (pure browser capability):** honoured — no harness file touched. The one
harness observation (a pre-existing RED in `manuk-page`'s `static_import_scanner…` lib test, present
at HEAD, unrelated to this tick) is recorded in the journal and not acted on.

**No invariant is being bent.** The ratchet held twice in one tick: once when a −0.3 SHAPE reading on
`en.wikipedia.org` stopped the first implementation from landing, and once when the *fix* for it
turned out to be a spec question (`getBoundingClientRect` returns the **decorated** box, stroke
included — `getBBox()` is the fill box) rather than a tolerance to widen.

### THE STEER

1. **BEFORE RANKING A CLUSTER, GREP THE WIKI FOR ITS SPEC.** `Ccd7f` had a build plan for 310 ticks.
   A cluster row and a `docs/wiki` build spec that describe the same mechanism should never again be
   discovered independently — the ledger should cite the spec.
2. **A SURFACE AUDIT SHOULD READ `docs/wiki/*` FOR "remaining"/"the other half"/"named residue".**
   Those phrases mark work the author bounded and deferred; nothing currently re-surfaces them.
3. **THE FULL CORPUS SWEEP IS STILL OWED** — four ticks running, and check #N−1's steer #1 said the
   registry is stale and is steering the loop. It still is.

**Next check due: tick 712.**

---

## Check #55 — tick 712

**Horizon:** H0 — Pareto Web Parity. **Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate —
render × function per site, `shape ≥ 0.75` on ≥95% of a fixed-denominator corpus.

**Gate or scoreboard?** Neither, for seven of the eight ticks, and that is the finding. Ticks
705–712 were:

```text
  705  measurement   the first honest full-corpus certificate on the rebuilt instrument
  706  measurement   "render-failed" is not a paint bug — the author CSS silently never arrives
  707  fix           fidelity-progress was parsing a vocabulary the instrument stopped printing at t531
  708  fetch         a deadline-cut stylesheet is now COUNTED as failed (it used to vanish)
  709  instrument    three "render-failed" sites were a bot wall wearing HTTP 200
  710  audit         surface audit #43 — the death-tail's price moved and nobody re-checked it
  711  measurement   the honest render-failed remainder is the SPA hydration wipe (= C3833)
  712  capability    a <script src> in the MARKUP that loaded told the page nothing
```

**One capability tick in eight.** Every one of the other seven was legitimate and several were
necessary — the instrument was *lying*, in four separate ways, and an instrument that lies outranks
everything it measures. But the honest accounting is that the H0 exit gate did not move for seven
ticks, and the loop knew it: `FIDELITY-PROGRESS.tsv` was frozen at 2026-07-20, the last full-corpus
sweep is **291 hours old**, and `tick.sh`'s own pre-flight has been printing *"a capability tick must
measure THIS tree"* into every landing.

**So the drift is not "we stopped measuring" — it is the opposite shape, and it is new.** The classic
failure this check exists to catch is a loop that optimises a number without fixing the instrument.
What happened here is a loop that **fixed the instrument four times and did not re-run it**. The end
state is identical from downstream: a correct instrument over stale data decides exactly as badly as
a broken one, and `CLUSTERS.md` — which PART III's standing rule makes the tie-breaker, and which the
board makes CO-#1 — is currently being read off a crawl that no longer describes the engine. Two of
its top rows already re-measured to zero once (t696).

**The correction, and it is this tick's action:** the full-corpus sweep is RUNNING, and no further
cluster claim is made until it lands. t712 named a mechanism inside `C3833` and moved a per-site
number (wix DOM inserts after the wipe 6 → 44); it did **not** claim the cluster shrank, because
only the sweep can say that and the sweep had not run in twelve days.

**On I4 (usage-weighted Pareto):** honoured. t712 was chosen off the oracle's ranked ledger (top
cluster by hits), and its population was stated as the **floor** it is (8 of 245 corpus snapshots
carry the idiom in the *served* document) rather than as the number that flatters it.

**On I5 (the oracle is the discovery engine):** ⚠ **sharpened, and this is the PART VI correction.**
Check #51 recorded that the primary discovery engine had become the *instrumented log* rather than
the oracle's volume. t712 is the first finding in that arc that the log **started but could not
finish**: the log said `google is not defined` and stopped there, and the actual defect — a `load`
event that never fires — emits no line at all, by construction. It was found by a **differential
probe against headless Chrome on a four-case fixture**, which is the oracle's method applied by hand
at the size of one behaviour instead of one page. Record that as a third instrument alongside the
other two: *the oracle is the breadth instrument, the log is the depth one, and a hand-built
differential fixture is the only one that can see an ABSENCE* — because an absence has no log line
and no divergent box, only a missing entry in a sequence some other engine produces.

**On PART VII / V1-SCOPE:** honoured. t712 moves component 1 (daily-driver rendering parity) and no
harness file was touched. The one adjacent finding that is genuinely out of scope was named and
dropped rather than pursued: `accounts.google.com` answers our honest User-Agent with 403, which is
the bot-wall track (PART IV, "the hostile bot-walled tier").

**On I3 (semantic model in lockstep):** not implicated — t712 adds a JS event with no new rendered
construct, so no semantic-model exposure is owed. Same reasoning as check #53.

**No invariant is being bent.**

### THE STEER

1. **AN INSTRUMENT FIX IS NOT DONE UNTIL THE INSTRUMENT HAS RE-RUN.** Four ticks repaired the
   fidelity rig and none of them re-swept, so the repairs bought nothing the ledger can see. Make the
   re-run part of the fix, not the next tick's problem.
2. **NO CLUSTER CLAIM WITHOUT A SWEEP THAT POSTDATES THE CHANGE.** The board's own rule
   (*"PROVE the cluster's site-count SHRINKS next sweep"*) has been unenforceable for twelve days.
   It becomes enforceable again when this sweep lands; until it does, every "cluster X moved" is a
   mechanism claim, not a count claim, and must say so.
3. **THE NEXT NON-MEASUREMENT TICK IS A GEOMETRY ONE.** The board's t685 refinement requires that
   every other tick attack GEOMETRY or `display` to move SHAPE, and `C01ca <div>` (111 sites / 14,002
   hits) has not been touched since t698. Coverage work alone cannot clear the 0.75 bar.

**Next check due: tick 720.**

---

## Check #56 — tick 721

**Horizon:** H0 — Pareto Web Parity. **Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate —
render × function per site, `shape ≥ 0.75` on ≥95% of a fixed-denominator corpus.

**Gate or scoreboard?** **Gate**, and by a wide margin — the reverse of check #55's finding eight
ticks ago. Ticks 713–721:

```text
  713  measurement   HEAD-20 re-run; the full sweep priced at ~10h and named unschedulable
  714  measurement   external CSS applied AFTER `load` — every script on that path measures nothing
  715  capability    the refusal RETRACTED (welt's 95.6% was our own timeout) — and reverted again
  716  measurement   the population read; keirin -7.2; the arithmetic that forbids the fix
  717  capability    a frame with nothing to FETCH got nothing to LOAD  (contentDocument)
  718  measurement   the probe is BLIND until its own bug is fixed
  719  capability    the CSS-ordering fix LANDS on the third design
  720  measurement   the population read on it; ikea 21 missing boxes -> 0 (isolated)
  721  audit         surface #44 — a `works` row that does not work
```

Three capability ticks and one of them is the largest correctness fix this session: a page's own
scripts on the agent/measurement path can now measure the document they are in. Check #55's steer —
*"an instrument fix is not done until the instrument has RE-RUN"* — was obeyed literally: **every
capability tick in this window was followed by a HEAD-20 read** (716 after 715, 720 after 719), and
that is what caught keirin's −7.2 and forced two reverts.

**On I4 (usage-weighted Pareto):** honoured, and sharpened. t719's caller audit corrected a claim the
loop had published three times — `load_async` has **no shell caller**, so the CSS-ordering bug was
the **agent's** and the **measurement rig's**, not the shipping browser's. Under I4 that is a *higher*
priority, not a lower one: PART VII component 2 is the agent surface, and the fidelity instrument
decides what the loop builds next. But the earlier framing was wrong and is corrected in the record.

**On I5 (the oracle is the discovery engine):** ⚠ **the PART VI correction this check owes.**
Check #55 named a third instrument class — *a probe that runs inside the page, the only one that can
see a SCHEDULE.* Tick 718 found it has a blind spot that no other instrument has: **an instrument
built to find one bug is calibrated BY that bug until it is fixed.** The probe reported a
spec-citable, top-cluster float-blockification bug across 85 elements of `keirin.jp` that **does not
exist** — a four-case control fixture blockifies floats exactly like Chrome. So the class is real and
its rule of use is: *before trusting a new instrument on a new subject, run it on a case where you
already know the answer.* Recorded in PART VI's spirit rather than its letter: the instrument list is
not the loop's problem, the instrument's **calibration** is.

**On PART VII / V1-SCOPE:** honoured. Nine ticks, no harness file touched. Two harness observations
were noted and left: the verify wall exceeds its 300s target (self-audit t715), and
`map-reconcile.sh` still does not search `shell/` (tenth audit running).

**No invariant is being bent.** The ratchet held **four separate times** in this window and every one
of them was a real save: t712 cleared of ikea's coverage loss by rebuilding the prior tree; t715's
refusal retracted by varying a knob the patch did not contain; t716's revert on a −7.2 that two
isolated runs per tree confirmed; and the `GATES 336 < 337` refusal that made removing a gate a
deliberate, explained act rather than a silent one.

### THE STEER

1. **A CONTROL THAT VARIES YOUR CHANGE ANSWERS ONLY "DID I DO THIS."** Revert-and-remeasure is this
   project's standard control and it produced a *wrong* conclusion at t714 — the number moved exactly
   with the patch and the cause was elsewhere. The second question, *"what else does this?"*, needs a
   knob the patch does not touch. Make it the second step of every regression attribution, not an
   afterthought.
2. **MEASURE A CONDITIONAL CAPABILITY UNDER BOTH CONDITIONS.** t719's fix degrades gracefully, so it
   reads at its floor in the batch and its ceiling in isolation (`ikea` 97.1% vs 100.0%). Either
   number alone is a future disagreement with ourselves, filed as "noise".
3. **THE FULL CORPUS SWEEP IS STILL OWED AND IS STILL UNSCHEDULABLE** — ~10 hours, serial, one Chrome
   per site, and it cannot share the box with the tick loop. Three checks have now asked for it. It
   needs a *window*, not another resolution; until then HEAD-20 is the honest substitute and every
   cluster claim stays a mechanism claim.

**Next check due: tick 729.**

---

## Check #57 — tick 729

**Horizon:** H0 — Pareto Web Parity. **Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate —
render × function per site, `shape ≥ 0.75` on ≥95% of a fixed-denominator corpus.

**Gate or scoreboard?** **Gate**, and the eight ticks since #56 are the densest capability run of the
session: 722 `rlh`, 723 root font metrics, 724 `CSS.supports`, 727 `sheet.media`, 728 the client box,
729 `elementsFromPoint` — six capability ticks, two measurement ticks (725, 726) and one audit (721),
with **no reverts**. That is the opposite composition of check #55's window and it happened without a
steer, because the work stopped being chosen off a backlog.

**The PART VI correction this check owes is about METHOD, and it is the session's largest.** Ticks
721–729 found nine defects, and **not one came from `CLUSTERS.md`.** They came from:

```text
  a probe that left the frame        surface audit #44 -> a `works` row that does not work
  a sentence in the previous tick    t722's "we now do two" of three -> t723
  the most BORING possible input     `color: red` -> CSS.supports answering false to everything
  a fixture built for something else t726's container-unit fixture -> a cascade-order bug
  24 surfaces diffed at once         t728 -> the client box was the border box everywhere
```

**The ranked ledger did not point at any of them, and all nine are real.** That is not an argument
against the ledger — it ranks what the oracle can *see*, which is boxes on 265 home pages. It is an
argument that **the oracle's ceiling (the tick-42 principle, still in STATUS.md) binds harder than
its ranking**: `CSS.supports` answering `false` to `display:flex` in the shipping browser is invisible
to a box diff, because a page that takes its fallback renders *something* and the diff sees a layout
difference, not a lie. So: **a differential probe against Chrome on a hand-built fixture is a
first-class discovery instrument, not a verification step**, and it is cheap — t728 cost one fixture
and found a bug touching every bordered element on the web.

**On I4 (usage-weighted Pareto):** honoured, with one honest qualification recorded at t725. The
`CSS.supports` fix measured **zero** movement across HEAD-20. That is not a failure of I4 — the
capability is justified by the capability — but it is evidence that *"usage-weighted"* and
*"measurable on twenty home pages"* are different sets, and the loop should stop expecting the second
to confirm the first.

**On PART VII / V1-SCOPE:** honoured. Eighteen ticks, no harness file touched.

**No invariant is being bent.** The ratchet was not invoked in this window because nothing went
backwards — and two gates were *corrected* rather than weakened: `g_element_from_point`'s NaN
assertion demanded `null` and cited CSSOM-View for it (an invented citation; Chrome throws), and the
map's `lh`/`rlh` row read `works` on a probe that tested one property. **Both corrections made the
assertion stricter.**

### THE STEER

1. **KEEP THE BROAD DIFFERENTIAL PROBE IN THE ROTATION.** One fixture, N surfaces, diffed whole
   against Chrome. It found the client-box bug in a tick that had no hypothesis, and the backlog it
   competed against is not sorted by importance.
2. **WHEN A TICK ENDS BY COUNTING HOW MANY OF A SET IT DID, THE REMAINDER IS THE NEXT HYPOTHESIS.**
   t722 → t723 took one tick because the sibling had a NAME. The same class has cost this project 310
   ticks when it did not.
3. **THE FULL CORPUS SWEEP IS OWED FOR THE FOURTH CHECK RUNNING.** ~10 hours, serial, unschedulable
   beside the loop. It is now the longest-standing unmet instruction in the log, and every cluster
   claim stays a mechanism claim until it runs.

**Next check due: tick 737.**

---

## Check #58 — tick 737

**Horizon:** H0 — Pareto Web Parity. **Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate.

**Gate or scoreboard?** Gate. Ticks 730–737: six capability ticks (`document.fonts`, the webfont
relayout, navigation timing, ARIA reflection, `adoptNode`, shadow-root identity, `composedPath`), one
measurement (the out-of-wall gate sample), no reverts, and **every one of the six came from a broad
differential probe against Chrome** rather than from a ledger.

**The PART VI correction this check owes is a NAME for what the probes keep finding.** Across
ticks 717–737 the same bug shape appeared **five** times, and none of it was reachable by feature
detection:

```text
  typeof null === 'object'         contentDocument "present", null on the next line     t717
  CSS.supports(…) === false        a correct boolean, false for `display:flex`          t724
  getEntriesByType() -> []         a correct Array; entries[0] is undefined             t733
  root.host -> "example.com:8080"  a correct string; the WRONG host                     t736
  composedPath() ends at document  a correct Array, one entry short                     t737
```

**A wrong answer of the right TYPE.** Every `typeof` check passes; every "is the API there?" audit
says yes; the failure is one index, one property or one boolean away. `G_CAPABILITY` — the ledger as
executable assertions — cannot see this class either, because it asserts *presence*. The only
instrument that finds it is **a probe that knows what the answer should be**, which in practice means
running the same fixture in Chrome.

So, added to PART VI's instrument list as a fourth entry: *the oracle diffs the OUTPUT, the log
reports EVENTS, a page-side probe observes the SCHEDULE, and **a hand-built differential fixture is
the only one that can check a VALUE.***

**On I4:** honoured. The picks were ranked by *what happens on absence* — a throw (`document.fonts`,
`adoptNode`, navigation timing), a contract libraries branch on (`closed` shadow roots,
`composedPath`), or the project's own moat (ARIA reflection is I3). Not by popularity.

**On PART VII / V1-SCOPE:** honoured; twenty-six ticks, no harness file touched.

**No invariant is being bent.** ⚠ And one gate nearly shipped **vacuous** this tick:
`has("det=I")` passed against the output `det=I>window`, because a substring is not a value. Caught by
running the mutation, which is the only reason it was caught at all.

### THE STEER

1. **ASSERT TO A FIELD BOUNDARY, NEVER A PREFIX.** `has("det=I")` matched `det=I>window` — the exact
   bug the assertion existed to catch. This is the *"assert the COUNT on data-file edits"* lesson in a
   new place, and the general form is: **a `contains` check is only as strong as what cannot follow
   it.**
2. **KEEP THE PROBE IN ROTATION, AND VARY THE BAND.** Four bands so far (DOM/CSSOM, forms/text,
   navigation/storage, events/shadow) — every one yielded at least one real defect, and the yield has
   not fallen off.
3. **THE FULL CORPUS SWEEP IS OWED FOR THE FIFTH CHECK RUNNING.** ~10 hours, unschedulable beside the
   loop. It is the longest-standing unmet instruction in the log.

**Next check due: tick 745.**

---

## Check #59 — tick 745

**Horizon:** H0 — Pareto Web Parity, under PART VII component **1 (daily-driver rendering parity)**.
**Gate:** the `DAILY-DRIVER-CERTIFICATION.md` certificate — and, as of the owner's 2026-07-29 steer, its
render leg has a *winnable* form for the first time: **shape ≥ 0.75 on ≥ 95% of the IN-SCOPE corpus**
(bot-walls excluded per cert §3, because they are excluded by PART IV and by our own no-stealth policy).
Baseline 11/209 = 5.3%; target 199; distance **+188**.

**Gate or scoreboard?** Gate, and the honest answer this check owes is about the DENOMINATOR, not the
work. Ticks 738–745 were: two protocol/agentic blockers found by trying (738), event retargeting (739),
a homeless method given a home (740), an "idempotent" wrapper hiding a component bug (741), SVG
`viewBox` laid out as pixels (742), the icon-sprite `<use>` that drew nothing (743), the priority
ledger keyed by HTML tag instead of by mechanism (744), and a UA hint that overwrote a correctly
cascaded `padding: 0` (745). Six of eight are render-fidelity root causes on the representative corpus,
which is the PART VII component 1 line exactly. No harness file was touched in any of them.

**⚠ THE CORRECTION: A METRIC WHOSE DENOMINATOR CONTRADICTS PART IV CANNOT BE REACHED, AND WE RAN ON ONE
FOR ~60 TICKS.** The render headline divided by all 265 corpus sites, but **56 of them (21%) are
bot-walled / probe-blocked / unreachable** — and I4 plus PART IV put that tier *out of the
compatibility mission by prior decision*. So "95% of 265" capped at ~80% **by construction**: the loop
was grinding toward a bar its own constitution forbids it to clear. Fixed in `fidelity-progress.sh`
(EXCLUDED is watched separately and capped; the pass rate divides by the in-scope 209, and crashed /
render-failed / shell-only stay IN because those are OUR bugs). This is the encoding-tail lesson of
VI.3 in a new place: **the number was not wrong, its frame was — and a ranking inside the wrong frame
is confident and wrong.** Added to VI.3 as a corollary: *when a metric refuses to reach its bar, check
the denominator against PART IV before adding work.*

**Second correction, to VI.2's I5 row (the instrument list).** Check #58 named four instruments (oracle
diffs OUTPUT, log reports EVENTS, page-probe observes SCHEDULE, differential fixture checks a VALUE).
Ticks 744–745 add the fifth, and it is about *keys*: **the oracle can only rank by what its record
carries across the process boundary.** `oracle::cluster` had computed the full mechanism signature —
`{displaced|mis-sized}: {width|height|x|y} ~Npx` — for hundreds of ticks, and the crawl's JSONL emitter
dropped the `delta` field, so every geometry divergence in the corpus reached the merge unkeyable and
351 ticks of work were ranked by HTML **tag**. t745 is the first tick ranked by *primitive* instead, and
it found its bug in ninety minutes off the new key. **A ledger is only as fine-grained as its serialised
record.** (And the missing field is REFUSED, never zero-filled: `~0px` would have read as measured.)

**On I4:** honoured, and sharpened. `td { padding: 0 }` is not a quirk — it is Tailwind preflight,
Normalize, and every reset since 2004, i.e. usage-weighted breadth. The 2px it cost was a `dy` term, so
it charged every row below it on every table-based page.

**On I5:** the oracle is doing exactly its constitutional job — the discovery engine. t745's chain was
*mechanism ledger on 6 anchor sites → the `dh=+2` band → four shrinking hand-written fixtures against
live Chromium → the guard, not the value*. Breadth instrument to depth instrument in one tick.

**On PART VII / V1-SCOPE:** honoured; thirty-four ticks, no harness file touched. The full-corpus sweep
now owed for **eight consecutive checks** is finally RUNNING as the agent's own process (t745's runner);
it was started before this tick's engine change and the change was deliberately kept out of
`target/release/manuk-wpt` so the baseline is one engine population, not two.

**No invariant is being bent.**

### THE STEER

1. **RANK BY PRIMITIVE NOW THAT THE LEDGER CAN.** The first mechanism-keyed corpus ledger arrives with
   the running sweep. One primitive per tick, ordered by (in-scope sites × dy severity), each verified on
   the fully-covered anchor sites — and **each must show its band shrink on the next sweep or be reverted
   or re-scoped**. That is the finiteness readout the burndown plan promises.
2. **GREP FOR POST-CASCADE WRITERS, NOT FOR CASCADE BUGS.** t744 (the writer's field list defines what
   the reader may know) and t745 (a hint ran after a correct cascade and overwrote it) are the same
   lesson two ticks apart: **when a value is right where it is computed and wrong where it is used, the
   bug is between them.** The next audit sweep is the rest of `apply_presentational_hints` — every field
   it guards on a value an author can legally write (`padding`, `margin`, `border-width`, `opacity: 1`,
   `z-index: auto`) is the same defect waiting.
3. **THE SWEEP IS A MEASUREMENT, SO PROTECT IT LIKE ONE.** A rebuild mid-sweep splits the baseline into
   two engine populations and the `instrument` column would not show it — it hashes the instrument, not
   the engine. Ticks land *around* a running sweep, never through it.

**Next check due: tick 753.**

---

## Check #60 — tick 754

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)**.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md`, milestone **M1 RENDER** — shape ≥ 0.75 on ≥ 95% of the
in-scope **representative CrUX** corpus (owner-locked 2026-07-30; the curated 265 is retired as the
driving set). Baseline established this window: **5/130 = 3.8%**, target 124, **need +119**.

**Gate or scoreboard?** Gate — and this window is unusual in that **most of it moved the INSTRUMENT, not
the engine**, so the question deserves a real answer rather than a reflex.

Ticks 745–754 were: the `td{padding:0}` post-cascade overwrite (745), the sig-stripper panic banked as a
Bar-0 crash (746), block-in-inline margin collapse (747), one-face-per-family webfonts (748), `system-ui`
aliased to the sans generic (749), the burndown sweep (750), UA-fallback pages scored as layout failures
(751), the CrUX baseline (752), watchdog timeouts banked as Bar-0 crashes (753), and the oracle's
class-signature keying (754).

**Four engine ticks, and they are the only reason there is a slope at all:** 745/747/748/749 moved
in-scope pass **4.9% → 6.7%** on the 265 (+4 sites across 0.75), with `news.ycombinator` 0.730→0.802 and
`martinfowler` 0.678→0.771 — the two sites t745's own journal had named. That is an exit-gate condition
moving, measured on the gate's own metric, not a scoreboard.

**The other six are not scoreboard work either, and here is the test I applied**: each removed a
*falsehood the loop would have acted on*. 751 stopped fetch failures being scored as layout failures. 753
removed 8 phantom **Bar 0** events — the category that outranks everything, so a phantom there costs the
whole board. 754 found the cluster ledger — *"the priority ledger, not a suggestion judgment may
override"* — was **~68% phantom** (2750 → 892 divergences over three sites), because one differing
ancestor class re-keyed whole subtrees; the shallowest "missing box" on `heart.org` was **`<body>`**.
A tick chosen from that ledger would have been a tick spent on nothing. **Instrument work that changes
what the next tick WILL BE is gate work; instrument work that only makes the number prettier is
scoreboard work.** These changed the ranking. None of them moved f12 by a point, and none claimed to.

**⚠ THE CORRECTION THIS CHECK OWES — I nearly ranked from the artefact.** After t752 I built a mechanism
ledger over the CrUX worst sites, read `missing box: <div>` at the top (9 of 12 sites, 1330 hits), and
was one step from spending the next several ticks on a coverage bug **that did not exist**. What stopped
it was sorting the cluster by DEPTH and looking at the shallowest member — one command. The general form,
which belongs in VI.3 beside the denominator corollary: **before ranking from a cluster, look at its
shallowest/simplest member and ask whether that member is credible.** A coverage failure that begins at
`<body>` is not a coverage failure. This is the fourth instrument in the I5 list (checked #58/#59) and it
is about KEYS again, one tick after t744's `delta`: *the record's identity is part of the measurement*.

**On I4 (usage-weighted breadth, tail excluded):** honoured, and sharpened twice this window. `system-ui`
is the body font of Bootstrap 4/5, Tailwind and GitHub — breadth, and the fix was Chrome-exact on all
four real stacks. Against that, `font-size-adjust` explains a whole site (matklad: 807 nodes, shape
0.004) and was **measured to 1 of 24 sites and deliberately NOT built** — recorded in the map as
`missing`. That is I4 working as written: a hit-count story losing to a site-count rule.

**On the corpus switch (PART VI correction).** The driving corpus is now the representative CrUX sample,
and the first honest reading is **3.8% against the 265's 6.7% — the curated corpus was flattering us by
~1.8×**, on the same engine, the same day, the same instrument. PART VI is corrected accordingly: the
distance to H0's render gate is **+119 in-scope sites**, not the +184 the 265 implied, and the population
is one whose tail (35% excluded, vs 22%) is materially more hostile.

**On PART VII / V1-SCOPE:** honoured; no harness file authored by me. One disclosure: tick 751 carried an
**observer-owned** `scripts/fidelity-progress.sh` change that appeared mid-tick, because `tick.sh` stages
with `git add -A` and reverting it is the documented way to clobber observer work. Attributed in that
commit rather than hidden or destroyed.

**No invariant is being bent.**

### THE STEER

1. **RE-CRAWL BEFORE RANKING.** `CLUSTERS.md` is invalidated by t754 and every number quoted from it —
   including the board's `MISSING_BOX 401 sites / 17154 hits` — was computed with the broken keys. The
   next ranking decision must come from a fresh crawl on the CrUX corpus, not from that file.
2. **THE ENGINE LEVER IS STILL SHARED-`dy` PRIMITIVES, AND IT IS CALIBRATED NOW.** Four such fixes bought
   +1.8 pts. That is the unit: Phase-0 render is **finite but not fast** — dozens of batches, not two.
   Resist the search for one lever that closes it; there is no evidence such a lever exists, and three
   plausible candidates (starved CSS, systematic advance error, flex sizing) were each refuted by a
   control this window.
3. **CONTROL BEFORE BLAME, AND IT PAID FOUR TIMES.** Every large per-site swing this window was an
   instrument artefact, confirmed by re-running the same binary (discourse.org: sweep 0.087, three
   controls 0.4964/0.4995/0.4995). A per-site delta is not a result until a control says the binary
   produces it twice.

**Next check due: tick 762.**

## Check #61 — tick 763

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)**.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md`, milestone **M1 RENDER** — shape ≥ 0.75 on ≥ 95% of the
in-scope **representative CrUX** corpus. Ledger: t752 **3.8%** → t758 **5.4%** (7/130), target 124,
**need +117**. Slope **+1.6 pts/sweep on the CrUX population**, six engine ticks apart.

**Gate or scoreboard?** Gate. Ticks 755–763 were: the wall-audit counter (755), a *correction* retracting
t749's reading of its own samples (756), CSS nesting's `&` resolving to `<html>` (757), the CrUX sweep
(758), `&nbsp;` collapsed as white space (759), a measure-and-pin of three defects under one fixture
(760), the phantom line box under every empty wrapper (761), column flex-wrap (762), and
`display:-webkit-box` rejected outright (763).

**Seven of nine are engine, and all seven are usage-weighted breadth:** CSS nesting is *41% of the corpus
in inline `<style>` alone*; `&nbsp;`, the empty wrapper, the sticky-footer page shell and the line-clamp
card excerpt are each idioms that appear on a large fraction of ordinary pages rather than deep in a spec
tail. That is I4 as written. The measured movement: t757 alone carried the sweep +1.6 pts; t763 moved
`momon-ga` shape 0.509 → 0.565 on the same instrument with a byte-identical control site.

**⚠ THE CORRECTION THIS CHECK OWES — the headline metric is BLIND to the worst class of failure.**
t762's defect put marktplaats.nl's header, nav, entire page body and footer in **four side-by-side
1200px columns** (`#page-wrapper` at x=2201). Fixing it moved **shape by one point**. Shape is
*parent-relative* by construction — every descendant is displaced *with* its container, so the
common-mode term cancels and a catastrophically wrong page scores like a good one. PART VI is corrected
accordingly:

> **The jarring invariants (`h_overflow`, `overlap`, `reading_order`) are not decoration beside shape —
> they are the only channel in the instrument that can see a whole-page error.** The ranked burndown
> reads them FIRST; `shape` ranks the residue after they are clean.

This is the same family as check #60's "look at the cluster's shallowest member": a summary statistic
computed in the wrong frame is confident and wrong. It is also the fifth entry in the I5 instrument list.

**On I2 (never patch dependencies).** Two of this window's fixes were around a vendored parser's build
flags, and neither forked it. stylo 0.19 gates `display:-webkit-box` and `-webkit-line-clamp` to
`#[cfg(feature = "gecko")]`; the answer was option 3 of `STATUS.md`'s borrowed-engine ladder — a
supplement in **our** cascade, recovered through the existing merge, recording a marker rather than
copying the weaker cascade's `display`. The fork surface is still empty.

**On I3 (semantic model in lockstep).** Held: every one of these is a geometry/computed-style fix, and
both channels the agent reads — `node_rects` and computed `display` — are the things being corrected.
t761 explicitly *refused* to drop reporter fragments because that would take elements out of
`node_rects`, i.e. it paid a small cost to keep the semantic model whole.

**On PART VII / V1-SCOPE:** honoured. No `scripts/` file authored this window.

**No invariant is being bent.**

### THE STEER

1. **RANK ON THE JARRING INVARIANTS, NOT ON SHAPE.** The next render ticks come from the sweep's
   `h_overflow` / `overlap` / `reading_order` columns. Named and open right now: `aftenbladet.no` 521,
   `mobile.ir` 259 (and `reading_order` **874**), `alphanews.live` 235, `razaoautomovel` overlap 71,
   `ta3lemkonline` reading_order 817. Each is a whole-page error that shape will barely register.
2. **THE MECHANISM ORACLE IS NOW THE ENTRY POINT, AND IT PAID TWICE IN ONE RUN.** One
   `oracle --urls a,b,c,d` over four fully-covered near-bar sites produced *both* t762 and t763 — the
   second row of the same output was the second tick. Run it on a batch, spend every row that names a
   mechanism, and do not re-run it per tick.
3. **A RECOVERED PROPERTY WHOSE ACTIVATING VALUE IS BEHIND THE SAME `cfg` IS DEAD.** t763's clamp had a
   recovery line, a test and a green gate, and could not fire on any real page. Audit the ~25-property
   recovery merge in `stylo_engine.rs` against the vendored parser's `cfg(feature = "gecko")` arms — this
   is a bounded, one-tick grep with a real chance of a second dead capability.

**Next check due: tick 771.**

## Check #62 — tick 772

**Date:** 2026-07-31. **Horizon:** H0, and the operative gate is not H0's four conditions in the
abstract — it is `DAILY-DRIVER-CERTIFICATION.md`'s per-origin certificate, which PART VII made the
near-term north star: **≥95% of in-scope CrUX origins pass both a FUNCTION gate and a VISUAL gate.**

### THE GATE, NAMED

Owner-locked 2026-07-30 (`PHASE0-MEASUREMENT-SYSTEM.md` §2): M1 and M2 are **two terms of one
function-gated number**, and **function leads** — a site must boot without an app-halting exception
before its shape is scorable at all. The binding quantity today is the **scorability ceiling**: 48 of
130 in-scope t767 origins do not render a real page, so even a perfect score on the 82 that do caps M1
at **63%**.

### GATE OR SCOREBOARD? — gate, and this window is the first that attacks the CEILING rather than the island

The last ~8 ticks (764–771) were: three RTL logical-axis fixes, a Bar-0 segfault fix (per-mutation
script compile), a hot-path grep, `box-sizing` on floats, a full sweep, and the parallel-sweep
throughput lever. Honest reading: **most of them optimised shape on the 82-site scored island, under a
cap none of them could move.** The sweep and the parallelisation are instrument work — necessary, and
t771 correctly *refused* its own faster number when the control showed parallelism costs scorability.
But the RTL arc crossed ~0 sites, exactly as `PHASE0-MEASUREMENT-SYSTEM.md` §7 predicted for
breadth-ranked work.

t772 is the first tick of the re-ranked leg, and it moved the thing the ranking says binds:
`coinmarketcap.com` crossed `render-failed` → **scored**, with the mutation restored to reverse it.

### PART VI CORRECTION

VI.2's `I5` row already records that **the instrumented log is the depth discovery engine**. This check
extends it with the sharper form the window produced:

> **The unscored-reason column of the sweep IS a ranked work-list, and it was being read as an
> exclusion list.** `render-failed` is the one reason the instrument itself flags as *ours*. Five of the
> t767 `render-failed` rows were the same bundle on five TLDs (`trivago.{be,de,fr,jp,pl}`) — a cluster
> visible by *reading the site names*, needing no oracle run at all. Rank the unscored rows by shared
> origin/bundle before ranking anything inside the scored island.

And the mechanism, which generalises past this API:

> **A HALF-INSTALLED API IS WORSE THAN AN ABSENT ONE.** Absence fails the feature-detect and routes the
> caller into the fallback its author wrote and tested. Half-presence *passes* the detect, the caller
> commits, and throws where nobody guarded. This codebase ships a large hand-written platform prelude
> built up one method at a time, so **the feature-detect surface and the call surface are different
> sets** throughout it. That is a standing audit target, not a one-site fix.

### INVARIANTS

- **I2 (never patch dependencies):** held — this is our own prelude, no vendored source touched, fork
  surface still empty.
- **I3 (semantic model in lockstep):** held. The fix makes pages *boot*; every element it recovers
  enters `node_rects` and the a11y tree by construction. `coinmarketcap.com` went from 2 to 380
  observable elements — that is the agent surface widening, not just the pixel one.
- **I4 (Pareto discipline):** held, and this is the strongest case in several windows. User Timing is
  Baseline-Widely-Available and instrumented by essentially every framework and RUM bundle — it is the
  opposite of the encoding tail. Two independent top-1k origins died on the same missing method.
- **I5 (the oracle/log is the discovery engine):** held and exercised — the fix came from one log line
  (`performance.clearMarks is not a function`), and the *second* rung (`navigationStart` resolving as a
  mark) was only visible after the first was fixed.
- **PART VII / V1-SCOPE:** honoured. No `scripts/` file authored.

**No invariant is being bent.**

### THE STEER

1. **KEEP WORKING THE UNSCORED LIST, RANKED BY SHARED BUNDLE — it is the ceiling.** Named and open right
   now, with the rung each one is stuck on: `trivago.{be,de,fr,jp,pl}` — failed dynamic `import()` of a
   relative module path (`./v/4.8.3/loader.polyfills…`) plus a 12s load budget exhausted at 23s wall;
   `pogoda.by` — Zone.js aborting because `Promise` was overwritten; `www.otomoto.pl` — still
   `render-failed`, uninvestigated. Each `render-failed` cleared raises the M1 cap **and** builds the M2
   function certificate for free.
2. **AUDIT THE PRELUDE FOR HALF-INSTALLED FAMILIES.** Bounded, one-tick, high-expected-yield: grep the
   prelude for objects assembled method-by-method (`performance` was one; candidates include
   `PerformanceObserver` — currently `observe`/`disconnect`/`takeRecords` no-ops with
   `supportedEntryTypes: []`, `navigator`, `screen`, `history`, `CSS`) and check each against its spec
   IDL block. The failure is silent until a real bundle calls the sibling nobody wrote.
3. **THE SWEEP CADENCE IS OWED.** The last honest burndown point is t767 (t771 is annotated
   CONTAMINATED). After ~5 more throw-killer fixes, run a full `corpus-crux-trend` sweep at
   `--jobs 2` and bank it — the scorability leg has no slope yet, and a leg with no slope is a
   hypothesis.

**Next check due: tick 780.**

---

## Check #63 — tick 780

**Horizon:** H0 / PART VII item 1 — *daily-driver rendering parity*, measured as `shape ≥ 0.75` on
≥95% of the in-scope CrUX corpus. **Gate:** the M1 leg of the owner-locked Phase-0 sequence.

**Gate or scoreboard?** ⚠ **This check exists to record that the gate has not moved in nineteen ticks,
and that the instrument said otherwise.**

The t777 sweep — the first cert-grade `--jobs 2` run since the function pivot — lands the leg's honest
trend next to its two honest predecessors:

```
tick    scored   pass   in-scope-pass   jarring-clean   M1 gate   shape_mean
t758      83      7        5.4%            23.8%         2.3%       41.3
t767      82      7        5.4%            25.4%         2.3%       43.5
t777      81      7        5.4%            24.0%         2.3%       46.3
```

Every crossing number is identical. The only thing that moved is `shape_mean`, +5.0 points. That is
`PHASE0-RENDER-BURNDOWN.md` §7's own predicted failure mode — *"the band rises faster than
crossings"* — now observed rather than forecast.

**And `fidelity-progress.sh` printed `+2.3 pts/sweep → ~39 more sweeps to 95%`**, because it diffs
against **t771**, the row whose own ledger header says it is contaminated. The honest slope is **0.0**.
Per I5 and the standing rule that a number's *name* is not its definition, the `+2.3` was refused
rather than banked, and the burndown carries no ETA right now. **A leg with a zero slope is not behind
schedule; it is un-ranked.**

### PART VI / VII CORRECTION — the ranking axis was wrong, and it was wrong in a knowable way

The previous check's steer (*"keep working the unscored list, ranked by shared bundle"*) was followed:
t772–t779 landed ~8 throw-killers, and they **worked** — `render-failed` 12→5, `timeout` 6→3, ~10 sites
that crashed at boot now boot. The leg's own metric still did not move, because the rescued sites
landed in `thin-overlap`: they boot and then render <20% of Chrome's DOM.

> **Clearing a boot throw is necessary and not sufficient. The function leg is a CHAIN, and its metric
> only moves when a site reaches the END of it.** Ranking by *the rung a site is stuck on* rewards
> clearing rung 1 on many sites; the metric pays only for sites that clear the last rung.

The complementary correction, on the render side, is that the loop had no crossing-ranked list at all:

> **Rank by MARGINAL CROSSINGS, not by cluster mass, band width, or mean shape.** Four fully-covered,
> jarring-clean sites sit within **0.06** of the bar with **fewer than 100 scored elements each**
> (`chat.google.com` +0.021, `kicktipp` +0.025, `255md.com` +0.052, `secure5.entertimeonline.com`
> +0.058) — roughly **eight mis-placed elements across three pages** would double the M1 gate. Banked
> as `PHASE0-RENDER-BURNDOWN.md` §8.

### ⚠ THE COHORT IS PARTLY AN ARTEFACT — measured this tick, and it invalidates part of t779

t779 split "booted-but-thin" into *timeout-starved* and *fast-but-empty*. Re-measuring the second group
against the sweep rows rather than the steer's site list breaks it:

- **`www.amazon.com.mx` was never in the cohort.** Its row reads `probe-blocked`, and a direct run today
  returns **HTTP 202 with a zero-byte body** — a bot wall, excluded by the NO-STEALTH policy. It was
  quoted in t779 as the flagship "fast but empty" case **because the steer's list named it and I did not
  check its row.** That claim is withdrawn.
- **`sports.yahoo.com` flips between runs**: `coverage 0.09` in the sweep, **0.97 (verdict `ok`)** on a
  direct run today, with the 12s budget exhausted six times in both. So **thin-overlap membership is a
  timing lottery for slow sites**, and any work-list built from one sweep's membership inherits that
  noise. *This is the same lesson as the ±40-pt per-site swings of t745-751, one label over.*
- **`www.naukri.com` is the one clean case** — 11.7s (under budget), no exhaustion, coverage 15.8%, 48
  missing elements, and the instrument names it itself: *"a coverage failure wearing an 'unscored'
  label"*. Its four unhandled rejections are a **permission-fingerprinting probe** (`clipboard`,
  `speaker`, `device-info`, `accessibility-events`), which Chrome largely rejects too — not a defect.

### INVARIANTS

- **I2 (never patch dependencies):** held — fork surface still empty; t777-t779 touched only our own
  prelude and bindings.
- **I3 (semantic model in lockstep):** held. t778's fix lets custom-element constructors complete, so
  every component element it recovers enters `node_rects` and the a11y tree by construction.
- **I4 (Pareto discipline):** held. Custom-element constructors and `permissions.query` are both
  Baseline-Widely-Available and on the boot path; neither is a tail item.
- **I5 (the oracle/log is the discovery engine):** held, and it paid twice. t778 came from a log line
  (`setting getter-only property "index"`, 18× on one site) and t779's whole measurement came from
  **mining the sweep log while the sweep was still running** — at zero CPU, on a box that could not be
  used for anything else.
- **PART VII / V1-SCOPE:** honoured. No `scripts/` file authored or edited in t777-t780.

**One invariant is being bent, and it is named rather than absorbed:** the standing rule that *"a fix
MUST raise in-scope-pass on the next sweep or it is reverted/re-scoped"* (`PHASE0-RENDER-BURNDOWN.md`
§4.3) has now failed nineteen consecutive ticks, and nothing was reverted. Reverting real, gated,
falsified fixes because a **badly-ranked** work-list did not move a metric would be the wrong reading —
but leaving the rule silently unenforced is how a ratchet becomes decoration. **It is re-scoped, not
dropped:** the rule binds from the next sweep onward, applied to ticks chosen off the §8
crossing-ranked list, where "did the named site cross" is a question the tick can actually answer.

### THE STEER

1. **TAKE THE §8 CROSSING-RANKED LIST, NOT THE CLUSTER LEDGER.** The next render tick starts at
   `secure5.entertimeonline.com`'s `<article>` — ours `[33 120 1134×452]`, Chrome `[0 120 487×354]`. We
   give a `width:auto` box the full containing block and centre it; Chrome shrink-wraps to 487. That is
   **min/max-content intrinsic sizing**, the step-function lever the board has carried for many ticks,
   now attached to named sites with per-element evidence. ⚠ The mechanism oracle **already prints
   per-element `[x y w×h] {font/size}` for both engines** — no instrument work is needed first.
2. **PERF IS NOW A FIDELITY BLOCKER, NOT A COMFORT METRIC — AND IT IS THE MAJORITY OF THE THIN COHORT.**
   trivago.de 25.7s vs Chrome 5.1s, trivago.be 25.5/5.7, monopolybingo 27.1/4.2, coinmarketcap
   38.8/11.7, sports.yahoo 41.0/24.3 — budget exhausted ~5× each, painting an unpopulated DOM. No API
   tick moves these. This is the one place where a load-path tick is squarely on-mandate.
3. **THE WRITE PATH IS AN UNAUDITED SURFACE.** t778's defect survived every existing gate because
   **every gate in this repo READS**. One bounded, high-expected-yield tick: enumerate the accessors
   installed on `Node.prototype` / `Document.prototype` and assert an *assignment* against each, the way
   `G_EXPANDO_READONLY` does for six. The same question generalises to the other access modes — lift-off
   -the-prototype, enumerate, delete.

**Next check due: tick 788.**

---

## Check #64 — tick 788 (2026-07-31)

**Horizon:** Phase-0 · M1 RENDER (`shape ≥ 0.75` **and** jarring-clean on ≥95% of the in-scope
representative CrUX corpus), owner-locked function-first, then M2 FUNCTION, then v1.0.0.

**Gate or scoreboard?** Gate, and for the first time in twenty ticks the gate MOVED for a reason the
loop can name. t786 banked the first clean `--jobs 2` sweep since t777:

```
                       t771    t777    t786
scored / in-scope       52      81     101   of 129     62.8% -> 78.3%
shape>=0.75 in-scope   2.0%    5.4%    7.8%
M1 (shape AND jarring)    ?    2.3%    3.9%
jarring-clean         17.6%   24.0%   26.4%
shape_mean             44.5    46.3    45.1
```

### THE STANDING RULE THAT HAD FAILED 19 TICKS IS SATISFIED THIS WINDOW

Check #63 recorded that `PHASE0-RENDER-BURNDOWN.md` §4.3 — *a fix MUST raise in-scope-pass on the next
sweep or it is reverted/re-scoped* — had gone unenforced for nineteen consecutive ticks, and re-scoped
it to bind from the next sweep. **It binds now, and it passes:** in-scope pass 5.4% → 7.8%, scorability
+15.5 points. The mechanism is nameable rather than atmospheric — `tree-divergence` rows 23 → 4,
`thin-overlap` 25 → 1, from the t784 key change.

⚠ **And the honest half: `shape_mean` FELL, 46.3 → 45.1.** That is the signature of a real conversion —
~20 sites entered the scored population near the bottom. A sweep where the mean had RISEN while twenty
new sites arrived would have been the suspicious one. The loop's own prediction, written before the
run, said so.

### WHAT THE LAST EIGHT TICKS ACTUALLY WERE

t781–t784 were four consecutive INSTRUMENT ticks, and the observer interrupted at t785 to demand a
measurement checkpoint before a fifth. **The interrupt was right and the ticks were right, which is the
uncomfortable combination.** Each keying fix was real and gated; what none of them could say was
whether it converted 20 rows or 2 — and answering that took 50 minutes, less than one of the ticks that
deferred it. Recorded as the rule: *a fix measured on the sites that motivated it is an anecdote until
the corpus prices it.*

The pivot the observer asked for then happened without further prompting: t785 (nested `@media`
declarations dropped whole), t787 (form-control intrinsic metrics) are ENGINE capability ticks, both
found by the same cheap move — **write a four-line fixture and ask Chrome for the number.**

### INVARIANTS

- **I2 (never patch dependencies):** held — the fork surface is still empty. t785 changed our own
  rule-index walker; t787 changed our own UA sheet and presentational-hint pass.
- **I3 (semantic model in lockstep):** held. Both fixes move BOX GEOMETRY only; no node appears or
  disappears, so `node_rects` and the a11y tree are unaffected by construction.
- **I4 (Pareto discipline):** held, and sharply. Nested group rules are in every stylesheet authored
  since ~2023; form controls are on every page with a form. Neither is a tail item.
- **I5 (the oracle/log is the discovery engine):** held — t787's whole work-list came from the fresh
  t786 rows plus the mechanism oracle's per-element `[x y w×h]` on the two named near-bar sites.
- **PART VII / V1-SCOPE:** honoured. No `scripts/` file authored or edited in t784–t788.

### ⚠ ONE TIER-0 ITEM HAS REGRESSED, AND IT IS NOT OURS TO FIX

`self-audit.sh` at t787: **the verify wall reads 881s against the 300s Tier-0 target.** It read 63s at
the start of this session; the difference is that these ticks touch `engine/css`, the shared-type edit
that cascades furthest, on a box whose disk is 93% full. Per PART VII the wall is observer-owned —
recorded here and in the journal, not touched. It costs ~15 minutes per tick, which is now the largest
single term in the cycle.

### THE STEER

1. **THE NEXT RENDER TICK IS THE `<select>` ARROW, AND ITS BLOCKER IS MEASURED.** A `<select>` is short
   by **exactly 17px** (142 vs 159 with a long option, 13 vs 30 with a one-character one) — the whole of
   `chat.google.com`'s form cluster, which is 3 of its 59 elements and would carry it across the M1 bar
   from 0.729. ⚠ It cannot be reserved unconditionally: with `appearance: none` Chrome drops to 139, so
   the reservation must be conditioned on the appearance value — and `clone_appearance()` is
   **gecko-only in stylo 0.19** (compile-probed this tick, `no method named clone_appearance`). It has
   to come through the `MinimalCascade` merge path, the same fence as `scrollbar-width` and
   `-webkit-line-clamp`. That is the shape of the tick: plumb one property, then reserve.
2. **`@layer` HAS NO PRECEDENCE** (audit #50, measured: Chrome 100, ours 333). Unlayered author
   declarations must beat layered ones regardless of document order. It needs a layer term in the
   cascade sort between origin and specificity — a real but bounded change to one comparator.
3. **JARRING-CLEAN IS NOW THE BINDING HALF OF M1, NOT SHAPE.** 26.4% clean against 7.8% shape-pass
   looks like shape is behind, but M1 needs BOTH at 95% and the jarring dimensions are
   reading-order 14.5%, overlap 20.0%, h-overflow 32.5%. Ranked by marginal crossings the next sweep
   should be read for *jarring-clean sites that are shape-short* (the §8 list) AND *shape-passing sites
   with exactly one dirty jarring dimension* — the second population has never been enumerated.

**Next check due: tick 796.**

---

## Check #65 — tick 796 (2026-07-31)

**Horizon:** Phase-0 · M1 RENDER, unchanged. Function-first stays owner-locked.

**Gate or scoreboard?** Gate — and this window is the first time the loop has priced a SINGLE change
against the corpus rather than a batch. t795 (the `inline-block` baseline) is the only engine change
between the t794 and t796 sweeps:

```
  paired over the 78 sites scored in BOTH sweeps:   mean Δshape +1.25 pts   ·  11 up, 4 down
  crossed the 0.75 shape bar:  marktplaats.nl 0.709→0.906 · chat.google.com 0.729→0.847
                               rpsc.rajasthan.gov.in 0.542→0.942            ·  crossed DOWN: none
  M1 gate (shape AND jarring-clean):  3 sites → 6
```

**One primitive was worth ~9× the seven-fix batch before it** (+1.25 pts against +0.14), and the
difference is not effort — it is that this one is on nearly every line of nearly every page.

### THE METHOD CORRECTION THAT CAME OUT OF t794 IS NOW IN THE INSTRUMENT

t794 measured that two sweeps four hours apart disagree about which sites are REACHABLE by five, worth
four points of headline before the engine is consulted, and banked the rule: *a sweep-to-sweep delta is
not a result unless it is PAIRED over the sites scored in both runs.* **The observer has since wired
exactly that into `fidelity-progress.sh` as the COMMON-SET BAND**, and it printed `+1.25 pts (78 sites
scored in BOTH) ← the REAL slope; ignore the pass-count sign when they disagree` on this very run —
where the unpaired pass count reads −0.1. The loop found the trap, the observer closed it in the
instrument, and the first run after that is the one that needed it. That is the intended division of
labour working.

### ⚠ THIS SWEEP IS PARTLY INVALID, AND THE REASON IS A HARNESS PARAMETER

37 rows read `crashed`, against **zero** in each of the two previous sweeps, and the readout duly
collapsed (scorability 76.4% → 55.6%, in-scope denominator 127 → 144). **No site crashes.** Every one
tested runs clean standalone — `payb.jp` scores 1.000, `sip777man.site` exits 0. The log gives the
mechanism outright:

```
  ⟳ chunk 1 exited early with 95 site(s) unrun — re-spawning (round 1)
  ⟳ chunk 1 exited early with 56 ...  47 ...  37 site(s) unrun — re-spawning (round 4)
  ⚠ chunk 1: 37 site(s) never produced a row after 4 rounds — filed as crashed
```

The per-site watchdog files its `timeout-150s` row and then `process::exit(0)`s — correct for a wedged
main thread — so **each slow site costs the chunk one of its four rounds**, and a chunk that meets four
slow sites files everything behind them as `crashed`. `CHUNK_ROUNDS = 4` is the parameter; `scripts/`
and the harness are observer-owned, so this is reported, not patched. **Read this sweep's paired band;
do not read its scorability.**

### INVARIANTS

- **I2 (never patch dependencies):** held — fork surface still empty across t789–t795.
- **I3 (semantic model in lockstep):** held, and asserted rather than assumed this window: t793's
  `order` gate checks the DOM's child sequence is untouched, because `order` is visual-only and
  reordering the tree would have rewritten what a screen reader announces.
- **I4 (Pareto discipline):** held. Every fix this window is on a primitive with corpus-wide reach —
  inline-block rows, floats, `order`, cascade layers, form controls, URL line breaking.
- **I5 (the oracle/log is the discovery engine):** held, with a sharpened form. Three of this window's
  findings came from a fixture built to ask a DIFFERENT question: nested `@layer` from a nesting
  control, the float clamp from a probe aimed at reading-order, and t795 — the largest single move in
  the session — from a sub-pixel probe whose own answer was "no bug".
- **PART VII / V1-SCOPE:** honoured. No `scripts/` file authored or edited in t789–t796.

### THE STEER

1. **THE PRIMITIVE-PROBE METHOD IS VINDICATED, BUT ONLY WHEN AIMED FROM THE NEAR-BAR LIST.** t794's
   verdict — seven primitive fixes, one crossing — and t795's — one primitive fix chosen by asking
   *what does `chat.google.com` need*, three crossings — are the same method with different aim. Keep
   starting from the §8 crossing-ranked list, then probe the primitive that site's divergence names.
2. **JARRING-CLEAN IS NOW THE BINDING HALF, HARDER THAN LAST CHECK.** shape ≥0.75 is 9 sites,
   jarring-clean is 26 of 144 (18.1%). The M1 gate needs BOTH at 95%. The three dimensions to work
   are reading-order, overlap and h-overflow, and t793 (`order`) is the first fix this loop has landed
   that targets one of them directly.
3. **THE 150s WATCHDOG COSTS A CHUNK A ROUND** (above). Until the observer re-parameterises it, prefer
   reading the paired band and treat any sweep with a non-zero `crashed` count as scorability-invalid.

**Next check due: tick 804.**

---

## Check #66 — tick 804 (2026-07-31)

**Horizon:** H0 — Pareto Web Parity. **Gate:** M1 RENDER — `shape >= 0.75` AND jarring-clean on
>= 95% of the in-scope CrUX corpus (`scripts/phase0-milestones.sh`).

### GATE OR SCOREBOARD? — GATE, and for the first time this window it MOVED BY CROSSINGS

The t796 check closed with the steer *"aim from the §8 crossing-ranked list, then probe the primitive
that site's divergence names"*. Eight ticks later that is exactly what the window did, and it is the
first window in this burndown's history to produce **two M1 crossings from two aimed fixes**:

```
  t799  anonymous block inherits (align + strut)   linkmake.in       0.622 → 0.703
  t800  MEASURE — clean --jobs2 sweep banked        (the checkpoint the board had asked for 3×)
  t801  max-width re-runs the auto-margin split     255md.com form   x 309 → 400 (masked)
  t802  a control does not inherit line-height      255md.com        0.721 → 0.767   ★ CROSSES
  t803  a text node is never out of flow            dapam-sirius.fr  0.633 → 0.800   ★ CROSSES
  t804  flex abspos item — REFUSED (this check)
```

M1 across the three banked sweeps: **2.4% → 4.2% → 5.5%**, in-scope shape-pass **6.3% → 6.2% → 8.3%**.
The band the t796 check called "the binding half" (jarring-clean, 18.1%) is unchanged at 17.2% — the
crossings came from sites that were *already* jarring-clean and short on shape, which is precisely the
§8 ranking's prediction. **The method is no longer a hypothesis.**

### THE INSTRUMENT CORRECTION OF THIS WINDOW — pairing the site list is not pairing the SITE

t794 banked *"a sweep-to-sweep delta is not a result unless it is PAIRED"* and the observer wired the
COMMON-SET BAND into `fidelity-progress.sh`. At t800 that band read **−0.35 pts (9 sites down, 2 up)**
and the loop treated it as a regression, as the ratchet requires. **Rebuilding `engine/` at the exact
tree behind the previous sweep reproduced TODAY's numbers, not the previous sweep's** — `nysainfo.pl`
0.678 in the band, 0.562 on the old binary run today. The band over live pages sums the engine's delta
with the WEB's and publishes both under the engine's name.

**Banked as a mechanism, and used twice since:** when a control moves, rebuild the old binary in the
same window before believing it. t803 did exactly that (wikipedia −2/1101 → confirmed as ours, landed
with the cause named) and t804 did it again (wikipedia −9/1074 → confirmed as ours, **refused**). It
cost three minutes each time and it changed the decision both times.

### ⚠ THE REFUSED TRADE, recorded because refusing is the harder half of the ratchet

t803 exposed a genuine defect it had been hiding: `taffy_tree::flex_items` pushes every element child
into the item list, where **Flexbox §4.1 says an absolutely-positioned child is not a flex item and
does not contribute to the container's size**. A six-case fixture confirmed it — a `width:fit-content`
flex row holding `ab` and a 100px abspos label is **18×20** in Chrome and **18×100** here — and
excluding out-of-flow items from the container's height made all six containers and all four children
Chrome-exact.

**On `en.wikipedia.org` it cost nine elements of 1074 (0.593 → 0.585), with no site crossing and no
mechanism connecting the fixture to the loss.** Five other controls were byte-identical. Spec-correct,
fixture-exact, and refused — the same verdict as the t695 fix that was 8/8 on its own fixture and
regressed its control. **A change whose blast radius on a real page is not understood is not a fix
yet, however right the specification is.** The fixture and the spec clause are banked in the journal
so the next attempt starts from the answer rather than the question.

### INVARIANTS

- **I2 (never patch dependencies):** held — fork surface still empty across t797–t804.
- **I3 (semantic model in lockstep):** held. t803's fix gives boxes to content that previously had
  none, which strictly improves what the a11y tree and hit-test can see; nothing was removed.
- **I4 (Pareto discipline):** held, and unusually literally. Every fix this window is a rule that runs
  on a large fraction of all pages: `.container { max-width; margin:0 auto }`, `body { line-height }`
  reaching a form control, a centred section with a block child, and `<div style="position:absolute">`
  around bare text. None was chosen for its WPT mass.
- **I5 (the log/oracle is the discovery engine):** held, and sharpened again. **t803's probe returned
  "no bug" on the question it was asked** — shrink-to-fit is Chrome-exact in eight of nine contexts,
  retiring the min-content hypothesis kicktipp suggested — and the ninth context was the finding. That
  is the third window running in which the largest result came from a fixture built to ask something
  else.
- **PART VII / V1-SCOPE:** honoured. No `scripts/` file authored or edited in t797–t804. Two harness
  problems were reported and not patched (below).

### PART VI CORRECTION — what is now the real blocker

The **scorability ceiling is unchanged and is still the cap**: 77/145 in-scope sites render, so M1
cannot exceed ~53% however good the layout gets. Shape work is buying real crossings inside that
ceiling and should continue while the near-bar list has jarring-clean sites within 0.06 — there are
still four (`kicktipp` +0.025, `gismart` +0.042, `linkmake` +0.047, `ikea` +0.052). But the arithmetic
has not changed since the t777 board block: **the function leg raises the cap and nothing else does.**

### HARNESS, reported not patched (PART VII)

1. **`CHUNK_ROUNDS = 4` again invalidated a sweep's scorability.** t800 filed **48** rows as `crashed`;
   the log states the mechanism outright (`chunk 1 exited early with 70 … 56 … 47 site(s) unrun`, then
   `47 site(s) never produced a row after 4 rounds — filed as crashed`). No site crashes. Second
   consecutive check-in reporting this; the paired band remains the only readable half.
2. **The self-audit's wall figure reads a stale receipt.** `verify wall: 1523s` comes from
   `.git/manuk-verify-receipt`, whose row carried `unattributed_seconds: 1523` and `load1: 7.59` — a
   contended box — against `LAST_WALL_TIME: 63s` in STATUS.md. Recorded at t799 and unchanged.

### THE STEER

1. **KEEP AIMING FROM THE §8 LIST — it is now measured, not argued.** Two aimed fixes, two crossings,
   in five ticks. The four remaining jarring-clean near-bar sites are the next four targets.
2. **RE-RUN THE OLD BINARY WHENEVER A CONTROL MOVES.** Three minutes, and it changed the verdict twice
   in this window — once to land (t803) and once to refuse (t804). It is now part of the tick, not a
   special measure.
3. **THE FLEX/ABSPOS FIX IS OPEN, WITH ITS FIXTURE BANKED.** Before retrying it, explain the nine
   wikipedia elements — the readable one is a 32×32 hamburger button becoming 100×36, which is a WIDTH
   change from a HEIGHT-only edit and therefore a coupling nobody has traced.

**Next check due: tick 812.**

---

## Check #67 — tick 812 (2026-08-01)

**Horizon:** H0 — Pareto Web Parity. **Gate:** M1 RENDER — `shape >= 0.75` AND jarring-clean on
>= 95% of the in-scope CrUX corpus.

### GATE OR SCOREBOARD? — GATE, and this window it also caught the loop cheating itself twice

Eight ticks since #66. Four engine fixes, one measurement tick, one sweep, one refusal, one revert:

```
  t805  text-align:justify            t806  letter-spacing on the space
  t807  MEASURE — M1 5.5% → 8.4%      t808  inline vertical padding   ★ linkmake.in CROSSES
  t809  unrendered ≠ display:none     t810  MEASURE — BFC/float specified
  t811  BFC avoids floats             t812  MEASURE — and t811 REVERTED (this check)
```

M1 across five sweeps: **2.4 → 4.2 → 5.5 → 8.4 → 8.0%**. The last step is a denominator move, not a
loss: the M1 COUNT is **11 in both** t807 and t812; in-scope went 131 → 138.

### ⚠⚠⚠ TWO SELF-CORRECTIONS THIS WINDOW, AND BOTH CAME FROM THE SAME PROCEDURE

1. **`mobcup.fm` did not cross.** t809 measured it at **0.909** and banked a crossing. The t812
   sweep reads **0.727**, and the old-binary control shows the ENGINE contribution is real but small
   (0.903 → 0.909, plus coverage 0.912 → 0.971). The 0.909 spot reading and the 0.727 sweep reading
   are the same binary on a dynamic media page. **A single spot measurement of a live site is not a
   crossing** — `linkmake.in` (t808) IS confirmed in the sweep at 0.703 → 0.757 with `n=74` both, and
   that is what a claimed crossing has to look like.
2. **t811 is REVERTED.** It was spec-correct, 7-of-8 Chrome-exact, and landed with **nine controls
   byte-identical** — and it costs `www.ta3lemkonline.com` **26 elements of 457** (0.540 → 0.484,
   same `n`, old binary rebuilt). Bisected to t811 exactly: the t809 tree reads 0.540481, the t811
   tree reads 0.483589.

**The rule the second one proves is not "test more controls".** It is that *nine byte-identical
controls is not evidence of no regression* — it is evidence about those nine sites. t804 refused a
change on this basis; t811 passed the same bar and failed anyway, on a site nobody had chosen.

### PART VI CORRECTION — the scorability ceiling is now the honest blocker, and the harness owns it

`scored 87/138 = 63.0%` — but the sweep filed **25** rows as `crashed`, and t800 filed 48 and t807
filed 0. **The scorability series (53% → 79% → 63%) is a measurement of `CHUNK_ROUNDS`, not of the
engine**, and no scorability claim should be read across these three sweeps. The shape half is paired
and readable; it is the only half this loop can currently act on.

### INVARIANTS

- **I2:** held — fork surface still empty.
- **I3:** held. t808 gives boxes their true height and t809 gives four elements their true computed
  `display`; both strictly improve what JS and the a11y tree can see.
- **I4:** held. Every fix this window is a rule that runs on a large fraction of all pages —
  justified text, tracked runs, padded inline links, `<picture><source>`.
- **I5:** held, and the negative results are the evidence. Two probes at `kicktipp` retired two
  hypotheses (min-content, then `nowrap` in 8 of 9 contexts) and found two unrelated defects in their
  ninth rows. **The ninth row is where a wide fixture pays.**
- **PART VII:** honoured. No `scripts/` file touched in t805–t812. Three harness items reported.

### HARNESS, reported not patched (PART VII)

1. **`CHUNK_ROUNDS = 4` has now invalidated three of five sweeps' scorability** (t796, t800 48 rows,
   t812 25 rows; t807 was clean). This is the third consecutive check-in reporting it.
2. **The self-audit's wall figure reads a stale contended receipt** (1192s against `LAST_WALL_TIME:
   63s`). Second audit running.
3. **`/home` reached 96% during a tick**; `tick.sh`'s own reclaim step handled it.

### THE STEER

1. **A CROSSING IS CLAIMED FROM A SWEEP, NOT FROM A SPOT CHECK.** `mobcup.fm` cost this loop a false
   claim in a journal entry. A spot measurement aims the next tick; only a paired sweep row banks one.
2. **THE CONTROL SET IS A SAMPLE, AND t811 IS THE PROOF.** For any change touching block placement or
   sizing, measure a *hostile* site as well as the friendly nine — `www.ta3lemkonline.com`
   (`reading_order` 816) is now named for exactly that.
3. **THE BFC/FLOAT WORK IS STILL RIGHT AND IS STILL OPEN** — its fixture, its Chrome table and its
   spec clause are banked twice over (t810, t811). What it lacks is an account of why a float-band
   narrowing costs a float-heavy page 26 elements, and that is the next attempt's first question.

**Next check due: tick 820.**

## Check #68 — tick 820 (2026-08-01)

**Horizon:** H0 — Pareto Web Parity. **Gate:** M1 RENDER — `shape >= 0.75` AND jarring-clean on
>= 95% of the in-scope CrUX corpus.

### GATE OR SCOREBOARD? — GATE, but this window it is UNPRICED and that has to be said first

Eight ticks since #67. Four engine fixes, two measurement ticks, one audit tick, and **no sweep**:

```
  t813  MEASURE — bisecting across TREES     t814  MEASURE — rowless table specified
  t815  rowless display:table                t816  orphan table-cell is ATOMIC
  t817  flex % line-break (Bootstrap)        t818  AUDIT — surface #53 + wall #27
  t819  flex-basis is a main size too, and t817's own residue label was WRONG
```

**M1 is unchanged at 8.0% because nothing has re-measured it.** The last sweep is `SWEEP-t812-rows`;
four engine fixes now sit unpriced. That is *inside* the board's own cadence (sweep after ~5–6) but it
is the honest headline: **this window produced no M1 movement claim, and none should be read into
it.** Every tick here says "no site is claimed to cross" in its own journal entry, and the
corpus price is genuinely owed. A sweep is the next non-fix unit.

### ⚠⚠⚠ THE SELF-CORRECTION THIS WINDOW WAS OF PROSE, NOT OF CODE — A NEW SHAPE

#67's two self-corrections were both *measurements* withdrawn (`mobcup.fm`'s crossing, t811's fix).
t819's is different in kind: **t817's FIX was correct and its LABEL was wrong.** t817 shipped, in
three files, "Bootstrap 4's `533`/`133` is the percentage applied twice — a flex-BASIS defect". Drop
the `max-width` and the same row is exactly `800`/`400`: the basis was never the culprit.

**The blast-radius asymmetry is the finding.** A wrong *fix* is caught by the gate that follows it. A
wrong *label* is caught by nothing — it is prose, it passes every test, it survives every sweep, and
it aims the next tick at the wrong organ. t814 established "a residue's stated cause is a guess until
measured on its own"; t815 and t816 honoured it by asserting residues at OUR numbers; and t817 still
wrote a causal claim from **one fixture that had both suspect properties set together**. *A residue
measured with two properties present names neither.* The guard cost two fixture rows.

### ⚠⚠ THE OLD-BINARY CONTROL EARNED ITS KEEP AGAIN, IN THE OPPOSITE DIRECTION

t819: `en.wikipedia.org` read **1017** misplaced at t816 and **1020** at t819 — and the **old binary
also reads 1020**. #67 used the control to *withdraw* a claim; here it *prevented a regression hunt
against my own change*. Same procedure, opposite service. It is now unconditional for any tick that
touches placement or sizing, and it costs one rebuild.

### INVARIANTS

- **I2 (never patched internally):** held, and tested this window. t817's defect is *inside* taffy
  (`flexbox.rs:930` breaks flex lines on a bare `>`). The obvious move was a `[patch.crates-io]`
  fork; it was refused. The quantisation went on **our** side of the boundary instead
  (`snap_row_item_percent_widths`), where `solve_subtree` already knows the container width. Fork
  surface still **empty**.
- **I3:** held. t815/t816 give boxes that did not exist, or existed at the wrong size, their true
  geometry — strictly more for `getBoundingClientRect` and the AX tree to see.
- **I4 (Pareto discipline):** held emphatically. t817 is Bootstrap's grid; t816 is the pre-flexbox
  `display:table-cell` vocabulary; t815 is `display:table` as a layout tool. All three are
  *representative real-web breadth*, none is a Chromium quirk or a tail completeness chase.
- **I5 (the oracle is the discovery engine):** held, and refined. Every fix this window came from a
  small fixture diffed against headless Chrome. t817 adds the aiming chain: **near-bar sweep row →
  composite screenshot names the organ → Chrome's own computed style names the framework.** Three
  cheap steps, no guessing, and it found a defect on the single most common layout idiom there is.
- **PART VII:** honoured. No `scripts/` file edited in t813–t820. Three harness items reported below.

### HARNESS, reported not patched (PART VII)

1. **Wall audit #27: 1216s of 1661s (73%) UNATTRIBUTED** — 58% at #26, named by subtraction at #25.
   **Three consecutive audits, same subtraction, no new information.** The audit's four questions are
   each *about a named section*, so no aimed remedy exists for anyone while the largest cost is
   unlabelled. Instrument the remainder before hunting bloat in the labelled 27%.
2. **`LAST_WALL_AUDIT` cannot be cleared in one pass.** It is derived by `status-update.sh` from
   `WALL-AUDIT.md`'s `## Audit #N — tick <N>` headers, but `tick.sh`'s preflight reads the
   *already-generated* STATUS.md. Running the audit and writing its ledger entry is not enough;
   `status-update.sh` must run in between. A hand edit does not survive. Cost three blocked launches.
3. **`F2 pipeline large/mid` went RED from its DENOMINATOR** — 8.11x against a 7.5x bar with `large`
   *unchanged* (233.92 → 232.72 ms) and `mid` **17% faster** (34.75 → 28.68 ms); re-runs read 6.60x,
   6.00x, 6.80x. A ratio gate divides out machine speed only when both legs move together. **Re-run,
   never retune** — nothing was touched.

### THE STEER

1. **SWEEP NEXT.** Four unpriced fixes, one of them (t817) on Bootstrap's grid — the widest-reach
   change this loop has landed in some time. The burndown has no slope until it is measured, and
   every entry this window explicitly deferred its corpus price to it.
2. **THE AIMING CHAIN IS THE METHOD NOW.** near-bar sweep row → composite → Chrome computed style →
   4-line fixture. It is cheaper than reading CLUSTERS.md and it found the Bootstrap defect.
3. **A RESIDUE IS MEASURED WITH ONE SUSPECT AT A TIME, OR IT IS NOT MEASURED.** And the open one is
   named precisely: `max-width: <pct>` on a flex item resolves against the item's **own**
   taffy-assigned width instead of its containing block (`800 × 0.666667 = 533`) — the documented
   `taffy_item_height` shape appearing on the width axis. It is a subsystem-scale change (every
   flex/grid item with min/max-width) and wants a sweep on both sides of it, not a tail-of-session fix.

**Next check due: tick 828.**

## Check #69 — tick 828 (2026-08-01)

TICK SHAPE: process — the constitution re-read due at 828, against CONSTITUTION.MD

VERDICT: **GATE, AND PRICED — the first window in twelve ticks that can say that.** Five ticks landed
(t823-t827), every one green on the full wall. The window's headline is not a rendering fix: it is
that **the burndown has a slope again**. `M1 8.0% → 10.0%`, count **11 → 13 sites**, jarring-clean
29 → 36, on the first clean 200-of-200 sweep since t812.

⚠⚠⚠ **THE HIGHEST-LEVERAGE TICK OF THE WINDOW CHANGED NO RENDERING CODE, AND THAT IS THE FINDING.**
The board said "MEASURE NOW" for ~12 ticks. Three sessions obeyed it *literally* — ran the sweep,
found it contaminated, refused it honestly, and moved on to engine work. Each refusal was correct in
isolation and the loop still went blind for twelve ticks. **A measurement that has failed three times
is a capability gap, not a chore to retry.** t824 treated the contamination as the tick; one
arithmetic fix (a constant re-spawn cap where the work is a variable) turned a 12-tick blind spot into
a 40-minute, 200-of-200 number that priced five fixes at once.

⚠⚠ **AND THE THING THAT BLINDED IT WAS A LABEL, WHICH IS #68'S OWN LESSON ARRIVING FROM OUTSIDE THE
ENGINE.** #68 recorded *"a wrong FIX is caught by the next gate; a wrong LABEL is caught by NOTHING."*
It said that about prose in a wiki. Here the wrong label was a **string in a data column**: sites the
instrument never opened were filed `crashed`, which is a Bar-0 word, and two sessions read the
teardown message printed beside it as a mozjs crash. The label survived three sweeps, passed every
test, and aimed two ticks at an engine that was not faulting. `Unmeasurable::NeverRan` exists so those
two events can never share a string again.

INVARIANTS: **I2 held.** t823/t827 are taffy's boundary, and the fix went on OUR side of it both
times — we stopped re-computing what taffy had already computed rather than patching taffy's
resolver. **Fork surface still empty.** **I3 held** — t823 gives every margined flex item and every
percentage-clamped column its true geometry; strictly more for `getBoundingClientRect` and the AX tree
to see. **I4 held emphatically**: Bootstrap 4's grid column is `flex: 0 0 X%; max-width: X%`, and it
rendered at two thirds of its width on every Bootstrap-4 page on the web. **I5 held, and was refined
by a failure** — t826's reduction came from the aiming chain, reproduced perfectly, and was *wrong*,
because the cheap harness runs `MinimalCascade` and the browser runs Stylo. **A reduction is not
confirmed until it has run on the SHIPPING cascade.** **PART VII honoured**: no `scripts/` file edited
in t823-t827; three harness items reported below. The one instrument change (t824) is in `tests/wpt`,
which the board names as agent territory.

### HARNESS, reported not patched (PART VII)

1. **`manuk-wpt` is in NEITHER the wall's crate-test list nor CI's.** Both run the same seven
   (`css layout paint dom net agent shell`). So the crate that produces the Phase-0 headline is the
   one crate no lane tests, and t824's `chunk_spawn_budget` gate — real, RED-proven — runs only by
   hand. A one-word change in two observer-owned files.
2. **`fidelity-progress.sh`'s `EXCLUDED-RISING` alert diffed against the t820 row that the file
   itself annotates as CONTAMINATED.** The annotation is a comment line the script does not read, so
   the alert's magnitude (25 → 70) is an artefact; against t812 the real move is 62 → 70.
3. **The verify wall is 776s against the 300s target** — the self-audit's single ✗. Also: `F2` went
   red at 7.84x on a loaded box and read **5.82x** on a settled one, on a docs-only tree. Re-run,
   never retune — the second window running.

### THE STEER

1. **AIM FROM THE BANKED COHORTS, DO NOT RE-DERIVE THEM.** t826's entry holds both M1-crossing lists
   off the fresh sweep: ten jarring-clean sites inside 0.20 of the bar (three inside **0.022**), and
   four already over the shape bar held out by **one jarring dimension each**. That second cohort is
   the cheaper of the two and has never been worked.
2. **THE NEXT REDUCTION RUNS ON `Page::load`.** Not negotiable after t826 — the cheap harness
   proposes, the shipping cascade disposes, and the failure mode is a fixture that reproduces the
   right number for the wrong reason.
3. **ONE OPEN ROW IS NAMED AND OWES A CONTROL:** `www.freesupertips.com` 0.7637 → 0.6674 at near-flat
   coverage, the only t825 crossing-down not explained by a coverage rise. Measure it with the
   old-binary control before it becomes a story.

**Next check due: tick 836.**

---

## Check #70 — tick 836 (2026-08-01)

HORIZON: **H0 — Pareto Web Parity.** Exit gate, all binary: ~83% WPT across categories · differential-
oracle viability across all four usage-weighted corpora · a daily-drivable headful shell · every
rendered construct queryable through the in-process semantic API.

GATE OR SCOREBOARD? **GATE, and the strongest window in the burndown's history — with one caveat
that belongs in the same sentence.** Seven ticks landed (t830-t836). The gate condition they serve is
the second one (oracle-verified viability), whose operational form is the in-scope shape bar:

```
                              t820     t825     t832
  in-scope shape>=0.75        5.7%    13.1%    19.1%
  M1 GATE (shape AND jarring)  4.0%    10.0%    13.0%
```

**+6.0 points of gate condition in one sweep window**, the largest yet, and every point of it is
layout math — H0 scope item 1, the item the constitution calls *"the single highest-leverage
architectural decision in the renderer."*

⚠ **THE CAVEAT, STATED HERE RATHER THAN IN A FOOTNOTE: SCORABILITY IS FLAT AT 101/131 = 77.1%, AND
THAT IS THE REAL CEILING.** Six render fixes moved shape and did not move the count of sites that
render at all. M1 cannot reach 95% while 30 in-scope sites never boot. The render leg is being worked
because it is the binding constraint *on the sites that score*; the FUNCTION leg is the binding
constraint on the bar itself, and no tick in this window touched it. That is not drift — the board's
CO-#1 says render — but the next check must ask whether the render leg has taken enough ground that
the ceiling deserves a window of its own.

WHAT THIS WINDOW ACTUALLY FOUND — **one mechanism class, four implementations of it.** Every one of
t831/t833/t834/t835 is the same sentence: *this engine resolves a replaced element's size in more
than one place, and the places disagree.*

```
  layout_float   no intrinsic ratio · no min/max at all · box-sizing on one axis     t831  ✗→✓
  layout_block   §10.4 ran inline→block only, never block→inline                     t833  ✗→✓
  layout_abs     no ratio at all: an abspos <img> was ZERO PIXELS TALL, always       t834  ✗→✓
  taffy leaf     <img> off the replaced-size list: content measured as ZERO          t835  ✗→✓
  taffy line     cross size from UNFLEXED main size                                  t836  REFUSED
```

The window's method is worth recording separately from its result: **t833 concluded "the grep is
symmetric or it is not a grep", and t834 executed it rather than filing it.** That single act of
taking a recorded lesson literally found the worst defect of the five — an absolutely positioned
image rendering at zero height on every page that uses `top/left` instead of `inset:0`.

INVARIANTS.
* **I2 held, and was tested hardest this window.** Four of the five defects are at taffy's boundary
  and every fix landed on OUR side of it — we stopped discarding, mis-guarding or overwriting what
  taffy had computed. t836 is the case that proves the invariant is real rather than convenient: the
  remaining defect is *inside* taffy's line-cross-size computation, the local patch was measured to
  encode a false rule, and the tick **refused it** rather than reaching across the boundary or
  shipping an approximation with a confident comment.
* **I4 held emphatically.** `float:left` on a header logo, `max-width:100%` + `max-height` on a
  Cognito login page, `display:flex; overflow-x:scroll` carousels — this is representative-web
  breadth, not tail. Nothing in the window chased a Chromium quirk.
* **I3 held.** Every fix produces a truer box, which is what `getBoundingClientRect` and the AX tree
  read; no new construct was added that lacks semantic exposure.
* **I5 held and did more than usual.** The oracle stopped being only a ranker this window: the new
  `--shape-dump` publishes the per-element misses `shape_stats` was already computing and throwing
  away, and it aimed t831 (in one command), t833, and t835.
* **PART VII honoured** — no `scripts/` file touched in t830-t836.

PART VI CORRECTION. §VI.2 still reads *"CSS layout breadth is the weak spot: css-flexbox 5.5%,
css-grid 4.7%."* That framing is now **stale in its instrument, not in its conclusion**: the
conclusion (layout is the weak spot) is exactly what this window confirmed, but the WPT percentages
are no longer how the loop measures it. The live gauge is the in-scope shape bar on the CrUX corpus
(19.1%, +6.0/window) plus the scorability ceiling (77.1%), both of which move on a sweep and neither
of which appears in Part VI. Part VI should carry those two numbers.

### HARNESS, reported not patched (PART VII)

1. **`F2` went RED at 7.70x on a loaded box and passed on a quiet re-run of the identical tree.**
   Third window running. The denominator is what moves: `mid` read 34.35ms on the red run, the
   fastest all session, which inflates a ratio whose numerator did not change. Re-run, never retune —
   but a gate that reddens from its own denominator on a busy box is costing a full wall each time.
2. **Swap sat at 94-99% for the whole window** while RAM was ~17GB free; `tick.sh` prints the warning
   itself. It is stale pages from an earlier spike, and it is the likeliest cause of item 1.
3. **`manuk-wpt` is still in neither the wall's crate-test list nor CI's** (reported at #69,
   unchanged). The `fidelity::shape_tests` added at t831 — including the invariant that the miss dump
   and the score are the same walk — run only by hand.

### THE STEER

1. **THE SWEEP IS THE ONLY THING THAT FINDS A REGRESSION, AND THIS WINDOW PROVED IT TWICE.** t832's
   sweep found three real regressions that t830's and t831's honest 14- and 16-site controls could
   not see, because the sites a fix costs are the ones you were not thinking about. Then t835's
   control found a regression *the fixture could not*, because the corrective guard had been written
   from the same hypothesis as the fix. **Keep both instruments; they fail in different directions.**
2. **NEXT REDUCTION AIMS WITH `--shape-dump`, NOT BY HAND.** It has now aimed three ticks in one
   command each, against the same frame the score is computed in. The hand-rolled `boxes` + headless-
   Chrome diff that every reduction used before t831 is retired for this purpose.
3. **THE OPEN RESIDUE IS NAMED AND ONE LAYER UP:** taffy's flex-line cross size is computed from
   unflexed main sizes, so a shrunk image gets its unflexed height. Refuted this window: the measure
   seam (t835), `align-items` (t835), the slot adoption (t836, counterexample = an image beside a
   taller sibling, which Chrome stretches to 120). **Anything that touches one item cannot fix a
   number that belongs to the line.**
4. **AND THE CEILING IS THE NEXT QUESTION, NOT THE NEXT TICK.** 30 in-scope sites do not render at
   all. Render work still pays — +6.0 points in one window — but the arithmetic of 101/131 does not
   move until something boots them.

**Next check due: tick 844.**

## Check #71 — tick 844 (2026-08-02)

**HORIZON: H0 (Pareto Web Parity), re-scoped by PART VII leg 1 — "daily-driver rendering parity,
breadth-first, usage-weighted; the bar is *reliably renders and runs the representative real
internet*, NOT a WPT percentage."** The loop's operational form of that gate is M1: `shape≥0.75 AND
jarring-clean` on the in-scope CrUX corpus, target 95%.

**GATE OR SCOREBOARD? — GATE, and this is the least ambiguous window in a long time.** Eight ticks,
four of them fixes, every one priced against the representative corpus and none against a WPT count:

```
  t836  measure   flex-line cross size — fix REFUSED (encoded a rule the tick had measured FALSE)
  t837  fix       %-height on an out-of-flow box resolved against the DOCUMENT, not the viewport
  t838  measure   a 1×1 lazy placeholder is a WRONG RATIO, not a missing image
  t839  fix       IntersectionObserver.observe() never delivered its INITIAL observation
  t840  measure   777juegos — two obvious mechanisms REFUTED, banked as refutations
  t841  fix       UAX #9 rule L2 over a line's INLINE BOXES (bidi was glyph-level only)
  t842  measure   clean --jobs 2 sweep: M1 13.0% → 14.6%
  t843  fix       a position:relative row inside an out-of-flow subtree was invisible as a CB
```

**M1 13.0% → 16.2%** across the window (17 → 21 of 130 in-scope sites), scorability 77.1% → 79.2%.
Zero WPT-flip ticks. `WPT:TOTAL` stayed a bookkeeping mark exactly as §VI.3 requires.

### INVARIANTS

* **I2 (never patch deps) held.** t841 consumed `unicode-bidi` as a library and added a public helper
  in `engine/text` rather than a second bidi implementation in `engine/layout` — one bidi for the
  engine, so the glyph order and the box order cannot disagree. The fork surface is still empty.
* **I4 (Pareto discipline) held — and the window produced a CORRECTION to how the loop applies it.**
  The board steered *against* t841 in advance: *"the RTL arc crossed ~0 because corpus-crux-trend is
  RTL-light… rank by corpus-sample frequency."* The corpus IS RTL-light — 3 of 200 carry
  `<html dir=rtl>` — and t842 then measured that t841 produced **the window's only clean crossing and
  all of its M1 movement**. I4 says weight by *actual usage*; it does not say weight by
  *instances-in-this-sample*. **A corpus with few instances of a defect can still have that defect's
  METRIC TERM as its binding constraint**, and Arabic/Hebrew/Persian/Urdu is a population, not a
  tail — exactly the kind of thing a CrUX sample of the English-reading web under-counts.
* **I3 (semantic model in lockstep) — BENT, and this is the finding of the check.** Both fixes that
  moved M1 this window moved it through `reading_order`, and `reading_order` **is a semantic-model
  property wearing a visual property's clothes**: it is the order an agent, or a screen reader, walks
  the page in. t841's RTL fix meant every RTL page's navigation had been read backwards by
  `manuk-agent`, and t843's meant every drawer row's caret was reported at one shared position. The
  loop has been treating reading-order as a rendering term because that is the column it lives in.
  **Neither tick asserted its `manuk-a11y` / `manuk-agent` exposure, and there is no gate anywhere
  that says the semantic tree's order matches Chrome's.** I3 says every renderer subsystem lands
  with its semantic-model exposure; here the exposure and the renderer are the same number, and only
  the renderer half is gated.
* **I5 (oracle is the discovery engine) held, and the aiming chain is now three links.** t842's sweep
  ranked the cohort → `--shape-dump` + the root-cause table named the element → a 12-line reduction
  from the site's own stylesheets named the mechanism. t843 went from "which site" to "which line of
  CSS" inside one tick.
* **PART VII honoured** — no `scripts/` file touched in t836-t843.

### PART VI CORRECTION

§VI.2 still reads *"CSS layout breadth is the weak spot: css-flexbox 5.5%, css-grid 4.7%."*
Check #70 already flagged the instrument as stale; this window makes the correction concrete, and it
is not only a change of units:

> **§VI.2 should read: the H0 render gauge is M1 on the in-scope CrUX corpus — `shape≥0.75` AND
> `jarring-clean` — currently 16.2% (21/130), against a scorability ceiling of 79.2% (103/130). M1 is
> a CONJUNCTION, and `docs/loop/PHASE0-RENDER-BURNDOWN.md` §3 ranks mechanisms for only one of its
> two conjuncts.** Nine ticks of shape work moved `shape≥0.75` by zero net sites and the common-set
> band by −0.26 points; two fixes to the jarring conjunct moved the gate by 3.2 points.
> `reading_order` is non-zero on 5 of the 6 sites in the current near-bar table and on 5 of the 6
> sites that are already `shape≥0.75` and fail M1 on jarring alone.

### HARNESS, reported not patched (PART VII)

1. **`manuk-wpt` is still in neither the wall's crate-test list nor CI's** — reported at #69 and #70,
   unchanged. `fidelity::shape_tests` still runs only by hand.
2. **The wall ran 1049s and 1081s on the two fix ticks this window** against 79-81s on the two
   measurement ticks (docs-only, which skip the receipt). Not a blocker and not mine to change; noted
   because the fix/measure alternation makes the average look better than the fix-tick cost is.
3. No false-RED this window: `F2` read 5.53x, 6.73x and passed on quiet boxes. The #70 item is not
   recurring right now.

### THE STEER

1. **RANK BY WHAT THE GATE IS A CONJUNCTION OF.** The next render ticks come from the cohort t842
   exposed: **sites already at `shape≥0.75` that fail M1 on jarring alone** — after t843 that is
   `www.tz.de` (reord 4, h_ov 3), `www.freesupertips.com` (reord 4), `payb.jp` (reord 6),
   `desiviral.net` (overlap 5). Each is one mechanism from a crossing, and they are cheaper than any
   shape nudge on the near-bar list.
2. **CLOSE THE I3 GAP THE CHEAP WAY.** Before more reading-order fixes, add the assertion that makes
   them semantic-model work rather than incidentally so: `manuk-agent`'s observation order for a page
   must match the geometric reading order the fidelity instrument scores. It is the same number in
   two subsystems and nothing checks that they agree.
3. **THE CEILING QUESTION IS UNCHANGED AND IS NOW THE LARGER HALF.** 27 in-scope sites do not render
   at all (79.2% scorability). Render work still pays — +3.2 points this window — but 21/130 cannot
   reach 95% until something boots them, and the throw-killer worklist has not been worked since the
   t777 batch.
4. **REFUSALS ARE STILL BEING BANKED CORRECTLY.** t836 refused a fix that improved two of three
   fixture families because it would have encoded a rule the same tick measured to be false, and t840
   banked two refutations instead of a guess. That is the behaviour §III's standing rule asks for and
   it is worth naming while it is happening rather than only after a bad tick.

**Next check due: tick 852.**

---

## Check #72 — tick 852

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)**.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md`, milestone **M1 RENDER** — `shape ≥ 0.75` **AND**
`jarring-clean` on ≥ 95% of the in-scope CrUX corpus. Latest honest reading: **15.4% (20 of 130)**,
scorability ceiling **77.7% (101 of 130)** (sweep t847).

Read this window: `CONSTITUTION.MD` Parts I–VII in full, `docs/loop/CONSTITUTION-CHECK.md` #71,
`STATUS.md`, `docs/loop/PHASE0-RENDER-BURNDOWN.md`, the t847 sweep rows.

### COMPLIANCE — what the window did right, stated so the failures below are not read as the whole

**PART VII / harness ownership held under real pressure.** Eight RED walls across t846–t848 were all
one infrastructure defect, and not one line of `scripts/` was edited. It was diagnosed precisely
(`verify.sh:191` calls `disk-hygiene.sh` **during** the wall; that script's `rm -rf target/debug` is
guarded only by `pct >= 95` and carries **no build-active check**, while the wall's own artifact set is
larger than this box's free space — so the build deletes its own inputs and reports it as
`linking with cc failed` on a different gate every run), reported in the journal, and **worked around
on the agent's own side of the line** by freeing headroom before the wall. That is what PART VII asks
for, done under the maximum incentive to do otherwise.

**I5 held, and the old-binary control is now the loop's sharpest instrument.** Four separate clean
`delta × n` integers this window — `gismart −7`, `payb.jp −66`, `taphouse23 −18`, `celeb.gate −7` —
every one of them refuted by re-running the OLD binary alone. Zero regressions traded for capability.

### FINDING 1 — **I3 IS BEING SATISFIED BY ACCIDENT, AND THE WINDOW'S BEST-AIMED RESIDUE IS AN I3 DEFECT THE LOOP FILED AS A SHAPE NUMBER**

I3: *"Every renderer subsystem lands with its semantic-model exposure or it is not done."* Five ticks
this window changed **element geometry** (t846, t848, t849, t850, t851). t845 established, and this
check re-verified from source rather than from memory, that geometry **is** the semantic model:

```
  LayoutBox::node_rects()  →  manuk_a11y::build_tree_with_rects(dom, rects)  →  A11yNode.bbox
                           →  hit_test / the agent's click point
```
(`engine/a11y/src/lib.rs:1008-1015`, `engine/page/src/lib.rs:6438,6470`.)

Each of those five ticks was therefore an I3 change, and each was gated **only** on `shape`/`overlap`.
They pass I3 because `node_rects` is a shared producer — **not because anyone checked.** A shared
producer means I3 compliance is a property of the plumbing, and the moment a fix touches the *producer
itself* the accident stops protecting us.

**And that is exactly the residue t851 measured and correctly declined to fix in the same tick:**

```text
  <div>A<span id=o1><span class=inline-block></span></span>B</div>
                                       Chrome            ours
    the wrapper <span>               [11,  0, 8, 17]   [11, 10, 8,  4]
    its inline-block child           [11, 10, 8,  4]   [11, 10, 8,  4]   ← IDENTICAL
```

`node_rects`'s `lift` walks a boxed child's rect up to boxless inline ancestors, so an icon-wrapping
`<span>` inherits the **4px-tall icon** instead of its own **17px line box**. Ranked on M1 that is a
rounding-scale `shape` term. **Ranked on I3 it is a mis-actuation surface**: the agent's click point
for that element is its bbox centre, so it is computed **3.5px low in a box 13px too short**, on
`<span class="icon"><i></i></span>` — one of the most common idioms on the web.

⚠ **THE LOOP RANKED IT ONLY ON M1, AND M1 IS THE WRONG RANKER FOR A DEFECT IN THE SHARED PRODUCER.**
The burndown ranks by `(in-scope sites × dy severity)`. That number is small here. The I3 number is
not, and nothing computes it.

### FINDING 2 — **USAGE-WEIGHT AND MEASURED-BREADTH DISAGREED FOUR TIMES, AND THE LOOP REPORTED THE DISAGREEMENT AS "ZERO MOVEMENT"**

VI.3's binding rule is *"the score is **usage-weight × failing-breadth**, not failing-subtest-count."*
Four consecutive ticks landed spec-correct, Chrome-exact, RED-proven primitives with enormous usage
weight — an insetless abspos box's static position (`.sr-only`, every framework page), the per-axis
static position (every `right:8px` badge), a button's vertical centring (**every button on the web**),
form-control `box-sizing` (every design-system button) — and the corpus moved by **+2 attributable
elements across 28 sites.**

The loop wrote that up honestly each time as "zero corpus movement." **That phrasing is accurate and
the inference drawn from it would be wrong.** These are not low-value fixes; they are
**high-usage, low-magnitude** errors, and the fidelity instrument scores a box as correct within a
tolerance. A 7px label offset inside a 50px button is below that tolerance on most pages *and is
visible to a human on all of them.* So:

> **The corpus cannot see the class of defect the constitution ranks highest**: universal idiom,
> small magnitude. `usage-weight × failing-breadth` and `Δ M1` are not the same ordering, and where
> they disagree the constitution says usage-weight wins.

⚠ This is **not** a reason to stop measuring — it is a reason to stop reading `Δ M1 ≈ 0` as a verdict
on the fix. t850 already drew the right operational conclusion (*diagnose ONE cohort site end to end
rather than reduce to a family and hope*) and t851 did exactly that, which is how the I3 residue above
was found at all. The constitutional framing is the missing half: **a Chrome-exact fix to a universal
idiom is on-mandate whether or not M1 moves, and the honest report is "the instrument cannot price
this", not "this bought nothing."**

### FINDING 3 — the corpus's own drift is now larger than four ticks of engine work, and only one site is watched for it

`celeb.gate.cc` was byte-identical (`0.783158`) in four consecutive A/Bs this window — the most stable
control the loop had — and then **moved on its own** to `0.768421`, which the OLD binary reproduced
twice. `payb.jp` spans `0.677824–0.825662` on ONE binary. `www.taphouse23.com`'s `overlap` wanders
10–13. Against that, the window's total attributable engine movement is `+2` elements.

The standing adversarial control (`www.ta3lemkonline.com`) is watched every tick and has been rock
steady (`0.573304`). **One stable control proves stability about one site.** The loop needs a small
fixed *panel* of controls re-read every A/B, and it has been improvising the panel per tick from
whatever the cohort happened to contain.

### HARNESS, reported not patched (PART VII)

1. **The wall self-purge described above.** `disk-hygiene.sh`'s `rm -rf target/debug` needs the same
   build-active guard its deps-prune already has, or the box needs ~120G free for a cold wall.
2. **Two crontab lines are DEAD from a quoting bug** — `disk-hygiene.sh` and `loop-watchdog.sh` are
   wrapped in `bash -lc \'…\'` and log `unexpected EOF while looking for matching '` on **every**
   fire. `ops-check.sh:25` already knows and alerts on the stale hygiene log.
3. **`manuk-wpt` is still in neither the wall's crate-test list nor CI's** — reported at #69, #70, #71.
4. `mem-guard.sh` reports **swap 91–94% full** on every wall of this session.

### THE STEER

1. **FIX `node_rects`'s INLINE RECT, AND RANK IT AS I3, NOT AS SHAPE.** It is the shared producer for
   layout, the a11y tree and the agent's click point, so it is the highest-leverage box in the engine
   and the only one where a defect is simultaneously a rendering bug and a mis-actuation bug. Scope is
   known and honest: `node_rects` takes only `&Dom` — no styles, no fonts — so the line's content area
   must be recorded at layout time, and `G6` clickability reads the same map, so it needs a real
   regression control. **Land it with an agent-side assertion in the same tick** (the t845 shape: the
   click point of an icon-wrapping span falls inside the span), which is what I3 actually asks for and
   what five geometry ticks in a row have been getting for free.
2. **STOP READING `Δ M1 ≈ 0` AS A VERDICT ON A CHROME-EXACT FIX.** Report it as *"the instrument
   cannot price this"* and say why (universal idiom, magnitude under tolerance). Where usage-weight
   and measured-breadth disagree, VI.3 says usage-weight wins — that is constitutional, not a
   preference.
3. **FIX A CONTROL PANEL AND RE-READ IT EVERY A/B.** Four sites, chosen once, carried across ticks:
   one adversarial RTL (`ta3lemkonline`), one large stable (`celeb.gate.cc` — *now known to drift*,
   which is itself the reason to watch it), one small deterministic (`mobcup.fm`), one known-bimodal
   (`payb.jp`). A control's status is earned run by run; the panel makes that cheap instead of
   improvised.
4. **THE SCORABILITY CEILING IS STILL THE LARGER HALF AND STILL UNTOUCHED.** 29 of 130 in-scope sites
   do not render at all (77.7%). Five render ticks this window; the throw-killer worklist has not been
   worked since t777. 20/130 cannot reach 95% until something boots them.

**Next check due: tick 860.**
