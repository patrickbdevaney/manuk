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
