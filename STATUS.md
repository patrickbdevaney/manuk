# manuk — STATUS

> **Read this first, every session, before anything else.** State the tier and any blocking items
> out loud before touching code. Do not proceed on assumed context from a previous session.
>
> **This file is GENERATED (`scripts/status-update.sh`), not hand-written.** A status file someone
> writes prose into starts describing what we *meant* to do. Every field below is read from the
> filesystem, git, the crawl output or the verify receipt.

```
TICK:              111
LAST_AUDIT_TICK:   111          (self-audit due every 10 ticks — the hook BLOCKS commits past that)
LAST_SURFACE_AUDIT: 103         (surface audit due every 10 ticks — from docs/loop/SURFACE-AUDIT.md)
LAST_CONSTITUTION_CHECK: 111     (constitution re-read due every 8 ticks — from docs/loop/CONSTITUTION-CHECK.md; anchors the loop to CONSTITUTION.MD)
LOOP_BUDGET:       977 ticks remaining (target tick 1088) — from docs/loop/AUTOLOOP; the loop STOPS and reports at 0
LAST_WALL_AUDIT:   93         (wall-time audit due every 20 ticks — scripts/wall-audit.sh; hunts wall bloat without cutting a gate)
CURRENT_TIER:      0                     (Part 21 — one Tier-0 item left: the SPA miner)
LAST_WALL_TIME:    60s
ORACLE_CORPUS:     265 sites
ORACLE_CRAWLED:    0 (PARTIAL — of 265; this run did not finish, so the hang count is a FLOOR, not a number) sites, 640 clusters  → docs/loop/CLUSTERS.md
ORACLE_HANGS:      0?   ← Bar 0, on OUR clock (manuk_ms > 30s). Outranks every visual cluster.
ORACLE_UNATTRIB:   0   ← oracle process hit its watchdog. Whose time? UNKNOWN — never ours by default.
PENDING_GATES:     G_SPAWN G_POOL_ISOLATION
SINGLE_SITE_TICKS: 0                    (this audit window — a rising count is the drift signal)
UPDATED:           2026-07-15
```


## THE RATCHET — the first principle, above everything below

> **Every tick leaves the browser strictly more capable than it found it, and nothing that worked
> before works less well after. Progress only turns one way.**

Three faces, none optional: **capability**, **performance**, **instrument fidelity**. A tick that buys
one by degrading another is a *trade*, and trades are refused. The unit of progress is a **qualitative
capability step-change** — *"this class of the web now works"* — banked behind a gate that has been
**proven to go red**, which is what makes it a ratchet tooth rather than a hope. (`CLAUDE.MD`)

## THE NORTH STAR — one sentence, and it decides what "done" means

> **Chromium is the CEILING on capability, and the FLOOR on everything else.**

- **Capability — MATCH it.** Whatever a page can do in Chrome, it must be able to do here: the scripts
  run, the layout resolves, the forms submit, the embeds render. This is the only axis where Chromium is
  the *target*, and on this axis we are behind and must catch up.
- **Performance, stability, resource use, honesty of failure — EXCEED it.** On these, Chromium is the
  *baseline to beat*, not the number to converge on. Being faster is not a divergence. There is nothing
  to regress toward.

This resolves the question the oracle's diff can never answer on its own, and it resolves it *in advance*
rather than case by case: **a structural divergence is a bug; a timing divergence in our favour is the
point.** The oracle diffs structure. It has never scored timing, and it must not start.

**The trap, and it is the one this project keeps catching itself in.** A speed advantage is only real if
it comes from doing the same work *better* — dedup, caching, single-flight, deferring what the author
said to defer — and **not from not doing the work at all.** *"Fast because we never loaded the images"*
and *"fast because we never ran the script"* are two lies already told and caught here; `G_FIRST_PAINT`
and `G_DEFER` exist for exactly that reason, because a speed number achieved by **skipping a capability**
is indistinguishable, on the clock, from one achieved by an optimisation.

So the mechanical form: **a speed claim is only admissible next to a coverage number**, and
`scripts/crawl-report.sh` prints coverage FIRST and has no flag to print speed alone. If coverage holds
and we are faster, we are faster. If coverage moved, the speed is a measurement of the thing we stopped
doing.

## A BORROWED ENGINE IS A MEANS, NOT A CONSTRAINT (settled, tick 41)

**When a vendored dependency cannot do something the web requires, the capability wins.** The question is
never *whether* to close the gap — it is *at what cost*, and the options are tried in this order:

| # | Option | Cost | Example |
|---|---|---|---|
| 1 | **Flip a pref** | none | `layout.grid.enabled` — Stylo ships grid off under `servo`; we turn it on |
| 2 | **A named, minimal flag delta** in the vendored source | a diff to re-apply on bump | `parse_has()` — Stylo's *servo* build says `false`, its *gecko* build says `true`. We take Gecko's answer. |
| 3 | **A hand-rolled supplement** for the specific gap | real code, real tests | e.g. matching a selector class the dependency drops, in our own engine |
| 4 | **A hand-rolled module** | large | only when 1–3 cannot reach parity |
| — | **Give up the capability** | ❌ **never** | this is not on the list |

**What the old rule actually meant.** *"Stylo and SpiderMonkey are never patched internally"* was written to
prevent two specific things, and **both remain absolutely in force**:

- **Never copy Blink/Gecko *code*.** Algorithm extraction only, cited by reference. Unchanged.
- **Never fork an engine's *algorithms*** — no rewriting the cascade, no reimplementing the JIT. Unchanged.

It was **not** written to make us permanently worse than Firefox at CSS because a build flag defaults the
other way. Reading it that way turned a guard-rail into a ceiling, and I did read it that way for a tick.

**The discipline that replaces it, for options 2–4:**

1. The delta is **named and minimal** — one flag, not a refactor.
2. It is **justified by the upstream itself** where possible: Gecko already ships `:has()`. We are not
   inventing behaviour, we are declining to inherit a *build* default.
3. It is **guarded by a test that fails if a dependency bump silently reverts it** — otherwise a
   `git checkout` of the vendor directory quietly deletes a capability and nothing says so.
4. It is **recorded here**, so the fork surface is a short, visible list and never a discovery.

**The fork surface (keep this SHORT — a growing list is the signal the rule is being abused):**

| Delta | Why | Guarded by |
|---|---|---|
| *(none yet)* | — | — |

**`:has()` is RE-PRICED, and it is not option 2.** I flipped `parse_has() -> true` in `./stylo/...` and it
changed nothing: the workspace depends on **`stylo = "0.19"` from crates.io**, and `./stylo` is a
*reference checkout that nothing builds*. So the real choices are:

- **Vendor Stylo** (`[patch.crates-io]` → a local fork) for one flag. Big surface, every bump re-applies.
- **Hand-roll a supplement** (option 3): Stylo *discards* `:has()` rules at parse, so we scan the
  stylesheet sources ourselves, parse the `:has()` rules with **our own** selector engine (which already
  backs `querySelectorAll`), and apply them as a second cascade pass. Contained, no fork, and the
  engine we would extend is one we own.

**The supplement is the right call** — it is smaller than a fork and it does not put a permanent tax on
every dependency bump. It is scoped as its own tick, not smuggled into this one.

## THE ORACLE'S SCOPE IS THE CEILING (settled, tick 42)

> **Raising what the instrument can SEE outranks fixing what it already sees.**

The oracle is a **static, single-snapshot, box-diffing** instrument. It is not under-resourced — it is
**structurally blind** to everything that happens *after first paint*:

| Axis | What it has ever observed |
|---|---|
| **time** (animation, timers, polling) | nothing |
| **interaction** (click, hover, focus, keyboard) | nothing |
| **scroll** (lazy-load, virtualization, sticky, infinite) | nothing |
| **session / auth** (logged-in surfaces, app shells) | nothing |
| **media** (playback, codecs) | nothing |
| **adversarial input** (malformed, hostile, edge-case) | nothing |
| **network reality** (HTTP/2, HTTP/3, TLS fingerprint, CORS/CSP) | nothing |

**Null is not zero.** A category with no data is not a category that is fine — it is a category nobody has
looked at. Optimising hard under a fixed ceiling *feels* like progress and is eventually a trap.

**This reframes tick selection:** once the backlog under the current ceiling thins, a tick that **expands
what the oracle can see** outranks a tick that fixes something it already sees.

## THE ASYNC CI LANE — redundant verification, and a real credibility signal

`.github/workflows/ci.yml` runs the **full verify wall on every push**, in parallel, at **zero cost to
iteration speed**: nothing in the local tick loop ever waits on it. A regression it finds is handled at the
next tick's *"read STATUS.md first"* check-in as an ordinary gate failure — **never an interrupt**.

**Two lanes, and the split is a deliberate honesty choice** (per the process-model directive:
*Linux-validated is NOT cross-platform-validated*):

- **`verify-linux` is the BADGE.** The shipping configuration (stylo + spidermonkey) on the primary target;
  it must be **green**, and a green badge means exactly *"this builds and its gate wall passes on Linux."*
- **`cross-platform` + `static-binary` are TRACKED KNOWN-GAPS** (`continue-on-error`). mozjs's cross-OS CI
  build is genuinely unverified, so these are visible, non-blocking trackers rather than a red badge on
  honestly-labelled in-progress work. **When a platform goes green, promote it into the badge lane and drop
  `continue-on-error`** — the same ratchet as everything else.

**Repo uplift:** the README badge is the fastest way an outside visitor confirms *"real, actively
maintained, currently building"* without reading code — and every commit's public full-wall pass makes the
tick-loop discipline something a visitor can **watch happen**, not just a claim in a doc. ⚠ **The badge is a
byproduct of running the lane correctly, never a reason to change what it checks. A red badge from a real
regression is more credible than a green one that has stopped meaning anything.**

## THE STANDING 100-TAB RSS BENCHMARK — the memory claim, proven rather than asserted

Extend the differential oracle to open **N tabs (20 → 100) in both Manuk and real Chromium**, drive to a
realistic idle/backgrounded steady state, and measure **aggregate RSS across all processes.** Track it
alongside the performance floors. **Linux-validated is NOT cross-platform-validated** — record the
per-platform status explicitly and label the gap rather than letting one platform's number stand in for
three. See `docs/loop/PROCESS-MODEL.md` §7.

The claim it exists to defend is **structural, not incremental:** *at 100 tabs the question is how many
tabs have a live process at all* — a correctly hibernated tab costs a few KB of restore token, not a
process. A 100-tab session should look like a handful of live tabs' worth of memory.

## THE SEVEN META-INSTRUMENTS (build in this order)

| # | Instrument | Why it is ranked here |
|---|---|---|
| 1 | **Unhandled-error harvester across the whole corpus** | Cheapest, highest yield, **already proven**: it turned Lit and Svelte from mysteries into named error messages in one move, and the aljazeera wipe peeled one layer per fix this way. Aggregate every throw, rejection and `console.error` across all 265 sites; cluster by message shape; rank by **distinct sites affected**. |
| 2 | **The capability probe, moved into every real corpus page** | Record what each site's own bundles *actually touch* — every global read, every method call. Turns "MDN lists 4,000 APIs" into "**these 180 are what 265 real sites call, and we are missing 12**". A measured, usage-weighted surface instead of an imagined one. |
| 3 | **Accounting reconciliation, as a first-class MECHANISM** | **8 of 30 process defects were caught by a number that did not add up — not by any gate.** That is the single most informative statistic in this project. Every measurement must reconcile: *parsed vs rendered elements · probed vs scored nodes · fetches issued vs performed · sites in corpus vs sites diffed.* Build gates for these over time; keep the manual check running as the backstop, **not** as a replacement. |
| 4 | **`falsify.sh`, generalised from the gate wall to the CORPUS** | It found `G_LOAD` had never tested its own budget, `G1` was structurally incapable of failing, and `G6` scored a browser finding **zero links** as perfect clickability. Extend it outward: **deliberately break the engine and see which sites' divergence scores do not move.** A site whose score is unmoved by a real regression is measuring nothing — and there will be more of those than expected. |
| 5 | **Scope expansion as a SCHEDULED tick, not an inspiration** | A standing audit check: *"which axes has this instrument never observed?"* Track time / interaction / scroll / session / media / adversarial / network explicitly. |
| 6 | **Wire up WPT** | `tests/wpt` and `blitz/wpt/runner` **already exist in the tree and are unused.** Densest signal-per-token available anywhere — written by the people who wrote the specs, naming exactly which behaviour each test checks. **Probably the single highest-leverage tick on the board. Do it early, not last.** |
| 7 | **Web research as a narrow, TRIGGERED disambiguator** | Never a substitute for the oracle, never a reason to seed the repo with the internet's source. Use only when a probe surfaces something the corpus cannot explain: *"what does x.com actually require to render", "which codec does Instagram serve to Chrome"*. **The oracle finds that something is broken; research finds why.** |

## THE PLATFORM MAP (priority order — each unlocks a CATEGORY, not a site)

1. ~~**Loading & viewport awareness**~~ — ✅ **ALREADY BUILT (verified tick 59, gated by `G_VIEWPORT`).**
   A probe written *before* implementing anything found the **entire chain working end-to-end**: the
   viewport moves → `scrollY` updates and `scroll` fires → **`IntersectionObserver` FIRES** → the callback
   sets `img.src` from `data-src` → **and the engine queues that URL for fetching.** *This ledger entry was
   stale, and it was about to send a tick at a feature that already existed — the FOURTH time (after
   `localStorage`, `FormData`, `position: sticky`).* **An absent measurement is not a negative
   measurement.** The original entry read:
   **the single biggest breadth-per-tick item on the board**, because
   it is *one missing primitive, not six missing features*: the browser paints at scroll 0 and **never
   tells anything the viewport moved**. That one gap is why lazy-loading, virtualization, sticky headers,
   scroll-linked animation and infinite scroll are **all simultaneously unsupported**. Build the live
   viewport once; it unlocks all five.
2. **Nested browsing contexts** — isolation is already free (a `PageContext` is per-`Page`; the hard part
   is done). What remains is *plumbing*: live child pages composited rather than rasterized, event routing
   with coordinate translation, `postMessage`, `contentWindow`/`contentDocument`, `sandbox`, nested scroll.
3. **Session, identity, and the network's real behaviour** — **the invisible category**, and the reason
   **41 of 265 sites were discarded and 13 timed out unattributed**. The corpus is systematically biased
   toward *sites that are easy to load* — the exact opposite of the sites that matter. Needs: a real cookie
   jar, a **believable TLS/HTTP fingerprint** (a "correct" browser that fingerprints wrong is a **bot** to
   Cloudflare/Akamai), **HTTP/2 and HTTP/3** (their absence is itself a bot signal, not just a speed
   issue), Service Workers, IndexedDB, real CORS/CSP.
4. **Real-time & background** — WebSocket (today: constructs, honestly reports it cannot connect → make it
   *actually* connect), SSE, Web Workers (today: honest but non-functional), `structuredClone`.
   **WebRTC is explicitly OUT OF SCOPE** rather than left ambiguous.
5. **Graphics** — **Canvas 2D is genuinely within reach**: `tiny_skia` already backs the painter with
   paths, fills and strokes. Today's no-op drawing calls are the honest correct trade *until it lands*.
   WebGL is a real subsystem (wgpu + GLSL translation). OffscreenCanvas/WebGPU are the frontier.
6. **Input & text** — IME/composition (**CJK input is impossible without it**), clipboard, drag & drop,
   `contenteditable` (the substrate of every rich editor), Selection/Range (a stub today), and bidi/complex
   shaping — **swash/fontdb are already doing this and nobody has verified it is correct.**
7. **Correctness under adversarial input** — this is *exactly* what WPT is for (#6 above), not a separate
   effort.
8. **Accessibility** — `engine/a11y` exists and `hit_test` uses it, but whether the **tree itself** is
   correct (roles, names, focus order) is **unmeasured** — and it matters directly for the agent-native
   architecture.
9. **Print, zoom, high-DPI, RTL, i18n text** — same unverified-correctness situation as bidi shaping.

## Settled Decisions — closed questions. Do not relitigate. (Part 29.2)

Re-deriving a decision that was already correctly made is the most expensive kind of drift: it
consumes real reasoning effort and *feels like progress* while producing no new ground truth.

- **Frameworks are debugged by RUNNING them, not by cloning their repos.** Every framework blocker this
  session — React, Svelte, Lit, and the `file://` scheme wall that silenced all of them at once — was
  found by running the thing and **reading what it said**. Four of five app-web blockers were bugs in
  **our own primitives** (an unrooted `*mut JSObject` surviving a GC; missing prototype accessors; a
  missing `CharacterData.data`), not framework internals. **When a framework fails silently, the bug is
  below the framework — which makes the framework's source precisely the place the answer is not.** WPT
  and a richer real-site corpus are the higher-signal investment. Do not seed the repo with framework
  source.

- **Media playback is TICK-SIZED, not a subsystem — and the plan is written** (`docs/loop/MEDIA.md`).
  A video frame **is** a `DecodedImage`; playing a video is swapping the `Rc` in the map the poster already
  occupies and calling `request_redraw`. **No new paint code.** `re_mp4` + `openh264` + `yuvutils-rs`
  lands muted looping `<video>` — *most of the `<video>` elements on the open web* — in ~2–3 days.
  **MSE is genuinely 2–4 weeks** and must come after. ⚠ **Never advertise `MediaSource` before it works:
  its absence is what makes YouTube serve the progressive fallback.**

- **EME/DRM is OUT OF SCOPE. Permanently.** Widevine is a proprietary binary CDM requiring an actual
  licensing relationship. Netflix, Spotify and Disney+ are **unreachable**, and no amount of engineering
  changes that. *Everything else in media* — demux, decode, A/V sync, adaptive bitrate — is achievable and
  is scoped as its own work, honestly labelled. **This is stated once and is not relitigated at each
  audit.**

- **PROCESS-PER-TAB IS DECIDED.** *(Supersedes the tick-42 entry, which was wrong — see below.)*
  In-process **containment of a SpiderMonkey memory-corruption fault is not achievable.** That is not a
  SpiderMonkey defect; it is true of **every production C++ JS engine**. Chromium's own model *assumes V8
  has such bugs* and relies on **OS process boundaries**, not on V8's own safety. Google's in-process
  alternative — the V8 Sandbox — took **3+ years by the team that wrote V8** and is *still not a declared
  security boundary*. In-process containment is the **harder** path, not the shortcut.

  **The decision: one OS process per tab, SpiderMonkey embedded in each** — the architecture Chrome itself
  shipped 2008–2018, before Site Isolation. Scoped, committed work: **process spawning, an IPC layer to a
  coordinating process, and a state-ownership redesign across that boundary.** A definite roadmap
  milestone, **sequenced after the current breadth work.** Full detail: `docs/loop/PROCESS-MODEL.md`.

  **A from-scratch memory-safe JS engine is RULED OUT** as the way around this. It would not even solve it
  (**JIT-generated code safety is a separate problem regardless of implementation language**) and costs
  more than the rest of the browser combined.

  ⚠ **What tick 42's version of this entry got wrong, recorded because the error is instructive:** it said
  per-tab process isolation was *"architecturally impossible in-process"* — which conflates two different
  things and is nearly meaningless. Process isolation **requires processes**; it is not "impossible
  in-process," it is *what you do instead of* in-process. What is impossible is **containing the fault
  without a process boundary.** *A settled decision that is fuzzy in its wording will be read as settled
  in its conclusion.*

- **PER-ORIGIN SITE ISOLATION IS REJECTED — not deferred.** Extending process-per-*tab* to process-per-
  *origin* is architecturally straightforward (same mechanism, finer grain). We are not doing it. **Chromium's
  own documentation names Site Isolation as the primary reason Chrome uses more memory than Firefox and
  Safari**, and Chromium's security team has said they are hitting the limits of what more process
  granularity buys, because **processes are not cheap.** That is Chromium — with vastly more budget for
  process overhead than this project has — naming this as a real cost. **Our stated goal is to be leaner
  than Chromium; adopting Chromium's own named bloat driver works directly against it.**
  **Accepted trade-off, stated plainly:** a compromised cross-origin iframe shares its tab's process. That
  is **the same trade-off Chrome itself accepted for a decade.**

- **Bar 2 (pixel precision) is deferred.** Breadth beats depth until Bar 1 is real. Pixel-exact on one
  site and broken on a thousand others is not what "usable" means.
- **Bar 0 (no crash, no hang, no unrecoverable panic) is the FLOOR**, checked before Bar 1 is even
  asked, for any pattern class. (Part 23.)
- **Never copy Blink/Gecko CODE, and never fork an engine's ALGORITHMS.** Both absolute. But a *build
  flag* that leaves us behind Firefox is not an algorithm — see "A BORROWED ENGINE IS A MEANS, NOT A
  CONSTRAINT" above. The fork surface is a named, short, guarded list.
- **No Blink/Gecko code is copied, ever, under any framing.** Algorithm extraction only, cited by
  reference. This one stays discipline, not a hook: a script cannot tell "extraction" from "close
  paraphrase" (Part 28.4), and pretending it could would be worse than naming it.
- **The oracle's cluster ranking IS the priority ledger** (`docs/loop/CLUSTERS.md`) — not a suggestion
  judgment may override outside of tie-breaking.
- **Crashes and hangs outrank every visual divergence** in that ledger (Part 24.3).
- **SpiderMonkey is settled** (Part 30). The "V8 is more capable" intuition does not survive contact
  with the evidence — sites broken on Firefox are overwhelmingly browser-sniffing, not conformance.
  And the capability bar (Chromium parity) rules out the lean/embedded tier — QuickJS, Hermes,
  JerryScript — **entirely**, not just against V8. Leanness that costs capability is not a trade this
  project can make. Do not reopen without evidence the BAR changed.
- **Chromium is the CEILING on capability, the FLOOR on everything else** — see THE NORTH STAR above.
  Match its capability; beat it on speed, stability and resource use. A timing divergence in our favour is
  not a bug to close. Do not reopen this.

- **The app web is ADDITIVE substrate, not a scheduling subsystem** (measured, tick 20: 0/8 → 3/8
  frameworks rendering from ~6 IDL fixes). This was the open question the whole schedule hung on.

## Lessons — promoted out of the journal because they recurred (Part 29.1)

Short by construction. If this grows without bound, lessons are being added that should have become
**gates** instead — which is always the better outcome, and is where most of these ended up.

1. **A gate that does not measure what the user feels reports green while the user suffers.** Every
   gate here was born this way: G_ALLOC (the wheel-event clone freeze), G_LOAD (the frozen tab),
   G_INTERACT, G_CONTAIN (the apple.com core dump). Before adding a feature, name the gate that would
   have gone red if it were already broken. If you cannot, add it first.
2. **The symptom names the wrong organ.** rust-lang.org's columns *looked* stacked — they were in a
   perfect row, overflowing off-screen. The oracle's font-width divergence *looked* like a metrics
   bug — `font-family` was never mapped at all. Measure the boxes before theorising from a screenshot.
3. **When every instrument says a bug is impossible, they are all sampling the same layer.** The reddit
   grey was in no display item, no decoded image, no rect — because it was a *letter*, rasterised from
   an unscaled outline at `font-size: 0`. Bisect the layer below; do not reason harder about the one
   you are in.
4. **An oracle must never be able to charge its own slowness to your account.** One wall-clock budget
   per site cannot tell "we hung" from "Chromium hung", and will confidently tell you the wrong one.
   Time each engine separately, always.

   **This one has now fired three times, and the third time it had produced the project's top-line
   metric.** "73/265 sites hang" was the oracle *process* hitting a watchdog that wraps Chromium — the
   slower engine on most news sites. The general form, which is the version to keep:

   > **Every number has a harness, and the harness is part of the number.** Before believing a metric
   > moved, ask what else moved. I widened the crawl from 4 jobs to 12 to make it finish sooner and
   > watched "the hang rate" go from 12.5% to 49% on the same binary in the same hour.

   It is a lesson I could recite while breaking it, which means it was decoration. It is four mechanisms
   now: `TIMEOUT` is attributed to nobody · Bar 0 counts `manuk_ms` · the crawl warns at a non-baseline
   job count · `status-update.sh` refuses to print a partial crawl as a number.
5. **A fix for one instance is not containment of a class.** Fixing apple.com's panic was necessary and
   was not Bar 0. You will not prevent every crash-class bug before Bar 1 — the tail of uncovered
   patterns is where the panics live, and it is infinite.

## Tier 0 — nothing in the backlog starts until these are done or genuinely blocked

| # | Item | Status | The fact |
|---|------|--------|----------|
| 1 | Verify wall under 5 minutes | ✅ **MET** | **181s (3m 01s)** worst realistic tick (touch `engine/css`, the shared-type edit that cascades furthest); **57s** warm. Measured by timing `./scripts/verify.sh`. mold/nextest/workspace-hack **not needed** — the target is already met, and doing that work anyway would be infrastructure theatre. Re-measure if it ever crosses 300s. |
| 2 | Oracle crawl frame at 200–500 sites | ✅ **DONE** | **265 sites, 15 design-pattern classes** (`docs/bench/oracle-corpus.txt`). Process-isolated, watchdogged, resumable, snapshot-cached, **run-stamped**. Latest clean run: 206 diffed, 44 discarded, 15 unattributed timeouts. |
| 3 | Ten SPA starter apps through the Framework Exception Miner | ✅ **DONE (tick 26)** | **8 of 8 frameworks mount and render** — React (TS+JS), Vue, Svelte, Solid, Preact, Lit, Vanilla. Every blocker was one of *our* primitives, not the framework: a use-after-GC in `ownerDocument`, `file://` unsupported by the net layer, `CharacterData.data` missing, a shadow root typed 8 instead of 11, and no accessors on `Node.prototype`. All six are now asserted in **G2 scenario 14**. |

**TIER 0 IS COMPLETE.** The definition of a productive session is no longer "Tier 0 advanced". The
schedule is now set by the ledger and by Bar 0's residue, which is **two sites** (wix.com, flickr.com).

**Definition of a productive session while TIER 0 is open: Tier 0 advanced.** Not bugs closed, not
features shipped. If I find myself reaching for a bug fix because it feels like more visible progress
than widening the oracle, that is the exact failure mode Part 21 exists to prevent, and I say so out
loud rather than quietly following the pull.

## Bar 0 — the stability floor (Part 23). Checked BEFORE Bar 1 is even asked.

| Requirement | Status |
|---|---|
| No unrecoverable Rust panic at process level | ✅ **contained** — `panic = "unwind"` + a supervised per-navigation boundary. A panic in parse/cascade/layout/paint kills the PAGE and shows an error; the browser and every other tab carry on. Proven by `G_CONTAIN`, which deliberately panics a build. |
| No SpiderMonkey crash cascade | ⚠️ **partial** — caught at the six binding sites; a fault raised inside SpiderMonkey's own C++ frames still cannot be caught in-process (unwinding across that FFI edge is UB). Full containment needs a per-tab process, correctly deferred. Stated honestly rather than claimed. |
| No unrecoverable hang | ✅ **9 of 206 sites exceed 30s on our clock (4.4%)** — and Chromium is slower still on 7 of those 9. We are **faster than Chromium on 175/206 (84%)**; median render 21.7s vs its 35.7s. The old "73/265 hang (27.5%)" was the oracle *process* hitting a watchdog that wraps Chromium too. **Remaining: two sites** (wix.com 39.1s vs 22.4s; flickr.com 31.1s vs 14.8s). Production interruptibility (a cancellable long task) is still not built. |

## Gates — standing up vs. pending

| Gate | Status | What it catches |
|------|--------|-----------------|
| build · parity 72/72 · G1 · G2 · G3 · G6 | ✅ in the wall | rendering + JS + affordances + clickability |
| G_ALLOC | ✅ in the wall | per-input-event allocation rate |
| G_TEARDOWN | ✅ in the wall | an exit path that skips the profile flush |
| G_LOAD | ✅ in the wall | a dead subresource holding the document hostage |
| G_INTERACT | ✅ in the wall | UI-thread stall on tab open/switch/close |
| F1 / F2 perf floors | ✅ in the wall | cascade ≤40ms, pipeline ≤125ms (asserted, not eyeballed) |
| **G_SILENT_FAIL** | ✅ **live** | an error on the load/render/script path that is swallowed. Named by the failure that cost several ticks: "React mounts, throws nothing, renders nothing" was React throwing *truthfully* inside an async render, with nothing listening. |
| **G_DEDUP** | ✅ **live** | the same URL on the **wire** twice for one navigation (nytimes was pulling one sprite down once per element that named it) |
| **G_HANG** | ✅ **live, and now honest** | Every oracle site runs in its own process under a watchdog. The watchdog is a **backstop against a true infinite loop** — it wraps our render *and Chromium's*, so when it fires it is recorded as `TIMEOUT` and **attributed to nobody**. The Bar 0 hang count comes from `manuk_ms`. A metric that cannot say whose time it measured must not name a culprit. |
| **G_CONTAIN** | ✅ **live** | Bar 0 — a panic kills the page, not the process (Part 23.2) |
| **G_REFLECT** | ✅ **live** | **HTML attribute reflection** — the largest single gap in the platform. `a.href`, `input.disabled`, `img.width`, `td.colSpan` were **all `undefined`**. ~38,000 WPT subtests behind one generic mechanism; `html/dom` **21.0% → 37.7% (+9,940)**. |
| **G_MUTATION** | ✅ **live** | `MutationObserver` was an **inert stub** that reported `function` — it observed nothing, forever. Real records now (attributes/childList, `oldValue`, `attributeFilter`, `subtree`, `disconnect`), delivered on a **microtask** so 100 appends batch into one callback. Whole `dom/` **+44 → 33.8%**. |
| **G_ATTRS** | ✅ **live** | `element.attributes` was **`undefined`** — not incomplete, absent. `.length` was a `TypeError`, so **DOMPurify could not enumerate attributes to strip `on*` handlers.** Live `NamedNodeMap` (a frozen length spins forever), `Attr` as a write-through **handle**, `createAttribute`, `toggleAttribute`. Whole `dom/` **+49 → 33.1%**. |
| **G_NAMES** | ✅ **live** | `classList` is a real **`DOMTokenList`** (indexed, and it **throws** on whitespace/empty tokens — `add('btn primary')` used to silently write one class matching neither selector). `createElement` validates its name. `createElementNS` keeps the **namespace**, so SVG's `linearGradient` is no longer uppercased into nothing. Whole `dom/` **+149 subtests → 32.3%**. |
| **G_EVENT_SURFACE** | ✅ **live** | `{once:true}` fired **forever** — the options object was read as a bare boolean, so `once` was dropped. `returnValue`/`cancelBubble` were `undefined`. `document.createEvent` did not exist. Every failure **silent**. Whole `dom/` **+118 subtests** — the largest single-tick move so far, crossing **30%**. |
| **G_COLLECTIONS** | ✅ **live** | `children` / `getElementsByTagName()` are **live** — they were snapshots. Not merely non-conformant: **a Bar 0 hang.** `while (el.children.length) el.removeChild(el.firstChild)` never terminates against a frozen `length`, and the tab locks up. Whole `dom/` **+17 subtests**. |
| **G_TRAVERSAL** | ✅ **live** | `NodeIterator` + `TreeWalker` with the spec's **filter protocol**. `FILTER_REJECT` prunes the subtree; `FILTER_SKIP` does not — a **security** property wearing a traversal property's clothes, since DOMPurify rejects `<script>` and must not be walked into it. `dom/traversal` 11/53 → **34/53**; whole `dom/` **+27 subtests**. |
| **G_RANGE** | ✅ **live** | A **real `Range`** — not the inert stub that sat in the interface list making `typeof Range === 'function'` true for sixty ticks. Boundary-point comparison, extract/clone/delete **across structure** (the partially-contained ends are *split*, which is the whole difficulty), `insertNode`, `surroundContents`. `dom/ranges` 2/200 → 16/200; whole `dom/` suite **+29 subtests**. |
| **G_DISPLAY_CONTENTS** | ✅ **live** | A `display: contents` wrapper **generates no box while its children still do** — they become the grandparent's grid/flex items. It was never parsed: `contents` fell through to `inline`, so the wrapper stayed a real box and the grid saw one anonymous item instead of three. The layout collapsed into a single cell with everything present, styled, and in the wrong place. |
| **G_TRANSFORM** | ✅ **live** | `getComputedStyle(el).transform` resolves to the spec's `matrix(a,b,c,d,e,f)`. The transform was always *applied* (the box moves); it just never reached JS — and `undefined + ' scale(2)'` is the **string** `"undefined scale(2)"`, which is how every animation library on the web silently stops animating. |
| **G_SCROLL** | ✅ **live** | `element.scrollTop` is **real** — truthful `scrollHeight`/`clientHeight`, clamped writes, survives re-layout, **moves the actual pixels**, fires `scroll`. It did not merely not work: `scrollHeight` was aliased to the element's own border box, so **`scrollHeight - clientHeight` was always 0** — the exact number every virtualised list divides by. |
| **G_CANVAS** | ✅ **live** | `<canvas>` 2D **rasterizes** — fills, strokes, paths, transforms, real `getImageData`/`toDataURL`, on tiny-skia. And the pixels **reach the page**: a canvas is composited as an image the page drew into. It was a stub that accepted every call and drew nothing, which is the worst shape a failure takes — the page is told YES and renders blank. |
| **G_CAPABILITY** | ✅ **live** | **The pattern ledger, as executable assertions.** 42 of its claims now run on every wall; a ✅ that stops being true stops the tick. Built because the ledger — the file that decides what gets built next — was wrong **six times**, always a ❌ nobody measured. Its top *three* priorities were all phantoms. |
| **G_PROTOTYPE** | ✅ **live** | DOM methods live on **prototypes**, not on every element. Patching `Element.prototype.setAttribute` now actually takes effect — it used to be a **silent no-op**, which is how every error tracker, ad-blocker and polyfill hooks the DOM. Also: `createElement` ×5,000 went **124ms → 2ms**, and own-properties per element **116 → 1**. |
| **G_CLEAN_EXIT** | ✅ **live** | Bar 0 — a process that ran JavaScript **exits 0**, without being told to shut down. Closes the exit-segfault residual this project carried for 60 ticks: SpiderMonkey needs `JS_ShutDown()` before the process ends, and the only thing providing it was every caller *remembering* to. Half of them did. |
| **G_RUNTIME_COUNT** | ✅ **live** | one async runtime for the process, not one per action (Part 25.2). The shell was building **two**. |
| **G_SPAWN / G_POOL_ISOLATION** | ⏹ **retired, with a reason** | G_SPAWN is subsumed by G_RUNTIME_COUNT; G_POOL_ISOLATION guards a rayon pool that **does not exist**. A gate on absent machinery passes forever and is counted as coverage — which is the definition of vacuous. Saying so beats building theatre to make an audit green. |
| **FALSIFY** | ✅ **live** | **a gate that cannot go red.** `scripts/falsify.sh` mutation-tests the wall against itself. On its first run it found `G_LOAD` — a *Bar 0* gate — had **never tested the thing it was named for**. |

## Enforcement — compliance is mechanical, not remembered

| Mechanism | Status | How it works |
|-----------|--------|--------------|
| Gate receipt | ✅ live | `verify.sh` writes `.git/manuk-verify-receipt` naming the **exact tree** it verified. `scripts/hooks/pre-commit` recomputes that name from what is being staged and **refuses the commit if they differ**. Verifying one version of the diff and committing another is now impossible, not merely discouraged. |
| Journal enforcement | ✅ live | The same hook refuses any commit unless `docs/loop/JOURNAL.md` has a `## Tick <N>` entry for the `TICK:` in this file. The entry is written at the *start* of a tick, so it states a hypothesis rather than narrating a success. |
| Session-start read | ✅ live | `CLAUDE.md` makes reading this file the literal first action of every session. |
| Self-audit every 10 ticks | ✅ live | `scripts/self-audit.sh` diffs what the methodology prescribes against what actually exists, and fails loudly on anything prescribed-but-not-executed. Due at tick 20. |

## Last 5 journal entries

- **Tick 15** — the invisible-content class: `font-size:0` painted glyph-shaped continents (swash rasterizes the *unscaled* outline at 0px); anonymous boxes stranded in stacking layer 0, burying text under its own ancestor's background; every insetless `position:absolute` element silently deleted (github coverage 91.4→97.8%); backgrounds stretched to their element. New gate G_INTERACT.
- **Tick 14** — the oracle pays for itself: `font-family` was never mapped from the cascade *at all*; the network layer had no timeout of any kind (w3schools 37.8s→15.0s); flex items could never shrink; a percentage width on a flex item resolved **twice** (used width came out squared); every responsive image rendered stretched.
- **Tick 13** — headless screenshot discipline; flex items with block children.
- **Tick 12** — in-process automation surface (selectors/wait/assert).
- **Tick 11** — file uploads.

## THE NUMBER THAT MATTERS RIGHT NOW

**Clean 265-site crawl, tick-36 binary, one RUN_ID, our own clock.** (`scripts/crawl-report.sh`)

```
BAR 1 — is the node THERE?          92.2%   (162,570 of 176,311 probed)
        ...and `display` agrees     73.0%   ← the next real gap
BAR 2 — geometry (DEFERRED)        123,796  the node exists, SAME SIZE, moved. Not a failure.

BAR 0 — over 30s on our clock       4/211   (1.9%)   was 4.4%
FASTER than Chromium              195/211   (92%)    was 84%
median render        ours 16.1s  ·  Chromium 36.5s
p90                  ours 24.7s  ·  Chromium 99.5s
```

**We are slower than Chromium on exactly one site** (atlassian.com, 34.6s vs 32.2s). Median **2.3×
faster**. That is the north star, measured: *capability approached, performance exceeded.*

**The next Bar 1 target is the 27% `display` disagreement** — 33,825 nodes where we render the node but
Chrome and we disagree about whether it is *shown*. `display: none` vs shown is the likely bulk of it, and
unlike geometry it is a **real** rendering difference: a node we hide that Chrome shows is content the
user cannot see.

> **This report lied on its first run.** It lumped `geometry` into "coverage" and announced **2.8%** for a
> browser that renders fine. The instrument built to stop me trusting bad numbers produced one immediately
> — the fourth time an instrument has done that here (see `docs/loop/PROCESS.md`). None of them get to be
> trusted on sight.

## Corpus (18 sites — the OLD frame, kept for per-site fidelity scores)

```
MEAN COVERAGE  99.0%   (Bar 1 — of what Chrome renders, what do we render at all)
MEAN VISUAL    81.1%   (Bar 2 — DEFERRED, per Part 21.2 item 5; do not micro-tune this)
```

Bar 2 stays deferred. A browser that is pixel-exact on one site and broken on a thousand others is
not what "usable" means here.
