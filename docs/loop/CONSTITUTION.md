# Manuk — Autonomous Perpetual Engineering Loop (Constitution)

This is the governing document for an indefinitely-running, self-directed engineering loop that
drives Manuk toward a **global maximum** of browser excellence: a production-grade, polished
browser that is genuinely competitive with Chromium/WebKit/Gecko, human-first from first
principles, and **agent-native** for the automation era. Any session (human-triggered or
scheduled) reads this file first, then executes the loop below. The loop is deterministic to
resume: it lives in `docs/loop/` markdown, not in any one session's memory.

---

## 0. Prime directive & north star (invariant)

Build the best browser. Concretely, drive every quality axis upward toward a coherent global
maximum without ever regressing one axis to gain another.

**Maximal traversal, earned by capability (amended — see ADR-004).** The ambition is near-total
traversal of the *real* internet: Chromium/Gecko-parity breadth across the **kinds** of site the
web is actually made of — real-time virtualized feeds, session-heavy professional platforms,
media-rich client apps, complex authenticated dashboards. Named sites (X, LinkedIn, Indeed,
Instagram, cloud consoles) are *representative points in that space*, **never a checklist** — the
target is the whole space they stand in for.

Manuk earns that access the way any real browser does: **by being one.** Its own genuine TLS
handshake, its own genuine engine fingerprint. Chrome has a Chrome fingerprint; Firefox a Firefox
one; **Manuk has a Manuk fingerprint, and it is earned, not hidden.** The whole strategy for
coverage is closing the **capability gap** — full JS/DOM depth, full layout/CSS fidelity,
real-time-feed-grade performance — until *being genuinely Manuk* is sufficient anywhere a real
browser is welcome. **A fifth real browser, not a disguise wearing someone else's face.**
Impersonation isn't just forbidden by §1.4 — it is *off-strategy*, a substitute for the very
capability that is the point.

The north star has two faces that must both hold:

- **Human-first.** A real person's daily driver: correct CSS/HTML/JS on the modern web,
  legitimate **human-user** fingerprinting (truthful — never evasion or competitor
  impersonation), fast/lean/low-memory with tab hibernation, and an ergonomic, fully accessible
  UI.
- **Agent-native.** Built from first principles for the agentic era: an in-browser AI agent via
  the local llama.cpp/GGUF runtime, **and** a first-class in-process automation surface an
  external harness can drive — the in-process advantage over CDP-over-socket, measured and real.

**Ambidextrous spine (amended — ADR-004): one engine; the split is who's driving, not which
binary.** A human drives the headful GUI. An agent drives **either** headless mode (no window;
scale/throughput) **or the same headful GUI, visibly and live**. Both are the identical engine and
page pipeline — differing only in whether a window is presented and who issues the actions. "Shared
core, diverge at consumption," made literal. **No agent-only or human-only fork of the page
pipeline**: an agent action in headful mode goes through the same code a human click does.
Divergence is a defect.

These faces are complementary, not in tension. The loop expands the **entire possibility surface**
of a diverse, cohesive, coexisting feature set — it is not limited to today's list.

**Prioritization consequence.** Rank candidates by *traversal-blocking capability*: "which class of
the real web does this unblock?" That elevates JS/DOM depth (remaining WebIDL surface), layout/CSS
fidelity (now VISUAL-verifiable, §7), virtualized-feed performance (scroll/recycle/incremental
relayout under a live feed), and session/auth durability (cookies, storage partitioning, OAuth,
long-lived logins).

## 1. Invariants (never violated; violating one fails the tick)

1. **Parity gate stays green:** `cargo run -q -p manuk-wpt --release -- parity` = 72/72 (±3px
   vs headless Chrome). This is the non-regression floor for rendering.
2. **Build green:** `cargo build --workspace` compiles; touched crates' tests pass.
3. **Memory safety:** no `unsafe` outside the sanctioned FFI boundary; **never** patch
   SpiderMonkey/mozjs or Stylo internals — only their embedding surfaces. Reuse audited crates
   for crypto/Unicode/JIT/GC; never hand-roll them.
4. **Honesty:** truthful UA/fingerprint for a human browser; no evasion/anti-detection/
   competitor-impersonation; report outcomes faithfully (headless-verified vs GUI-bound vs
   needs-external — never claim GUI pixels are verified when they were not).
5. **Coherence:** every change is committed to `main` and pushed, commit message ending
   `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`; each change is
   seam-scoped and self-contained (no half-landed features on `main`).
6. **No local regression:** a change may not lower any axis score in `LEDGER.md` to raise
   another. If a tradeoff is unavoidable, it is an ADR in `DECISIONS.md`, not a silent regress.

## 2. Knowledge base (the Karpathy-wiki; prioritized, cross-linked)

All in `docs/loop/`. Keep each concise; prune ruthlessly; link with `[[file]]`.

- **CONSTITUTION.md** — this file. The rules. Amended only via an ADR in `DECISIONS.md`.
- **STATE.md** — living snapshot: what the browser *is* and *has* right now, per axis. The
  "take stock" source of truth. Updated every tick that changes reality.
- **LEDGER.md** — the work ledger + bandit scores. Every candidate item: `id`, one-line intent,
  axis, value V(1-10), cost C(1-10), uncertainty U(1-10), touches T, status
  (`backlog|active|done|superseded|blocked`), verification class. The selection source.
- **RESEARCH.md** — frontier scan notes (how Chromium/WebKit/Gecko/Servo/Ladybird do X; papers;
  primary sources). Feeds new LEDGER items. Dedup against here before re-researching.
- **DECISIONS.md** — append-only ADRs: tradeoffs, constitution amendments, axis changes,
  north-star-drift rationale. Long-horizon coherence lives here.
- **JOURNAL.md** — append-only, one short block per tick (tick#, item, outcome, verify, commit).
  Minimal history for audit + resume.
- **RESUME.md** — the single deterministic **resume pointer**: the exact next action and the
  minimal context to continue, overwritten every tick (and on token exhaustion mid-item).

## 3. Axes of the global maximum (extensible)

Track a score (0-100) per axis in `STATE.md`. Add axes as the surface expands; never delete an
axis's history without an ADR. Seed axes:

`RENDER` (CSS/layout/text fidelity) · `JS` (engine + DOM/BOM + SPA support) · `NET` (protocols,
cache, cookies, TLS) · `UI` (chrome, ergonomics, accessibility, a11y) · `PERF` (latency,
throughput, frame pacing) · `MEM` (footprint, hibernation) · `AGENT-IN` (in-browser llama.cpp
agent) · `AGENT-EXT` (external automation tool surface) · `FINGERPRINT` (legit human-browser
identity) · `COMPAT` (real-world site support: SPAs, auth flows, media) · `STABILITY` (crash-
freedom, polish) · `SECURITY` (sandboxing, safe defaults).

The goal is the **joint** maximum: a broad, cohesive frontier, not a spike on one axis.

## 4. The loop (one tick)

```
TICK:
  1. TAKE STOCK   read RESUME.md, STATE.md, LEDGER.md (and RESEARCH.md if exploring).
  2. SELECT       pick the next item by UCB (§5). Dedup: skip done/superseded; verify the
                  chosen item still matches reality (files/symbols exist).
  3. RESEARCH     if the item is under-explored (high U), do a bounded scan (primary sources
                  first), append findings to RESEARCH.md, refine the item's plan.
  4. IMPLEMENT    smallest coherent change that advances the item. Robust, defensive code
                  (assume it must work in every situation; a human verifies GUI later).
  5. VERIFY       build + parity + touched tests. Headless-verify what can be; mark GUI-bound
                  items as such. An invariant breach (§1) → revert/fix, do not commit.
  6. REFLECT      update LEDGER (T++, re-score V/C/U, set status), STATE.md (axis deltas),
                  DECISIONS.md (if a tradeoff/amendment). Spawn any newly-discovered items.
  7. CHECKPOINT   append a JOURNAL block; overwrite RESUME.md with the next action.
  8. COMMIT       git add -p the seam; commit (co-author line); push. Then goto TICK.
```

Run ticks continuously. There is **no obligate handback**: keep going until tokens run low, then
§6. A tick should be small enough to finish and commit — prefer many small green ticks over one
large risky change.

## 5. Selection — explore/exploit (UCB / GEPA-flavored)

For each `backlog` item compute:

```
exploit  = V / C                       # value per unit effort (both 1-10)
explore  = K_e * sqrt( ln(1 + TICKS) / (1 + T) )   # optimism for the untried
noveltyU = K_u * (U / 10)              # bonus for high-uncertainty / blue-ocean edges
score    = exploit + explore + noveltyU
```

Pick the max `score`. Defaults `K_e = 1.5`, `K_u = 1.0`. **Cadence:** every 5th tick force an
**explore** pick (highest-U item, ties→least-touched) even if a higher-exploit item exists — this
is how the loop reaches new surfaces instead of grinding one axis. GEPA-style: after implementing,
*reflect* on what the change revealed and mutate the LEDGER (new items, re-scored V/U) — the
population of ideas evolves; good directions breed neighbors, dead ends are marked `superseded`.

Non-regression guard: an item that would raise its axis but lower another is **blocked** until an
ADR resolves the tradeoff or the change is redesigned to be Pareto-improving.

## 6. Checkpoint & deterministic resume (token-exhaustion safe)

The loop must survive a session ending at any point:

- **Every tick** ends by overwriting `RESUME.md` with: current TICK number, the exact next item
  id + its one-line plan, any partial-progress notes, and the shell commands to re-establish
  context (build/parity). This is the single source a fresh session needs.
- **Mid-item exhaustion:** if a tick can't finish, still write `RESUME.md` describing the partial
  state (files touched, what's left, how to verify), and either commit a green partial behind a
  flag or leave the working tree noted as dirty in RESUME. Never leave `main` red.
- A new session's first act is: read `CONSTITUTION.md` → `RESUME.md` → resume at the named item.
  Same inputs ⇒ same next action (determinism).

## 7. Verification classes (honesty tags)

Each LEDGER item carries one. Never upgrade a class silently; never claim pixels are verified when
they were not.

- `HEADLESS` — provable via test / parity / **render screenshot**. Do so.
- `VISUAL` — the rendered *pixels* must be right. **Now autonomously verifiable** (user unblocked
  it, Tick 13): `cargo run -q -p manuk-wpt --release -- render (--html FILE | --inline HTML)
  --out PNG [--width W] [--height H] [--chrome]` paints the page through the CPU painter (no
  window/GPU) to a real PNG; **Read the PNG to eyeball it**, and `--chrome` writes a headless-
  Chrome reference PNG beside it for side-by-side diff. This is how a "look like Chromium" item is
  now worked + verified — no display needed. (It already caught the flex/block-child collapse.)
  *Limitation:* renders page content, not the shell chrome (tab strip/menus) — a shell headless-
  paint path is the follow-on for chrome-pixel items; until then those stay `GUI`.
- `GUI` — live winit-window interaction (real input events, GPU present, multi-window). Still
  needs a human at the machine; write robustly and mark "needs user verification". Prefer to
  carve off a `VISUAL` or `HEADLESS` slice that IS checkable.
- `EXTERNAL` — needs a service/model/key. **llama.cpp is now runnable** (user granted, Tick 13):
  models live under `/home/patrickd` (e.g. `qwen_35_4b_claude/Qwen3.5-4B.Q4_K_M.gguf` + an
  `mmproj` vision projector); `llama-server` at `~/llama.cpp/build/bin/llama-server`. Start:
  `llama-server -m <gguf> --host 127.0.0.1 --port 8099 -c 2048 -ngl 0 --no-webui &`, poll
  `/health`, hit the OpenAI-compat `/v1/chat/completions`. Qwen3.5 is a reasoning model — append
  `/no_think` for clean JSON-only output. Verified end-to-end: prompt → `{"Type":{"field":"Email",
  ...}}` → `grounding::ground_action`. **Stop the server when done** (frees RAM/CPU); restart on
  demand. The mmproj enables multimodal (screenshot-in) grounding — a future lever.
- `MEASURE` — a number must be produced (latency/memory) and published.

## 8. Disk hygiene (every tick; the loop must not fill the disk)

An indefinite loop accretes build artifacts. Each tick's CHECKPOINT step also reclaims space,
**incrementally and safely** (never delete source, git, or `docs/loop/`):

- After a successful release verify, prune stale **debug** artifacts: `cargo clean -p <crate>`
  for crates not currently under test, or remove `target/debug/incremental/` and old
  `target/debug/deps/*` that the current build no longer references. The release binary used by
  the parity gate + renders is what matters; debug bloat is disposable.
- Delete superseded render/scratch outputs and any `*.png`/temp probes outside the repo (use the
  session scratchpad, not the repo).
- Periodically (every ~10 ticks or when `target/` exceeds a few GB) run a fuller `cargo clean`
  keeping only what the next tick needs, then rebuild release once. Log reclaimed space in
  JOURNAL.
- Guard: never `cargo clean` away the release artifacts mid-loop without rebuilding; never touch
  `~/.cargo` registry caches (re-download cost) or vendored engine trees (`mozjs/`, `stylo/`).

Concretely, a safe default each tick: `rm -rf target/debug/incremental` and prune
`target/debug/` if it exceeds ~1.5 GB; keep `target/release`.

## 9. Anti-goals

No feature bloat that regresses PERF/MEM/STABILITY; no evasion/anti-detection; no dark patterns;
no vendored-engine internal patches; no unverified "done" claims; no north-star abandonment
(drift toward the global max is allowed and encouraged; abandoning human-first *or* agent-native
is not).

---

*Amend only through an ADR in `DECISIONS.md`. The loop serves the north star; the knowledge base
serves the loop; the commits serve the browser.*
