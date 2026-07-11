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
