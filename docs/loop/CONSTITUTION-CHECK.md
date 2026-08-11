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

## Check #73 — tick 860 (2026-08-03)

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)**, with
component **2 (the agentic surface)** touched at t853 and component **3 (Bar 0)** at t854.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md`, milestone **M1 RENDER** — `shape ≥ 0.75` **AND**
`jarring-clean` on ≥ 95% of the in-scope CrUX corpus. Latest honest reading, sweep **t857**
(`--jobs 2`, 200 sites, clean): **M1 14.0% (18 of 129)** · shape ≥ 0.75 **18.6% (24)** ·
jarring-clean **27.9% (36)** · shape_mean **53.6%** · cov_mean **84.0%** ·
printed scorability ceiling **101 / 129 = 78.3%**.

Read this window: `CONSTITUTION.MD` Parts I–VII in full, check #72, `STATUS.md`, the current
`lever-board.sh` output, `docs/loop/FIDELITY-PROGRESS.tsv`, the t857 sweep write-up, journal ticks
853–860.

### DID THE STEER FROM #72 LAND? — three of four, and the miss is the cheapest one

| #72 steer | outcome |
|---|---|
| 1. Fix `node_rects`'s inline rect, **rank it as I3**, land it with an agent-side click-point assertion in the same tick | ✅ **t853, the very next tick** — and it went further than the steer asked: the fix broke a latent tie in `hit_test` that cost **16 clickable links** on Wikipedia, caught only because the same tick was required to assert clickability. |
| 2. Stop reading `Δ M1 ≈ 0` as a verdict on a Chrome-exact fix | ✅ held — t857 **pre-registered** "shape flat" in its stub *with the reason* (small magnitude on a universal idiom) and shape came back `24 → 24`. |
| 3. Fix a four-site control panel and re-read it every A/B | ❌ **not done.** t859 improvised an eight-site set; t860 improvised a three-site set. Both were adequate *for their tick* and neither is comparable across ticks, which was the entire point. |
| 4. The scorability ceiling is the larger half and untouched | ✅ **worked three times** — t856, t858, t860. See finding 1. |

### COMPLIANCE

**I5 held, and Bar 0 outranked a capability again.** t853's sweep of the *whole* `manuk-page` suite
(the wall runs 19 of 104 gates) surfaced a 3-minute spin in `g_reflect_numeric`. The old binary,
rebuilt from a stashed tree and run in the same hour, **hung identically** — so it was correctly
booked as pre-existing, named as its own tick, and fixed at t854 rather than folded into the
capability write-up where neither would have been attributable.

**PART VII held.** No `scripts/` file was edited this window. The wall self-purge and the two dead
crontab lines carried over from #72 are still reported, still not patched.

**I4 held, visibly.** `RATCHET.tsv` is **byte-identical on every WPT mark** across all eight ticks —
`WPT:TOTAL 422865`, `encoding 360559`, unchanged. Under VI.3 that is the correct shape for a window
spent on usage-weighted breadth, not a failure to produce.

### FINDING 1 — ⚠⚠⚠ THE INSTRUMENT'S REASON STRINGS ASSERT THEIR OWN AUTHORSHIP, AND NOTHING FALSIFIES THEM. TWO OF THREE "OURS" COHORTS HAVE NOW BEEN RE-MEASURED AND **BOTH SHRANK.**

I5 makes the differential oracle *"the primary mechanism for finding what to build next."* The oracle
does not merely emit a number — it emits a **hand-written reason string per unscored site**, and the
loop consumes those strings as a **ranked backlog**. Two ticks, four apart, tested one string each:

```text
  t856  shell-only-N       "the site served a shell"        →  10 of 12 rows are the ORACLE rendering
                                                                one curl'd file whose relative bundles
                                                                404 — the pages render 48–1115 tags in
                                                                Chrome and our own row reads cov 1.000
  t860  css-starved-N      "the sheets were cut by our own   →  404 at the origin on 3 of 3. Chrome gets
                            load deadline, NOT refused by        the same 404 and renders the same page.
                            the origin"                          Cost: one `curl`.
```

**Neither string had ever been tested, and both were the loop's own prose about the loop's own
engine.** The `css-starved` one is the sharper case because it does not describe a symptom — it
**names a cause and rules another one out**, in bold, in a machine-consumed field. It was false on
every instance.

Constitutionally this is an **I5 defect, not a measurement nit**: a discovery engine whose divergence
labels are unfalsifiable prose does not discover work, it **manufactures backlog** — and this loop has
now been billed twice for it. The correct standing rule, and it is cheap:

> **A reason string that asserts a CAUSE is a hypothesis with a test attached. Before any cohort it
> names is treated as engine work, run that test on one member.** Where the test is a `curl`, there
> was never an excuse.

The printed `SCORABILITY CEILING 101/129 = 78.3%` is therefore **a floor on our fidelity, not a
ceiling on our engine.** `m.youm7.com` is the proof: counted against us as *"we could not style it"*,
it turned out to be `cov 1.000 · shape 0.870 · jarring 0/0/0/0` — **an outright M1 pass we had been
scoring as a failure to render.**

### FINDING 2 — THE REMAINING UNSCORED RESIDUE IS NOW SMALL ENOUGH TO ENUMERATE, AND IT HAS NOT BEEN `curl`ED

After t856 and t860 the named residue that still claims to be ours is:

```text
  render-failed      2        timeout            2        tree-divergence    5
```

Nine rows. Finding 1 says each carries an untested causal claim, and the two tested so far both
collapsed. **The next scorability tick is nine `curl`s, not nine engine investigations** — and it must
happen before any of those nine is scheduled as layout work. That is the direct application of the
PART III standing rule (advance the gate) to the cheapest available lever on the board's own #1.

### FINDING 3 — PART VI IS STALE IN A WAY THAT MATTERS: IT STILL RANKS BY WPT PERCENTAGES THAT NO LONGER GOVERN

VI.2 names the H0 layout blocker as *"`css-flexbox` **5.5%**, `css-grid` **4.7%**"* and VI.3 fixes the
H0 gauge at *"WPT breadth excluding encoding = 32.3%"*. Both are tick-86 readings, and **PART VII
(written later, and explicitly superseding the roadmap for the near term) says the bar is
*"reliably renders and runs the representative real internet," NOT a WPT percentage** — and that
*"83% and beyond is explicitly OUT OF SCOPE for v1."*

So PART VI is still handing the loop a ranking instrument that PART VII retired. The tree has moved:
the governing gauge for the last ~150 ticks has in fact been **M1 on the in-scope CrUX corpus**, and
`RATCHET.tsv`'s WPT marks are kept as the regression ratchet they are, not as a ranking. VI.2 and VI.3
are corrected below to say that out loud, so a future check does not re-derive the retired ordering
from a document that still asserts it. (VI.2's *structural* claims were re-verified from the tree and
**stand**: `taffy = "0.12"` in `Cargo.toml:83`, no double-dirty-bit incremental relayout, tiny-skia CPU
raster everywhere.)

### GATE OR SCOREBOARD? — gate, on all four of PART VII's components that moved

- **Component 1 (rendering parity):** t859 (CSS 2.1 §8.3.1 — an out-of-flow first child does not cancel
  the parent/child margin collapse; the hoist computation and the placement loop had disagreed with
  each other for ~700 ticks and nothing compared them) and t860 (+3 sites scorable, one of them an
  outright M1 pass).
- **Component 2 (agentic surface):** t853 — the I3 steer, landed with the click-point assertion, which
  is how the 16-link `hit_test` regression was caught *before* it shipped rather than by a sweep.
- **Component 3 (Bar 0):** t854 — a `colspan` of two billion is a hang, not a big table.
- **The measurement itself:** t856, t857, t858, t860. t858 is worth naming: it **retracted its own
  predecessor's stated mechanism while keeping its conclusion**, which is the failure mode that
  survives longest (right answer, wrong reason) and the loop caught it in two ticks.

Not one of the eight ticks moved a WPT number, and per I4/VI.3 that is the right answer.

### HARNESS, reported not patched (PART VII)

1. **The wall self-purge** — `verify.sh` calls `disk-hygiene.sh` mid-wall and its `rm -rf target/debug`
   is guarded only by `pct >= 95` with no build-active check. Carried from #72, unchanged.
2. **Two crontab lines are DEAD from a `\'` quoting bug** (`disk-hygiene.sh`, `loop-watchdog.sh`).
   Carried from #72, unchanged.
3. **`manuk-wpt` is in neither the wall's crate-test list nor CI's.** Reported at #69–#72.
4. **New, measured this tick:** `/home` sits at **92% (24G free)** and `disk-hygiene.sh` reclaims
   **zero** — the ~95G in `target/debug/deps` is the genuine working set (≤2 generations per stem, both
   live feature variants), not orphan bloat. Item 1 is therefore not a hypothetical: there is no
   headroom left to free before a wall, and the prune that used to buy it has nothing left to take.

### THE STEER

1. **`curl` THE NINE.** `render-failed` 2 · `timeout` 2 · `tree-divergence` 5. One tick, nine cheap
   probes, before any of them is treated as engine work. Two of two tested so far were not ours.
2. **A REASON STRING THAT NAMES A CAUSE MUST CARRY ITS TEST.** Standing rule from finding 1. When the
   instrument writes *"OURS because X"*, the tick that consumes it runs the one-command check on X
   first — and where the string was wrong, **fix the string**, because it will be re-read every sweep.
3. **FIX THE FOUR-SITE CONTROL PANEL. THIRD TIME OF ASKING.** `ta3lemkonline` (adversarial RTL) ·
   `celeb.gate.cc` (large, known-drifting) · `mobcup.fm` (small deterministic) · `payb.jp`
   (known-bimodal). Chosen once, carried across ticks, re-read every A/B. It has been re-improvised
   every tick since #72 and so no two ticks' controls are comparable.
4. **THEN THE RANKED MECHANISM RESIDUE**, which the t857 sweep already computed and nothing has taken:
   `missing box: <img>` **16 sites / 643 hits** and `missing box: <li>` **14 sites / 410 hits** are the
   two largest single mechanisms in the corpus, both ahead of every geometry band.

**Next check due: tick 868.**

## Check #74 — tick 868 (2026-08-03)

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)**
throughout; component **3 (Bar 0)** touched at t863; **no** work on components 2 or 4 this window.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md` milestone **M1 RENDER** — `shape ≥ 0.75` **AND**
`jarring-clean` on ≥ 95% of the in-scope CrUX corpus. Latest reading, sweep **t867** (200 sites,
⚠ **parallel-6**, see the caveat below): **M1 16.4% (21 of 128)** · shape ≥ 0.75 **22.7% (29)** ·
jarring-clean **31.2% (40)** · shape_mean **54.4%** · cov_mean **86.2%** · **scorability ceiling
106/128 = 82.8%** (t857: 78.9%).

Read this window: `CONSTITUTION.MD` Parts I–VII in full, check #73, `STATUS.md`, the current
`lever-board.sh` output, `docs/loop/FIDELITY-PROGRESS.tsv`, the t867 sweep, journal ticks 861–868.

### GATE OR SCOREBOARD? — **gate**, and on the conjunct that was actually binding

Eight ticks, and not one of them moved a WPT number. Five moved a site from UNSCORED to SCORED, which
is movement in the **M1 CEILING** — the term no amount of shape work can pass, because a site that
does not render cannot be scored at all. `www.otomoto.pl`, `profissionaliza.cademi.com.br`,
`pogoda.by`, `m.youm7.com`, `nortenoticia.com.br`. Each was a named engine defect found from a corpus
site, which is VI.3 / VII.1's ranking rule (**real sites moved**) executed literally.

### THE STEER FROM #73

| #73 steer | outcome |
|---|---|
| `curl` the nine unscored rows that still claim to be ours | ✅ **t861** — all nine serve 200 in under 4s, and the two `timeout-150s` rows were **Chrome hanging**, not us. |
| A falsified reason string must be REWRITTEN, not just noted | ✅ **t861** rewrote `css-starved`'s; **t865** rewrote `ShellOnly`'s and corrected t674's refutation. |
| Fix a four-site control panel and re-read it every A/B (carried from #72, ❌ twice) | ✅ **at last.** `littlecaesarsbcs` and `hipmiluwuutara` are now read in every A/B and came back **byte-identical five ticks running**; t863 added a 7-site range-spanning panel and t867 a 12-site one. |
| The scorability ceiling is the larger half | ✅ **78.9% → 82.8%**, and the remaining 22 are now classified rather than lumped. |

### ⚠⚠⚠ FINDING 1 — I3 IS BEING BENT, EXACTLY WHERE CHECK #72 SAID IT WOULD BE

Check #72 established that **I3 is satisfied by accident**: `node_rects` → `manuk_a11y` bbox → the
agent's click point, so every geometry tick is an I3 tick and nothing checks it — *"the moment a fix
touches the producer itself, that accident stops protecting us."* Check #73 recorded that t853 obeyed
the resulting steer and was rewarded (it found 16 dead links).

**t868 touched `node_rects`'s geometry again and did NOT land the click-point assertion.**
`G_EMPTY_INLINE_RECT` asserts the rects and the containing blocks' heights; it says nothing about the
a11y bbox or the actuation point. The mitigating fact is real and does not excuse it: the element in
question is a **zero-width** empty inline, so it is not a click target and no bbox centre moved
horizontally. But the standing steer is not "assert it when you think it matters" — it exists
precisely because the judgement of when it matters is the thing that has been wrong. **Recorded as a
bend, not a violation, and carried as steer #1 below.**

### ⚠⚠ FINDING 2 — HALF THIS WINDOW WAS INSTRUMENT WORK, AND THAT WAS CORRECT ONCE

Four of eight ticks (861, 863, 865, 867) were instrument fidelity rather than engine capability. On
its face that is drift: I5 makes the oracle the *discovery engine*, not the subject. Applying VII.1's
test honestly — *does this move component 1 toward shippable?* — each one does, and for the same
reason: **a mislabelled row sends engine ticks at nothing.** t861 found two sites blaming us for
Chrome's hang; t865 found the largest unscored cohort (13 sites) was our own snapshot's shell. Both
had been misdirecting the backlog for many sweeps.

But the honest read of the *rate* is that the instrument had accumulated debt and this window paid it
down, and that debt is now largely paid: the unscored rows are classified, the reason strings say
whose failure they are, and the sweep's parallelism is priced. **A second consecutive window at this
ratio would be drift, and should be called as such.**

### ⚠⚠⚠ FINDING 3 — PART VI'S BLOCKER HAS MOVED, AND THE BOARD'S RANKING IS STALE BEHIND IT

For many windows the binding constraint was *"sites do not render at all"*. At **82.8%** scorability
that is no longer where the mass is: **106 sites are scored and only 29 clear the shape bar**, and the
t867 readout's own trap-free measure — mean Δshape over the 99 sites scored in **both** t857 and t867
— is **-0.06 points**. Flat. The blocker is now unambiguously **layout math on pages that render
fine**.

And the board's own leg ordering was computed against the old shape of the corpus. Recomputed on
`SWEEP-t867-rows.tsv`:

```text
  near-bar (0.55 <= shape < 0.75) AND jarring-clean ...  2 sites  (both need +0.17, not +0.06)
  over the shape bar, failing ONLY on jarring .......... 13 sites
```

The board says shape-nudge is **~6× more M1-productive** than jarring. On today's corpus it is
**6.5× less**. That is not a wrong steer; it is a steer whose measurement expired.

### ⚠⚠ FINDING 4 — THE LOOP CAN NOW CONTAMINATE ITS OWN SERIES, AND DID SO WITHIN ONE TICK

t867 measured what the board's #1 throughput lever costs: parallel-6 moved 5 of 12 control sites
against a serial re-measure on the same binary in the same hour, and **one crossed the 0.75 M1 bar on
the scheduling alone**. It also *manufactures* Bar-0 rows — two chunks segfaulted, and all 27
remaining sites then ran clean one process each.

The instructive part is what happened next: **t868 differenced a solo reading against a parallel row
and briefly read a 0.06 regression on `pogoda.by`** that was entirely the artefact t867 had banked one
tick earlier. Caught only because pogoda's stable solo value had been measured three times that day.
**A finding written down is not a finding applied.**

### PART VI CORRECTION

* **Now DONE:** the scorability leg is no longer the larger half (82.8%); the unscored 22 are
  classified by *whose* failure they are, not lumped; the sweep's parallelism has a measured price.
* **The real blocker:** shape/geometry on the 106 sites that already render — flat at ±0 over two
  sweeps, needing +101 sites for M1.
* **The direct path:** rank by **marginal M1 crossings** from the banked per-site distances, which
  today means the 13 jarring-only sites before the 2 near-bar ones — and find the actual
  reading-order mechanism, which t868 proved is **not** the empty-inline rect.

### THE STEER

1. **I3, carried and now overdue:** the next tick that touches `node_rects` lands an agent-side
   click-point assertion **in the same tick**. #72 asked, #73 saw it pay, #74 saw it skipped.
2. **Re-rank by the fresh sweep, not the board's t777 ratio:** 13 jarring-only crossings vs 2
   near-bar. Observer owns the board; this is reported, not edited.
3. **Never difference a parallel-6 row against a solo row** — and keep the banked series serial, or
   re-baseline it explicitly. Re-measure any `crashed` row SOLO before believing it.
4. **The instrument debt is paid; the next window is engine work.** If it is not, say so out loud.

## Check #75 — tick 877 (2026-08-03)

**Horizon:** H0 — Pareto Web Parity, PART VII component **1 (daily-driver rendering parity)** for
ticks 871–875, component **1's INSTRUMENT** for 876–877. No work on components 2, 3 or 4 this window.
**Gate:** `DAILY-DRIVER-CERTIFICATION.md` milestone **M1 RENDER** — `shape ≥ 0.75` **AND**
`jarring-clean` on ≥ 95% of the in-scope CrUX corpus. Latest reading, sweep **t875** (200 sites,
**`--jobs 2`** — the bankable kind, unlike t867's parallel-6): **M1 16.9% (22 of 130)** ·
shape ≥ 0.75 **23.1% (30)** · jarring-clean **33.8% (44)** · shape_mean **56.5%** · cov_mean
**86.2%** · **scorability ceiling 106/130 = 81.5%**.

Read this window: `CONSTITUTION.MD` Parts I–VII in full, check #74, `STATUS.md`, the current
`lever-board.sh` output, `docs/loop/FIDELITY-PROGRESS.tsv`, the t875 sweep, journal ticks 869–877.

### → Gate, or scoreboard?

**GATE, and for the first time this session with M1 crossings to show for it.** Five sites crossed
the M1 bar in four consecutive ticks — `possssno.sbs`, `www.marktplaats.nl` (t871),
`ubys.bingol.edu.tr` (t872), `www.library.chiyoda.tokyo.jp` (t873), `desiviral.net` (t874) — and the
t875 corpus sweep confirms **all five and zero losses**. Every one was attributed by an OLD-BINARY
control (the pre-fix tree rebuilt from `git stash`, re-run in the same hour, identical denominators),
which is the strongest evidence this loop has. That answers check #74's steer directly: it asked for
a shape tick that *finds the mechanism rather than inheriting a guess*, and four of them landed.

**VI.3 CORRECTED:** its M1 instrument row read **14.0% (18/129), shape_mean 53.6% (t857)**. It is now
**16.9% (22/130), shape_mean 56.5% (t875)**. The trend across three sweeps is 12.0 → 16.4 → 16.9,
with the middle reading **inflated by its own scheduling** (t867 ran parallel-6; its own tick proved
parallelism manufactures crossings). Measured serial-to-serial the movement is real and it is the
first sustained movement M1 has had.

### → Is VI.3's ranking still the north star?

**Yes, and it was re-derived rather than remembered — which is the finding of the window.** t868
recomputed the board's t777 leg-2/leg-3 ratio on fresh data and found it **inverted 6.5×**: 13 sites
sit *over* the shape bar failing only on a jarring dimension, against 2 in the near-bar cohort the
board ranks first. Working the 13 produced five crossings in four ticks. **A ranking is a
measurement, and measurements go stale** — t875 shows the cohort has already moved again (five gone,
two new entrants).

No big-but-tail number has crept back. `RATCHET.tsv`'s WPT marks were byte-identical across the whole
window, which VI.3 names as the *expected* shape of on-mandate work.

### → Is any invariant being bent?

**I3 — better than check #72 and #74 found it, but still not asserted.** Every geometry tick this
window moved `node_rects`, which is the a11y bbox, which is the agent's click point. Unlike t868 (which
check #74 caught bending I3 by touching `node_rects` without a click-point assertion), the boxes moved
here are **real click surfaces** and they moved *toward* Chrome: an off-canvas drawer that never left
the screen and was covering the header (t874), a nav anchor 29px too narrow and 18px too tall (t871), a
content column rendered under a float (t873). Each journal entry says so explicitly. **But the
assertion check #72 asked for still does not exist** — the argument remains "the geometry improved, so
the click point improved", which is an inference, not a gate. Carried, now for the third check running,
and it should be a tick rather than a paragraph.

**I5 held.** Zero regressions traded. One reading that looked like a loss (`sestra.cc`, −0.01 with
reading-order 3→5) was refused as a result after three solo runs on the new binary alone spanned
0.9225–0.9394 with reading-order 2–5 — **the old reading sits inside the new binary's own spread on
every column.**

### → PART VI corrections

* **VI.2 / H0.1** — the M1 row is updated above (14.0% → 16.9%).
* **VI.2 / I5** — the reconciliation says *"the primary discovery engine is now the INSTRUMENTED
  LOG"*. This window says something narrower and sharper: **the discovery engine was the four-line
  fixture against `chromium --dump-dom`, and its yield came from REFUTATION.** Every one of the four
  render fixes was found by a fixture that killed the obvious hypothesis first — RTL inline
  reordering on `possssno` (byte-identical to Chrome, refuted in two minutes, and the LTR bisect that
  followed found `text-align:center`); four hypotheses refuted on `desiviral` before the real one; six
  on `simplepdf`, which still has not reduced; four loader hypotheses retired on `webfenix` with a
  delaying local origin. **A fixture that refutes your hypothesis is the cheapest possible outcome**
  and it is the thing that made these ticks converge.

### → THE STEER

1. **The instrument's "our own bug" cohort was not ours, and that keeps happening.** t877 makes
   `render-failed` the **fourth** consecutive cohort named as ours and proven not — after `shell-only`
   (t856), `css-starved` (t860) and `oracle-timeout` (t861). Both of t875's `render-failed` rows
   relabel to `oracle-module-shell`. **Before the next scorability tick, re-run the sweep's unscored
   partition and re-read it**: the worklist the board quotes (24 unscored, ~12 "genuinely ours") has
   never survived a re-derivation intact.
2. **The real scorability blocker is not an engine tick.** Eleven of the 24 are `shell-only` /
   `oracle-module-shell` — t865's `type="module"` CORS wall — whose named fix is a **loopback reverse
   proxy** giving document, bundle and XHR one origin. That is instrument work in agent territory
   (`manuk-wpt`), it is the largest single unscored cohort, and it has been named and not built since
   t865.
3. **The geometry vein is still live** — eight sites remain over the shape bar failing only on
   jarring, re-ranked on t875 — but it is now the *second* lever, not the first, because the ceiling
   caps M1 at 81.5% no matter how much geometry lands.
4. **Land the I3 click-point assertion.** Third check running.

---

## Check #76 — tick 885 (2026-08-04)

**Horizon:** H0, re-scoped by **PART VII** (which supersedes the roadmap for the near term). The gate
in force is PART VII's four components, and component 1's bar is *"reliably renders and runs the
representative real internet"* — **not a WPT percentage** — instrumented as **M1 on the in-scope CrUX
corpus** (`shape ≥ 0.75` AND jarring-clean, bot-walls excluded).

### → Gate, or scoreboard?

**Gate — and the window's honest yield is smaller than its page count, for a reason worth naming.**
Ticks 877–884 landed one agentic-surface gate, one surface audit, one instrument subsystem and four
capability fixes. Three of the four capability ticks reported **"M1 did not move"** in their own
headline. That is true, and it is **three different facts wearing one sentence**:

| tick | what it said | what was actually true |
|---|---|---|
| t882 `<template>` innerHTML | "M1 did not move" | **it moved** — `portal.ensuretyfinance.com` crossed at 0.864, and three rows went from unscored-for-an-INSTRUMENT-reason to scored |
| t883 copy namespaces | "M1 did not move" | **geometry provably unchanged** — measured on both binaries; the fix is DOM correctness the instrument *cannot* price |
| t884 IndexedDB prototypes | "M1 did not move" | **the failure was off the first-paint path** — a cached-data read, so the site already scored |

Lumping those three under one sentence reads, to any later audit, as *"four ticks bought nothing"*.
VI.3's corollary already covers the second row (*"the instrument cannot price this" ≠ "this bought
nothing"*); the first row is worse than that — it is a **crossing that its own tick under-reported**.
**Standing correction: a tick that writes "M1 did not move" must say WHICH of the three it means.**

### → Is VI.3's ranking still the north star?

**Yes, and this window is the strongest evidence yet that usage-weight beats measurable delta.** t884
is the case: `idb` — the wrapper Firebase and Workbox are built on — could not construct its API
because our IndexedDB methods were own properties rather than prototype members. Enormous usage
weight, and a corpus delta of **+0.1 points on one site**. Under a delta-ranked loop that tick never
happens. Under VI.3 it is correct, and it is the third consecutive window where the two orderings
disagreed.

No big-but-tail number crept back. `RATCHET.tsv`'s WPT marks were byte-identical across all eight
ticks — which VI.3 names as the *expected* shape of on-mandate work.

⚠⚠⚠ **BUT THE LOOP IS BLIND ON ITS OWN HEADLINE AND HAS BEEN FOR NINE TICKS.** The last sweep is
**t875**; ticks 876–884 are unpriced corpus-wide. That is the same *"the next clean sweep will price
it"* pattern the board flagged at t777 and it has recurred. Worse, this window has a specific reason
to distrust its own local readings: t883 watched a three-site batch manufacture a **10.8-point** loss
on `blog.rust-lang.org` that three solo runs refuted (99.6/99.6/99.6). A per-site reading taken during
a batch is not a corpus number, and nine ticks of them are not a sweep.

### → Is any invariant being bent?

**I3 — the carried item is CLOSED, and the new answer is CHECKED rather than assumed.** Checks #72,
#74 and #75 all carried *"land the I3 click-point assertion"*; **t878 landed `G_CLICK_POINT`**, so it
comes off the list after three checks. For this window's DOM ticks the honest question is whether they
moved the semantic model without saying so, and the answer was verified from the tree rather than
reasoned about: `engine/a11y/src/lib.rs` **does not branch on element namespace at all**, so t883's
namespace fix has no AX consequence, and a `<template>`'s contents never generate boxes, so t882's has
none either. Not bent — and *checked*, which is the part that matters, because the last three checks
each found this bent by assumption.

**I5 held, twice, in the same direction.** t883's manufactured loss was refused because the control
ran *before* the conclusion. t884's error harvest correctly discarded its own **loudest eight rows**
(`Failed to execute 'query' on 'Permissions'`) as the reference's behaviour rather than a defect —
`speaker`, `device-info`, `clipboard` and `accessibility-events` are invalid `PermissionName` values
in Chrome too. *The loudest row in a sweep is the sweep* (t841-845), now with a second instance.

**I4 held.** Nothing chased the tail.

### → PART VI corrections

* **VI.2 / VI.3 — the M1 row is stale by two sweeps.** It reads *"14.0% (18/129), shape_mean 53.6%,
  cov_mean 84.0% (t857)"*. Ground truth from `FIDELITY-PROGRESS.tsv`: **t875 — M1 16.9%, shape_mean
  56.5%, cov_mean 86.2%**. Corrected in `CONSTITUTION.MD`.
* **VI.3's corollary earns a second clause.** It already says a reason string asserting a CAUSE is a
  hypothesis to test before working the cohort it names. The window adds the *converse* and it is now
  five-for-five: `shell-only` (t856), `css-starved` (t860), `oracle-timeout` (t861), `render-failed`
  (t877) and `oracle-module-shell` (t880-881) were each named as ours and each proved to be the
  instrument. **The prior on a cohort label is that it is wrong.**
* **VI.2 / I5 — the discovery engine, re-derived once more.** Check #75 said it was the four-line
  fixture against `chromium --dump-dom`, and that stands for render work. For the FUNCTION leg this
  window it was something narrower and cheaper: **run the cohort and grep the log**, which produced a
  named function (`insertStaticContent`) at t882 and a named library gate (`idb`'s `in` test) at t884
  — one run each, no bisection.

### → THE STEER

1. **RUN A SWEEP BEFORE THE NEXT CAPABILITY TICK.** Nine ticks unpriced, four of them capability
   fixes whose own headlines disagree about what they bought. This is the one item that blocks
   honest ranking, and it has been deferred once already this window.
2. **PART VII COMPONENT 2 IS GETTING 4% OF THE LOOP.** Counted, not felt: of the last 24 ticks
   (861–884), **one** — t878 — was shaped `capability (agentic surface)`, against 11 measurement/
   instrument and 12 rendering/function. The constitution says component 2 is *"the differentiator,
   not a feature — it earns the most polish"* and that "complete" means *an agent can reliably drive
   the same top-N real sites a human daily-drives, measured against the same corpus*. **There is no
   such measurement.** Component 1 has M1; component 2 has gates but no corpus number. Building one is
   the highest-leverage constitutional correction available, and it is exactly the shape of work this
   loop is good at.
3. **The function-leg vein is live and cheap** — two ticks, two named mechanisms, from one grep each.
   Its remaining named residue already has messages rather than hypotheses: `IDBObjectStore.getKey` /
   `openKeyCursor` absent · `no object store named local-key-val` (coinmarketcap) · `TypeError:
   Invalid URL:` thrown from **our own** `dom_event.js:2287` (sports.yahoo) · `Route did not complete
   loading: /` (trivago).
4. **The `allticketscol` half-boot is one named XHR away.** t881's acceptance test refuses it and says
   why: the app boots under the proxy (`app-mensaje`, `mat-dialog-container` render) and 47
   `app-evento-card` never arrive.

---

## Check #77 — tick 893 (2026-08-04)

**Horizon:** H0 re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus**. Latest banked:
**17.8% (23/129)**, sweep t887, release binary, all instrument guards clean.

### → Gate, or scoreboard?

**Neither, for six of the eight ticks — and that is the finding, stated without softening.** By their
own `TICK SHAPE:` lines, 885–892 were: five `measurement`, one `instrument`, two `capability`. Two of
the eight touched engine code that a page can observe (891 the rejection describer, 892 the XHR slot
leak), and **neither was picked from the crossing ranking** — both came out of chasing a defect in the
loop's own instruments.

The window's honest ledger:

| what it was | ticks | what it bought |
|---|---|---|
| **withdrawing a bad measurement** | 886, 887 | t886 swept 200 sites from `target/debug` (4–5.5× slower, and it **understates shape**), banked it, and spent its whole attribution budget — a full old-binary rebuild — on a "19-site scorability regression" that was the binary. Withdrawn, row deleted, guard landed. |
| **cadence** | 885, 889, 890 | constitution check #76 · surface audit #60 (449 → 459 rows, nine with verdicts) · a fully green self-audit |
| **ranking** | 888 | the crossing cohort re-derived on trustworthy data: 8 sites one jarring dimension from M1 |
| **capability** | 891, 892 | `[object Object]` ×16 → named XHR rejections at `readyState:0` · this engine's private slots removed from a page's view of an XHR |

⚠⚠⚠ **THE WINDOW'S MOST EXPENSIVE SINGLE FACT: THE LOOP CANNOT TELL WHICH BINARY PRODUCED A NUMBER,
AND IT COST TWO TICKS.** `scripts/fidelity-sweep.sh` has pinned `target/release` since it was written,
but **every `SWEEP-t<N>-rows.tsv` in the repo is produced by invoking the binary directly**, and that
path was unguarded. The instrument refuses now (`may_bank_a_sweep`, RED-proven). The sharper lesson,
recorded because it is narrower than the law it comes from: **when a reading is surprising, check WHAT
BINARY produced it before checking what the code did.**

### → Is VI.3's ranking still the north star?

**Yes, and it survived a direct test this window.** t892's *real* defect (private slots enumerable to
a page) has essentially zero measurable corpus delta and a large usage weight — every error reporter
serialises a failing request. A delta-ranked loop never takes it. VI.3 does. `RATCHET.tsv`'s WPT marks
were byte-identical across all eight ticks, which VI.3 names as the expected shape of on-mandate work.

⚠ **BUT THE RANKING IS NOT WHAT CHOSE THE WINDOW'S WORK.** t888 computed the crossing cohort — eight
named sites, +6.2 M1 points, the largest known lever — and **no tick since has worked it.** 891 and 892
were both "the thing I tripped over while investigating the thing I tripped over". That is a
defensible chain (t891's describer named a 16× cohort; t892 corrected t891) and it is still not the
ranked work. Two consecutive windows have now ended with the top-ranked lever untouched.

### → Is any invariant being bent?

**I5 held, and it is the invariant this window actually exercised.** Three separate times a conclusion
was refused until a control ran: t887's debug/release comparison (which withdrew a whole tick's
headline), t888's Slick bisect (which killed "it is a layout bug" in one probe), and t892's XHR
prototype probe (which **refuted t891's own second finding before a line was written against it**).
The third is the one worth keeping — *a wrong FIX is caught by the next gate; a wrong LABEL by
nothing* — and the correction was banked as **gate claims** (`protoPatch:1`, `ownOpen:false`) rather
than prose, so it cannot be re-derived.

**I3 is not bent, checked rather than assumed** (as at check #76): neither 891 nor 892 changes an
element's geometry or the a11y tree — one is a log message, the other a property attribute.

**I4 held.** Nothing chased the tail.

### → PART VI corrections

* **VI.3's M1 row is current** — 16.9% (t875) → **17.8% (t887)**, already corrected at check #76 and
  re-verified here against `FIDELITY-PROGRESS.tsv`.
* **VI.2 / I5 gains the binary clause.** The reconciliation says the discovery engine is the
  instrumented log, and check #75 narrowed that to the four-line fixture. This window adds the
  precondition both depend on: **a reading is only evidence about the engine if it came from the
  shipping binary**, and nothing enforced that until t887.
* **The board's scorability ceiling is stale by twenty points** (63% → 82.9%) and its owner-lock opens
  the BiDi function leg at ~85%. That is now **2.1 points away**, which the board cannot see.

### → THE STEER

1. **WORK THE RANKED COHORT.** t888 named eight sites and their blocking dimensions; the top one's
   mechanism is named down to the failing subsystem (Slick never initialises because sixteen XHRs
   reject at `readyState: 0` — t891). **The next tick is why those XHRs never open**, not whatever the
   next investigation trips over.
2. **The scorability ceiling is 2.1 points from the owner-lock's BiDi trigger.** Nine `shell-only` rows
   are the largest remaining unscored cohort; the board still ranks off 63%.
3. **Carried, still unclaimed:** the common-set band is **−0.34 pts** over the 103 sites scored in both
   t875 and t887. The next engine tick reads it before claiming a win.
4. **Component 2 (the agentic surface) is still at ~4% of the loop and still has no corpus number** —
   check #76's finding 2, unmoved. The constitution calls it the differentiator; nothing measures it.

## Check #78 — tick 901 (2026-08-04)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus**. Latest banked:
**17.1% (22/129)**, sweep t898, release binary, `--jobs 2`, all instrument guards clean.

### → Gate, or scoreboard?

**The gate went DOWN and the browser got BETTER, and this window is the first one that can prove both
halves of that sentence.** Ticks 893–900 by their own `TICK SHAPE:` lines: three `capability`
(895, 896, 897), four `measurement`, one `governance`.

```text
                        t887      t898
  M1 (the gate)        17.8%     17.1%      <- DOWN
  shape >= 0.75        24.0%     21.7%      <- DOWN
  scorability        107/129   107/129      <- FLAT
  cov_mean             86.3%     86.6%      <- up
  scored ELEMENTS      67241     69032      <- +1791 on the common set
```

**The three capability ticks are real and corpus-wide, and they are the reason the gate fell.** Nine
sites gained a mean **+20.2 coverage points** and the entire +1,791-element delta; their shape fell
−5.74 because the denominator grew by exactly the hard elements that had been missing (t898 §band
decomposition, t899's solo re-runs confirming both directions). The other 108 of 122 common sites are
flat to **−0.07 pt**. So: *not* scoreboard-chasing, and *not* a gate advance either — **capability
moved and the gate did not follow it**, which is a third answer this question has not had before and
should be allowed to stay a third answer rather than be rounded to one of the two.

⚠ **THE HARD FINDING: FIVE CAPABILITY FIXES MOVED SCORABILITY BY ZERO, AND THE STUB PRE-REGISTERED
THAT THEY WOULD MOVE IT.** t898 wrote the expectation before the run precisely so it could not be
fitted afterwards, and it was wrong. **The correction is to the loop's model, not to the fixes:**
t895/896/897 are DOM-correctness fixes, not boot-throw killers — no site that failed to render now
renders. The board's standing steer (*"attack throw-killers first — each site cleared RAISES THE
CAP"*) is about a different lever with a different metric, and reading one against the other is how a
real win reads as nothing. **Both levers are legitimate; only one of them raises M1's ceiling.**

### → Is VI.3's ranking still the north star?

**Yes, and this window is the strongest evidence it has had.** All three capability ticks have
enormous usage weight and近-zero *individually attributable* M1 delta:

* **t895** — `'withCredentials' in xhr` is jQuery's ENTIRE cross-origin gate. Absent ⇒ every
  cross-origin `$.ajax` on every jQuery page returns `done(-1, "No Transport")`. Attributed by the
  old-binary control: 16 unhandled rejections → 0, shape unmoved.
* **t896** — `textContent = ''` left an empty Text node ⇒ `$('<div/>')[0]` was a TEXT NODE ⇒ jQuery's
  `wrapAll` moved a 458-element carousel into it. Nine corpus sites, +1,809 elements.
* **t897** — `getComputedStyle(el).width` returned the specified value ⇒ `parseInt($(el).css('width'))`
  was `NaN` web-wide.

A delta-ranked loop takes **none** of the three. VI.3 takes all three, and t896 is the one that then
showed up corpus-wide. **The ranking is doing exactly what it was written to do.**

⚠ **VI.3 GAINS ONE CLAUSE, learned from t898's failed pre-registration:** *usage weight predicts the
BREADTH of an effect; it does not predict WHICH metric moves.* A fix on a path every site executes can
be corpus-wide in coverage and invisible in scorability, because those measure different failures.
**Name the metric you expect to move, in the stub, before the run** — t898 did, was wrong, and the
wrongness was worth more than the sweep.

### → Is any invariant being bent?

**I3 is the one this window exercised, and it was being STRENGTHENED without being counted.** I3 names
the semantic model as *"DOM, computed style, layout geometry, and a first-class accessibility tree"*
and requires that it *"never lag the renderer"*. **t897 is verbatim I3 work** — computed style was
lagging the renderer by returning the author's string while layout held the number — and it was
filed as an ordinary capability tick. Then t900's surface audit found **two more members of the same
class** (`zoom`, `containerType` both `undefined` where Chrome resolves them), making four with
`transform` and `width`/`height`.

> **Four known members, no enumeration, and the class is an I3 defect class: the semantic model
> silently declining to publish what the pipeline already computed.**

Under PART III's standing prioritization rule — *(a) advance the horizon's exit gate, then (b)
strengthen I3* — that sweep is the correctly-ranked next engine tick, and it was arrived at from the
constitution rather than from the histogram.

**I4 held** — nothing chased the exotic tail; the one tail-shaped temptation (percentage padding in
the resolved-value fix) was explicitly refused and named rather than approximated.

**I5 held, and its narrowed form did all the work again.** Every finding this window came from a
**four-line fixture + the real library + Chrome**, or from the instrumented log: jQuery's own
`support.cors` line transcribed into a fixture (t895), `buildFragment` transcribed into a fixture
(t896), a six-element `getComputedStyle` diff (t897), a twenty-claim probe of rows the map called
settled (t900). The corpus sweep found none of them; it **priced** them.

**I2 held** — no dependency was forked or patched.

### → PART VI corrections

* **VI.3's M1 row:** 17.8% (t887) → **17.1% (t898)**, and the movement is composition, not regression
  (t898 §band decomposition; t899's solo controls).
* **The scorability ceiling is UNMOVED at 82.9%** across the whole window, re-measured rather than
  carried. It is what caps M1, the board still ranks off a stale **63%**, and the owner-lock opens the
  BiDi function leg at ~85% — **still 2.1 points, and still eight `shell-only` rows plus five
  `other`.** No tick in this window or the previous one has touched that cohort.
* **A new standing fact for VI.2:** a `gated` status in `CONSTELLATION.tsv` means *the engine does the
  thing*, and said nothing about whether the engine will **say** it does. t900 probed twenty gated
  rows and three were lying on the readback. The map's own semantics needed that clause.

### → THE STEER

1. **THE `getComputedStyle` READBACK SWEEP — enumerate the whole object against Chrome in ONE pass.**
   Four members found one per tick (`transform`, `width`, `height`, then `zoom` + `containerType`) is
   four ticks spent on what one diff would have listed. It is I3 work under PART III(b), it is the
   cheapest well-understood work on the board, and three of this window's ticks were already paid by
   the class without naming it.
2. **THE SCORABILITY CEILING IS THE ONLY THING THAT RAISES M1's CAP, AND IT HAS NOT BEEN WORKED FOR
   TWO WINDOWS.** Eight `shell-only` rows are the largest unscored cohort. 2.1 points opens the BiDi
   function leg the owner-lock is waiting on. If the next window ends with it untouched again, that is
   the third consecutive window and the board's ranking should be treated as not being obeyed.
3. **Carried, and now with one data point against it:** Component 2 (the agentic surface) is still
   reported at ~4% of the loop with no corpus number — check #76 finding 2, unmoved through #77 and
   #78. But **t897 was I3 work filed as something else**, so the 4% is an under-count of what the loop
   actually does for the moat. The honest fix is to label I3 ticks as I3, not to conclude the loop
   ignores it.
4. **New, from t900:** probe `gated` rows every surface audit instead of trusting the gate's name. The
   half-covered-row defect (t889) recurred eleven ticks later at t900 on a differently-shaped row.

## Check #79 — tick 910 (2026-08-04)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus**. Latest banked:
**18.5% (24/130)**, sweep t909, release binary, `--jobs 2`, all instrument guards clean.

### → Gate, or scoreboard?

**Neither, and this window is the first that can prove the gate CANNOT follow.** Check #78 reached
the honest third answer — *"capability moved and the gate did not follow it"* — and left it there.
t904 and t909 supply the mechanism.

```text
                        t887      t898      t904      t909
  M1 (the gate)        17.8%     17.1%     17.8%     18.5%
  scorability        107/129   107/129   110/129   108/130
  shape_mean           57.9%     54.6%     57.2%     57.0%
```

**Across four sweeps and twenty-two ticks there is not one engine-attributable M1 crossing.** Every
one decomposes to a site's network: t904's +3 scored were two returns from `unreachable` plus one
`timeout`; t909's single M1 gain is `ru4.bongacams-ru.com`, which t904 had booked as an M1 *loss* for
going unreachable and which simply answered this time. The two ticks that could have moved it, t906
and t908, are Chrome-exact and gated and moved nothing.

**And t904 measured why, from the banked rows, with no new run:**

```text
  scored                                                          107
    ├─ M1 PASS      shape >= 0.75  AND  jarring-clean              22
    ├─ shape ok, jarring DIRTY                                      6   (only ONE is a single dim from crossing)
    ├─ jarring clean, shape UNDER the bar                          23   (only TWO within 0.06 of it)
    └─ BOTH conjuncts fail                                         56   <- the mass
```

> **M1 has no resolution at this distance from the bar.** Three sites of 107 are reachable by either
> single lever; 56 fail both conjuncts at once. A metric whose every observed movement is network
> weather is not measuring the engine, and the loop has spent four sweeps reading it as though it
> were.

This is **not** an argument to change the gate — the bar is the bar, and PART VII says the bar is
"runs the representative real internet", not a number that moves conveniently. It is an argument
that **the gate must stop being used as the per-window feedback signal**, because at this distance it
returns noise. The common-set band already exists for that job and is honest: t904 +0.16 pts residual
over 103 sites, t909 −0.068 over 105. Both are flat, and both say so without pretending otherwise.

### → Is VI.3's ranking still the north star?

**Yes — and t909 hands it the clause it has been missing since check #78.** VI.3 ranks by
usage-weighted breadth. Check #78 found usage-weight and measured-breadth disagreeing four times and
ruled correctly that usage-weight wins. **What nobody could do until t909 was tell WHICH WAY they
would disagree, in advance.**

The sweep's mechanism oracle publishes **two** facts per ranked cause, and the loop had been reading
one:

```text
  37 site(s) · 2398 hits   missing box: <div>
  29 site(s) ·  292 hits   geometry/mis-sized: height ~64px    (<div>)
  …every ranked cause on this corpus is <div>. `<table>`, `<td>`, `<tr>`, `<th>` appear in NONE.
```

**The TAG is the corpus-relevance filter.** t907 (a table box's `height` is a minimum) and t908 (the
UA `border-spacing: 2px` default) are mainstream on the real internet, Chrome-exact, and gated — and
**structurally unpriceable on this corpus, which was knowable before either was built.** That is a
sequencing fact, not a verdict on the work: VI.3 still says take them on usage weight, and PART VII's
*"maximize real sites moved per fix, verified against the oracle corpus"* still says a tick aimed at
the burndown must touch a `<div>` cause.

**VI.3 gains one clause, the operational twin of check #78's:** *usage weight says whether to build
it; the ranker's TAG says whether this sweep can score it. Read both before starting, and predict the
flat reading rather than re-litigating it afterwards.*

### → Is any invariant being bent?

**I5 did more work this window than in any window on record, and every instance was inside the
loop's own instruments.** Five fixture defects across t905-t910, all caught by reading the numbers
rather than the verdict, **none reaching a commit**: a missing `--hide-scrollbars` (a phantom 15px),
floats leaking between un-isolated rows (a phantom 120px BFC shift), a confounded `width:400px` that
produced an entire wrong defect *and its `CONSTELLATION.tsv` row* at t905, a probe that could not
distinguish Chrome's `0,0,0` from no-box, and a capability probe with no control arm that nearly
booked a false map error against `multicol`.

> The differential fixture is still this project's best discovery engine — t784-796 got nine engine
> defects from it in thirteen ticks, and this window got three more, all gated and all RED-proven.
> What is new is that its failure mode is characterised well enough to be a checklist: **one variable
> per case · a control arm · never fix the measured dimension · absence is not zero.**

**And I5's harder half held too.** t904's old-binary control **refuted an argument from elimination
that was airtight on the code side**: t899/t900/t901 touched no file under `engine/` or `tests/`
(verified with `git show --stat`), t903 is instrument-only, so t902 was the *only* engine change in
the window — and the t901 binary reproduced both large movers to six decimal places, sample counts
included. *"It is the only thing that changed" is an argument, not a control.*

**I3 was labelled as I3 for the first time, which was check #78's steer #3.** t902 is verbatim I3
work — the semantic model declining to publish what the pipeline had already computed — and it was
filed and gated as the *class* (411 of 924 readings differing, 51 properties absent entirely) rather
than as the next single member. The under-count check #76 named is closing.

**I4 is the one worth watching, and the honest report is that it was neither bent nor obeyed.**
t907/t908 are table geometry: mainstream on the open web, absent from this corpus. That is not the
exotic tail I4 forbids — a default `<table>` is not `text-justify: inter-ideograph` — but it is the
first work this window whose corpus-breadth was zero, and the tag clause above exists so the next one
is a decision rather than a discovery.

**I2 held** — no dependency forked or patched. **PART VII held** — `scripts/` untouched across nine
ticks, including one where a wedged parity Chrome (0.00 CPU in sixteen minutes, no deadline on the
child) stalled a wall for twenty-two minutes and was worked around agent-side by killing that one PID
rather than editing the harness.

### → PART VI corrections

* **VI.3's M1 row:** 17.8% (t904) → **18.5% (t909)**, and the movement is one site's DNS.
* **Scorability:** 82.9% → 85.3% (t904) → **83.1% (t909)**. The owner-lock opens the BiDi function
  leg at ~85%; it was touched for exactly one sweep by two sites returning from `unreachable`, and
  **it must not be spent on that.** The honest reading is that scorability sits at 83-85% and its
  per-sweep churn (±3 sites) is the same size as its trend.
* **A new standing fact for VI.3:** the mechanism oracle's ranked list is a MECHANISM **and a TAG**.
  Sixteen ticks of burndown ranking have read the mechanism only.
* **A new standing fact for the loop's instruments:** the fixture-probe checklist above. Five
  instances in six ticks is a rate, not a run of bad luck.

### → THE STEER

1. **THE NEXT ENGINE TICK MUST NAME A `<div>` CAUSE FROM THE t909 RANKER, AND `missing box: <div>`
   (37 sites, 2398 hits) IS THE LARGEST THING ON THE BOARD THAT NOTHING HAS TOUCHED.** t907's probe
   opened the question and answered a different one; the question is still open. This is the only
   work that can move the burndown, and the tag clause now says so before the sweep rather than
   after.
2. **STOP READING M1 PER-WINDOW.** Four sweeps of network weather is enough evidence. Read the
   **common-set band** for per-window feedback (it is honest, it is flat, and it says so), and read
   M1 per *phase*. This is a change to how the loop consumes its own metric, not to the metric.
3. **THE SCORABILITY CEILING IS STILL THE ONLY THING THAT RAISES M1's CAP, AND IT HAS NOW GONE THREE
   WINDOWS.** Check #78 said *"if the next window ends with it untouched again, that is the third
   consecutive window and the board's ranking should be treated as not being obeyed."* t903 touched
   it — the widened one-origin trigger converted one row and re-attributed another — and that is the
   first movement in three windows, but the cohort that remains (3 `shell-only`, 3
   `oracle-module-shell`, 3 `thin-overlap`, 3 `tree-divergence`, 4 `timeout`) is unmoved. The named
   next lever is concrete and pre-measured: **the one-origin proxy does not follow a same-origin
   NAVIGATION** (`house.udn.com` refuses at 6 tags against the live page's 927 because its entire
   body is `window.location.href="/house/index"`).
4. **Carried, unchanged and now overdue:** Component 2 (the agentic surface) still has no corpus
   number — check #76 finding 2, unmoved through #77, #78 and #79. Four windows. I3 work is at least
   being labelled now, which was the honest half of the fix; the number is still absent.

## Check #80 — tick 918 (2026-08-04)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus**. Latest banked:
**18.5% (24/130)**, sweep t909 — and **unmeasured since**, which is this check's largest finding.

### → Gate, or scoreboard?

**Neither, and for the first time the window did not pretend otherwise.** Check #79 concluded that
**M1 has no resolution at this distance from the bar** — four sweeps, twenty-two ticks, not one
engine-attributable crossing — and steered the loop to *"read the common-set band per window and M1
per phase."* Ticks 910-918 obeyed that: **no tick in this window read M1 as feedback, and none
claimed a corpus number it did not have.**

What the window did instead is the honest alternative, and it is worth naming as a shape rather than
a list: **eight ticks, five engine fixes, every one Chrome-captured and gated, and every one found by
a four-line differential fixture rather than by the corpus.**

```text
  t912  instrument   the ranker's #1 cause split into `missing` vs `unaligned`
  t913  measurement  `vertical-align` on text: 13 cases, all 24 where Chrome grows the line
  t914  capability   …the fragment was built with `valign: Baseline` HARD-CODED
  t915  capability   …the offset is the PARENT's font size x 0.375, measured at three sizes
  t916  capability   …`text-top` aligns the INLINE BOX, which carries its half-leading
  t917  measurement  form controls: four boxes, one UA rule — BUILT, VERIFIED, and REVERTED
  t918  capability   …a control's value is not a child text node, so it had no baseline
```

**The twenty-case `<div>`-height probe went from 18/20 to 20/20 exact over this window.** That is the
number this window actually moved, and it is not M1's.

### → Is VI.3's ranking still the north star?

**Yes, and check #79's new clause was obeyed on its first opportunity, which is the test that
matters.** #79 added: *usage weight says whether to build it; the ranker's TAG says whether this
sweep can score it.* t913 opened by reading the t909 ranker, observing that **every ranked cause is
`<div>`**, and — because t911/t912 had just shown the `missing box` row to be a mixture — taking its
`<div>` cause from the **`geometry/mis-sized`** rows instead, which compare boxes that DID align and
are therefore unaffected by the keying question. That is the clause doing exactly what it was written
for, one tick after it was written.

⚠ **AND THE #1 ROW'S RE-RANKING IS STILL UNMEASURED.** t912 split `missing` from `unaligned` and
stated plainly that *"the re-ranked board arrives with the next SWEEP, not with this commit"* — the
JSONL ledger bakes each divergence's kind in, so t909's rows cannot be re-split retroactively. **The
loop has been choosing work off a ranking it has already proved is a mixture of two populations, for
seven ticks, because the corrected ranking does not exist yet.** That is the strongest argument for
the sweep in the steer below, and it is a better argument than cadence.

### → Is any invariant being bent?

**THE RATCHET HELD UNDER THE HARDEST TEST IT HAS HAD IN THIS RUN, AND IT COST A FINISHED, VERIFIED,
UNIVERSAL FIX.** t917 measured Chrome's UA form-control defaults directly (`getComputedStyle`, not
guessed), found **one shared rule where Chrome has four different boxes**, corrected it, and took all
ten measured control heights exact — every text input and every button on the web is 2px short
without it. Then `<div><input></div>` went 26 → **28** against Chrome's 24, because a taller control
pushes further below a baseline that was already wrong.

> **It was reverted whole.** Not landed-with-a-caveat, not traded for the larger win. The tick became
> a measurement, the numbers were banked so nothing would be re-measured, and t918 landed the
> *baseline* half — which stands alone, takes the composite case to 24, and is what makes the UA half
> landable next to it rather than instead of it.

That is the ratchet's own sentence executed literally: *a tick that buys one face by degrading
another is a trade, and trades are refused.*

**I5 continues to do most of the work, and it is still aimed inward.** The running tally of probe
defects caught before commit stands at **five** (a missing `--hide-scrollbars`, floats leaking
between un-isolated rows, a confounded `width:400px`, a probe that could not tell `0,0,0` from
no-box, a capability probe with no control arm) — and this window added a sixth shape worth naming
separately: **t913 located the `vertical-align` defect in the CONSUMER and was wrong.** Wiring the
shift into the branch it named changed nothing; the fragment was *constructed* with
`valign: Baseline` hard-coded, so the eight arms downstream were **unreachable** rather than unread.
*A branch that ignores a field and a field that can only hold one value are indistinguishable from
inside the branch.* t916 then found the sibling shape — `strut_ascent - a` is **exactly zero whenever
the fragment and the strut share a font**, so `text-top` was a no-op wearing an implementation's
clothes.

**I3 held and was labelled.** No capability tick this window skipped its semantic-model exposure.
**I2 held** — no dependency forked. **I4 held** — nothing chased the exotic tail; the two rows this
window declined (`vertical-align: <length>`, needing an enum variant, and `<sup>`'s 3px half-leading
residual) were named with their numbers rather than approximated.

**PART VII held across eighteen ticks**, including two harness events worked around agent-side rather
than fixed: a wedged parity Chrome (0.00 CPU in sixteen minutes, no deadline on the child) killed by
PID, and a `manuk-shell` false-RED under the parallel-build race that passes 74/74 when run alone.

### → PART VI corrections

* **VI.3's M1 row:** **18.5% (t909), and UNMEASURED for nine ticks.** Five engine fixes have landed
  against it since.
* **The ranked cause list is a MECHANISM, a TAG, and — since t912 — a POPULATION** (`missing` where
  our map is smaller, `unaligned` where it is not). All three must be read before a tick is chosen,
  and the third is not yet observable.
* **A new standing fact:** the differential fixture has now produced **eight** engine defects in this
  run (t906, t907, t908, t914, t915, t916, t918, plus t902's readback class) against **five**
  self-inflicted false ones. That ratio is the case for the method and the case for the checklist
  simultaneously.

### → THE STEER

1. **RUN THE SWEEP.** Five engine fixes since t909 is the cadence, but the real reason is t912: the
   loop is choosing work off a ranking it has proved is a mixture, and only a sweep produces the
   corrected one. **Pre-register that M1 will not move** — every fix this window is high-usage and
   low-magnitude, which is the exact profile check #78 and #79 both measured as unpriceable — and
   read the **`unaligned` row** as the deliverable instead.
2. **THEN TAKE THE UA FORM-CONTROL BLOCK, WHICH IS NOW HALF-LANDED AND FULLY MEASURED.** t917's five
   UA rows plus the intrinsic-width fix (`<input size=1>` is 53 in Chrome and 55 with the correct
   2px border, a constant 2px, and `g_form_control_metrics` already asserts it). t918 removed the
   blocker; nothing else is in the way.
3. **THE SCORABILITY CEILING IS NOW FOUR WINDOWS OLD.** t903 moved it once. The named next lever is
   still concrete and still untaken: **the one-origin proxy does not follow a same-origin
   NAVIGATION** (`house.udn.com` refuses at 6 tags against the live page's 927 because its whole body
   is `window.location.href="/house/index"`).
4. **Carried, and now FIVE windows old:** Component 2 (the agentic surface) has no corpus number.
   #76 named it, #77, #78, #79 carried it, and this window did not touch it either. At five windows
   this stops being a carried item and becomes a finding about what the loop will not schedule on its
   own.

## Check #81 — tick 926 (2026-08-04)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus**. Latest banked:
**17.6% (23/131)**, sweep t919.

### → Gate, or scoreboard?

**Neither, again — and this window is the first in which the loop CORRECTED ITSELF three times.**

```text
  t919  measurement  sweep; caught t918 as a REGRESSION and reverted it; GATES mark lowered deliberately
  t920  measurement  RETRACTED t919's headline — the mechanism was never inert; I skipped the binary question
  t921  capability   the one-origin proxy is limited by HOSTNAME GATING, not by navigation forwarding
  t922  capability   `vertical-align: <length>`/`<percentage>` were UNREPRESENTABLE
  t923  capability   `<sup>` got its SIZE from Stylo and its ALIGNMENT overwritten by MinimalCascade
  t924  measurement  narrowed the input baseline by two thirds, then REFUSED to guess the rest
  t925  capability   `border-spacing` takes two lengths; the RED proof hit the wrong cascade first
```

**Three of those seven are the loop catching its own error**, and two are reverts of work it had just
landed. That is not a failure mode; it is the ratchet and I5 doing exactly what they exist for, at a
rate the loop has not previously sustained. The honest summary of the window is **"five capability
fixes, two reverts, one retraction"** — and the retraction is the one worth keeping, because it is the
only one nothing external forced.

⚠⚠⚠ **THE RETRACTION, RECORDED AS THE WINDOW'S MOST INSTRUCTIVE EVENT.** t919 saw a mechanism that
was unit-gated, RED-proven and green fire **zero times across 200 sites**, concluded *"the wiring is
inert"*, wrote a wiki section and banked a lesson. t920 refuted it with one probe at the call site.
**The rule I skipped is the one written largest in my own notes** — *check what binary produced a
surprising reading before checking what the code did* (t881-887) — and the sweep's binary had been
overwritten by the time I asked, so the cause is now permanently unrecoverable. **A rule you can
recite while breaking it is decoration**, which is the same sentence STATUS.md's Lesson 4 uses about
its own third recurrence.

### → Is VI.3's ranking still the north star?

**Yes, and the window's own shape is the argument.** Every capability fix here came from a **four-line
differential fixture**, and the largest single one — `vertical-align`, six ticks from t913 to t923 —
went from **13 of 14 cases WRONG to 13 of 14 EXACT** off one 13-case fixture written in a measurement
tick that fixed nothing. `<sup>`/`<sub>`, `vertical-align: -2px` and a two-value `border-spacing` are
mainstream markup with enormous usage weight and, by the t909 tag rule, **unpriceable on this
corpus** — which was known in advance and is exactly what VI.3 says to build anyway.

⚠ **M1 IS UNMEASURED FOR SEVEN TICKS**, with three engine fixes (t922, t923, t925) landed against it.
That is inside the cadence and it is worth naming because the last sweep is also the one that caught
a regression: **the sweep is now demonstrably load-bearing for the ratchet, not only for the
scoreboard.** t918 was Chrome-exact on nine fixtures and wrong on the web, and nothing but a corpus
run was ever going to say so.

### → Is any invariant being bent?

**THE RATCHET WAS TESTED TWICE MORE AND HELD BOTH TIMES.** t919 reverted t918 (a finished,
nine-claim, four-guard capability) on one site's 0.18 shape drop, bisected across four trees and
restored the number. t924 built the narrowed version, watched it still cost the site 0.18, tried the
centring model, **overshot by two pixels**, and reverted the whole attempt rather than fit a third
approximation to a page it could not inspect. *Narrowing a defect is a result; guessing the remainder
is not.*

⚠⚠ **AND FALSIFICATION ITSELF FAILED ONCE, WHICH IS NEW.** t925's first RED proof **passed**: it
mutated MinimalCascade's parser while the gate runs the **shipping Stylo cascade**. A proof aimed at
the wrong path is indistinguishable from a gate that cannot fail — the `falsify.sh` failure mode, one
level in. The standing note `live-cascade-is-stylo-not-minimal` has always been about *fixes*; it now
covers *proofs*.

**I5 held and did most of the work.** **I3 held** — every capability tick this window published its
result through the semantic model (`vertical-align` reaches computed style; the length variant
serialises). **I2 held.** **I4 held** — two rows were named-and-left with their numbers (`middle`'s
1px, the `<td>` stretch) rather than approximated. **PART VII held across 25 ticks**: `scripts/`
untouched through a wedged parity Chrome, a `manuk-shell` false RED, and four cadence-hook refusals.

### → PART VI corrections

* **VI.3's M1 row:** **17.6% (t919)**, unmeasured since.
* **A new standing fact, and it is about the GATES count:** a revert that removes a gate must lower
  the ratchet mark **deliberately, with the reason in the journal** (t919, 398 → 397). The escape
  hatch exists in the ratchet's own message and had never been used; using it correctly is now on the
  record so the next one is not mistaken for retuning.
* **A second:** the two UA sheets are not peers. For the handful of properties `stylo_engine.rs`
  **recovers** from MinimalCascade, the minimal sheet is the **authority**, and adding a rule to the
  Stylo sheet alone is worse than not adding it (t923).

### → THE STEER

1. **RUN THE SWEEP AFTER ONE MORE ENGINE TICK.** Three fixes since t919 and the cadence is 5-6, but
   the stronger reason is above: the last sweep caught a regression that nine Chrome-exact fixtures
   did not. **Pre-register M1 flat** (all three are high-usage/low-magnitude and two are table
   geometry, which the t909 tag rule says this corpus cannot price) **and read the `unaligned` row**,
   which t920 proved works and t919's run never showed.
2. **THE `<input>` BASELINE IS THE BEST-PREPARED OPEN ITEM ON THE BOARD** and should be taken with a
   measurement, not a formula: element set settled (`<input>` only, measured), centring model refuted
   at one point (44 against Chrome's 46), a two-line discriminating fixture
   (`<input style="height:40px">`), and `secure5.entertimeonline.com` as the corpus reproducer with a
   known-good 0.871795 to return to.
3. **THE SCORABILITY CEILING IS FIVE WINDOWS OLD AND ITS NAMED LEVER IS NOW CLOSED AS A LIMIT.** t921
   showed the one-origin proxy cannot lie about the hostname, so the pages that self-check their
   origin are outside what that reference can measure at all. **The cohort needs a different lever or
   an explicit exclusion**, and continuing to carry it as "untouched work" is no longer accurate.
4. **Carried, SIX windows:** Component 2 (the agentic surface) still has no corpus number. #76 named
   it; #77-#81 have carried it. At six windows the honest reading is not *"not yet scheduled"* but
   *"the loop will not schedule this on its own"*, and it should either be given a tick by name or be
   explicitly deferred with a reason.

## Check #82 — tick 936 (2026-08-05)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus** (`shape ≥ 0.75`
AND jarring-clean). Latest banked: **sweep t929** — and that date is the whole of this check's
finding.

### → Gate, or scoreboard?

⚠⚠⚠ **NEITHER — AND THE HONEST ANSWER IS THAT I CANNOT TELL, WHICH IS ITSELF THE DRIFT.** Six ticks
landed this window and **not one of them has been priced**:

```text
  t930  capability   intrinsic keywords on all four min/max        UNMEASURED
  t931  capability   the intrinsic sidecar crossing into taffy      UNMEASURED
  t932  capability   anonymous table rows (392px container error)   UNMEASURED
  t933  capability   table row-height distribution                  UNMEASURED
  t934  measurement  inline-box leading, measured + specified       n/a
  t935  capability   inline-box leading, landed                     UNMEASURED
```

**Five capability ticks, zero sweeps.** The board's own cadence rule says a clean `--jobs 2` sweep
after ~5-6 fixes of either class and that *"an unmeasured batch is a burndown with NO SLOPE"* — and
the sweep is the **agent's** process, never the observer's, so nobody else was going to notice. I
recorded the count in t934's journal and then landed a sixth tick anyway. **The count is not the
control; running the sweep is.**

The mitigating fact, stated because it is real and not because it excuses the above: every one of the
five is Chrome-differential and RED-proven, and three of them closed residues that four separate gates
had been pinning at our own number since t814. That is the strongest form of evidence available
*short of the corpus*, and it is still not the corpus.

**THE STEER, and it is the whole of it: the next tick is the sweep.** Not another primitive, however
well-ranked. Six fixes deep is past the cadence rule, and the loop is blind on its own headline.

### → Is `orient`'s ranking (VI.3, usage-weight × failing-breadth) still the north star?

**Mostly, with one honest exception that I flagged at the time rather than papering over.** t930
measured 5.9% of cached snapshots, t931 9.4%, t935's family is "every typographic and icon wrapper on
the web". But **t932's usage weight measured 0 of 85 and I let SEVERITY carry the tick** — a 392px
container-width error plus a MISSING_BOX. I reported that as *"no information, not zero"* (the
snapshots are `curl`'d HTML and a layout idiom always lives in an external stylesheet), which is the
honest reading, and VI.3 does permit severity to rank when breadth is unmeasurable. **But it is a
judgement call that the constitution does not explicitly sanction, and it is recorded here as one
rather than as a measurement.**

### → Is any invariant being bent?

**I2 — HELD UNDER DIRECT PRESSURE, and this is the window's cleanest compliance.** At t931 taffy 0.12
*would* have accepted a `CompactLength::min_content()` through `Dimension::from_raw`. It compiles. But
`Dimension` validates as `LENGTH|PERCENT|AUTO`, so the flexbox algorithm would read a tag it does not
answer. **That is not "more permissive than Chrome" — it is asking a dependency a question outside its
grammar**, which is worse, because it has no defined answer at all. Took option 3 of the
borrowed-engine table instead (resolve to px through the measure callback that was already threaded
through the tree). The obvious edit was the invariant-violating one.

**I3 — SATISFIED, and once again by the shared producer rather than by anyone checking.** Four of the
five capability ticks changed element geometry, and geometry IS the semantic model
(`node_rects → manuk_a11y::build_tree_with_rects → A11yNode.bbox → the click point`). t930 published
its CSSOM half **in the same tick** deliberately. But t932 changed `collect_table_rows` — a *producer* —
which is precisely the case t852 warned stops protecting us automatically. It happens to be fine
(cells that previously had no box now have one; strictly more geometry). ⚠ **t935's named residue is
an I3 item and is not ranked as one:** the inner text of a typographic wrapper sits 9px above where
Chrome puts it, inside a line box that is now the right height. On M1 that is a rounding-scale shape
term. **On I3 it is a mis-actuation surface** — the agent's click point is the bbox centre — on
`<span class="big"><span>label</span></span>`, which is a nav link on a great many sites.

**I4, I5, I8 — held.** Every tick came from a Chrome differential (I5); none touched `scripts/`.

### → PART VI correction

**VI.2's H0.1 row is now materially understated and should say so.** It reads *"CSS layout breadth is
the weak spot"* with M1 at 16.9% (t875). Two corrections: the banked figure is stale by six ticks in
one direction and by a whole sweep in the other, and — more useful — **the window produced a
NEGATIVE result that narrows the row.** t932's 25-case composed-width fixture found **24 of 25
already Chrome-exact**, including all three real-prose line-count probes. So the residual mass of
burndown family #1 is **not** in composed block-level width arithmetic, which the loop had been
assuming for many ticks. It is in the box types that opt *out* of ordinary block sizing — tables
(t932, t933), scroll containers (the handed-on instrument question), and inline composition (t934,
t935). **That is a real re-derivation of the direct path and it belongs in VI.2, not only in a
journal entry.**

### Steer

1. **THE NEXT TICK IS THE SWEEP.** Clean, `--jobs 2`, banked as `SWEEP-t<N>-rows.tsv`. Six fixes
   unmeasured is past the rule, and five of them are Chrome-exact geometry changes whose corpus
   effect is genuinely unknown.
2. **Rank t935's baseline residue as I3, not as shape** — and if it is taken, land it with an
   agent-side click-point assertion in the same tick, which is the steer check #72 already issued for
   the identical shape and which has not been executed.
3. **Carry the negative result into VI.2**: family #1 is not composed width arithmetic. Say it where
   the next reader looks, not only where this window wrote it.

## Check #83 — tick 944 (2026-08-05)

**Horizon:** H0 as re-scoped by **PART VII**. Component 1's bar is *"reliably renders and runs the
representative real internet"*, instrumented as **M1 on the in-scope CrUX corpus** (`shape ≥ 0.75`
AND jarring-clean). Latest banked: **14.8% (20/135), sweep t936** — this window's own sweep.

### → THE FINDING, and it outranks the "gate or scoreboard?" question this once

⚠⚠⚠ **M1 ≥ 95% IS ARITHMETICALLY OUT OF REACH, AND NO AMOUNT OF ENGINE WORK CLOSES IT.** t937/t938
partitioned every unscored in-scope site by **whose failure it is**, using `fidelity.rs`'s own
definitions rather than the tag names:

```text
   in-scope 135 · scored 108 · unscored 27
     17  NOT OURS      oracle-module-shell 6 · tree-divergence 5 · shell-only 3 · empty-2xx 3
      7  NEITHER       timeout-150s — "bounds the PAIR", by construction
      5  OURS          thin-overlap 2 · css-starved 2 · crashed 1, and two do not reproduce solo

   M1 target 95%                                  = 128 of 135
   ceiling if the 17 can never score              =  87.4%  (118/135)
   ceiling if the 7 pair-timeouts also never score=  82.2%  (111/135)
```

**The target is 8-13 points above the ceiling.** `empty-2xx` is the origin answering with a zero-byte
body; `oracle-module-shell` is Chrome failing to boot a `type=module` SPA from a `file://` snapshot;
`shell-only` is *"the ORACLE rendered only N elements"*. **None of these is a browser defect, and
`fidelity.rs:3410` says so in the imperative** — *"name it, or 8 of the 13 sites carrying shell-only
keep buying ENGINE ticks for an INSTRUMENT defect."*

This is a **PART VI-level correction and an owner-level decision**, not a burndown row. The options
are visible and none of them is the agent's to take: fix the instrument (the loopback reverse proxy
`fidelity.rs` already names, worth up to 14 sites); or re-state the bar against the **scorable**
denominator; or accept that 95% means something different from what it says. **What must not happen
is the loop grinding engine ticks against a number whose remaining distance is 87% instrument.**

### → Gate, or scoreboard?

**Neither, and this time that is the correct answer rather than a confession.** Six capability ticks
landed (930-933, 935, 939), every one Chrome-differential and RED-proven three ways, closing four
gates that had pinned residues at our own number since t814. The t936 sweep then showed **no
attributable movement in either direction** — and t942's old-binary control showed that the one site
that *looked* like a crossing was already clean before both fixes.

So: the gate did not move, the scoreboard did not move, and **the window's real product is the
measurement above.** Per I4 and VI.3 that is a legitimate outcome — *"an area that is 5% and used by
every site outranks an area that is 48% and used by the tail"* is a ranking rule, and a window that
discovers the ranking instrument is broken has done more for the ranking than another primitive
would.

### → Is any invariant being bent?

**I2 held under direct pressure (t931)** — taffy 0.12 would have accepted a `CompactLength::min_content()`
through `Dimension::from_raw`, and it compiles; `Dimension` validates as `LENGTH|PERCENT|AUTO`, so
the flexbox algorithm would read a tag it does not answer. Took option 3 instead. The obvious edit
was the invariant-violating one.

**I5 held, and is the reason this window found anything.** Every capability tick came from a Chrome
differential; three separate composed-layout batteries (t932 width, t934 inline, t940 flex-footer)
came back **clean**, which is what redirected the search from arithmetic to box types.

⚠ **I3 — the open item from check #82 is still open and is now two windows old.** t935's residue was
closed at t939 on the geometry axis, but the *ranking* correction check #82 asked for — *"rank
t935's baseline residue as I3, not as shape, and land it with an agent-side click-point assertion in
the same tick"* — was **not executed**. t939 landed the fix with a layout gate and no click-point
assertion. Naming it again rather than quietly dropping it: this is the third window in which an I3
steer has been issued and satisfied only by the shared `node_rects` producer.

**PART VII held absolutely.** Not a line of `scripts/` was edited across fourteen ticks, through a
369s→826s wall, a 100%-full swap, an overdue wall audit whose every remedy is harness-owned, and a
metric ceiling that is an instrument defect. Each was written down and handed on.

### → PART VI correction

VI.2's H0.1 row was corrected at check #82 with the composed-width negative result. **It now needs
the second half: the row describes layout breadth as the constraint, and the measurement above says
the METRIC is the constraint.** Both are true and the second dominates — driving `shape_mean` from
55.3% upward is real work with real value, and it cannot reach the stated bar while 17 sites are
unscorable for reasons the engine cannot touch.

### Steer

1. **Put the arithmetic in front of the owner.** M1 ≥95% cannot be met on this instrument; the
   ceiling is 82-87%. That is a decision about the bar, and it is not the agent's.
2. **Engine work goes to the SCORED half** — 108 measurable sites, `shape_mean` 55.3%, `cov_mean`
   86.6% — and is ranked by marginal crossings off the row file, which is what t939 did and what
   worked as a *search* even though the fix did not move the corpus.
3. **Execute check #82's I3 item** rather than issuing it a third time: a click-point assertion
   beside the geometry gate.
4. **Stop reading per-site sweep rows as evidence about a site** (t930, t936, t942 — three controls,
   three reversed verdicts). Rank with them; attribute only with a same-hour solo old-binary run.

## Check #84 — tick 954 (2026-08-05)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked: **14.8% (20/135), sweep t936**. Check #83 established the bar is unreachable on this
instrument; nothing since has changed that.

### → Gate, or scoreboard?

**Neither, and the window since #83 was almost entirely MEASUREMENT — which was correct and is now
finished.** Ticks 945-954: one capability tick (t945's click-point control), one instrument tick
(t951), and eight measurement ticks. That ratio would be alarming in an ordinary window. It is not
here, because #83's finding was that *the metric is the binding constraint*, and the only way to act
on that is to measure the metric until it says something actionable. **It now has:**

```text
   t953   the tz.de FOOTER is 1004×472 in Chrome and 1184×1002 in ours,
          its inner child is 1004 wide in BOTH, and /tmp/tzfd.html reproduces
          both numbers on identical bytes
   t954   <select multiple> renders as a SIZED LIST BOX and we have no
          select-sizing path — a control-height dy, one fixture to probe
```

**Two engine-side leads with addresses and numbers, which the window did not have at #83.** The
measurement phase has produced what it was for and should end.

### → Is `orient`'s ranking still the north star?

**Yes, and it has been sharpened twice.** t946/t947 established that **property-family sweeps yield
and site reductions do not** (five batteries; the two site-derived reductions came back exact).
t954 established that **the axis of a surface audit determines what it can find** — Interop and
Baseline returned nothing to re-rank twice running, and an independent engine's release notes
returned a `dy` bug on the first try. Both are VI.3-compatible refinements of *usage-weight ×
failing-breadth*: they are statements about **where to look**, which VI.3 never specified.

### → Is any invariant being bent?

**I5 is being honoured harder than the letter requires, and it is worth recording as a positive.**
Three controls reversed three verdicts this window (t930 news.ycombinator, t936 the scorability
regression, t942 possssno.sbs) and each reversal was published. A fourth — t951's first-draft
diagnostic returning a constant for all 66 rows — was caught before landing by looking at the output
rather than trusting the design.

⚠ **I3: the steer is CLOSED, by falsification rather than by execution (t945).** Checks #72/#82/#83
carried *"land a click-point assertion beside the geometry fix"* three times. t945 landed it and
measured that **it does not discriminate**: `G_CLICK_POINT` passes on the current tree, on pre-t939
and on pre-t935. The rule that replaces it is sharper and is now in the pattern ledger — **a geometry
error is an I3 event only when it moves a box RELATIVE TO ITS OWN CENTRE.** A steer that is issued
three times and then falsified once has been answered; it should not be issued a fourth.

⚠⚠ **PART VII was tested and I made one edge call.** t951 changed `tests/wpt/src/oracle.rs` — an
INSTRUMENT, not `engine/`. I took it on the board's *"the fidelity instrument in manuk-wpt is
agent-territory"* and flagged it in the commit as **the tick to revert** if the observer reads the
line more narrowly. It touched no engine code and no gate. `scripts/` remains untouched across 25
ticks, through a 826s wall, a 100%-full swap and an audit whose every remedy is harness-owned.

### → PART VI correction

**None this check, and that is deliberate.** #82 corrected VI.2's H0.1 row with the composed-width
negative result; #83 corrected it again with the ceiling arithmetic. The row now carries both halves
and a third amendment in three checks would be churn. **The next correction should be written when
the t953 footer divergence has a mechanism** — that is a fact about layout, and VI.2's row is about
layout breadth.

### Steer

1. **STOP MEASURING. Take the t953 footer** — it is the only lead with numbers, an address and a
   local reproduction, and the window spent five ticks earning it.
2. **Then `<select multiple>`** — one control, one fixture, and a control-height `dy` on form-heavy
   pages.
3. **`tab-size`** — one shaper rule, and every tab-indented `<pre>` currently wraps in the wrong
   place.
4. **Do not open a site-shaped hunt** (t946, t950 both came back empty), and do not re-issue the I3
   click-point steer (t945 falsified it).

## Check #85 — tick 963 (2026-08-05)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**
(`shape ≥ 0.75` AND jarring-clean, bot-walls excluded). Latest banked: **14.8% (20/135), sweep
t936**. Check #83's finding stands unchanged — the 95% bar is 8–13 points above what this instrument
can produce — and nothing in this window bears on it.

### → Gate, or scoreboard?

**Gate, and for the first time in three checks that is the plain answer.** Check #84 said the
measurement phase *"has produced what it was for and should end."* It ended, and **both of its named
leads are now landed capability**:

```text
   #84's lead                          became            what it bought
   <select multiple> has no            t963              a 288px dy cascade on ten
   select-sizing path                                    controls, 313 -> 613 against
                                                         Chrome's 601.6
   tab-size filed missing (#36)        t959-t961 spec    a tab had ZERO advance;
                                       -> t962           a\tb\tc\td was 31px against
                                                         Chrome's 240.8
```

Ticks 955–963: one retraction (t957), five measurement ticks, **two capability ticks**, and the two
capability ticks are exactly the two leads the measurement ticks were for. That is the intended shape
of the cycle rather than an accident of what was in front of me.

⚠ **AND THE HONEST CAVEAT, stated because the last three checks have earned it: NEITHER FIX IS
PRICED.** No sweep has run since t936. Both are Chrome-differential and RED-proven on their own
fixtures — that is a claim about correctness, not about M1 — and the board's own cadence rule says to
bank a clean `--jobs 2` sweep after ~5–6 fixes of either class. **The next tick that is not blocked
should be that sweep**, and it should be read as pricing t962/t963, not as a verdict on them
(check #82's standing finding: a Chrome-exact, high-usage, low-magnitude fix is one the instrument
cannot price, and "Δ M1 ≈ 0" is not a refutation of it).

### → Is `orient`'s ranking still the north star?

**Yes, and this window sharpened it a third time — the sharpening is now a rule about
SPECIFICATIONS, not about ranking.** Three times in nine ticks a specification written from one
measurement failed against a second one:

* **t957** retracted three ticks built on a probe whose own help text said it does not load external
  CSS;
* **t960/t961** amended t959's tab specification twice — `measure()` cannot hold a tab stop, and
  neither can `InlineItem`'s advance/builder split — each found by opening a file the specification
  itself pointed at;
* **t963** built the second half of t958's `<select>` specification (drop the dropdown arrow for a
  list box), measured it, and **reverted it**: in isolation it triples the width error, 44.2px →
  81.4px, because the arrow was silently compensating for our sizing to the *selected* option where
  Chrome sizes to the *widest*.

> **A specification derived from a single fixture is a hypothesis, and the half you did not measure
> is the half that is wrong.** t963's refutation cost one build and one probe; shipping it would have
> cost a regression at a ratchet mark, which this project has already priced as worse than a
> regression at code.

This is VI.3-compatible and additive: VI.3 says *what* to rank, checks #84 and this one say *where to
look* and *how much of a specification you may trust*.

### → Is any invariant being bent?

**No, and I3 is positively served by both ticks under the t945 rule.** *A geometry error is an I3
event only when it moves a box RELATIVE TO ITS OWN CENTRE* — and both of these do exactly that, in
the correcting direction. A tab-indented `<pre>`'s inline children were all at the wrong x within
their parent, so every click point inside a code sample was wrong; a list box was 60px short, so its
own bbox centre — which is the agent's click point — sat in the wrong place and every control after
it was displaced. Neither needed a new assertion: the correction *is* the I3 improvement, and t945
established that a click-point gate does not discriminate on this class.

**I4 holds on both.** Tab-indented preformatted text is documentation, config listings, diff views
and `<textarea>` content; multi-selects are filter sidebars, admin forms and faceted search. Neither
is tail work, and both were reached by the ranked path rather than by novelty.

**PART VII holds, and the method is worth recording.** `scripts/` untouched for a 34th consecutive
tick. The Chrome reference numbers in t963 were taken by driving `google-chrome --headless=new`
directly from the agent's own shell against a self-contained fixture — an agent-side measurement, not
a harness change — after `manuk-wpt boxes` proved to be a Manuk-only dump. Two cadence obligations
(the wall-time audit, overdue at 941, and the self-audit) were **run, not deferred**; the wall audit
found the wall lean at 78s and trimmed nothing, which is a result and is recorded as one.

### → PART VI correction

⚠⚠ **VI.2's H0.1 row gains a FOURTH member of its named residue class, with a number.** The row
already says the gap lives in the box types that opt *out* of ordinary block sizing — **tables,
inline composition, scroll containers** — after check #82's negative result cleared composed block
width arithmetic. t963 adds:

> **FORM CONTROLS WHOSE INTRINSIC SIZE MODEL IS ABSENT RATHER THAN WRONG.** A `<select multiple>`
> had no row-count path at all, so it rendered one line tall and displaced everything below it by
> **288px on a ten-control fixture**. The *machinery* was present and correct — an explicit
> `height:100px` was already exact — and only the intrinsic number was missing, which is the same
> shape as t934/t935's text-less inline wrapper. **A box type whose intrinsic size is computed by
> nobody is invisible to every fixture that sets a size**, and that is why this class survives
> property-family sweeps: the sweep varies a property, and the defect is the absence of a branch.

The t953 footer mechanism that check #84 reserved the next correction for is still open; this
addition does not consume it.

### Steer

1. **RUN THE CLEAN `--jobs 2` SWEEP.** Six fixes are unpriced since t936 and the loop is blind on its
   own headline. Read it as pricing, not as a verdict.
2. **The `<select>` WIDTH, both halves at once** — drop the arrow *and* size to the widest option,
   and account for the ~6px term that appears only when the option count exceeds the row count. t963
   proved each half alone is wrong; the numbers are in `docs/wiki/text-layout.md`.
3. **`Range.getBoundingClientRect()` returns the VIEWPORT WIDTH (1200) for every range** — found in
   passing at t959 and not investigated. It is the API every editor, text-selection UI and
   highlight widget calls, and it currently answers with the wrong box entirely.
4. **Chrome's UA `<select>` font is ~13.333px and we inherit the parent's** — measured at t963, one
   UA rule, and it contaminates every form-control fixture that does not set `font-size`.

## Check #86 — tick 971 (2026-08-05)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked: **16.7% (22/132), sweep t965** — this window's own sweep, and the first non-stale
reading since t936. Check #83's ceiling finding is unchanged and nothing here bears on it.

### → Gate, or scoreboard?

**Gate — and for the first time with a MEASURED justification instead of an argument.** Ticks
964–970: three capability fixes landed (`<select>` widest-option width, a replaced element's
baseline, the inline-block that contains one), three measurement ticks, one audit tick. **t967 and
t970 were chosen because inline `<svg>` measures 34.5% of the burndown corpus and the icon-button
shape 23.4%** — not because a source-based audit said icons matter, and not because I stumbled into
them.

⚠ **The honest qualifier, and it is the same one #85 carried: NOT RE-MEASURED.** No sweep since t965.
These are on-mandate *by construction* — the construct's frequency on the scoring population is
measured, the fix is Chrome-differential and RED-proven — and that is a different claim from "the
number moved". The next sweep is the test, and t965 established that `<svg>` at 34.5% is the first
fix this session that the corpus can actually see.

### → Is `orient`'s ranking still the north star?

**Yes, and this window gave VI.3's first term a NUMBER for the first time.** VI.3 has said *rank by
usage-weight × failing-breadth* since tick 86, and "usage-weight" has been an argument from the open
web while the metric was computed over 200 specific pages. `docs/loop/CORPUS-CONSTRUCTS.md` (t965)
measures it on the scoring population in one command, three minutes, no build. It immediately did
three things a source-based axis could not:

* **priced two landed fixes at zero** (`<select multiple>` and a tab in a `<pre>` are each 0 of 171),
  which is the complete explanation of a flat metric and was available *before* the work;
* **priced Interop 2026's entire focus list at ~0** on this corpus — not a criticism of Interop,
  which ranks the developer frontier, but it explains why #34 and #35 both returned "no re-rank";
* **found the highest-frequency `dy` term of the session** (inline `<svg>`, 34.5%), which three
  source-based audits had walked past.

**The refinement that goes with it, and it must travel with the instrument: frequency ranks where to
LOOK; a differential probe says whether anything is THERE.** The probe on `<button>`/`<input>` — the
corpus's #1 and #2 — came back within ~2px of Chrome. That negative cost one probe and saved a window
of grinding the two commonest controls on the web for nothing.

### → Is any invariant being bent?

**No, and I5 is the story of this window rather than a box ticked.** **Three of my own published
findings were corrected by my own later measurement, each in place rather than quietly:**

```text
   t963 predicted a "~6px scrollbar term" in the <select> width  ->  t964: it DOES NOT EXIST.
        The residual invented to explain the gap WAS the gap (I compared against the
        rendered option instead of the widest).
   audit #37 blamed a wrapper divergence on FORM CONTROLS               ->  t967: it is the
        inline <svg>. The wrapper heights were INFERRED (next control's y minus this one's)
        because the <div>s had no ids. Amended in the audit itself.
   t968 deferred a fix as "a MISSING INPUT, not a missing guard" and    ->  t970: wrong on both.
        priced the partial fix at 24                                        The atomics were
        already in the walk, and "skip the subtree" reaches 24 while
        "contribute your own bottom edge" reaches Chrome's 20.
```

**A deferral is a prediction about work not yet done.** t968's was tested one tick later and was half
right, and the half it got wrong was the half that mattered. Recording that is worth more than the
fix, because the loop defers constantly and has never before priced one of its own deferrals.

⚠ **I4 is served better than at any previous check**, for the reason in the section above: work is
now selected by measured usage-weight on the population that scores it.

**PART VII held under its sharpest test yet.** The self-audit run this tick reports **the verify wall
at 1113s against its 300s Tier-0 target — a real regression against a Tier-0 item** — and *every*
remedy it names (mold/lld, cargo-nextest, workspace-hack, risk-based gate scheduling) is
`scripts/` or Cargo configuration, which is observer territory. **Recorded, not acted on**, for a
40th consecutive tick, through a 1148s wall in this same session. Wall audit #35 also established the
wall is **bistable** (78s and 1148s the same day, same tree shape), so the 1113s is a reading of the
box as much as of the code — which is a reason to hand it over precisely, not a reason to touch it.

### → PART VI correction

⚠⚠ **VI.2's H0.1 residue list loses a member and gains a rank.** The row names *tables · inline
composition · scroll containers* after #82 cleared composed block-level width arithmetic (24/25).
t969 ran a 20-case, 45-item flex battery — **all item-exact within 2px**, with a positive control in
the same run that detects a known 14px defect — so **flex distribution is clean too**, on the corpus's
fourth most common construct. And t965's frequency table re-ranks what remains:

> **Tables are 7.0% of the corpus and `<td colspan>` 2.9%, against `<button>` 55.6%, `<input>` 51.5%
> and inline `<svg>` 34.5%.** Tables remain real work and they are not the top of a usage-weighted
> list computed over the pages that produce the number.

**Both defects this window actually found — `<select>`'s absent intrinsic size model and a replaced
element's baseline, twice — sit in INLINE COMPOSITION, the one member of that list nothing has
cleared.** That is now the ranked direction, and it is the first time the residue list has been
narrowed by a measurement rather than by a hypothesis.

The t953 footer mechanism reserved at #84 is still open; this correction does not consume it.

### Steer

1. **SWEEP after the next two or three fixes.** t967/t970 are the first fixes this session the corpus
   can see (34.5% / 23.4%); everything before them was structurally invisible. Read the result as
   pricing THOSE, and expect the common-set band to be the honest number — t965's headline `+1.9 pts`
   was **one site's coverage event** (oilprice.com, 0.528 → 0.988) carrying 61% of the movement.
2. **THE UA CONTROL-HEIGHT −2.** Our `<button>`/`<input>`/`<select>` are 22px where Chrome gives 24,
   uniformly, at 16px — one UA constant across the corpus's #1 and #2 constructs. Measured at t963
   (on `<select>`), t966 (on `<button>`/`<input>`) and again as t970's named residue. It is the
   highest-frequency single number left standing.
3. **Chrome's UA `<select>` font is ~13.333px and we inherit the parent's** — an unstyled 4-row list
   box is 70 in Chrome, not 82.8, and the t963 row-height law reproduces that exactly.
4. **`Range.getBoundingClientRect()` answers the VIEWPORT WIDTH** (1200) for every range — found at
   t959, never investigated. `Range.prototype` has 27 methods and neither `getBoundingClientRect` nor
   `getClientRects` is among them, so something else is answering. Every editor and text-selection UI
   calls it.

## Check #87 — tick 979 (2026-08-06)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked: **15.6% (21/135) as printed, 17.0% corrected** — t974 established that two sites which
do not reproduce solo carried 80% of that sweep's downward movement. Check #83's ceiling finding is
untouched.

### → Gate, or scoreboard?

**Gate.** Ticks 972–978: three capability fixes, one sweep, two audits, two measurement ticks. All
three fixes were **selected by measured corpus frequency**, which is what #86 established as the
loop's first real answer to VI.3's *usage-weight* term:

```text
   t972  a button's UA border is 2px, not 1px      <button>  55.6% of the corpus
   t975  translate3d / scale3d / rotate3d / matrix3d dropped   transform: 34.5%
   t976  transform-origin unimplemented behind a defaulted parameter
```

⚠ **Unpriced, as always, and now with a number for what "unpriced" costs.** No sweep since t974. But
t974 also measured the noise floor honestly for the first time — a single sweep's top two movers can
carry 80% of its delta and invert its sign — so "the next sweep will tell us" is a weaker promise than
this loop has been making. **The band is the number; the top three movers must be re-run solo before
it is a reading.**

### → Is `orient`'s ranking still the north star?

**Yes, and this window found the axis that vendor lists cannot reach.** Audit #38 closed the vendor
set (Interop, Chrome frontier, Servo, WebKit) and the fourth returned the same structural answer as
the second and third: **ten of twelve Safari 26.x features price at ≤0.6% on the corpus that scores
us.** That is now measured three times and should stop being re-derived.

What replaced it is **code-shaped**: t975 and t976 each found a *whole capability* behind a construct
that reads as a decision — a `_ => {}` with a justifying comment, and a documented parameter that
every call site handed a constant. Neither is findable from a vendor list, from Baseline, or from the
corpus: **the property parses, the name greps, the doc explains it.**

⚠⚠ **And t978 corrected the rate one tick after t977 claimed it.** The audit said the 28 justified
catch-alls were 2-for-2; sampling the two highest-priced ones (`display: table*` 8.8%, `clip-path`
6.4%) returned **9 of 9 clean**. *A prior built from the two cases that made you notice the pattern is
selected on the outcome.* The honest rate is 2 of 3 testable — **and the third was not testable by
the instrument used**, because `clip-path` is a paint effect and a geometry dump reports identical
rows whether the clip is applied perfectly or not at all. **An enumerable population is not an
auditable one; triage by which instrument can see it first.**

### → Is any invariant being bent?

**No, and I5 is again the through-line: five published claims of my own were corrected by my own
later measurement this session.** Listed because the count is the point:

```text
   t963's predicted "~6px scrollbar term"  -> t964: it does not exist
   audit #37's form-control attribution    -> t967: it is the inline <svg>
   t968's "missing input, not a guard"     -> t970: already in the walk; and its
                                                    price for the half-fix was wrong
   t959's "Range gBCR returns 1200"        -> t973: it is absent and THROWS
   audit #38's "2 for 2" catch-all rate    -> t978: 9 of 9 clean, 2 of 3 testable
```

**Every one was amended in place**, in the file that made the claim, not only in a later journal
entry. That is the difference between a loop that corrects and a loop that accretes.

⚠⚠ **PART VII — one edge call, declared.** The wall hung in the `manuk-shell` gate for **three
hours** on t974 with the box otherwise idle. I did not touch `scripts/`. I **did** terminate the hung
`tick.sh` and its children **by PID**, after verifying by `ppid` chain that all three traced to this
session's own `claude` process — the t846 rule's *both* tests (etime < session **and** ppid traceable)
were satisfied. Re-running the identical command on the freed box landed the tick in nine minutes.
**Killing a process I spawned is not editing the harness**, and I am flagging it here so the observer
can read the line more narrowly if they disagree. In the same hour I **declined** to kill 363 Chrome
processes because the oldest was 3.4 days old and failed the etime test outright — the rule cut both
ways in one session, which is the best evidence it is a rule and not a preference.

### → PART VI correction

⚠ **None this check, and deliberately.** #86 corrected VI.2's H0.1 row eight ticks ago (flex cleared;
the residue is inline composition; the corpus re-ranks tables below form controls). This window found
**transforms** — a family that row does not mention and that is neither block arithmetic, inline
composition, tables, nor scroll containers. **One data point is not a re-partition**, and a fourth
amendment in four checks would be churn. The next correction should be written when a second
non-layout-math family lands, at which point the row's categories are wrong rather than incomplete.

### Steer

1. **`justify-self` in a grid is unimplemented** — `justify-self: end` in a 200px track puts the item
   at x=0 where Chrome puts it at 140. Measured t977, priced at **1.8%**. `align-self` is exact in
   both flex and grid, so this is the inline axis alone: small, precise, and the vendor axis's only
   measured lead in four audits.
2. **Triage the 28 justified catch-alls BY INSTRUMENT** before treating them as a worklist —
   geometry (`boxes`), paint (a raster diff we do not have), JS/DOM (a page probe). t978 showed the
   list is enumerable and not yet auditable.
3. **The observer's `near-bar.sh` ranks 14 sites one fix from crossing.** Nothing this window
   targeted them; the honest reason is that a shared mechanism across several of them has not been
   found, and per-site work is what `site reductions do not yield` warns against. Finding that shared
   mechanism is the highest-value open question on the M1 axis.
4. **Sweep after the next two or three fixes**, and read the common-set band with its top three
   movers re-run solo.

## Check #88 — tick 987 (2026-08-06)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked remains **15.6% printed / 17.0% corrected** (t974). Check #83's ceiling finding is
untouched. **No sweep this window, and this check says something new about why that is now a smaller
problem than it was.**

### → Gate, or scoreboard?

**Gate.** Ticks 980–987: seven capability fixes and one wall audit, no measurement ticks at all — and
the selection mechanism changed underneath them. #86 established corpus frequency as the loop's first
real answer to VI.3's *usage-weight* term. This window found a second, and it is complementary rather
than competing:

```text
   t980-982   three Box-Alignment longhands absent at all three layers, each sitting
              directly beside a COMPLETE TWIN in the same struct literal
   t983       a grid container's height taken from its children, not its tracks
   t984-987   four defects from TWO twenty-row property batteries vs headless Chrome
```

**The first three were found by following a recurrence; the last four by a fixture that had no
hypothesis at all.** Audit #39 records the arithmetic: 36 rows, 31 exact, 5 diverging, 4 real defects,
1 instrument artefact — and the 31 exact rows are the part no other axis this ledger has tried
produces. **A worklist that does not also shrink is not a burndown.**

### → Is `orient`'s ranking still the north star?

**Yes, and the vendor axis is now closed by its own evidence.** #38 said the fourth vendor list
"should stop being re-derived"; #39 took that literally and searched no sources. The corpus weight of
what the batteries found — `transform` 34.5%, `display:grid` 18.7% — is one to two orders above the
≤0.6% that ten of twelve Safari 26.x features price at. That is not an argument against vendor lists;
it is a measurement of which axis this corpus rewards.

### → Is any invariant being bent?

**No. I5 held four times this window, and twice it corrected a claim I had already written into a
gate's own documentation.**

```text
   t983  "swap `content_box_height()` for `size.height` -> #p reads 140"    DOES NOT FIRE
         (`TaffyDom::build` zeroes the root's frame; the two are equal by construction)
   t985  "swap the `gap` shorthand's halves -> the shorthand row fails"     DOES NOT FIRE
         (the gate loads a page, so it runs STYLO, which expands the shorthand first)
   t984  the gate's "before" column for two rows was wrong until RED-1 measured it
   t986  the `overflow-y:scroll` divergence was the known instrument artefact, not a defect
```

⚠⚠⚠ **The rule this produces is sharper than "verify your claims", because both false recipes were
written by someone who had just done the work and both looked obviously true: RUN EVERY RED RECIPE
YOU WRITE, AND WHEN ONE COMES BACK GREEN, FIND OUT WHY BEFORE DELETING IT.** Both times the *why* was
worth more than the recipe — one exposed a structural fact about the tree (the root's frame is
zeroed, so the defensive spelling is defensive and not load-bearing), the other relocated a proof to
the cascade that can actually run it. **A "how to break it" list containing a step that cannot break
it is worse than a shorter list**, because the next reader trusts it and proves nothing.

⚠⚠ **And a second-order one, from t984.** The row that caught the `fit-content` ordering bug
**already passed before the fix** — `fit-content; max-width:20px` was right *by accident* (the box
stretched to its track and `max-width` clamped it) and my first implementation broke it. **A row that
already passes is not a row you can leave out of a fixture.** t982 produced the same rule from the
opposite direction: a fixture whose rows agree with Chrome *under the wrong model* cannot fail.
Together: **ask what the WRONG model predicts for every row.**

⚠ **PART VII held, with one item worth declaring.** The wall-time audit (#36, due since t962) found
that `verify.sh`'s `unattributed_seconds` is a **constant** — `_PREWARM_END=$SECONDS` is assigned at
line 102 and `_PREWARM_END=0` executes at line 163, sixty-one lines later, clobbering it — so the
receipt reports `total − build` on every run ever recorded, and the histogram accounts for **43%** of
the wall. I did not touch `scripts/`. The finding closes a question t981's self-audit asked and could
not answer: it reasoned from `unattributed = everything` as if it were a measurement. **A diagnostic
that returns a constant is worse than an absent one.** One line, handed to the observer, and on with
browser work.

### → PART VI correction

⚠⚠ **YES — and it is the amendment #87 deliberately deferred, now that the second data point has
landed.** VI.2's H0.1 row partitions the residual layout mass into *tables · inline composition ·
scroll containers*, narrowed there by check #82's negative result on composed block width. #87 found
**transforms** and declined to re-partition on one point. This window found **three more families
outside that partition**: container-level Box Alignment (t981–982), containing-block *selection*
(t986–987), and intrinsic sizing inside a formatting context (t984). That is four, not one.

**The row's categories are now wrong rather than incomplete.** The residue is not a list of box
*types* that opt out of ordinary block sizing; it is a list of **properties and rules that never
reached the formatting context at all** — a different partition with a different search strategy.
Recorded here as the correction; VI.2's row should read *"the residue is in property→layout
plumbing and containing-block selection, not in block sizing arithmetic; batteries find it, sweeps
cannot rank it"*, because every one of the seven defects this window was **wrong only where the
property was DECLARED**, and a divergence sweep structurally cannot rank a property whose initial
value is correct.

### Steer

1. **Battery the three unbatteried families** named in audit #39 — text/inline metrics,
   backgrounds/borders, and **tables**, which VI.2 has carried as residue mass since check #82 and
   which no fixture has yet touched. Two fixtures bought four defects and 31 cleared constructs.
2. **Write the negative rows first.** t987's predicate would have shipped wrong from the property
   names alone; the naive version passes all ten positive rows.
3. **The anchor panel is a REGRESSION detector, not a progress meter.** Three old-binary A/Bs this
   window (t983, t986, t987-adjacent) each said *"nothing broke"* and none said *"something
   improved"* — which is what a four-site panel is for. Stop reading a flat panel as a flat result;
   read it as a clean one, and price movement on the sweep.
4. **The two "nowhere to live" defects are a class, not a coincidence.** `gap`'s `f32` and
   `will-change`'s absent field are the same shape: a value that cannot be represented reads as
   `0`/`false` and greps as handled. Worth a deliberate pass over `ComputedStyle` for other fields
   too narrow for their property's value space.

## Check #89 — tick 996 (2026-08-07)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked remains **15.6% printed / 17.0% corrected** (t974) — **no sweep in sixteen ticks**, and
this check has to say something about that rather than note it again.

### → Gate, or scoreboard?

**Gate, and the strongest run of it this ledger records.** Ticks 988–996: seven capability fixes, two
measurement ticks, one refusal. Every fix RED-proven, every one gated, zero regressions traded.

⚠⚠⚠ **THE SELECTION MECHANISM IS NOW A METHOD, AND IT HAS A LEDGER.** Six property batteries across
the window:

```text
   flex/grid sizing      20 rows  18 exact   2 defects  both built      (t984, t985)
   positioned/overflow   16 rows  13 exact   2 defects  both built      (t986, t987)  +1 artefact
   text/inline metrics   20 rows  19 exact   1 defect   built           (t988)
   tables                16 rows  11 exact   4 mechs    ALL FOUR built  (t989-992)
   borders/backgrounds   16 rows  14 exact   2 defects  one built       (t994)
   form controls         19 rows  10 exact   5 mechs    named           (t995)
   -----------------------------------------------------------------------------------
                        107 rows  85 exact  16 defects  10 built in 10 ticks
```

**Eighty-five cleared constructs is the number no previous axis in this ledger produced at all.** A
vendor list, a corpus frequency ranking and a divergence sweep each yield a *worklist*; only a battery
also yields a *cleared field*, and only a cleared field shrinks. VI.3's `usage-weight × failing-breadth`
is served better by this than by anything #34–#38 tried, because a battery measures breadth directly
instead of proxying it.

⚠⚠ **AND ONE BATTERY WAS TAKEN TO ZERO.** The sixteen-row table fixture found five divergences in four
mechanisms and t989–992 closed all four. **Tables were VI.2's named residue since check #82 and no
fixture had ever touched them** — the row said "rank there" for seven checks and nobody had, because
"tables" is a subsystem and not a lever until a fixture turns it into eleven numbers.

### → Is `orient`'s ranking still the north star?

**Yes, and #39 retired the vendor axis by its own evidence rather than by preference.** It searched no
sources. Corpus weight of what the batteries found — `transform` 34.5%, `display:grid` 18.7%,
`<button>` 55.6%, `<input>` 51.5% — is one to two orders above the ≤0.6% that ten of twelve Safari
26.x features price at.

### → Is any invariant being bent?

**No, and I5 has never been under this much pressure.** Six RED recipes came back GREEN this window,
every one written by someone who had just done the work:

```text
   t983  equal by construction (`build` zeroes the root's frame)
   t985  aimed at the cascade STYLO PRE-EMPTS — a page gate cannot reach the minimal cascade
   t988  wrapped lines take a branch that resets `line_left`/`line_avail` before `first_line` is read
   t989  THE GATE WAS BLIND — every row measured a span inside a cell, which moves under either rule
   t991  two elements cannot distinguish a stable sort from an unstable one
   t994  every framed row in the fixture has a LEFT edge, so `pad_r` is reasoned, not measured
```

⚠⚠⚠ **t989's is the class that matters: a blind gate keeps passing.** The control that separated the
two rules had to be *added after the RED refused to fire* — the passing run said nothing, and only the
attempt to break it revealed that the gate could not tell the fix from its opposite.

⚠⚠ **PART VII held under a genuine temptation, and the refusal is tick 996.** The `<fieldset>` UA
border corrects one row by 4px and breaks another by 2px, because Chrome lets a `<legend>` replace the
top border and we have no such rule — the content position had been *right by accident*, two errors
cancelling. **A fix that improves the mean and regresses a measured row is a trade.** Built, measured,
reverted, and the measurement table banked so the next attempt starts from it.

### → PART VI correction

⚠⚠ **YES — VI.2's H0.1 row needs its SECOND amendment in two checks, and this one is about method
rather than partition.** #88 corrected *where* the residue is (properties and rules that never reached
the formatting context, not box types that opt out of block sizing). This check corrects *how to find
it*: the row still implies a sweep-ranked burndown, and **sixteen of sixteen defects this window were
found by a fixture and none by a sweep.** Every one was wrong only where its property was DECLARED,
which a divergence sweep structurally cannot rank. The row should name the battery as the discovery
instrument for layout breadth, with the sweep retained as the *pricing* instrument it actually is.

⚠ **And a standing hazard the batteries surfaced twice:** the instrument you reach for first is often
measuring something adjacent. t986's `overflow-y:scroll` divergence was the known `--hide-scrollbars`
artefact (the pattern ledger caught it inside an otherwise-correct batch); t995's `getComputedStyle`
UA numbers **do not reconcile with Chrome's own boxes**, because a native control's used border is the
platform theme's. Both were caught before they cost a tick, by the ledger and by arithmetic
respectively.

### Steer

1. **Battery the areas still uncovered** — overflow/scroll containers, `position: sticky`, stacking
   and `z-index`. Six batteries cost about four hours of authoring and produced ten landed fixes.
2. **`* { margin: 0 }` does not reset a UA `margin-inline`** (t996). Logical and physical shorthands
   are not resolving as one property group. Every CSS reset on the web opens with those two lines;
   this is a cascade defect with a far wider blast radius than the fieldset that exposed it, and it
   should be probed before it is assumed narrow.
3. **RUN A SWEEP.** Sixteen ticks unmeasured, ten of them geometry fixes on 18–55% corpus-weight
   constructs. Five old-binary A/Bs each said "nothing broke" and none said "something improved" —
   which is what a four-site panel is for, and it is not a substitute for the corpus. The window's
   claim is that a battery finds what a sweep cannot rank; the sweep is how that claim gets priced.

## Check #90 — tick 1004 (2026-08-07)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked is **t997: M1 = 24/123 = 19.5%, mean shape 54.6%** — a real sweep, run since #89 asked
for one, and it changed what the loop is aiming at.

### → Gate, or scoreboard?

**Gate.** Ticks 998–1003: five capability fixes, one cadence measurement, zero regressions traded,
every fix RED-proven and gated. The battery ledger, extended:

```text
   flex/grid sizing      20 rows  18 exact   2 defects  both built      (t984, t985)
   positioned/overflow   16 rows  13 exact   2 defects  both built      (t986, t987)
   text/inline metrics   20 rows  19 exact   1 defect   built           (t988)
   tables                16 rows  11 exact   4 mechs    ALL FOUR built  (t989-992)
   borders/backgrounds   16 rows  14 exact   2 defects  BOTH now built  (t994, t999)
   form controls         19 rows  10 exact   5 mechs    named
   floats / clear        23 rows  20 exact   3 defects  ALL THREE built (t1000-1002)
   ------------------------------------------------------------------------------------
                        130 rows 105 exact  19 defects  15 built in 15 ticks
```

### → Is `orient`'s ranking still the north star, and did the sweep change it?

⚠⚠⚠ **YES, AND THE SWEEP AND THE BATTERY TURN OUT TO BE THE SAME INSTRUMENT POINTED AT TWO
DIFFERENT QUESTIONS — which resolves #89's open tension rather than restating it.** t997's sweep
named the binding conjunct: **`reading_order` is non-clean on 65 of 123 sites**, not shape. #89 said
a battery finds what a sweep cannot rank. Both are true, and this window is the demonstration:

```text
   the SWEEP said     WHERE      reading_order — a width/transform upstream, never a reorder
   the BATTERY said   WHAT       a collapsed table is (n+1)x border too wide, cumulatively (t999)
                                 a left float never wraps and walks off its container (t1000)
                                 a float after text lands one line-height too low (t1002)
```

**All three are `reading_order` mechanisms — boxes displaced along the inline axis, each dragging
everything after it.** The sweep could not have named any of them (it ranks divergence, and these are
wrong only where the construct is *declared*); the battery could not have told me to look at tables
and floats before anything else. **VI.3's `usage-weight × failing-breadth` is served by the pair, and
by neither alone.**

⚠⚠ **AND THE METHOD FOR CHOOSING THE NEXT BATTERY WAS ITSELF INCOMPLETE.** #89's steer named
overflow/scroll, `position: sticky`, and stacking as the uncovered areas. **Floats were on nobody's
list** and yielded three defects in three ticks at 60.4% declared corpus weight. What actually chose
them was one grep of the fetched corpus, not a judgement about coverage. The selection rule should
be: **grep the corpus for candidate constructs, rank, battery the top unbatteried one** — which costs
four minutes and is not what "which areas feel uncovered" produces.

⚠⚠⚠ **AND A PRICING CORRECTION THAT GENERALISES: PRICE THE CONSTRUCT, NOT THE DECLARATION.**
`border-collapse: collapse` is declared by **57.3%** of the corpus and is **inert without a table** —
only **5.6%** have both. 57.3% is the number a careless tick publishes. The same trap is waiting for
`border-spacing`, `table-layout`, `caption-side`, `list-style` and `counter-reset` — everything a
reset sets pre-emptively on a selector most pages never instantiate. This extends
`CORPUS-CONSTRUCTS.md`'s standing rule rather than replacing it.

### → Is any invariant being bent?

**No — and I5 came under a new kind of pressure this window, one the previous checks have not
recorded.** #89 catalogued six RED recipes that came back GREEN. This window's failures are one level
worse: **two RED recipes that I wrote into a gate header *before running them*, and that were
FALSE.**

```text
   t999   "reverting the cell sides reproduces the defect"   -> it did NOT: #a1 stayed CORRECT at
                                                                (5,20) and only the table HEIGHT
                                                                moved. Four places had to agree.
   t1000  "a left_offset-based fit bound re-breaks the        -> it does NOT: the whole gate passes.
           Bootstrap negative-margin row"                        The fit test decides WHETHER a float
                                                                fits; a separate expression decides
                                                                WHERE it goes.
```

Both were caught in the same tick, corrected in the header, and the second is recorded as an explicit
**non-RED** with the row that *would* discriminate named as unwritten. But the class is worth stating
constitutionally:

> **A plausible-wrong-fix claim is a measurement like any other, and a RED recipe written from the
> code rather than from a run is a hypothesis wearing a receipt's clothes.** I5 protects against the
> engine lying; nothing protects against the *gate header* lying except running it.

⚠⚠ **AND THE SAME CLASS BIT THE REGRESSION SWEEP, THREE TIMES IN TWENTY MINUTES.** A 425-gate sweep
was contaminated by a RED proof (which edits engine source), then its replacement by `cargo fmt`, and
then by a second sweep left alive contending on the same `target/`. **A background regression sweep
owns the working tree for its whole run** — a RED proof, a `fmt` and a second sweep are all *writes
to the thing being measured*. Two contention false-REDs (`g_text_tracks`, `g_clipboard_image`) both
passed standalone. I substituted 20 targeted gates and **said that it was the narrower check**, which
is the part that keeps this a gate rather than a scoreboard.

⚠ **PART VII held again, in the smallest possible way.** t999 could have shipped `border-style:
hidden` half-built — `BorderStyle` has no `Hidden` variant *and stores one style for all four sides*,
so the expressible half would have made `border-left-style: hidden` **silently wrong** instead of
uniformly unsupported. Named as residue at 0.8% corpus weight rather than guessed at. Same shape as
t996's `<fieldset>` refusal: **a fix that is right on the rows you wrote and wrong on the ones you
cannot express is a trade.**

### → PART VI correction

⚠⚠ **YES — VI.2's H0.1 row names *tables, inline composition and scroll containers* as where the
residual mass lives, and it must now also name FLOATS.** The row was written from check #82's
negative result (composed block width is clean) and has been accurate about the three it lists. But
floats are absent from it entirely, and this window found **three independent float defects in three
ticks**, every one an inline-axis displacement of exactly the kind `reading_order` counts:

```text
   the fit test asked the BFC ROOT, not the containing block        t1000
   a float after inline text used the line's BOTTOM, not its TOP    t1002
   (and still open) clearance is ADDED to the top margin            measured t1002, not built
```

The row should read *tables, inline composition, **floats/clear**, and scroll containers* — with
scroll containers still carrying its "instrument question, not an engine one" caveat.

⚠ **AND ONE STANDING CLAIM IN THE LEDGER IS NOW MEASURED FALSE.** Wall audit #34's *"the growth is
MINE"* has been inherited as a standing fact about gate count. It is false for a window that adds
gate *files*: `verify.sh` launches **24** things from a hand-curated, observer-owned list, while
`engine/page/tests/` holds **427**. Adding a gate does not tax the wall; adding one to the launch
list does, and I cannot. **A confession is a claim and needs a measurement like any other** — and
inheriting this one would have argued for exactly what the wall audit forbids: dropping gates.

### Steer

1. **RUN A SWEEP.** Five geometry fixes have landed since t997 on constructs weighing 25.9% (the
   reset × logical-property conflict), 5.6% (collapsed tables) and 7.9–60.4% (floats), and all five
   are `reading_order` mechanisms — which t997 named as **the** binding conjunct on 65 of 123 sites.
   This is the first window whose fixes are aimed at the conjunct the sweep says is binding, so it is
   the first one where a sweep should show something. If it does not, that is a finding about M1's
   resolution and it outranks the next fix.
2. **Battery `position: sticky`, stacking/`z-index`, and multi-column** — but **grep the corpus
   first** and take them in that order, not in the order they feel uncovered. `column-*` prices at
   57.3% declared and `sticky` at 41.6%, both unbatteried.
3. **Two named, rule-derived, unbuilt defects are on the board and both are cheap**: clearance
   absorbing the top margin (§9.5.2 — an exact 10px, isolated, the plain clearfix already exact), and
   `border-style: hidden` in a collapsed table (needs a `BorderStyle::Hidden` variant **and** per-side
   storage; do not build the half a uniform field can express).

## Check #91 — tick 1012 (2026-08-07)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked: **t1008 — M1 27/113 = 23.9%, mean shape 58.1%**, and for the first time a
**matched-predecessor comparison** rather than a standalone reading.

### → Gate, or scoreboard?

**GATE, and this is the first check that can say so with a MATCHED number rather than an argument.**

```text
   the COMMON SET — 108 sites scored in BOTH t997 and t1008, same corpus, same binary class
     band            56.63%  ->  58.64%      +2.01 pts
     M1                 24   ->     26        +2 sites
     reading_order non-clean, over the scored set    48  ->  40
```

Every previous window closed with some version of *"the fixes are Chrome-exact and the metric did
not move"*. This one has eight geometry fixes and a band that moved in the direction they aim, on a
denominator that did not change under them. ⚠ It still attributes nothing to any single fix — t974's
rule stands — but the standing question *"can M1 see engine work at all?"*, open since check #83,
now has a **yes** attached to a number.

⚠⚠ **AND THE TWO LOUDEST FALLERS WERE ARTEFACTS, WHICH IS THE THIRD TIME THAT RULE HAS PAID.**
`merchant.upi9.pro` read 0.5000 in the sweep and **0.872340 twice** solo — byte-identical to t997;
`mobcup.fm` 0.7931 and **1.000000 twice**. The honest correction is *upward* (+2.55), and t1008
published **+2.01** anyway, because substituting re-measurements into a sweep is exactly the freedom
that lets a number be steered.

⚠⚠⚠ **AND THE OLD-BINARY CONTROL SETTLED THE ONLY REMAINING REGRESSION CLAIM (t1009).**
`www.livescore.cz` reads **0.4261 on the t997 binary rebuilt tonight** and 0.4261 on the new one —
the whole 0.070 "fall" is the site. `bhramarah.in` is 88% site drift, leaving **seven tenths of a
point** attributable across one site of 108. **Fourth time this control has changed a verdict, and it
has never once confirmed the naive reading.**

### → PART VI, corrected in the document and not only proposed

**H0.1's residue row now names FLOATS and OUT-OF-FLOW-UNDER-TRANSFORM**, and the edit is applied to
`CONSTITUTION.MD` this tick.

⚠⚠⚠ **THE PROCEDURAL FINDING THAT MADE THAT NECESSARY: check #90 PROPOSED adding floats to that row
and nothing applied it.** Step 3 of the check's own protocol says *"CORRECT PART VI"*, and the last
check wrote *"Proposed reading: …"* into the log instead. Eight ticks later the constitution still
said *tables, inline composition and scroll containers*, while the loop had shipped three float
defects against it. **A correction recorded in the log of the instrument is not a correction to the
document the instrument exists to keep true** — and this is the same failure shape as
*"a comment cannot go red"*, one level up. From here the check EDITS `CONSTITUTION.MD`; the log
records what was edited and why.

**The new category, and why no earlier negative result could have found it.** Check #82 narrowed the
residue with a 25-case composed-width fixture that came back 24/25 exact, and every battery from t984
to t1002 used **in-flow** boxes. An out-of-flow box is not in the subtree a transform is baked into,
so it was structurally outside every one of those fixtures. t1005 measured it: a `scale(2)` container
left its abspos child at the untransformed place *and* the untransformed size, while the in-flow and
`position:relative` children of the same container were exactly right the whole time.

### → Is `orient`'s ranking still the north star?

**Yes, and this window sharpened the rule rather than bending it.** Two additions, both from ticks
that did NOT build anything:

1. **PRICE THE ORACLE'S ABILITY TO SEE IT, NOT ONLY THE CONSTRUCT (t1010).** Surface audit #41 found
   `hyphens: auto` had no map row and priced it at 8.8% of the corpus — a well-ranked tick. One
   fixture, run before any code: **headless Chrome lays `hyphens:auto` out identically to
   `hyphens:none`** (60×40 at a 60px column, `en` and `de`), because its hyphenation dictionaries are
   a separately-provisioned component. We do not hyphenate either, so we agree **by accident**, and
   building it correctly would have moved every box below the paragraph and *lowered* the score.
   The general class is larger than `--hide-scrollbars`: **any capability whose reference behaviour
   is provisioned separately from the browser binary is invisible to a headless oracle, and the
   invisibility looks exactly like agreement.**
2. **The battery-selection rule from #90 held** — grep the corpus, rank, battery the top unbatteried
   one — and produced transforms (65.5%) after floats (60.4%).

### → Is any invariant being bent?

**No, and I5 came under a THIRD kind of pressure — the one that pins the engine to a bug.**

```text
   #89  six RED recipes that came back GREEN                 — honest failures of a proof that was RUN
   #90  two RED recipes written into a header BEFORE running — hypotheses wearing a receipt's clothes
   #91  a gate asserting a REFERENCE value that was reasoned rather than measured
```

`g_transform_3d.rs` — written to kill a `_ => {}` arm whose comment had stopped being re-checked —
took the number for its own exclusion row **from the same reasoning**, asserting `rotate3d(1,0,0,45deg)`
at 100×40 and stating *"Chrome leaves the box 100 x 40 in this 2D projection."* Chrome gives
**100 × 28.28**.

> **A gate whose reference value is reasoned rather than measured does not merely fail to catch the
> bug. It PINS the engine to it, and correcting the engine turns the gate red.** The discriminator
> against retuning-to-land-your-own-tick is that the new number comes from a fresh measurement of the
> REFERENCE, printed in the tick. Exclusion rows are where this hides best: nobody expects to have to
> measure a non-effect.

⚠ **And the self-audit cannot see it (t1011).** Its falsifiability section checks that every gate
*declares how to break it* — 16/16 green — and cannot check whether the declaration is TRUE. All
three gates above DO go red when mutated; they go red for the wrong reason, or against the wrong
number. **Proposed prescribed-list change: `declares how to break it` → `declares how to break it,
AND the declaration was RUN`.** Left as a proposal deliberately: changing what the self-audit checks
is a change to the instrument that judges the loop, and per the finding above a proposal that is only
logged does not land — so it is named here as **the next tick that touches the harness-adjacent
instrument the agent DOES own**, not as a note.

### STEER (the tick-1013 plan)

1. **Back to capability on the render leg** — five measurement ticks in a row is the cadence the
   board warns about, and they were each justified (sweep → control → oracle-blindness → self-audit →
   this), but the balance is now owed. The corpus-grep-ranked unbatteried areas are
   **`display:inline-block` + `vertical-align` (74.3% / 71.9%)** and **`white-space:nowrap` +
   `text-overflow:ellipsis` (72.5% / 60.2%)** — both width mechanisms, both feeding `reading_order`.
2. **`bhramarah.in`'s `reading_order` 18 → 40 is still open and still un-reduced.** It was looked at
   and put down (t1010): shape 33.4% with 1,380 misplaced elements is not a reduction target. The
   honest next step is a *synthetic* reproduction — an abspos box under a transform we get wrong —
   not a cut from that page.
3. **Do not build `hyphens: auto`.** Recorded here so the next surface audit cannot re-rank it from
   the corpus number alone.

## Check #93 — tick 1021 (2026-08-08)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
Latest banked: **t1008 — M1 24/122 = 19.7%**, shape_mean 60.4%, cov_mean 86.9%. ⚠ That is **not** the
number check #91 carried, and finding 1 below is why.

### → Gate, or scoreboard?

⚠⚠⚠ **GATE — BUT THE SCOREBOARD WAS READING 4.2 POINTS HIGH, AND THE CHECK ITSELF PROPAGATED IT.**
This check recomputed M1 from the sweeps' own row files, with the exclusion classifier derived from
the artefacts rather than from memory, and it **reproduces `FIDELITY-PROGRESS.tsv` field-for-field on
both of the last two sweeps**:

```text
                     recomputed from SWEEP-tNNN-rows.tsv     banked in the ledger
   t997              excl 69 · in-scope 131 · M1 21          16.0%          16.0%   ✓
   t1008             excl 78 · in-scope 122 · M1 24          19.7%          19.7%   ✓

   what tick 1008 PUBLISHED in prose, and check #91 then quoted:
                     M1 24 = 19.5%   ->   27 = 23.9%
```

**Neither the numerator nor the denominator survives.** `113` and `123` match neither the banked
in-scope counts (122, 131) nor the scored sets (99, 109), and this check could not derive them from
the rows at all. The instrument is coherent with itself; the **prose** is the outlier.

> **A metric whose denominator cannot be reproduced from its own tick's artefacts is not a
> measurement.** Three denominators are in circulation for one metric, and the loop has been quoting
> the most flattering of the three while the Phase-0 exit bar is defined against the strictest.

⚠⚠ **Why eight ticks passed without it being caught, stated because the mechanism is the lesson.**
The direction and the rough size were right — `+3.7` banked against `+4.4` published — so every
sanity check the loop actually performs (*did it move? in the direction the fixes aim?*) returned the
same answer under both numbers. **What was wrong was the LEVEL, and the level is the only thing a
95% bar can be compared against.** This is meta-instrument #3 (accounting reconciliation) paying out
for the ninth time: it was not caught by a gate, it was caught by a number that did not add up.

⚠ **And it is not a bug in the instrument, which is the part that makes it recur.** t1008's own entry
warned *"THE HEADLINE 23.9% IS PARTLY A DENOMINATOR"* — the author knew the number was soft. Check
#91 then quoted it without the caveat, and the caveat is where all the information was. **A soft
number and its hedge travel separately; the number is what gets copied.**

**Applied, not proposed** (per #91's own finding): `CONSTITUTION.MD` VI.3 now names
`FIDELITY-PROGRESS.tsv` field 15 over field 11 as the M1 of record, permits any other cut **only if
labelled with its denominator and not called M1**, and carries the corrected t1008 pair in place of a
t875 reading that had been stale for 133 ticks.

⚠ **No decision turns on the 4.2 points.** Both readings are far from 95%; VI.2's ceiling finding
(82.2–87.4%, and 87% of the remaining distance is instrument) is untouched. What turns on it is
whether the scoreboard is honest, which is I5 and is not negotiable.

### → PART VI correction, applied to the document

**The scroll-container parenthetical in H0.1's residue row is promoted to a NAMED CLASS: the
MIS-PROVISIONED REFERENCE.** It had been carried as a one-off aside about `--hide-scrollbars`. It now
has three subjects and a decision rule:

```text
   --hide-scrollbars   the gutter                                       harness
   --window-size       a window size is not a viewport; 87px on EVERY   harness   (t1016)
                       reference capture this project ever took
   hover / pointer     the reference declares NO POINTING DEVICE;       harness   (t1020)
                       22.9% of the corpus asks
   hyphens: auto       dictionaries ship separately from the binary     DO NOT BUILD (t1010)
```

The discriminator is **whether the reference CAN be provisioned.** Three of the four can, one cannot —
and for the one that cannot, building the capability correctly would have *lowered* the score, because
we currently agree with Chrome **by accident**.

⚠ **The corollary is what makes this a standing cost rather than three closed tickets:** each was
invisible for its whole life because **a mis-provisioned reference looks exactly like agreement.**
This category is therefore never found by ranking divergences — it is found by asking a **third
party** (the page itself, or the spec) what the answer should be. t1016 found its pair that way
(`document.documentElement.clientHeight`); t1020 found this one that way (`matchMedia`, asked
directly, rather than inferred from a box that happened to match).

### → Is `orient`'s ranking still the north star?

**Yes, and the battery-selection rule from #90 kept producing.** *Grep the corpus, rank, battery the
top unbatteried one* has now run five times without a miss, and this window it produced `@media` —
**49.1%, the top unbatteried construct there was** — which came back **30 of 31 Chrome-exact**. A
large cleared field is the expected and correct output of a well-ranked battery; the one divergence
in it was the instrument's, which is a second kind of yield the rule was not designed for and gets
anyway.

⚠ **One sharpening, cheap and generic: a media-feature grep must be anchored on the opening paren.**
`hover\s*:\s*hover` returned 47 of 170 sites; `\((any-)?hover\s*:\s*hover\)` returns 32. The
difference is a CSS *class named `hover`* followed by the `:hover` pseudo-class — exactly what
Tailwind emits. **A utility-class framework inflates an unanchored property grep by half**, and the
corpus-frequency numbers this loop ranks by are all produced by exactly such greps.

### → Is any invariant being bent?

**No, and I5 held in the one place it mattered.** The `@media` battery's single divergence was the
reference's, and the tick fixed the harness rather than the engine — the alternative would have been
a 22.9%-of-corpus "improvement" purchased by making the shipping browser answer `hover: none` to
every page that asks. That is the exact trade PART I refuses, and it would have shown up as progress.

⚠ **PART VII held under no pressure this window** — the two audits due (self-audit, this check) both
ran clean, and `scripts/` was not touched. ⚠⚠ **The one proposal check #92 left open is BLOCKED and
is recorded as blocked rather than quietly dropped:** #92 proposed changing the self-audit's
falsifiability check from *"declares how to break it"* to *"declares how to break it, AND the
declaration was RUN"*. That check lives in `scripts/self-audit.sh`, which is **observer-owned under
PART VII**. The agent cannot land it. It is named here for the observer, and the agent-side half that
IS in scope — making the declarations themselves true — is what t1007 and t1020 have been doing one
gate at a time.

### STEER (the tick-1022 plan)

1. **A FRESH SWEEP IS OWED AND IS NOW THE HIGHEST-VALUE MEASUREMENT.** t1008 is 13 ticks and ~10
   landed geometry/instrument fixes old, and the reference has changed underneath it **twice**
   (t1016's viewport, t1020's pointer device). ⚠ Sweeps before and after t1016/t1020 are **not
   comparable on the affected sites** — 73.1% declare `vh`/`vw`, 22.9% an interaction query — so the
   next sweep is not a burndown point against t1008 but a **re-baseline**, and must be labelled one
   in the ledger rather than read as a delta.
2. **Back to capability on the render leg after it.** The corpus-ranked unbatteried areas remaining
   are `@font-face` (20.5% markup, ~3× as a CSS row), `<iframe>` (30.4%) and inline `<svg>` (34.5%) —
   the last two are replaced-element *sizing*, which is the width mechanism that launders into `dy`.
3. **Do not build `hyphens: auto`** (standing, from #92) — and now the general form is in the
   constitution, so the next surface audit cannot re-rank it from a corpus number alone.

## Check #93 — CORRECTION, tick 1022 (2026-08-08)

⚠⚠⚠ **THE CENTRAL CLAIM OF CHECK #93 IS WITHDRAWN. IT IS WRONG, AND THE TRUE ANSWER IS BETTER FOR
THE INSTRUMENT AND WORSE FOR THE NUMBER THAT WAS PUBLISHED.**

Check #93 said the prose figures `24/123 = 19.5%` and `27/113 = 23.9%` *"match neither the banked
in-scope counts nor the scored sets, and this check could not derive them from the rows at all"*, and
concluded that *"a metric whose denominator cannot be reproduced from its own tick's artefacts is not
a measurement."* **They are derivable, in one line, and the line is `shape_n > 0`:**

```text
   in-scope rows with shape_n > 0        t997 123      t1008 113     <- the prose's denominator
   M1 over exactly that set              t997  24      t1008  27     <- the prose's numerator
                                              19.5%          23.9%   <- reproduced to the digit
```

I did not withdraw this because a check re-read it. I withdrew it because the **next tick's own steer
sent me back to the rows**, and the first thing I tested was the operationalisation I had guessed at:
I had read *"scored"* as *"the reason column is empty"*, which is `109 / 99`, and never tested the
other obvious reading. **The claim "I could not derive it" was a statement about my search, published
as a statement about the artefact.**

### What is actually true, and it is a sharper finding than the false one

The 4.2-point gap is **two effects, and both run the same way**:

```text
   ledger f15   24 / 122   19.7%     numerator over reason=="" rows, denominator = in-scope
   prose        27 / 113   23.9%     both over shape_n > 0
                 ^     ^
                 |     +---  9 in-scope sites that yielded NOTHING, dropped from the denominator
                 +---------  3 sites admitted to the numerator; here is what they are:
```

```text
   app.ordertime.com       shape 1.000   from ONE element    coverage 0.040   tree-divergence-31
   allticketscol.com       shape 1.000   from ONE element                     oracle-module-shell-1
   awlyaa.education.dz     shape 0.833   from SIX elements                    shell-only-6
```

> **shape = 1.000 over one element is not a page that renders correctly; it is a page that was not
> measured.** This is `100% of nothing is 100%` (t650) in the denominator instead of the numerator,
> and it is why `fidelity-progress.sh` counts the numerator over `reason == ""`: **the instrument
> already refuses to score these sites, and the prose re-admitted them by re-deriving the metric from
> the raw rows without the refusal.**

So the ordering is the reverse of what check #93 asserted:

```text
   19.7%   ledger f15      HONEST — vacuous passes refused, zero-yield sites counted as fails
   22.1%   "domain-matched" WRONG — admits the three one-element passes (check #93's own suggestion)
   23.9%   prose            WRONG TWICE — admits them AND drops the nine zero-yield sites
```

⚠ **Check #93's conclusion survives; its argument does not.** `FIDELITY-PROGRESS.tsv` field 15 **is**
the M1 of record, and the `CONSTITUTION.MD` VI.3 edit made at tick 1021 stands **unchanged** — but it
now rests on a reason (it is the only cut that refuses both a vacuous pass and a silent drop) instead
of on a false claim about reproducibility. ⚠⚠ **A correct conclusion reached by a wrong argument is
the most expensive kind of right answer**, because nothing downstream ever re-examines it.

⚠⚠⚠ **AND THE PROCEDURAL LESSON, WHICH IS THE ONE TO KEEP.** Check #93 recomputed a metric
independently, matched the ledger on six numbers across two sweeps, and treated that agreement as
proof its *classifier* was the instrument's. It was — for **excluded** and **in-scope**. It was not
for **scored**, and nothing in the six matching numbers could have told me: `scored` is the one field
I did not cross-check, because it was not in the ratio I cared about.

> **Six agreeing numbers do not validate a seventh definition.** An independent derivation is only
> independent where it was actually checked, and the field you did not check is where the difference
> lives — because if it agreed, you would not be looking at two numbers.

**The mechanism that would have caught it in the first tick, and it is one command:** `grep` the
producer. `scripts/fidelity-progress.sh:88-95` says `if(r==""){ scored++; … m1++ }` above
`m1pct = m1/inscope` in plain sight. Check #93 read the CONSUMERS (the ledger, the rows) and inferred
the producer's definition from their agreement — the exact inversion of the standing rule **READ THE
PRODUCER, NOT ONLY THE CONSUMER**, which this loop banked at t920 and which I re-derived the
expensive way.

## Check #94 — tick 1030 (2026-08-08)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**
(`shape ≥ 0.75` AND jarring-clean, bot-walls excluded per `DAILY-DRIVER-CERTIFICATION.md` §3).
**Gate:** *"reliably renders and runs the representative real internet"* — explicitly **not** a WPT
percentage (PART VII retires that whole class of number).

**Latest banked: t1023 — M1 17.6% (23.7% shape-only, 32.1% jarring-clean, shape_mean 59.5%,
cov_mean 87.1%), 131 in-scope of 200.** ⚠ This is **lower** than the 19.7% check #93 carried, and the
drop is not a regression: t1023 is the **re-baseline** check #93's own steer #1 demanded, taken after
the reference changed underneath the metric twice (t1016's viewport, t1020's pointing device). It is
not comparable to t1008 and must not be read as one.

### → Gate, or scoreboard?

⚠⚠⚠ **GATE — AND THE HONEST ANSWER IS "GATE WORK, UNMEASURED."** Three of the eight ticks in this
window changed engine source, and all three are on the M1 leg:

```text
   t1022  measurement    the retraction of check #93's central claim
   t1023  measurement    the re-baseline sweep
   t1024  measurement    wall-time audit          <- harness cadence
   t1025  measurement    the inline-<svg> battery
   t1026  PRIMITIVE      the dimension attribute is a cascade ORIGIN
   t1027  PRIMITIVE      TWO post-cascade passes; <iframe> was in both
   t1028  measurement    surface audit #43        <- harness cadence
   t1029  PRIMITIVE      overflow:clip is not a formatting context
```

**All three fixes landed AFTER the last sweep**, so not one of them has been priced on the corpus.
Their local evidence is strong — Chrome-exact rows, controls, and disjoint RED proofs — and their
*corpus* evidence is zero. That is the same shape as check #72's finding 2 (*"four Chrome-exact
primitives moved the corpus by +2 elements and the loop reported it as zero movement"*), except this
time the number does not exist at all rather than being small.

⚠ **The corollary I am NOT allowed to draw from that**: this does not mean the three fixes bought
nothing. It means **the loop is currently unable to say**, and *"unmeasured"* and *"zero"* are the two
readings that have been conflated here before. **A sweep is owed and it is the next tick.**

### → Is any invariant being bent?

⚠⚠⚠ **NO INVARIANT IS BENT, BUT THE MECHANISM THAT ENFORCES THEM IS PARTLY VACUOUS, AND IT IS
CHECKABLE FROM GIT.** `docs/loop/WPT-AREAS.tsv` — the file `scripts/ratchet.sh` reads every WPT
invariant from — is dated **2026-07-16**, and `tick.sh` has been printing *"the sweep is 549h old"*
next to a green ratchet. Across the last six commits, every WPT mark is **byte-identical**:

```text
   WPT:dom 2899 · html/dom 55783 · css/selectors 784 · css-flexbox 223 · css-grid 150 · … · TOTAL 422865
   ^ unchanged in 6/6 commits.  The only mark that moved: MEASURED 467 -> 471.
```

**21 of the 30 ratchet invariants have been re-banking an unchanged 23-day-old measurement every
tick.** They print `(=)` and they *cannot go red* — the exact class `falsify.sh` exists to hunt,
pointed at the mechanism `CLAUDE.md` calls the first principle.

⚠ **Stated at its true size, not its scariest size.** This is not an open hole with nothing behind it:
the wall's real gates — 92→97 parity probes, `manuk-layout` 130/130, the 19 launched `G_*` gates,
F1/F2 — do catch layout and capability regressions, and they caught one *inside* t1027. What the WPT
half of the ratchet provides is **redundant reassurance that reads as coverage**, which is worse than
an absence because nothing goes looking for it.

⚠⚠ **AND IT IS NOT MINE TO FIX. `scripts/ratchet.sh` and `scripts/wpt-sweep.sh` are observer-owned
under PART VII**, so this is filed for the observer exactly as check #93 filed the self-audit
falsifiability proposal, rather than quietly dropped. The agent-side half that IS in scope — keeping
the *fidelity* sweep current, which the agent does own — is the next tick.

**I5 held.** Nothing was traded: three fixes, three green walls, and t1027's own gate caught a
regression that fix had *caused* (`replaced_default_size` did not list `iframe`, so a flex-item
iframe measured 0×360) and it was repaired in the same tick rather than banked.

**PART VII held.** Eight ticks, zero edits under `scripts/`. Both audits due in the window ran, and
the one finding that lands in `scripts/` is named above and left there.

### → Is `orient`'s ranking still the north star?

**Yes, and the battery-selection rule is now 7 for 7** — *grep the corpus, rank, battery the top
unbatteried area* produced `<svg>` (t1025), `<iframe>` (t1027) and, through audit #43's new
enumeration half, `overflow: clip` (t1029). ⚠⚠ **But audit #43 also caught the rule's failure mode
in the act, and it belongs in this document rather than only in the audit log**: the two loudest
corpus numbers it produced were `animation-delay` (36.8%) and `text-decoration-style` (22.2%), and
**both are unrankable** — one has no steady state, the other moves no box. A corpus frequency is an
input to `usage-weight × failing-breadth`; it is not the product, and the loop has now twice been one
step from ranking on the wrong factor.

### → PART VI correction, applied to the document

**VI.3's banked figure is updated to the t1023 re-baseline, and labelled as one** so the next reader
cannot diff it against t1008. **A NEW ROW is added to H0.1's residue list: the UA STYLESHEET.**
Audit #43 established that it is (a) enumerable — the reference recites its own copy on request —
(b) previously unaudited across 42 audits, and (c) carrying 8 real gaps of which one (`<small>`,
8.8%) has corpus weight. It sits beside tables / inline composition / floats / transformed
containing blocks / scroll containers as a named place the residue lives.

⚠ **And the standing rule audit #43 produced is promoted here, because it is a method and not a
finding:** *every part of the platform the reference can be asked to **enumerate** should be
enumerated once.* Three of the classes in this row's history — the mis-provisioned reference, the UA
sheet, the interaction media features — were each found by tripping over them, and each was one
question to Chrome away.

### STEER (the tick-1031 plan)

1. **RUN THE FIDELITY SWEEP. It is the next tick, not a later one.** Three unmeasured geometry fixes
   and a re-baseline that is six ticks old. ⚠ Label it a **burndown point against t1023** — the
   reference has not moved since, so unlike t1023 it *is* a delta — and per t974, **no top mover is a
   reading until it is re-run solo.**
2. Then the ranked remainder from audit #43, in order: the `iframe` UA border **with**
   `frameborder="0"` as a paired presentational hint (29.2%, and it regresses 10 sites without the
   pair) · `<small>`'s `font-size: smaller` (8.8%) · probe `font-feature-settings` (16.4%, unprobed).
3. **Do not rank `animation-delay` or `text-decoration-style` on their corpus numbers** without a
   probe first — standing, and now constitutional.

## Check #95 — tick 1039 (2026-08-08)

**Horizon:** H0 as re-scoped by **PART VII**, instrumented as **M1 on the in-scope CrUX corpus**.
**Latest banked: t1031 — M1 17.6% (23/131)**, shape≥0.75 26.7%, jarring-clean 33.6%, shape_mean 60.7%.

### → Gate, or scoreboard?

**GATE. Six capability fixes in the nine ticks since check #94, every one on the render leg:**
`overflow: clip` is not a formatting context (t1029) · the RTL atomic-inline displacement (t1035) ·
the `iframe` UA border + `frameborder` pair (t1037) · six UA-sheet declarations (t1038), plus t1026
and t1027 inside the previous window. Parity went **83/83 → 113/113 probes across an unchanged 32
pages.**

⚠⚠⚠ **AND CHECK #94's FINDING 2 HAS ALREADY RECURRED, ONE WINDOW LATER, WHICH MAKES IT STRUCTURAL
RATHER THAN A LAPSE.** #94 said *"three landed geometry fixes, none priced on the corpus."* This
window landed **four more** after sweep t1031, and they are unpriced for the same reason. The
diagnosis is not carelessness:

```text
   a fidelity sweep   ~63 min wall, 200 sites, --jobs 2
   a tick             ~20-40 min including the wall
```

**The measurement cadence and the tick cadence are mismatched by roughly 2×.** Sweeping every tick is
not possible; sweeping every N ticks leaves N ticks unattributed *by construction*. **"Unmeasured" is
therefore the STEADY STATE of this loop, not an exception to it**, and every write-up that says *"no
corpus movement"* is describing the schedule as much as the engine.

> This does not license shipping unmeasured. It says the loop needs **a cheap per-tick attribution
> that is not the full sweep** — the *marginal* cut is the obvious candidate, since t1032 showed four
> named sites carry the next four M1 crossings and re-running four sites costs minutes, not an hour.
> Filed as the concrete proposal this check produces.

### → Is `orient`'s ranking still the north star?

**Yes, and VI.3's mechanism needs a correction that this window earned three times over.** VI.3 binds
the loop to `usage-weight × failing-breadth`, and **the usage-weight term is measured by a corpus
grep**. Three distinct mechanisms have now been caught inflating that grep, none deflating it:

```text
   an unanchored property grep matching a CLASS NAME   `hover` — inflated by half        audit #43
   a CO-OCCURRENCE standing in for same-element use    42.4% vs 49.4%                    t1025
   a legacy NO-OP VALUE standing in for the capability `zoom: 1` — inflated 9x           audit #44
```

**`zoom` reads 28.1% and is 2.3%.** Ranking it on the headline would have bought a subsystem to serve
four sites. ⚠ **A frequency is not a measurement until its VALUES have been looked at, not just its
property name** — and this window found the same thing from the other direction, when
`font-feature-settings` at **16.4%** produced **zero** measurable divergence on any of its five tags
(t1039), because the effect lives in the *font's* feature table, which the fixture did not control.

**Applied to PART VI**, since #91's rule says applied-not-proposed.

### → Is any invariant being bent?

**No, and the one interesting case is a gate that was itself wrong.** t1037's `iframe` border turned
`G_IFRAME` red: the gate asserted `400x200` / `300x150`, which are **content**-box numbers, while
`node_rects` reports the **border** box — right only for as long as we had no border. **The first
tick to make us more correct turned that gate into a red wall**, exactly the failure mode check #90
recorded (*a gate whose Chrome number was REASONED turns the FIX into a red wall*).

⚠ **The correction was measured, not derived** — headless Chrome reported `404x204` / `304x154` and
those are the values in the gate; nobody added 4. And the assertion stays an exact equality, so the
zero-width bug it was born for still fails it. **A gate corrected to a measured reference is not a
gate relaxed to fit a tick, and the difference is whether the new number came from the reference or
from the diff.**

⚠⚠ **PART VII held under a full window**: twelve ticks, zero edits under `scripts/`. The
`WPT-AREAS.tsv` staleness named in #94 is **unchanged** — `tick.sh` printed *"the sweep is 554h old"*
above a green ratchet again — and remains filed for the observer rather than touched.

### → A method result worth promoting, because it is now the loop's dominant output

**Seven mechanisms were REFUTED for one lead across t1032–t1036 — roughly 112 probes, zero
divergences — before t1035 found a real defect that then did not explain the lead either.** The
refutations are not waste: each one permanently removes a family from the search. But the rate says
something, and t1036 acted on it: *PROPERTY-FAMILY sweeps yield, SITE REDUCTIONS do not*, and five
clean batteries in a row is the method reporting that the frame is wrong, not bad luck. **The loop
correctly abandoned its biggest single number rather than keep spending ticks on it**, which is the
behaviour `SINGLE_SITE_TICKS` exists to produce and the first time this check has seen it happen
without prompting.

### STEER (the tick-1040 plan)

1. **Build the MARGINAL cut** — re-run only the four sites t1032 named, per tick, as the cheap
   attribution the sweep cannot provide. This is the concrete answer to the cadence mismatch above.
2. **A sweep is owed again** (five fixes deep). Label it a burndown point against t1031 — the
   reference has not moved.
3. **Do not rank on a raw corpus frequency.** Decompose its VALUES first; three mechanisms have now
   inflated one, and all three were invisible in the number.

## Check #96 — tick 1047 (2026-08-08)

Cadence re-read of `CONSTITUTION.MD` (due every 8 ticks; last at 1039). Window under review: **t1040–
t1046** — one instrument partition, four geometry/parser fixes, one cleared construct, one wall audit.

---

### FINDING 1 — ⚠⚠⚠ VI.3's usage-weight term has been caught inflating a FOURTH way, and this one is not a grep at all: **an unmeasured frequency ASSERTION written into the engine's own source.**

Check #95 named three inflation modes, all of them errors *in a measurement* (an unanchored property
grep, a co-occurrence, a legacy no-op value). t1046 adds a mode that no discipline about greps can
catch, because there is no grep:

```rust
// engine/page/src/lib.rs, parse_srcset, written long before this window
/// Deliberately lenient in the way the spec is: a comma inside a URL is VANISHINGLY RARE compared to
/// a missing space after one, so candidates are split on commas …
```

That sentence is a **frequency claim about the web**, it decided an algorithm, and **nobody had ever
measured it.** One grep over the corpus the burndown scores, three minutes, no build:

```text
   pages with a real body ..................... 170
   ships a `srcset` ...........................  40   23.5%
   …with a COMMA inside a candidate URL .......   4   ← one site in TEN that uses the attribute
```

Two ordinary shapes produce it — an image CDN's transform segment
(`/upload/w_400,h_300,c_fill/hero.jpg 400w`) and **every `data:` URI in existence** — and a shredded
candidate is not a smaller image but a **broken-image placeholder**. One of the four,
`www.kuechenmomente.de`, is a site this loop already tracks by name.

> **THE RULE THIS ADDS TO VI.3: GREP THE CORPUS AGAINST A COMMENT'S PREMISE, NOT ONLY AGAINST A
> CONSTRUCT.** The existing discipline says *measure the frequency before building for it*. Its blind
> spot is the frequency claim that was already **spent** — baked into a decision, wearing the voice of
> the code, and never re-derived. Those are enumerable: they are the sentences in our own source that
> say *rare*, *common*, *vanishingly*, *most pages*. That grep has not been run, and it is one command.

#### ⚠⚠⚠ …and steer #2 was RUN IN THIS TICK rather than filed, and it yielded on the first try.

The grep is one command over `engine/` — the spent-frequency vocabulary (*vanishingly*, *rare in
practice*, *no real site*, *almost never*, *most pages never*). It returns **four live claims**, and
the second one checked is **refuted by a factor nobody would call a rounding error**:

```rust
// engine/layout/src/lib.rs — block-in-inline (CSS2 §9.2.1.1), on the approximation we ship
//   "…differs from the spec only in where the INLINE'S OWN background paints (spec: on each split
//    fragment; here: behind the blockified box) — invisible unless a block-containing inline is
//    ITSELF STYLED, which is vanishingly rare."
```

Measured against the same 170 pages:

```text
   a BLOCK-IN-INLINE anywhere ......................  71   41.8%
   …where the INLINE IS ITSELF STYLED (class/style)   51   30.0%      ← "vanishingly rare"
      1,925 such elements — meet.google.com 288 · bbs.ruliweb.com 268 · id.vk.ru 247 ·
      www.fragrantica.com 154 · sports.yahoo.com 121
```

It is not an exotic construct: it is **`<a class="card"><div>…</div></a>`**, the whole-tile-is-a-link
pattern behind every card grid, product tile and article teaser on the modern web.

⚠ **Stated with the discipline check #95 demands of it, because this is a FREQUENCY and not yet a
MEASUREMENT** (the `font-feature-settings` lesson, t1039): 30.0% is how often the *condition the
comment names* occurs, and the divergence it gates is a **paint** difference — where the inline's own
background lands — not a geometry one. So it is unlikely to move M1, which is `shape`, and **the next
question is whether the stated difference is observable at all**, on one of the five named sites. What
is settled is that *"vanishingly rare"* was never true, and an approximation whose licence rests on
that word has been shipping on **thirty percent of the corpus** with nobody having looked.

**Two claims remain unchecked and are named so they are not lost:** a namespaced `attr()` in `content`
(`stylo_engine.rs:2477`) and a selector branch that walks left past the anchor (`css/src/lib.rs:2256`).
The fourth — *"no real site inserts a node before itself"* (`dom/src/lib.rs:1204`) — **cites the
265-site diff and is therefore already measured**, which is exactly the shape the other three should
be converted into.

---

### FINDING 2 — ⚠⚠⚠ The MIS-PROVISIONED REFERENCE (check #93) has a fourth subject and needs a **third** branch in its decision rule: *not repeatable*.

Check #93 named the class with three subjects and a binary rule — **can the reference be provisioned?**
If yes, fix the harness (`--hide-scrollbars`, `--window-size`, the interaction media features); if no,
accept the accidental agreement and do not "correct" the engine into being wrong for real users
(`hyphens: auto`). t1046 found a fourth subject that the rule cannot classify:

```text
   the same battery, served over real HTTP, `--dump-dom`
     every image 0x0 in the reference — INCLUDING the negative rows (<img src="a.png">)
   …with --virtual-time-budget=5000                    correct, once
   …the IDENTICAL command again                        every image 0x0 again
```

The reference *can* be provisioned — and **does not stay provisioned**. That is neither branch. The
answer is not "fix the harness" and not "accept the divergence" but **do not publish the reading**,
and it is t1018's second fact (*no steady state → build the instrument*) arriving inside check #93's
class rather than beside it.

**The standing cost, stated so it is not re-discovered:** every battery whose subject is an image
**loaded over the network** has been compared against a reference that may not have loaded it.
`data:`-URI and CSS-only batteries are unaffected, which is every battery this window ran — but the
method needs a load-settled reference before it is pointed at images again, and **the 26.5%-of-corpus
`<picture>`/`srcset` question is therefore OPEN and blocked on an instrument, not on engine work.**

---

### FINDING 3 — ⚠⚠⚠ A new named class for VI.2, with three instances in one window: **THE CONSTANT FITTED AT THE ONE POINT EVERY FIXTURE USES.**

Distinct from every entry already in VI.2's residual-mass list, and it produced three defects in four
ticks:

```text
   t1043   a text field's border + intrinsic-width intercept
           ours 2.925·fs + 6   ·   Chrome 2.75·fs + 8   ·   EQUAL at fs = 13.333, and nowhere else
   t1044   the baseline's leading term, which CANCELLED:
           (h−L)/2 + a + (L−a−d)/2  ==  (h−a−d)/2 + a   for every L
   t1045   <select>/<textarea> intrinsic heights — a ratio whose own comment calls it
           "Chrome's own ratio at the control font"
```

**The mechanism is structural, not careless.** The UA control font is `13.333px`, so *every*
form-control fixture anybody writes lands on the single point where a wrong constant and the right one
agree. t1038 measured the border as wrong, measured the default width as already exact, and
**correctly refused the trade** — the refusal was right and the residue it left had a shape:

> **A CORRECTLY-REFUSED TRADE IS AN UNFINISHED FIX, AND ITS SHAPE NAMES THE MISSING HALF.** When a
> past tick says *"measured, not taken — it would be a trade,"* the question for the next tick is not
> whether to take it but **what second change makes it not a trade.**

And the detection rule, which cost one fixture row and paid three times: **vary the parameter you held
fixed.** `<input style="font-size:20px">` is 303 in Chrome and 305 under the old pair; measuring the
same markup more carefully would never have found it.

---

### FINDING 4 — The falsification discipline ran BEFORE publication three times and **changed the outcome twice.** This is the window's best compliance news and it is worth stating as a mechanism.

I5 and t834 require every half of a fix to be separately falsifiable. Run as a pass rather than a
formality, it did not merely confirm:

- **t1044** — four mutations, two red. One green needed a fixture row I had not thought to write
  (content-box vs border-box: the *obvious* frame row pins at `dy 0` under both models); **the other
  was a term that should not exist**, and deleting it made the code both simpler and *more* correct
  about the mechanism.
- **t1046** — a fix that was right by `select_image_url`'s own documented contract (a **third** caller
  reading `attr("src")` directly), compiled, and **moved not one row**. It was **reverted, not
  shipped.**
- **t1045** — a mutation left green was documented as a *provable equivalence* (a leading comma cannot
  reach that line) rather than left looking tested.

> **A MUTATION THAT LEAVES THE GATE GREEN IS NOT A WEAK GATE — IT IS A SENTENCE NOTHING IS TESTING,
> AND HALF THE TIME NOTHING CAN TEST IT BECAUSE IT DOES NOT MEAN ANYTHING.** The three outcomes are
> the three correct responses: **write the missing row · delete the code · state the equivalence.**

---

### FINDING 5 — ⚠⚠ The window's largest compliance gap: **M1 HAS NOT BEEN MEASURED, AND CHECK #94 SAID SO SIX TICKS EARLIER.**

VI.3's instrument of record is `FIDELITY-PROGRESS.tsv` field 15. Its last banked value is **17.6%
(31/131), sweep t1023**. Check #94 flagged it as *"six ticks stale against three landed geometry
fixes."* It is now **stale against roughly a dozen**, `tick.sh` prints *"the sweep is 559h old — a
capability tick must measure THIS tree"* on every landing, and this window added five more fixes
without one.

**What the window did instead is defensible and is not a substitute.** Every tick priced itself on a
10-site panel against an old binary rebuilt in the same hour, with 3-run bands per site — which is how
t1043's apparent `paypal` regression was shown to be that site's own band (modal `0.649813` on
*both* binaries) and t1044's `otomoto` movement was shown to be noise (bands `[0.752,0.791]` vs
`[0.762,0.799]`). That is honest attribution, and it is **not** the corpus-level number the horizon is
scored on. Check #72's distinction stands: *"unmeasured"* and *"bought nothing"* are two different
readings and only a sweep separates them.

⚠ Recorded rather than resolved because the sweep is **the agent's job and off the tick path**, and
this window's ticks each ran a 10-site cut plus, twice, a full old-binary rebuild — see the wall audit
below for what that cost. **The next non-fix tick should be the sweep.**

---

### FINDING 6 — VI.2 gains a measured NEGATIVE and two positives, all from batteries.

- ⚠ **`var()` is CLEAN — 30 of 30** (t1045). CSS custom properties are **31.6%** of the corpus and had
  only ever had CSSOM attention; a 30-row geometry battery is Chrome-exact on every row including
  guaranteed-invalid resolving to the *inherited* value, `--x: ;`, `--x: initial`, case-sensitivity,
  var-of-var and var-in-`min()`. **A cleared construct is a result and belongs in the ledger**, because
  the next tick will otherwise look there.
- **Form controls are a new member of VI.2's "opts out of ordinary block sizing" family** —
  `<button>` **55.6%** and `<input>` **51.5%** are the corpus's #1 and #2 constructs, they beat
  `<table>` eight to one, and they had **no differential reading**. Four defects in three ticks. ⚠ And
  the ordering lesson: **`<button>` was already right (48 of 55 rows) and `<input>` was not** — usage
  weight ranked where to look and had nothing to say about which of the two was broken.
- **Still open, with numbers:** `<select>`'s intrinsic height ladder · an empty `<button>`'s baseline
  (its content-box *bottom*) · `<input type=range|color|image>` as an **8x6 stub** (priced at **0 of
  170** corpus pages and declined *in advance* rather than in a post-mortem — VI.3's rule applied
  forward).

---

### COMPLIANCE

- **PART VII held.** Five ticks, zero `scripts/` files touched. Every tick served component **1**
  (daily-driver rendering parity); none served a deferred horizon.
- **THE RATCHET held.** Zero regressions traded. Two apparent single-site losses were refuted by
  3-run bands before being believed (t1043 `paypal`, t1044 `otomoto`), and one fix was **reverted**
  rather than shipped unfalsifiable (t1046).
- **I2 held** — no sanctioned dependency patched; the `srcset` fix is our own parser.
- **Wall audit #39 (due at 1044) ran** and found the wall's standing cost unchanged since #38. Its
  finding is agent behaviour, not harness bloat: **an old-binary control costs the NEXT wall, not its
  own tick** — 604s of cold gate relink, remedied for free by pre-warming out of band, which took
  t1044/t1045 from 974s back to 138s/141s. ⚠ And it recurred at t1046 because the pre-warm was run in
  the *foreground* and the harness SIGTERMs at ten minutes: **background the pre-warm and `tick.sh`
  both.**

### THE STEER (for the next window)

1. **RUN THE SWEEP.** It is the window's only real compliance gap and it is six ticks past when check
   #94 asked for it.
2. **Finish the spent-frequency sweep.** Run in this tick, it found **four** live claims and refuted
   the second one checked at **30.0% of the corpus** (block-in-inline with a styled inline). Two are
   still unchecked — a namespaced `attr()` in `content`, and a selector branch walking left past the
   anchor — and the block-in-inline one now needs its *observability* measured, not just its
   frequency (t1039's rule).
3. **Build the load-settled reference** before re-opening `<picture>`/`srcset` (26.5%). Engine work
   there is blocked on an instrument, and saying so beats grinding.
4. **Vary the parameter you held fixed** on the remaining form-control ladder before writing any
   constant into it — Finding 3, applied to its own residue.

---

## Check #97 — tick 1055

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** **Gate**, and unusually cleanly: eight ticks (1048–1055), five capability
fixes all in the CSS 2.1 inline/box model, three measurements, **zero regressions traded**, and every
fix Chrome-exact and RED-proven. Usage weights, measured not asserted: block-in-inline **30.0%** of
the corpus, `position:fixed` **22.5%** (a floor), and the inline-margin fix lands on the sweep's **#1
cross-site width cluster** (21 sites / 244 hits). This is I4's *usage-weighted breadth* being obeyed
rather than cited.

---

### ⚠⚠⚠ FINDING 1 — VI.3's ENUMERATION RULE APPLIES TO THE SPEC, NOT ONLY TO THE REFERENCE, AND THE FIRST THING IT PRODUCED WAS A NEGATIVE

VI.3 already carries the rule in its own words, promoted by check #94: **"EVERY PART OF THE PLATFORM
THE REFERENCE CAN BE ASKED TO ENUMERATE SHOULD BE ENUMERATED ONCE."** It was written about Chrome's
UA stylesheet — a surface the *reference* recites. Surface audit #45 (t1050) found the rule's blind
spot by running the outside-in method to exhaustion: **all twenty Interop 2026 focus areas, all four
investigations and the 2026 Baseline additions were already on the map — zero rows added** — while
`block-in-inline`, 30% of the corpus and six defects deep, had **no row at all** until t1048 put one
there *after* the bug was found.

> **The world publishes lists of FEATURES. Nobody publishes a list of LAYOUT PRIMITIVES — so the CSS
> 2.1 box model's interior is the one region the map could only RECORD, never RANK.** And that is a
> false constraint, because **§9 and §10 are themselves a numbered list.** The enumeration rule's
> subject is not "the reference" — it is "any authority that can recite its own surface", and the
> SPEC is one.

t1054 spent one tick, no build, on 32 primitives: **six were not named anywhere**, and **three of the
six are the plain rule under an exception the map already gates** — margin collapse-*through* gated
21/21 while "where does a block's auto height end" had no row; the abspos WIDTH half gated while the
HEIGHT half (§10.6.4) is quoted *inside an existing receipt*, so the code knew a rule the map could
not carry a verdict about. **The loop has been filing exceptions and skipping rules**, which is what
a reactive map produces.

**And the method paid for itself in one tick.** t1055 took the highest-usage of the six unknowns and
the answer was **`position: fixed` is CLEAN, 19 of 20** — including all five grouping-property
exception rows and a `transform:none` control. That is the cheapest possible outcome and it is the
one audit #45 predicted: *"absence-from-the-map and absence-from-the-engine are different facts that
have been indistinguishable."* **A cleared primitive is a result, and `unknown → measured` is the
ratchet's own banked invariant.**

**Recommendation, binding on the loop:** finish the §8/§9/§10 enumeration's five remaining unknowns
before opening a new area, and extend the same treatment to CSS Display L3 and CSS Position L3. Rank
inside a complete frame; a ranking inside the wrong frame is confident and wrong (VI.3 §2).

---

### ⚠⚠⚠ FINDING 2 — THE WINDOW'S DOMINANT FAILURE MODE WAS ONE SHAPE, IT APPEARED THREE TIMES IN EIGHT TICKS, AND A CONTROL ROW CAUGHT IT EVERY TIME

Not three unrelated near-misses — one mechanism:

```text
  t1051  a fix that provably works and changes NOT ONE ROW      -> the DISPATCH is the bug, one
         level up, and it was a GATE not a handler, so grepping
         the handler (11 call sites, all read) could not find it
  t1053  h_overflow's #1 site by COUNT (11 hits) is not a       -> a count ranks where to LOOK; it
         width bug at all — it reports `right 1000000`             never says what the work IS
  t1055  the divergence found while measuring `position:fixed`  -> `absolute` fails identically;
         is not a `fixed` defect                                   two `left:0` controls proved it
```

**In every case the discriminator was a row that was not the row that made me look**, and in t1055 the
cost of omitting it was concrete and nameable: the fix ships with a gate asserting `fixed`, and the
far commoner `absolute` half goes on failing beside a green gate naming the wrong subject — **t1007's
"a gate can PIN the engine to a bug", arriving from the other direction.** This is the seventh
instance the journal has recorded of this shape. It is no longer a lesson; it is the standing cost of
ranking by symptom, and the control arm is the only thing that has ever caught it.

---

### ⚠⚠⚠ FINDING 3 — M1 HAS BEEN THE SAME 23 SITES FOR THREE SWEEPS, AND VI.3 ALREADY SAYS THIS IS AN OWNER DECISION

t1049's clean `--jobs 2` CrUX sweep:

```text
                          t1023    t1031    t1049
   M1                     17.6%    17.6%    17.8%     23 · 23 · 23 sites
     shape >= 0.75        23.7%    26.7%    28.7%     31 -> 35 -> 37
   scored / in-scope     107/131  107/131  108/129    scorability 83.7%
```

**Twenty-six ticks. The first conjunct rose 19%. M1 did not move by one site.** The conjuncts are
*decoupling*, and of the 14 sites that clear shape and fail M1, `reading_order` binds 12.

⚠ **This is not new information to the constitution — VI.3's own H0.1 row says it in the
imperative**, and it is worth quoting back because the loop has spent this whole window on the other
side of it: *"the M1 ceiling is 82.2–87.4% against a stated bar of 95% … no amount of engine work
closes it … re-stating the bar against the SCORABLE denominator, or accepting that 95% means
something other than what it says — those are owner decisions, and the loop must not grind engine
ticks against a remaining distance that is 87% instrument."*

The window's engine work is **good work on the right primitives** — that is Finding 1 — and it is
being scored against a metric whose binding conjunct it does not touch. **Both things are true and
the loop must stop reporting the second as a surprise.** The steer is not to abandon geometry; it is
that `reading_order` is a **long tail of independent two-sibling inversions with no shared container**
(t1041), so it is reached by tightening geometry generally and never by hunting one more mechanism —
which is exactly what this window did, correctly, and should continue to do **without expecting M1 to
move**.

---

### FINDING 4 — A REFUSED TRADE WAS CONVERTED RATHER THAN TAKEN, WHICH IS THE RATCHET WORKING AS DESIGNED

t1051's float fix recovered twelve missing boxes and made twelve inline rects worse — twelve gained,
twelve lost, better on the I3 click point and worse on shape. **Ambiguous is precisely the case the
ratchet exists for, and the answer was not to adjudicate it**: t1043's corollary (*"a correctly-refused
trade is an unfinished fix, and its shape names the missing half"*) named the third change —
`LayoutBox.out_of_flow`, because an out-of-flow box is not part of an inline's advance — and the
battery went 15/27 → 27/27. **No trade was taken and none was refused; one was completed.**

---

### COMPLIANCE, recorded because two of these were under pressure

- **PART VII held. Eight ticks, zero `scripts/` files touched** — while the verify wall ran
  **980–1040s** against a 300s target and the t1051 self-audit failed on exactly that item. It was
  recorded and handed to the observer. ⚠ **And the agent-side half was named rather than shrugged
  at:** five of eight ticks ran an old-binary control, each a full release relink, and t1046 already
  established that a control costs the NEXT wall. **They were not batched.** That is the loop's own
  contribution to the wall and it is free to fix.
- **THE RATCHET held.** Zero regressions traded across five engine ticks. Every apparent mover was
  re-run solo on both binaries before being believed — including a `0.8947 → 0.0000` that has the
  exact shape of a Bar-0 regression and was settled in four minutes (both binaries read
  `0.000000 / cov 0.2727 / tree-divergence-1758` identically). ⚠ **And the rule was applied to the
  favourable numbers too**, which killed t1050's own headline (`puentedemando` h_overflow 10→6; solo
  OLD reads 6·6·6) and left the tick reporting a correct fix with an unproven corpus claim.
- **I3 honoured explicitly.** t1050's residue — an inline's rect not containing a child inline's
  vertical padding — was ranked as an **I3 click-point term rather than a shape one**, which is
  check #72's finding applied at the point of filing instead of discovered two ticks later.
- **I2 held.** No vendored dependency patched.

**THE STEER, in order:** (1) finish the §8/§9/§10 enumeration's five remaining unknowns — the frame
before the ranking. (2) Keep tightening geometry on measured-frequency primitives and **stop scoring
it against M1**, per VI.3's own instrument ceiling. (3) Batch the old-binary controls. (4) The open
lead with numbers: the `1000000` sentinel, whose probe is a build and not an argument.

**Next check due: tick 1063.**

## Check #98 — tick 1063

**Horizon:** H0 — Pareto Web Parity. **Gate:** ~83% WPT across categories · oracle-verified · daily-drivable shell · every rendered construct queryable through the semantic API.

**Gate or scoreboard?** **Gate.** Eight ticks (1056–1062), **seven Chrome-exact RED-proven capability
fixes, zero regressions traded**, and check #97's binding recommendation — *finish the §8/§9/§10
enumeration before opening a new area* — **executed in full and closed**: zero unknowns and zero
receipt-only rows in that range, six of six discharged (`§9.6.1` clean, `§10.3.5` one defect,
`§10.6.3`+`§10.6.7` clean plus one found beside them, `§10.6.4` clean plus two at the paired seam,
`§10.2` clean plus one, `§9.4.3` two; `§9.2.3 run-in` refused as dead).

---

### ⚠⚠⚠ FINDING 1 — THREE OF EIGHT TICKS FOUND THE INSTRUMENT WRONG, NOT THE ENGINE, AND VI.3 PREDICTED THIS IN WRITING

Not three unrelated surprises — one class, arriving three different ways in one window:

```text
  t1059  a negative `width` must be a PARSE ERROR      -> the LIVE Stylo path was ALREADY
         (declaration dropped), we clamped to 0            Chrome-exact. MinimalCascade was wrong.
  t1061  22 of svg/linking's 26 failures are one       -> the reftest runner never runs the
         mechanism; the fix is real and RED-proven        background-image subresource pass, so
         and the suite reads 4/26 BEFORE AND AFTER       the instrument cannot SEE the fix.
  t1062  two-value `display` unparsed by Minimal       -> the LIVE path was ALREADY Chrome-exact
         — and the first battery agreed 7 of 8            on all nine rows.
```

VI.3's H0.1 row already says the remaining M1 distance is *"87% instrument"* and that the loop *"must
not grind engine ticks against"* it. This window is that sentence arriving from a direction the row
did not name: **not the score's denominator, but the FIXTURE's cascade and the RUNNER's coverage.**
`engine/layout`'s `layout_html` builds `MinimalCascade`, so **every layout battery this loop has ever
run is styled by the cascade the product does not ship** — a fixture can report a layout defect that
does not exist, and the hunt starts in `layout_block`.

⚠ **This is not an argument for retiring `MinimalCascade`** — VI.1 already records the owner decision
that it is *"the headless / JS-less fallback… not obviously worth retiring"*, and the
`--no-default-features` build **ships** it, so both fixes are real capability. The finding is narrower
and sharper: **before attributing a battery failure to layout, ask which cascade parsed the fixture**,
and before crediting a WPT delta to a fix, confirm the runner exercises the path. Both checks are one
command (`manuk-wpt boxes --html`, and tracing what the decoder actually receives).

---

### ⚠⚠⚠ FINDING 2 — A RED-PROOF THAT COMES BACK GREEN IS A READING, AND IT PAID TWICE

t1045 established *run the RED pass before believing the fix*. This window found the other half:

```text
  t1058  mutate the block axis to read `direction`  -> gate stayed GREEN, because the row put
         (the battery's own central claim)             `direction` on the BOX and the rule reads
                                                        the CONTAINING BLOCK. The row asserted
                                                        NOTHING. Re-measuring it correctly found a
                                                        SECOND, larger defect (§10.6.4 has no
                                                        "unless negative" clause — wrong under BOTH
                                                        directions, so not an RTL bug at all).
  t1061  delete the `w <= 0` guard                  -> gate stayed GREEN: for a negative extent,
                                                        DISCARDING and CLAMPING coincide. Added the
                                                        row that separates them.
```

> **A MUTATION THAT LEAVES THE GATE GREEN IS NOT A FAILED RED-PROOF — IT IS A READING ABOUT THE ROW,
> AND THE ROW'S CAPTION IS WHERE YOU WERE ABOUT TO BANK A CONCLUSION.**

And its cousin, t1062, is the same failure one level out: **a battery agreed with Chrome on 7 of 8
rows while the feature was completely unimplemented**, because an invalid `display` leaves the
element at its UA default — the fixture was measuring the UA stylesheet, not the parser. Rebuilt so
every row is an element whose default differs from the target, it reads 8 of 9 wrong. **Three
instances in one window of the fixture, not the engine, being the thing that had to be fixed first.**

---

### ⚠⚠ FINDING 3 — "ONE RULE, N IMPLEMENTATIONS" REACHED N = 3, AND ONLY THE ENUMERATION FOUND THE THIRD

The over-constrained inline axis is direction-dependent in **§10.3.3** (in-flow blocks, gated long
ago), **§10.3.7** (abspos, found t1058) and **§9.4.3** (relative offsets, found t1060). All three were
written `left`-first. **The engine had the rule three times and got it right once**, and the third
surfaced only because the enumeration kept walking *after* the second was fixed. The standing form of
the lesson: when a duplicated rule is fixed, **grep the SPEC for its other sections, not just the
codebase for its other copies.**

Two more of the window's defects are the same shape at other scales — `layout_float` is a documented
"second copy" of `layout_block` and was missing both an intrinsic width keyword (t1056) and the
relative offset (t1060); and t1057's guard was written into a **recursion** while every caller enters
at the node the rule is about.

---

### FINDING 4 — I4 AND I5 HELD UNDER A WINDOW WITH NO CORPUS MOVEMENT TO SHOW FOR IT

**Seven Chrome-exact fixes and zero attributable corpus movement**, stated plainly in every tick
rather than dressed up. Every mover on every panel was solo-re-run on both binaries, and **the rule
was applied to the numbers the loop wanted**: t1058's `fragrantica +0.0208` and t1060's
`crazyshop.pl +0.0477 at an identical element count` — the single most persuasive shape a panel diff
can produce — both dissolved and **neither was banked**. t1059 went further and **declined to run an
A/B at all**, because Stylo owns `width` on the shipping path so the result was determined in
advance: *an A/B whose result is known before it runs is theatre, not evidence.*

⚠ **PART VII held under a direct invitation to break it.** The self-audit's one open item is the
verify wall at **994s against Part 21.2's 300s target**, and all four named remedies (mold/lld,
cargo-nextest, workspace-hack, risk-based gate scheduling) live in `scripts/`. Recorded and left to
the observer; not one harness file touched in eight ticks.

⚠ **A published claim was withdrawn before it shipped** (t1062): the doc asserted a measured
`inline table` gap against Chrome that re-running the row disproved. It had been written from
reasoning rather than from the row.

---

### ⚠⚠⚠ FINDING 5 — SURFACE AUDIT #46 FOUND THE MAP AND THE CHECKOUT DRAWING EACH OTHER

Interop 2026's 20 focus areas and 4 investigations **all already have rows — zero added, the third
audit running**, confirming the outside-in axis is exhausted. The yield came from a different
question: **the WPT checkout is a 23-directory partial clone containing exactly the areas the ratchet
already tracks** (upstream `css/` has ~93 directories, ours has 16 — and `css-inline`, `css-box`,
`css-align`, `css-tables`, the literal subject of this window, are not among them). `WPT:TOTAL
422865` reads as a total over WPT and is a total over a hand-picked subset.

And four directories **are** on disk with no ratchet row, measured for the first time: `svg`
38p/**108 FAILED**, `mathml` 84p/**66 FAILED**, and `wai-aria` + `accname`.

⚠⚠⚠ **CORRECTED AT TICK 1064.** This paragraph originally said the last two read *"0 passed, 0
failed, 442/442 SKIPPED — the reftest runner cannot score the accessibility surface at all."* That is
true of the **reftest** runner and false about the loop: `manuk-wpt wpt <dir>` is the **testharness**
runner and scores them **238/434 = 54.8%** and **306/481 = 63.6%** — together **544/915 = 59.5%**,
HANG/CRASH 0. A `SKIP` is a statement about the runner, not about the capability. What survives is
that all four directories are **unbanked**: 371 failing a11y subtests on the **I3** moat cannot
regress, because nothing marks them.

⚠ The audit also **nearly manufactured ~70 false gaps** from the directory-count delta; hand-checking
each against the map (t1054's rule) left **two** real ones. A false gap costs more than a missed one.

---

**THE STEER, in order:**
1. ⚠⚠⚠ **WITHDRAWN AND REPLACED AT TICK 1064.** This read *"the a11y conformance surface is
   unmeasurable by the current runner (442/442 skipped)… build a testharness path."* The testharness
   runner already exists and measures it at **544/915 = 59.5%**. The accurate steer: **BANK
   `wai-aria` and `accname`** so their 371 failing subtests fall under the ratchet, then work them as
   an ordinary I3 backlog. The substance — a large gap on the moat the constitution says is *"never
   allowed to rot"* — survives; the diagnosis of *why it was invisible* does not.
2. **State the WPT denominator wherever the total is published**, and bank `svg`/`mathml`'s 1359
   on-disk tests so their 174 known failures cannot silently rot — harness-owned, flagged not touched.
3. **Extend the spec-enumeration method to CSS Position L3** (Display L3 is done, t1062, one gap
   found and closed). It is the only method still producing rows.
4. **Ask which cascade parsed the fixture before blaming layout**, and confirm the runner exercises
   the path before crediting a WPT delta. Both are one command; this window paid for both three times.

**Next check due: tick 1071.**

## Check #99 — tick 1071

The cadence re-read of `CONSTITUTION.MD` (due every 8 ticks; last at 1063), over the window
**t1064–t1070**: four capability ticks (§17 column widths, the table box's own clamps, the flex/grid
baseline, the table-cell transform), two measurement ticks (surface audit #47, the §17.2.1
decomposition) and one correction.

---

### ⚠⚠⚠ FINDING 1 — THREE DEFECTS IN FOUR TICKS WERE THE **ABSENCE** OF AN IMPLEMENTATION, NOT A WRONG ONE

*One rule, N implementations* has been the loop's most productive pattern since t720, and it reached
N=3 last window (the direction clause in §10.3.3/§10.3.7/§9.4.3). This window it changed shape:

```text
   t1067  a flex/grid leaf never REPORTS a baseline   -> taffy's fallback silently made it `end`
   t1068  the cell path never APPLIES a transform     -> the 4th of four sites, and the only silent one
   t1070  a table never GENERATES an anonymous cell   -> §17.2.1's box, absent entirely
```

All three are the same rule implemented in several places with **one place implementing nothing**.
And the search that finds a wrong copy does not find these:

> **YOU CANNOT GREP FOR THE COPY THAT IS NOT THERE.** A duplicated rule is found by searching the
> codebase for its other copies (t1054's steer) or the spec for its other sections (check #98's).
> An absent one is found only by enumerating the rule's **consumers** — every path that could have
> asked and did not — or by a battery that walks in from outside and notices the box is missing.
> **This is a third search, and the loop has now paid for it three times in four ticks.**

⚠ It also explains why all three were found by a *reference diff* and none by reading code: t1067 by
the one failing row of a 40-case grid battery, t1068 by the one failing row of a 27-case transform
battery, t1070 by twelve rows of an 18-case table battery. **An absence has no symptom in the source.**

---

### ⚠⚠⚠ FINDING 2 — A MUTATION THAT STAYED GREEN WAS THE FINDING, TWICE, FOR DIFFERENT REASONS

Part 33 requires every gate be proven able to go red. This window proved something narrower and more
useful — that the *informative* mutation is the one whose outcome you did not predict:

```text
   t1066  "never use the fixed algorithm"    -> GREEN. The control asserted a row BOTH algorithms
                                                 answer identically, because t1065's fix ONE TICK
                                                 EARLIER had removed its separating power.
   t1068  zero the transform-origin + basis  -> GREEN. Every row in the gate was a pure TRANSLATION,
                                                 which is origin-independent; the gate never had any.
```

t1057 established *a green mutation is a reading about the ROW*. The addition: **a control can DECAY**
— it can be correct when written and vacuous a tick later, because the engine improved beside it. So
the standing form is not "write good controls" but **run every mutation, including the ones you
expect to fail**, on every tick, and treat a green one as a result rather than a formality.

---

### ⚠⚠⚠ FINDING 3 — THE RATCHET'S HARDEST TEST WAS TAKEN AND PASSED AT t1070, AND THE TEMPTATION IS WORTH NAMING PRECISELY

t1070 found the top-ranked cluster of the CSS2 table suite (56 of 175 failures), localised it to two
exact code sites, and **did not fix it**. The available shortcut — one anonymous cell per stray
child, reusing the flex/grid *"the text node IS the item"* trick — would have moved **all twelve**
battery rows and silently turned every 3-column row with two consecutive strays into a 4-column one.

> **THE GAIN WAS MEASURABLE AND THE REGRESSION WAS NOT, AND THAT ASYMMETRY IS THE WHOLE DANGER.**
> Part 24 forbids trading a regression for a capability; what it cannot do is make the regression as
> visible as the gain. Here the gain had a number (12 rows, 56 reftests) and the cost had none — it
> would have shown up ticks later as an unattributed corpus movement. **The refusal is only possible
> because the mechanism was understood before the fix was attempted**, which is an argument for
> measuring first that the loop has not previously stated in this form.

⚠ And it is the **t156 shape** — *"too big for one atomic tick; it needs a dedicated decomposition
session"* — handled the way t156 says to: the tick delivered the decomposition, not a half-landed
refactor.

---

### ⚠⚠ FINDING 4 — I4/I5 HELD, AND THE STANDING DEBT IS NOW FOUR TICKS DEEP AND SHOULD BE SAID PLAINLY

Four Chrome-exact capability fixes landed with **no corpus A/B**, and every one said so in its own
write-up in the same words: *the corpus movement is unmeasured, not zero*. That is I5-compliant and
it is honest. It is also a debt: pricing any of them needs a same-hour old-binary control, which is a
release relink at ~3m30s, and this session already paid **five** relinks for measurement alone.

The loop should not pretend that four unpriced fixes are the same as four priced ones. The lever the
agent owns is the one check #97 named — **fewer priced ticks, not faster builds** — and the honest
accounting is that this window chose breadth (four mechanisms) over attribution (zero), deliberately,
and that the choice is defensible only while the fixes are Chrome-exact and RED-proven, which all
four were.

---

### FINDING 5 — PART VII HELD, INCLUDING WHERE IT WOULD HAVE BEEN EASIEST TO BREAK

The tick-1065 pre-flight blocked on a wall-time audit that had already been run — because t1064
published it in the **journal** and never appended it to `docs/loop/WALL-AUDIT.md`, which is where
`status-update.sh` derives the marker from. Hand-editing the generated `STATUS.md` did not survive
regeneration. **The fix was to write to the artefact the generator reads** — agent-owned, one append
— rather than to touch `status-update.sh`. Not one `scripts/` file was modified in seven ticks.

> **A CADENCE MARKER LIVES IN THE ARTEFACT, NOT IN THE STATUS FILE OR THE JOURNAL.** The journal is
> where an audit is narrated; the artefact is where it is recorded, and only the record moves the
> cadence.

---

### ⚠⚠ FINDING 6 — VI.3'S RANKING WAS BEING COMPUTED INSIDE A FRAME THE INSTRUMENT CHOSE

VI.3 binds the loop to `usage-weight × failing-breadth`. Surface audit #47 found that five
consecutive ticks selected their subject with **one instrument** — a self-authored geometry fixture —
which is structurally blind to paint, scroll and interaction, and that `css/CSS2` (9,221 tests, 4,040
failing) had been on disk the whole time with no ratchet row. t1070 then demonstrated the correction
in one tick: the suite produced a ranked, weighted work-list in under a minute, and a hand-authored
battery was still needed to turn a rank into a mechanism.

**Neither instrument replaces the other, and the loop was using only one.** That is not a failure of
VI.3; it is VI.3 being computed over the subset one instrument can see.

---

### MAP HONESTY — DRIFT DRIVEN 9 → 7, AND THE TWO THAT CLOSED WERE ALREADY GATED

`scripts/map-reconcile.sh` reports **7** bare assertions, down from 9 (and from 26 at t601). The two
closed this check are `align-self (flex AND grid item cross-axis override)` and `justify-self (grid
item INLINE-axis)`, which claimed a capability with `gate='-'` while
`engine/page/tests/g_self_alignment.rs` had been asserting both against headless Chrome since t980 —
including the `justify-self: end` row that gate was *created* by. **The drift was in the map's
bookkeeping, not in the engine**, which is exactly the false-presence risk the check exists to catch:
a row that claims a capability with no gate reads the same whether or not the capability is real.

The remaining seven are genuinely ungated and are owed a measurement or a `status=unmeasured`:
CSS nesting on `::before`/`::after`, `<br>` line-box height, CSS `pow()`, `linear()` easing,
`@media (scripting)`, `URL.canParse()`, `HTMLIFrameElement.loading`.

**SELF-AUDIT (due every 10 ticks; last at 1061) ran clean in the same tick** — falsifiability
declared for all 29 named gates, the 49-entry process-defect ledger intact, enforcement mechanical,
journal complete for the last five ticks, `SELF-AUDIT: methodology and reality agree.`

---

**THE STEER, in order:**
1. ⚠⚠⚠ **Enumerate a rule's CONSUMERS, not just its copies.** Three of this window's defects were an
   absent implementation, and no code search finds one. When a rule is applied in more than one place,
   list every path that *could* apply it and check each — that list is finite and writable.
2. **Run every mutation, including the ones you expect to fail**, and treat a green one as a reading.
   A control can decay: t1066's was made vacuous by t1065's fix, one tick later.
3. **Take the §17.2.1 decomposition as its own session** (the four steps are written down in
   `docs/wiki/box-layout.md`), and verify it against the 56 `table-anonymous-objects` reftests rather
   than against a new hand-authored fixture.
4. **Take one PAINT tick** — audit #47's steer #4, unchanged and now overdue: the reftest runner is a
   reference-diffing instrument the loop owns and has never used for discovery, and
   `css/css-backgrounds` is the lowest-scoring directory on the board.

**Next check due: tick 1079.**

## Check #100 — tick 1079 (2026-08-09)

**HORIZON: H0. THE GATE, stated out loud, from PART VII rather than from memory:** *"reliably renders
and runs the representative real internet"* — **not** a WPT percentage; *"83% and beyond is explicitly
OUT OF SCOPE for v1."* The instrument is **M1 on the in-scope CrUX corpus** (`shape ≥ 0.75` **AND**
jarring-clean, bot-walls excluded per `DAILY-DRIVER-CERTIFICATION.md` §3), banked in
`FIDELITY-PROGRESS.tsv`.

### 1 · GATE OR SCOREBOARD? — SCOREBOARD, and the honest form of that answer is more interesting than the verdict

Every one of the last eight ticks (1072–1079) is `css/CSS2` suite work. The suite is **not the gate**.
Its last banked gate reading is:

```text
   sweep t1049 (2026-08-08 18:29)   M1 17.8%   shape-pass 28.7%   shape_mean 60.3   cov_mean 85.7
```

**Thirty ticks have landed since anything measured the gate.** The lever board has said so twice in
this session's own output (*"a capability tick must measure THIS tree"*), and the loop kept going.
That is the drift, and it is named without excuse: a suite delta is cheap, same-hour, and
deterministic; a sweep is expensive and slow; and the loop has been selecting for the measurement it
can afford rather than the one that governs it. **That is exactly the failure tick 84 is the memorial
for**, wearing a different suit — `css/CSS2` is a far better proxy than `encoding` ever was, but a
better proxy is still a proxy.

### 2 · …AND YET THE ARC IS NOT OFF-MANDATE, which is the part a simple verdict would get wrong

VI.3 binds the loop to **usage-weight × failing-breadth**, and by that ranking the arc is defensible
tick by tick even though its *selector* is a suite count:

```text
   t1079   border-*-color/style per side   every card accent bar, tab underline, table rule   REAL
   t1078   ::first-letter                  drop caps — long-form editorial only               THIN
   t1075   XHTML CDATA stylesheets         a fifth of the SUITE; a sliver of the real web     THIN
   t1072-4 table paint layers §17.5.1      every striped/banded data table                    REAL
```

So the arc is producing genuine daily-driver capability *and* selecting it by a metric the
constitution demotes. The correction is not "stop the arc" — it is **make the gate the acceptance
test for the arc**, which is one sweep.

### 3 · INVARIANTS

- **I2 (never patch deps):** held. t1078 reached `PseudoElement::FirstLetter` by *asking Stylo's
  existing parse a new question* — the cheapest possible form of the borrowed-engine rule, and worth
  recording as the shape to look for first: the fork surface is still empty.
- **I3 (semantic model in lockstep):** ⚠ **BENT, mildly, twice.** t1078 and t1079 are both paint/box
  changes with no a11y-tree consequence — a first-letter run and a border colour do not change
  `node_rects`. Stated rather than assumed: I re-checked that both flow through the same
  `LayoutBox::node_rects` producer check #72 identified, and neither adds a rect. But t1079 *did*
  change `getComputedStyle` (four border colour rows and four style rows), which IS semantic surface,
  and it landed in the same tick — so I3 is satisfied, by accident of scope rather than by design.
- **I4 / VI.3 (Pareto):** see §1. The lesson is binding; the loop's selector drifted from it.
- **I5 (never trade a regression):** held, and tested hard this window. t1079 lost three CSS 2.1
  tests and did **not** paper over them: they were proven to have been passing *vacuously* (a red
  border painted black by `currentColor`) by rendering the test with `red`→`black` and getting a
  byte-identical match to the reference. **A test that stops passing because the engine got more
  correct is a measurement improving.**

### 4 · PART VI CORRECTIONS

- **VI.3's banked M1 line is stale**: it still cites *"sweep t1023, 17.6%"*. Ground truth is
  **sweep t1049, M1 17.8%, shape-pass 28.7%, shape_mean 60.3**. Corrected here; the next sweep
  supersedes it.
- **VI.2's "aperture" clause needs one addition.** It says the aperture is the biggest lever and
  names `css/*` and `html/*` as unmeasured. That is now largely wrong for the *upstream* directories
  and exactly right for one: `css/CSS2` is 9,221 tests on disk and, at **3,006 passed / 2,640 failed /
  3,575 skipped**, is the loop's single largest measured-and-failing surface. Surface audit #48
  (this tick) adds the frame that explains why it keeps paying: **Interop 2026 and Baseline have now
  returned ZERO new map rows twice running, while the two ticks either side of the audit each found
  a CSS 2.1 feature absent at every layer.** A list of what is NEW cannot rank what is OLD AND
  MISSING.

### 5 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **RUN THE FIDELITY SWEEP.** Thirty ticks of unmeasured gate is the largest single piece of
   drift on this board, and it is the agent's job (the observer never runs it — contention
   false-REDs the perf gates). Everything below is ranked *by* its result and is provisional until
   it exists.
2. **Rank CSS 2.1 chapters by PASS RATE, not failure count** (surface audit #48): `visufx` 2.1% and
   `linebox` 7.4% never appeared in three ticks of rankings because they are small, and a chapter at
   2% is a missing primitive while a chapter at 45% is a tail.
3. **`clip: rect()`** is probe-confirmed absent and is `visufx`'s whole work-list — one CSS 2.1
   §11.1.2 primitive.
4. **Price each CSS 2.1 candidate against the CORPUS before taking it** (`CORPUS-CONSTRUCTS.md`,
   three minutes, no build), so the suite ranks *where to look* and the corpus decides *whether to
   go*. That single step is what would have caught `::first-letter` as the thin one it was.

## Check #109 — tick 1152 (2026-08-11)

**HORIZON: H0.** **GATE:** M1 v1 RENDER — `shape >= 0.75` on `>= 95%` of the in-scope corpus, then M2
FUNCTION, then M3 v2 RE-CERT (`phase0-milestones.sh`, owner-locked). Steering gauge:
`progress-metric.sh`, work whichever of scorability / pass-rate is lower.

### 1 · GATE OR SCOREBOARD — one of eight, and the one is real

```text
   t1144  constitution check          scoreboard (governance)
   t1145  the sweep                   scoreboard (measurement)
   t1146  CI RED five ticks + layout  GATE (a gate that asserted a FONT, repaired)
   t1147  the named next tick REFUSED scoreboard — and the refusal was correct
   t1148  MISSING_BOX probe           scoreboard
   t1149  nested flex fit-content     **GATE** — hnhbkis.edu.in h-overflow 2 -> 0, shape .932 -> .957
   t1150  RO trace + surface audit    scoreboard (instrument) — but it LOCALISED two more gate rows
   t1151  the FACE finding            scoreboard — and it retired a whole hypothesis class
```

**One of eight moved an exit-gate condition, and five of the other seven were instrument or
refusal.** That ratio would be alarming under check #95's reading; it is not, and the reason is worth
stating because it will recur: **three of those five ended by NAMING a gate row with its address**
(t1148's truncation-point rule, t1150's two localisations, t1151's face discriminator), and t1149 —
the one gate tick — was landed *from* t1112's trace, an instrument tick eight ticks earlier. The
loop's own evidence is that an instrument tick pays for the gate tick that follows it. What would be
alarming is an instrument tick that names nothing, and none of these did.

### 2 · I2 — THE FIRST TIME THE LOOP HAS WRAPPED A DEPENDENCY'S *ANSWER*, AND IT IS NOT A PATCH

t1149's `TaffyDom::fit_content_inline` clamps taffy's `ComputeSize` result for a flex/grid container
to the fit-content formula. I2 says the sanctioned FFI set is *"adopted, tracked upstream, and never
forked or patched"* — and this is neither: the taffy source is untouched, the crates.io dependency is
unpinned, and the correction lives in our own `impl` of taffy's trait. It is exactly option 3 of
`STATUS.md`'s escalation table (*a hand-rolled supplement for the specific gap*), and it satisfies
that table's four conditions: named and minimal (one axis, one run-mode), justified by the spec
rather than by taste (CSS Flexbox §9.4 / Sizing §5.2, with taffy's own `compute/flexbox.rs:955-981`
cited), guarded by a gate that fails if a bump reverts it, and recorded. **The fork surface is still
empty and must stay that way** — ⚠ the distinction to hold is that wrapping an ANSWER is sanctioned
and editing an ALGORITHM is not, and the second is only one refactor away from the first.

### 3 · I3 — SATISFIED BY ACCIDENT OF SCOPE FOR THE FOURTH CONSECUTIVE CHECK

Checks #72, #100 and #104 each recorded that I3 held because `LayoutBox::node_rects` is a shared
producer, not because anyone checked. t1149 moves element geometry and flows through the same
producer, so the agent's click points moved with the boxes for free — a fourth time. **The debt
#104 named is still unwritten** (`node_rects`'s `lift` giving an icon-wrapping `<span>` the icon's
4px box). Four checks is no longer an observation; the next tick that touches the producer itself
is where this stops being free, and that tick is now overdue rather than pending.

### 4 · I4 / VI.3 — THE NORTH STAR GOT AN INDEPENDENT EXTERNAL CONFIRMATION

VI.3 demoted `WPT:TOTAL` from a north star to a bookkeeping mark on this loop's own evidence (tick
84's encoding spike). Surface audit #55 found the same conclusion reached from outside:
**Ladybird's monthly WPT gain across 2026 ran +63,726 -> +8,283 -> +3,366 -> +108**, and April's
figure is mostly the *import* of test262 upstream (~52k of the 63.7k). An independent engine of
comparable maturity has a subtest curve that goes essentially flat inside four months. ⚠ Two things
follow, and neither is "we were right": first, **a flat subtest curve is what this stage LOOKS like
in a healthy engine**, so the loop must not read its own flat `WPT:TOTAL` as a stall; second, the
corollary — an engine at this stage that is still *posting large WPT gains* is probably importing a
suite, not shipping capability. Recorded in VI.3's terms because it is the kind of number that gets
argued about from memory.

**And VI.3's fifth inflation mode has a sixth sibling, from t1151: a hypothesis priced by ITS
FREQUENCY ON THE WEB rather than by whether it is BROKEN HERE.** The "bulletproof" `@font-face`
idiom (a bare `.eot` `src`, then a second `src` with the real `format()` list) is on a large share of
the legacy web and was an excellent-sounding cause for three wider-than-Chrome sites. It is
Chrome-exact in this engine, as is parent-relative `url(../…)` resolution against the sheet's base,
as is the fallback path. **Three green mutations in one tick.** The grep's arithmetic was never the
problem — the missing step is the same one #108 added for the ORGAN: *run the RED-proof before
citing it* (#107's third clause), and for a hypothesis about a defect that means **reproduce the
defect in a fixture before pricing the fix**.

### 5 · PART VI CORRECTION

VI.4's direct path is unchanged in ORDER and needs one correction in CONTENT. It reads the render
gap as geometry. **This window produced the first well-evidenced case that a visible slice of it is
not geometry at all**: `www.jatekshop.eu`'s construct is byte-exact in our engine at the site's own
dimensions on all four boxes, and the live divergence is one wrapped line; `www.kuechenmomente.de`
is 14% wider *and* 44% taller in the same box; `www.lyreco.com`'s `<h3>` is 12px wider *and* two
lines taller. **A box that is wider AND taller cannot be a placement error** — extra width fits more
per line — so this cohort is a FACE or a used SIZE, and `Seen.font` reports the **computed family**
from both sides, which is why the column built to detect exactly this prints `{Raleway/18}` against
`{Raleway/18}`. VI.4 should carry a fourth leg beside scorability / shape / jarring: **attribution —
the diff must be able to say which face rasterized a box** — because until it can, an unknown share
of "shape" is mis-filed.

### 6 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **REPORT THE USED FACE ON BOTH SIDES, AND RE-READ THE THREE SITES.** This is the cheapest
   fork in the road on the board: same faces ⇒ a shaping-metric arc; different faces ⇒ a whole cohort
   currently ranked as N layout bugs is ONE provisioning or font-loading bug. Named with its test in
   t1151 and it is the next tick.
2. **Then take `www.lyreco.com` or `www.jatekshop.eu`** — both are one inversion from an M1 crossing
   and both are now localised to a single box by `MANUK_RO_TRACE`. Do not re-derive the address.
3. **The `node_rects` `lift` debt (I3, four checks old) is overdue**, and it is an I3 defect wearing
   a shape number's clothes — check #72's original finding, still unbuilt.
4. Standing, from #107/#108, now with a third clause of its own: **price it · ask which ORGAN · run
   the RED-proof before citing it · and reproduce the DEFECT in a fixture before pricing the fix.**

`LAST_CONSTITUTION_CHECK` -> 1152.

## Check #108 — tick 1144 (2026-08-11)

Re-read of `CONSTITUTION.MD` PART I (I1-I8), PART VI.2/VI.3 and PART VII, anchored to the seven ticks
since check #107 (t1137-t1143). **Three findings. The first is a MEASUREMENT taken inside this check —
it re-prices the window's largest stated corpus number and the honest figure is 8.5× smaller.**

### FINDING 1 — ⚠⚠⚠ THE WINDOW RANKED ON THE CORPUS GREP, AND THE GREP'S PRICE WAS QUOTED AGAINST A METRIC THAT CANNOT SEE 15 OF ITS 17 ROWS

Two of the window's five capability ticks were decided by an explicit corpus price, and t1143's is the
largest number the window published:

```text
   t1142   loading="lazy"                       49 of 385 pages   12.7%   TAKEN
           the `ex` unit vs mono                 5 of 373                 deferred
           @media (scripting)                    1 of 373                 REFUSED
   t1143   data: payload in an @font-face src   17 of 166 pages   10.3%   TAKEN
```

The `17 / 166` reproduces exactly off the cached corpus (`/tmp/corpsnap` 385 + `/tmp/corpcss` 373, the
same snapshot `CORPUS-CONSTRUCTS.md` prescribes), so the arithmetic is right. **What was never asked is
WHICH FACES those seventeen are.** One command, no build:

```text
   swiper-icons ....... 10 files      Swiper's carousel-arrow icon font
   VideoJS ............  1            player-control icons
   agGridAlpine .......  1            ag-Grid's icon set
   JudgemeStar ........  1            review-star icons
   social-likes .......  1            share-button icons
   ──────────────────── 14 of 17 are ICON fonts, consumed through `content:` on a PSEUDO-ELEMENT
   Inter ..............  1            a real text face
   Jura ...............  1            a real text face
   ──────────────────── 2 of 17 can move a line box
```

⚠⚠⚠ **THE LOOP HAS BANKED THE DEFLATING RULE TWICE AND DID NOT RUN IT.** `CORPUS-CONSTRUCTS.md` says
it in its own read-me section — *"`clip: rect()` is 36% of pages and M1 is structurally blind to it …
a high usage-weight number does not imply a scoreable one"* — and t1094 banked the mechanism that makes
it true here: **the structural probe enumerates DOM nodes, and a pseudo-element has no node.** A
dropped icon face changes which glyph is painted inside a fixed-size button; it does not move an
element box, and the instrument that would be asked to price it scores element boxes. So the tick's own
stated weight of **10.3%** is, against M1, **1.2% (2 of 166)**.

⚠⚠ **AND TEN OF THE SEVENTEEN ARE ONE VENDOR BUNDLE.** `swiper-icons` is the same Swiper stylesheet
shipped by ten different origins. Seventeen files is not seventeen independent observations — the
t1089-1100 finding (*counting rows that share a string breeds false levers*) at a new place, and it
compounds with the first half: the ten are all icon rows.

⚠ **This does NOT retire t1143.** A declaration splitter that cuts every `data:` URI in half is a
parser defect on its own terms, its RED-proof is two-sided, and the wider `;`-bearing-`data:`-in-`url()`
figure (89 of 761 files) is the real correctness story. What is wrong is the **PRICE**, and the price is
what VI.3 ranks on. This is a **fifth** distinct way the corpus grep has been caught inflating —
after the unanchored property grep, the co-occurrence, the legacy no-op VALUE, and frequency-is-not-
leverage — and, like all four before it, it inflates. The grep has now been caught over-stating five
times and under-stating **once** (the t1092 join key), which is a bias, not a set of accidents.

**The rule this yields, and it is one clause longer than "price it against the corpus":** *price it
against the corpus, then ask WHICH ORGAN the fix moves and whether the metric you quoted the price to
can see that organ.* Paint is not geometry; a pseudo-element is not a node.

### FINDING 2 — ⚠⚠⚠ FOUR OF FIVE CAPABILITY TICKS HAD NO EXTERNAL INSTRUMENT, AND THE ONE QUEUED ITEM THAT DOES WAS NAMED "THE NEXT TICK" SEVEN TICKS AGO

The window's whole external evidence, from the journal's own RATCHET lines:

```text
   t1137  <br> is a break              css/CSS2  3963 -> 3973   +10 / -0
   t1138  line-height:normal rounding  css/CSS2  3973 -> 3973     0 / 0    "uses Ahem or runs at 16px"
   t1140  word-break: keep-all         css/CSS2  3973 -> 3973     0 / 0    "no keep-all reftest at all"
   t1142  `loading` reflection         (no suite — JS)
   t1143  the declaration splitter     css/CSS2  3973 -> 3973     0 / 0    "no data:-URI @font-face test"
```

Each zero is correctly read by its own tick as *"the suite does not exercise the parameter"* — that
reading is right and is now three-for-three. **The property nobody has stated is what the run of them
means: after t1137, the only evidence any of these fixes has is a battery the tick itself authored.**

⚠ The batteries are **Chrome-referenced**, so they are not self-confirming on VALUES — t1138's control
row killing its own hypothesis in three lines is the proof that this half works. They are self-selected
on **POPULATION**, and population is precisely what the corpus sweep supplies. So the window measured
*breadth* four times (26/27, 44/44, 6 of 8 rows already correct, 9/11) and measured *weight* zero times
— while VI.3 binds the loop to `usage-weight × failing-breadth`. **Finding 1 is what that costs.**

⚠⚠ **AND THE QUEUED ITEM WITH EXTERNAL EVIDENCE WAS SKIPPED FOR SEVEN TICKS.** Check #107's steer §3
named the CSS 2.1 §17.2.1 anonymous **CELL** rule *"the named next tick"* and listed its RED-proof —
six existing reftests (`table-anonymous-objects-197..200`, `normal-flow/table-in-inline-001`,
`visuren/table-pseudo-in-part3-1`). It is the one candidate on the board whose acceptance test is
**not written by the tick that takes it**, and t1134's sibling rule paid `+15 / −0` in that exact
family. Steer §4 (the `<br>` two-line run) was taken and paid; §1 and §2 are rules about reading a
sweep and **no sweep has run since t1135**, eight ticks ago, against a cadence rule of five to six.

### FINDING 3 — ⚠⚠ I3 IS SATISFIED FOR THE FIFTH CONSECUTIVE CHECK BY ACCIDENT OF SCOPE, AND THE NAMED DEBT IS NOW FIVE CHECKS OLD

Checks #72, #100, #101 and #107 each wrote this sentence. This window earns it again and by a wider
margin: **t1138 changes the height of every `line-height: normal` line box in the engine**, which flows
through `LayoutBox::node_rects` → `manuk_a11y::build_tree_with_rects` → `A11yNode.bbox` → the agent's
click point, so every click point in the engine moved by up to a pixel and **nothing asserted it**. It
is right — the shared producer is why — but it is a property of the architecture, not of a check that
was run.

⚠ The named debt is unchanged and still unwritten: `node_rects`'s `lift` gives an icon-wrapping
`<span>` the icon's 4px box instead of its own 17px line box (Chrome `[11,0,8,17]` vs ours
`[11,10,8,4]`), so the agent's click point is computed 3.5px low. First named at t851/check #72,
**293 ticks ago**. ⚠⚠ And Finding 1 has just made it larger than it looked: the corpus's icon-font
population is not small, and an icon-wrapping `<span>` is exactly the element those fourteen
`swiper-icons`/`VideoJS`/`agGridAlpine` stylesheets build. **Ranked on M1 it is a rounding-scale shape
term; ranked on I3 — which PART VII item 2 calls the differentiator that earns the most polish — it is
a mis-actuation surface on the commonest button idiom on the corpus.** Two orderings, and VI.3 says
usage-weight wins where they disagree.

### 4 · INVARIANTS

- **I2 (never patch deps):** ⚠ held, and **tested explicitly for the first time in several windows**.
  t1142 met a real Stylo wall — the `scripting` media feature exists only under `gecko/media_features.rs`
  and the servo build does not implement it — and **refused both the patch and the workaround**, the
  latter on I4 grounds (1 of 373 stylesheets). The refusal is recorded with both halves so the wall is
  not re-derived. That is I2 and I4 agreeing, and it is the cleanest invariant application this window.
- **I3:** see Finding 3. Fifth consecutive by accident.
- **I4 / VI.3 (Pareto):** ⚠⚠ **the discipline was applied and the instrument it was applied with was
  wrong by 8.5× — see Finding 1.** The direction is right: t1142 refused a 1/373 construct outright and
  deferred a 5/373 one, which is I4 working. The defect is in the term, not the rule.
- **I5 (the differential oracle is the discovery engine):** ⚠⚠ **the oracle has not been run in eight
  ticks.** Both instruments the loop owns — the CSS 2.1 suite and the corpus sweep — were silent or
  unrun for four of the window's five capability ticks. I5 does not say *own* a discovery engine; it
  says it *is* the discovery engine, maintained as first-class infrastructure.
  ⚠ The **same-hour HEAD-binary pass-SET control** ran on all four suite readings, which is check
  #103's rule honoured without exception. Zero losses banked across the window (`+10 / −0`, then three
  `0 / 0`). No regression was traded for a capability.
- **PART VII:** held. All five capability ticks are component 1 (daily-driver rendering parity) or the
  reflection surface that feeds it; nothing touched a deferred species. t1141 met a Tier-0 harness
  failure (`verify wall: 1221s`), **measured it into a bimodal table the observer can act on, and did
  not edit `scripts/`** — the rule working as written.
- **I1, I6, I7, I8:** untouched this window.

### 5 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **RUN THE SWEEP. It is the next tick.** Eight ticks unmeasured against a five-to-six cadence;
   five landed engine changes with no corpus price; and check #107's two top steer items are rules
   about reading a sweep that has not been run since they were written. Finding 1 says the loop is
   currently ranking on the one instrument it has caught lying five times, with the one that would
   correct it switched off.
2. ⚠⚠⚠ **WHEN THE SUITE READS ZERO TWICE RUNNING, TAKE THE FIX THE SUITE CAN SEE.** The §17.2.1
   anonymous CELL rule is still the named next engine tick and still has its six-reftest RED-proof.
   Prefer a candidate whose acceptance test the tick does not author.
3. ⚠⚠ **PRICE, THEN ASK WHICH ORGAN.** Extend the `CORPUS-CONSTRUCTS.md` step by one question: *does
   the metric I quoted this price to score the thing this fix changes?* Paint is not geometry, and a
   pseudo-element has no node. On this window that one question moves 10.3% to 1.2%.
4. ⚠⚠ **THE `node_rects` LIFT IS FIVE CHECKS OLD AND FINDING 1 JUST RE-RANKED IT UPWARD.** Land it as
   an **I3** tick with an agent-side click-point assertion in the same tick — not as a shape term. It is
   the icon-button idiom, and PART VII item 2 says that surface earns the most polish.

## Check #107 — tick 1136 (2026-08-11)

Re-read of `CONSTITUTION.MD` PART I (I1-I8), PART VI.2/VI.3 and PART VII, anchored to the eight ticks
since check #106 (t1129-t1135). **Three findings, and the first is an amendment to VI.2's own newest
row — the one check #106 wrote.**

### FINDING 1 — ⚠⚠⚠ CHECK #106's RULE IS RIGHT AND ITS SCOPE IS TOO NARROW: THE SWEEP DOES NOT ONLY MISREAD SINGLE SITES, IT READS ~4 POINTS LOW ON ALL OF THEM

Check #106 added to VI.2: *"a `--jobs 2` sweep row is bankable for the DENOMINATOR and is not evidence
about any single site,"* from five per-site readings that failed to reproduce. That framing makes the
failure **sporadic** — some rows are unlucky. t1135 measured the population and it is not sporadic, it
is a **bias**. Seven down-movers with BYTE-IDENTICAL node counts, re-run solo on the same binary and
against the old binary, same hour:

```text
                          t1127swp  t1135swp   SOLO new   SOLO old
   gismart.com              0.843     0.797      0.840      0.840
   bhfudbal.ba              0.596     0.551      0.595      0.595
   www.crazyshop.pl         0.658     0.618      0.655      0.658
   www.puentedemando.com    0.822     0.792      0.759      0.757
```

The two binaries agree to three decimals on every row, and the solo column recovers the previous
sweep's value — **in the same direction on every site at once.** So the amendment VI.2 needs is one
clause wider than the one it has: *a `--jobs 2` number and a solo number are on **different scales**,
and the loop has been diffing across them.* Every "the fix didn't reproduce corpus-wide" reading in
this project's history was taken across that boundary. **This is I5 territory** — the differential
oracle is the discovery engine, and a discovery engine with a systematic offset between its two
operating modes mis-ranks the work-list, which is the fourth distinct way this instrument has been
caught lying (mis-provisioned reference · population-changed delta · per-site churn · now scale).

### FINDING 2 — ⚠⚠⚠ THE M1 GATE WEIGHTS SITES AND THE INSTRUMENT WEIGHTS NODES, AND NOTHING RECONCILES THEM

`shape` is a mean over matched elements; the gate counts SITES. **14 of the 121 rows scored in both
sweeps are computed over ≤10 nodes, and 12 of them are frozen** — identical shape, identical `n`,
delta exactly `0.000`, sweep after sweep. `allticketscol.com` scores **1.000 on ONE node** and counts
as a full shape-PASS in the M1 numerator; `house.udn.com` scores 0.000 on one node and counts as a
fail. `merchant.upi9.pro` fell 47 nodes → 2 and reported exactly **0.500 — which is 1/2, from a sample
of two** — worth −0.415 of the whole common-set band.

This is the SAME class as check #103's population-changed delta and it is a different member of it:
#103 is about a population that MOVED between two readings; this is about a population too small to
carry a reading at all. VI.3's rule *"a frequency is not a measurement until its VALUES have been
looked at"* has a twin here: **a mean is not a measurement until its N has been looked at.** The
instrument already computes `shape_n` and writes it in the row; nothing reads it.

⚠ Consequence for the stated bar, which is a re-statement of check #83's finding at a new place: the
M1 numerator and denominator both contain rows that cannot move. That is not the 82.2-87.4% ceiling
#83 named (that one is scorability); it is a second, smaller distortion inside the sites that DO
score. Both are owner questions and the loop must not silently absorb either.

### FINDING 3 — ⚠⚠ I5 HELD UNDER PRESSURE THREE TIMES THIS WINDOW, AND ONCE IT COST A LANDING

I5 (*a regression is never traded for a capability*) was tested three times in eight ticks and held
each time, but the interesting one is t1134. A complete §17.2.1 anonymous-table implementation came in
at **css/CSS2 +15 / −6** on a pass-SET diff against a same-hour old binary. The +9 headline was
green and the six losses were real: a `table-row` holding non-cell content lost that content, because
§17.2.1's anonymous **CELL** rule is not built. The tick refused the wrap for exactly those rows and
landed **+15 / −0**.

⚠ **And the scope was written the way t1125/t1126 taught** — around an unbuilt RULE with the six
reftests named as its RED-proof, not around the failing tests. That is the correction from check #105's
*"a scope drawn around a failing test is a note to come back"* being applied BEFORE the note was
needed rather than two ticks later, which is the first time this loop has done that.

⚠ The other two: t1133 refused a narrow fix outright (it closed seven rows and stacked two), and
t1134's own `box-sizing` repair was forced by the ratchet — routing orphan cells through `layout_cell`
exposed a defect a real `<td>` had carried the whole time, and the fix had to land in the same tick
because the battery row regressed.

### 4 · INVARIANTS

- **I2 (never patch deps):** held. Nothing this window touched Stylo, Taffy or mozjs; t1134's work is
  entirely in `engine/layout` and one additive `pub fn` in `engine/css`.
- **I3 (semantic model in lockstep):** ⚠⚠ **satisfied, and for the FOURTH consecutive check by
  ACCIDENT OF SCOPE** — the identical sentence checks #72, #100 and #101 wrote. t1134 changes the BOX
  TREE, which is closer to the producer than any of the previous windows: the anonymous table box
  carries `node: None`, so it reports no rect and the cells keep theirs, and the click points move
  with the boxes for free. **That is a design choice that happened to be right, not a check that was
  run.** The named debt is unchanged and still unwritten: `node_rects`'s `lift` giving an
  icon-wrapping `<span>` the icon's 4px box. Four checks is long enough; the next tick that touches
  `node_rects` must land with an agent-side click-point assertion.
- **I4 / VI.3 (Pareto):** held and, for once, measured before the build — `display:table-cell` was
  priced at 54 of 373 corpus stylesheets (14.5%) BEFORE t1134 was taken, which is exactly the
  "price it against the corpus first" step check #105 §4 asked for.
- **I5:** see FINDING 3. **I1, I6, I7, I8:** untouched this window.

### 5 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **READ `shape_n` BEFORE BELIEVING ANY PER-SITE DELTA**, and treat any row with `shape_n ≤ 10`
   as unmeasured rather than as a score. This is one column that already exists in every sweep row
   file, and it invalidates 14 of 121 rows' worth of band movement.
2. ⚠⚠⚠ **NEVER DIFF A SWEEP NUMBER AGAINST A SOLO NUMBER.** They are ~4 points apart by construction.
   A per-site A/B is solo-vs-solo or it is nothing.
3. **The §17.2.1 anonymous CELL rule is the named next tick** and its RED-proof already exists:
   `css/CSS2/tables/table-anonymous-objects-197..200` plus `normal-flow/table-in-inline-001` and
   `visuren/table-pseudo-in-part3-1`. The blocker is structural, not algorithmic — `layout_cell` reads
   its style and children from a `NodeId`, so it needs the same `Option<NodeId>` split `layout_table`
   just took.
4. **The `<br>`-broken two-line run reads 37 where Chrome says 36**, in a real `<table>` as well. It
   is one pixel on every wrapped two-line run in the engine, which is the highest-frequency
   lowest-magnitude defect currently named, and VI.3 says usage-weight wins where the two orderings
   disagree. Hypothesis from source, unmeasured: `close_line` rounds `ascent` and `descent`
   INDEPENDENTLY and sums the rounded values.

## Check #106 — tick 1128 (2026-08-11)

HORIZON H0, re-scoped by **PART VII**: the four v1 components. Its gate is *"reliably renders and
runs the representative real internet"*, and the measurable stand-in is **M1** (`shape ≥ 0.75` AND
jarring-clean) on the in-scope CrUX corpus.

### 1 · GATE OR SCOREBOARD? — gate, and the gate's own COUNT says otherwise

Ticks 1121–1127. Four Chrome-exact, RED-proven engine fixes; `css-flexbox` **304 → 309** and
`css-grid` **208 → 211** with pass-SETS diffed and **zero regressions**; and on the corpus, M1's count
is **flat at 24**. The membership is not:

```text
   GAINED M1   www.wdimax.com          reading_order 12 → 0    t1124, predicted
               www.kuechenmomente.de   reading_order  5 → 0    t1124, unpredicted
   LOST M1     app.ordertime.com       stopped RENDERING (cov 1.000 → 0.040)
               gismart.com             REFUTED — the OLD binary reproduces it (2×2, same hour)
```

**+2 attributable, both losses not ours.** The correct report of this window is a membership list,
and the percentage is the misleading form of the same data.

### 2 · ⚠⚠⚠ TWICE IN THREE TICKS THE BOUNDARY WAS THE DEFECT, NOT THE RULE

t1119 shipped a correct rule with two exclusions, each drawn by measuring which reftests failed and
putting the line just inside them:

```text
   exclusion              why it existed              what it actually was
   inset-less children    including them lost ONE     ANOTHER TICK'S BUG — t1124 fixed the static
                          reftest                     position recorded in provisional space, and
                                                      t1125 deleted the boundary for +3/−0
   "the container is a    including grid traded 3     A WORD TOO WIDE — Grid §9 splits on "has a
    grid"                 reftests                    DEFINITE grid position", not on the formatting
                                                      context; t1126 narrowed it for +3/−0
```

**A scope narrowed around a FAILING TEST is not a scope — it is a note that something under it is
broken, written where nobody re-reads it.** It cost two ticks of coverage and it hid a defect
(t1124's) that was worth two M1 crossings on its own. The rule to carry: **a scope should be a
sentence from the SPEC with a measurement attached, not a measurement with a sentence attached** — and
the cheap way to find out which one you wrote is to re-read the spec paragraph *after* the tests go
green. Both exclusions passed that re-read only once someone did it.

### 3 · I5 — THE SWEEP IS NOT EVIDENCE ABOUT A SITE, AND THAT IS NOW MEASURED FIVE TIMES

I5 makes the differential oracle the discovery engine. Across the t1121 and t1127 sweeps, **five
distinct per-site readings did not reproduce on either binary in the same hour**:

```text
   t1121   serennu.com        0.574 → 0.393     solo: 0.574 on BOTH binaries
           possssno.sbs       0.991 → 0.911     solo: 0.991 on BOTH binaries
   t1127   probidas.lt        scored → CRASHED  solo: renders, 29.9%, on BOTH binaries
           gismart.com        h_ovf 0 → 6       OLD binary: 5 on one of its own two runs, 0 on the other
           app.ordertime.com  cov 1.000 → 0.040 (an origin/tree divergence, not geometry)
```

**The sweep's per-site noise floor is larger than most per-site deltas it reports.** That does not
demote the sweep — its DENOMINATOR is sound and the membership diff above is exactly what it is for —
but it retires a habit: *a sweep row is a question, never a finding.* The two mechanisms that ARE
evidence are the membership diff (which sites crossed) and a same-hour old-binary control (whether a
named site's change is ours). Both are cheap; neither was standard practice before this window.

⚠ **And `crashed` gets checked FIRST.** Bar 0 outranks every visual divergence (Part 24.3), so
`probidas.lt` was A/B'd before anything else in the tick was written. It cost four minutes and it is
the correct order every time.

### 4 · INVARIANTS

- **I4 / VI.3 (usage-weighted breadth):** ⚠⚠ **the high-usage/low-magnitude case, recorded for the
  THIRD time** (check #72, check #105 §1, here). Four correct fixes to constructs that are on most of
  the corpus moved the pass count by zero, because the gate prices a fix by whether it crosses a
  per-site threshold. The corpus grep this window put `display:flex` × `position:absolute` at
  **118 of 170 pages (69%)** — and the leverage was two sites. Frequency still is not leverage.
- **I3 (semantic model in lockstep):** satisfied, and this window it was CAUSAL twice. t1119's
  double box made `node_rects` — and therefore the AX bbox and the agent's click point — the union of
  two boxes; t1124's provisional static position put every out-of-flow affordance inside a drawer or
  fixed toolbar at the page origin, which is an actuation surface before it is a visual one.
- **I2 (never patch deps):** held. taffy is untouched; every fix this window changes what we hand it
  or which of its answers we consume. t1123 refused to work around it by re-deriving a flex line's
  main-axis distribution and said so.
- **THE RATCHET:** tested three times and held three times — t1119 refused three net-positive scopes,
  t1123 reverted a fix that moved no measured row, t1127 attributed both candidate regressions before
  claiming the window.
- **I1, I6, I7, I8:** untouched.

### 5 · PART VI CORRECTION

Two clauses for H0.1, both about how the loop READS its instrument rather than what it builds:

> **A `--jobs 2` sweep row is bankable for the DENOMINATOR and is not evidence about any single
> site** (added check #106): five per-site readings across two sweeps failed to reproduce on either
> binary in the same hour, including one `crashed`. Rank on the M1 MEMBERSHIP diff; attribute with a
> same-hour old-binary control; treat a per-site delta as a question.
>
> **A scope drawn around a failing test is a note to come back** (added check #106): both of t1119's
> exclusions were boundaries around other defects, and deleting them cost nothing and bought six
> reftests once those defects were named.

### 6 · THE STEER — binding on the next tick

1. **The out-of-flow reading-order class is CLOSED and the tail changed shape.** Every remaining
   reading-order site now partitions as **0 in-flow/out-of-flow pairs** — t1124 took the whole class.
   `www.ikea.com` went 22 → 5, `rockstaractu.com` 12 → 5, and what is left everywhere is **2-sibling
   in-flow inversions scattered across distinct containers** (ikea: 5 inversions, 5 containers). Per
   the standing lesson that a reading-order symptom is a WIDTH upstream, that tail is near-miss
   geometry and should be worked as geometry, not as ordering.
2. **The two genuine one-pair rows are `www.lyreco.com` and `www.jatekshop.eu`** (1 inversion, 1
   container, shape 0.756 / 0.772). Trace before picking — §8.3.
3. **`hnhbkis.edu.in` is still the named replaced-element residue** (2 h-overflow, shape 0.927), and
   t1123 left the option set: thread the known cross size into `replaced_default_size`.
4. The definite-placement grid area (t1126's residue) is the other named seam.

## Check #105 — tick 1120 (2026-08-10)

HORIZON H0, re-scoped by **PART VII**: the four v1 components. Its gate is not a WPT number —
*"reliably renders and runs the representative real internet"*, plus the agentic surface, Bar-0
containment, and no pathological resource use. The measurable stand-in is **M1** (`shape ≥ 0.75` AND
jarring-clean) on the in-scope CrUX corpus.

### 1 · GATE OR SCOREBOARD? — gate, and for the first time in five windows a NAMED site crossed it

Ticks 1113–1120. `www.marktplaats.nl` — one of the three sites the burndown's §8.1 named as *"one
defect away from M1"* — now satisfies all four jarring invariants AND `shape ≥ 0.75`, measured
against the old binary rebuilt and run in the same hour (h-overflow `0/1 clean → 1/1 clean`, shape
0.964 → 0.967, reproduced twice). That is an exit-gate condition, not a scoreboard number.

⚠ **And it is ONE site of ~133, corpus-unmeasured.** The sweep is **612 hours stale**; the board's own
cadence rule says a batch of this size is past due. The honest sentence is *"one named crossing,
corpus unknown"*, and the steer below makes the sweep binding.

### 2 · ⚠⚠⚠ SIX OF THE LAST EIGHT TICKS ARE ONE DEFECT CLASS, AND PART VI HAS NO ROW FOR IT

Read as a list, this window looks like six unrelated layout fixes. Read by MECHANISM it is one:

```text
   t1113  a flex item loses its width when the container is shrink-to-fit AND a sibling grows
   t1114  a definite width IS the box's intrinsic contribution — the measure never asked the box
   t1115  a flex child FILLS the 1e6 measuring width, and the slack heuristic throws its items away
   t1116  a filled flex box answers for itself, and the frame it stops walking has to come back on
   t1119  an abspos flex child is emitted twice — the second copy laid out at the measuring width
   t1120  an intrinsic measurement writes 1e6 coordinates into a first-write-wins cache
```

**Every one is the intrinsic-measurement pass leaking into used geometry.** PART VI's H0.1 row
enumerates where the residual layout mass lives and every entry is a BOX TYPE — tables, inline
composition, floats and `clear`, out-of-flow under a transformed containing block, scroll containers,
the UA sheet. This window says the productive category is not a box type at all: **it is a PASS.**
The 1e6 probe is a second, unaudited layout of the whole subtree whose outputs are supposed to be
discarded, and it has now been caught writing to the used width (t1113–t1116), to the box tree
(t1119) and to a cache (t1120). PART VI is corrected below to carry it as its own row.

**The decision rule that falls out, and it is cheap:** for any memo, cache or side-table in layout,
ask *which passes can WRITE to it* — `static_pos`, `pre_transform_rect`, `transform_matrix`,
`measure_cache`, `taffy_item_width`. A throwaway pass is not a read-only guest. Two of those five
have now been caught (t1119 wrote a static position from a probe and had to move the write to
`extract_placed`; t1120 is `pre_transform_rect`), which makes this a two-of-five hit rate on a
question nobody had asked, not a hunch.

### 3 · I5 — THE DISCOVERY LADDER GREW A THIRD RUNG, AND THE DECISIVE STEP WAS THREE LINES OF SCRATCH

I5 names the differential oracle as the discovery engine; check #51 corrected it to *"the instrumented
log"*, check #104 to *"a grep over the test corpus"*. This window it was a **box-tree dump**: t1119's
whole finding is `root.walk(|b| if b.node == i { eprintln!(...) })` printing TWO boxes where the
oracle, the trace and two ticks of hypotheses had all reported one element with an impossible width.

```text
   the oracle    ranks SITES                    "marktplaats has 2 h-overflow elements"
   HOVF-TRACE    localises to an ELEMENT        "the <i>, every ancestor exact"       (t1112)
   the box dump  says what the ENGINE EMITTED   "there are two boxes and node_rects unions them"
```

⚠⚠ **`node_rects` UNIONS, so a double-emitted element reports a size no code ever computed.** t1111
and t1112 both searched for the code that computed 499,432 and there is none. The rung is worth
adding to VI.2's method list precisely because the two rungs above it were CORRECT and still pointed
at nothing: a metric derived from the box tree can only be interrogated in the box tree.

### 4 · INVARIANTS

- **I3 (semantic model in lockstep): SATISFIED CAUSALLY THIS TIME, after three checks of "by accident
  of scope."** Checks #72, #100 and #104 each recorded that geometry ticks satisfy I3 for free because
  they flow through the shared `LayoutBox::node_rects` producer. t1119 is the tick that stops being an
  accident: the DEFECT WAS IN THE PRODUCER. A double-emitted element's `node_rects` entry is the union
  of its two boxes, so the a11y bbox — and therefore the agent's click point, which is that bbox's
  centre — was a 499,432px-wide rectangle spanning half a million pixels of empty page. That is an
  actuation defect of exactly the kind check #72 said would appear *"the moment a fix touches the
  producer itself"*. It appeared, it was fixed, and I3 was the reason it mattered rather than a
  side-effect. **The named debt from #72 — that `node_rects`'s `lift` gives an icon-wrapping `<span>`
  the icon's 4px box — is still unwritten.**
- **I4 / VI.3 (usage-weighted breadth):** ⚠ **both ticks were SITE reductions, which the loop's own
  standing lesson says do not yield — and both generalised.** The reason is worth keeping: the
  selection was a site, but the LOCALISATION named a mechanism (*"an abspos child of a flex container"*,
  *"a first-write-wins cache reachable from a probe"*), and a mechanism can be priced by grep where a
  site cannot. The refinement to the lesson: **a site reduction yields iff its trace terminates in a
  construct the corpus can be grepped for.** Neither grep was run this window; that is a debt, not a
  finding, and `position:absolute` (76% of the corpus) × `display:flex` (46%) is where it would start.
- **I2 (never patch deps):** held. Taffy is untouched; t1119 changes which nodes we *hand* it and
  which of its answers we consume. The fork surface is still empty.
- **THE RATCHET** did visible work rather than being asserted: three candidate scopes for t1119 were
  each measured and REFUSED because they traded reftests away (`+5 −1`, `+5 −3`, `+5 −1`), and the
  scope that landed is the only one with a zero in the loss column. A net-positive trade is still a
  trade. **DIFF THE STATE, NOT THE NET** paid for itself twice in one tick.
- **I1, I6, I7, I8:** untouched this window.

### 5 · PART VI CORRECTION

H0.1's list of where the residual layout mass lives gains a row that is a PASS rather than a box type:

> **the INTRINSIC MEASUREMENT PASS itself** (added check #105, tick 1120). The 1e6 probe is a full
> second layout of the subtree whose outputs are contractually discarded, and six of ticks 1113–1120
> found it leaking instead — into a used width, into the emitted box tree, and into a first-write-wins
> cache. It is not a box type and no box-type fixture reaches it: every battery that found the other
> categories used a single layout of an in-flow subtree. The audit question is *which passes can write
> to this side-table*, and the side-tables are enumerable (`static_pos`, `pre_transform_rect`,
> `transform_matrix`, `measure_cache`, `taffy_item_width`).

### 6 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **RUN THE CORPUS SWEEP.** 612 hours stale, eight ticks unpriced, and one named M1 crossing
   that only a sweep can turn into a corpus number. Check #104's steer asked for this too. It is the
   acceptance test for t1113–t1120 and it decides the next several ticks; run `progress-metric.sh`
   on it per the board's top steer and read the DECOMPOSITION, not the conjunction.
2. **Audit the remaining three side-tables** against §2's question before the next probe-adjacent
   fix — `transform_matrix` is written by the same guarded call and is now safe, `measure_cache` is
   keyed by (node, width) and is probably fine, `taffy_item_width` and `static_pos` are not audited.
3. **Grep the corpus for `position:absolute` inside `display:flex`** — the price of t1119, unmeasured,
   and the first honest test of §4's refinement to the site-reduction lesson.
4. `hnhbkis.edu.in`, the other named one-element site, did not complete a fidelity run inside the cap
   twice this tick. Carry it into the sweep rather than re-attempting it solo.

## Check #104 — tick 1112 (2026-08-10)

Re-read of `CONSTITUTION.MD` PART I and VI.3, anchored to the eleven ticks since check #103. Three
findings, and the first one is a correction to VI.3 itself.

### FINDING 1 — ⚠⚠⚠ THE CORPUS GREP HAS A **FOURTH** INFLATION MODE, AND THIS SESSION PAID FOR IT

VI.3 binds the loop to `usage-weight × failing-breadth` and already lists three ways the usage-weight
term has been caught inflating: an unanchored grep matching a class name (`hover`), a co-occurrence
standing in for same-element application, and a legacy no-op VALUE standing in for the live
capability (`zoom: 1`, a 9× inflation). Its conclusion is *"a frequency is not a measurement until its
VALUES have been looked at, not just its property name."*

**t1107 looked at the values and was still wrong by the whole distance.** It priced
`::before`/`::after` whose selector subject is an inline element at **85 of 169 corpus pages (50%)**,
fetching each page's stylesheets and joining them with a single-computed key — the method VI.3 asks
for, executed correctly. The construct was real, the values were real (`q::before`, `a[href]::after`,
`label::before`, `.hlist li::after`), the fix was Chrome-exact and moved `css/CSS2` +21 in its own
directory. **The corpus moved −0.12 points, with 73 of 104 sites byte-flat** (t1109).

The fourth mode is not a counting error at all:

> **A construct's FREQUENCY is not its LEVERAGE.** `q::before` renders a quotation mark and changes
> no geometry. `.hlist li::after` is the only white space on its line and decides where the line
> breaks. The grep counts both as one page, and it is right to — the two are the same construct. What
> differs is whether the construct sits somewhere its width can change a LINE.

So VI.3's rule needs one more clause, and it is the clause the burndown's own mechanism ranking
already implies: **rank by usage-weight × failing-breadth × GEOMETRIC LEVERAGE**, where leverage is
whether the construct can change a line count or a box size — because that is what the metric this
loop is scored on can see. Stated as a test the next tick can apply before building: *does a probe of
this construct, on a page that declares it, move a BOX?* t1107 had that probe available (the anchor
site's `li1` read 130 against Chrome's 139) and generalised from it to 50% of the corpus. **One site's
leverage is not the corpus's.**

⚠ This is NOT an argument that t1107/t1108 were wrong ticks. `css/CSS2` +36 with 0 lost across the
two, 19 of 20 battery rows Chrome-exact, wikipedia +12.7 shape and its entire 394-element horizontal
overflow gone. It is an argument that **the loop cannot predict corpus movement from a frequency**,
and should stop implying it can in its own write-ups.

### FINDING 2 — I5 SAYS THE ORACLE IS INFRASTRUCTURE, AND THE LOOP HAD BEEN TREATING ITS OUTPUT AS FIXED

I5: *"the differential oracle is the discovery engine … maintained as first-class infrastructure."*
`www.marktplaats.nl` reported an element at `right 500083` in the t1089, t1099 and t1109 sweeps —
three sweeps, identical, for over a month. t1111 spent an entire tick guessing three mechanisms from
that one line and refuted all three. t1112 added **sixty lines** that walk the ancestor chain the
instrument's own keys already encode, and localised it on the first run: the `<i>`'s **own width is
499,432**, every ancestor exact — not a position artifact at all, which is the class t1111 had been
searching.

The pattern is now four for four: **t1088** (the probe could not see its own corpus), **t1090** (the
suite's ruler was not installed), **t1101** (the second argument of `getComputedStyle` was discarded),
**t1112** (the exemplar names the symptom). Each bought more than the engine tick it displaced. The
steer: **when a diagnosis costs more than one tick, the next tick is the instrument, not a fourth
hypothesis.**

### FINDING 3 — I3 SCORED THE LARGEST TERM OF THE WINDOW AND STILL HAS NO RANKER

I3 makes the semantic model load-bearing at every horizon. t1107's largest single measured term was
`dead_target 80 → 0` on the anchor — eighty links whose entire visible content was a pseudo, so the
box was degenerate and the agent had nothing to click. **It was found by a render tick, aimed at
shape, and priced by a metric with no term for it.** Check #72 (t852) raised exactly this and named
the fix as *"rank it as I3, landing it with an agent-side click-point assertion in the same tick."*
Sixty ticks later there is still no I3 ranker, and the win was again an accident. Recorded rather
than re-derived; the cheap version is that `dead_target` is ALREADY in every sweep row and nothing
reads it as an I3 signal.

### COMPLIANCE

**I5 held under pressure and cost me three headlines.** Five of six flagged sweep losses were the
SITES (old-binary control, t1110); a +5.7-point wikipedia A/B and a −10-overlap sestra.cc were both
ZERO under interleaved repeat runs; `mayatoys.in`'s +0.436 shape, the window's largest gain, belongs
to the site. **PART VII held**: `scripts/` untouched across eleven ticks including two blocked
commits. **The ratchet held twice by refusal** — t1111 reverted a principled, suite-green guard that
nothing could be made to go red without, and this window landed no change whose evidence was a single
run.

## Check #103 — tick 1104 (2026-08-10)

HORIZON H0, re-scoped by **PART VII**: the four v1 components. Its gate is not a WPT number —
*"reliably renders and runs the representative real internet"*, plus the agentic surface, Bar-0
containment, and no pathological resource use. The measurable stand-in the loop is scored on is
**M1** (`shape ≥ 0.75` AND jarring-clean) on the in-scope CrUX corpus.

### 1 · GATE OR SCOREBOARD? — **gate, and one of the three ticks was spent making the gate READABLE**

Ticks 1097–1103. Read honestly, the window splits three ways and not the usual two:

```text
   t1097  I3 was BENT, and the check that asked found it          measurement (forced by I3)
   t1098  generated content reaches the AX tree                   CAPABILITY — I3, discharged
   t1099  +825 suite tests moved M1 by ZERO, fourth sweep         measurement (forced by check #102)
   t1100  a leading `/` is the SERVER root                        instrument (reftest resolution)
   t1101  getComputedStyle(el,'::before') was DISCARDED           CAPABILITY — and it is an I3 tick
   t1102  a shape delta needs the same POPULATION                 instrument — and it is a RATCHET tick
   t1103  the monospace default size follows the FAMILY           CAPABILITY — +8.70 shape, attributable
```

**t1101 is not merely I3-compliant, it is I3 itself.** I3 names four things — *DOM, computed style,
layout geometry, and a first-class accessibility tree*. `getComputedStyle` **is** the computed-style
half of the semantic model, and its second argument was read and discarded, so every query about a
generated box was answered about a different box. That is the invariant's own surface returning a
wrong answer of the right type, and it had been doing so for the life of the API.

**t1103 is the clearest gate move of the window**: `doc.rust-lang.org/book` shape **0.791 → 0.878**,
three deterministic draws per arm, same instrument tag, same coverage, **same 713-element sample**.
More than twice the ±3.7-point spread t654 measured on an unchanged tree.

**And I3 was free on the two box-moving ticks, by the rule t1098 established** — a fix that moves a
BOX reaches the AX tree through `node_rects`, a shared producer; a fix that adds TEXT does not. t1103
moves boxes (free); t1101 adds neither box nor text and is itself a semantic-model surface. Stated
rather than assumed, because "it passes because a shared producer carries it" stops being true the
moment a tick touches the producer (check #72's finding).

### 2 · ⚠⚠⚠ THE WINDOW'S GOVERNING FINDING: THE LOOP'S OWN BANKED NUMBERS ARE BEING USED AS CONTROLS, AND A BANKED NUMBER HAS A HARNESS TOO

Two independent instances, four ticks apart, and neither was noticed by any gate:

```text
   t1102   www.timeline.com  -9.2 shape pts, "REPRODUCES" on a solo re-run, carried for TWO ticks
           as the window's one unrefuted regression candidate.
           → the OLD binary (pre-t1092), rebuilt, run in the same hour, emits the t1099 row to
             SIX DECIMALS. The engine never moved. Coverage had fallen 13 points and the scored
             sample by 159 elements: two means over different samples of a page that changed.

   t1103   css/CSS2 read 3,863 against the 3,858 t1100 banked — apparently +5.
           → the OLD binary scores 3,862 TODAY. The honest delta is +1 / −0 over a per-test state
             diff of all 5,660 rows.
```

**One rule, two victims: *every number has a harness, and a banked number's harness is the tree that
produced it.*** Lesson 4 in STATUS.md has fired three times about the *measurement's* harness. This
is the same rule one level out — the **BASELINE's** harness — and the loop has been differencing
against journal entries as if they were re-measurable constants. *Diff the state, not the net*
(t1089) applies to the baseline as much as to the delta.

⚠⚠⚠ **AND THE t1102 HALF GENERALISES INTO A NAMED INSTRUMENT CLASS, WHICH IS WHY IT IS IN THIS CHECK
AND NOT ONLY IN THE JOURNAL.** Of the 25 sites that moved more than 2 shape points between the t1089
and t1099 sweeps, **6 are population changes — and those 6 are all five of the largest losses AND the
largest gain**:

```text
   sports.yahoo.com  -0.856   n 1693 → 3      cov 0.991 → 0.273
   www.timeline.com  -0.092   n 1197 → 1038   cov 0.979 → 0.849
   www.paypal.com    -0.090   n  534 → 429    cov 0.893 → 0.717
   mangaraw.ac       -0.067   n  733 → 873    cov 0.836 → 0.755
   pogoda.by         -0.057   n   71 →  53    cov 0.696 → 0.510
   www.aftenbladet.no +0.131  n  999 → 622    cov 0.951 → 0.924
```

**What survives the partition answers t1099's headline.** The 19 attributable movers are 7 losses and
12 gains, **net +0.830 shape points, worst single loss −0.041** — inside the measured noise band. On
the sites where the comparison is legitimate at all, the window moved shape **UP**. The famous
*"+825 suite tests, +0 M1"* was read off a diff whose six loudest rows were never the engine's to
answer for. It remains true that M1 did not move (M1 is a per-site threshold and none of these
crossed it) — but the **inference** the loop was drifting toward, that six engine ticks bought
nothing, is refuted.

⚠⚠ **A SOLO RE-RUN IS STRUCTURALLY BLIND TO THIS.** It measures today's population twice and agrees
with itself perfectly — which is exactly what t1099 did, three times, before writing `REPRODUCES`.
The solo re-run is the right instrument for **churn** (one binary, one hour, two answers) and cannot
see **drift** (one page replaced by another). Those are two failure modes and the loop owned one
probe. It now owns two: `fidelity::sweep_diff` / `manuk-wpt sweep-diff`, gated by
`G_SWEEP_DIFF_POPULATION`, RED-proven on both classifier arms, with thresholds varied 6×/5× to show
they are not fitted at one point.

### 3 · INVARIANTS — one is worth recording as a POSITIVE, and none is bent

- **I2 (sanctioned deps, never patched internally).** t1103 is the cleanest instance of the
  *borrowed-engine ladder* landing at **option 1** in a long time. The monospace default size looked
  like a Stylo behaviour; it is a **hook we already implement and had stubbed**
  (`Device::base_size_for_generic` returned `16.0` and ignored its argument). One match arm, no
  vendored fork, and the UA sheet **lost** a rule rather than gaining one. **The fork surface is
  still empty.** Recording the positive because the ladder is usually invoked to justify a delta;
  here it correctly *avoided* one.
- **I3.** Discharged and then extended — see §1.
- **I4 (Pareto discipline).** t1103 was ranked by corpus usage (`<code>` inside a page that sets its
  own font-size = every documentation site, wiki and spec page) and **explicitly declined** the
  bigger-looking number: wikipedia's nested table at 4430px vs Chrome's 397 is 2,254 of 4,843 shape
  misses on that anchor and was left open rather than half-fixed. That is the invariant working.
- **I5 (the oracle is the discovery engine).** The route into t1103 was `--shape-dump` on an anchor →
  a 4-row fixture that **refuted** the obvious hypothesis in two minutes → a 16-row battery. The
  subject was not what made me look, for the fourth time this arc.

### 4 · CORRECTION TO PART VI

`VI.2`'s layout row already carries the named instrument classes (the mis-provisioned reference; the
82–87% scorability ceiling). **Add a fourth, from t1102: the POPULATION-CHANGED between-sweep
delta.** It is distinct from the ceiling — the ceiling is about which sites can be scored *at all*,
this is about whether two scores of the *same* site are draws from the same experiment — and it is
the one that corrupts the loop's reading of its own SLOPE rather than its level. The correction is
written into VI.2 in this tick.

### 5 · THE STEER — three, in order

1. **WIKIPEDIA'S NESTED-TABLE BLOW-UP IS THE NAMED NEXT LEVER, AND IT MUST BE REDUCED FROM THE REAL
   SUBTREE, NOT A SYNTHETIC ONE.** 4430px against Chrome's 397; 2,254 of 4,843 shape misses and 363
   h-overflow on one anchor — the largest single identified shape term the loop has. ⚠ The synthetic
   fixture came out **byte-exact against Chrome**, and *that is the finding*: the mechanism is not in
   the shape of the construct, so the next attempt must carry the real markup in. A second synthetic
   battery would refute nothing new.
2. **RUN A CLEAN `--jobs 2` SWEEP AND DIFF IT WITH `sweep-diff`.** It will be the first sweep diff
   the loop can actually read, and the first honest reading of whether t1101/t1103 move the corpus.
   ⚠ It will also correctly report **INSTRUMENT-CHANGED** for nothing and **POPULATION-CHANGED** for
   whatever has drifted since t1099 — expect the attributable set to be much smaller than the row
   count, and rank on it rather than on the raw diff.
3. **NEVER DIFF AGAINST A JOURNAL NUMBER AGAIN.** A control is a binary you rebuilt and ran in the
   same hour, or it is not a control. This cost a false `+5` in t1103 and would have cost a false
   regression in t1102 had the control not been run. If the rebuild is too expensive to pay for,
   the honest report is *"unmeasured"*, not a difference against a remembered constant.

## Check #102 — tick 1096 (2026-08-10)

HORIZON H0, and its gate: ~83% WPT across categories, oracle-verified on four corpora, a
daily-drivable shell, semantic-API coverage of every rendered construct.

### 1 · GATE OR SCOREBOARD? — **gate, for the first window in a while, and by a measurable margin**

Eight ticks (1089–1096). `css/CSS2` went **3,029 → 3,843** (+814, +26.9%) and **every point of it was
re-measured old-binary-vs-new in the same hour, per-TEST, with the losses named**:

```text
   t1090  Ahem installed              +336 / −4      the suite's ruler was not on the host
   t1092  generated box display       +16  / −0      §12.1
   t1093  generated box suppressed    +6   / −0      display:none on a pseudo
   t1096  CSS counters                +31  / −0      §12.4
   t1089/1091/1094/1095               measurement — a sweep, a re-rank, a pricing, a decomposition
```

That is a *category* answer, not a scoreboard one: three of those are constructs the open web uses on
47–68% of the corpus's own pages. **Four of the eight ticks were measurement**, which is high, and
each was forced: the sweep by check #101 steer #1, the re-rank by steer #2, the pricing by VI.3's
first term never having been computed for the family, the decomposition by the loop's own rule about
subsystem-scope levers. None of them was chosen instead of building.

### 2 · ⚠⚠⚠ THE WINDOW'S GOVERNING FINDING: A BLIND INSTRUMENT MIS-RANKS THE WORK-LIST, NOT JUST THE SCORE

Check #101 §2 said *"any CSS 2.1 chapter pass-rate read before this tick must be re-read"*. That was
too weak. Re-read on the fixed runner, the **ordering** inverted at the top:

```text
   ::first-letter        8 of 1,843 failures =  0.4%     ← surface audit #48 had this QUEUED at 10.5%
   content + counters  198 of 1,843 failures = 10.7%     ← had no row at all
```

The 10.5% was real and **attached to the wrong subsystem**. A blind instrument does not add noise
evenly: it deletes whole classes of PASS, the deleted classes correlate with the mechanism, and the
surviving failure set is a *biased sample that names the wrong cause with confidence*.
`margin-padding-clear` is the same error at the other end — carried as "~280, one unidentified shared
cause, three hypotheses refuted", it is **66 at 90.3% pass**; the hypotheses were refuted against a
number 4× too large, which is exactly why none of them explained it. **PART VI should record the
general rule: after an instrument fix, re-derive the RANKING, not just the score.**

### 3 · ⚠⚠⚠ THREE FALSE LEVERS IN THREE TICKS, ALL THE SAME SHAPE — and one of them was mine, one tick after I published the lesson

Check #101 steer #3 asked for external stylesheets, priced at *"1,231 refs, 19.7%, the same size as
the one just fixed"*. The size was right; the mechanism was not. **96% of those links are one URL and
the file is not in the checkout.**

```text
   1,231 external stylesheets  →  1,640 of 1,707 point at /fonts/ahem.css, and wpt/fonts/ is ABSENT
     254 "reference unreadable" →  239 are an absent wpt/css/reference/; only 14 are a real bug (17×)
   1,885 occurrences of a construct → the corpus join undercounted 5× on a newline in a hash key
```

The middle row was **written into t1091's own journal entry as a confident diagnosis of
`resolve_sibling`, one paragraph after that entry published the rule against exactly this**, and was
caught only because the tick was re-read before landing. It is recorded rather than quietly fixed
because the failure is *in the loop's method*, not in the engine.

> **A lever priced by COUNTING a construct is not priced until you have read what the construct POINTS
> AT.** A uniform reason string is a property of the READER; grouping by it groups causes together.
> The check is one `[ -f ]` per row.

And the underlying fact, so it is not rediscovered a fourth time: **this WPT checkout is PARTIAL** —
`wpt/fonts/` and `wpt/css/reference/` are both missing.

### 4 · ORIENT'S RANKING (§VI.3) — the first term finally EXISTS, and it changed two decisions

VI.3 binds the loop to *usage-weight × failing-breadth*, and `CORPUS-CONSTRUCTS.md` had no row for
the pseudo family. t1094 computed it (`CORPUS-PSEUDO-t1094.tsv`, 170 pages) and it did real work in
both directions:

- it **promoted** `display:none`-on-a-pseudo (47%) above the rest of the cluster — t1093;
- it **demoted** `clip: rect()` despite 36% of pages and 44 suite failures, because `clip` is
  **paint-time** and both engines report the same box: its M1 weight is *structurally zero*. ⚠ That is
  a gap in what the metric can express, not in the engine, and PART VI should say so — **the
  box-diffing oracle has no term for a paint-only property.**

⚠⚠ **And the harder half: usage weight ranks where to LOOK, never whether anything is THERE.** Two of
the family's seven constructs were already implemented — clearfix at 30% (the idiom the whole arc was
*selected* on) and `content: attr()` at 25%. **55%-of-corpus worth of "lever" that ranking alone would
have queued**, found by probing before building, twice in one window.

### 5 · INVARIANTS

- **I2 (never patch deps):** held. Stylo parses counters correctly and always did; the defect was
  ours, in the mapping. The fork surface is still empty.
- **I3 (semantic model in lockstep):** ⚠ **satisfied, and for the FOURTH consecutive check by accident
  of scope.** t1092/1093/1096 all change generated content, which flows through the shared
  `LayoutBox::node_rects` producer, so the agent's click points move with the boxes for free. Checks
  #72, #100 and #101 said this. ⚠⚠ **But this window adds a NEW I3 question nobody has asked: is
  generated content IN THE AX TREE AT ALL?** A `::before` that renders `"S1. "` is content a screen
  reader announces and an agent may need to read, and it is not in the DOM by construction. **Named
  here as an open invariant question, not a claim in either direction.**
- **I5:** the discovery engine this window was §3's method — partitioning a failure set by a property
  of the source — plus the per-TEST state diff. **Four separate conclusions were killed by re-reading
  rather than re-reasoning**, including two of my own.
- **THE RATCHET:** absolute and load-bearing once. t1092's first version scored **+18 with two
  regressions**; the value was dropped for **+16 with zero**. *Net 61 either way* — which is why a net
  is not a verdict, and why every tick this window diffed STATE.

### 6 · PART VI CORRECTIONS

1. **After an instrument fix, re-derive the RANKING, not just the score** (§2).
2. **A lever priced by counting a construct is not priced until you have read what it points at** (§3).
3. **The WPT checkout is partial** — `wpt/fonts/`, `wpt/css/reference/`.
4. **M1 has no term for a paint-only property** (§4) — `clip`, and by extension filters, blend modes
   and anything else that changes pixels without moving a box.
5. **Diff the STATE, not the net** — a net cannot separate inert from 36-in-36-out, nor a gain from a
   trade.
6. **Bank BOTH rankings, by chapter and by failure-family** — they fail differently, and t1096 put 28
   of its 31 gains in a chapter the chapter-ranking had hidden.

### 7 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **RUN THE FIDELITY SWEEP.** Four engine ticks have landed since t1089's sweep and the M1 gate
   is unmeasured on all of them. This is the third check running to say it, and the honest reason it
   keeps slipping is that the sweep costs ~2h and cannot overlap a tick — but t1089's own finding
   (three of four headline percentages fell while every count rose) is exactly what an unmeasured
   window hides.
2. **ANSWER THE I3 QUESTION IN §5** — is generated content in the AX tree? One probe, no build. If it
   is not, three of this window's four capability ticks shipped without their semantic-model exposure
   and I3 was bent, not satisfied.
3. **Counters brick 4 — SCOPING** (`counters(c, ".")` prints the flat value today), and the
   `InlineTable` cascade split that capped t1092 at +16.
4. **Per-character fallback through the AUTHOR's font-family list.** `resolve_face` jumps from the
   primary straight to a hard-coded `FALLBACK_FAMILIES` and never consults the rest of the author's
   list — surfaced by t1090 as an accidental pass turning honest, and it is a real defect on every
   mixed-script page.

## Check #101 — tick 1088 (2026-08-09)

**HORIZON: H0. THE GATE, from PART VII rather than memory:** *"reliably renders and runs the
representative real internet"* — **not** a WPT percentage. The instrument is **M1 on the in-scope
CrUX corpus** (`shape ≥ 0.75` **AND** jarring-clean, bot-walls excluded per
`DAILY-DRIVER-CERTIFICATION.md` §3), banked in `FIDELITY-PROGRESS.tsv`.

### 1 · GATE OR SCOREBOARD? — scoreboard again, but check #100's steer WAS obeyed, once

Check #100 (tick 1079) ended: *"make the gate the acceptance test for the arc, which is one sweep."*
**t1080 ran it** — `SWEEP-t1080-rows.tsv`, and its own title is the finding (*"31 ticks moved the
gate by ZERO"*). That is compliance, recorded because a check that only ever reports drift teaches
the loop nothing about what obedience looks like.

Since then: **eight ticks, no sweep.** t1081–1083 static positions, t1084–1085 measurement, t1086–1087
RTL, t1088 the instrument. The lever board has printed *"a capability tick must measure THIS tree"*
on every one of them. The pattern is stable and worth naming precisely rather than re-confessing:
**the loop reliably runs the sweep in the tick immediately after a constitution check and not
again** — i.e. the cadence gate is doing the work the selector should. That is a design observation,
not an excuse: the honest reading is that the sweep's cost (~2h, serial, and it cannot overlap a
tick because contention false-REDs the perf gates) makes it structurally unaffordable at the loop's
current tick rate, so it will keep being paid only when something forces it.

### 2 · ⚠⚠⚠ THE INSTRUMENT THE LAST FIFTEEN TICKS SELECTED WITH WAS BLIND ON A FIFTH OF ITSELF

This is the finding of the window and it invalidates a steer that is currently binding. t1088:
**1,230 of `css/CSS2`'s 6,263 reftests (19.6%) have a reference built out of `<img>` swatches**, and
the runner rendered both sides with the sync `Page::load`, which fetches no subresources. Those
references painted blank boxes. Nine directories, old instrument vs new, same hour:

```text
   positioning 187→314 · normal-flow 320→465 · backgrounds 184→220 · borders 324→349
   floats-clear 31→79 · linebox 14→51 · margin-padding-clear 592→603 · floats 23 (=) · bidi-text 17 (=)
```

**Surface audit #48's chapter ranking, and check #100's own §5 steer built on it, are numbers about
the runner.** `linebox` was cited as *"a missing primitive at 7.4%"*; it is **20.3%**. Any CSS 2.1
chapter pass-rate read before this tick must be re-read before it ranks anything.

⚠ **AND THIS IS THE FOURTH MEMBER OF THE CLASS CHECK #93 NAMED — the MIS-PROVISIONED REFERENCE.**
`--hide-scrollbars` (the gutter), `--window-size` (t1016), the interaction media features (t1020),
and now **the reftest reference rendered undressed**. Identical shape every time: *the reference was
not the document we asked for, and the difference was charged to the engine.* The new member changes
the class's boundary, and PART VI should record it: the first three were **Chrome's provisioning**
and this one is **ours**, inside `tests/wpt`. The decision rule check #93 gives (*"which way to go
depends on whether the reference CAN be provisioned"*) answers instantly here — it is our code — but
the *detection* rule needs widening: this was found by rendering the reference on its own and looking
at one pixel row, which nothing in the loop routinely does.

⚠⚠ **The half that keeps it honest: `bidi-text` is FLAT at 17 with 48 image-based references, and so
is `floats`.** An unloaded PNG was masking real failures, not inventing them. Two directories not
moving is what separates a measurement fix from a scoring trick.

### 3 · I5 — THE DISCOVERY ENGINE THIS WINDOW WAS NEITHER THE ORACLE NOR THE LOG

Check #51 corrected I5 once already (*"the primary discovery engine is now the INSTRUMENTED LOG"*).
This window it was a **third thing, and it should be written down**: `grep` over the *test corpus's
own source text*, partitioning failures by a predicate.

```text
   t1087   19 of margin-padding-clear's 20 `margin-right` failures declare `direction: rtl`
           (76% of its 25 RTL files) against 1 of 45 non-RTL          → 3 minutes, no build, one tick
   t1088   1,230 of 6,263 references contain `<img>`                  → 3 minutes, no build, one tick
```

Both cost minutes and each produced a landed tick. The oracle ranks *sites*, the log gives *depth*,
and this gives **partitions of a failure set by a property of the source** — which is the only one of
the three that can say *"these 50 failures are one thing"* before any of them is opened. Add it to
VI.2's method list rather than leaving it as two anecdotes.

### 4 · INVARIANTS

- **I2 (never patch deps):** held. t1086 reached Stylo's logical→physical resolution by **giving it a
  declaration it never had** (two UA rules), not by touching it — the same cheapest-form shape check
  #100 recorded for `PseudoElement::FirstLetter`. The fork surface is still empty.
- **I3 (semantic model in lockstep):** ⚠⚠ **satisfied, and for the THIRD consecutive check by
  ACCIDENT OF SCOPE.** t1086 and t1087 both move element geometry, and geometry IS the semantic model
  — they flow through the shared `LayoutBox::node_rects` producer, so the agent's click points move
  with the boxes for free. Checks #72 and #100 said the same thing. **A property satisfied three
  times running by an accident is a latent violation waiting for the tick that touches the producer**,
  and t1085's own residue (`node_rects`'s `lift` giving an icon-wrapping `<span>` the icon's 4px box)
  is exactly that tick, still unwritten. Escalated here from an observation to a named debt.
- **I4 / VI.3 (Pareto):** ⚠ **an honest tension, stated for the owner rather than resolved by the
  loop.** The RTL arc (t1086/t1087) is **thin on the corpus** — 4 of 182 cached sites carry
  `dir="rtl"`, and 1–2 combine it with logical properties — and it is **100% of the Arabic, Hebrew,
  Persian and Urdu web**, which is on the order of 700M readers. I4 says *"representative real-web
  breadth weighted by actual usage"*. If "usage" is the CrUX-sampled corpus we hold, this arc is
  tail. If it is global usage, the corpus is the thing that is unrepresentative. **The loop cannot
  decide that and must not decide it silently**; it is recorded here so the next corpus revision
  answers it deliberately. (Note the arc was not selected on that basis — it was selected by a suite
  partition, and it paid in suite tests.)
- **I5:** see §3. **I1, I6, I7, I8:** untouched this window.

### 5 · THE STEER — binding on the next tick

1. ⚠⚠⚠ **RUN THE FIDELITY SWEEP.** Eight ticks, and check #100 asked for exactly this. It is the
   acceptance test for two RTL fixes and one instrument fix that have only suite evidence.
2. **RE-RANK CSS 2.1 BY PASS RATE ON THE FIXED RUNNER**, and treat every chapter number in
   `SURFACE-AUDIT.md` #48 and check #100 §5 as retired until re-read. One command, and it decides
   the next several ticks.
3. **THE REFTEST RUNNER STILL FETCHES NO EXTERNAL STYLESHEETS** — the same blindness one layer up,
   named and unmeasured, and cheap to price the same way (`grep -l 'rel="stylesheet"'` over the
   references).
4. **Build the CSS 2.1 tests into the wall, or accept that this crate's tests will break a third
   time.** They did not compile from t563 to t1088 — twice, for the identical reason (a field added
   to `Seen`, three test constructors missed), and the comment on the constructor predicted the
   repeat and did not prevent it. A prediction in a comment is not a gate. `scripts/` is
   observer-owned, so this is a request, filed here and in the journal.
