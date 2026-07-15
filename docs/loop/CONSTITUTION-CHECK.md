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
