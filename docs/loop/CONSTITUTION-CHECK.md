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
