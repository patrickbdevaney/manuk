# M1/M2 plateau-breaker — research-backed implementation plan

> Authored by the observer (2026-08-05, sweep t974) from a 9-agent deep-research workflow that read this
> repo's own tree (oracle.rs, DEATH-TAIL.tsv, the burndown) plus the closest living analogs (Ladybird,
> Servo). It is a PLAN + two already-landed read-only instruments. It changes no engine behavior.
> Division of labor is strict: **observer** owns `scripts/` + this doc; **grind** owns `engine/` + `tests/wpt/`.

## The diagnosis (why M1 stuck 15–18% for ~65 ticks)

The loop has been ranking work by **tag MASS** (CLUSTERS.md hit-count). Tag mass lifts the shape *band*
and crosses **zero sites** — which is exactly the DEATH-TAIL signature: band rises, crossings ±1, for ~65
ticks. Two things actually gate M1, and neither is "a missing Chromium algorithm":

1. **Scorability ceiling (~82%).** 24 of 135 in-scope sites never yield a scored tree, so M1 is
   arithmetically capped at 82% no matter how good layout gets. This is the binding constraint.
2. **The bar is a conjunction** (shape≥0.75 **AND** jarring-clean). A correct fix that moves one conjunct
   on an already-scored site crosses nothing.

Empirically for THIS corpus, M1 grew 2.3%→~16% almost entirely by converting **unscored→scored** (scored
81→114), not by fidelity polish. Function-first is already owner-locked and already validated. So the
fastest path is: **drain throw-killers to lift the ceiling, then nudge the near-bar cohort — ranked by
measurement, not by tag frequency.**

## Ranked levers (deduped across the 7 research axes; grounded in our cluster ledger)

| # | Lever | Type | Expected M1 effect |
|---|---|---|---|
| 1 | **Interface-object completeness + "stub-never-throw / match-absence-exactly"** — register missing DOM/BOM globals with the FULL prototype chain so `instanceof`/`extends` resolve; one absent constructor `ReferenceError`s a whole SPA boot (welt.de → 0%). | fix-subsystem (engine/js) | Largest boot-throw bucket → scorability |
| 2 | **Container-width shrink-to-fit / min-max-content intrinsic sizing** — a wrong used inline-size re-wraps text, changing box height and displacing every sibling (width→dy launder) AND surfacing overflow+overlap. Fixes BOTH conjuncts on the same site. | fix-subsystem (engine/layout ~1985-2017) | The ~4–6 near-bar cohort + band lift on ~73 pure-position sites |
| 3 | **Mechanism-attribution: stop discarding the divergence signature** — the oracle already computes `displaced\|mis-sized:axis~mag(tag)` at oracle.rs:344-377 and `run_oracle_merge` collapses it to `geometry:<tag>` before the loop reads it. Bank it as appended SWEEP columns 11+. | change-method (grind: tests/wpt) | 0 direct; ENDS the wrong-axis churn — precondition for evidence-driven ranks 1–2 |
| 4 | **Async-boot correctness** — HTML event-loop "update-the-rendering" + on-demand synchronous reflow (measure→mutate→measure) + boot-convergence budget. Converts shell-only/thin/timeout sites. | fix-subsystem (engine/js+page) | The shell/thin/timeout buckets → scorability |
| 5 | **IndexedDB indexes + cursors** (Firestore/Firebase-Auth/Amplify open IDB at boot and `createIndex()`/`openCursor()` immediately; a throwing IDBIndex aborts the data layer). | fix-subsystem (engine, or adopt redb/sled backing) | The Firebase-backed unscored slice |
| 6 | **Observer trio must FIRE not merely exist** (IntersectionObserver first — lazy content is invisible to a static post-load frame until it fires). | fix-subsystem | The thin bucket |
| 7 | **Text pipeline seam** — swap swash→harfrust shaper (low-risk on-ramp), then Parley to fuse shape+measure+break+bidi in one pass (kills the two-pass width mismatch = the rank-2 launder at pipeline level). | adopt-crate (harfrust already vendored) | Broad shape lift, staged behind ratchet |
| 8 | **Font robustness** — font-family reaching form controls + variable-font axes + WOFF2 + fontique fallback. One metrics fix moves shape across many nodes. | fix-subsystem + fontique | Multi-site shape lift |
| 9–14 | zoom/appearance scalers · async DNS + bounded parallel fetch (the ~8 timeouts) · automated HTML/CSS reducer (cvise/shrinkray on the oracle predicate) · overflow/sticky containment · truthful modern UA + `-webkit-` aliasing (no stealth) · Interop layout-WPT subset as an own-code ratchet feed. | mixed | Secondary / tooling |

**Explicitly rejected:** Servo `layout_2020` (not embeddable, no incremental ratchet path — its benefit IS
Parley+Taffy), webrender (GPU compositor, wrong metric for a placement gate), selectors/cssparser/Servo
style (already inside Stylo). No lever forks/transcribes Blink/Gecko/WebKit; harfrust/Parley/fontique/redb
are permitted maintained-crate linkage.

Key sources: Ladybird 2026-04 newsletter (per-site wins; DNS off the event loop) · Servo 2026-06 "real
world compat" (variable-font readability on Zulip/Speedtest) · Servo #19242 "conform to the event-loop
processing model" · Interop / wpt.fyi / Chrome Use Counters.

## The locked sequence (do these in order)

1. **[grind]** Bank the mechanism key onto the SWEEP row (rank 3). Zero engine-behavior change → zero
   regression. Falsify with a known style-only-height fixture asserting `mis-sized:height`. Append at
   cols 11+; **cols 1-10 byte-identical** (fidelity-progress.sh / death-tail.sh / phase0-milestones.sh
   read by index).
2. **[grind]** Drain the top throw-killer bucket the signature names — start with interface-object
   completeness (rank 1). Each gate a RED boot-throw fixture that asserts the **expected value**, never
   just presence (the wrong-answer-of-the-right-type trap). Fully-absent OR complete-shaped, never half.
3. **[grind]** Fix the container-width shrink-to-fit primitive (rank 2). Prove the crossing on **all**
   near-bar sites via `scripts/old-binary-control.sh` (same-hour, same-snapshot) before claiming it — a
   single +0.02 is below the ~3.7pt noise floor.
4. **[grind]** One throw-class per tick thereafter (rank 4/5/6), each gated by an **in-page** probe
   (a schedule/order bug is invisible to any final-frame diff).
5. **[grind]** Build the BiDi function A/B cert (bidi/src/protocol.rs) **only** once scorability ≥85%.
   Until then the throw-class proxy riding the M1 sweeps is the sufficient, cheaper M2 signal.

## Loop/harness changes — status

| Change | Owner | Status |
|---|---|---|
| `scripts/near-bar.sh` — marginal-crossing ranker (blockers-to-cross, one-fix-first) | observer | **LANDED** |
| `scripts/throw-killer.sh` — scorability worklist, largest bucket first, lists sites | observer | **LANDED** |
| Rank every render/scorability tick from throw-killer.sh then near-bar.sh, NOT tag-frequency | grind steer | **directive** |
| Mechanism key appended to SWEEP cols 11+ (rank 3) | grind | proposed (step 1) |
| `scripts/old-binary-control.sh` — per-fix crossing attribution (grind invokes; observer never sweeps) | observer | proposed |
| `verify.sh` auto-wall the `g_m1_*.rs` prefix + `gate-coverage-check.sh` (gates-not-in-the-wall) | observer | proposed — **hold** (live-wall surgery; land on a quiet box, not mid-grind) |
| `phase0-milestones.sh` BiDi trigger at scorability ≥85% | observer | proposed |
| Wire near-bar.sh/throw-killer.sh into `orient.sh` so the grind sees them each tick | observer | proposed — **hold** (live tick-path edit) |

The two landed instruments are new, read-only, `exit 0` — they cannot brick a tick. The "hold" items edit
the live wall/tick path while the grind is looping; land them deliberately on a quiet box, never as a
side-effect of this plan.

## Current worklist (measured, sweep t974 — re-run the scripts each sweep)

**THROW-KILLERS (24 in-scope unscored — drain largest bucket first):**
- timeout (8): ebay, bbs.ruliweb, coinmarketcap, friulioggi, swiftspinus, secure.paymentech, morikoshi, 7info.ru
- shell-only (8): comix.to, awlyaa.education.dz, esaj.tjsp.jus.br, house.udn.com, forums.moneysavingexpert, vk.com, allticketscol, d2rwkn96…cloudfront
- tree-divergence (5): dashboard.twitch.tv, app.ordertime.com, villaggioposeidone, experiencia.pichincha, mayatoys.in
- thin (2): redemoura.gupy.io, booking.directferries.com · render-fail (1): webfenix.movilidadbogota.gov.co

**NEAR-BAR one-fix-from-M1 (14 — a single shape nudge crosses each):** pasarbokep (+0.119), agoda
(+0.153), profissionaliza.cademi (+0.162), cyoinatu-onna (+0.170), momon-ga (+0.185), livescore.cz,
kroftools, hnhbkis.edu.in, crm.majoo.id, funinjeet, ioi-russia.vdi.mipt, monopolybingogame, cbse.gov.in,
hope.cap-systems. **Two-fix tier** incl. jatekshop.eu (shape 0.740, +0.010 — one reading-order flag away).

## Observer finding — possible M1 inflation (verify before trusting the headline)

near-bar counts **20** genuinely-rendered M1 passes; the official gate counts ~21–23 because it includes
1-node/6-node shells that trivially pass shape≥0.75 ∧ jarring-clean (app.ordertime.com: 1 node shape
1.000; allticketscol.com: 1 node shape 1.000; awlyaa: 6 nodes shape 0.833) — the "100%-of-nothing" trap,
and the SAME sites appear on the throw-killer list. A **min-node-count (or min-coverage) floor on M1
eligibility** would de-inflate the headline. Flagged for the grind to verify — not an observer-side gate
change.
