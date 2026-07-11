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
