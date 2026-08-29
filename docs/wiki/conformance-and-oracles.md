# CONFORMANCE AND ORACLES — how we MEASURE, and what each instrument cannot see

## The differential oracle (265 real sites vs Chromium) has two structural blind spots

1. **It can only see what those sites happen to exercise.** A DOM method no site in the corpus calls
   is, to the oracle, **correct by default**.
2. **It needs Chromium to say what "right" is** — so every answer is a *diff*, and a diff cannot tell
   you whether **both** engines are wrong, or whether we are wrong in a way that happens not to move a
   box.

It is also **static and single-snapshot**: it has never observed time, interaction, scroll, session,
media, adversarial input, or network reality. **Null is not zero** — a category with no data is a
category nobody has looked at.

## WPT has neither blind spot, because the tests carry their own verdict

`assert_equals` either holds or it does not. **No oracle is required at all.** It is the difference
between *"we render this page differently from Chrome"* and *"`Node.prototype.after()` is specified to
do X and we do Y."*

### Integration mechanics (these are the sanctioned hooks, not workarounds)

- **`resources/testharnessreport.js` is the vendor hook.** WPT's own copy says so in its header:
  *"intended for vendors to implement code needed to integrate testharness.js tests with their own test
  systems."* We serve our own in its place; it registers `add_completion_callback` and writes results
  into the DOM as JSON, which the Rust side reads back with `querySelector`. Overriding it **in the
  server** rather than by writing into the checkout keeps the corpus pristine — *a runner that mutates
  its own corpus is a runner whose corpus you cannot trust.*
- **`setup({ output: false })` is required.** testharness's HTML results renderer is *page code*; any
  DOM gap it trips over throws **inside `notify_complete`**, aborting the completion-callback loop, so
  the file reports **nothing**. wptrunner passes `output: false` for the same reason.
- **Serve over real HTTP, never `file://`.** `file://` is an **opaque origin**, so every storage/origin
  test would fail *because of the harness* and be recorded as an engine failure. This project has
  already been burned by exactly that: a `file://` harness bug left *"React renders nothing"* in the
  ledger for ticks as a **React** problem.
- **`.any.js`/`.window.js` need wptserve to generate their wrappers** (~2.5% of tests). Skipped —
  **and counted, with the reason printed.** *A runner that silently drops what it cannot run is
  reporting a pass rate for a suite it did not run.*

### A hang can only be contained by a PROCESS boundary

`tokio::time::timeout` **cannot interrupt synchronous JavaScript**. A test that spins inside
SpiderMonkey never yields, so the timeout future never runs and the whole suite wedges. The runner
therefore forks a **child process per batch**; the child appends one flushed JSON line per finished
test, so when the driver kills a stalled child, **the test after the last flushed line is the one that
hung** — named, recorded, and stepped over.

> This is the same conclusion the tab process model reached (`docs/loop/PROCESS-MODEL.md`), arrived at
> independently and for the same reason: **only an OS process boundary contains a spinning C++ JIT
> frame.**

## A SUITE HAS A RULER, AND INSTALLING IT ON ONE LEG IS WORSE THAN NOT INSTALLING IT AT ALL

WPT does not test prose. It lays text out in **Ahem** — a face whose every glyph is exactly
`1em × 1em`, `0.8em` ascent and `0.2em` descent — so a layout expectation becomes an integer the
test can assert: `font: 25px/1 Ahem` on `XX` is **50 × 25**, and nothing else. WPT's own runner
requirement is that the face be **installed on the host**; the `<link href="/fonts/ahem.css">` every
such test carries is served by `wpt.py` out of `wpt/fonts/`, a directory **this checkout does not
have**.

`reftest::install_ahem` has registered the face into `fontdb` since t1088. `harness::run_one` — the
**other** leg, and the one the project's primary metric (the monotonic WPT subtest total) is read
through — did not, for a further 95 ticks:

```text
   3,804 files under css/ link /fonts/ahem.css
     1,637 css/CSS2    844 css/css-text    835 css/css-grid   <- the board's #1 lever
```

**The gap was invisible because the dependency is stated in the markup, not in `meta flags`, and
because the failure it produces is shaped exactly like an engine defect.**
`css/css-grid/abspos/positioned-grid-descendants-*` is 32 files, 3,200 subtests, a flat zero, and
every one of them opens `width expected 50 but got 0`. Nothing in that message says *ruler*. It took
reading the fixture's CSS — `font: 25px/1 Ahem` — to see that the 50 was two em boxes.

Installing it on the testharness leg, measured against the same-day t1182 sweep, one binary rebuilt
per arm in the same hour:

```text
   css/css-grid      1765 -> 2018   +253
   css/css-text      1603 -> 1708   +105
   css/css-sizing     792 ->  853    +61
   css/css-flexbox   1450 -> 1469    +19
   css/css-ui         241 ->  242     +1
   nine other areas   unmoved to the subtest
```

**Two rules, and the second is the one that cost the extra hour.**

1. **The ruler goes at the LIBRARY entry point, not in the driver.** `install_ahem` is called from
   `run_one`, so the gate (`a_testharness_run_lays_text_out_in_the_ahem_face_the_suite_measures_with`)
   exercises the real call site and deleting the call turns it RED. Put it in `main.rs` and the gate
   can only test the function, never the wiring — which is precisely the shape of the defect it is
   guarding.
2. **INSTALLING A RULER TURNS ACCIDENTAL PASSES INTO HONEST FAILS, and one of them was a real
   defect.** `css/css-fonts` went **−3**, all three in `matching/font-unicode-PUA.html`, which
   asserts that `font-family: serif, sans-serif, …, 'Ahem'` and `font-family: 'Ahem'` render
   **U+F000–F002** to the same width — css-fonts-4's rule that a Private-Use-Area codepoint must
   match only **non-generic** families. With no Ahem installed *both* arms fell back to serif and
   agreed at `21.484375`; with Ahem installed the second arm is `22` and the first still
   `21.484375`, because the engine matched `serif` first. **Two errors cancelled and read as
   agreement.** The −3 is the ruler exposing the defect, not causing it; the fix is in the same
   tick.

> **The general rule: when an instrument has two legs, a fix applied to one of them is a
> DIVERGENCE, not a partial fix** — and the leg you are not debugging is the one that keeps
> reporting.

**What the subset ruler did NOT cost, measured rather than assumed.** The face installed is
`engine/text/tests/fixtures/Ahem.woff2`, a **245-codepoint** subset of upstream's 278. Swapping in
the full `wpt/fonts/Ahem.ttf` was built and run across all fourteen areas above: **every number
identical, to the subtest.** It was reverted — it bought nothing measurable and would have been an
unmeasured change to the *reftest* leg, which shares the constant. The hole is real and currently
inert.

## Guard every instrument against measuring ITSELF

The runner prints a warning when >25% of files report nothing:

> *"Above ~25% this number is not measuring the engine's conformance — it is measuring whether
> testharness.js can RUN here at all."*

**It fired on the very first run (100%), and it was right.** Without that guard the honest reading of
"0%" would have been "our DOM is catastrophically broken" instead of "we never defined
`window.parent`".

**Corollary, learned three times now:** a verdict from a new instrument is a **claim**, and claims get
verified before they are believed. The first `cold-read.sh` run reported "tick 42 has no hypothesis"
about a journal entry that plainly had one — `awk` has no `\b` word-boundary escape, so the pattern
matched nothing. **The auditor was wrong, not the file.**

## THREE DIFFERENT FINDINGS MUST NEVER SHARE A NAME

The WPT runner called all of these `TIMEOUT`:

- **our** budget expiring (a *perf* finding),
- **testharness's** own status-2 verdict — an `async_test` that never completed (a *conformance* finding),
- a driver-killed **hang** (a *Bar 0* finding).

So a baseline reported **"90 Bar 0 hangs"** when the real number was **one**. The engine was fine; the
*word* was overloaded. Four columns now: `HANG`, `CRASH`, `SLOW`, `TH_TIMEOUT`.

> **The general rule: an instrument that collapses distinct findings into one label is not a coarse
> instrument — it is a WRONG one**, because the label is what gets acted on.

## A runner must account for the child that DIED, not just the one that hung

When a batch child *crashed* (rather than hanging), the driver advanced past the whole batch — **33 of
457 files silently vanished**, and the pass rate was computed over the remainder with nothing to say so.
Fixing it made **5 real crashes visible** that had been invisible from the start.

**A crash is a finding, not an accident.** Both a hang and a crash must name the test they died on and
step over it.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## ONE SNAPSHOT, BOTH ENGINES — and never diff against a DEGRADED oracle

**Fetching the page separately per engine compares two DIFFERENT documents and calls the difference a
bug.** The live origin injects a fundraising banner that a saved copy does not — and a metric stayed pinned
at **exactly 5,122px across four genuinely-correct fixes** because of it.

Equally: **the oracle's health check must ask what Chromium actually DREW** (elements drawn, visible text
length), **not how many elements carried an id** — otherwise a **bot wall is scored as YOUR rendering
bug.**

**Both hazards are made impossible IN CODE, rather than left as things to remember.**

## Probing only `[id]` elements makes the oracle nearly BLIND

**Most of the web does not put ids on things.** `text.npr.org` reported **ONE** probed element — and across
a 265-site corpus the oracle was about to report *"no divergences"* **with total confidence.**

Keying on a **structural path** (`div[0]/main[0]/p[3]`), computed identically by both engines from the same
snapshot, took npr **1 → 75** probed elements, lite.cnn → **226**, the Rust book → **540**.

⚠ **Getting the two path functions to agree is a precondition for the diff meaning anything.** **Chromium's
walk stops at `parentElement === null`, so `<html>` contributes NO component** — emitting `html[0]` shifts
every key by one level and reports **`<html>` and `<body>` as MISSING on every site.**

## THE SCORE GATES; THE EYEBALL DIAGNOSES — a pixel score is a poor proxy for correctness

Recorded **four separate times in one arc**:

- Wikipedia scored **75%** while being **visibly, structurally broken**.
- A massive structural repair (hidden dropdowns no longer painting over the article) moved it **81.0% →
  81.7%**.
- Restoring an **entire missing TOC sidebar** moved it **81.7% → 80.7%** — *it went DOWN.*
- **An entirely absent sidebar moved the visual score by less than ONE POINT.**

**The honest metric is COVERAGE**: *of everything Chromium renders, what fraction do we render **at all**?*
**A missing region cannot hide in COVERAGE.** Placement drift is reported **separately**, because on real
pages it is dominated by **font-metric differences** — a fidelity concern, not a correctness one.

**Corollary:** a coverage number can be **100% while the page is wrong** — coverage said every element was
present on rust-lang.org while the page was **printing its own JavaScript source down the left margin.**
*That is what a second bar is for.*

## Cluster by ROOT CAUSE, not by tag name — and rank by DISTINCT SITES

Naive differential diffing runs at **90%+ false positives** before clustering. A clusters file whose top
entries are `geometry: <div>` / `<a>` / `<body>` is **a ranking by tag name** — a restatement of *"the
oracle found divergences"* — and **cannot be worked on.**

Cluster by: **(1)** first-divergence signature, **(2)** computed-`display`-mismatch class, **(3)** the CSS
property/selector implicated. **Then rank by how many DISTINCT SITES each cluster explains** — one site with
500 `<div>`s must not outvote 200 sites with one `<iframe>`.

> **A cluster IS a website class, so the cluster registry IS the taxonomy** — empirically derived rather
> than hand-enumerated. **Crashes and hangs are a third category and outrank every visual cluster.**

**And never score timing.** A first-pass report that lumped all divergence kinds together printed
*"structural agreement: 2.8%"* for a browser rendering fine — because **`geometry` (123,796 nodes, 70% of
the total) means the node EXISTS, at the same SIZE, in a different place.** The real Bar 1 number was
**92.2%**.

## Score geometry PARENT-RELATIVE (SHAPE), never against the document origin (tick 335)

The same amplification bites a second way, and it is subtler than tag-name ranking. The first
category-stratified sweep read `PLACE(ok) 4.5%` against a ≥75% bar and looked like a Presto-style tail of
hundreds of independent layout bugs. It was **one bug counted N times.** microsoft.com had a **23px median
dy with zero elements in tolerance** — a *tight* spread *around* 23px, i.e. nearly every element off by the
**same** amount. One ancestor placed 23px too low drags its entire subtree 23px, and absolute-position
diffing then charges each descendant with its own `geometry` divergence. **A page shifted 23px is not
jarring to a user; it scored 0%.** The metric measured *absolute document position* when Phase 0's bar is
*"a user does not notice they left Chromium."*

The fix is to score each element's box **relative to the nearest ancestor present in both engines**
(`oracle::common_frame`), not to the page origin: `rel = (child.x − frame.x, child.y − frame.y, w, h)`. A
purely inherited translation **cancels** — both engines measure the child against the *same* frame, so a
constant offset in that frame drops out of the difference. Only the element where the offset **originates**
has a wrong shape and is reported; the whole subtree below it collapses from N divergences to zero. The
element keys already encode this for free: they are `tag[i]/tag[i]/…` paths from the root, so an ancestor's
key is a prefix of its descendants' — walking up is `rfind('/')`, and the "present in both maps" test is a
plain `HashMap::get`. Width and height are translation-invariant, so they stay absolute. **RED proof:**
revert `diff_page` to `m.rect[i] − c.rect[i]` and the synthetic "uniform 23px shift + one genuinely
misshapen box" fixture reports the two pure inheritors as bugs again
(`oracle::tests::shape_scoring_suppresses_inherited_offset_keeps_real_bug`).

> **Absolute-position parity is NEVER the gate — it is the layer that produced the misleading 4.5%.** SHAPE
> is the Layer-1 gate; the jarring invariants (overlap / clipping / reading-order / hittability /
> post-load-stability) are Layer 2; pixel diffing stays a diagnostic on a small corpus only.
> (`docs/loop/FIDELITY-SCORING-REDESIGN.md`.)

**The same SHAPE metric now lives in the fidelity probe, not only the oracle (tick 531).** The oracle
(`oracle.rs`, the 265-site differential crawl) grew SHAPE at tick 335; the **G1 fidelity gate / `fidelity-sweep.sh`
probe** (`fidelity.rs`) — the code the redesign explicitly names as the *Phase-0 EXIT instrument* — still scored
`placement_stats` (absolute document position), the very metric that produced the misleading `PLACE(ok) 4.5%`.
`fidelity::shape_stats` ports the primitive across with **byte-identical semantics**: nearest-ancestor-present-in-both
via `rfind('/')`, x/y subtracted against that shared frame, w/h left absolute, one definition of SHAPE across the
whole instrument (no divergent second implementation). It is landed as a **tested primitive with a RED-provable gate**
(`fidelity::shape_tests`: a uniform-offset fixture scores it once at the origin, a genuinely misshapen leaf still
fails, coverage misses are not SHAPE misses; reverting the parent-subtraction to absolute flips the offset test red)
**Tick 532 built the enabling brick and WIRED SHAPE in: the selector-path producer** (redesign §3a) on both
engine sides. The G1 producer previously emitted `[id]` keys (no `/`-ancestry), against which `shape_stats`
silently degraded to absolute and *lied*; 39% of the corpus has too few ids to probe at all. Now the Chrome
side (`chrome::capture_boxes_all_paths` + `PROBE_ALL_PATHS_JS`) and the Manuk side both key every rendered
element by `tag.SIG:nth-child(n)/…` from the root. **One definition of the key:** the Manuk `sig_of`/`path_of`
were extracted from the oracle's local closures into shared free functions used by BOTH the oracle and the G1
probe, and the JS `fnv`/`sigOf`/`pathOf` is a byte-identical contract (UTF-16 `charCodeAt` hash, `<html>`
excluded because its parent is not an element — emitting a root term once shifted every key a level and
reported `<html>` MISSING everywhere). `shape_stats` now replaces the absolute `placement_stats` as the G1
report's Layer-1 number (`Fidelity.shape`, `MEAN SHAPE`); absolute placement stays only as a Layer-3 `[diag]`.
RED-provable producer gate (`path_key_tests`): `join("/")`→`join("_")` fails the `/`-ancestry assertion, and
`sig_of` is cross-checked against an independent fnv1a-32 reference. **Decompose-first boundary kept:** the
gate FLOOR still gates on structural COVERAGE — flipping it to SHAPE awaits a broad path-keyed sweep to
recalibrate the 0.75 bar honestly (the number is claimed later).

**Tick 533 started Layer-2 (the jarring invariants — §2's "actual Phase-0 bar") in the G1 probe: horizontal
overflow first**, because it is rect-only and needs no producer change. `oracle::jarring_h_overflow` (on `Seen`
maps) was refactored to delegate to a new generic `oracle::h_overflow_boxes<K>` on `HashMap<K,[i64;4]>`, so the
oracle and the G1 probe (Box4 keys) score overflow through ONE definition — the same discipline SHAPE uses. G1
now prints `H-OVERFLOW: N escape the viewport` after SHAPE. RED-proven: dropping the `edge(c) <= vw+tol` guard
flips both the Box4 and the delegated `Seen` test 1→2 (the "blame only OUR spill" guard is genuinely tested).
**Tick 536 added the second Layer-2 invariant — collapsed interactive target** (`oracle::collapsed_target_boxes`),
the box-dump half of hittability: a control an interactive tag names, that Chrome renders clickable but Manuk
collapses to <2px on an axis. Like h-overflow it needs no producer change — but where h-overflow was a clean
delegate (the `Seen` version used only `.rect`), collapsed-target's `Seen` version reads `.tag`, so the Box4
core reads the tag from the **path key's leaf** instead (`button.SIG:nth-child(n)`→`button`, sound because
`path_of`'s leaf IS the tag). It is a deliberate MIRROR, not a delegate: `jarring_collapsed_target`'s unit
test keys elements in the old `tag[n]` form with no `.`/`:` to split, so a key-parsing delegate would break it
— the mirror is obviated once the producer emits `Seen`. RED-proven (drop the `hittable(c)` guard → a
both-engines-collapsed control counts, 1→2).

**Tick 537 (brick 4b) enriched the G1 producer to `Seen` and wired the last two Layer-2 invariants.** The
path producer now emits the SAME 6-tuple `[tag, display, x, y, w, h]` the differential `oracle_probe` emits
(`chrome::capture_seen_all_paths` + `parse_seen_probe_json`; `PROBE_ALL_PATHS_JS` adds `getComputedStyle(e).display`),
and the Manuk side builds `oracle::Seen` with display from `page.styles_map()`. So the G1 exit gate now carries
`Seen` maps and calls **all four** jarring invariants — `jarring_h_overflow`, `jarring_overlap`,
`jarring_reading_order`, `jarring_collapsed_target` — **directly** on the oracle's own functions; the rect-only
Box4 view is derived cheaply for the placement scorers (SHAPE/coverage/first-divergence still take bare box maps).
This collapses the `collapsed_target_boxes<K>` mirror entirely (deleted, along with its test — the `Seen` version's
test covers the invariant) and obviates the tag-from-key parse; `h_overflow_boxes<K>` stays only as
`jarring_h_overflow`'s internal delegate. The two newly-wired invariants (overlap, reading-order) were previously
uncomputable in G1 because the Box4 producer carried no sibling grouping through `Seen`. RED-provable gate for the
enrichment (`chrome::tests::parse_seen_probe_json_reads_tag_display_and_box`): swapping the tag/display parse indices
flips the tag assertion red, and a bare 4-tuple entry is skipped rather than mis-parsed. **Decompose-first boundary
kept:** the gate FLOOR still gates on structural COVERAGE — the coverage→SHAPE flip still awaits a recalibrating sweep.

The remaining fidelity work is root-cause clustering (§3b), then the coverage→SHAPE gate-floor flip.

**Layer 2 — jarring invariants (SHAPE cannot see these).** SHAPE forgives a constant offset because a user
does not perceive one; but a box shaped *correctly relative to an over-wide parent* can still spill off the
viewport, and content cut off / an unexpected horizontal scrollbar is a top "this page is broken"
perception. `oracle::jarring_h_overflow` counts elements whose right edge passes `vw` in **Manuk** while
Chrome keeps the *same* element inside — the guard that requires "Chrome fits" is load-bearing: without it,
a site that legitimately scrolls sideways (right edge 2000 in both engines) is blamed on us. It reports per
site in the oracle run and as `h_overflow` in the `--emit` meta line. This is the first of the Layer-2 set;
the reading-order, unhittable-target, and post-load-shift invariants are not yet wired, and the instrument
does not claim Layer 2 is complete until they are.

`oracle::jarring_overlap` adds the *#1* "broken page" perception — text on text, a control under a banner.
It counts pairs of **siblings** (same parent path) that Chrome renders disjoint but Manuk renders
overlapping in both axes past `tol`; the "Chrome keeps them apart" guard is load-bearing (without it a
design that legitimately stacks is blamed on us). Scoped to siblings on purpose — that is where perceived
collisions cluster (flex/flow items, list rows, stacked cards) and it keeps the O(n²) bounded; groups above
`MAX_GROUP` skip the pairwise scan and the skipped-group count is surfaced so a bounded scan is never read
as a clean page. Cross-subtree occlusion belongs to the (not-yet-wired) hittability invariant, not here.

`oracle::jarring_reading_order` adds the third Layer-2 invariant — **reading order preserved.** A float, an
abspos, or a mis-placed grid item that escapes its slot makes a later element render *before* an earlier one;
the content jumps out of sequence even when nothing overlaps and every box is well-shaped, so SHAPE and
overlap both miss it. It counts pairs of **siblings** whose reading order (top-to-bottom, then left-to-right)
**Chrome and Manuk disagree about** — Chrome reads A-before-B while Manuk reads B-before-A, each with a clear
margin. Chrome is the reference for the *intended* order: a normal-flow engine lays siblings out in DOM order,
so a disagreement is Manuk pulling one out of place, never a legitimately reordered design (a site that
reorders is reflected in Chrome too and the pair agrees). Both orders must be **definite** (past `tol` on the
deciding axis); a pair too close to call in either engine is skipped, so tolerance jitter never manufactures
an inversion. Same bound and skipped-group accounting as `jarring_overlap`, and surfaced per site (`⚠ N
reorder`) and as `reorder` in the `--emit` meta.

`oracle::jarring_collapsed_target` takes the fourth Layer-2 invariant — **interactive targets hittable** —
in its box-dump-computable half. Hittability fails two ways: a control *collapses* so it has no clickable
area, or a control is *covered* by something painted over it (a button under a banner). This counts the
first: an element with an interactive tag (`a`/`button`/`input`/`select`/`textarea`/`summary`/`details`/
`label`) that Chrome renders with a real clickable box (both axes ≥ `min_hit`) but Manuk collapses (either
axis below it) — a dead control. It is **offset-invariant** (a page shifted 23px collapses nothing, so it
never re-charges the constant offset SHAPE forgives), and the "Chrome gives it area" guard is load-bearing
exactly like the overlap guard — a control the *site* itself collapses is hidden in both engines and is not
our bug. Fully-collapsed (0×0) controls never reach it: the probe drops them, so they surface as a *missing*
divergence; this catches the single-axis collapse (a zero-height button from a collapsed flex/grid track)
that keeps a box but kills the target. Surfaced per site (`⚠ N dead-target`) and as `dead_target` in the
`--emit` meta. **The occlusion-cover half of hittability** (a control under a banner) needs paint order /
opacity, which the geometry snapshot does not carry; it is partially surfaced already by `jarring_overlap`
and left for a later pass — this function does not claim to be the whole invariant. Of the five Layer-2
invariants, only **post-load stability** (a CLS-equivalent, needing a second post-settle snapshot) is now
entirely unwired.

**The corpus roll-up — where the invariants become the exit bar.** The per-site invariant counts are emitted
into each result file's meta line (`overlap` / `h_overflow` / `reorder` / `dead_target`), but a per-site
count certifies nothing about the corpus. `oracle::tally_jarring` rolls a slice of per-site rows into
`(sites_affected, total)` per invariant, and `oracle-merge` prints it as the **JARRING INVARIANTS (Phase-0
exit bar)** section: for each invariant, how many of the diffed sites exhibit it (with a percentage) and the
raw instance count. Sites-affected leads deliberately — the redesign gates on the *fraction of the corpus
that is not jarring*, so one site with 40 overlaps must not outweigh 40 sites with one each. Result files
that predate an invariant read 0 for it, which is correct. This is the number a Phase-0 exit claim is made
against; before it, the invariants were computed every crawl and discarded at merge time.

## Gates must run the SHIPPING configuration

The parity harness **defaulted to the simple cascade while the shell shipped Stylo** — so parity, fidelity
and the perf bench were all validating **a cascade no user had ever seen.** Fixing it changed the numbers
**in both directions at once**: fidelity was **understated** (81.2% → 86.3%) *while simultaneously hiding a
near-total Wikipedia layout failure* that only a screenshot revealed.

Later amended: **gating without the JS engine charges the ENGINE for the absence of the SCRIPT engine.**

## A gate that is never INVOKED is indistinguishable from a gate that passes (tick 239)

The strongest form of the rule below, and the one that survived longest here undetected. Everything in
this file assumes the gate *ran*. Measured at tick 239: `engine/page/tests/` held **104** gate files and
`scripts/verify.sh` named **19**. The only package-wide `manuk-page` invocation was a **`--no-run`
pre-warm** — it linked all 104 binaries and executed none of them, which is the cruellest possible shape,
because a build failure in any of them still REDs the wall and so the gates *look* tended.

**85 gates were therefore unwatched, and `CONSTELLATION.tsv` marked rows `gated` naming gates inside that
85.** A ratchet tooth nothing bites on. The sweep found 98 passing and 2 red, so nothing had actually been
lost — the finding is a blind spot, not a disaster. But one of the two reds was **`g_capability` itself**,
the gate written because the pattern ledger had been wrong six times, and it had gone stale in precisely
the way it exists to catch: it asserted the pre-2020 QName rule for `createDocumentType` while the engine
had correctly moved to the spec's "valid doctype name" at tick 135.

**Why the existing instruments could not see it.** `falsify.sh` mutation-tests the gates that run — it
answers *"can this gate go red?"*, never *"is anyone asking it?"*. A gate is proven red at authoring time,
committed, and then silently drops out of the conversation. The failure is in the **invocation list**, and
nothing audits a list.

**The mechanical fix is a shape, not a list:** a sweep with a NAMED deny-list, so a newly added gate is
watched BY DEFAULT and excluding one is a deliberate act with a reason attached. Hard-coding 85 more
`_launch` lines re-creates the same staleness one commit later. Where the wall budget cannot absorb the
sweep, run it OFF the per-tick path and bank pass/fail into `RATCHET.tsv` — the trade FID-SWEEP already
made. Full measurement and the exclusion set: `docs/loop/GATE-COVERAGE.md`.

**And the corollary that cost the most time here: when a gate and the engine disagree, the gate is not
automatically right.** I nearly "fixed" a spec-conformant engine to satisfy a stale claim. WPT settled it
in one grep — `dom/nodes/DOMImplementation-createDocumentType.html` expects `InvalidCharacterError` for
exactly two of ~70 names and a doctype back for `''`. **Check the spec's own test before you believe
either side of your own instrument.**

## A gate that CANNOT FAIL is a decoration — and they go vacuous SILENTLY

A coverage gate returned **1.0 when `probed == 0`**, and its own default URL list contained **`example.com`,
which has NO `[id]` elements at all** — so it probed nothing, **scored a perfect 100%**, and *inflated the
mean of the very gate meant to catch missing content.*

**Proven by mutation: emptying `node_rects()` so the browser rendered NOTHING still scored 100% there.**
The clickability gate had the identical shape (a browser that finds **zero links** scores as *perfectly
clickable*).

## MUTATION-TEST THE WALL — and then verify the mutation tester

`falsify.sh` installs, for each gate, **the exact bug that gate exists to catch**, and asserts it goes
**RED**. Its first run found **five** defects, including a **Bar 0 gate — the one between the user and a
frozen tab — that was VACUOUS**: deleting the page-budget function outright left it **green**, because it
was being protected by an unrelated per-request timeout.

**Three further traps, all real:**

1. **A mutation that fails to COMPILE returns non-zero exactly like a failing assertion** — so a typo
   *certifies the gate by nothing*. The falsifier must **BUILD first** and report a build failure as
   **FALSIFIER BROKEN**, never as evidence about the gate.
2. **Two gates racing over a process-global `OnceLock`** made the verdict depend on **thread scheduling**.
   (`request_timeout()`/`load_budget()` memoise process-wide: **the first caller to read them wins,
   forever.** One test file = one binary = its own `OnceLock`.)
3. **A killed run left a mutated constant in the tree** (`MAX_TASKS_PER_DRAIN = u32::MAX`, in a Bar 0
   path), which the next run then **"backed up" and "restored" as if it were the original.**

> **A "VACUOUS" verdict is a CLAIM ABOUT THE GATE. Verify it before believing it.** Six times the verdict
> was false and *the gate was right while the mutation was wrong* (aimed at a dead function, an unscanned
> file, the wrong score axis). **The tool that checks the instruments is an instrument.**

**And a linker OOM is not evidence about your code:** `ld terminated with signal 9` made the harness report
FALSIFIER BROKEN for two perfectly good mutations. Retrying at `CARGO_BUILD_JOBS=2` proved both.

## "The probe didn't say yes" is NOT "the probe said no"

Made **three times in four ticks**. `localStorage`, `FormData`/`URLSearchParams` and `position: sticky` were
each recorded as **"❌ missing"** and each **already worked** — **twice the replacement was written before
anyone noticed.**

One read as missing only because **the capability probe was served from a `file://` URL — an OPAQUE ORIGIN,
which correctly answers `QuotaExceededError` in EVERY browser.**

> **Serve capability probes over real HTTP, through the real pipeline. And if the probe does not test it,
> its status is UNKNOWN — which is not "missing".**
> **An absent measurement is not a negative measurement.**

## Corpus BREADTH, not verification throughput, surfaces class bugs

- **3 sites** reported COVERAGE 99.7% and *"everything is fine."*
- **20 sites** found that a page was **printing its own JavaScript**, that `:checked` never matched
  anywhere, that checkboxes were invisible, and that docs.python.org rendered **entirely dark**.
- **265 sites** found a **SIGSEGV core dump** (apple.com) and the whole hang class.

> **A three-site sample is not a benchmark; it is an anecdote that confidently reports that a bug on one of
> those three is the most important bug on the web.** *The bugs a corpus cannot find are exactly the ones no
> corpus site happens to use.*

## Every number has a HARNESS, and the harness is part of the number

- **Job count is part of the measurement.** 4 jobs → 11 hangs/88 sites (**12.5%**); 12 jobs → 22/45
  (**49%**) — **same binary, same corpus, same hour.** (Twelve parallel oracle runs meant **189 concurrent
  Chromium processes**, and the watchdog fired on *manufactured contention*.)
- **`export -f` + xargs workers SURVIVE the death of their driver** — a previous crawl's workers kept
  writing into the new run's results directory. Caught **only by luck** (the two script versions used
  different labels). Every record now carries a **`RUN_ID`**, and the crawl **refuses to start on live
  workers**.
- **An interrupted crawl always UNDER-reports**, because *the sites that hang are the ones still running
  when you kill it.* The status script **refuses to print a partial run.**
- **A benchmark that shares a machine with a compile is not a benchmark** — and **RAM, not cores, was the
  binding constraint.**

## Residual-bug estimation must use a REMOVAL model, because discovery is SERIAL

Each tick's fix changes the codebase, so this is **not** independent sampling of a frozen artifact — a naive
Lincoln-Petersen estimator will **UNDERESTIMATE** the residual. Use a **removal model (Zippin/Moran)**: fit
the declining discovery-rate curve; the x-intercept estimates the total population.

**Report it as a LOWER BOUND, scoped to the current capability surface — and EXPECT the estimate to GROW as
the crawl frame expands.** *A rising number from better instrumentation is the method working, not
failing.*

## Read Blink/Gecko for the ALGORITHM; never copy the CODE — and know what that buys

For any ambiguous, edge-case-heavy behaviour (margin collapsing, line breaking, float/BFC interaction, event
dispatch order, IDL reflection), read the reference source **first** and extract the *algorithm and its
edge-case list*, **citing the file/function in the commit.**

**Stated ceiling, so it is not over-extrapolated:** this compresses **DISCOVERY, not IMPLEMENTATION** — the
Rust still has to be written — and it does **nothing** for external-integration problems (codec licensing,
GPU drivers, DRM), which are not algorithm-discovery problems.

## Three gates that exist because green gates coexisted with real bugs

- **G_ALLOC** — every perf floor stayed green through a clone-per-wheel-event regression, because **a
  load-time bench measures throughput on an idle queue, not the marginal cost of an EVENT.**
- **G_TEARDOWN** — forbids `libc::_exit()` or any process-exit path bypassing Rust `Drop`. *A workaround
  that hides a crash is a data-loss bug wearing a disguise.*
- **G_SILENT_FAIL** — a swallowed script exception hid two missing IDL properties that were killing
  navigation on **every mdbook site**. *A caught error that is not logged or surfaced is a gate violation,
  not defensive coding.*

## THREE anchors of parity scope, and each sees what the others cannot

1. **The differential oracle** (265 real sites vs Chromium) — *what real pages do*; needs Chromium to say
   what's right; blind to anything that does not move a box.
2. **WPT** (`docs/wiki/wpt-horizon.md`) — *what the spec says*; carries its own verdict, no oracle needed;
   sees adversarial cases no real site generates; blind to which spec features the real web actually uses.
3. **The doc/app/platform-web capability roadmap** (`PARITY-LEDGER.md`) — *which class of the web works
   end-to-end*.

**No one of them is sufficient.** The oracle found the cascade dropping 41% of real sites' nested rules;
WPT found `insert_before(X,X)` looping forever and `load` never firing; the roadmap is what says iframes and
lazy-load come before media. **Feed all measured WPT categories into the priority ledger with the same
`usage × divergence` formula the oracle's cluster ranking uses — one ledger, not three.**

## The async CI lane is redundant verification you never wait on

`.github/workflows/ci.yml` runs the full wall on every push, in parallel, at zero cost to the local loop —
a regression it finds is an ordinary gate failure at the next check-in, never an interrupt. Split into a
**badge-bearing Linux lane** (shipping config, must be green) and a **cross-platform known-gap lane**
(`continue-on-error`, promoted into the badge when a platform goes green). *A green badge that has stopped
meaning anything is worse than a red one from a real regression.*

## The pattern ledger, and why it is now executable

`docs/loop/WEB-PATTERNS.md` decides what this project builds next. It is the most load-bearing instrument
in the loop, and for a long time it was **the least verified file in the repo**.

At tick 65 every `❌` in it was probed. The result:

| The ledger said | The truth |
|---|---|
| *"~1 site in 4 **hangs** — Bar 0. Nothing else matters at this ratio."* | **4 sites in 265** (1.5%). Off by 16×, and it was steering the roadmap. |
| *"React committing its render — ❌ still silent. Renders nothing."* | **React renders.** `#root` gets its children, the app's text, zero errors. |
| *"`append`/`prepend`/`before`/`after`/`replaceWith` ❌"* | **All five work.** So do `insertAdjacentHTML` and `remove`. |
| *"`outerHTML`, `innerText` ❌"* | **Both work.** |
| *"`Blob`/`File`/`FileReader` ❌"* | **All three work.** |
| *"`getSelection`/`Range` ❌"* | Both **exist**; only `document.createRange()` is missing. |
| *"CSS `transform` — not in computed style, a real gap"* | The transform **is applied** — the box really moves. Only the *computed-style read-back* is missing. |

**Three of its top three priorities were phantoms.** The loop had been aiming at ghosts.

### The mechanism

The lesson — *an absent measurement is not a negative measurement* — had been written down **five times**
(PROCESS #19, #20, #21, #35, #41) and did not hold. A rule you can recite while breaking it is a
decoration. So it stopped being a rule:

> **`G_CAPABILITY` runs the ledger's claims as assertions**, on every wall. 42 of them. A `✅` that stops
> being true **fails the tick** — which is the RATCHET (*never regress capability*) made mechanical. And
> every `❌` prints a **receipt** from the same run, so the next person reads a measurement instead of
> inheriting a rumour.

The ledger cannot drift from reality, because reality is what runs.

### The gaps that are real (with receipts, tick 65)

* **`<canvas>` 2D draws nothing.** Not absent — a *stub*: `getContext('2d')` returns a context, `fillRect`
  is a function, and filling the canvas red then reading a pixel gives `0,0,0,0`. It is deliberate (a
  blank chart beats a `TypeError` that takes the whole bundle down) and it warns in-product. But a page
  that feature-detects canvas is told **yes** and renders nothing.
* **`scrollTop` lies.** Reading gives `undefined`; writing silently creates a plain JS property that
  scrolls nothing. A virtualised list sets it, reads it back, and believes it worked.
* `getComputedStyle().transform` → `undefined` (the transform itself works).
* `display: contents` → reports `inline`.
* `document.createRange`, `document.createEvent`, `URL.createObjectURL` → absent.

## The cadence ledger — measuring the loop, not just the browser

For sixty-nine ticks this project measured the browser exhaustively and **the loop not at all**. "Tick 69
landed" is a receipt, not progress data — and the project has two horizons whose only honest question is
*are we getting there, and how fast?*

`scripts/tick-log.sh` runs from `scripts/tick.sh` **after a successful push** (a tick that did not land is
not a tick) and appends one row of ground truth to `docs/loop/CADENCE.tsv`:

* **when** it landed, and **Δ since the previous tick** — the real cycle of implement → debug →
  verify-wall → land. This is the loop's clock speed and the denominator of every rate.
* **what it cost**: wall seconds, files, lines.
* **what it bought**, measured rather than asserted — NEAR: capabilities asserted by `G_CAPABILITY`, live
  gates, ✅ rows in the capability ledger, oracle hangs. FAR: WPT subtests.
* **the shape, and the tick's own headline** — which is already written per tick in terms of what changed
  for the browser, so it *is* the qualitative impact statement.

`scripts/cadence-report.py` regenerates `docs/loop/CADENCE.md` from it. **The row is not the point. The
derivative is.**

### Backfilled from git, and what was deliberately left blank

Sixty-two past ticks were reconstructed from history — every tick is a commit, and a commit carries its
timestamp, its diff and its message; the journal carries the shape and the headline.

**The verify-wall time, the WPT figure, and the gate/capability counts of past ticks were left EMPTY.**
`STATUS.md` records only the *latest* wall; WPT was measured a handful of times; the counts are obtained by
grepping the tree, and the tree is *now*. Counting today's tree and labelling it "tick 42" would produce a
beautiful, entirely fictional curve. **An empty cell is a fact. A guessed one is a lie that gets quoted
back later as evidence.**

A WPT figure carried forward from an earlier tick is marked (`·`) and is **never counted as a
measurement**.

### What it found on its first run, and it is strategic

| | |
|---|---|
| ticks landed | 62 (ticks 1–69) |
| median cycle | **19m** (17m over the last 10) |
| ticks/hour | **0.85** across 71.8h elapsed |
| capability ticks | **25 of 62** (40%) |
| median diff | +288 / −10 lines, 7 files |
| WPT (`dom/`) | 1736/6418 (tick 64) → **1737/6418** (tick 69) |

That last row is the finding, and it is worth more than the rest put together:

> **Ticks 64–69 shipped a 60× DOM speedup, real prototypes, a canvas rasterizer, element scrolling and
> `display: contents` — every one a genuine daily-driver capability win — and WPT moved by ONE subtest.**
>
> The two horizons are **nearly orthogonal**. The far horizon will not arrive as a side-effect of the near
> one; **it has to be spent on directly.**

That is not a failure. It is the first measurement of the *relationship between the two horizons*, and it
changes how ticks get allocated. It could not have been made without this ledger.

### The one number it refuses to give

A finish line. The rate is measured on the `dom/` subset (6,418 subtests); the far horizon is ~50,000
across all of WPT, **which this project has never run**. Multiplying a subset's rate up to the whole is not
an extrapolation, it is a category error dressed as arithmetic — so the projection is not made.

## A doubled event handler inflates the WPT count ~2× — the score can lie in your favor

When a `<body onload>` (or any) handler that **creates subtests** fires twice, the harness counts the
subtests twice. This is not a crash and not a visible failure — it silently **inflates** the pass count.
Tick 96 found `__fireLoad` invoking `window.onload` via both `dispatchEvent` and an explicit call; measured
same-binary, encoding went **110,111 → 55,057 = exactly 2.00×** once fixed, and the whole-suite headline
dropped from 749,793 / 47.5% to the honest **388,674 / 32.11%**.

**The lesson:** a rising WPT number is not self-evidently real. A double-dispatched lifecycle event, a
retried async test, a harness that re-runs a file — each can inflate. Trust the number only when the
**mechanism** that produces it is understood. When re-basing a metric downward to correct an inflation, do
it as a **documented one-time correction** with the prior marks saved — the ratchet's `bank` only ever
raises, precisely so a real regression can't be laundered; an honesty correction must be explicit, not
sneaked through.

## The batch-size crash class — heavy layout areas OOM a shared process

The sweep runs N files per process (`batch_for`) to amortize runtime startup. **Heavy layout areas**
(css-grid: full runtime + DOM + grid tree per file) retain enough memory that a 40-file process is killed —
a `crashes=1` that is a **batch-SIZE artifact, not an engine bug**: the file runs clean in isolation and the
pass count is **batch-invariant** (css-grid 150 at batch 40 and batch 10). Diagnosis: does it reproduce at
`--batch 1`? If no, it is accumulation. Fix: right-size `batch_for` for that area (encoding=4, css-grid=10),
exactly as encoding already is — never hide it by dropping the area or ignoring the crash. [[wpt-horizon]]

## The crash class is a real SIGSEGV UAF, not OOM — and ISOLATION-RETRY separates it from a per-page Bar 0

Tick 101 corrected the tick-96 read above. The heavy-layout batch crash is **exit 139 (SIGSEGV), not 137
(OOM)** — a genuine **use-after-free** in the reflector/rooting teardown when many pages share the
process-global SpiderMonkey runtime (thread-local `ENGINE`/`RUNTIME`, `ManuallyDrop`; each file makes a
fresh `Page` but the *runtime* is reused). It is a **Heisenbug**: it needs cross-file heap accumulation,
reproduces reliably only under memory pressure, and **vanishes under `gdb`** (perturbed heap) — so gdb
gives no backtrace and the real fix needs **ASAN**, not gdb. Batch-sizing does NOT reliably fix it
(heavy files accumulate faster; only `--batch 1` guarantees clean, an unacceptable permanent tax).

**The instrument fix (not a mask): isolation-retry.** When a batch child dies by *signal*, the driver
re-runs the single culprit in a **fresh** single-file runtime. If it passes alone → its per-page result
is the truth, recorded as **`ACCUM`** (a distinct, printed metric — the artifact is never invisible), and
NOT counted toward the Bar-0 `HANG/CRASH`. A file that crashes **alone too** stays `CRASH` — *a real
per-page Bar 0 is never reclassified away*. This is honest because the batch harness's runtime-reuse is a
speed hack real browsing never does (one document per fresh context); the isolation result is the real
single-page behavior. **The underlying UAF remains a tracked Bar-0 to FIX with ASAN** (see the
flexbox-relayout-segfault repro: `manuk-wpt wpt css/css-flexbox --child --limit 40` → 139, `--limit 20`
→ clean; culprit `stretched-child-shrink-on-relayout.html`). [[js-engine]] [[wpt-horizon]]

## A SECOND, distinct SIGSEGV: css-values/calc-size interpolate-size — a REAL per-page crash (survives isolation)

Tick 126. orient's tick-125 full sweep surfaced `css/css-values crashes=1`. Unlike the flexbox UAF above
(an **ACCUM** artifact — clean when run alone, so isolation-retry reclassifies it), this one **crashes in a
fresh single-file process**, so isolation-retry keeps it as a real `HANG/CRASH` Bar-0 — the more serious
class (a real page could trip it). Reproducer, deterministic exit 139:

```
target/release/manuk-wpt wpt css/css-values/calc-size --child --out /tmp/o.jsonl --start 5 --limit 1 --timeout 10
```

Crashing files: `interpolate-size-computed.html` and `animation/interpolate-size-interpolation.html`. Both
load template-literal-heavy support JS (`computed-testcommon.js` / `interpolation-testcommon.js`) that our
engine *also* rejects with `SyntaxError: unexpected token: identifier` — so the `test_*` bodies never run;
the fault is in **parse/compile/execute of the support JS + testharness.js**, not interpolate-size. Same
Heisenbug signature as the flexbox UAF but harder: **release-only** (debug runs the identical JS cleanly),
**non-deterministic on minimal repros** (near-identical inputs flip on tiny heap deltas — so the
"template-literal" correlation is noise, not cause), **all `?? ()` under gdb inside statically-linked
SpiderMonkey** with NaN-boxed GC values on the faulting stack (no OUR-code frames), and unaffected by a
256 MB stack — a wild GC-object pointer, corrupted earlier and faulted on later. **Not a tick 117–125
regression** (every JS change that window is pure-JS prelude or a native binding the crashing files never
call; crashes=0 at sweeps 114–116 was a flaky sample). Needs **ASAN** to localize the corrupting write —
tracked open Bar-0 for a fresh, well-resourced context, exactly like the flexbox one. [[js-engine]]

## Probing the constellation: `unknown` is a bug, not a state

`docs/loop/CONSTELLATION.tsv` carries a `status` per capability, and the lever board computes its
priorities **from it**. That makes an `unknown` row actively harmful rather than neutral: it steers
the loop while carrying no evidence. Tick 225 probed 16 of them and the result argues the point —
**WebAssembly, CJK line breaking and media queries were all carried as `unknown` and all already
worked.** WebAssembly in particular ("Figma, games, ffmpeg.wasm") compiles a real module, instantiates
it, resolves an export and returns the right integer.

Two failure modes the file had accumulated, worth checking for periodically:

- **Stale cells.** Five rows still said `unknown`/`missing` for capabilities that later ticks had
  *landed with gates*: bidi (t215, `G_BIDI_BASE`), CJK/emoji font fallback (t214, `G_COMPLEX_SCRIPT`),
  `<details>`/`<summary>` (t216, `G_DETAILS`), `URL.createObjectURL` (t223, `G_MSE`), CORS (t170-173,
  `engine/net/src/cors.rs`). Nothing updates these automatically, so a landed capability keeps
  reporting as a hole and keeps attracting ticks.
- **Never-measured cells** that are cheap to settle and were simply never looked at.

### A probe must be behavioural, and in this engine that is not pedantry

`typeof X === 'function'` is exactly the check an **inert stub** passes, and this engine deliberately
ships a whole list of them (`event_loop.rs`'s inert-name sweep, whose own comment records a stub
having once silently disabled a working implementation). `drag and drop` is the live example:
`DataTransfer` *exists* — as an inert stub — while `ondragstart` does not, so a presence check would
have reported a capability that does nothing. So `g_probe_capabilities` measures behaviour:
WebAssembly by calling the export, multicol and container queries by reading back the geometry they
should produce, CJK breaking by whether the text actually wrapped inside its box.

### The probe gate is a ratchet, not a survey

It asserts only what measured **true**, so a capability found working can never silently regress to
missing. What measured false is written into the TSV as `missing` with the gate as its receipt —
*measured absence*, which is a different and far more useful thing than never having looked, and
which starts failing the day someone implements it (at which point the claim moves into the pinned
list). One run therefore both flips cells green and installs the guard that keeps them green.

### A probe whose claim cannot fail measures nothing (tick 230)

Two probes in the second batch were **vacuous**, and one of them reported a capability that does not
exist:

- `querySelectorAll('video:muted').length >= 0` is true of every engine that does not throw —
  including one that ignores the pseudo-class entirely and returns an empty list. It reported **yes**.
  Rewritten to discriminate — a muted and an unmuted `<video>`, with the selector required to match
  exactly the muted one — it reports **no**.
- A flag-based check (`__cspInlineRan !== true`) where nothing ever set the flag.

Both would have flipped a constellation cell on no evidence, which is strictly worse than the
`unknown` they replaced: an `unknown` invites measurement, a false `works` closes the question.

**The rule this yields:** every probe must be written so that some reachable state makes it FAIL, and
that state should be named. `wasm` fails if the export returns anything but 7; `multicol` fails if the
column box is full width; `mediapseudo` fails if the selector matches neither video or both.

### Some capabilities cannot be probed from inside the page

CSP enforcement is the example, and it is structural rather than incidental. The natural test — an
inline script must be blocked by `script-src 'self'` — **cannot be run from an inline script**,
because a working implementation prevents the probe from executing at all. Absence of the result is
indistinguishable from the probe never running. It needs an external-script harness and a real
response header, so the cell stays `unknown` rather than taking a verdict this harness cannot earn.

## The oracle must know whether the engine actually styled the page (tick 383)

The one-snapshot rule pins the DOCUMENT, not its SUBRESOURCES. Under crawl load (6 jobs +
Chromium), the engine's per-resource fetch timeout starves author stylesheets, the page renders
UA-default, and the diff dutifully books the difference as engine drift: apnews.com carried 291
`display none→block` divergences in the tick-380 crawl (dropdown submenus the author's sheet
hides) and ZERO on a quiet re-run — the whole "author-style-not-applied" ledger family
(none→block 49 sites / flex→block 43 / block→inline 39) was substantially this artifact. Same
snapshot, two runs: 1268 vs 541 total divergences.

The seam, in two halves:
- **engine** — `Page::failed_stylesheet_fetches()`: render-blocking sheets requested and never
  arrived (a later round's success removes the URL — the set holds what is failed NOW). Claimed
  in G_SILENT_FAIL: a dead sheet must be countable, not only logged.
- **instrument** — the oracle DISCARDS a site when the count is non-zero, mirror of
  `oracle_is_healthy` on the Chrome side: counted, labelled, never scored. The crawl driver
  already records a skipped site as DISCARDED (a missing result file is itself the signal).

The general rule: **a differential instrument needs a health check on BOTH engines.** We had one
for the reference for hundreds of ticks before the tick-380 crawl showed the same failure mode on
our own side. Residue (named): per-site jarring counts still vary run-to-run with live-network
weather even when styled (apnews overlap 0↔22 across two healthy runs); full determinism needs
subresource snapshotting — CSS cached beside the HTML — which is the next instrument rung.

## Same-tag index paths misattribute TREE drift as STYLE drift (tick 395)

The t392 ledger's surviving `none→block` family (okta 59 / nasa 38 / cnn 25) probed to its
mechanism — and it is the INSTRUMENT again, one layer deeper than t383. The evidence chain on
okta.com (all quiet-box, all against the cached snapshot): the hiding rule
(`.cmp-cards-item__date-bottom{display:none}`, plain class, top-level in clientlib-site) applies
in a minimal inline fixture, applies via the real linked sheet, and applies ON THE FULL PAGE at
the oracle's own 1200px viewport — a probe div carrying the class stays unpainted while an
uncla ssed control paints. The engine's cascade is right end-to-end; yet the oracle reports 316
display divergences (Chrome author-values vs "our" UA-defaults) with only ONE missing box.

The mechanism: the diff keys elements by `tag[same-tag-sibling-index]` chained to the root. One
inserted or removed node early in `<body>` (a consent banner one engine's scripts create, a
wrapper the other's parser nests differently) shifts EVERY later same-tag index at that level —
and div-vs-div mispairs pass silently, each engine truthfully reporting a DIFFERENT element.
Style diffs produced this way are phantoms; only tag mismatches surface as tree drift.

The fix is already specified: FIDELITY-SCORING-REDESIGN.md's selector-path keying
(`tag[.class-sig]:nth-child(n)`) — the class signature makes a mispaired div fail the KEY match
instead of minting a phantom style diff. This probe converts that item from "unlocks unmeasurable
React sites" to "removes a phantom-divergence class the current ledger is polluted by": the
display-diff families in CLUSTERS.md should be read as UPPER BOUNDS until it lands.

## BUILD SPEC — selector-path keying: the class signature that stops silent mispairing (tick 399)

The redesign's item (a), re-priced by t395 into the board's top instrument work. The current key
is `tag[same-tag-sibling-index]` chained root-ward, computed twice — in the Chrome-side JS probe
(`chrome.rs::oracle_probe`, `pathOf`) and in the Rust walk (`main.rs`, `path_of`) — and the two
MUST stay byte-identical (the `html[0]` lesson: a one-level naming skew once reported `<html>`
missing on every site, with total confidence).

The new key component: `tag.SIG:nth-child(N)` where
- `N` = index among ALL element siblings (not same-tag) — cheaper to compute identically and
  stabler under mixed-tag insertion;
- `SIG` = fnv1a-32 hex over the element's class list, ASCII-lowercased, SORTED, joined with
  `.` — sorted so attribute-order and framework class-shuffling don't change identity; hashed so
  Tailwind's 40-class strings don't bloat keys; fnv1a because both sides already have it (Rust:
  `fnv` in main.rs; JS: 8 lines, `h=0x811c9dc5; h^=c; h*=0x01000193>>>0`);
- classless elements emit `tag:nth-child(N)` (no empty sig — most of the anonymous-div web).

Semantics change, and it is the POINT: an element whose class list differs from its positional
counterpart now FAILS the key lookup and books as missing+extra (tree drift, which it is),
instead of minting a phantom style/geometry diff between two unrelated elements. Expected
effects, stated before the run so the run can falsify them: okta's 316 display diffs collapse
(the t395 proof); the 9-LOW_SAMPLE/4-NO_IDS unmeasurable sites become measurable (the item's
original motivation); MISSING totals RISE where trees genuinely drift (hydration) — that rise
is honest and must not be "fixed" by loosening the sig.

Build order (one tick): (1) fnv1a-32 in the JS probe + sig in `pathOf`; (2) the identical sig in
`path_of`; (3) RED both ways — perturb the sig on ONE side (extra class in the hash) and every
element on a fixture page must go missing; restore → align; (4) e2e okta: display diffs must
drop by roughly the phantom mass; (5) re-crawl banks the re-keyed baseline as the NEW ratchet
line (old numbers are not comparable — say so in the journal, do not splice trends).

### LANDED (tick 401) — the keying is live, and the predictions held

Both sides now compute `tag.SIG:nth-child(N)` (chrome.rs `sigOf`/`pathOf`, main.rs
`sig_of`/`path_of`). Contract details fixed at build time: N is 1-based over ALL element
siblings; SIG hashes the ASCII-lowercased sorted deduped class list joined with `.`, and the
hash runs over UTF-16 CODE UNITS on both sides (JS `charCodeAt` ↔ Rust `encode_utf16`) —
hashing UTF-8 bytes on the Rust side would silently skew every non-ASCII class name. The hex
is zero-padded to 8 on both sides. Hashing also keeps Tailwind's `/` out of the path, so
`rfind('/')` parent-walks (common_frame, jarring siblings) stay format-agnostic.

Evidence: (RED) a one-sided sig perturbation books exactly the 5 classed elements of a 9-element
fixture as missing — classless and empty-class elements still pair; restore → 0 divergences.
(e2e) okta's 316-hit phantom display family collapsed to 0–35 across two quiet runs (residue is
run-variance in our own render, not pairing), while MISSING rose to ~128 — the honest tree-drift
booking (our JS demonstrably fails to mount components there: named errors on the console).
Baseline-reset rule is IN FORCE: no ledger number from before tick 401 is comparable to one
after; re-crawl and re-rank before steering off any display-diff family.

## JS-platform-surface probe sweep — vein mined out (tick 487)

A measurement tick swept ~40 site-critical JS platform surfaces across two batches to find the next
bounded probe→build (the pattern that landed t486 `navigator.userActivation`). The result is a boundary
finding worth pinning: the **clean-bounded JS-platform-surface vein is MINED OUT.** Already-built and
re-confirmed present (the seventh re-confirmation of the standing stale-PESSIMISTIC rule — probe before
building anything marked missing): connection, scheduler.postTask/yield, locks, permissions, wakeLock,
mediaSession, storage, clipboard, CSS.supports, structuredClone, reportError, queueMicrotask, sendBeacon,
PerformanceObserver, crypto.randomUUID/getRandomValues, visualViewport, AbortSignal.timeout/any,
ResizeObserver, IntersectionObserver, Object.hasOwn, Array.at, performance.*, matchMedia.addEventListener.
The only JS surfaces still absent — navigator.share/canShare, vibrate, cpuPerformance, CSS.registerProperty —
are either **honest-absent** (they match desktop-Linux Chrome and feature-detect cleanly; adding a
present-but-always-rejecting `navigator.share` would create the same present-but-broken trap OPFS
`getDirectory` is deliberately kept absent to avoid) or **present-but-inert traps** (`CSS.registerProperty`
without registered-custom-property cascade integration reads as "typed custom props work" to a feature-detect
while silently not applying `initialValue` — worse than absent). So the honest next frontier is the sized
SUBSYSTEMS in PHASE0-BOUNDED-REMAINDER.md, not more surface probing.

## The DOM-method / CSS-property surface vein is mined out too — one level deeper than t487 (tick 492)

After ticks 489–491 mined the last clean bounded bricks from this vein (`[hidden]` collapse, the
`inputMode`/`enterKeyHint` mis-key, `dialog.requestClose`), a second probe sweep confirmed the vein is
exhausted at the DOM-method and CSS-property layers as well. **Present and correct** (probe before building —
the stale-pessimistic rule pays again): `checkVisibility`, `getAnimations`, `moveBefore`, `setHTMLUnsafe`,
`replaceChildren`/`append`, `togglePopover`, `form.requestSubmit`, `dialog.close`/`showModal`, and the whole
**form-constraint-validation surface** (`stepUp`/`stepDown`→correct arithmetic, `valueAsNumber`, `validity`
with `valueMissing`/`typeMismatch`, `checkValidity`/`reportValidity`/`setCustomValidity`). Also correct:
`datalist`/`template`/`noscript` collapse to `display:none`; `text-align:end`→`right` (correct for LTR).

**What's left is not atomic.** The still-absent items each need a subsystem, not a brick:
- **CSS Typed OM** (`computedStyleMap`, `CSS.px`/`CSS.number`, `attributeStyleMap`) — a whole numeric-value API.
- **Custom Highlight API** (`Highlight`, `CSS.highlights`) — range styling machinery.
- **`Element.getHTML()`** — looks like `innerHTML`, but shadow DOM is real here (`attachShadow` works), so a
  naive `getHTML = innerHTML` is a *subtle lie* the moment a caller passes `{serializableShadowRoots:true}`; a
  correct impl is a shadow-serializer.
- **`showPicker()`** — no picker UI to show; honest form is a `NotAllowedError`-without-activation stub, weak value.
- **CSS `accent-color` / `touch-action` / `overscroll-behavior` / `text-decoration-thickness` /
  `text-underline-offset` / `text-wrap`** — measured **servo-DROPS**: absent from the built
  `stylo/out/properties.rs`, so Stylo's own parser cannot see them. The `@container` source-supplement trick
  does NOT rescue a dropped *property* (only dropped *at-rules* and whole declaration blocks, which Stylo's
  public parsers still accept) — reviving these means patching the Stylo build, a subsystem.

**One flag worth a re-probe:** `field-sizing` is marked `gated` (t388) but on the LIVE Stylo path
`getComputedStyle('field-sizing')` is empty and `CSS.supports('field-sizing','content')` is false — the t388
recovery was MinimalCascade-only and does not hold on the shipping cascade (the two-cascades trap again). The
next frontier remains the named subsystems (ch/ex font metrics, media codecs, the fidelity-instrument rebuild),
each decomposed before starting — not more surface probing.

**Next lever located: ch/ex real font metrics (Tier-2 item 23).** `engine/css/src/stylo_engine.rs`
`StubFontMetrics::query_font_metrics` returns `FontMetrics::default()`, so `1ch = 1ex = 0.5em` for every font
(measured: monospace-10ch == serif-10ch == 10ex == 80px at 16px — a monospace `Nch` code block or terminal is
~20% too narrow). This moves the REAL Phase-0 gate (placement fidelity) but is a 2-3 tick cross-crate
subsystem: the provider lives in the `Device` Stylo shares across rayon parallel-cascade threads while manuk's
`FontContext` is RefCell-based, so it needs a Send+Sync-safe metrics path (a thread-local/RefCell shortcut is
unsound under concurrent cascade), and `ex` needs a new x-height query in manuk-text.

## The exit certificate is now COMPUTED, not read off 265 stanzas of stderr (tick 547)

`FIDELITY-SCORING-REDESIGN.md §3` states the Phase-0 exit rule mechanically — *shape ≥ 0.75 on ≥95%
of sites, and ≥95% of sites clean on each of the four jarring invariants* — and the rebuilt instrument
(bricks t531–540) measured every term of it. Then it **printed them per site and threw them away.**
The four jarring counts existed only as `eprintln!` lines; `Fidelity` had no field for them. So
turning a sweep into the certificate meant a human reading 265 stanzas of stderr and adding up — which
is exactly the step that gets skipped and then estimated, and an estimated certificate is the one
thing the whole redesign exists to prevent.

So: `Fidelity.jarring: [usize; 4]` (h-overflow · overlap · reading-order · dead-target, order pinned
by `JARRING_NAMES` so a report cannot silently relabel three columns at once), and
`fidelity::certificate(rows) -> Cert` with `holds()` / `shortfalls()`. `certificate_report` prints on
**every** fidelity run, including the two-site G1 gate — a block that says "sites 2" is obviously not
a corpus read, which is safer than a headline that appears only when someone remembers to ask.

**Three design decisions, each one a way the certificate could have been passed without being met:**

- **It is a CONJUNCTION.** One term below the bar fails it. Averaging the four invariants together
  would let a 60%-clean reading-order hide behind three 100%s — and reading-order is the widest error
  bar in the roadmap's own risk register.
- **Unscored sites count AGAINST the bar, not out of it.** `shape_frac` divides by `sites`, never by
  `scored`. Dividing by `scored` means the certificate can be met **by failing to measure**, which is
  the same defect the NaN check in `report` was added for after `example.com` (no `[id]` elements)
  scored a perfect 100% in the gate whose job is finding missing content. RED-PROVEN: switching the
  denominator to `scored` and dropping the `scored == sites` term makes
  `the_certificate_is_a_conjunction_not_an_average` fail.
- **The floor and the bar are `const`, not parameters.** `CERT_SHAPE_FLOOR = 0.75`,
  `CERT_SITE_BAR = 0.95`. A floor a caller can pass in is a floor that will eventually be passed in,
  and *"widen the bar to pass"* is the one move this project refuses outright.

`--urls-file PATH` drives the instrument from a corpus file (one URL per line, `#` comments, and a
leading `category<TAB>url` — `docs/bench/oracle-corpus.txt`'s own shape — has the category stripped).
265 URLs are not expressible as a comma list without hitting `ARG_MAX`, and more importantly a file
leaves a record of *which* list was swept.

**First real reading, one site, `news.ycombinator.com`:** coverage 100%, visual 90.3%,
**shape 72.9%** — just under the floor — and all four jarring invariants clean. That single row is the
shape of the finding the corpus sweep is expected to produce: not missing content, not jarring
breakage, but placement drift sitting a few points below the bar. [[fidelity-instrument-shared-snapshot]]

⚠ **Harness note, observer-owned:** `scripts/fidelity-sweep.sh` still greps for the OLD
`PLACEMENT: N% within Npx | median offset dx=… dy=…` line, which brick 4b replaced with
`SHAPE: … | [diag] absolute PLACEMENT …`. Its `place` column therefore comes back EMPTY and it cannot
see SHAPE or the jarring invariants at all. The sweep driven directly through
`manuk-wpt fidelity --urls-file` does not have that gap. Recorded, not fixed: `scripts/` is
observer-owned.

## The certificate's FIRST sweep found the certificate could be passed vacuously (tick 549)

The first stratified corpus read on the rebuilt instrument — 72 sites round-robin across all 15 category
classes, 54 scored — printed **shape ≥0.75 on 12 of 55 sites (21.8%)**. That number is wrong, and
finding out why is the whole tick.

`shape_stats` returns a **ratio**, and `0/0` is `1.0`. Seven sites reported
`SHAPE: 100.0% within 8px vs shared ancestor (0 scored)` — and one of them was `gov.uk`, where
**all 418 probed elements were MISSING.** A page we render nothing of scored perfect placement, and the
certificate counted it as meeting the placement bar. Nine of the twelve apparent passes were vacuous.
Corrected: **3 of 54 (5.6%)**, a factor of four.

**This is the fifth instrument built here that produced a bad number on its first real run** (the crawl
report announced 2.8% for a browser that renders fine; `G_LOAD` had never tested its own budget; `G6`
scored a browser finding zero links as perfect clickability; `example.com` scored 100% coverage with no
`[id]` elements). The shape is always identical: **a denominator nobody checked.** A ratio is not a
measurement until you know what it was computed over — so `Fidelity.shape_n` now travels with the score,
`CERT_MIN_SHAPE_SAMPLE = 10` makes a thin sample **UNSCORED** (which counts *against* the bar, not out
of it), and the sample size round-trips through the accumulated-rows file so a vacuous pass refused
in-process cannot come back from the chunk boundary. RED-PROVEN: removing the `shape_n` term from
`certificate` makes `a_shape_score_over_an_empty_sample_is_never_a_pass` fail on the exact `gov.uk` row
the sweep produced.

Ten is not arbitrary: it is `scripts/fidelity-sweep.sh`'s own `LOW_SAMPLE` threshold, added to that
script for exactly this reason. The lesson had been learned in one instrument and not the others —
*again*, which is the thing `verify.sh`'s own header says about SHORT-vs-CRASH.

### What the sweep actually says

| term | measured | bar |
|---|---|---|
| shape ≥0.75 | **5.6%** of sites | 95% |
| h-overflow clean | 77.8% | 95% |
| overlap clean | 59.3% | 95% |
| reading-order clean | 46.3% | 95% |
| dead-target clean | 75.9% | 95% |

And the finding that outranks all five: **13 of 54 sites render under 5% of what Chrome renders** —
nytimes.com 0.04%, stripe.com 0.14%, reactjs.org 0.13%, notion.so 0.32%, terraform.io 0.30%,
bitbucket.org 0.36%, and cdc.gov / intel.com / gov.uk / harvard.edu / newyorker.com / propublica.org /
squarespace.com at 0.0%. That is a **class failure, not placement drift**, and it means the drift
numbers above are measured on the sites that work. The three sites that clear the shape bar are all
static-ish blogs (`jvns.ca` 94.3%, `blog.rust-lang.org` 87.4%, `lobste.rs` 85.7%) — the class this
engine has always been good at.

**Two honesty notes that must travel with the number.** (1) Three of 24 chunks hit the 600s cap, so 18
of the 72 sampled sites are absent — and the ones that time out are the slow ones, so the reading is
biased **optimistic**. (2) It is **not comparable** to the `PHASE0-ROADMAP-ANCHOR.md` §2 t380→t392
table: different keying (selector-path), different metric (parent-relative SHAPE vs absolute placement),
different corpus slice. This is a new baseline, not a delta. [[fidelity-instrument-shared-snapshot]]

## The class signature was making healthy pages read as 0% coverage (tick 550)

t549's sweep found 13 of 54 sites under 5% coverage and called it a class failure. **Two of the three
sites checked were not failing at all — the instrument was.**

The selector-path key carries a `.SIG` (an fnv-1a hash of the sorted, deduped class list) on **every**
component, so a path's identity is hostage to the class lists of *all* its ancestors — and class lists
are the single most JS-mutated thing on the web. Chrome's `gov.uk` body key is
`body.ba1d8e99:nth-child(2)` (its own JS adds `js-enabled`); Chrome's `nytimes.com` body key is
`body:nth-child(2)` (no class at all). **One differing class on `<body>` invalidates every descendant
key on the page, in either direction.** It is the Wikipedia `client-nojs → client-js` lesson, which this
repo already records in `tests/wpt/Cargo.toml`, arriving one layer up.

### The ablation, measured on six sites, decisive in both directions

| | sigs ON | sigs OFF |
|---|---|---|
| jvns.ca | cov 100.0% · shape 94.3% | **byte-identical** |
| blog.rust-lang.org | cov 100.0% · shape 87.4% | **byte-identical** |
| lobste.rs | cov 84.1% · shape 85.8% | **byte-identical** |
| **gov.uk** | **cov 0.0%** (418 of 418 missing) | **cov 82.8%** (72 missing) |
| **stripe.com** | cov 0.1% (1439 of 1441 missing) | **cov 43.1%** (820 missing) |
| nytimes.com | cov 0.0% (2381 of 2382 missing) | cov 0.0% — **unmoved** |

The signature adds **no discriminating power where the two DOMs agree** — three healthy sites are
unchanged to the decimal — and **destroys the measurement where one ancestor's class list differs.**
`nth-child` already distinguishes siblings uniquely, so the sig never carried identity; it carried only
fragility. It is off the key by default from tick 550. `MANUK_G1_CLASS_SIG=1` restores it, so the
decision stays auditable instead of becoming folklore. RED-PROVEN on live data both ways: default gives
`gov.uk` 82.8%, the restore flag gives 0.0%.

**nytimes.com did not move**, and that is the other half of the finding: it is a *genuine* second
failure, not the same bug. Had the sweep been read as one homogeneous "sub-5% class", the fix for
gov.uk-and-friends would have been credited with nytimes too, and the real bug would have gone back into
hiding. *A class of failures that shares a symptom does not share a cause until it is measured.*

### What this invalidates, said plainly

The **t549 certificate's coverage figures for the sub-5% class are wrong in the PESSIMISTIC direction**
and must be re-swept — the anchor's §6 t549 line carries that correction. The four jarring-invariant
percentages are less affected (they score sibling relationships within each side), but they were computed
over the intersection of keys, and the intersection just grew, so they change too. And
`run_oracle_cmd`'s crawl keys **still carry sigs** — the same correction is owed there, as its own tick.

The generalisable rule, because this is the second keying defect in twenty ticks: **an identity key must
not be built out of the mutable state of things other than the element it identifies.** Position in the
tree is structural; a class list is application state. [[fidelity-instrument-shared-snapshot]]

## The diff carries the COMPUTED FONT — and a rect-only diff could not have asked the question (tick 563)

By t562 every remaining text-metric lead was blocked on one missing datum. `martinfowler.com` reported
`[74×16] vs [76×18]` and that 2px could equally be **a different face**, **a different used size**, or **a
different line-box rule** — three different fixes, indistinguishable in a rect. So `oracle::Seen` gained
`font`: `"<resolved family>/<used px>"`, emitted by Chromium's probe from `getComputedStyle().fontFamily` /
`fontSize` and by ours from the resolved `FontFamily` plus the used size, recorded at the same point as the
box and printed on every instance. An **absent** font prints as absence, never `{/0}` — a fabricated zero
reads like a measurement, which is the failure this repo keeps catching in its own instruments.

**It paid on the first run, with two answers a rect could not give.**

**1 — same face, same size, different metrics.**
```
…/a:nth-child(37): [551 3126 51×16] {Open Sans/13}  vs  [112 3229 57×18] {Open Sans/13}
```
Identical `{Open Sans/13}` on both sides, and Chromium renders 51×16 where we render 57×18: ~12% wider, 2px
taller. So it is **not** face selection (fixed at t557/t558) and **not** font-size — it is the advance and
line box of the *same face at the same size*, which points at the **variant** (Open Sans ships as a variable
font; a different named instance has different advances) or at hinting/rounding.

**2 — a webfont Chromium loads and we do not.**
```
…/p:nth-child(3): [20 2029 293×20] {Lora/13}  vs  [20 1752 619×20] {serif/13}
```
Chromium resolves `Lora`; we fall back to `serif`. `fc-list` reports **zero** Lora faces installed, so
Chromium is fetching it from a declaration we are not seeing or not parsing. The `<p>` is **293px wide in
Chromium and 619px in ours** — a different wrap width entirely, which cascades to everything below it and
dwarfs the 2px line box.

**The pattern this is the sixth instance of:** the ranked cluster list was never wrong about *where* the
divergence was, only mute about *why*, and each brick that made the diff carry one more datum split one
"cause" into the two or three real ones underneath it — `.SIG` off the key (t550), `median_mag` (t552),
printed instances (t553), displaced-vs-mis-sized (t554), three instances (t555), the font (t563). **Make the
diff carry the datum the next question needs, then ask the question.** Twelve ticks of rect-only diffing could
only say "displaced"; one field said "a missing webfont and a variable-font variant". [[fidelity-instrument-shared-snapshot]]

## `curl` exits 0 on a 403, so the certificate could not tell a bot wall from a document (tick 611)

The certification design's §0 rule reads: *"a timeout/crash/bot-wall is a COUNTED outcome
(FAIL/EXCLUDED **with reason**), never a silent drop."* The counting half was built at t583. **The
reason half was not, and could not be** — the information was destroyed one layer below any code that
reports, in the probe's own fetch:

```rust
let out = Command::new("curl").args(["-sL", "--max-time", "25", "-A", UA, url]).output()?;
if !out.status.success() { bail!("curl failed for {url}"); }   // curl's PROCESS exit, not the HTTP status
Ok(String::from_utf8_lossy(&out.stdout).into_owned())
```

`curl -sL` without `-f` exits **0** on a 403. So the Cloudflare interstitial came back to the caller as
*the document*, and an `imdb.com` reply of **202 with a zero-byte body** came back as *an empty
document*. Neither is a page, and nothing downstream could discover that.

**Measured across the 20 HEAD sites of `corpus-v2.tsv`, all in one pass:**

```text
  200 with a body ......... 11        measurable
  403 Cloudflare .......... 5         tamildhool · mangago · supjav · fdown · quora
  202 zero bytes .......... 1         imdb.com
  transport failure ....... 3         docomo · pitc · fawanews
```

**Six of twenty answer with a status the instrument never read.** That is the bulk of the pilot's
*"9 of 14 could not be scored"*, and it is why "find out why" was not answerable from the instrument's
own output: every distinct way of failing to reach a page arrived at the report as the same bare `—`.

### The second mechanism, which a status check alone would not have found

A 403's body is not inert. Cloudflare's challenge page ships

```html
<meta http-equiv="content-security-policy" content="default-src 'none'; script-src 'nonce-…' …">
```

and the probe works by **injecting a `<script>` into the fetched HTML**. A nonce-based CSP blocks it:
Chrome parses the probe, refuses to execute it, and dumps a DOM in which the probe is present *as
text* and its output never existed. `parse_seen_probe_json` then fails with exactly the right words —
*"no `__PARITY__` probe output in dumped DOM (did Chrome run the script?)"* — and the caller was

```rust
if let Ok(mut cseen) = capture_seen_all_paths(url, vw, vh) { … }
```

which throws that sentence away. **An `if let Ok` is how a diagnostic that already exists goes
unheard for five ticks.** The error type is now `Unmeasurable`, which an `if let Ok` cannot silently
discard, because the caller has to say what it did with it.

### A refusal is not a rendering result — and this is the sharp edge

t607 landed the complementary truth for the ENGINE: **an HTTP error status IS a document, and 403/404/
429/500 pages must render**, because the user has to see them. Both are correct at once, and the
distinction is the whole tick:

> **The browser renders the 403. The certificate refuses to count it as evidence about the site
> behind it.**

The harm is not hypothetical, and it is worse than a missing label. A challenge page is a *real*
document that **both engines render, and they agree** — so left on the ordinary path it scores as
**high fidelity on a site we never reached**: a gate passing by comparing a refusal against itself.
`certificate()` therefore skips a row carrying a reason *before* it reads any score, and says so, since
until now "unscored" was true only by accident of control flow — a failed probe left `shape` at `None`
— and the accident was one edit away from reversing.

### And the denominator had a hole the whole time

The sweep loop's chrome-capture arm was `eprintln!; continue`. A site we could not reach **left no
row, so it left the denominator too**, and `sites N` silently shrank by however many origins refused
us that day. Demonstrated by restoring the `continue` on a two-site sweep:

```text
  with the drop      sites 1 · scored 1        ← 100% of the corpus scored
  counted            sites 2 · scored 1 — 1×bot-wall-403
```

That is §0's cause #1 — *dropping the hard sites is what made every past reading optimistic* —
reproduced live, inside the instrument built to prevent it, 28 ticks after the rule was written.
**A rule enforced at one layer is not enforced.** The same shape as t610's `run_with_fetcher` (one
drain bound, two implementations, one enforcing it) and t591's `@supports` (one fix, one of two
lists), three sessions running.

### What the report says now

```text
  3 of 4 sites UNSCORED (cannot be claimed, counted against the bar) — 2×bot-wall-403, 1×empty-202
```

Each cause is owned by a different part of the project, which is the point of splitting them: a **bot
wall** is the identity/fingerprint axis and no amount of rendering work moves it; an **http-404** is
corpus construction; a **probe-blocked** is the measurement channel; an **empty body** is neither. A
single total cannot be worked. And a site unscored with *no* recorded reason is itself printed as a
shortfall — *"the instrument could not say why, which is an instrument gap, not a result"* — so the
decomposition can never look complete merely because the explained causes are the only ones listed.
[[certification-redesign]] [[reliability-doctrine]]

## The oracle renders a SHELL for JS-built pages, and the certificate was scoring it (tick 614)

The fidelity probe feeds both engines **one fetched copy, served from a `file://` temp file.** That is
deliberate and defends a real invariant, recorded in `chrome.rs`: point Chrome at the live URL instead
and the *two Chrome probes* render different pages — Wikipedia's origin injects a fundraising banner a
local copy never sees, which once pinned a metric at 5,122px across four correct fixes.

**The cost of that choice was never priced.** From `file://` the page's own origin is `null`, so a
JS-rendered site's `fetch`es and module loads are cross-origin and blocked, and Chrome builds almost
nothing:

```text
  comix.to    file:// snapshot (what the instrument scores)    28 elements ·  4 with a box
              live navigation                                ~2643 tags
```

A **94× gap** — and the certificate scored the small side of it. `comix.to` reported
**`coverage 66.7%`**, computed over **three elements**, printed in the same column and the same units
as `bbs.ruliweb.com`'s 4,122-path score. That is not a measurement of comix.to; it is a measurement of
comix.to's pre-hydration shell.

### Naming it, not fixing it

`Unmeasurable::ShellOnly(n)` carries the element count that proves the condition. The threshold is
`CERT_MIN_SHAPE_SAMPLE`, **reused rather than invented** — the certificate already refused to score a
placement ratio computed over fewer than ten elements, so **no verdict changes**. What changes is that
the refusal states its cause, which was the whole of t611's *"unscored with NO recorded reason"*
residue.

```text
  comix.to        UNMEASURABLE [shell-only-3]
  www.naukri.com  UNMEASURABLE [shell-only-1]
```

Fixing the oracle is a separate question and deliberately not answered here — see below.

### The vacuous rows were inflating the headline

`shape_stats` returns `1.0` over an empty sample, so a shell contributed a *perfect* placement score to
the mean. Recomputed from the t611 sweep's own saved output:

```text
  comix.to      100.0%  n=2   ← vacuous       desitales2   63.0%  n=598
  welt.de         0.0%  n=1   ← vacuous       agoda.com     7.7%  n=13
  naukri.com    100.0%  n=0   ← vacuous       keirin.jp     9.9%  n=503
  ebay.com        0.0%  n=4   ← vacuous       ikea.com     51.7%  n=698
                                              ruliweb      53.1%  n=4091

  MEAN SHAPE as reported (9 rows)   42.8%
  MEAN SHAPE over n>=10 only        37.1%
```

**5.7 points optimistic**, from four rows scored over 0–4 elements. All three headline means
(`MEAN VISUAL`, `MEAN COVERAGE`, `MEAN SHAPE`) now take the **same site set the certificate does** —
otherwise two numbers printed three lines apart are computed over two different populations, which is
the accounting mismatch of `THE SEVEN META-INSTRUMENTS` #3.

### The open question, stated rather than smuggled

**Can this oracle measure a JS-rendered page at all?** `file://` protects one invariant (both Chrome
probes see one document) at the cost of another (the document is the one a user would actually get).
Serving the snapshot from a local `http://127.0.0.1` origin would satisfy both and is the obvious
candidate — but it changes what **every past number meant**, so it needs its own tick with a
before/after on the same corpus, not a quiet swap. Until then the honest position is the printed one:
those sites are UNMEASURED, with a reason. [[certification-redesign]]

## A diff field must measure the same quantity on both sides (tick 627)

t563 added a font field to `oracle::Seen` so a rect could say which face and size produced it —
*"`[74x16] vs [76x18]` is unattributable without it"*. The two sides recorded different things:

```text
  Chromium   getComputedStyle(e).fontFamily.split(',')[0]     the first DECLARED family
  Manuk      fonts.resolved_family_name(&st.font_family)      the RESOLVED family
```

`getComputedStyle` **cannot** report the face actually used, and no DOM API can. So the field compared
a declaration against a resolution, and therefore differed on essentially every element of every page
that ships a font stack — which is every page. The output looked like a finding:

```text
  {-apple-system/17}  vs  {sans-serif/17}
```

and reads as *"Chromium used the system font, we fell back"*. Chromium on Linux has no `-apple-system`
either and falls back exactly as we do. **The field manufactured a plausible false cause and put it at
the top of the ranked root-cause list — which is exactly where the next tick looks.**

The comment above it asserted the property the code did not have: *"resolved family name (not the
declared stack) and used size, **so the two are comparable**."*

### The fix, and what the field can honestly answer

Report the first **declared** family on our side too. The field now answers *"did the two engines'
CASCADES arrive at the same font-family declaration for this element?"* — a real question with a real
failure mode. It does **not** answer *"did they use the same face"*, and cannot: that needs a channel
Chromium does not expose.

After the change, every one of those clusters reads `{-apple-system/14}` on both sides, and the cause
that was underneath becomes visible:

```text
  30 hit(s)  geometry/mis-sized: height ~8px  (<path>)
    …svg/path:nth-child(2): [40 649 10×9]  vs  [40 744 0×22]
```

A zero-width SVG `<path>` — specific, attributable, and previously hidden behind a font signal that
does not exist.

⚠ **Retroactive:** t563's `{Lora/13} vs {serif/13}` is the same shape. The box widths there (293 vs
619) were a genuine divergence, but the attribution *"a webfont Chromium loads and we do not"* rested
on this field and must be re-derived rather than inherited.

**The general rule:** before trusting a per-element diff field, check that both producers compute the
same quantity. A field that is cheap to add on one side and approximated on the other does not fail
loudly — it fails by generating confident, wrong, well-ranked leads.

## The load budget trades COVERAGE against SHAPE (tick 632)

`keirin.jp`, the same tree, the same corpus, one environment variable apart:

```text
                       default (12s)        MANUK_LOAD_BUDGET_MS=60000
  load                 17.8s                27.9s          (chromium 6.0s)
  SHAPE                10.4%                34.3%
  median dx / dw       202 / 175            0 / 0
  COVERAGE             83.3%                58.0%
```

**More time buys placement and costs presence.** With 60s the stylesheets apply — the median x and
width offsets collapse to **zero** and shape more than triples. With 60s the document also ends up with
**174 fewer boxes**, because more of the page's own JavaScript runs (on this site, plausibly its
unsupported-environment path hiding content: 「現在お使いの環境では当ページを正常に表示することができません」
is in the served HTML).

**So the budget cannot be tuned to improve the certificate — only to choose which half to flatter.**
A short budget under-reports placement; a long one under-reports presence. There is no setting that is
honest for both, which means **neither number is a pure property of the engine's layout**.

t602 promoted performance to *"a fidelity input, not a comfort metric"* on the evidence that a page
painted incomplete scores as a layout failure. This is that claim measured, with its opposite half
attached. Practical consequence: **`MEAN SHAPE` should be read as a lower bound contaminated by our own
latency** until the latency is fixed — we take 17-28s where Chromium takes 6, and `OURS IS SLOW` fired
on 10 of 14 sites at t606.

⚠ **Do not "fix" this by raising the budget.** It would raise shape, lower coverage, and settle nothing
— the definition of tuning an instrument to produce a preferred number.

## The fix whose gate passes before the fix (tick 637)

A small correctness fix looked obviously right and was reverted unshipped. The reasoning generalises
past the unit it was about.

**The setup.** CSS `ic` is defined as the used advance of U+6C34 (水). Stylo computes it from
`metrics.mIcWidth`; our `FontMetricsProvider` returns `FontMetrics { zero_advance_measure, x_height,
cap_height, ..Default }` — `ic_width` never set — so Stylo assumes `1em` unconditionally. The spec
permits `1em` only *"where it is impossible or impractical to determine the ideographic advance
measure"*, and it is neither: the shaping context that measures `0` for `zero_advance_px` measures
水 for the price of a different string. This is precisely the shape `cap` was in before t507, when
filling it stopped `cap`-sized boxes collapsing to zero. Writing `ic_width_px` took minutes.

**Why it was reverted.**

1. **The gate would pass before the change.** Every font on the machine measures 水 at *exactly*
   1em — CJK faces are designed full-width, so `1ic == 1em` is the normal case, not a coincidence.
   No available fixture makes computed differ from assumed, so the assertion cannot fail the
   unfixed engine.
2. **And it would have been actively worse.** Latin-only faces (`sans-serif`, `DejaVu Sans`)
   returned `Some(16.0)` — they have **no 水 glyph**, so that number came from the fontconfig
   *fallback chain*, not from the styled face. Shipping it would replace a principled spec fallback
   with a fallback-chain artifact that merely coincides with it today.

> **A fix that cannot be distinguished from its absence is not a small win, it is a claim.** The
> operational form of *"would I make this change if it did not help me land?"* is: **what does its
> gate look like?** If the honest answer is *"it passes before the change"*, the deliverable is the
> finding, not the diff.

**The disambiguating control is worth reusing.** `10ic` measuring exactly `10em` is consistent with
two opposite stories — *the unit is unsupported and the declaration was dropped*, or *the unit
resolved to the spec's 1em fallback*. A **bogus-unit control** separates them in one line:
`width: 10zz` is dropped to `auto` (full container width) while `width: 10ic` is 160px, so `ic`
parses. Whenever a "missing" feature's output coincides with a plausible fallback, the control is a
declaration the parser must reject.

**Recorded as `partial`, with the re-pricing condition named**: a proportional CJK face where
水 ≠ 1em is what would make this measurable. An honest *"cannot know"* rots invisibly precisely
because it is documented as intentional, so it must carry the condition that brings it back.

## Two citation dialects, and each instrument was blind to one (surface audit #36, tick 638)

The map (`CONSTELLATION.tsv`) cites the gate backing each capability claim. Two instruments check
that citation, and between them they left a hole that eleven claims sat in:

* `map-reconcile.sh` validates only tokens matching `G_[A-Z0-9_]+`. A row citing **`g_a11y_roles`**
  in lowercase is not a token it recognises, so it filed the row as `descriptive-floor` — prose.
* The gate-directory diff (`ls engine/page/tests/`, uppercased, against the map's `G_*` tokens)
  compares uppercase, so it counted **the same gate** as unmapped.

Each instrument's blind spot was the other's input format. Uppercasing the citations moved
machine-validated claims **259 → 271** with drift still 0 — and that zero is the evidence they were
real all along: a wrong citation surfaces as drift the moment it becomes visible.

> **A claim no instrument can read is not a weak claim, it is an unaudited one.** From every
> direction anyone actually checks, it is indistinguishable from a strong one.

**Two further blind spots worth keeping in view:**

1. **`map-reconcile.sh` searches `engine agent tests`, not `shell/`.** Seven gates live as
   `#[test] fn` inside `shell/src/media.rs` (`g_avif_paint`, `g_av1_drive`, `g_media_drive`,
   `g_mp3_drive`, `g_muted_out`, `g_idl_feed`, `g_webm_av1_drive`). A row citing one of them is
   **true and unverifiable at the same time**. Handled by naming an engine-side equivalent where one
   exists; the script itself is harness-owned.
2. **A capability can have no row at all.** `URLPattern` had a passing gate and zero occurrences in
   the map. That is not a wrong claim but an *absent* one — the failure mode a map structurally
   cannot report on itself, and the reason the gate-directory direction of the diff is not optional.

**And the timing lesson.** Three media rows were stale by the same author, from the same session, and
none was caught by anything that reads the map — all three surfaced from the gate side. Landing a
capability and updating the map are two actions, and the second is skipped **by the person who just
performed the first**, because to them the capability is now obviously present.

## Ranked by area is not ranked by sites moved (constitution check #46, tick 639)

`CONSTITUTION.MD` PART VII.1 ranks work by **"real sites moved per fix, verified against the oracle
corpus."** Eight ticks satisfied that rule *in intent* and produced **no site measurement at all**.

Three consecutive media capability ticks — WebM demux, AV1-in-WebM, `mediaCapabilities.decodingInfo`
— were evidenced entirely by **fixtures**: two `.webm` files and six contentType strings. Media is
unambiguously the right area (PART VII.1 names depth on the handful of destinations where people
spend time, and this is the YouTube story). But area is not the ranking term, and *"this unblocks
YouTube"* is plausible rather than measured.

**The mechanism is compounding local reasons, which is why the existing rule did not catch it.** Each
tick had a good reason to skip the sweep: it costs ~45 minutes, the capability is obviously present,
a fixture is more precise than a live site. Every one of those is true in isolation. Three of them in
a row produce a class of work with no corpus evidence behind it.

> **A capability arc must produce ONE real-site measurement before it produces its fourth rung.** Not
> per tick — per arc. Otherwise *"ranked by real sites moved"* degrades into *"ranked by which area
> real sites are in"*, which is a much weaker rule wearing the same words.

The general shape is worth keeping separately from the media case: **a rule that is checked per-unit
can be satisfied by every unit and violated by the sequence.** Any rule of the form "prefer X
evidence" needs a cadence attached, or the preference is expressible entirely as intent.

## Drift concentrates on the oldest question marks (tick 645)

The constellation's last four `?` cells were resolved. **Three were map drift, and two of them by a
wide margin** — they were the project's two headline external claims:

| row | the map said | reality |
|---|---|---|
| `test262` | `?` since audit **t83**: *"we have NEVER RUN IT"* | **run at t546**, 99 ticks earlier, by `tests/wpt/src/test262.rs` in our own tree — 94.14% of 87,009 executed, 81.41% of the 100,617 defined |
| `100-tab RSS` | `?`: *"the memory thesis rests on zero data"* | **run at t571**, 74 ticks earlier — median 0.90 MB/tab, p90 49.7 MB, aggregate 4390 MB |
| `audio output device` | `?` | `AudioOut` (cpal) plus three gates, all in `shell/src/` |

> **Map drift is not uniform across a map. It concentrates on the rows that have been `?` the
> longest**, because a long-standing question mark stops being read as a question. Nobody re-checks
> the row everybody knows is unknown — and the longer it sits, the more it reads as a settled
> property of the project rather than an open measurement.

The practical consequence for the audit: **sort the unknowns by age and start at the top.** The
oldest `?` is the most likely to be stale, which is the opposite of the intuition that old unknowns
are old because they are hard.

**The fourth was a different failure, and a subtler one.** `apply_prop`'s doc said `playbackRate` was
*"accepted and DROPPED here, deliberately"* — true when written, and **falsified one tick later** by
code three lines below it.

> **A stale comment describing a deliberate NON-implementation is the hardest kind to notice, because
> it reads as a decision rather than an omission.** *"This does X"* gets checked by anyone editing
> nearby. *"We deliberately do not do X"* is read as settled and skipped. It is
> `[[honest-answer-is-not-a-fixed-answer]]` living in a comment instead of an assertion — where
> nothing can go red.

That probe produced a **split verdict** rather than a flip, which is usually the honest shape: the
rate reaches the clock (gated) and not the audio (`AudioFeed` has no rate control), so a podcast at
1.5x would drift against its own sound. Recorded `partial` with the missing half named, not `gated`.

## An unverified MSRV is a claim with teeth (tick 648)

Re-taking the Opus decision surfaced a blocker that has nothing to do with audio:

```text
error: no version of crate `opus-decoder` can maintain manuk-media's rust-version of 1.80
help:  pass --ignore-rust-version to select opus-decoder@0.1.1 which requires rustc 1.85
```

`rust-version = "1.80"`. The local toolchain is **1.88**. CI uses `dtolnay/rust-toolchain@stable`,
which is ≥1.88. **Nothing anywhere builds this workspace at 1.80, and nothing checks that it could.**
The manifest's own comments give it away — the `openh264` pin reasons *"this workspace is on 1.88"*
and the `avif-parse` pin *"toolchain here is 1.88"*, both in a file that declares 1.80.

> **An unverified `rust-version` is not a conservative promise — it is a claim nothing can falsify,
> and unlike most such claims it has TEETH.** Cargo enforces it at *resolution*, so it silently
> narrows the dependency set for a compatibility nobody tests, nobody ships and nobody has ever
> observed. It is the `@supports`-answering-*"does it parse"* defect wearing a manifest field: a
> check that genuinely runs, and answers a different question than the one everyone reads it as.

The general shape is worth separating from Rust: **a declared constraint that no build exercises is
indistinguishable from a comment, right up until a resolver enforces it.** Version floors, feature
minimums, platform lists and `engines` fields all behave this way. The test is not *"is the number
plausible?"* but *"what would fail if it were wrong?"* — and if the answer is *nothing*, the number
is decoration that can still cost you a dependency.

**Two legitimate resolutions, and picking the convenient one is not among them:** verify the floor by
building it, at which point blocking a dependency is a real trade-off — or set it to the version
actually built and tested, and let resolution reflect reality. Replacing an unverified 1.80 with an
unverified 1.85 because that is what today's dependency wants is not progress; it is the same defect
with a newer number.

## The instrument must survive the engine dying under it (tick 650)

The standing debt was a number: **the certificate had not been measured since t626**, while jQuery,
DOMPurify and htmx went from silently dead to working. Three HEAD-20 runs were started to get it.
**Two were killed mid-corpus by an engine SIGSEGV** — one at site 5, one at site 11 — and neither
produced a certificate, a row file, or any record of the ten sites that had already been measured.

The rendering half was not the blocker. **The instrument was.** `--rows-out` appended once, *after*
the loop, so every completed site was hostage to the survival of the whole run:

```text
  run 1  crashed at site  5 of 20   →  /tmp/cert-t650-rows.tsv   ABSENT
  run 2  crashed at site 11 of 20   →  /tmp/cert-t650e-rows.tsv  ABSENT
```

**This exact harm was already written down, one level out.** `Unmeasurable::Timeout`'s doc comment
says it: *"the sweep runs sites in ONE process, so an unbounded child stalls the WHOLE corpus and the
run yields no certificate at all — the sites that already finished are lost with it, which is
strictly worse than the silent drop the fixed-denominator rule exists to prevent."* t625 closed that
for a **child process we invoke**, by bounding it. The case where **we** are the process that dies
was left open, because the fix that worked there — a deadline on somebody else — has no analogue
when the corpse is your own.

> **A fixed denominator protects you from dropping the hard site. It does nothing about dropping the
> whole run, and the hard site is exactly the one that drops it.** The two failures are the same
> failure at different scales, and closing one reads as closing both.

The fix is durability rather than prevention, because the crash is not ours to prevent today:

1. **Append as earned, not at the end.** The flush happens at the **top** of each iteration — the
   loop body has five `continue` paths and a bottom-of-loop flush would silently skip every one.
   Invariant: before site N touches the engine, sites 1..N-1 are on disk.
2. **A crash is a COUNTED outcome** (`Unmeasurable::Crashed`). A process that dies writes nothing, so
   the crash has to be recorded *before* the work: an in-flight sidecar names the site being
   rendered, and the **next** run turns a leftover marker into a counted row. Otherwise the site that
   kills the sweep is the one site guaranteed to escape the denominator — and it is the hardest site
   in the corpus.
3. **Resume supersedes, never accumulates.** `certificate()` reads `sites` straight from
   `rows.len()`, so re-running a crashed sweep would have grown the corpus: 3 sites became 6. Rows
   dedup by name, **last wins** — which also means a site that crashed once and then rendered stops
   being `crashed`. A denominator nobody chose is the defect whichever direction it moves; inflating
   it is not safer than shrinking it.

RED-proven three ways, and the third is the one that matters: the unit gate covers (2) and (3) by
mutation, but it drives the ledger functions directly and **cannot see the wiring in `main.rs`, which
is what actually changed.** So that half was proven end-to-end on the real binary — `kill -9` during
site 2 of 3, then resume:

```text
  killed mid-site-2  →  rows.tsv holds site 1 · marker names site 2
  resumed            →  "RECOVERED [crashed]: example.org"
  file has 5 physical rows  ·  certificate --rows reads  sites 3
```

**The reusable shape:** when an instrument and its subject share a process, every reliability
property of the subject silently becomes a *measurability* property of the instrument. The engine's
crash rate was a rendering concern until it became the reason no number could be produced at all.

## The remembered reproducer that no longer reproduces (tick 650)

The crash above matches `[[calc-size-interpolate-size-segfault]]` on four independent marks:
release-only, threshold-dependent on allocation churn, every gdb frame `?? ()` inside statically
linked SpiderMonkey, NaN-boxed GC values (`0xfff9…`/`0xfffe…`) on the faulting stack and **no frame
of ours anywhere on it**. Four matches is enough to file it under a known, deliberately-parked Bar-0
and move on.

**The control says otherwise.** That note carries a reproducer described as deterministic, so it was
run — both named files, plus the six around them:

```text
  css/css-values/calc-size/animation/interpolate-size-interpolation.html   rc=0
  css/css-values/calc-size/animation/interpolate-size-computed-*           rc=0
  …8 files, start=0..7                                                     rc=0  (all)
```

Every one clean. The reproducer that made t126 actionable **is dead**, so the memory cannot certify
today's crash as t126's, and "known open Bar-0" is not available as a reason to park it. What is true
is narrower and worth stating in exactly its own size: *an unattributed mozjs heap-corruption crash
is live on the real-site path, it takes ~10 sites of allocation churn to fault, and it still needs
ASAN.* It is not covered by any current reproducer.

> **A parked bug's REPRODUCER rots faster than its diagnosis, and the diagnosis is what gets
> remembered.** Signature-matching against a parked ticket is how a live crash gets filed as an old
> one; the reproducer is the only part of the note that can be checked, so check it before inheriting
> the verdict. Four matching marks argued for the memory. One command falsified it.

## 100% of nothing is 100% (tick 651)

`www.agoda.com`, a top-100k HEAD site, renders as a **completely blank white page** in this engine.
The certificate reported it as:

```text
  page                       visual   COVERAGE  missing misplaced verdict
  www.agoda.com               69.9%   100.0%        0        13      ok
```

Four independent guards were in position and every one of them missed it, each for its own reason:

| guard | why it missed |
|---|---|
| **COVERAGE 100%** | computed against the oracle's `file://` probe, which built a **13-element shell**. We had all 13. |
| **`ShellOnly`** | counts the oracle's SAMPLE SIZE, and 13 clears the floor — the guard written for this shape of lie missed it by a threshold. |
| **VISUAL 69.9%** | most of a page is background either way. The instrument's own caveat (*"an absent sidebar moved it <1 point"*) arriving as a false PASS. |
| **SHAPE 0.0%** | did fire — but it is reported as a *placement* number, so it sends you hunting a layout bug on a page that has no layout. |

> **They are all DOM-side measurements, and the DOM was fine.** The elements exist, they have boxes,
> they compare. No amount of care on that side can see that the paint is empty.

The fix is not a better DOM check; it is the **other population**, which §6.4 already asks for and
which was sitting unused: both screenshots are in hand when `compare()` runs. **Ink** — the fraction
of blocks differing from the image's own modal block colour — is deliberately *self-relative*, so it
asks *"did this engine draw anything"* rather than *"is this page white"*: a dark theme is not blank,
and a wrong background is not blank.

The thresholds are **measured, not chosen** (t650 HEAD-20): `agoda` **0.00%** against every genuine
render at **≥1.07%** (`aparat` 1.07 · `keirin` 8.11 · `desitales2` 22.0 · `ebay` 25.6 · `ikea` 33.6 ·
`welt` 76.1). A gap that wide either side of the line is what makes it a constant instead of a knob.

**The two guard cases carry the design, and a blank-detector without them is worse than none:**

- The **oracle** must clear 10% ink for our 0% to mean anything — `comix` (0.00% Chrome-side) and
  `naukri` (1.92%) are oracle shells, and reporting those as our render failures is a false accusation.
- A page rendered **badly** is not a page rendered **not at all**: `keirin` at 8.11% ink / 2.2% shape
  must stay SCORED. **Excusing our worst real renders as unmeasurable is how a certificate launders
  its own failures** — the same disease as the false pass, pointed the other way.

**The general shape:** when a metric's denominator comes from the thing being measured, it can be
satisfied by measuring less. Coverage over the oracle's shell, a ratio over 13 elements, a pixel
score dominated by background — each is a number that gets *easier* as the page gets *emptier*. The
only reliable defence is a second population that fails differently, not a stricter threshold on the
first.

## The rule could name the oracle failing us, and had no word for us failing (tick 653)

A site is refused a placement score when there are too few elements to compare. There are **two ways**
to get there, they are refused by the same floor, and only one of them had a name:

| | who failed | named? |
|---|---|---|
| the oracle's `file://` copy built almost nothing (`comix.to`: 3 elements) | **the oracle** — not our bug, not evidence about the site | `ShellOnly` ✅ |
| the oracle built the page and **we** rendered 16% of it (`ebay`: probed 25, common 4) | **us** | *nothing* ❌ |

So `ebay` went out **unscored with no reason**, and the certificate printed its own shortfall about
it: *"the instrument could not say why, which is an instrument gap, not a result."* That row had been
open since t614, was carried by t626, was driven to zero at t650 — and came straight back at t653.

**The cause is that the check was asked too early.** It ran at the producer on `probed` alone, before
the comparable count existed in the function, so it *could not* ask "did we render the page?" — only
"did the oracle?". A check placed where half the evidence is missing answers the half it can see, and
looks complete doing it.

> **An asymmetric rule is invisible while the asymmetry matches reality.** The oracle is the thing
> that usually breaks, so a rule that can only blame the oracle passes review for 39 ticks. It fails
> the first time *we* are the broken one — which is the case it most needed to catch.

`unscoreable_reason(probed, common)` now decides both in one place, retiring a second implementation
along the way (*one rule, N implementations* again). The two reasons blame opposite parties, which is
the point: `ThinOverlap` is **ours**, and reads as a coverage failure wearing an *unscored* label.

⚠ **The live re-run of `ebay` produced a THIRD outcome** — `timeout-300s`, after `probe-blocked` at
t650 and `thin-overlap` at t653. So the gate asserts against the **recorded** numbers from the sweep
row, never a re-fetch: *a gate whose expected value comes from today's network is a gate that measures
the network.*

## A live site's fidelity score has an error bar, and it is bigger than most of our deltas (tick 657)

**Every per-site fidelity comparison this loop has made was a single reading against a single
reading.** Tick 657 measured what that is worth. Two live HEAD sites, re-rendered three more times
each on **one unchanged tree**:

```text
  keirin.jp      0.4044  ->  0.3972  ->  0.3673      spread 3.7 pts    n 497 / 496 / 490
  www.ikea.com   0.5186  ->  0.5158  ->  0.5158      spread 0.3 pts    n 698 / 698 / 698
```

`keirin.jp` varies by **3.7 SHAPE points between consecutive runs of the same binary.** The tick was
one paragraph from reporting a 0.7-point "regression" caused by the previous two ticks — a number
five times inside the site's own noise.

### Why a live site is not a fixture

A fixture is entitled to a single reading; a live site is not. Its content, its ads, its A/B bucket
and its node count move underneath the measurement — `keirin`'s scored population drifted 497 → 490
across three consecutive runs, and `welt.de` has read 2957 / 3060 / 3092 / 3149 across four sweeps.
The score's denominator is *the thing being measured*, so a page that changes changes the score
without anything in the engine changing at all.

**The rule this yields, and it is not "run more sweeps":**

> **A per-site delta smaller than that site's own spread is not a small result. It is not a result.**

The site's spread is not a global constant either — `ikea` is 0.3 points and `keirin` is 3.7 on the
same day. The error bar is **per site**, and it has to be measured on the same tree as the delta.

### This does not retract the deltas that were real

Tick 654 moved `keirin` **+38.6 points**. That is ten times the spread measured here, which is
precisely what makes it a result and the three sub-point deltas after it not. The distinction was
always the error bar; the loop simply did not have one, so it could not tell a finding from a
fluctuation and had no way to know which it was looking at.

### The instrument now carries it

`rows_from_tsv` collapses repeated rows for a site to the **last** one. That is the right tie-break
for a resumed or chunked sweep (see its own doc comment — the denominator must not grow every time a
run is resumed) and it **throws away the only evidence of the spread**, which is why the number above
had to be rediscovered by hand from a file that already contained it. *(Tick 673 split that rule: a
**consecutive** run of repeats now collapses to its median, and last-wins governs only **separated**
rows. See "the spread was printed for fifteen ticks" below.)*

`fidelity::shape_spreads` reads the accumulated rows and `certificate --rows` prints the block
**above** the certificate — deliberately, because a reader who sees the headline first has already
formed an opinion about a delta:

```text
  ⚠ INSTRUMENT SPREAD — sites this file measured more than once:
      keirin.jp                    0.3673 .. 0.4044   Δ 3.7 pts over 4 runs
      www.ikea.com                 0.5158 .. 0.5186   Δ 0.3 pts over 4 runs
    A per-site delta smaller than that site's own spread is NOISE, not a result.
```

**An unscored row contributes nothing.** A site that rendered once and bot-walled once was measured
*once*; parsing `-` as a score would manufacture a spread covering the site's entire range out of a
row that never carried a number. `spread_tests` asserts both halves — the range is the measured
min..max, and unscored rows and single readings yield nothing — and is RED-proven by collapsing the
repeat check, at which point every site reports a range of zero. **A spread that silently becomes
zero is a noisy number starting to look like a precise one**, which is the failure this whole block
exists to make impossible.

### The spread was printed for fifteen ticks and nothing consumed it (tick 673)

The block above is a *report*. It sat four lines above a certificate whose per-site rows were still
single draws, and for fifteen ticks that was fine — until tick 672, when the sweep drew `keirin.jp`
at **0.048 against a ~0.40 population** and the certificate published it. Three controls on the same
tree minutes later read 0.400 / 0.351 / 0.402. The next sentence being written was a **35-point
regression report aimed at the previous tick's own work**, and the only thing that stopped it was a
human reading the spread block and choosing to run a control.

> **A measurement whose error bar is computed, printed, and not acted on is a decoration.** The
> instrument knew the number was a draw. Nothing downstream was allowed to know.

**The fix is two halves, and neither works without the other.**

**1. The sweep repeats the sites its own spread says are unstable.** `fidelity::repeat_plan` reads
the accumulated rows file and names every site whose recorded spread exceeds `SPREAD_UNSTABLE_PTS`
(**5.0**); `repeat_urls` expands the corpus so those sites are rendered `UNSTABLE_REPEATS` (**3**)
times **back to back**. On the real HEAD-20 rows that is two sites — `keirin.jp` (Δ 34.9) and
`www.agoda.com` (Δ 7.7) — for four extra renders on a twenty-site sweep, against a blanket triple-run
that would cost forty. The five deterministic sites (Δ ≤ 0.3) are left alone.

The threshold sits between two measured populations rather than being tuned: every spread this
project has recorded is either ≤ 3.7 points or ≥ 7.7. It is deliberately *not* set at keirin's own
3.7-point calm range, because **a spread only costs the certificate something if it can change a
term** — all of keirin's calm readings sit the same distance below the 0.75 floor, so repeating that
site to resolve a 3.7-point wobble would buy precision nobody reads.

The plan is **monotone**: a site that has ever produced a wide draw keeps its repeats. One calm sweep
is not evidence the tail is gone, and the two costs are not symmetric — being wrong that way costs
two renders, being wrong the other way costs a phantom regression aimed at the last tick's work.

**2. `rows_from_tsv` collapses a CONSECUTIVE run to its MEDIAN.** This is the half with teeth, and it
required splitting one rule into two, because *repeat* and *re-measure* are different events that the
old single rule ran together:

| repeats are… | what they mean | which row survives |
|---|---|---|
| **consecutive** (what `repeat_urls` produces) | `n` draws from one distribution, one tree | the **median by shape** |
| **separated** by other sites (a resumed or crashed sweep) | a **re-measurement** superseding an earlier attempt | the **last** |

Last-wins is still right for the second row — it is how a recovered `crashed` row is superseded once
the site renders — and it is exactly wrong for the first, where it hands the certificate whichever
draw the sweep happened to finish on. Only scored draws vote; an even run takes the **lower** middle,
because a bar must never be cleared by a rounding convention.

**RED-proven, with the real numbers.** `repeat_tests` feeds keirin's three actual readings in the
order that hurts — `0.400, 0.402, 0.048`, outlier last — and asserts the collapse reads **0.400398**.
Disabling the median collapse reproduces tick 672's phantom exactly: *"the certificate took 0.047800
for keirin.jp."* Four of the module's eight tests go red on that one mutation, and the plan half goes
red independently when the threshold is moved.

**And the change broke the denominator in the same breath.** The first live run of the repeat plan on
a two-site corpus printed **`sites 4`** — the fixed-denominator rule, cause #1 in the certification
design's list of historically flattering numbers, broken by the tick that was fixing the numerator.
The sweep printed its certificate from the rows it had just built while the reader collapsed the
file, so the two paths disagreed the moment repeats existed. Both now go through one public
`collapse_repeats`, and a reconciliation test asserts they reach the same denominator and the same
per-site score.

> Nothing caught that but the accounting. **8 of 30 process defects in this project were caught by a
> number that did not add up, and not by any gate** — this is the ninth, and it was found by reading
> the output of the change rather than by trusting that the tests covering the new rule also covered
> the old one.

### The general form, which is the fourth time this project has paid for it

> **Every number has a harness, and the harness is part of the number** (`STATUS.md`, Lesson 4).

This is that lesson in its measurement-noise form. The earlier three were about *whose* time a
metric charged; this one is about *how precise* a metric is entitled to sound. Both are the same
question — what else moved? — and the answer here was: the website did.

## The oracle's probe never waited for the page to render (tick 674)

Two of the certificate's twenty rows read `shell-only` — *"the ORACLE rendered a shell, so there is
nothing to compare against"*. The reason's own doc comment named the cause:

> from `file://` the page's own origin is `null`, so a JS-rendered site's fetches and module loads
> are cross-origin and blocked, and Chrome builds almost nothing.

Plausible, load-bearing, and **never measured**. It would have bought a loopback HTTP server.

### One probe killed it

Same fetched document, same Chrome, same flags, two origins:

```text
  comix.to        file://              3 elements   (dom 377625 bytes)
                  http://127.0.0.1/    3 elements   (dom 377625 bytes)
  www.naukri.com  file://              4 elements   (dom 112900 bytes)
                  http://127.0.0.1/    4 elements   (dom 112951 bytes)
```

Byte-identical. **The origin is not the mechanism.**

### The funnel named the real one in one field

377KB of dumped DOM and **29 elements total**, 26 of them `<script>`/`<meta>`/`<link>` — the bytes
are inline script text, not content. And the field that ended it: **`readyState: loading`**.

The probe is injected at the end of the document and runs *synchronously during parse*, so it reports
the DOM before DOMContentLoaded — before any deferred script, module, or hydration. `PROBE_JS`'s own
doc comment said so out loud: *"runs synchronously at end of body, after layout, so it needs no load
event."* That is **true** of the static parity fixtures it was written for, and **false** of every
live JS-rendered site the fidelity sweep later pointed it at. The sentence was correct where it was
written and became a bug where it was inherited.

Snapshotting one page at five moments:

```text
                      PARSE     DCL     LOAD   T+2000   T+5000
  comix.to                3       4        5        6        7
  www.naukri.com          4      37       59       60       61
  www.welt.de          3199    3200     3177     3201     3176
```

The asymmetry is the whole case. `naukri` gains **15×**; `welt.de` — server-rendered, one of the five
rows currently carrying the certificate — does not move at all. The fix converts the unscoreable
population and leaves the scored one alone, which is what makes it a ratchet tooth rather than a
trade.

### What it bought, stated honestly

`www.naukri.com` moved **`shell-only-1` → `thin-overlap-2`**. That is *not* a scored row, and saying
otherwise would repeat the error this session already named three times. What changed is **whose
problem it is**: `shell-only` is an instrument limit nobody can act on, `thin-overlap` is ours — the
oracle now builds 57 elements of naukri and we render 3.5% of them. A category conversion from
"unmeasurable" to "a coverage bug with an address" is the currency the unscored rows are actually
paid in.

`comix.to` reached 7 elements and stays honestly unscored; its scripts do not run for the oracle at
all, which points at the snapshot fetch being bot-walled rather than at the probe.

### The gate is on the source, deliberately

A live assertion would need the network and would be the flaky gate this project refuses to build.
What is hermetically assertable is that the deferral is **present**, that it is the **same text in
both** live probes (`probe_defer_tail!` is one definition, not a pattern two constants happen to
follow), that each probe **skips its own sentinel** — once a probe re-runs, the `<pre id="__PARITY__">`
it already appended is a rendered element with a box and it would measure itself — and that the
static-fixture probe is still **not** deferred, so that exception stays a decision instead of
decaying into a copy-paste. Each assertion is RED-proven by its own mutation.

**Monotone by construction:** `capture()` still runs at parse *first* and later events overwrite the
same element, so a page whose `load` never fires emits exactly what it emits today. A probe that
could end up emitting *nothing* would read as `ProbeBlocked` and silently cost a whole row.

### The lesson, and it is an old one wearing new clothes

> **A cause written into a doc comment is an assertion, and an assertion nobody measured is a
> hypothesis with tenure.**

This one had sat since the reason was introduced, was quoted verbatim in the sweep's own output on
every run, and was wrong. It cost nothing to check — one script, two Chrome invocations — and the
board's own process rule already said to: *re-probe stale unknowns before building them.* The rule
exists because this project has now paid for it five times.

## Every deferred throw on the app web went into an array only the WPT runner reads (tick 675)

The HEAD-20 sweep's own log named this while it was measuring something else. On `comix.to`:

```text
WARN page.console: reportError: TypeError: can't access property Symbol.iterator, e.children is undefined
```

Two defects are visible in that one line, and a third is visible only in what is *missing* from it.

### 1. `reportError()` did not report

`globalThis.reportError(e)` is the WHATWG global whose **entire definition** is *"report the exception
to the global scope"* — fire `onerror`, dispatch an `error` event. Ours was:

```js
globalThis.reportError = function(e){ __hostLog('warn', 'reportError: ' + e); };
```

A console line and nothing else. **The page's own handler never ran.** That handler is every
telemetry client, every React error boundary, and every retry path on the web: a page that reports an
exception so it can *recover* from it got no recovery, and a page that reports one so it can *record*
it recorded nothing.

**And a probe pinned this row `WORKS` at tick 599**, because `typeof reportError` answered `function`.
Presence standing in for behaviour — the false-YES class, fifth occurrence. `typeof` can only ever
answer whether a name is bound; it has never been able to answer whether the thing behind the name
does its job, and a stub is *exactly* the case where the two answers differ.

### 2. `__reportError` — the funnel — stored errors instead of saying them

`__reportError` is where every **deferred** throw on the app web lands: a `setTimeout` callback, a
microtask, a `MutationObserver` callback, an inline `on*` handler, an event listener. Its body pushed
the error into `globalThis.__errors` and stopped.

`__errors` is read by **exactly one caller in the tree** — `manuk-wpt`'s diag JSON. So under WPT the
error was visible and on a real site it was not. That is not a small asymmetry: it means the funnel
carrying the largest share of app-web failures was silent on precisely the population the browser is
being built for, and `G_SILENT_FAIL` — the gate whose whole subject is *"an error on the load/render/
script path must never be swallowed"* — was looking one step upstream of it.

It is also the storage the **unhandled-error harvester** wants (`STATUS.md`'s meta-instrument #1, the
cheapest and highest-yield instrument on the list). **A harvester cannot harvest what is never
emitted.**

### 3. Neither carried the address — one rule, three implementations

Tick 666 lifted `fileName` / `lineNumber` / `stack` off an exception on the **native** boundary
(`pending_exception`) and stopped there. The two **JS** paths kept stringifying the message alone, so
on minified production code the report read `TypeError: e.children is undefined` — a sentence about
every variable on the page.

This is the class this project has now been bitten by nine ticks running: **fix one implementation of
a rule, then grep for the others.** The properties are SpiderMonkey's own on an `Error`, reachable
from JS as ordinary fields; `__errorAddr` lifts them, and a thrown non-object (`throw 42`) has none
and degrades to exactly the old string rather than to a lie about its origin.

### What landed

- `__errorAddr(e)` — one place that turns an exception into `{file, line, col}` or `null`.
- `__reportError` **logs** (`uncaught (reported): <msg> at <file>:<line>:<col>` + stack), and passes
  the real `filename`/`lineno`/`colno` into both `onerror` and the `ErrorEvent` it dispatches.
- `globalThis.reportError` **delegates** to `__reportError` rather than growing a second copy of
  "how to report an exception" that drifts. Late-bound, because `__reportError` is installed further
  down the same prelude.

**Deduped by `(message, address)`, because this funnel is the one that can flood.** A throwing
`setInterval` reaches the runaway-timer ceiling at 20,000 tasks, and 20,000 identical stacks in the
log is the *same* failure as zero of them — the signal is buried instead of missing. First occurrence
in full; repeats counted and announced at the tenth and every hundredth, so a *rate* stays visible;
the distinct-key table capped at 200 so a page minting unique messages cannot grow it without bound.

### The honest remainder

The address is real and specific — `inline.js:13:42`, with a stack naming the frame — but the *file*
is the constant `inline.js`: SpiderMonkey compiles every inline `<script>` under that one name in
`run_one_script`. On a page with forty inline scripts, `inline.js:13` identifies a line in an unnamed
one of forty. Chrome reports the document URL here. That is recorded in the gate as a named gap rather
than asserted away, and it is the next tick on this thread.

### The gate

`G_SILENT_FAIL`'s existing test, extended rather than duplicated — the file already carries two tests
and a third page-loading `#[test]` in one binary is how this project has produced
`SpiderMonkey has already been shut down in this process` before. One page now exercises all four
paths, and the deferred-throw assertion reads the **single log line** rather than asking whether the
message and the address both appear *somewhere* (two unrelated lines satisfy that, which is the
accounting error this project keeps catching). RED-proven by three independent mutations: drop the
delegation → the page's listener never runs (`never-caught`); drop the host log → the deferred throw
is silent; drop the address → the line is anonymous.

[[reliability-doctrine]] [[honest-answer-is-not-a-fixed-answer]]

## A step change in the instrument is not an error bar on the subject (tick 676)

The HEAD-20 sweep tick 673 and 674 both owed ran on the t674 tree, seeded from
`head20-rows-t672.tsv` into `head20-rows-t675.tsv`. The certificate's headline did not move —
**sites 20 · scored 5 · shape ≥0.75 on 0** — and everything interesting is in what moved underneath
it.

### What t674's deferred probe did to the ORACLE'S POPULATION

| site | t672 (probe at parse) | t675 (probe at `load`) | |
|---|---|---|---|
| `keirin.jp` | 0.708 / 0.048 · n=356 | 0.744 / **0.573** · n=**1036** | 3× the population, h-overflow 79 → 3 |
| `www.agoda.com` | 0.385 / — · n=5 `thin-overlap-5` | 0.073 / **0.586** · n=58 | **newly SCORED** |
| `www.naukri.com` | — · `shell-only-1` | — · `thin-overlap-2` | t674's claim, confirmed on the corpus |
| `playhop.com` | 0.965 / 0.636 · n=550 | 0.047 / 0.200 · n=**5** `thin-overlap-5` | **scored row LOST** |
| `www.welt.de` | 0.957 / 0.6657 | 0.957 / 0.6659 | server-rendered: unmoved, as designed |

**`playhop.com`'s 0.636 was a SHELL score.** Its pre-`load` DOM agreed with Chrome's on 550 elements;
its post-`load` app agrees on 5. That is tick 614's finding — *the oracle renders a shell for JS-built
pages and the certificate was scoring it* — recurring on a site the certificate was actively counting,
and the instrument getting more honest is what took the row away. **Losing a row this way is a ratchet
ADVANCE on the instrument face, not a regression:** the term the Phase-0 exit hangs on
(`shape ≥0.75 on N sites`) was 0 before and is 0 after, and one of the five rows behind it has stopped
being a claim about a page nobody was looking at.

### …and the spread block read all of that as the sites' own noise

```text
www.naukri.com    0.0000 .. 1.0000   Δ 100.0 pts over 2 runs
www.agoda.com     0.0000 .. 0.5862   Δ  58.6 pts over 2 runs
keirin.jp         0.0478 .. 0.5734   Δ  52.6 pts over 2 runs
playhop.com       0.2000 .. 0.6364   Δ  43.6 pts over 2 runs
```

Every genuine per-site spread this project has ever recorded is ≤ 3.7 points. These four are the
**instrument changing under the file**, and the rows file — append-only, accumulated across ticks —
had no way to say so. Two consequences, and the second is the expensive one:

1. A reader (including me) sizes a real delta against a fabricated error bar.
2. **`repeat_plan` consumes this function.** Tick 673 made the sweep re-render any site whose recorded
   spread exceeds 5 points, three times, to take a median. All four sites above would have been
   rendered three times on *every future sweep, forever*, to re-measure a variance that is not
   variance — eight extra live renders per sweep, paid indefinitely, for nothing.

### The fix: the version IS the probes' text

`chrome::instrument_tag()` is a stable 8-hex digest of `PROBE_ALL_IDS_JS` + `PROBE_ALL_PATHS_JS`, and
`append_rows_tsv` writes it as a tenth column. `shape_spreads` takes the **last** tag in the file as
the current instrument and forms the error bar from those rows only.

**Derived, not declared.** A hand-maintained version constant is the exact thing that gets forgotten
on the tick that changes the probe — the edit lands, the bump does not, and two instruments pool while
everything downstream believes they did not. The gate therefore asserts that the tag *is* the digest of
the probe sources and that editing a probe *moves* it, rather than merely that a tag exists.

**Backward compatible by construction.** A file with no tags — every sweep already banked in
`docs/bench/` — has no "current" tag, so the filter is skipped and the old behaviour holds exactly.
That is asserted, because a fix that silently discarded the banked error bars would have cost more
than the defect.

**And the mixture is DISCLOSED.** `spread_report` prints every version present with its row count when
there is more than one. An older row still counts in the certificate for a site the current instrument
never reached — dropping it would shrink the denominator, which is cause #1 in the certification
design's list of historically flattering numbers — so both facts are printed rather than one being
quietly true. A bounded exclusion nobody is told about reads as "everything was included".

### RED-proven from both sides

Removing the version filter reddens it (keirin's step change is printed as a 52.6-point error bar).
**Over-suppressing** — dropping every tagged row instead of every *older* tagged row — reddens it
separately, on `ikea`'s two genuine current-instrument readings. A one-sided assertion here would have
been satisfied by an instrument that had simply stopped reporting spread at all, which is the failure
mode the whole spread block exists to prevent.

[[certification-redesign]] [[reliability-doctrine]] [[parity-methodology]]

## ~~`scored 5 → 6`~~ — RETRACTED at tick 682; the real reading is `scored 5` (tick 681)

> ⚠⚠ **RETRACTION, read this first.** The `scored 5 → 6` headline below and its "Defect 1" are
> **wrong**, and the run that produced them says so. `shape_n` is the count of paths **COMMON to both
> engines**, not the oracle's population — and the oracle built **808 paths in every agoda draw** and
> **57 in every naukri draw**. The document never changed; the variance is **ours**, which is exactly
> what the median exists to absorb. The filter that disqualified agoda's thin draws was keeping our best
> draw and discarding our typical one, and it moved a certificate term on the tick that introduced it.
> Both rules were reverted at tick 682, the tie-break was re-pointed at the SMALLEST sample, and the
> honest number is **`scored 5`, unchanged**. See "a rule whose justification is falsified by the run
> that motivated it" below. The rest of this section — the population growth on naukri, the control, the
> cached-snapshot finding — stands.

## `scored 5 → 6`, and the repeat machinery's first real use exposed two defects (tick 681)

The corpus was four ticks stale — t677 (named access on the Window object), t679 (attribution) and t680
(the virtual-clock horizon) had never been measured on it. Constitution check #51 named that as the
steer, and this is the sweep.

### The certificate moved

```text
              t675 (t674 tree)          t681 (t680 tree)
  scored              5           ->            6
  h-overflow clean    4 (20.0%)   ->            5 (25.0%)
  reading-order       1 ( 5.0%)   ->            2 (10.0%)
  dead-target         5 (25.0%)   ->            6 (30.0%)
  shape >= 0.75 on    0           ->            0
```

`www.naukri.com` is a **scored row** for the first time. Its oracle-shared population went **2 → 10**
and its coverage **3.5% → 17.5%** — a 5× gain, from t677 (`window.<id>` was `undefined`, so the site's
data island never became its state) and t680 (the hang guard was tripping on our own unbounded clock).

⚠ **And it lands at exactly the sample floor.** `CERT_MIN_SHAPE_SAMPLE` is 10 and naukri's three draws
are n = 10, 9, 9. One element either way flips whether the site is scored at all. Stated because a term
that moved by one element is not a term that has moved.

The control holds: `www.desitales2.com` read 0.6112 against 0.6139, inside its recorded 2.3-point
spread. `welt.de` 0.6659 → 0.6705 (+14 elements), `keirin` 0.5734 → 0.5717, `ikea` identical.

### Defect 1 — a draw whose ORACLE population collapsed is a different DOCUMENT

`www.agoda.com`'s three draws, minutes apart in one sweep:

```text
  cov=0.080446  shape=0.507692  n=65     <- the oracle built 65 elements
  cov=0.012376  shape=0.100000  n=10     <- and 10 on these two
  cov=0.012376  shape=0.100000  n=10
```

A 6.5× change in **the oracle's** element count is the site serving something else, and
`oracle_probe`'s own doc comment already forbids the comparison: *"the oracle must feed ONE identical
document to both engines; fetching independently per engine compares two different documents and calls
the difference a bug."* Under the old rule the two thin draws outvoted the representative one and the
certificate published **0.100 for a site it had measured at 0.508**.

A draw whose `shape_n` is below half the run's maximum now does not VOTE. It stays in the rows file —
the evidence is the point — and the collapse prints what it set aside, because a bounded exclusion
nobody is told about reads as *"everything was included"*.

⚠ Note what this does **not** do: it does not prefer the higher score. Had agoda's thin draw been the
*better* one it would be set aside just the same. The disqualifier is the population change, not the
direction of the result.

### Defect 2 — a tie at the median was decided by nothing

`www.naukri.com`'s three draws carry an **identical** shape (0.0) and differ only in sample size
(10, 9, 9). The middle draw was therefore whichever the sort happened to place there — an n=9 row,
below the floor — while an n=10 draw that clears it sat in the same run. Among draws that tie at the
median shape, the one with the **larger sample** now represents the site: among equal point estimates
more evidence is a better estimate, which is the same reason `CERT_MIN_SHAPE_SAMPLE` exists. It cannot
change *which score* the certificate reads — only how much evidence stands behind it.

### And the repeats are worthless on three of the four sites they cost

```text
  www.agoda.com    0.1000 .. 0.5077   Δ 40.8 pts over 3 runs   <- real, and large
  www.naukri.com   0.0000 .. 0.0000   Δ  0.0 pts over 3 runs
  keirin.jp        0.5717 .. 0.5717   Δ  0.0 pts over 3 runs
  playhop.com      0.1429 .. 0.1429   Δ  0.0 pts over 3 runs
```

Three sites returned **byte-identical** rows across three renders, because the document snapshot is
cached — so the repeats are three renders of the same bytes and measure nothing. Six extra live
renders per sweep for an error bar of zero. **The variance the repeat plan was built for is in the
DOCUMENT, not in our render**, which is why only agoda — the site that served two different documents —
produced a spread at all. Named, not fixed: the plan should key on whether a site's variance is
reproducible within a sweep, and that is its own tick.

⚠ The t675 rows are untagged, so they are excluded from the error bar even though the probes were in
fact unchanged between them and t681. Conservative and correct by construction, and it costs the bar
once — the honest price of a column that did not exist yet.

[[certification-redesign]] [[reliability-doctrine]]

## A rule whose justification is falsified by the run that motivated it (tick 682)

Tick 681 introduced two collapse rules and one of them was wrong. The evidence that falsifies it was in
the log of the same sweep, four lines away from the numbers that motivated it.

### What t681 claimed

`www.agoda.com`'s three draws carried `shape_n` of 65, 10, 10. t681 read that as *"the ORACLE built 65
elements on one draw and 10 on the others, so the site served a different document"*, disqualified the
two thin draws from voting, and the certificate went from `scored 5` to `scored 6` with agoda reading
0.508 instead of 0.100.

### What the log said

```text
  www.agoda.com   structural: 8.0% (808 paths, 743 missing, 63 misplaced)   -> shared 65
  www.agoda.com   structural: 1.2% (808 paths, 798 missing,  9 misplaced)   -> shared 10
  www.agoda.com   structural: 1.2% (808 paths, 798 missing,  9 misplaced)   -> shared 10
  www.naukri.com  structural:17.5% ( 57 paths,  47 missing, 10 misplaced)   -> shared 10
  www.naukri.com  structural:15.8% ( 57 paths,  48 missing,  9 misplaced)   -> shared  9
```

**808 paths in every agoda draw. 57 in every naukri draw.** `shape_n` is the count of paths COMMON to
both engines — the intersection, not the oracle's population. The oracle served the *same* document
every time and **the variance is entirely ours**: our own render shared 65 paths on one draw and 10 on
the next.

That is precisely the variance `repeat_plan` exists to sample, and precisely what the MEDIAN is for. The
filter was **keeping our best draw and discarding our typical one** — the flattering direction this
whole file exists to close — and it moved a certificate term on the tick that introduced it.

### Both rules corrected

- The population-collapse filter is **removed**. The median stands.
- The tie-break at the median is **re-pointed at the SMALLEST sample.** t681 chose the largest, arguing
  that more evidence is a better estimate. That is true in general and it is the wrong direction here,
  because on `www.naukri.com` (n = 10, 9, 9, every draw shape 0.0) the choice decides whether the site
  clears `CERT_MIN_SHAPE_SAMPLE` at all — so choosing the largest is choosing the draw that helps. **A
  bar must never be cleared by a convention**, which is the same principle that makes an even-length run
  take its lower middle.

Honest number, recomputed on the unchanged rows file: **`sites 20 · scored 5 · shape ≥0.75 on 0`.**
Naukri's population really did grow 2 → 10 (t677 + t680, and that part of t681 stands), and it is
honestly UNSCORED because its typical draw shares 9 paths and the floor is 10.

### The lesson, and it is a sharper form of one already on file

> **A number's NAME is not its definition.** `shape_n` is documented as the scored sample size and reads
> like "how big was the page"; it is the size of the *intersection*. One `grep` of the sweep's own
> `structural:` line — printed immediately above every row it produced — would have settled it before
> the rule was written.

This is the same class as *"an absent measurement is not a negative measurement"* and *"suspect the
instrument before the subject"*, and the mechanism that caught it is the one this project keeps
returning to: **read the output of the change, next to the change.** It took one tick to publish and one
grep to falsify.

[[reliability-doctrine]] [[certification-redesign]] [[honest-answer-is-not-a-fixed-answer]]

## A repeat that measured nothing is not paid for twice (tick 687)

Tick 673 built the per-site repeat plan; tick 681 was its first real use, and the result was that **three
of the four sites it repeated returned an identical shape on all three renders:**

```text
  www.agoda.com    0.1000 .. 0.5077   Δ 40.8 pts over 3 runs   <- real, and large
  www.naukri.com   0.0000 .. 0.0000   Δ  0.0 pts over 3 runs
  keirin.jp        0.5717 .. 0.5717   Δ  0.0 pts over 3 runs
  playhop.com      0.1429 .. 0.1429   Δ  0.0 pts over 3 runs
```

The document snapshot is cached, so those were **three renders of the same bytes** — six extra live
renders per sweep, indefinitely, for an error bar of exactly zero. The variance those sites showed
*across* sweeps lives in the document, and no amount of repeating inside one sweep can sample it.

`repeat_plan` now excludes any site whose most recent CONSECUTIVE run was flat. The rule can only ever
**retire a repeat that has already been paid for once**: a site with fewer than `UNSTABLE_REPEATS`
readings in its run is *unknown*, not deterministic, and keeps its repeats.

### This breaks tick 673's monotonicity argument on purpose

t673 wrote: *"a site that has ever drawn wide keeps its repeats, because the two errors are not
symmetric (two renders vs a phantom regression)."* That was right while the within-sweep spread was
unknown. It is now measured, and **where it is zero the median of three identical draws IS the single
draw** — so the repeats cannot prevent a phantom regression, they can only cost renders. Asymmetric
errors justify paying for information, not for none.

### ⚠ And the gate's first draft was vacuous — the third time this session

The fixture held only keirin's identical run. With just that, `shape_spreads` reports Δ 0.0 and the
pre-existing `> SPREAD_UNSTABLE_PTS` filter already drops the site — so disabling the new guard changed
nothing and the mutation stayed **green**. The guard only bites where a file holds **both** a wide
cross-run spread and a flat within-sweep run, which is keirin's actual state (Δ 52.6 across sweeps, Δ 0.0
within one). Adding an earlier differing reading to the fixture made the mutation red.

Three vacuous assertions caught by running the mutation in one session (t675's whole-log disjunction,
t680's flag read from inside a task that always ran first, this one). Each was green, plausible, and
measuring nothing. **The falsify pass is not a formality; it is the only thing that distinguishes a gate
from a comment.**

[[certification-redesign]] [[reliability-doctrine]]

## The claimed clusters moved — `ikea` coverage 97.1% → 100%, `keirin` dy 206 → 161 (tick 692)

Two ticks claimed clusters and the mandate's rule is that a claim is proven by a sweep: t689 (a broken
`<img>` reserves Chrome's 16×16 placeholder) claimed `Cc4e6 <img>`; t691 (every line box starts with a strut)
claimed `C01ca <div>` and `C7eb9 <body>`. Same corpus, same instrument tag, t691 tree.

```text
  site              metric        t681      t692     delta
  www.ikea.com      coverage     0.9708   1.0000    +2.9 pts  -> EVERY element Chrome renders
  keirin.jp         shape        0.5717   0.5873    +1.6 pts
  keirin.jp         median dy       206      161    -45 px
  www.welt.de       median dy      3077     2957   -120 px
  www.agoda.com     shape (n=59) 0.5077   0.5593    +5.2 pts
  www.agoda.com     median dy        14       12     -2 px
  www.desitales2    median dy        91      110    +19 px    <- WORSE
  www.ikea.com      median dy       145      145    unchanged
  playhop / naukri  unchanged (render-failed n=7 / thin-overlap-9)
```

**The `dy` term moved on three of the four sites that carried it**, which is the term tick 688 measured as
the SHAPE driver — and `ikea` reached **100% coverage**. The mechanism the two levers targeted is the
mechanism that moved.

⚠ **The certificate's headline did NOT move: `sites 20 · scored 5 · shape ≥0.75 on 0`.** The 0.75 bar is far
above 0.59, so a 1.6-point or 5.2-point gain cannot cross it. Saying the certificate moved would be false;
saying the work did nothing would also be false. **What moved is the distance to the bar, measured in the
term that governs it.**

⚠ **And one site got worse: `desitales2`'s median dy went 91 → 110.** Its SHAPE is stable across the two
runs (0.6061..0.6112, Δ 0.5 pts) so the shape score is not noise-hiding a regression, but the vertical
displacement grew by 19px and neither lever predicts that. Recorded as an open item rather than averaged
away — a lever that improves three sites and worsens a fourth has a second mechanism in it.

⚠ `agoda`'s within-sweep spread is now **Δ 45.9 pts over 6 runs** and its median row is still the thin
`0.100` draw. Its bimodality (t683) is unexplained and it remains the least trustworthy site in the corpus.

[[certification-redesign]] [[box-layout]]

## The priority ledger was keyed by TAG because `delta` never crossed the serialisation boundary (tick 744)

`oracle::cluster()` — the in-process `manuk-wpt oracle --urls …` path — has built the full mechanism key
since t554: `geometry/{displaced|mis-sized}: {axis} ~{band}px  (<tag>)`. It separates a box that is the
wrong SIZE (a sizing fact about the element) from one that is the right size in the wrong PLACE (an
ancestor-frame fact), names which of the four numbers is wrong, and bands the magnitude so a 23px
near-miss does not merge with a 1400px page collapse.

`docs/loop/CLUSTERS.md` — **the file the board ranks the entire loop's work by** — is not written by that
path. It is written by `run_oracle_merge`, which reads the crawl's JSONL, and its geometry arm was one
line:

```rust
_ => format!("geometry: <{tag}>"),
```

So the corpus ledger's #1 row read `geometry: <div>   1781 sites / 14002 hits` — a row that merges a 129px
column swap with a 2px line-height residue and **cannot be attacked as one cause**. Every geometry
divergence in the corpus, for 351 ticks, was ranked by an HTML element name.

⚠⚠⚠ **And the merge was not merely choosing a coarser key — the information had already been destroyed.**
The emitter wrote `site`, `class`, `tag`, `dkind`, `chrome`, `manuk`, `id` and **not `delta`**. The four
deltas are the only field that distinguishes a wrong width from a wrong height from a pure displacement,
and they were dropped on the floor at the `format!` that serialised each divergence. No fix to the merge
alone could have restored them; the reader was poor because the writer had already answered the question.

**This is [[tick 743]]'s lesson at the other end of the same pipe: a serialisation boundary is a semantic
one.** There, handing an `<svg>` subtree to a parser as a standalone document silently answered a question
about reference SCOPE. Here, serialising a `Divergence` silently answered *"what is a root cause?"* — and
answered it with the tag, because the tag is what survived the write.

### One rule, ONE implementation

`oracle::signature_of(&Divergence)` is now the single definition, called by both `cluster()` and
`run_oracle_merge`. `div_to_jsonl` / `div_from_jsonl` are the matching halves of the boundary, in the same
file, with a round-trip test between them asserting the signature is **byte-identical** before and after.

Measured on a 2-site crawl, the same divergences, before and after:

```text
  before                    after
  geometry: <span>          geometry/displaced: x (horizontal) ~128px  (<span>)   med=200px   44 hits
  geometry: <tr>            geometry/displaced: y (vertical drift) ~64px (<tr>)   med=85px    21 hits
  geometry: <a>             geometry/mis-sized: height ~8px  (<a>)                med=9px     29 hits
```

`med=` is the REAL median of the dominant axis, printed beside the band because **`~Npx` is a power-of-two
BAND and a reader cannot tell that by looking**. t551 read *"the deltas are QUANTISED — 8/16/32/64/128 —
the signature of ONE systematic box-model delta"* off exactly that, and the quantisation was the printer.
Both numbers travel now.

### A missing delta is REFUSED, not zeroed

A pre-t744 crawl directory has no `delta`. The available quiet failure is to default it to `[0,0,0,0]`,
which keys as `geometry/displaced: x (horizontal) ~0px` — **a wrong answer of exactly the right type**, and
every row in the resulting ledger would look measured. `div_from_jsonl` returns `None` for a *geometry*
record with no delta; the merge counts them and prints `217 divergence records REFUSED … Re-crawl to rank
by primitive.` `display` and `missing` records carry no meaningful delta and still parse — they are keyed
by tag by definition.

⚠ `oracle-merge <dir>` also **unconditionally overwrote `docs/loop/CLUSTERS.md`** from whatever directory
it was pointed at, so verifying the merge on a two-site test crawl would have replaced the 265-site
priority ledger with a two-site one. It fired during this tick's own development. `--no-registry` now
exists for an ad-hoc run to say *"I am not the corpus"*; the default is unchanged so `oracle-crawl.sh`
keeps working.

### The RED proof for the band had itself never run

`cluster_bands_geometry_by_offset_magnitude` — the test that proves a 23px near-miss and a 1400px collapse
are different causes — had its `#[test]` attribute and doc comment stranded **190 lines above its own
function**, because a later test was inserted between them. The compiler said so, quietly, as a
`duplicate_macro_attributes` warning on the function that ended up with two. **So the datum was lost twice,
independently, in the same subject — the ledger discarded the band, and the proof that the band matters
was dead — and neither loss made a sound.** Reunited; it now runs, and it goes RED when `signature_of`
falls back to the tag.

[[certification-redesign]] [[box-layout]]

## The instrument charged its own panic to the engine — and 12 of 13 "crashes" were timeouts (tick 748)

Two honesty defects in the fidelity sweep's own bookkeeping, both found by reading the t745 corpus
run's output rather than by any gate.

### A non-ASCII class name panicked the sweep, and the site was banked as `crashed`

`strip_sigs` (`tests/wpt/src/main.rs`) removes the `.SIG` component from every selector-path key. It
runs on **every site by default** — t549 turned the class signature off, and `MANUK_G1_CLASS_SIG=1`
restores it — and it looked back exactly **nine bytes** from the end of each path component:

```rust
if head.len() >= 9 {
    let cand = &head[head.len() - 9..];      // <- byte index, not a char boundary
```

A sig is nine ASCII bytes, so where one exists that index *is* a char boundary. But a class name is
arbitrary author text, and a page served with a broken charset produces components full of multi-byte
`U+FFFD`. `swift.org` did exactly that:

```text
thread 'main' panicked at tests/wpt/src/main.rs:2481:
byte index 194 is not a char boundary; it is inside '\u{fffd}' (bytes 193..195)
```

The process died 7 seconds in, and the recovery path banked the site as **`reason=crashed`** — which
reads as a browser Bar-0 event. *The instrument charged its own panic to the engine, in the one file
whose entire job is to measure the engine honestly.*

Fixed with `head.len().checked_sub(9)` + `is_char_boundary(cut)`. The guard is not a
micro-optimisation and the gate says so: `a_multibyte_class_name_does_not_panic_the_sig_stripper`
asserts **both** directions — a component whose last nine bytes are not a sig passes through
unchanged, and a real sig sitting immediately after multi-byte text is **still stripped**, so the
guard cannot trade a panic for a silently unstripped key. RED-proven by restoring the byte-length
test, which reproduces the panic exactly.

### `reason=crashed` conflates a panic with a watchdog kill — 12 of 13, measured

The t745 sweep runs each site as its own process under `timeout -k 10 180`, and logs the process's
exit code **outside** the instrument precisely because an external `SIGKILL` and a `SIGSEGV` leave the
same in-flight marker (t706). Cross-referencing the two files over the first 141 sites:

```text
  reason=crashed in the rows file        13
  rc=124 (the 180s watchdog fired)       12      <- theguardian, apnews, npr, cnn, engadget,
  rc=101 (a real abnormal exit)           1         stackoverflow, askubuntu, go.dev, redis.io,
                                                    sourceforge, slashdot, walmart
```

**Exactly one of the thirteen was a crash, and it was the instrument's own.** The other twelve are
sites that exceeded 180 seconds of wall clock — and per the standing rule that *every number has a
harness*, that budget wraps **our render, Chromium's render and the probe**, so a `124` names nobody.
The rows file cannot see the exit code, so the label is wrong in the file the burndown reads.

⚠ Not fixed here, and it is not a one-liner: the in-flight marker genuinely cannot distinguish "died"
from "was killed" from inside the dying process. The runner already has the answer (`/tmp/*-rc.tsv`);
the fix is to feed it back in, as its own tick. Until then, **read `reason=crashed` as
`crashed-or-timed-out` and check the rc file before quoting a Bar-0 number from a sweep.**

## Two instruments answered one question, and the permissive one published (tick 751)

`Page::failed_stylesheet_fetches()` counts render-blocking author stylesheets that were requested and
never arrived. Its doc comment states its own purpose exactly:

> Non-zero means the current layout is (partly) UA-default fallback, NOT this engine's rendering of the
> author's page — a measurement that diffs it against a fully-styled reference is charging network
> weather to the engine's account. **The differential oracle discards such runs.**

The oracle does. `run_oracle_cmd` calls it and prints `DISCARDED — N author stylesheet(s) never arrived`.
**The fidelity sweep — the path that computes the Phase-0 headline — never called it.** So the same site,
in the same session, was `DISCARDED` by one instrument and scored `cov 1.000 / shape 0.018` over 337 nodes
by the other.

### The signature, and why it reads as noise

A UA-fallback render is **high coverage with collapsed shape**: every element is present (there was no CSS
to `display:none` anything) and almost none is where Chrome puts it (there was no CSS to position it).

```
discourse.org     cov 0.971   shape 0.087      arstechnica.com  cov 0.988  shape 0.043
basecamp.com      cov 0.987   shape 0.115      postgresql.org   cov 1.000  shape 0.018
```

It is **intermittent**, which is what makes it dangerous rather than merely wrong. Three consecutive runs
of one binary on `discourse.org` give `0.4964 / 0.4995 / 0.4995` — a spread of **0.003** — while a sweep
row for the same binary recorded **0.087**. A site therefore alternates between its real shape and ~0
across sweeps, and the series shows ±40-point per-site swings that look exactly like layout regressions
and are not. The set of affected sites is **per-run, not fixed**, so it cannot be identified by filtering
one sweep's rows for the signature — an error worth naming because it was made here first.

### The refusal is about ASYMMETRY, not about starvation

The first version of this check refused any page with `failed_css > 0` and turned the wall's `G1` gate
into `MEAN VISUAL: NaN%`. G1 feeds both engines `file://` snapshots from `.verify-cache`, where a
snapshot's `href="/static/…"` cannot resolve — `failed_css` is non-zero **by construction**, so every G1
site went unscored and the gate had an empty set to average.

That is the distinction, and it is the whole rule:

| comparison | reference engine | verdict |
|---|---|---|
| `file://` snapshot, sheets unresolvable | sees the **same** missing sheets, renders unstyled too | **symmetric — still scorable** |
| live `http(s)://`, our fetch cut by the deadline | fetches its own subresources and **succeeds** | **asymmetric — refuse** |

A diff survives both engines missing the same thing. It does not survive the two sides looking at
*different documents*. So the check keys on the URL scheme — not as a special case to appease a gate, but
because the scheme is what determines whether the reference had the CSS we lacked.

### Where the refusal goes, and which side of the denominator

At the same point the oracle's is: after the page is built and painted, **before** the reference engine is
invoked, so a starved site costs no screenshot.

`css-starved-N` stays **IN-SCOPE**. `fidelity-progress.sh` partitions on the reason string, and EXCLUDED is
reserved for sites permanently unreachable under our own no-stealth policy. Our own load deadline cut a
sheet the origin served — that is our bug, and a daily driver may not render a reachable site in UA
fallback. Filing it EXCLUDED would *raise* the headline by removing our failure from the denominator, the
exact move `EXCLUDED-RISING` alarms on. The gate mirrors the shell script's exclusion list in code so the
reason cannot drift across that line silently.

Expect, on the first sweep after this lands: `scored` falls, `shape_mean` **rises**, and the
**DENOMINATOR-TRAP** alert fires. It is correct to fire — the population changed. Quote
`inscope_pass_pct`, which is unaffected by construction because these rows were failing either way.

### The rule

**When two instruments answer the same question, the permissive one is the one that publishes.** Nothing
was missing here: the refusal existed, was correct, was documented, and was called — by the other
consumer. Grep for the class: a predicate whose doc comment says *"so a measurement can refuse X"*, then
enumerate its callers and check that **every** measurement is among them. A guard with one caller is a
guard with a hole shaped like every other caller.

## A killed process and a faulting one leave the identical trace (tick 753)

The fidelity sweep claims each site with an **in-flight marker** before touching the engine, so a run that
dies is counted rather than dropped — `recover_inflight` turns a leftover marker into a row on the next
run. The marker carries **only a site name**. So the recovery pass cannot tell *"the external watchdog
killed it"* from *"it faulted"*, and files `Unmeasurable::Crashed` unconditionally.

That default is expensive because of **what `Crashed` outranks**. It is a Bar 0 event, and Bar 0 sits
above every visual divergence in the priority ledger (Part 24.3) — so a phantom `crashed` does not just
add a wrong row, it commands the whole board. The t752 CrUX baseline reported **8 crashed sites**; every
one was `rc=124` at exactly 180s, the runner's `timeout`. No panic, no SIGSEGV, no OOM.

**The information existed the whole time.** The sweep runner logs `rc` beside each row *precisely* because
of this — t706's lesson, in the runner's own comment: *"an external SIGKILL and a SIGSEGV leave the same
marker."* But the row is written by the recovery pass, which has never read the runner's file. One
question, two records, and they never meet.

### Why the fix goes in the failing process, not in the reader

Teaching `recover_inflight` to consult the runner's `rc` file would work and would be wrong: it couples
the instrument to one particular runner, and the corpus runner is an agent-owned script outside the repo,
so the coupling would be both invisible and unversioned.

Instead the ambiguity is removed at the source. The instrument takes a budget **strictly under** the
external watchdog, and a detached per-site timer files an honest `Timeout(secs)` row, clears the marker,
and exits 0 before the kill can land. The external `timeout` becomes a true backstop — and if it ever
fires again, `crashed` means what it says.

Two details that are load-bearing:

- **The budget is measured, not picked.** Across the 438 runs that succeeded in two real corpus sweeps,
  the slowest was **132s** (`<30s: 275 · 30–60: 137 · 60–90: 20 · 90–120: 4 · 120–150: 2 · ≥150: 0`), so
  150s sits above every observed success and below the 180s watchdog: kills become timeouts at a cost of
  zero measurable sites. A mis-ordering against the external watchdog fails *visibly* — rows go back to
  reading `crashed`.
- **A detached timer cannot be joined, so it is disarmed by a generation counter.** Each timer captures
  `SITE_GEN` when armed and re-checks before acting; `flush` bumps it as each row reaches disk, making
  "the row exists" and "the timer is disarmed" the same event. Without it, a `--urls a,b` run lets site
  A's timer fire while B is rendering and files B's row under A's timeout.

### The rule

**When a failure label is chosen by a recovery path, ask what the recovering process can still SEE.** It
sees a marker — not a signal, not an exit code, not a core file. Either put the discriminator *in* the
marker, or better, have the failing process label itself while it is still alive, which needs no
cooperating reader at all. An instrument that cannot distinguish two causes will pick one, and if the one
it picks is the alarming one, the cost is the whole board's attention.

## The key is part of the measurement: one ancestor's class list re-keyed whole documents (tick 754)

Selector-path keys carry an 8-hex **class signature** per component — `body.d6441846:nth-child(2)`. The
intent (tick 395) was sound: a positional counterpart with a *different class list* should fail the key
lookup and book as missing+extra (tree drift, which it is) rather than mint a phantom style diff between
two unrelated elements.

The cost is that `nth-child` **already** identifies a sibling uniquely, so the signature adds no
discriminating power where the DOMs agree — and destroys the measurement where they differ by one class.
A script that sets `js`/`loaded` on `<body>`, a hydration class, a viewport modifier: **every descendant
key changes at once**, nothing matches, and the diff reports the entire subtree as `missing box`.

t549 measured this for the G1 exit gate and turned the signature off there:

```
gov.uk      coverage  0.0% -> 82.8%   (418 paths: 418 missing -> 72)
stripe.com  coverage  0.1% -> 43.1%   (1441 paths: 1439 -> 820)
nytimes.com          0.0% ->  0.0%    <- did NOT move: a real, separate failure
```

That last row is why the conclusion is trustworthy: the correction is not a blanket improvement, it moves
exactly the sites whose keys were broken.

**The oracle kept the signature for ~200 more ticks**, and the oracle writes `CLUSTERS.md` — the file
STATUS.md calls *the priority ledger, not a suggestion judgment may override*. So the instrument choosing
what to build next was ranking by its own keying artefact. Measured on the same pages, same binary:

| | `missing box: <div>` (heart.org) | divergence hits, 3 CrUX sites |
|---|---|---|
| sigs ON | 211 | 2750 |
| sigs OFF | absent | 892 |

**~68% phantom.** The tell was available the whole time and takes one sort to see: the *shallowest*
missing element on `heart.org` was **`<body>`** (`block [0 0 1200×5993] vs (no box)`) — for a page that
plainly renders. A coverage failure that starts at `<body>` is not a coverage failure.

⚠ The **scores** were never affected — the fidelity sweep has stripped signatures since t549. Only the
**ranking** was, which is the more expensive half: a wrong number is corrected by the next sweep, a wrong
ranking spends ticks.

### The rules

1. **Before believing a "missing box" cluster, sort its members by DEPTH and look at the shallowest.**
   If the shallowest missing element is an ancestor that obviously renders, the key is broken, not the
   engine.
2. **A key is not metadata about a measurement; it IS the measurement.** Two engines agreeing on a naming
   scheme is a precondition for the diff meaning anything, and any component of the key that is not
   *identity* is a way for agreement to fail spuriously.
3. **When a fix names its own unfinished half, that sentence belongs in the ledger, not only in a
   comment.** t549 wrote *"the same correction is owed there, as its own tick"* — correct, precise, and
   invisible for two hundred ticks. Grep the tree for `owed` / `as its own tick` / `the same correction`.

---

## The parallel sweep — and what parallelism COSTS (tick 771)

`manuk-wpt fidelity --jobs N` fans the corpus out across N **child processes** (not threads):

- a site that SEGFAULTs kills its own chunk and nothing else (tick 768 found a live one);
- each child gets its own SpiderMonkey runtime and its own Chromium, which is the model everywhere else;
- the parent renders nothing — it splits **round-robin** (the corpus is stratified, so contiguous blocks
  would hand one child every slow HEAD site), spawns, waits, merges, and prints `sampled` vs `rows`.

**200 sites: 3h20m serial → 29m55s at `--jobs 8`.**

### Two accounting defects it surfaced while being built

1. A killed child leaves its in-flight site as a marker in its **own** chunk file; without recovering it
   there, the site that killed the chunk is the one site nobody counts.
2. The per-site watchdog files its `timeout-150s` row and then `std::process::exit(0)`s — correct for a
   wedged main thread, fatal for a chunk, because every site **queued behind it** never runs. A 9-site
   trial merged 8 rows with `www.ikea.com` silently absent.

A chunk is therefore a **spawn loop**: re-spawn the remainder (cap 4 rounds), file an explicit `crashed`
row for anything still missing, and reconcile `sampled == rows` out loud.

### ⚠ The speed is not free, and the cost is SCORABILITY, not accuracy

The first full `--jobs 8` sweep read `scored 82 → 52`. The control — re-run the lost sites serially on the
same binary:

| | scored |
|---|---|
| 10 lost sites, **serial** | 9 (the 10th is genuinely unreachable) |
| the same 10, `--jobs 4` | 8 |
| the same 10, `--jobs 2` | 9 |
| **\|Δ shape\| for any site that scores, at any job count** | **max 0.1 pt** |

**A site that completes scores the same number at any job count.** What parallelism costs is whether it
completes: the per-site budget and the network timeouts are **wall-clock**, so eight concurrent
Chrome+manuk pairs push hard sites past a deadline that measures the box rather than the engine.

**The rule that follows:** `--jobs 2` for cert-grade sweeps (2× faster, denominator intact); higher job
counts are for *triage* sweeps where losing the slowest fifth of the corpus is a stated cost. A run that
loses sites must never be banked as a burndown point — the t771 row is annotated as contaminated in
`FIDELITY-PROGRESS.tsv` for exactly that reason.

**And the general form, which outlives this tool:** *a faster instrument is a different instrument until
a control says otherwise.* When you change HOW a measurement is taken, the first run of the new
instrument is a comparison against the old one, not a result. The tell for this failure is a
**scorability shift with stable scores** — which is what the denominator trap looks like from the inside.

## The reference probe was WIDENING the page it measured (t781)

The Chrome-side probe serialises its result into a `<pre id="__PARITY__">` and appends that element to
`document.documentElement` — a **sibling of `<body>`** — so `--dump-dom` carries the JSON back. That was
inert for as long as the probe ran **once, at end of parse, and appended afterwards**.

t674 deferred the probe: `capture()` now runs at parse, DOMContentLoaded, `load` and T+3000, and `emit()`
creates the sentinel on the **first** call. From that tick on, three of the four readings measured a
document that already contained the probe's own output — and a `<pre>` does not wrap, so an element
holding ~30KB of JSON on one line has a max-content width of tens of thousands of px.

On a page whose root box is **stretched to the ICB** that costs nothing: `<html>` is the viewport width
and the sentinel simply overflows. On a page whose root is **intrinsically sized**, `<html>` sizes to the
widest thing in it — the sentinel — and `<body>` inherits that width.

Measured on `www.naukri.com`, deterministically, replaying the harness's own instrumented temp file:

| | `<body>` | elements captured | heights |
|---|---|---|---|
| as shipped | **89,905 × 352** | 57 | correct |
| `+ pre.style.display='none'` | **1,200 × 352** | 57 | correct |

A 1,200px viewport, a body reported 75× too wide, and the centred 1,120px content column placed at
`x = 44,392` — which is exactly `(89905 − 1120) / 2`. **Every `x` and every `width` the reference
reported for that site was a number the reference had created**, and `shape_stats` charged all of it to
the engine: the row read `thin-overlap`, whose own explanation text says *"this is OURS"*.

The heights were right throughout, which is why it survived: the failure is horizontal-only, and the
element **population** — the thing t674 was careful about, and the thing `instrument_tag()` fingerprints —
never moved.

### Why the near-miss took a while to name

Four hand-replications of the probe returned `1200` and disagreed with the harness. Each differed from
the harness in something plausible (probe timing, curl UA, capture phase) and none was the cause. What
settled it was **running Chrome on the harness's own instrumented file**, recovered by copying
`/tmp/manuk-shape-*.html` out from under the run before it deletes it — after which the 89,905 reproduced
2/2 and a one-line patch to that same file returned it to 1,200. *When a replication disagrees with the
instrument, replicate the instrument's actual artefact, not your reading of its source.*

### The general form

**An instrument that writes into the thing it measures must be re-checked the moment it starts measuring
more than once.** The sentinel's inertness was a property of the OLD ordering — measure, then write. t674
changed the ordering to write, then measure again, and re-derived the population question but not this
one. There is no way to notice from the diff: the deferral tick and the sentinel are twenty lines apart
and neither mentions the other.

Guarded by two tests in `tests/wpt/src/chrome.rs`:

- `every_probe_sentinel_is_display_none` scans **this file's own source**, not the two probe constants —
  there are five sentinels (`PROBE_JS`, `PROBE_ALL_IDS_JS`, `PROBE_ALL_PATHS_JS`, `__ORACLE__`, `__G5__`)
  and only two are constants. A test naming the two would have passed while the differential crawl's
  probe (which feeds `CLUSTERS.md`) rotted.
- `the_reference_probe_does_not_widen_an_intrinsically_sized_root` runs the real probe over a `file://`
  fixture whose `<html>` is `width:max-content` around a 300px block. **Proven red:** without the fix it
  reports `<body>` at 1221px against the 300 it must be.

⚠ **What this did NOT fix.** naukri's `coverage` is unchanged at 15.8% with 48 missing elements — that
half is a real engine gap and is still the open lead. What moved is `misplaced 9 → 4` and the geometry
the burndown ranks on.

## `thin-overlap` said "this is OURS", and the number that decides it was never read (t782)

`unscoreable_reason` classified an unscoreable row from two numbers: `probed` (how many box-bearing
elements the ORACLE built) and `common` (how many paths both engines share). Whenever the
intersection fell under the certificate's sample floor it returned `ThinOverlap`, whose text reads:

> *"the oracle built the page and we did not, so the missing elements are a coverage failure wearing
> an 'unscored' label"*

That is a claim about **our** element count, and our element count was never an argument. It is
sitting in `mseen` at the one call site and was dropped on the floor.

### What the cohort actually looks like

The 25 `thin-overlap` rows of the t777 sweep, re-measured with our own side printed. `ours` is the
count the reason now carries; `common` is the intersection the score is computed over:

| site | **ours** | common | the row's coverage |
|---|---|---|---|
| www.crazyshop.pl | 1481 | 1 | 0.07% |
| www.trivago.de / .fr / .pl / .be / .jp | 1355 / 1347 / 1345 / 1345 / 1335 | **0** | 0.00% |
| sports.yahoo.com | 1838 | 1 | 9.1% |
| www.ebay.com | 1226 | 4 | 16.0% |
| www.timeline.com | 1055 | 2 | 0.17% |
| mayatoys.in | 1044 | 3 | 0.19% |
| www.freesupertips.com | 738 | 6 | 0.77% |
| a1.ro | 689 | 1 | 0.15% |
| www.ta3lemkonline.com | 645 | 1 | 0.22% |
| portagelearning.edu | 492 | 8 | 4.8% |
| www.kroftools.com | 444 | 2 | 0.30% |
| www.naukri.com | 434 | 9 | 15.8% |
| app.ordertime.com · crm.majoo.id · mobcup.fm | 31 / 32 / 32 | 1 | 2-4% |

**We draw hundreds to thousands of boxes and the two engines agree on between zero and nine paths.**
`www.naukri.com`, measured on its own so both sides are exactly attributable: **oracle 57 paths,
ours 434** — 7.6× — and the row said we had rendered *15.8% of the page*. A coverage gap cannot look
like this.

It is two path spaces that do not line up — the same failure mode `main.rs` already records for the
class-hash component of the key (*"a single ancestor's class list differs DESTROYS the measurement"*,
sigs ON→OFF recovered gov.uk from 0.0% to 82.8%), arriving this time through `nth-child` indices:
**one differing element near the root re-numbers every sibling beneath it and every key below changes
at once.**

And the documents genuinely do differ. The oracle renders a `curl` SNAPSHOT from `file://`; we render
the LIVE url through our own net stack. Two fetches of an ad-serving, personalised, hydrating page
are two documents, and one extra `<div>` in either is enough.

**After the change: 23 of the 25 rows read `tree-divergence`, and the other two are `unreachable`
and `timeout-150s`. Not one row survives as `thin-overlap`.**

⚠ **The per-site table above is taken from the ROWS FILE, not from the run log, and that distinction
cost a wrong draft.** Under `--jobs 2` the log interleaves two chunks, so a `fidelity: <name>` line
and the next `structural:` line belong to *different sites*; pairing them by adjacency attributed
`tracker.shadowfax.in` a 1410-element oracle count that belongs to a trivago row. The TSV carries the
name in the row, so it is the only row-attributed source. *An interleaved log is not a table.*

### The new reason, and what it deliberately does NOT do

`Unmeasurable::TreeDivergence(ours)` — *"both engines built a page and they still share almost none
of the same paths"* — carrying OUR element count so the row can never again describe one side only.

The rule is the **symmetric counterpart of `ShellOnly`**, reusing `CERT_MIN_SHAPE_SAMPLE` rather than
inventing a ratio: `ShellOnly` asks *did the ORACLE build a page*, this asks *did WE build a page*,
and when both did, a thin intersection is divergence. `ThinOverlap` keeps exactly the case its
sentence can support — the oracle built a page and we are the one below the floor.

⚠ **The first draft tested `ours >= probed` and the cohort measurement caught it**: that kept
`shadowfax` (1410 · 1355 · 0) and `mayatoys` (1417 · 1335 · 0) filed as *"we did not build the
page"*, on pages where we drew over thirteen hundred boxes. Kept as a regression assertion.

⚠⚠ **`tree-divergence-N` stays IN-SCOPE and stays UNSCORED**, and that is the load-bearing half. The
tempting next step — "the comparison is unsound, so exclude these rows" — would move 20-odd of 129
in-scope rows into EXCLUDED and raise the Phase-0 headline for free. **A comparison being unsound is
a reason to stop MIS-ATTRIBUTING it, never a reason to stop COUNTING it.** The certificate's
arithmetic is byte-for-byte unchanged; only what the loop is told to go and fix changes.

### The general form

**A verdict that assigns blame needs every party's number.** This one compared the reference against
the intersection and pronounced on the subject, which was never in the expression. The tell is
grammatical and worth memorising: the sentence names *us*, the inputs do not.

## Is a zero intersection a RENDERING result or a KEYING result? (t783)

t782 left 23 in-scope rows reading `tree-divergence`: two engines each drawing hundreds-to-thousands
of boxes and sharing between zero and nine selector-paths. Exactly two explanations survive that
shape, and their fixes point in opposite directions:

1. **Index shift** — the trees are substantially the same and the KEY is brittle. `:nth-child(N)` is
   an absolute sibling index, so one element present in one document and not the other re-numbers
   every sibling beneath it. Fix: a key that survives an insertion.
2. **Different documents** — the oracle renders a `curl` snapshot from `file://` and we render the
   LIVE url. Fix: hand both engines the same bytes.

`fidelity::tree_alignment` discriminates them without building either. It re-keys both sides on the
**tag path alone** (every `:nth-child(N)` stripped, so `body:nth-child(2)/div:nth-child(4)` becomes
`body/div`) and reports the MULTISET overlap — the ceiling an index-insensitive key could reach —
beside the exact intersection and the shallowest depth whose element count differs.

| site | exact / oracle | tag-path multiset | ours | first differing depth |
|---|---|---|---|---|
| a1.ro | **1 / 685** | **685 (100%)** | 689 | 3 |
| www.timeline.com | 2 / 1212 | 1055 (87%) | 1055 | 8 |
| www.freesupertips.com | 6 / 756 | 507 (67%) | 738 | 2 |
| www.kroftools.com | 2 / 667 | 442 (66%) | 444 | 1 |
| www.naukri.com | 9 / 57 | 25 (44%) | 434 | 2 |
| tracker.shadowfax.in | 1 / 29 | 1 (3%) | 48 | 2 |

**`a1.ro` is the finding in one row: the two trees are identical under a tag-only key — 685 of 685 —
and the shipped key recovers ONE element.** That row goes out as `coverage 0.15%`. On the four large
sites an index-insensitive key reaches 66–100% where the shipped key reaches 0.1–0.8%.

So it is **index shift on the population that matters**, and the small sites are the mixed tail:
`shadowfax` at 3% really is two different documents, `naukri` at 44% is both. `first differing depth`
confirms rather than restates the mechanism — **1, 2 and 3** on half the rows. A node inserted that
close to the root re-numbers essentially the whole document, which is exactly why `exact` collapses
to 1 or 2 while the tag-path multiset holds.

⚠ **The 50% cut that picks the printed WORD is a reading aid, not a fact.** The first draft printed
*"DIFFERENT DOCUMENTS: no key recovers this"* below it, and naukri came in at 44% — where a better
key still recovers nearly 3× what the current one does. A threshold that decides a word is still a
threshold; the honest content of the line is the ratio.

⚠ **The insertion fixture asserts `exact == 3`, not 0.** A shift does not destroy every key: the bare
CONTAINER paths still collide across it, because the sibling that moves into slot N usually has the
same tag as the one that left. What a shift reliably destroys is the LEAVES — which is where a page's
elements are, and why the wild reading is `1 of 685`.

**The open half.** `main.rs` already carries the mirror image of this correction: the key's class-hash
component was turned OFF at t549 because *"a single ancestor's class list differs DESTROYS the
measurement"* (gov.uk 0.0% → 82.8%). The sibling index is the same failure through the other
component, and it is now a bounded fix rather than a subsystem.

## The key that survives an inserted sibling — `:nth-child` → `:nth-of-type` (t784)

The open half above was closed the next tick, and it is one counting rule in three producers that had
to move together: the live-site probe's JS `pathOf`, the snapshot oracle probe's JS `pathOf`, and
Rust's `path_of` over our own DOM. The rule: **the ordinal counts the element siblings that share this
element's TAG, not all of them.**

Nothing else about the key changed. `(tag, N)` is still unique among a parent's children, so a path is
still unique to an element — this weakens what a MISMATCH means and leaves what a MATCH means exactly
where it was. That asymmetry is the reason it is safe: a weaker key that *mints* agreement is how this
would go wrong, and the alignment gate holds two unrelated trees at zero under both strengths.

**Measured on live sites, same binary vintage, one variable — including the control that decides
whether this is a key change or a score change:**

| site | old key `:nth-child` | new key `:nth-of-type` |
|---|---|---|
| `www.timeline.com` | 0.2% coverage, **2** elements scored | **87.0%**, **1055** scored — the tag-path ceiling, exactly |
| `www.kroftools.com` | 0.3%, 2 scored | 5.7%, 38 scored |
| `a1.ro` | 0.1%, 1 scored | 2.3%, 16 scored |
| `blog.rust-lang.org` **(CONTROL)** | 100.0% cov / 73.6% shape, 1664 paths | **byte-identical: 100.0% / 73.6%, 1664 paths** |

**The control is the load-bearing row.** Where the two DOMs agree, the two keys are *the same key* —
so a healthy site cannot move, and if one had, the change would have been a change to the SCORE
wearing a key's clothes. This is the same shape of evidence the t550 class-signature ablation
produced (healthy sites byte-identical, broken sites recovered), and it was demanded up front in the
tick's hypothesis rather than noticed afterwards.

`timeline.com` is the whole point in one row: it was UNSCORED (`tree-divergence`) and is now SCORED,
at shape 33.9% with all three jarring invariants dirty. **That is a fail, and a fail is a result** —
1055 elements of real per-element evidence where the instrument previously had two.

⚠ **The recovery is NOT uniform, and the two weak rows say why.** `a1.ro` reaches 16 of a possible
685 and `kroftools` 38 of a possible ~440: a per-tag ordinal absorbs an inserted `<div>` among
`<header>/<main>/<footer>`, but **not an inserted `<div>` among other `<div>`s** — and near the root
that is the common case. The residue is named, gated (`a_key_survives_an_inserted_sibling_of_another_tag`
pins both the property and its limit), and is the next lead: group the ordinal by **tag AND class
signature**, which puts the sig's discrimination back into the key's *counting* without putting the
sig back into the key's *text* — the t550 finding and this one satisfied at the same time.

## The chunked sweep's spawn-loop arithmetic — a constant budget against a variable workload (t824)

**This is why three consecutive corpus sweeps (t820, t821, the aborted t824 run) were unbankable, and
why the loop's headline had no honest slope for ~12 ticks.**

### The mechanism, in the order the log prints it

```text
  UNMEASURABLE [timeout-150s]: this engine did not finish the site inside its own budget …
  mozilla::detail::MutexImpl::~MutexImpl: pthread_mutex_destroy failed: Device or resource busy
    ⟳ chunk 0 exited early with 97 site(s) unrun — re-spawning (round 1)
```

Three lines, and they were read bottom-up. Read top-down they say:

1. A site spent its per-site budget. The sweep's **own watchdog** wrote the site's `timeout` row,
   cleared the in-flight marker, and called `std::process::exit(0)` — deliberately, because the main
   thread is wedged in whatever took too long. This is documented at the call site and is correct.
2. `std::process::exit` skips every thread-local destructor, so SpiderMonkey's `JS_ShutDown()` never
   runs and its C++ statics fault on the way out. **`engine/js/src/spidermonkey.rs` predicts this
   message, in these words, in its own doc comment.** It is the signature of an un-torn-down engine at
   exit — *not* a crash, and not a Bar-0 event.
3. The parent notices the child died with sites unrun and re-spawns the remainder.

### The defect: `CHUNK_ROUNDS = 4`

A chunk child exits **once per slow site**. The re-spawn cap was a constant `4`, so a bucket could
absorb exactly four of them. A 100-site bucket carrying a dozen slow sites burned its budget on the
first four and filed the ~90 sites *behind* them — most never opened — as **`crashed`**, which is a
Bar-0 event that outranks every visual divergence in the priority ledger.

| sweep | `crashed` | `bot-wall` | scored |
|---|---|---|---|
| t812 (honest) | 25 | 33 | 87 |
| t820 | **118** | **10** | 40 |

⚠ **The falling `bot-wall` count is the tell.** Sites cannot be *classified* as bot-walled if their
chunk never opened them, so a `crashed` count that quadruples while every other reason *shrinks* is
arithmetic about the instrument, not about the corpus.

### The fix

* **`chunk_round_budget(n) = n + 4`** — the budget scales with the bucket, so the pathological case
  (every site times out) is absorbable. It does **not** multiply the run's cost: every round makes at
  least one site's progress, so wall-clock stays bounded by the sum of the per-site budgets.
* **`CHUNK_STALL_LIMIT = 2`** is the real terminator. A child that exits over a timeout *wrote that
  site's row* — that is progress. A child that produces nothing twice running is failing to start,
  which is the only condition worth giving up on.
* **`Unmeasurable::NeverRan` (`never-ran`)**, split out of `Crashed`. An instrument budget and an
  engine fault are different events and must not share a string. It still counts against the bar —
  `fidelity-progress.sh` lands any unrecognised reason in-scope, which is the conservative side.
* **The deliberate exit announces itself** on the line above the fault, so the teardown message can
  never again be read as the cause of the death.

### Verified live, on the sites that did it

`--jobs 2` over 10 sites including `bbs.ruliweb.com` and `www.bilibili.com` (the two that killed
chunks): **10 sampled, 10 rows, zero `crashed`, zero `never-ran`.** `bbs.ruliweb.com` timed out, was
labelled, and the chunk re-spawned; `www.bilibili.com` produced a score of 0.549; and
`janitorai.com` — previously *recovered as `crashed`* — classified itself as `bot-wall-403`, which is
the direct receipt for the inference t821 drew from the histogram alone.

**Gate:** `chunk_spawn_budget` (`tests/wpt/src/fidelity.rs`) — four tests simulating the loop's
arithmetic with no processes spawned. RED-proven by restoring the constant.

⚠ **`manuk-wpt` is in neither the wall's crate-test list nor CI's**, so nothing runs that gate
automatically. Reported to the observer; both files are observer-owned.

⚠ **The lesson worth carrying: the last line before a death is not the cause of it.** The line that
named the cause was one line higher in the same log, three separate times. Make every deliberate exit
announce itself, so an unlabelled fault is the only kind left.

## Attributing a per-site sweep delta (t847) — the integer test is NOT the control

A fidelity sweep row is `shape = passed / shape_n`, so a change of `Δ` on a site with `n` scored
elements corresponds to `Δ × n` elements changing verdict. When that product lands on a whole
number, the delta is **real** — a definite set of elements flipped, and it is not a rounding
artefact or a re-binned average. That is the whole of what the integer test proves, and t846
overstated it.

**It does not say WHO flipped them.** A site that serves a slightly different page between two
visits — one more ad slot, a lazy image that arrived, a cookie banner that did not — also flips an
integer number of elements. So does the sweep's own load: the run is two rendering processes against
the live internet under a per-site time budget.

t847 cut `t842 → t847` down to the 21 rows with **identical coverage AND identical `shape_n`**, the
strictest same-page filter the TSV supports, then ran both binaries against seven of them in the
same hour:

* **Not one of the five tested losses reproduced.** `gismart.com` read `0.679715`, `0.654804` and
  `0.729537` (twice) *on the same page, at byte-identical coverage and byte-identical element
  count*. `developers.google.com`'s two sweep rows turned out to be its two binaries' values with
  the labels **inverted**.
* **The two real wins reproduced exactly**, and a three-point ladder (old tree / one-commit-later /
  HEAD) placed the whole of each one on a single commit.

**Identical coverage plus identical `shape_n` is not "the same page."** The rule:

> The integer `delta × n` test separates a real verdict change from a rounding artefact.
> **Only a same-hour run of the OLD BINARY separates the ENGINE from the DAY.**

### A sweep row is a LOWER BOUND on the same binary's solo reading

Re-running HEAD alone against its own t847 sweep rows: `gismart` +0.075, `possssno` +0.108,
`developers.google` +0.018, `pivaldi` +0.005, and `celeb.gate` / `ubys` / `mobcup` byte-identical —
**four up, three equal, zero down.** Sweep contention depresses some sites and inflates none, so the
headline (M1) is a floor rather than a point estimate, and a per-site sweep delta is not evidence
about the engine until the same binary reproduces it *alone*.

### A corollary about corpus reach

The same window's other fix (CSS 2.1 §10.3.3 under `rtl`) moved nothing, and the population says why
rather than the fix being wrong: **5 of the 101 scored sites carry any RTL markup at all.** A corpus
that cannot exercise a mechanism cannot price it, in either direction — which is an argument about
the corpus, not a verdict on the fix.

## An oracle timeout is the REFERENCE hanging, and for 57 ticks it was booked as ours (t861)

**`bbs.ruliweb.com` and `www.friulioggi.it` carried `timeout-150s` in NINE consecutive sweeps** —
t800, t807, t812, t820, t825, t832, t842, t847, t857 — and in all that time nobody asked *whose
clock burned*. The reason string said only *"a child process did not return within 150s and was
killed"*, and every consumer of the ranked backlog reads that as this engine.

**It is Google Chrome. Measured, with a control, same snapshots, same flags, one binary apart:**

```text
                        chromium    google-chrome-stable    OUR ENGINE
  www.friulioggi.it       1.04s       >120s  (killed)         27.5s
  bbs.ruliweb.com         1.10s       >120s  (killed)         34.0s
  blog.rust-lang.org         —         2.26s (scores)            —      <- the control
```

`chrome_bin()` prefers `google-chrome-stable` over `chromium`, so **the oracle runs the half that
hangs.** Our engine renders both pages; `curl` serves both in under 2s, interleaved against our own
fetch in the same seconds.

### The four wrong turns this took, because each one looked like the answer

1. **"The origin is slow."** `curl` returned 200 in 1.3–2.0s. Refuted.
2. **"Our net stack is slow."** It looked that way — one run logged `timed out after 30.0s` on a URL
   curl served in 1.3s. Then the *same* curl hung on the next attempt: `bbs.ruliweb.com` is
   **intermittent**, and the first comparison was two different moments wearing one conclusion. A
   tight interleave (curl, ours, curl, ours ×4) is what made it attributable — and it showed ours at
   33.8–40.0s against curl at 1.5–2.0s, which is real but is **not** what the row was measuring.
3. **"It is the injected probe."** `pathOf` walks every previous sibling for `nth-of-type`, which is
   O(n²) on a page with 1199 sibling `<a>`s, and `capture()` runs up to four times. Plausible,
   cheap to test, and **wrong**: Chrome + probe finished in 1.04s.
4. **"It is the screenshot."** Also wrong — 1.05s, and the run's own output proves it, because the
   visual score printed **75.0%** on a row that came back UNMEASURABLE. The screenshot had succeeded.

**Every one of those four measurements was run against `chromium`, because that is what
`command -v chromium` finds and what a human types.** The instrument runs `which
google-chrome-stable` FIRST. Four consecutive refutations were all measuring a browser the oracle
never invokes — the t858 lesson (*replicate the instrument, not your model of it*) reappearing one
level down, in the **binary** rather than the code path. `chrome_bin()`'s candidate list is four
names long and the answer changes with the box.

> **When a subprocess is the subject, the resolved PATH is part of the instrument.** Print it, or
> reproduce with `$(which …)` in the same order the code does.

### What changed, and deliberately what did not

`Unmeasurable::OracleTimeout(u64)` now carries the two Chrome-side deadlines
(`capture_seen_all_paths`, the screenshot); the sweep's own per-site budget keeps the bare
`Timeout`, and its text now says out loud that it bounds **both engines together**, so it cannot be
read as ours either.

⚠⚠ **THE DENOMINATOR DOES NOT MOVE.** `oracle-timeout-N` is COUNTED and UNSCORED exactly as a plain
timeout was, and the gate asserts it against the EXCLUDED partition. "The reference failed" is the
most tempting licence this instrument has ever been offered to launder its hardest sites out of the
denominator and raise the headline for free — the `EXCLUDED-RISING` failure §0's fixed-denominator
rule exists to forbid. **The comparison being unsound is a reason to stop mis-attributing it, never
a reason to stop counting it.** What this buys is attribution, not arithmetic: the ranked backlog
stops selling engine ticks for a defect in the reference binary.

Legacy rows still parse: `timeout-150s` reads back as `Timeout`, because nobody knew whose clock
those nine sweeps measured and re-labelling history would invent a measurement that was never made.

Gated by `an_unscored_site_must_name_its_cause` — RED-proven in both directions (drop the
`oracle-timeout-` parse arm and the round-trip reads back as `Timeout`, re-blaming us; strip the
word ORACLE from `explain()` and the attribution assertion fails).

### The third cohort in a row that was not ours

`shell-only` (t856), `css-starved` (t860), `timeout` (t861). Three consecutive named-as-ours cohorts,
three cheap tests, three refutations. The sweep's printed `SCORABILITY CEILING` is a **floor on our
fidelity, not a ceiling on our engine** — and the `css-starved` string, which t860 falsified but did
not rewrite, is corrected here too. A reason string is re-read on every sweep; leaving a falsified
one in place re-sells the same wrong tick forever.

## A quiet UNSCORED label can be hiding a Bar 0 (tick 863)

`redemoura.gupy.io` was filed `thin-overlap-2` by the t857 sweep — an unscored row, one of twenty-odd,
the kind that gets read as *"the instrument could not measure it"*. Re-measured SOLO on the current
release binary it **segfaults, 9 of 9**.

`Unmeasurable::ThinOverlap` and `Unmeasurable::Crashed` are not neighbours on any severity scale. The
instrument's own text for the second one says *"our own bug, like render-failed, and the most expensive
kind"*, and Part 24.3 puts a crash above every visual divergence in the ledger. **One sweep's quiet
label was carrying a Bar 0 for the length of that sweep's life.**

Read next to the three preceding re-reads of a reason string — `shell-only` (t856), `css-starved`
(t860), `timeout` (t861) — this is the fourth, and the first that moves blame **onto** this engine
rather than off it. A reason string is re-read on every sweep by whoever is picking the next tick, so
being wrong in the flattering direction is not the only way it costs.

### Four discriminators localized a nine-of-nine segfault with NOT ONE symbol

The gdb backtrace is **eleven consecutive `??` frames** with NaN-boxed values on the stack — entirely
inside statically-linked SpiderMonkey, no frame of ours. Nothing in it names a subsystem. What named
one was four cheap A/Bs, none of which needed a symbol:

| probe | result | what it eliminates |
|---|---|---|
| **the OLD-BINARY control** (previous tick's sources, rebuilt, re-run the same hour) | 2/2 crash | it is not this tick's change — run this BEFORE believing anything, it has flipped a verdict three times |
| **debug build, same URL** | 0/4 crash, scores normally | not a logic bug: release-only ⇒ the heap-corruption class |
| **`MANUK_LOAD_BUDGET_MS=1`** | 0/1, loads clean to the end | the external-script phase fetches nothing in 1ms, so there are no deferred script BODIES — the fault needs them |
| **`boxes --fetch` (the SYNC `Page::load`)** | exit 0 | the same discriminator from the other side: sync load has no network to fetch external script bodies with |

Together they put the fault inside **`page.run_deferred_scripts(...)`** in `load_async` — confirmed by
the phase ledger, whose last line before the fault is `author CSS applied before the lifecycle events`,
the statement immediately preceding that call, and whose `deferred scripts` phase never completes.

### Why a dead reproducer is worse than an open bug

This crash class has been open and TRACKED for a long time; what it had not had since t650 is a
**reproducer**. The note that parks it records the reproducer as needing *"~10 sites of allocation
churn, never alone, never on 8× one site"* — which is another way of saying nobody could attach a
debugger to it. **A parked bug's DIAGNOSIS outlives its REPRODUCER, and the diagnosis is what gets
remembered**, so the parked verdict kept being inherited while the thing that could act on it rotted.
One URL, one process, 9/9 is the difference between a tracked bug and an actionable one.

It is deliberately **not fixed here**: localizing the corrupting write needs ASAN or an instrumented
mozjs rebuild, and in-process containment of a SpiderMonkey memory fault is settled as unachievable
without the process boundary. The tick pays what it can pay — the reproducer and the attribution.

[[fidelity-instrument]] [[js-engine]] [[architecture]]

## A control that changes a VARIABLE without changing the MECHANISM is silence, not a refutation (tick 865)

The `Unmeasurable::ShellOnly` doc comment carried this correction from tick 674:

> **⚠ THE CAUSE THIS COMMENT ASSERTED WAS WRONG, AND IT WAS NEVER MEASURED.** It read: *"from
> `file://` the page's own origin is `null`, so a JS-rendered site's fetches and module loads are
> cross-origin and blocked."* Plausible, stated as fact, and load-bearing — it would have bought a
> loopback HTTP server. One probe killed it: the **same document served over `http://127.0.0.1` gives
> a byte-identical dump.**

Every sentence of that is well-reasoned, and the refutation is **empty**. `127.0.0.1` is *just as
cross-origin to the site as `file://` is.* The probe moved the document from one foreign origin to a
different foreign origin — both arms are the treatment, neither is a control, and a null result was
guaranteed before it ran. The arm reproduces today (comix.to 1 div either way, pt88.app 2 either way)
and still says nothing.

The control that actually varies the mechanism is **removing the CORS check**:

```text
  pt88.app, identical snapshot, identical flags
    http://127.0.0.1                            2 divs
    http://127.0.0.1 + --disable-web-security  98 divs
```

The original hypothesis was right, and it sat marked "measured false" for eleven ticks. This is the
sibling of *"every number has a harness"*: **before believing a refutation, ask which variable the
control moved and whether the mechanism could tell the difference.** A control that cannot separate
the hypotheses does not return "no" — it returns nothing, and nothing gets written down as "no".

### The mechanism, and the size of it

A `type="module"` script is **always** fetched in CORS mode. The oracle renders a fetched COPY of the
document from a foreign origin, and a site has no reason to send `Access-Control-Allow-Origin` for
its own bundle — `allticketscol.com/main-*.js` answers `200` with **no ACAO** to a foreign `Origin`.
The entry bundle never loads, the app never boots, and the reference is a shell of the instrument's
own making. Chrome renders every one of these pages from its live URL:

```text
                            oracle snapshot   LIVE url
  allticketscol.com               0 divs        312
  comix.to                        1 div        1258
  pt88.app                        2 divs        147
```

**8 of the 13 in-scope sites carrying `shell-only`** ship module scripts — the largest unscored
cohort, and an instrument defect rather than engine backlog.

### The cheap fix was measured and REFUSED

An INLINE module is not CORS-fetched, so the snapshot can inline every bundle it fetches. Four lines:

```text
                     snapshot   INLINED   live
  pt88.app              2         71       147
  allticketscol.com     0         15       312
  comix.to              1          2      1258
```

It boots the app and then stops, because the booted app's own same-origin `fetch()` is still
cross-origin from the snapshot. **A half-built reference is strictly worse than an honest shell:**
the count clears the shell floor, `ShellOnly` stops firing, and the instrument starts scoring our
complete render against Chrome's partial one *as though the difference were ours* — manufacturing
eight sites of phantom engine backlog while looking like progress. That is
*absence routes to the fallback; HALF-presence routes into a wall*, aimed at the instrument.

What landed instead is the LABEL — `oracle-module-shell-N`, decided in the one place `ShellOnly` is
decided, from a scan of the bytes the ORACLE was handed. **Counted and unscored; the denominator does
not move.** The fix that would work is a loopback reverse proxy serving document, subresources and
XHR under one origin — named, sized, and left to its own tick.

[[fidelity-instrument]] [[frameworks]] [[js-engine]]

## A 0% on a suite nobody has run is indistinguishable from a capability zero (tick 870)

The WPT accessibility suites — `accname`, `wai-aria`, `html-aam`, 1,250 subtests, written by the
spec authors and scored by all four vendors in Interop's accessibility investigation — had never
been run here. Wiring them up produced, in order:

| attempt | reading | what was actually wrong |
|---|---|---|
| shim `test_driver` before the head scripts | **0 / 1250** | `testdriver.js` loads after and assigns its OWN `test_driver`; mine was replaced |
| shim `test_driver_internal` before the head scripts | **0 / 1250** | `testdriver.js:2423` assigns `window.test_driver_internal = { … }` **wholesale** — replaced, not merged |
| shim `test_driver_internal` at the top of `<body>` | **797 / 1250** | — |

Two clean, total, entirely-harness zeroes on a capability that had genuinely never been measured. The
finding *"the accessibility tree scores 0% on the spec's own tests"* was one commit away from being
banked, and it would have been believed, because it is exactly the number an unmeasured subsystem is
expected to produce.

**What separated them was refusing to publish a score without reading a failure MESSAGE.**

```text
  promise_test: Unhandled rejection with value:
    object "TypeError: window.test_driver_internal.get_computed_label is not a function"
```

That is not an assertion about accessibility. **The score said "the engine"; the message said "the
harness."** A score is a single number that any of a dozen causes can produce; a failure message
names one. This is t650's *"100% of nothing is 100%"* inverted — **0% of nothing is 0%** — and the
rule generalises past both:

> **The first run of a new instrument measures the instrument.** Do not report its number as a
> finding about the subject until at least one failure has been read and found to be about the
> subject.

### The sibling error, in the same tick

The predicate deciding which testdriver tests to admit scanned each TEST file for `test_driver.` and
found none — because the calls live in the shared `/wai-aria/scripts/aria-utils.js` those files
import. It skipped **60 of the 61 files it was written to admit**, and reported that as a clean
"needs testdriver" skip. An instrument's *filter* fails as silently as its *measurement*, and neither
shows up as an error.

### And the skip reason had been true of the file and false of the test for the whole corpus

The rule read `if body.contains("testdriver.js") → "needs testdriver (synthetic input)"`. Its own
reason names what it is about — **synthetic input** — and these suites import `testdriver.js` for
exactly two **read-only** accessors that synthesise nothing. A file-level skip hid a spec-authored,
1,250-subtest suite behind a sentence that was accurate about the import and wrong about the test.
Keyed on the actions the file actually calls now, and conservative: an unknown `test_driver.*` call
means "not ax-only", so a suite that grows a new dependency is skipped again rather than silently
reporting failures that are the harness's.

[[browser-capabilities]] [[fidelity-instrument]]

## `--window-size` is a WINDOW size, and two defects were hiding each other behind it

Measured on Chrome 145 `--headless=new`, asking the page for `document.documentElement.clientHeight`:

```text
   --window-size=1200,600   ->  viewport 1200 x 513
   --window-size=1200,800   ->  viewport 1200 x 713
   --window-size=1200,1000  ->  viewport 1200 x 913
   --window-size=800,800    ->  viewport  800 x 713
```

A **constant 87px** on the block axis and **zero** on the inline one. Every reference capture this
project has ever taken laid the page out in a viewport 87px shorter than the one our engine was told
to use — so every `vh` in the corpus was compared against a 12.2%-different height. `vh`/`vw` is
declared by **73.1%** of the burndown corpus and `min-height: 100vh` — the full-bleed hero — by
**36.3%**.

The correction belongs in the instrument, and it is **measured rather than hard-coded**: the offset is
a property of the Chrome build and platform, so `viewport_chrome_offset()` probes it once per process
with a one-line document, caches it in a `OnceLock`, and `base_flags` asks for
`window = requested_viewport + offset`. A failed probe returns `(0, 0)`, which is exactly today's
behaviour — the instrument degrades to what it already did rather than to something new.

### ⚠⚠⚠ The reason this stayed invisible: the engine had the mirror-image bug

`manuk_css::values::VP_H` — the viewport height every `vh` resolves against — is a global that
defaults to **720** and **has no caller**. `cascade_styles` calls `set_viewport_width` (which
deliberately preserves "the last-known height"), and nothing ever sets that height. So:

```text
                          100vh resolves to
   our engine                   720      (the never-updated default)
   the reference                713      (800 asked for, 87 taken by the window frame)
   what BOTH should say         800
```

**The two defects agreed to within one percent, and each one alone is worse than the pair.** Fix the
instrument only and the divergence goes from 7px to 80px. Fix the engine only and it goes from 7px to
87px. Neither shows up as a regression against the other, and the pair is the only correct state —
which is why the sweep has never flagged viewport units despite three quarters of the corpus using
them.

> **Two errors of similar size in opposite directions read as agreement, and a differential
> instrument cannot tell that from correctness.** The only thing that separates them is asking a
> third party — here, `document.documentElement.clientHeight` and the spec — what the answer should
> be, rather than asking the two implementations whether they match.

**Both halves are now built** (instrument t1016, engine t1017), and the pair measures exactly:

```text
                      Chrome    before    after
   height: 10vh         80        72        80
   min-height: 50vh    400       360       400
   width:  25vw        300       300       300     <- the CONTROL, and it names the defect
```

⚠ **The width row is what makes this precise.** It was right the whole time, because
`set_viewport_width` *does* have a caller — so the defect is not *"viewport units are
unimplemented"*, it is *"one of the two axes has a caller and the other does not"*. A fixture with
only `vh` rows would have supported the wrong diagnosis and sent the fix at the unit parser.

The height is set **once**, in the binary's `main`, from the same `--width`/`--height` flags every
subcommand parses with the same defaults — rather than at each of the six sites that re-parse them.
The global is per-process; one authoritative write is easier to reason about than six that must
agree.

### ⚠ And the third caller: the SHELL had none either (t1019)

t1017 gave `manuk-wpt` a viewport-height caller, which fixed the number the loop is scored on. The
**shipping browser** still had none: `Gui::build_page` and `build_page_contained` call `Page::load`
with a width, and nothing published a height — so a `min-height: 100vh` hero was **720px tall in a
1080-tall window and 720px tall in a 600-tall one**.

Published at the first point that knows the answer — the content viewport is
`window − CHROME_TOP`, which `Page::load`'s width-only signature cannot see. ⚠ And the viewport is
now sized **before** the build rather than after: the old ordering built the page, *then* assigned
`self.viewport`, so the first navigation after a resize would have cascaded against the size the
window used to be.

⚠⚠ **What the gate can hold, and what it cannot.** `Gui` needs a real window and cannot be
constructed in a test, so the call site itself is not asserted — the rule (`content_viewport`'s
arithmetic) and its consequence (a `100vh` box laid out against it) are. The `!= 720` assertion is
load-bearing: 720 is the value the defect produced, so a fixture that happened to pick a 720px
content height would pass while measuring nothing.


## The reference browser had no mouse, and 22.9% of the corpus asks (t1020)

The `@media` battery — 31 rows, the top unbatteried construct in the corpus at **49.1%** — came back
**30 Chrome-exact**. The thirty-first was not ours.

Asked directly with `matchMedia`, Chrome 145 `--headless=new` answers:

```text
   (hover: hover)      false      (hover: none)        true
   (any-hover: hover)  false      (any-hover: none)    true
   (pointer: fine)     false      (pointer: none)      true
   (any-pointer: fine) false      (any-pointer: none)  true
```

Not *coarse*, not *unknown* — **`none`**, the value reserved for a device that cannot point. Every
other feature in the battery agrees with us exactly (`prefers-color-scheme`, `prefers-reduced-motion`,
`scripting`, `display-mode`, `color`, `min-resolution`, `forced-colors`, `update`), which is what makes
this attributable rather than a general mismatch. Our engine says `hover: hover` / `pointer: fine`,
and it is **right** — this is a desktop browser with a mouse.

> **A reference that renders a branch of the stylesheet no user of the browser under test will ever
> see is not a reference for that browser.** The correction belongs in the harness. Correcting the
> ENGINE instead would have made the shipping browser wrong for every real user in order to move a
> number, and it would have looked like progress.

### The decision rule, now with three subjects and two branches

`--hide-scrollbars`, `--window-size` (t1016) and this are one class: **the reference was not rendering
the page we asked for, and the difference was charged to the engine.** t1010 and t1018 split
*"the oracle cannot see it"* into two facts; the mis-provisioned branch splits the same way, and the
discriminator is **whether the reference CAN be provisioned**:

```text
   hyphens: auto (t1010)   dictionaries are a SEPARATE COMPONENT of the browser  -> unfixable,
                           Chrome would differ if it had them; it does not          do not build
   the pointer family      Chrome HAS the capability, it is CONFIGURED off       -> one flag,
                                                                                    fix the harness
```

`--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4`
(`HoverType::kHoverHoverType == 2`, `PointerType::kPointerFine == 4`). ⚠ **Run the control arm before
believing a flag**: asking instead for `HoverType=1,PointerType=2` yields `hover:none` /
`pointer:coarse`, which is what proves the flag is doing the work rather than coinciding with a
default.

⚠ **Set, not probed, and the difference from `viewport_chrome_offset` is deliberate.** The viewport
offset is a fact about the Chrome build that only measurement can supply. This is a **configuration we
are choosing**: the value we want is fixed by what Manuk *is*, not by what Chrome defaults to. What
stays falsifiable is whether the flag still takes effect — which is what the gate asserts, by laying
out a `@media (hover: hover)` rule through the real capture path.

### Pricing it, and the grep that lied by half

Stylesheet-inclusive over the corpus that produces M1 (170 sites with a real body, 551 stylesheets):

```text
   (hover: hover)     32/170   18.8%
   (pointer: fine)    24/170   14.1%
   (hover: none)      13/170    7.6%     <- the reference was applying THIS branch
   (pointer: coarse)   8/170    4.7%
   ANY of the four    39/170   22.9%
```

⚠ **`hover\s*:\s*hover` first returned 47 sites, and the over-count is generic.** It matches a CSS
*class named `hover`* followed by the `:hover` pseudo-class — precisely what Tailwind emits
(`.hover\:bg-x:hover`). **A media-feature grep must be anchored on the opening paren**, or a
utility-class framework inflates it by half.

### The gate, and why its negative probe is the load-bearing one

`tests/wpt/corpus/media-interaction.html` — five probes, which puts this in the wall **for free**:
`parity` already runs there and fails on `probes_passed < probes_run` with no retry. RED-proven by
commenting the flag out: `media-interaction 1/5`, `TOTAL 73/77`. Three probes fail because the
reference stops matching `hover`/`pointer`; **`p-nohover` fails in the OPPOSITE direction**, because
the reference starts matching `(hover: none)` while we do not.

> **A fixture whose failures all point one way can be satisfied by one wrong constant.** The negative
> probe is what makes the correction falsifiable in both directions — the same reason t1016's `vw`
> control row is what named its defect precisely.

### What the battery cleared, and the one row that discriminates

Twelve rows that must NOT match (`print`, `min-width:1201px`, `max-width:1199px`, `min-height:801px`,
`orientation:portrait`, `prefers-color-scheme:dark`, `min-resolution:2dppx`, `pointer:coarse`, an
unknown feature, `not screen`, an all-false comma list, a range `400px <= width <= 800px`) and
eighteen that must (both inclusive boundaries at exactly 1200, `screen and`, `only screen`, a comma
list with one matching arm, `min-height`/`max-height`, `orientation:landscape`, `width >= 600px`, an
`and` pair, nested `@media`, `aspect-ratio: 3/2`, `min-resolution:1dppx`, `all`, and the
`<style media>` **attribute** in both directions).

The row that earns its place: **`@media (min-width: 70em)` under `html{font-size:20px}` matches** —
a media-query `em` resolves against the **initial** font size and never the root element's, so it is
1120px and not 1400px. Nothing else in the battery could tell those two implementations apart.

⚠ And `engine/css/src/lib.rs:2889` warns that the Stylo cascade and the JS `matchMedia` shim need not
agree on an identical query. Run on the same five queries in one page, they returned the same five
answers.

## The reading-order conjunct is geometry after all, and one site said otherwise (t1084)

t1083 ended a three-tick hunt with a hypothesis and refused to act on it: `www.wdimax.com`'s footer
holds 4 links and 3 absolutely positioned separators, `4 × 3 = 12`, and its `reading_order` count is
exactly 12 — every in-flow ↔ out-of-flow pair in the container and no others. `jarring_reading_order`
compares every sibling pair by rect **with no notion of whether either box is in the flow**, and an
out-of-flow box has no reading order relative to its in-flow siblings.

That could not be tested: `Seen` carried `tag`, `display`, `rect` and `font` — not `position`. So the
field was added to both probes and the count PARTITIONED rather than filtered, which is t1034's rule
and is the rule precisely because this is the shape where a filter would be a threshold tuned to move
a number. Chrome's own `position` string is the discriminator, so the classification is the reference
engine's and not ours.

```text
   site                  inversions   mixed-flow    share
   www.wdimax.com               12           12     100%
   www.ikea.com                 22           17      77%   (the same 17 are already "parked")
   rockstaractu.com             13            7      54%   (the same 7 are already "zero-area")
   m.youm7.com                  24            0       0%
   www.otomoto.pl               11            0       0%
   www.taphouse23.com          123            0       0%
   payb.jp                     264            0       0%
   ─────────────────────────────────────────────────────
   total                       469           36     7.7%
```

⚠⚠⚠ **CONFIRMED ON THE SITE IT CAME FROM AND REFUTED AS AN EXPLANATION.** 12 of 12 on the one site,
and on the two others where it is large it is **entirely inside partitions that already exist** — the
17 on `ikea.com` are the same 17 parked off-viewport, the 7 on `rockstaractu.com` are the same 7
zero-area boxes. Net new signal after those: **zero**. The three sites carrying the actual mass —
`payb.jp` 264, `taphouse23` 123, `youm7` 24 — have **no mixed-flow inversions at all.**

**So the filter is not taken**, and the conclusion t1083 reached from one site is corrected here:
`reading_order`'s bulk is in-flow against in-flow, which means it *is* a real engine target and
`www.wdimax.com` was the atypical site the loop happened to open first. **Ranking by inversion COUNT
would have picked `payb.jp` (264) and gone straight there; ranking by "cleanest to localise" picked
the one site whose defect was an instrument artefact.**

### The guard that was tolerant in exactly one direction

Adding the field broke the instrument, and the failure did not look like a parser failure:

```text
   ⚠ www.wdimax.com UNMEASURABLE [oracle-module-shell-0]: the ORACLE rendered only 0 element(s)
     — and this document is a `type="module"` SPA, so THE SHELL IS OUR SNAPSHOT
```

`parse_seen_probe_json` guarded with `if a.len() != 6 && a.len() != 7 { continue }`, whose own comment
says *"an absent datum must not silently remove the element from the diff"* — and an 8-element array
was dropped, every element, every page. **A guard written to be forward-tolerant was enumerating
lengths instead of taking a minimum**, so it tolerated the past and was silently fatal to the future,
and its symptom named a cause *on the page* for a defect in the reader. Now `if a.len() < 6`.

## A BLIND INSTRUMENT MIS-RANKS THE WORK-LIST, NOT JUST THE SCORE (t1091)

Two instrument fixes — t1088's bitmap references and t1090's Ahem face — took `css/CSS2` from 3,029
to **3,790 passing (67.3% of 5,633 reftests)**. The interesting part is not the score. It is what the
re-rank did to the *ordering*, because the loop selects work from the ordering.

The carried headline was *"`::first-letter` is **10.5%** of all remaining CSS 2.1 failures and has no
map row"*, and it was queued as the next arc. Re-derived on the fixed runner:

```text
   ::first-letter        8 of 1,843 failures =  0.4%     ← the arc that was queued
   content + counters  198 of 1,843 failures = 10.7%     ← the arc that had no row
```

**The share was real and it was attached to the wrong subsystem.** A blind instrument does not add
noise evenly; it deletes whole classes of pass, and the classes it deletes are correlated with the
mechanism, so the surviving failure set is a *biased sample* that names the wrong cause with
confidence. `margin-padding-clear` shows the same thing at the other end: carried as *"~280, one
unidentified shared cause, three hypotheses already refuted"*, it is now **66 failures at 90.3%
pass**. Three hypotheses were refuted against a number ~4× too large — which is exactly why none of
them explained it.

> **Re-derive the RANKING after an instrument fix, not just the score.** A score that moves tells you
> the fix worked. A ranking that moves tells you which of your queued ticks were selected by the
> defect.

### ⚠⚠⚠ A REASON STRING IS A PROPERTY OF THE READER — grouping by it groups CAUSES together

Skips partition by the runner's own reason strings, and one group looked like the next lever:

```text
   2,831  not a reftest (no rel=match/mismatch)   correct — testharness tests, a DIFFERENT runner
     461  needs JS/testharness                    correct and honest
     254  reference unreadable                    ← "bounded, ~254, the next tick"
```

That reading was written down and it was wrong. Checking each of the 254 against the filesystem
rather than inferring the mechanism from the shape of the `href`:

```text
   239  the reference is genuinely ABSENT from this checkout  (`wpt/css/reference/` is not there)
    14  the file EXISTS and the runner could not resolve it   ← the only real bug
```

**The lever is 14, not 254 — off by 17×.** And the accused code was innocent: `Path::join` handles
`../..` lexically, so `../../reference/x.xht` resolves fine *when the directory exists*. The 14 are
the server-root-absolute form, `href="/css/CSS2/…"`, where `dir.join("/abs")` discards the base.

This is the **third consecutive** false lever produced by counting rows that share a reason string —
1,231 external stylesheets (really one absent file), 254 unreadable references (really 14). Both
collapse into one provisioning fact: **the WPT checkout is PARTIAL** — `wpt/fonts/` and
`wpt/css/reference/` are both missing, and between them they explain 1,640 stylesheet links and 239
skips. Neither is a defect in the engine or the runner.

> **One `[ -f ]` per row, before the row becomes a plan.** A uniform reason string is evidence that
> one *reader* took one branch — never that one *cause* was present.

**LANDED at t1100 — the 14, and only the 14.** WPT serves its corpus over HTTP, so a leading `/` is
the *server* root; on disk that is the checkout root, and `Path::join` with an absolute argument
discards the base. `css/CSS2` **3,854 → 3,858** and `reference unreadable` **254 → 240**. Four of the
fourteen pass and ten now fail for real reasons — the honest shape of an instrument fix, and why the
headline is +4 rather than +14. The gate asserts the `..` form is **left alone**, because that was
the half accused at t1091 and found innocent: rewriting a path that was already right is how a fix
for one cause breaks the other.

## AHEM IS THE SUITE'S RULER — 1,090 CSS 2.1 reftests (17.4%) measure with a font that was not installed (t1090)

A CSS 2.1 test does not compare two renderings of prose. It lays text out in **Ahem** — a face whose
every glyph is exactly `1em × 1em`, with an `0.8em` ascent and `0.2em` descent — so that a line box's
geometry becomes an *arithmetic fact*, and the reference can then draw the same rectangle with a
`background-color`. `linebox/line-height-102.xht` is the house style entire: `font: 20px/1 Ahem`,
`width: 1em`, *"the 2 vertical black stripes have the same height"*. Substitute any other face and the
test measures that face's metrics instead, so it can only fail.

The suite declares the dependency itself, which makes the corpus partitionable without opening a file:

```text
   1,406 css/CSS2 files declare  <meta name="flags" content="ahem">
   1,090 of those are REFTESTS   = 17.4% of the suite's 6,263
     786 …whose reference does NOT use Ahem   → unpassable by construction
     295 …whose reference DOES use Ahem       → both sides in the fallback face
```

`fc-list | grep -i ahem` was **empty**. Installing the vendored `Ahem.woff2` into the reftest runner's
`FontContext` — `register_font`, so it enters `fontdb` under its own internal family name exactly as
`fc-cache` would have — took all 41 measurable directories from **3,458 to 3,790**: 336 gained, 4 lost.

### ⚠⚠⚠ It is NOT a missing external-stylesheet fetch, and counting the construct said it was

1,640 of the suite's 1,707 `rel="stylesheet"` links point at one URL, `/fonts/ahem.css`. That makes
*"the runner fetches no external stylesheets"* look exactly like the mechanism, and it was priced at
19.7% on that basis — **the right size and the wrong lever**, because `wpt/fonts/` is not in the
checkout at all. A runner that fetched external stylesheets flawlessly would have fetched a 404.

> **A lever priced by COUNTING a construct is not priced until you have read what the construct POINTS
> AT.** t1088 counted `<img>` and was right because the PNGs were on disk. The follow-up counted
> `<link>` and was wrong for want of one `ls`.

WPT's own runner contract is not *"fetch this stylesheet"* — it is **"Ahem must be installed on the
host."** Provisioning the host is the fix; the stylesheet is a symptom of it.

### ⚠⚠ A net-zero directory is not evidence of anything — diff the STATE, not the count

`borders` has 54 ahem-flagged reftests and moved by **exactly zero**, which is the shape t1088 got
burned by (`backgrounds` 184 → 123, two blank renders cancelling into an apparent agreement). Diffed
per-test instead of believed: **0 gained, 0 lost**, the same 36 of 54 passing on both binaries. A
border drawn round a box does not depend on the metrics of the text inside it — the flag is declared
and the geometry under test is font-independent. **No net delta can distinguish "inert" from "36 in,
36 out."** Only the per-test state diff can, and it costs one `comm`.

### The 4 losses were each an accidental pass, and each named a real defect

Installing a correct instrument can take a test from passing to failing, and when it does, **the new
number is the true one**. Both mechanisms here were invisible while the ruler was missing:

| Test | Why it passed before | The defect it now names |
|---|---|---|
| `normal-flow/min-height-104`, `-106` | `overflow:auto; width:200px` on `XXX` — in the fallback face that string is ~167px, **under** 200, so nothing overflowed and no scrollbar was needed | In Ahem it is exactly 300px. We do not subtract the horizontal scrollbar's height from the containing block. |
| `fonts/font-family-013`, `fonts-013` | `font-family: "Ahem", "Arial"` over `Ţ ę ş ţ` — glyphs Ahem deliberately lacks. With no Ahem, the first family was skipped whole | **Per-character fallback**: a selected face missing a glyph must fall through to the next family. We keep the face. |

## A REFERENCE IS A DOCUMENT — 1,230 CSS 2.1 reftests were unpassable by construction (t1088)

The reftest runner rendered both sides with the **sync** `Page::load`, which parses and lays out and
fetches **no subresources**. The CSS 2.1 suite's house style — Microsoft's and Gérard Talbot's, which
is most of the corpus — is to draw the *expected result* out of coloured swatch PNGs:

```xml
  <div><img src="support/blue15x15.png"  width="5" height="96" />
       <img src="support/swatch-orange.png" width="5" height="96" /></div>
```

So the reference painted two blank boxes while the test painted the real borders, and the comparison
could only ever say *"render differs"*. Measured on `positioning/right-004.xht`, the pixel row where
the borders belong:

```text
  reference, sync path      …white white white white white…    <- the swatches never loaded
  reference, with images    …blue blue blue orange orange…
  the TEST, either way      …blue blue blue orange orange…     <- the engine was always right
```

**Scale: 1,230 of `css/CSS2`'s 6,263 reftests (19.6%) have a reference containing `<img>`** —
`normal-flow` 276, `backgrounds` 236, `positioning` 220, `borders` 130, `linebox` 111,
`floats-clear` 90, `bidi-text` 48, `css1` 39, and a tail. Every one of them was unpassable.

⚠⚠⚠ **AND IT LOOKED EXACTLY LIKE A BROKEN ENGINE PRIMITIVE.** All 50 RTL `right-*` tests failed —
0 passing — which is the signature of an absent feature, and it is what the tick was chasing when it
opened them. **A 100% failure rate is evidence about the INSTRUMENT at least as often as about the
engine**, and the cheapest way to tell is to render the reference on its own and look at it.

⚠⚠⚠ **LOADING `<img>` ALONE IS NOT HALF A FIX — IT IS A DIFFERENT BIAS.** On that intermediate
build, `css/CSS2/backgrounds` went **184 → 123 (−61)**: its *tests* draw with `background-image` and
its *references* draw with `<img>`, so both being blank was a **cancellation that read as agreement**
(the `two-errors-cancel-and-read-as-agreement` shape, in the instrument this time). Loading both
bitmap kinds took it to **220**. `positioning` shows the same effect mirrored — 339 on the
`<img>`-only build, 314 with both, against 187 before. **Only running every affected directory on
both builds finds this; the headline (+345 on the intermediate) was larger and wrong.**

OLD-INSTRUMENT CONTROL, same hour, nine directories:

```text
                          OLD    <img> only   BOTH        net
  positioning             187        339       314      +127
  normal-flow             320        465       465      +145
  backgrounds             184        123       220       +36
  borders                 324        345       349       +25
  floats-clear             31         78        79       +48
  linebox                  14         51        51       +37
  margin-padding-clear    592        596       603       +11
  floats                   23         23        23         0
  bidi-text                17         17        17         0
  ─────────────────────────────────────────────────────────────
                                                        +429
```

⚠ **`bidi-text` is flat at 17 despite 48 image-based references, and that is the honest half.** An
unloaded PNG was masking real failures, not inventing them: where the engine genuinely cannot draw
the chapter, dressing the reference changes nothing. A directory that does not move is the control
that says this is a measurement fix and not a scoring trick.

⚠ **ONE more phase and no more.** `Page::load` stays — no JS (scripted tests are skipped) and **no
external stylesheets**, which is a second, separate absence with its own number. Bundling it would
have made the +429 unattributable.

### The struct field that broke this crate's tests for the SECOND time

`cargo test -p manuk-wpt --bin manuk-wpt` had not compiled since **t563**, when `font` was added to
`Seen` and three test constructors were missed — and the comment sitting on that constructor *says
so*, and predicts the repeat. It repeated at **t1084**, when `position` was added for the
`reading_order` partition. Both were invisible because nothing in the wall builds this crate's tests,
so the compiler's own enumeration of a field's sites is never run.

Making it compile immediately produced a finding: `a_differing_ancestor_class_signature_must_not_
book_the_whole_subtree_as_missing` asserted `kind == "missing"`, and the answer is now `"unaligned"`
— the classification introduced at **t951** precisely for *"the two trees are NUMBERED differently,
and calling that an absence is a lie"*. `oracle.rs`'s own tests were updated then; this copy could
not be, because it did not build. **A test that does not compile does not merely stop testing: it
preserves the vocabulary of the day it broke, and reads as a contradiction the moment it returns.**

## A bucketed probe loses the distinction it was built to find (t1142)

The probe encoded three outcomes as widths — 90 *correct*, 60 *present but wrong*, 30 *absent* — and
ran the same fixture through Chrome and through us. On `<img>.loading` with no attribute **both
engines answered 60**, so a real disagreement read as agreement. The second pass encoded the
**returned string** instead (`eager`→10, `lazy`→20, `auto`→30, `""`→40, absent→60) and the row came
back `chrome 30 / ours 60`: Chrome returns the legacy `auto`, we returned `undefined`.

**A probe that maps many values onto one bucket can only find the disagreements that happen to land in
different buckets.** The fix is free — report the value, not the verdict — and it is the same shape as
`--dump-dom`'s trap: the instrument agreed with itself and not with the thing it was measuring.

### The readout trick this used

The JS half of a battery can be diffed by the ordinary box instrument with no new tooling: a
`<script>` sets an element's `style.width` from what the API returns, and `cmp.sh` prices it against
Chrome exactly like a geometry row. That is how eight map rows — CSS nesting pseudos, `pow`/`sqrt`/
`hypot`/`log`/`exp`, `linear()`/`steps()`/`cubic-bezier()`, `@media (scripting)`, the `ex` unit,
`URL.canParse()`, `HTMLIFrameElement.loading` — were measured in one 25-row fixture.

**Six of the eight were already correct.** A `partial` row with no gate is not a known gap; it is an
unmeasured claim, and these were pessimistic.

## A gate that asserts a wrapped line COUNT asserts the installed fonts

t1140 landed `word_break_keep_all_suppresses_only_the_letter_unit_opportunities` with six absolute
heights measured against Chrome on the dev box. It passed here and **failed on every CI run for the
next five ticks**:

```text
   CONTROL: #ctl is h40 and Chrome says 60 — `word-break: normal` must still break between ideographs
```

`#ctl` is fourteen ideographs in a 120px box at `line-height: 20px`. This machine has a CJK face and
wraps to **three** lines; the runner has none, falls back to narrower advances, and wraps to **two**.
The wrapping is identical — only the count differs.

**The diagnosis is inside the same test.** The three `keep-all` rows, which assert *one line*, passed
on CI untouched. *One line* is a property of the break opportunities; *three lines* is a property of
the advances. So:

| assertion | depends on the font? |
|---|---|
| an unbreakable run occupies exactly one line | no — there is nowhere to break it |
| a run with an opportunity left occupies more than one | no — the opportunity is a codepoint property |
| a run occupies exactly N lines, N > 1 | **yes** — N is total advance ÷ container width |

The rule itself belongs where it cannot be font-dependent at all. `break_segments(word, keep_all)`
reads codepoints through UAX #14, so `assert_eq!(break_segments("日本語", true), vec!["日本語"])` is
the rule, and the layout rows are kept only to prove the style *reaches* it.

> **The mutation that matters is the one that severs the wiring.** Deleting the `keep_all` guard
> fails the segment assertions; the over-fix fails the hyphen control; but
> `let keep_all = false && cs.word_break == …` leaves `break_segments` **correct** and only
> disconnects it — so the segment assertions stay green and the layout rows have to catch it alone.
> Without running that third mutation, the layout half is decoration.

⚠ **And the process half, which cost more than the defect.** CI reported this correctly from the
first completed run and four subsequent ticks landed on top of it. It stayed invisible because the
repo runs two workflows and the run list shows a green `release` next to every red `CI` for the same
commit — the eye reads the green one. *Read the workflow NAME, not the colour of the newest row.*

## A RED-proof that is already green is not a RED-proof

Constitution checks #107 and #108 both named the CSS 2.1 §17.2.1 anonymous **CELL** rule as *"the
named next tick"* and both justified it the same way: *"its RED-proof already exists"* — six reftests,
listed by name. At t1147 someone ran them.

```text
   tables/table-anonymous-objects-197..200   PASS
   normal-flow/table-in-inline-001           PASS
   visuren/table-pseudo-in-part3-1           PASS
```

All six pass, and they pass *because* the rule is unbuilt: t1134's `table_run_drops_content` guard
declines to generate the anonymous table in exactly the cases that would break them. The six are the
**boundary the guard defends**, not evidence of a defect. t1134 measured the whole feature at
`+15 / −6` and landed `+15 / −0` by adding the guard, so the suite's verdict on the missing rule is
**zero, and always was**.

The corpus agreed independently, on both halves of the construct:

```text
   pages with a <tr> holding non-cell content ......  2 of 385  (4 rows)
   stylesheets with display:table-row and NO cell ..  6 of 373  (1.6%)
```

⚠ **It passes the organ test and fails on weight** — the rule moves element boxes, so the metric
*could* see it. That is the distinction worth keeping: *"the instrument is blind to this"* and *"the
web barely does this"* are two different refusals, and only the second is an I4 judgement.

> **A steer is a hypothesis with a test attached — the same standing rule as a reason string.** Two
> governing documents carried this one for eight ticks, each citing the other's citation. The step is:
> **price it · ask which ORGAN it moves · and RUN THE RED-PROOF BEFORE CITING IT.**

## WPT `fuzzy` is the author's allowance — and on this suite it banks nothing (t1156)

`<meta name=fuzzy content="maxDifference=0-2;totalPixels=0-100">` is WPT's mechanism for a reftest
whose reference cannot be byte-identical (antialiasing on a rotated edge, a gradient's dithering).
The runner now honours it: `parse_fuzzy` handles the spellings WPT ships (either key order,
whitespace, a bare number as a range of itself, an absent key as unconstrained) and **declines a
`<ref-url>:`-prefixed allowance** rather than apply one reference's tolerance to another. It applies
to `match` references only — a `mismatch` asserts the renders *differ*, and an allowance there would
mean "different by at least a little", which WPT does not define.

⚠ **Honouring it is conformance; a blanket tolerance would be loosening the bar.** The number is
chosen by the test author, per test, checked into WPT, and a test with no annotation stays
byte-exact. A default fuzz would move 6,263 tests at once on a number this loop picked for itself.

**Priced before building, and the price refuted the reason for building it.** The annotation is on
**6 files in `css/CSS2`** (282 in `css/`, 425 in the whole checkout). And every failing `match` now
prints `[maxdiff N, Mpx]`, which turns the suite into its own histogram:

```text
                   failures   maxdiff<=2   within 0-2/0-100   maxdiff>128
  normal-flow         181         0               0               165
  positioning         173         0               0               169
  floats-clear         96         0               0                94
  linebox              35         0               0                33
  ───────────────────────────────────────────────────────────────────────
                      485         0               0               461  (95%)
```

**Not one failing reftest is a near miss.** 95% differ by more than 128 on a channel — blue against
white, a box somewhere else — and even the smallest-area failures (under 100 pixels) exceed maxdiff
32. So byte-exact comparison costs this suite nothing, and *"visually-correct pages fail on 1px
antialiasing"* is not what is happening: **the plateau is the layout, not the scoring.**

The general lesson is the one t1112 and t1150 already paid for: the premise was answerable from
failures the runner was already producing and throwing away. **The instrument had the datum and
printed the verdict.**

## The WPT checkout is SPARSE, and `css/support/` holds the library the whole CSS corpus is written against

`~/wpt` is a **cone-mode sparse checkout** (`scripts/wpt-setup.sh` → `git sparse-checkout set
$SUBSETS`). Its pattern list names test directories — `css/css-grid`, `css/css-color`, … — and until
t1176 it did **not** name `css/support/`, which is not a test directory and so was never noticed.

`css/support/` contains nine **testharness helper libraries**:

```text
   parsing-testcommon.js · computed-testcommon.js · interpolation-testcommon.js
   shorthand-testcommon.js · color-testcommon.js · inheritance-testcommon.js
   numeric-testcommon.js · query-testcommon.js · serialize-testcommon.js
```

…plus `grid.css` and `alignment.css`, whose first rule is literally `.grid { display: grid }`.
Roughly 700 CSS test files `<script src>` or `<link>` one of them.

**What their absence looks like from inside the metric, and why it is nearly invisible:**

- a file whose helper 404s **throws at its first `test_valid_value(...)` call** and reports ONE
  harness error instead of its several hundred subtests. It does not crash, it does not time out
  loudly, and it does not appear as a skip;
- a `checkLayout('.grid')` file scores a page in which **`.grid` is not a grid at all**, so it fails
  on geometry that was never asked for. `css/css-grid/abspos` read **0.9%**, and 2,974 of its 3,479
  failures were the single shape `width expected N but got 0`;
- the **reftest leg is completely blind to it** — reftests do not load the helpers, so
  `css/css-grid`'s reftest count was byte-identical (210) across the repair while its testharness
  count moved +899. Two legs, and only one of them can see this class.

**Measured, one line of sparse-checkout, full sweep before and after:**

```text
   css/css-color            32/108      ->    5625/11005    +5593
   css/css-grid            558/9281     ->    1457/10911     +899
   css/css-values          471/1881     ->     877/4193      +406
   …every CSS area rose or held; NOT ONE FELL
   dom · html/dom · domparsing · url · encoding   BYTE-IDENTICAL   ← the controls
   TOTAL               433162/1228830   ->  441427/1249461  +8265 pass / +20631 counted
```

> **This is a CORRECTION, not progress — no engine code changed.** The tell is that the percentage
> barely moved (35.25% → 35.33%) while both halves grew by thousands: when a metric's numerator and
> denominator move together, the thing that was wrong is the **denominator**, and the ranking built
> on it was wrong in a way the headline could not show. `css/css-color` was listed last of seventeen
> areas with 76 failing subtests; on the repaired corpus it carries **5,380**.

⚠ **A checkout that is missing files is not a checkout that is missing tests.** The test-file counts
were unchanged across the repair (`css-grid` 3204, `css-flexbox` 1763, `css-sizing` 855) — 61 files
were added and none of them is a test. Any audit that reconciles "tests present" would have passed.
The reconciliation that catches this one is **"does every `src`/`href` a test names actually
resolve"**, which nothing runs.

⚠ Durability: `sparse-checkout set` is authoritative, so a re-run of `scripts/wpt-setup.sh` reverts
this. `css/support` belongs in that script's `SUBSETS` list.

## The engine routing bought ZERO, and the pair bought +120 (t1208)

**Half one — the engine.** Nothing on the frame-loading path looked at the response `Content-Type`,
so every framed document went through the **HTML parser**: wrapped in `<html><head><body>`, every tag
name lowercased, errors recovered from silently. `render_iframe_with_type` now routes on the MIME
Sniffing rule — `text/xml`, `application/xml`, or **anything ending in `+xml`**, the suffix that
makes `xhtml+xml`, `svg+xml`, `rss+xml` and `atom+xml` all work without an enumeration that would be
wrong for the next one.

**It moved zero.** `dom` 7397 → 7397, with the gate proving the routing works. A zero from a
*reachable, observed* mechanism is a question about the diagnosis — so read what the assertion
actually says rather than what it implies:

```text
   assert_equals: XML document didn't load
       expected "Dummy XML document"  but got  "Dummy XML document\n"
```

The frame loads, the text is right, **there is a trailing newline** — exactly the HTML-vs-XML
difference (in XML the newline after the root element is outside the document element; the HTML
parser puts it inside `<body>`). The routing was right and could never fire.

### Half two — the instrument, and it is the MIS-PROVISIONED REFERENCE class for the fourth time

```text
   tests/wpt/src/harness.rs
     "xht" | "xhtml"  =>  "text/html"               ← wptserve sends application/xhtml+xml
     "xml"            =>  (no arm) "text/plain; …"  ← nothing it served was ever XML
```

**The engine was answering honestly about the bytes it was told it had.** After `--hide-scrollbars`,
`--window-size` and the interaction media features, this is the fourth subject — and the first where
the mis-provisioning lives in a **MIME table**, a place nobody looks because it reads as
configuration rather than as a measurement input.

```text
   engine routing ALONE     dom 7397 → 7397    (+0)
   BOTH                     dom 7397 → 7517  (+120)   0 crashes, controls flat
```

**Neither half moves anything alone.** A pair whose halves are each inert is a shape this loop has
met before (t1149-1152), and it is why the zero had to be investigated rather than banked or
reverted.

⚠ **Residual, pinned by the fixture rather than hidden:** the XML parser does not preserve name case
— a `<Foo>` root comes back with `localName` `foo`. XML is case-sensitive; that is a separate defect
below `parse_xml`, asserted at its honest current value so the tick that fixes it must edit the line.

## `css/css-values` has a ±72-subtest error bar, and it comes from the ACCUM Bar 0 (t1235)

Three runs of `manuk-wpt wpt css/css-values`, **same binary, same hour**:

```text
  2168 / 4174      2168 / 4201      2240 / 4200      <- spread 72; the DENOMINATOR moves too
```

The instrument names the cause in its own output: **`ACCUM` — a file that SIGSEGVs the shared batch
runtime and passes in a fresh one — and WHICH files do it changes between runs.** One run lost
`calc-infinity-nan-computed` + `if-style-invalidation` + `viewport-units-media-queries`; the next lost
`viewport-units-css2-001` instead of `if-style-invalidation`. A different file poisoning the shared
runtime takes a different set of subtests with it, which moves numerator and denominator together.

⚠⚠⚠ **This retroactively refutes a t1234 finding.** That tick measured a one-line change at **+41** in
this area and banked it as a property of the change; **2240 appears with none of that change in the
binary.** Both readings are draws from one distribution.

> **An area's ERROR BAR is measured by running the SAME binary twice, and until you have it, a delta
> is not a result.** `css/cssom` (2789) and `dom` (8142) were exact across three runs the same hour —
> so this is a property of *this area*, not of the harness in general, and the discriminator is the
> ACCUM count printed on every run. **Read it before believing a `css/css-values` delta.**

Practical rule: for this area, treat anything under ~±72 as unmeasured, take the ACCUM count as the
confounder, and re-run before banking. Fixing the underlying cross-file runtime-reuse UAF would
remove the noise at its source and is tracked as a Bar 0.

## The capability ledger's parts did not sum to its whole, for 247 ticks (t1265)

`scripts/phase0-progress.sh` buckets every row of `CONSTELLATION.tsv` on its status column into
exactly five names, and divides by **every row**:

```text
  caps=584   gated=342  works=17  partial=41  missing=143  unknown=38
                                                            SUM=581   UNACCOUNTED=3
```

Three rows carried `unmeasurable`, which is in no bucket — so each counted **0 in the numerator and 1
in the denominator**, appeared in no column of the per-class table, and `PHASE0-PROGRESS.tsv` printed
a three-capability discrepancy on every landed tick since t1018.

### The status column answers ONE question, and the row was answering two

`hyphens: auto`, `scroll-behavior: smooth`, `scroll-margin`/`scroll-padding` are **`missing`** —
`engine/css` parses none of them, which is what `missing` asserts. What is *unmeasurable* is **the
oracle's ability to price them** (each row carries a probe showing Chrome's own boxes are
byte-identical with and without the property). That is a **ranking** fact, it already lives in the
receipt column, and putting it in the status column is what pushed the row out of the vocabulary.

```text
  after:  caps=584  SUM=584  UNACCOUNTED=0
          readiness 65%  ·  gate-locked 59%  ·  measured 93%     <- IDENTICAL before and after
```

⚠ **The unchanged percentage is the receipt.** `unmeasurable` already scored 0 and `missing` scores 0;
a correction that *moved* readiness would have meant three capabilities were quietly re-graded rather
than filed.

### Two fixes were available and one of them was a disguise

Widening the gate's vocabulary to six values would have made the gate green — and the tally script
(observer-owned) still buckets on five, so the arithmetic would still not see those rows. That
**converts a loud RED into a silent hole**, which is the shape of the bug, not its cure.

### And no "the buckets must sum" assertion was added

It would be **vacuous by construction**: the gate already asserts every status is one of five, and
given that, the buckets sum to the row count necessarily. A second assertion that cannot fail while
the first passes is what `G_POOL_ISOLATION` was retired for.

> **The gate worked. It went RED and stayed RED, and nothing ran it.** `verify.sh` runs 19 of 104
> gates. A gate outside the wall is a claim nobody is checking — and the failure mode is not a
> missing assertion, it is an unread one.

## ~1,900 WPT subtests were passing on `"" === ""`, and publishing one property revealed it (t1270)

**The primary metric structurally rewards NOT implementing a CSSOM property.** This is not a claim
about grid; grid is only where it was caught.

`css/support/interpolation-testcommon.js` backs the **194 `*-interpolation.html` files across twelve
CSS areas**. Its comparison is two reads of the same property (lines 459 / 473):

```js
  var expectedValue = getComputedStyle(expectedTargetContainer.target).getPropertyValue(property);
  ...
  comparisonFunction(getComputedStyle(target).getPropertyValue(property), expectedValue);
```

`getPropertyValue` answers `""` for a property the computed-style object does not carry — **on the
animated target and on the reference target alike**. So for any unpublished property the assertion is
`assert_equals("", "")` and **every subtest in the file passes**, at every progress point, for an
engine with no interpolation whatsoever. t1016's *two errors cancel and read as agreement*, at the
scale of a whole harness.

**Measured, on an OLD/NEW binary pair built in the same hour on the same box.** t1270 published
`grid-template-columns` / `grid-template-rows` — two properties whose resolved value is the USED one,
recovered from taffy's `set_detailed_grid_info`. Same command, `css/css-grid --batch 10`:

```text
  css/css-grid              OLD  7707/14687        NEW  6276/14629        net -1431
    grid-definition          462/1284  ->   802/1284     +340   ← real: used tracks now readable
    layout-algorithm          87/528   ->   217/528      +130   ← real
    parsing                  967/1598  ->  1037/1598      +70   ← real
    animation               1952/2030  ->   822/2092    -1130   ← was VACUOUS
    grid-lanes/animation    1331/1571  ->   544/1634     -787   ← was VACUOUS
  HANG/CRASH 0 both. Every other subdirectory byte-identical.
  CONTROLS: css/css-flexbox 1700/4693 in BOTH (identical). css/css-sizing 2330 -> 2352 (rate
  39.75% -> 39.92%, inside its known band). css/css-values 3075/8282 -> 3124/8133.
```

⚠⚠⚠ **The two loss rows were diffed by FAILING TITLE, not inferred from the totals.** Of 784 distinct
failing titles under the new binary in `css/css-grid/animation`, **760 are new and every one of them
names `grid-template-columns` or `grid-template-rows`** — 380 each, an exact split. The remaining 24
are the pre-existing `grid-auto-*` failures the old binary already had.

⚠⚠ **A CORRECTION, NOT A REGRESSION — and the project has already banked the same shape in the other
direction** (t1176: the WPT checkout was missing `css/support/`, the number that moved was the
instrument, *"a CORRECTION, not progress → RE-RANK"*). Nothing that worked works less well: no page
renders differently and no API returns a worse answer. What fell is a count of assertions that were
never looking at the engine. The +585 real passes and the −1917 vacuous ones are **the same one-line
change** and cannot be separated, so no advance is claimed either.

⚠ **The dodge was available, measured, and refused.** `interpolation-testcommon.js` builds plain
`<div>` targets, so restricting publication to grid *containers* would have kept all ~1,900 vacuous
passes **and** the +585. It is refused because it is score-tuning wearing a spec argument: on a
non-grid element the resolved value IS the computed value, the cascade holds it, and withholding a
value we have is the FALSE ABSENCE the reliability doctrine ranks beside a false presence — t1267
priced one of those at 909 subtests.

**What this predicts, and it is the actionable half.** Every future CSSOM publication tick pays this
toll, and the toll's size is set by how many interpolation subtests the property owns, *not* by how
right the published value is. The reason it bites at all is one sentence in `animatable_js.rs`: the
Web Animations shim **fast-forwards to the end state rather than tweening**. Two resolutions, for the
observer to rank: (1) land real interpolation — the shim is the shared blocker for all twelve areas,
not a grid problem; or (2) treat an interpolation leg as unscored until the property's own
`isSupported` leg is real, so the metric stops paying for absence.

## A gate that PRINTS ON SUCCESS is writing into another gate's input (t1328)

`verify.sh` runs the whole `manuk-shell` crate as ONE `cargo test -- --nocapture` and then parses
that stdout **twice**: `G3 · affordance` greps for `test result: ok. N passed`, and `G_INTERACT`
greps for `^  (open|switch|close)`. libtest runs tests in **parallel threads onto one unsynchronised
fd**, so anything a second test prints interleaves with the first. Captured from a real wall run:

```text
    ....................  reap: worst focus 0.074ms (bar 2ms) · 30 pages queued …
    .  open    median   0.044ms   worst   0.081ms   (one frame = 16ms)
```

⭐ `.  open` does not match `^  open`, and a write landing **mid-line** splits
`test result: ok. 76 passed` into two pieces that match neither grep. **Both gates then fail while
the suite is green** — and `T · crate tests`, which re-runs the same crate on its own, reports
`manuk-shell: ok. 76 passed` in the very same wall.

### Why it read as a mystery for eleven wall runs

That signature is *"deterministic inside verify, unreproducible outside it"*, which is exactly how
t1317 wrote it down after five failed walls — and it is true, because outside verify nothing parses
the stdout. It is not flakiness: it is a **race whose loser is decided by thread scheduling**, so a
re-run clears it often enough to look like noise and never often enough to land reliably. Four more
wall runs went to it at t1327.

⚠ And it was self-inflicted: `G_TAB_REAP` (t1318) printed one line on success. Before that,
`G_INTERACT` was the only printer in the crate.

### The fix, and the two halves it needs

1. **`G_TAB_REAP` no longer prints.** Its numbers were already in its assertion messages, which is
   where a number is actually read.
2. **`G_INTERACT` emits a leading `\n`.** libtest's own progress dots share the fd, so even a single
   printer has its FIRST line appended to a run of dots — `.......  open …`. Only the second and
   third lines were carrying that gate. One newline puts all three at the start of their own line.

### The gate on the gate

`G_ONE_PRINTING_GATE` scans this file's test region and allows **two** print sites, both
`G_INTERACT`'s. ⚠ Two details, each of which the red proof found rather than the reasoning:

- **it must exclude its own text**, or it counts the macros quoted in its own assertion message (the
  first draft reported seven sites, four of them its own — *a scanner that reads its own source
  measures the ruler*);
- **the budget must be exact.** The first draft allowed three, on the reasoning that `println!(`
  also contains `print!(`. It does not — `print` + `ln!(` — so the budget carried a spare slot, and
  the red-proof patch spent it and passed. **A gate with slack it cannot justify is a gate with a
  hole**, and only running the red proof showed it.

## `grep` in this shell SKIPS any file that is not valid UTF-8, and reports it as no matches (t1331)

⚠⚠⚠ A published finding was wrong because of this, so it is written down as a mechanism rather than a
note.

`grep` here is a **shell function** that routes to `ugrep` with `-I` (*ignore binary files*). A file
that is not valid UTF-8 — `Content-Type: text/html; charset=iso-8859-2`, Shift-JIS, Windows-1251,
GB2312 — is classified as **binary and skipped entirely**: zero matches, exit 1, no warning. That is
byte-for-byte indistinguishable from *"the string is not in the file."*

```text
    grep -c "bottom-html" /tmp/cs.html            0   (exit 1)     ← the shell function → ugrep -I
    command grep -c "bottom-html" /tmp/cs.html    1   (exit 0)     ← the real binary
```

⭐ **For a browser project this is a false-ABSENCE generator aimed squarely at its own corpus.** t1327
and t1328 both recorded that six containers on `www.crazyshop.pl` are script-created and in *"none of
the served HTML"*. `div.bottom-html` is in the served HTML, at offset 159,265, plainly. The grep that
"proved" otherwise had silently skipped a 167KB ISO-8859-2 file.

**The rule: any grep over FETCHED or CORPUS content uses `command grep`, or python.** Repository
source is ASCII/UTF-8 and unaffected — the trap is exclusively on the real-world bytes this project
exists to read, which is the worst possible place for it.

⚠ And the general form, which this project has now collected four times: *an instrument that answers
"absent" without distinguishing "I did not look" is not an instrument.* WPT's `SHORT` vs `CRASH`, the
crate suite's INSTRUMENT-vs-RED split, `_out`'s BUILD-FAILED branch, and now this.

## CI WAS RED FOR SEVEN TICKS BEHIND A GREEN SIBLING, AND THE GATE WAS ONE APT PACKAGE FROM BEING MEASURED (t1351)

Every push runs two workflows. `release` was **green** and `CI` was **red**, from the moment t1343
landed its CJK line-box gate until t1350 — seven consecutive commits, none of which read it.

The failure was not a regression. It was the gate's own **PRECONDITION**:

```text
  PRECONDITION: this gate needs both a Latin primary (DejaVu Sans) and the CJK fallback face
  (Noto Sans CJK JP) installed — without two DIFFERENT faces every row below is the same number
  and the test cannot fail. Install fonts-noto-cjk + fonts-dejavu.
```

The gate is right to refuse: a line box is a property of the **FACE**, so proving *"a CJK line takes
the fallback face's line box, not the primary's"* needs two genuinely different faces on the host or
every row it compares is the same number. On a bare GitHub runner neither font exists.

### ⭐⭐⭐ A PRECONDITION PANIC IS INDISTINGUISHABLE FROM AN ENGINE FAILURE, AND IT STOPS THE WHOLE STEP

`cargo test` exits non-zero and the step ends. CI runs **the wall's crate list in order** —
`manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell` — and `manuk-layout`
is **second**. So for seven ticks CI measured **two crates of seven**, and the five it never reached
were reported as neither pass nor fail. *An unmeasured gate is not a passing one*, and here the loop
did not even know which ones were unmeasured.

**The fix is to satisfy the precondition, not to soften the gate.** `fonts-dejavu-core` +
`fonts-noto-cjk` in the CI apt step — the same step that already grew `libasound2-dev` for exactly
this class of reason (`cpal` → `alsa-sys` needed headers, and its absence turned the badge red too).

⚠ **THE ENVIRONMENT IS PART OF THE GATE.** A text gate has a FONT dependency as surely as a build has
a header dependency, and the local box happening to have the fonts is what let it ship untested. Any
gate whose correctness depends on installed data must have that data provisioned where it runs.
