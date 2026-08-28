# THE FIDELITY INSTRUMENT — what it can and cannot see

## `shell-only-N` is the oracle rendering a DIFFERENT DOCUMENT than the site serves (t856, mechanism corrected t858)

`shell-only-N` says **the ORACLE built fewer than `CERT_MIN_SHAPE_SAMPLE` elements** — it is a claim
about Chrome's count, never ours. The board's #1 (the scorability ceiling) names it as part of
*"29 of 130 in-scope sites do not render at all"*, so what it actually measures decides whether those
are engine ticks or nothing at all.

**The oracle's document is one `curl` of the URL** (`chrome::fetch_document`), rendered by Chrome from
`file://` with a `<base href>` injected (see the t858 correction below). Measured across the whole 12-site
`shell-only` cohort — raw tag count in the curl'd body, Chrome on that file (the oracle's own path),
and Chrome on the LIVE url:

```text
  site                            reason         raw     onfile   live
  awlyaa.education.dz             shell-only-6      9        9       9    <- honest: a 9-tag page
  esaj.tjsp.jus.br                shell-only-4     30       30     300
  house.udn.com                   shell-only-1      5        5     949
  d2rwkn96gppqo1.cloudfront.net   shell-only-9    172      174     554
  forums.moneysavingexpert.com    shell-only-9   2583     2345      48    <- INVERTED, see below
  experiencia.pichincha.com       shell-only-8     53       53     239
  vk.com                          shell-only-5      0      215     347
  merchant.upi9.pro               shell-only-2     20       20      68
  booking.directferries.com       shell-only-3     58       59      87
  allticketscol.com               shell-only-1     34       36    1115
  pt88.app                        shell-only-3     39       40     435
  portal.ensuretyfinance.com      shell-only-1     23       23      48
```

All twelve answer **HTTP 200** — none is bot-walled at the fetch. On **ten of twelve** the site
renders perfectly well (live 48–1115 tags) and the oracle physically cannot see it.

⚠⚠⚠ **CORRECTED AT t858 — THE MECHANISM STATED HERE AT t856 WAS WRONG, AND THE CONCLUSION SURVIVES
IT.** t856 wrote *"the oracle renders from `file://`, so every relative bundle resolves to
`file:///tmp/…` and 404s"*. **It does not.** `chrome::capture_seen_all_paths` — the very function
that produces the `probed` count this reason is computed from — inserts `<base href="{url}">` into
the document before handing it to Chrome (`chrome.rs:520`). Relative subresources therefore resolve
against the real origin. Re-measured with the tag inserted exactly as the oracle inserts it:

```text
                     t856 (no base tag)      corrected (base, as the oracle does it)
  allticketscol.com   36 tags                 38 tags · sheets 10 · font Lato   <- CSS LOADS
  trivago.be          —                     1622 tags · sheets  0 · Times New Roman
  house.udn.com        5 tags                  8 tags · sheets  0 · Times New Roman
```

`allticketscol` alone falsifies it: relative stylesheets resolve and load. **t856 measured a plain
`file://` copy without the base tag — a different pipeline from the one whose numbers it was
explaining.** Right answer, wrong reason, which is the failure mode that survives longest because the
conclusion keeps checking out.

⚠⚠⚠ **THE DEMONSTRATED MECHANISM FOR THE WORST ROW IS `document.URL`.** `house.udn.com`'s entire
document is a five-tag stub:

```html
  <script language="javascript">
  if (document.URL.indexOf("house.udn.com") != -1) { window.location.href = "/house/index"; }
  </script>
```

The oracle's `document.URL` is `file:///tmp/manuk-shape-….html`, so `indexOf` is **-1 and the redirect
never fires.** `<base href>` cannot help — it changes URL *resolution*, not `document.URL`. This is a
CLASS, not one site: anything branching on `location.hostname`, `document.URL` or `location.protocol`
takes the wrong branch for the oracle.

⚠⚠ **THE CONCLUSION IS UNCHANGED AND BETTER SUPPORTED.** Even with the base tag the oracle builds
**38 tags for allticketscol against 1115 live** and **8 for house.udn.com against 949**, while our own
rows read `coverage 1.000000` against a one-element reference. The cohort is an instrument bound, not
engine work.

⚠ **WITHDRAWN, not caveated:** t858 also counted that 36 of 101 scored sites carry at least one
relative stylesheet `href` and nearly published that as *"36% are compared against an unstyled
Chrome"*. With a `<base href>` present a relative href is not by itself fatal, so that count measures
href SHAPE, not CSS delivery; the probe that would have settled it returned no reporter on all 20
sample sites, which is a probe failure and not a result. The one established instance is `trivago.be`:
five `<link rel=stylesheet>`, **zero** loaded by the oracle. One site, named — not a rate.

⚠⚠ **ONE ROW REFUTES THE SIMPLE STORY, AND IT IS THE MOST INFORMATIVE ONE.**
`forums.moneysavingexpert.com` is inverted: curl gets **2583** tags, Chrome-on-file builds **2345** —
the oracle had a full document — yet the sweep recorded `shell-only-9`. So its `probed 9` is a
**probe-side** gap, not a fetch-side one, and it needs its own diagnosis rather than this
explanation. Its live reading (48) also shows headless Chrome being walled on the live URL while
curl is not, i.e. **the two channels disagree in both directions** and neither is authoritative on
its own. ⚠ Note the units: `raw`/`onfile`/`live` are **tag counts**, while `probed` is the probe's
box-bearing, path-keyed element count — they are not the same quantity, which is exactly why the
inverted row cannot be read as "the fetch was fine, therefore the probe is broken" without the
follow-up.

⚠ **AND ONE ROW IS HONEST**: `awlyaa.education.dz` is 9 tags in every channel. `shell-only-6` is a
true statement about that site, which is what stops this finding from being *"the reason is always
wrong"*.

### The ranked fix — RETRACTED at t858, because it is already there

t856 proposed injecting `<base href="ORIGINAL_URL">` as the fix. **`chrome.rs:520` has done it all
along**; proposing it was the visible symptom of having reconstructed the pipeline from a comment
instead of reading it. What remains open is harder and is not a one-liner: the oracle's document
answers `file:///tmp/…` to `document.URL`, so origin-conditional boot code cannot run correctly for
it, and no amount of URL rewriting changes that. Serving the snapshot from a loopback origin **at the
site's own hostname** would, and that is a real design question for whoever owns the instrument's
contract — not a tick.

⚠ **Until it lands, do not spend throw-killer ticks on this cohort.** Ten of the twelve have no
engine defect visible in this data at all — the pages render in Chrome and our own score is
`coverage 1.000000` against a one-element reference. Working them would be optimising against an
artefact, which is the failure this loop has now been caught by four times.

## `css-starved-N` blamed our load deadline for the AUTHOR's dead link (t860)

`Page::failed_stylesheet_fetches()` answers exactly one question — *"is this layout (partly)
UA-default fallback rather than our rendering of the author's design?"* — and the fidelity instrument
turns a non-zero answer into `css-starved-N`, which makes the site **UNSCORABLE and books it against
us**. The reason string it printed ended:

> *"OURS and IN-SCOPE: the sheets were cut by our own load deadline, **not refused by the origin**"*

That is a testable sentence and it was **false on 3 of 3** sites carrying the tag. One `curl` each:

```text
  www.cuneocronaca.it   css/normalize.css  ·  css/simple_slider.css   404   (its other 4 sheets: 200)
  m.youm7.com           landing/landingstyle.css                      404
  nortenoticia.com.br   cdnjs …/tailwindcss/3.4.3/tailwind.min.css    404
```

**A 404 answers the question NO.** The `<link>` is dead in the author's own HTML; the reference
browser requests the same URL, gets the same 404, and renders the same page without it. We are not
*less*-styled than the reference — we are *identically* styled, which is precisely the state a
differential measurement exists to score.

**The mechanism is a status that was read and then discarded.** `subresource_text` checks
`status >= 400` and returns `None`, so a sheet the origin does not have arrives at the same `None` arm
as a sheet that died on the wire, and that arm books both into `failed_css`.

**Where the line is drawn, and why not wider.** Only `404`/`410`. A `403` on a stylesheet is very
often a bot-wall answering *us* differently than it answers Chrome — a divergence we own. A `5xx` is
weather that may well have served Chrome fine. *"The resource does not exist"* is the one status that
is the same answer for every client. The gate asserts the `5xx` half beside the `404` half, so the fix
cannot drift into "stop counting failures".

### ⚠⚠ The exemption needed a SECOND half, and the first half looked complete

With the 404 arm alone, two of the three sites scored and `cuneocronaca` did not — with the exemption
firing correctly. A 404 sheet never enters `external_css`, so the *"don't re-fetch what we already
have"* filter **re-requested it on every re-entry** after a round of dynamic scripts; on a late
re-entry the load deadline cuts it before it can settle, and the deadline's `cut` block books it as
"never settled". The exemption fired on pass 1 and was overwritten on pass 2.

> **An exemption keyed on the OUTCOME of a fetch is undone by anything that stops the fetch from
> having an outcome.**

Closed with `absent_css`: a 404/410 is a **settled** answer, so Part 22.3's *"no URL on the wire twice
for one navigation"* applies to it exactly as it applies to a hit. That also removes one redundant
fetch per re-entry per dead sheet.

**Result.** All three score. `m.youm7.com` comes out `cov 1.000 · shape 0.870` with all four jarring
invariants clean — **an M1 PASS**, on a site that had been counted against us as one we could not
style. Controls (wikipedia, blog.rust-lang.org, possssno.sbs) byte-identical on every term.

**Gate.** `a_stylesheet_the_origin_does_not_have_is_not_counted_as_unstyled` — a local one-shot origin
serving `404` (must not count) and `503` (must still count), so the STATUS is the only variable.
RED-proven both ways: drop the `absent` arm and the first assertion reads 1; widen it past 404/410 and
the second reads 0. `G_SILENT_FAIL` and `a_stylesheet_the_deadline_cut_off_is_counted_as_failed` are
untouched — their fixtures are a refused connection and a cancelled fetch, neither of which is a 404.

⚠ **This is the second `unscorable-and-ours` cohort in five ticks to shrink under measurement** (see
the `shell-only` section above). The printed `SCORABILITY CEILING` is a floor on the *instrument's*
fidelity, not a ceiling on the engine — and `render-failed`, `timeout` and `tree-divergence` have not
been checked this way yet.

## Parallelising the sweep is not free — and here is the control that prices it (tick 867)

The board's #1 throughput lever asks for the fidelity sweep to run in parallel chunks: *"it runs
SERIAL now (~2h/200 sites) … target ~15-20min. This alone nearly HALVES the cycle."* The speedup is
real (≈2h → ≈35min at `-P 6`). The cost had never been measured.

**The control**: 12 SCORED sites spanning the whole shape range (0.00 → 1.00), measured inside a
`-P 6` chunked sweep and then again **one at a time**, on the same binary, minutes apart.

```text
  byte-identical                       7 / 12
  MOVED                                5 / 12
    www.trivago.be        cov 0.9144 -> 0.9564
    probidas.lt           cov 0.6378 -> 0.2682     <- 0.37 of coverage, harness alone
    www.repubblica.it     cov 0.9939 -> 0.9673 · shape 0.4468 -> 0.4249
    www.puentedemando.com shape 0.7775 -> 0.7792
    seduniaselat.com      shape 0.7415 -> 0.7500   <- CROSSES THE 0.75 M1 BAR
```

**One site in twelve crosses the M1 bar on the scheduling alone.** In the same sweep the headline
moved by +2 M1 sites — so the artefact is the same order of magnitude as the signal it would be read
as. That is the "every number has a harness" lesson with a price tag attached.

The mechanism is not mysterious: our own load budget is a **wall clock**. Six concurrent renders plus
six concurrent Chromes contend for cores and network, so a page that finished its subresources inside
12s serially may not finish them at 6-way concurrency — and coverage is a direct function of what
arrived before the deadline.

### What survives parallelism and what does not

| reads as | robust under parallelism? |
|---|---|
| a site's **reason string** changing (`render-failed` → SCORED, `shell-only` → `oracle-module-shell`) | **yes** — scheduling does not invent a boot or a module script |
| a site's **shape at three decimal places** | **no** |
| an **aggregate** M1 / scorability delta against a serially-measured sweep | **no** — the series is not differenceable across the harness change |
| a **`crashed`** row | **no**, and worse — see below |

### Parallelism MANUFACTURES Bar-0 rows

Two chunks segfaulted, one of them five times, and the retry loop stopped converging. Every one of
the 27 remaining sites then ran **clean when given its own process**. That is the parked mozjs
heap-corruption bug's known threshold behaviour reproduced from the other side: it needs allocation
churn, a batched sweep supplies the churn, so a batched run produces crash rows a solo run does not.
**A crash count read off a batched sweep is a reading about the batch.** (The reverse of t863, where
a SOLO run produced a deterministic 9/9 crash — that one is real precisely because it was alone.)

### The usable rule

Use parallel sweeps for **scorability and reason-string** questions, where the label is robust. Keep
the banked series **serial** for the burndown slope, or re-baseline it explicitly — and re-measure any
`crashed` row solo before believing it.

[[conformance-and-oracles]] [[performance]]

## Two classifiers, one rule, and the one that ran first could not apply it (t877)

An unscoreable row's reason is decided in two places that do not know the same things:

```rust
  // fidelity.rs, inside `compare()` — the PIXEL classifier, from the two screenshots:
  unmeasurable: (ink(&ma) < BLANK_INK && ink(&mb) >= ORACLE_MIN_INK).then_some(RenderFailed)

  // main.rs — the DOM classifier, which KNOWS about module shells:
  if f.unmeasurable.is_none() { … unscoreable_reason(probed, common, ours, ships_modules) … }
```

The pixel one runs first and used to be final, on the rule that *"the earlier cause is the true
one"*. That rule is right about a bot-wall — a property of the origin, which nothing downstream
knows better — and **wrong here**, because `document_ships_module_scripts` is consulted by the DOM
classifier and has never been available to the pixel one.

`RenderFailed` is the single reason the instrument describes as *"our own bug … the one that most
deserves to count against the score"*, so a mislabel here aims ticks at an engine defect that does
not exist.

### What it was mislabelling

On a `type="module"` SPA the oracle's ink can be the app's own **splash screen** — the shell that
paints while the bundles load. Since a module script is always CORS-fetched and the oracle renders a
fetched copy from a foreign origin, those bundles never load *for the reference either*. So the very
shell that proves the page cannot be scored from a snapshot was the evidence used to blame us:

```text
  webfenix.movilidadbogota.gov.co     Chrome LIVE 22 <div>  ·  Chrome SNAPSHOT 4  ·  ours 4
```

`unscoreable_reason` now arbitrates the pixel verdict instead of deferring to it. **Both** of the
t875 sweep's `render-failed` rows relabel:

```text
  webfenix.movilidadbogota.gov.co   render-failed → oracle-module-shell-8
  d2rwkn96gppqo1.cloudfront.net     render-failed → oracle-module-shell-9
```

### The override is deliberately narrow, and the gate proves the narrowness

It requires **both** that the document ships module scripts **and** that the ORACLE itself came in
under `CERT_MIN_SHAPE_SAMPLE`. A blank paint on a module page whose oracle *did* build the document
stays `RenderFailed` and stays ours — laundering a real blank render would be worse than the mislabel
this fixes. `a_blank_render_is_ours_unless_the_oracle_only_built_a_module_shell` asserts both
clauses and was RED-proven twice: once by removing the override, and once by **widening** it (dropping
the oracle-built-the-page clause), which is the direction a careless fix would take.

Neither label moves the arithmetic — both are UNSCORED and both count against the bar. This changes
only what the loop is told to go and fix. It is the **fourth** consecutive cohort named as ours and
proven not ours: `shell-only` (t856), `css-starved` (t860), `oracle-timeout` (t861), `render-failed`
(here).

## The one-origin proxy works, and a naive one half-boots the app it was built to rescue (t880)

t865 named the largest unscored cohort's cause and its fix: a `type="module"` script is **always**
CORS-fetched, the oracle renders a fetched copy from a foreign origin, a site does not send
`Access-Control-Allow-Origin` for its own bundle, so the entry bundle never loads and the app never
boots — **for the reference as well as for us**. The named fix was *"a loopback reverse PROXY so
document, bundle and XHR share ONE origin"*. It had never been tested.

It is tested now, with a throwaway proxy, before any of it is built into the instrument. `<div>`
counts from `chromium --dump-dom`:

```text
                                     LIVE    PROXY    SNAPSHOT (what the oracle does today)
  pt88.app                            147      147       0
  booking.directferries.com             8        8       1
  portal.ensuretyfinance.com            8        8       0
  webfenix.movilidadbogota.gov.co      22       22       4
  allticketscol.com                   336     → 38 ←     0
```

**Four of five recover EXACTLY.** The design is right, and the cohort really is a measurement channel
rather than an engine gap.

### The fifth is the whole finding

`allticketscol.com` recovers **38 of 336**, and that is precisely the failure t865 refused to accept
from bundle-inlining: *"a HALF-BUILT reference is worse than an honest shell — it clears the shell
floor and the instrument starts charging Chrome's missing half to us."* A proxy that half-boots does
exactly that, silently, and looks like progress.

The cause is one line of its document: the bundles live on **`static.allticketscol.com`**, a different
host. A single-origin proxy rewrites nothing, so those requests still go straight to a foreign origin
and are CORS-blocked all over again.

### What that means for the build

Proxying "more hosts" is unbounded and cannot be the answer — a page may pull from any number of
CDNs, and chasing them is the bot-wall treadmill in a different costume. The answer is an
**acceptance test**, which turns t865's warning into something measurable:

> **A proxied render is only usable as a reference when it AGREES with the LIVE render.** Compare the
> two, and score the row only if they match; otherwise keep the honest `oracle-module-shell` label and
> the unmoved denominator.

That is cheap (both renders are one `--dump-dom` each), it is falsifiable in both directions, and it
makes the half-boot a *detected* state rather than a silent one. Build the proxy behind it, never in
front of it.

## The one-origin proxy, built (t881) — and what it actually bought

`tests/wpt/src/proxy.rs`. Bind `127.0.0.1:0`; serve the document at **its own path** under that
origin (no `<base>` — the point is that the author's relative URLs resolve as the author wrote them);
proxy every other path upstream through `curl`; put `Access-Control-Allow-Origin: *` on **every**
response. `chrome::capture_seen_all_paths` reaches for it only when the snapshot reference came in
**under the shell floor** *and* the document ships module scripts — most of the modern corpus ships
modules, so gating on that alone would double this crate's Chrome bill on healthy sites for nothing.

**The site's OWN hosts are one origin; a third party is not.** `static.allticketscol.com` is the
document host's registrable domain wearing a deployment convention, and it is re-served under
`/__manuk_origin__/<scheme>/<host>/…` — *under its own name*, because collapsing two hosts onto one
root would fetch `example.com/main.js` when the page asked for `static.example.com/main.js`, and a
proxy that answers the **wrong** upstream produces a subtly wrong reference rather than an obviously
broken one. A genuine third-party CDN is left alone: it either already sends ACAO (which is why it
works from any origin in a real browser) or the acceptance test refuses the row.

### THE INSTRUMENT COUNTED ITSELF, AND THE ACCEPTANCE TEST CAUGHT IT ON THE FIRST RUN

The first acceptance counter read every `<` followed by a letter. Only the **proxied** render carries
the injected probe, so every `i<lim`, `j<toks.length` and `i<str.length` inside `PROBE_ALL_PATHS_JS`
counted as page content the live render had "lost". Measured on `portal.ensuretyfinance.com`: proxy
**55** against live **48**, and the refusal's own tag-delta line named the whole difference —

```text
    only LIVE has:  —
    only PROXY has: pre×2 a×1 lim×1 script×1 str×1 toks×1
```

— *nothing* missing from the proxy, seven artefacts added by it, six of them fragments of the probe's
own JavaScript. That is t780-783 one layer up (*the probe's own sentinel widened its subject*): an
instrument that measures itself reports **its own presence as the site's absence**. The fix is
symmetric rather than a carve-out for our probe — a `<` inside `<script>`/`<style>` is data on both
sides, so the content is skipped on both. The refusal line printing its tag histogram is what made
this a two-minute finding instead of a hypothesis.

### Measured, all five of t880's sites, one binary

```text
                                    LIVE   PROXY   verdict     probed elements   label before → after
  pt88.app                           435     435   ACCEPTED          2 → 206      module-shell → thin-overlap
  booking.directferries.com           85      86   ACCEPTED          1 →  21      module-shell → thin-overlap
  portal.ensuretyfinance.com          48      49   ACCEPTED          0 →  22      module-shell → thin-overlap
  webfenix.movilidadbogota.gov.co     92      93   ACCEPTED          4 →  50      module-shell → render-failed
  allticketscol.com                 1192     168   REFUSED           —            module-shell (unchanged)
```

⚠⚠⚠ **AND THE HONEST HEADLINE IS THAT M1 DID NOT MOVE.** Four rows stopped being unscored *for an
instrument reason* and immediately became unscored *for OUR reason* — `thin-overlap` and
`render-failed` — because once the reference is a real page, it is visible that we render almost none
of it. The denominator is unchanged and no site crossed the bar. What changed is **where the loop is
told to look**: the largest single cohort the board ranked as a measurement defect is now four named
engine gaps and one refused row, and the refused row says exactly what is missing (`app-evento-card
×47` — the app booted and its data fetch did not).

This is the same shape the board's t777 block predicted for throw-killers: *the function leg is a
chain*, and clearing one link exposes the next rather than scoring the site.

## A debug binary is not this browser — 4–5.5× slower, and it understates shape as well (t887)

`scripts/fidelity-sweep.sh:42` pins `BIN=target/release/manuk-wpt` and always has. But **every
`SWEEP-t<N>-rows.tsv` in this repo is produced by invoking `manuk-wpt fidelity --rows-out` directly**,
and that path had no guard. Tick 886 swept all 200 CrUX sites from `target/debug`.

```text
                    DEBUG      RELEASE    Chromium    release vs Chrome
  sip777man.site  191,825ms    34,814ms    12,029ms         2.9×
  beb88run.xyz    172,591ms    32,198ms    11,273ms         2.9×
  www.ikea.com     50,717ms     9,193ms     6,690ms         1.4×
  payb.jp         102,352ms    40,537ms    35,314ms         1.15×
```

**What that did to the corpus reading:**

```text
                                   t875     t886(debug)   t887(release)
  M1                                16.9%      17.4%✗        17.8%
  scored / in-scope                106/130     92/132✗      107/129
  timeout rows                        3        22✗            4
  scorability ceiling                 —        69.7%✗        82.9%
```

t886 concluded *"we are 4–17× slower than Chromium"* and *"the timeout cohort is the largest unscored
reason"*. Both are **withdrawn**: on the shipping binary the gap is **1.15–2.9×**, and `payb.jp` is at
parity. Its old-binary control was sound and its conclusion ("not a code regression") stands — it was
debug-vs-debug, which is exactly the comparison that cannot see this.

### The half that would have stayed hidden

A page that does not settle inside the load budget scores **what it managed**, so a debug build also
**understates fidelity**: `sip777man` 90.5% → 94.2%, `beb88run` 81.6% → 86.8%, `payb.jp` 64.7% →
74.6%. The error makes the browser look *slower and worse at once*, which is the most convincing
possible shape for a wrong number.

### The guard

`fidelity::may_bank_a_sweep(banking, is_debug, override)` — `manuk-wpt fidelity` now **refuses** to
write `--rows-out` from a debug build, with `MANUK_ALLOW_DEBUG_SWEEP=1` as the deliberate override so
the refusal cannot become a reason to delete the check. A debug run *without* `--rows-out` still
measures: per-site diagnosis is how every engine tick here is verified, and blocking it would trade
one defect for a worse one.

**The withdrawn row was removed from `FIDELITY-PROGRESS.tsv`, not averaged in** — a debug sweep is not
a slow measurement of this browser, it is a measurement of a different one, and leaving it in would
make every future Δ diff against it.

### The lesson, narrower than the law it comes from

*Every number has a harness, and the harness is part of the number* — walked into by the tick that was
quoting it. The sharper form: **when a reading is surprising, check WHAT BINARY produced it before
checking what the code did.** t886 spent its entire attribution budget, including a full old-binary
rebuild, on a question whose answer was in `ls -la target/`.

## The M1 crossing ranking, computed from a sweep rather than remembered (t888, from SWEEP-t887)

`M1 = shape ≥ 0.75 AND jarring-clean`, so there are exactly two ways to cross it, and which one is
cheaper is a **measurement**, not a preference. From `SWEEP-t887-rows.tsv` (release binary, all guards
clean):

```text
  COHORT A — over the shape bar, failing ONLY on jarring          8 sites  → +6.2 M1 points
    sip777man.site   0.942   h=1  o=6   r=16
    sestra.cc        0.896   h=8  o=0   r=7
    beb88run.xyz     0.868   h=0  o=14  r=4
    www.unoeste.br   0.860   h=0  o=3   r=2   d=1
    www.tz.de        0.814   h=3  o=2   r=5
    www.otomoto.pl   0.797   h=0  o=3   r=11
    www.freesupertips.com  0.776   h=1  o=1   r=4
    simplepdf.com    0.756   h=5  o=0   r=0    ← the ONLY single-dimension blocker

  COHORT B — jarring-clean, below the shape bar                  12 sites
    nearest 0.588 — a 0.162 gap. No cheap crossing.
```

Cohort A is the lever and it is not close. Blocking dimensions across the eight: `reading_order` 7 ·
`overlap` 6 · `h_overflow` 5 · `dead_target` 1.

**Recompute it every sweep.** The same vein held 13 sites at t868 and 8 at t887 — the membership turns
over, and a ranking obeyed from memory ranks a cohort that has moved.

**Also stale by twenty points: the board's scorability ceiling.** It has ranked "SCORABILITY FIRST"
off a measured 63% since 2026-07-30; on t887 it is **82.9% (107/129)**, with the 22 unscored spread as
`shell-only 9 · other 5 · timeout 4 · render-fail 2 · thin-overlap 2` — no cohort above nine.

### The named mechanism at the top of cohort A

`beb88run.xyz` is missing **458 boxes** (`div×186 img×119 a×93 li×44 ul×14 span×2`) at x-coordinates
of 1303 · 4740 · 5925 · 12991 · 15361 · 17731 · 18358 — one very long horizontal row. Chrome says the
missing node is:

```text
  body>div:nth-of-type(3)   DIV.banner                            [0 146 1185×380]
  …>div:nth-of-type(2)      DIV.banner-carousel slick-initialized [0 146 1185×380]
  …>div:nth-of-type(1)      DIV.slick-list draggable              [0 146 1185×380]  overflow hidden/hidden
```

A **Slick carousel**. `slick-list` clips a `slick-track` laid out as one row of slides; we emit no box
for `slick-list`, so the track escapes its clip and 14 sibling pairs collide. That is an *overlap*
symptom with a *containment* cause — the usual "a reading-order/overlap symptom is a width or a
transform upstream", one variant further out: the clipping container itself. Slick is on a large slice
of the template-built web, so it is a class rather than a site.

**The first bisect for whoever takes it:** separate *"the DOM Slick built differs"* from *"we lay out
the DOM it built differently"* before touching layout — two candidate subsystems that share no code.

## A negative common-set band is not a regression until you split it on COVERAGE

`fidelity-progress.sh` reports the common-set band — the mean Δshape over the sites scored in **both**
sweeps — precisely because a pass-count is noisy at ±2-4 sites. But the band has its own trap, and
t898 walked into it and back out:

```text
  reported:  band -1.71 pts DOWN — "could be engine OR site-drift"
```

**Split the common set on the one variable that separates a regression from a composition change —
whether COVERAGE moved:**

```text
  COMMON SET (122 sites)              shape band  -1.67 pts   ·  scored elements 67241 -> 69032
    COVERAGE-UP cohort (9 sites, cov >= +2 pts)
        mean coverage  +20.22 pts   mean shape  -5.74 pts   elements +1809  (the ENTIRE gain)
    EVERYTHING ELSE (113 sites)                   shape  -1.34 pts
    …minus 5 named outliers (108)                 shape  -0.08 pts     <- FLAT
```

Nine sites gained twenty coverage points and 1,809 elements; their shape fell because the denominator
grew by exactly the hard elements that had been missing. That is *a shape drop is a coverage win*
(t813-818) at corpus scale, and it is how a real capability win shows up as a headline going the wrong
way. The other 108 sites are flat to within 0.08 pt — which is also the strongest available statement
that the fixes were **inert where they should be**, a claim a four-site control panel cannot make.

**The method, as a rule:**

1. Take the common set (sites scored in both sweeps). Never the pass count.
2. Split it: `Δcov >= +2 pts` is the composition cohort. Read `Δshape` and `Δn` there together — a
   large negative Δshape beside a large positive Δn is a WIN, not a loss.
3. Read the band over the remainder. That is the number that can contain a regression.
4. Name the residual outliers individually and run the **OLD-BINARY control** on them. A clean delta
   attributes nothing until the old binary has refused to reproduce it.
5. Discard any row whose `shape_n` is too small to carry a verdict — t898 had `vk.com` at n=2 and
   `mobcup.fm` at n=29 sitting in the down-movers, and neither is a measurement.

⚠ **And read scorability as a SEPARATE lever.** t898 pre-registered *"scorability should rise"* after
five capability fixes and it did not move at all (107/129 both sweeps). The fixes were DOM-correctness
fixes, not boot-throw killers: they moved coverage on sites that already scored. Scorability moves
only when a site that did not render starts rendering. Two levers, two metrics, and reading one
against the other is how a real win reads as nothing.

## Refute with the NEW binary first; the OLD-binary control is for a delta that REPRODUCES

The standing rule is *"a clean delta attributes nothing until the old binary has refused to reproduce
it"* — rebuild the old tree, re-measure now. That is right, and it costs ~10 minutes of LTO per
question. t899 found the cheaper first move:

> **When the NEW binary refuses to reproduce the loss, the delta was never real and there is nothing
> for an old binary to explain.**

Three sites carried the non-composition half of t898's −1.67 pt band. Three solo runs each, on the
current binary, and every one landed back at its pre-fix value — `www.crazyshop.pl` byte-identical to
its t887 row on all six dimensions. No rebuild was needed. Order the two controls by cost:

1. **Solo re-run on the current binary** (~1 min). If the loss vanishes, it was a batch reading.
2. **Old-binary control** (~10 min) — only once the loss REPRODUCES, to say whose it is.

### Run the control in BOTH directions, or it proves nothing

If solo runs simply flattered every site, step 1 would be worthless. So the *composition* cohort gets
the same treatment, and it must REPRODUCE:

```text
  www.taphouse23.com   t898 cov 0.9782 n 1975  ->  SOLO cov 0.9782 n 1975   (exact, 6 places)
  probidas.lt          t898 cov 0.9409 n  701  ->  SOLO cov 0.4001 n  673   (element gain holds)
```

A batch reading that survives a solo re-run is a measurement; one that does not is a harness reading.

### A `--jobs 2` row is bankable for the CORPUS, and is not evidence about ANY SINGLE SITE

The loop treats `--jobs 2` as clean because t771 showed `--jobs 8` costs hard sites their scorability.
That is still true, and it is not the same claim as *"a `--jobs 2` row is trustworthy per site"*.
t899's three artefact rows all fail in the same direction — the batch **under-renders**:

```text
  ubys.bingol.edu.tr   73 of 166 elements rendered      (solo: 166)
  www.crazyshop.pl     reading-order 535 -> 37          (solo: 535 — the page laid out differently)
  www.freesupertips.com  geometry scrambled at a FLAT element count (458 vs 466)
```

The `instrument` column was identical across both sweeps, so it is the run, not the instrument.
**Per-site verdicts — a crossing, a regression, a cohort membership — require a solo re-run.** The
aggregate band is what the batch is for.

### Decompose the band before reading it

t898's method, completed here: the −1.67 pt band accounts for itself exactly.

```text
  5 unreproducible / undersized rows   -1.17 pts   (5 sites)
  composition (coverage-up) cohort     -0.42 pts   (9 sites, +1809 elements)
  everything else                      -0.07 pts   (108 sites)   <- FLAT
                                      ────────
                                       -1.67 pts
```

A band that does not add up is an instrument bug, not a result — the accounting-reconciliation
mechanism, pointed at the loop's own headline.

## A fix gated on its own DISCOVERY STORY reaches only the cohort it was found in (t903)

The one-origin proxy was built at t880/t881 for the `oracle-module-shell` cohort, and its trigger
inherited that cohort's *defining test*:

```rust
  if seen.len() < CERT_MIN_SHAPE_SAMPLE && document_ships_module_scripts(&html)
```

Twenty-three ticks later the sweep still carried **five `shell-only` rows** with the fix built,
wired, green, and never offered to any of them. Not one ships a `type="module"` script — and every
one is 3–10× short from the snapshot regardless. Measured before writing any code, `curl` for the
document plus two `google-chrome-stable --dump-dom` runs each:

```text
                                 ships type=module?   file:// snapshot   LIVE
  esaj.tjsp.jus.br                     NO                     30          300
  house.udn.com                        NO                     99          958
  merchant.upi9.pro                    NO                     20           68
  experiencia.pichincha.com            NO                     53          567
```

### Modules are ONE WAY to the origin wall, not the wall

`merchant.upi9.pro` is Next.js with classic `<script src="/_next/…" defer>`; from a `file://`
snapshot that is `file:///_next/…`. `house.udn.com` is a 195-byte document whose entire body is
`window.location.href="/house/index"`. Neither involves a CORS-mode fetch. **Every root-relative
subresource and every same-document navigation is broken from a snapshot**, and one origin removes
all of them by construction — which is what the proxy was for and what the module test hid.

The cost argument the old trigger made was already satisfied without it. Its own comment said so
while the code did otherwise: *"gating on [modules] alone would double this crate's Chrome bill …
gating on **the reference came in under the shell floor** confines the extra work to the ~11 rows
that are unscored today."* A healthy reference is over the floor and pays nothing. The module
conjunct bought no budget and cost four rows.

> **When a fix is generalised from one diagnosed cohort, check whether the trigger tests the CAUSE
> you diagnosed or the CONDITION you repair.** A trigger written from the discovery story reaches
> exactly the rows that produced the story.

### The acceptance test is what makes widening safe — and it refused half the cohort

`proxy::renders_agree` requires a proxied render to AGREE with the live one, so a wider trigger can
only convert rows that test has already vouched for. Measured, five sites, three runs, identical
decisions every time:

```text
                             BEFORE                 AFTER
  merchant.upi9.pro          shell-only-2           SCORED · shape 0.830 · n=47   <- crosses M1
  experiencia.pichincha.com  shell-only-8           tree-divergence-25 (oracle 8 -> 34/46 probed)
  awlyaa.education.dz        shell-only-6           shell-only-6 · proxy ACCEPTED (10 vs 9 tags)
  esaj.tjsp.jus.br           shell-only-4           shell-only-4 · proxy REFUSED (37 vs 300)
  house.udn.com              shell-only-1           shell-only-1 · proxy REFUSED (6 vs 927)
```

**The pre-registered expectation was FOUR rows and the honest answer is ONE scored plus one
re-attributed.** The two refusals are the design working: a half-built reference is strictly worse
than an honest shell, and the instrument said so out loud instead of scoring our complete render
against Chrome's partial one.

### An ACCEPTED proxy that still returns a shell is an ATTRIBUTION, not a failure

`awlyaa.education.dz` is the one worth keeping. The proxy was accepted — 10 open tags against the
live page's 9 — and the reference is still six elements, because the live page **is** six elements:

```html
<html><head><title>Request Rejected</title></head><body>The requested URL was rejected.
Please consult with your administrator.<br><br>Your support ID is: 3937191494515588361 …
```

That is F5 BIG-IP ASM's block page, served `200 OK`, and `classify_fetch`'s 2xx bot-wall test keys
off Cloudflare infrastructure markers only, so it reads as a real document. Before this tick the row
said *"the ORACLE rendered a shell"* as a hypothesis; now the one-origin reference has **proved the
shell is the site's**. The row is still mislabelled as in-scope engine work and that is its own tick
— the 2xx bot-wall detector deliberately refuses prose markers, because mislabelling a genuine
render failure as a bot wall EXCUSES our own bug, which is the expensive direction.

### What the two refusals name for the next tick

`house.udn.com` refuses at **6 tags against 927**: the proxy serves the document, the document's
only content is a JS navigation to `/house/index`, and the proxied render does not arrive there.
A same-origin *navigation* is a different case from a same-origin *subresource*, and only the second
one is covered today. `esaj.tjsp.jus.br` half-boots at 37 of 300.

### The cost, stated rather than discovered later

The widened trigger runs two extra Chrome processes on every under-floor row — ~22 instead of ~11.
The first batch run of the new binary booked `experiencia.pichincha.com` as **`crashed`**; a solo
re-run and an identical repeat batch both returned `tree-divergence-25`, and all five proxy
decisions were byte-identical across the three runs. It is recorded rather than averaged away
(t881's rule), and it is the predictable shape of the cost: two heavy proxy paths concurrent at
`--jobs 2` is more resident memory than the same cohort was ever asked for before.

## "It is the only thing that changed" is an argument, not a control (t904)

The t904 sweep's common-set band came in at **+1.21 pts over 105 sites**, and two sites carried
almost all of it: `www.freesupertips.com` **0.042 → 0.766** and `www.crazyshop.pl` **0.255 → 0.641**.
Both re-measured byte-identical across repeat runs, so it was not weather-within-the-hour.

The argument from elimination was as clean as this loop ever gets. Between the two sweeps there are
five commits. **t899, t900 and t901 touched no file under `engine/` or `tests/` at all** — verified
with `git show --stat`, not remembered. t903 is instrument-only and structurally cannot move a row
that was already scored. So **t902 was the only engine change in the window**, and a +0.72 shape gain
on a real site would have been a major finding about the computed-style readback class.

One rebuild refuted it. `git checkout <t901> -- engine tests`, an incremental release build, the same
two sites in the same hour:

```text
                          t898      t904 (new)    OLD BINARY, same hour
  www.freesupertips.com   0.0415     0.766094        0.766094   n=466   cov 0.620506
  www.crazyshop.pl        0.2553     0.641281        0.641281   n=1405  cov 0.914118
```

Identical to six decimal places on both sites, **including the sample counts**. The sites moved. Strip
them and the band over the remaining 103 sites is **+0.16 pts — flat.**

### The reconciliation, which is the deliverable

```text
  corpus shape_mean   0.5465 -> 0.5718   (+2.53 pts)
    ├─ composition (5 scored in, 2 out; the newcomers score 0.98 and 1.00)   +1.32
    └─ common set, 105 sites                                                 +1.21
         ├─ 2 sites the OLD BINARY reproduces EXACTLY                        +1.05   <- the corpus
         └─ the other 103 sites                                              +0.16   <- FLAT
```

> **Elimination narrows the CODE candidate; it says nothing about the CORPUS candidate.** The
> code-side argument here was airtight and the corpus-side one was simply never made. A live
> population is a second explanation for every movement, and it is available at all times without
> anyone proposing it.

### The same discipline on the loss side, and there it cost nothing

`pogoda.by` fell 0.803 → 0.736 and crossed M1 downward. Three solo runs, ONE binary, ten minutes:

```text
  run 1   shape 0.789   cov 0.696   n=71     <- ABOVE the bar
  run 2   shape 0.717   cov 0.520   n=53
  run 3   —             —           n=0      css-starved-1  (unscored entirely)
```

The sweep's value sits inside the site's own spread, the sample count swings 53↔71, and the row
intermittently goes `css-starved`. **Refute with the NEW binary first** (t899): the old-binary
control is for a delta that reproduces, and this one did not.

### What a +3 scorability move was actually made of

```text
  ATTRIBUTABLE (t903)   merchant.upi9.pro    shell-only-2 -> SCORED 0.830   <- the M1 crossing
                        experiencia.pichincha shell-only-8 -> tree-divergence-25 (still unscored)
  WEATHER — in          7info.ru  ·  rpsc.rajasthan.gov.in  ·  app.ordertime.com
  WEATHER — out         coinmarketcap.com  ·  ru4.bongacams-ru.com  ·  pogoda.by
```

Scorability 107 → 110 (82.9% → **85.3%**) crosses the owner-lock's ~85% threshold for opening the
BiDi function leg. **It should not be spent yet**: two of the three gains are sites that answered the
network this time and did not last time, and both directions of that trade are one sweep deep.

### The number the tick exists to state

**M1 is 17.8% at t887 and 17.8% at t904 — flat across 17 ticks**, with the t898 dip in between being
the composition effect t898/t899 had already decomposed.

## The ranked cause list is a MECHANISM and a TAG, and the tag is the corpus-relevance filter (t909)

The mechanism oracle's output looks like a mechanism ranking. It is two facts:

```text
  37 site(s) · 2398 hits   missing box: <div>
  29 site(s) ·  292 hits   geometry/mis-sized: height ~64px    (<div>)
  29 site(s) ·  288 hits   geometry/mis-sized: height ~256px   (<div>)
  28 site(s) ·  365 hits   geometry/mis-sized: height ~32px    (<div>)
  …every ranked cause on this corpus is <div>. `<table>`, `<td>`, `<tr>`, `<th>` appear in NONE.
```

t907 (a table box's `height` is a minimum) and t908 (the UA `border-spacing` default) are both
Chrome-exact, both gated, and both **structurally unpriceable by this instrument on this corpus** —
not small, not lost in the noise: the corpus does not contain the failure. That was knowable before
either was built, because the ranker prints the tag beside the mechanism.

> **A fix found by a probe is worth taking on usage weight (VI.3), and it is worth knowing in advance
> that the sweep will score it zero.** Otherwise the flat reading gets re-litigated as a
> disappointment instead of being predicted as arithmetic.

### What the t909 sweep actually measured

```text
                       t887      t898      t904      t909
  M1 (the gate)       17.8%     17.1%     17.8%     18.5%
  scorability        107/129   107/129   110/129   108/130
  shape_mean           57.9%     54.6%     57.2%     57.0%
```

M1's +1 is `ru4.bongacams-ru.com`, `unreachable` at t904 and answering this time — the same site t904
booked as a *loss* for the same reason. Every scorability change is weather or the instrument
(`css-starved` twice, a site that now 404s, two returns from `unreachable`). The common-set band is
**−0.26 pts over 106 sites, 19 up · 68 flat · 19 down**; one noisy site (`mobcup.fm`, n=29) is −0.20
of it, and stripping it leaves **−0.068 pts — flat, inside the ±0.16 floor.**

### Four sweeps, twenty-two ticks, zero engine-attributable M1 crossings

t887 17.8 → t898 17.1 → t904 17.8 → t909 18.5, and every crossing in all four decomposes to a site's
network conditions. Beside t904's structural result — 56 of 107 scored sites fail BOTH conjuncts,
only 2 within 0.06 of the shape bar, only 1 a single jarring dimension from crossing — the reading is
not that the work is not landing. It is that **M1 has no resolution at this distance from the bar**,
and the loop's own headline cannot distinguish three correct geometry fixes from nothing at all.

## `missing` means the KEY is absent, not the box (t911)

`oracle.rs:172` books a `missing` divergence when, for one of Chrome's selector-path keys,
`manuk.get(id)` returns `None`. Three different failures collapse into that one word:

1. the node is absent from our DOM (a function / hydration gap),
2. the node exists and we gave it no box (a layout gap), or
3. **the node exists WITH a box under a different path** — `nth-of-type` is absolute, so one inserted
   sibling re-numbers every key beneath it (t780-783).

The sweep prints both engines' path counts on every site. Read against each other for the first time
at t911:

```text
  div_miss  oracle    ours  missing   site
       471    2407    2380     1247   sip777man.site      <- 99% as many boxes, 1247 "missing"
       322     665     625      625   www.kroftools.com   <- 94% as many, EVERY path missing
       220     458     601      456   www.jatekshop.eu    <- WE DRAW MORE, and share 2 of 458
       181     696     676      680   a1.ro               <- 676 vs 696, and 16 paths in common
        66    2032    2033       96   www.tz.de           <- WE DRAW MORE
```

**Of the 58 sites carrying a missing-`<div>` count, 22 render as many or more box-bearing paths than
Chrome.** `a1.ro` draws 676 against Chrome's 696 and shares sixteen.

> **Two engines that each draw ~690 boxes and agree on 16 paths are not one engine failing to render.
> They are two trees numbered differently.**

### The instrument already made this correction — for the reason string only

t782 split `TreeDivergence` out of `ThinOverlap` after measuring *"the one thing this variant never
looked at: our own element count"*. That correction reached the **unscored** path and stopped there.
A site that scores keeps feeding raw `missing` divergences into the ranked cause list, and the ranker
never asks t782's question. Same defect, one level out.

### The tell, on the top near-bar site

`www.jatekshop.eu`'s missing sample is not scattered — it is one contiguous subtree,
`body/div[1]/footer[1]` and every descendant beneath a chain of `div:nth-of-type(1)`. **A whole
subtree going missing at once is what an index shift at its root looks like**, not what dropped boxes
look like.

### What this does NOT say

The cause is not empty. 36 of the 58 sites do render fewer paths than Chrome, and `morikoshi.net`
(307 against **2**) and `www.agoda.com` (798 against 76) are real and severe. The finding is that the
ranked number is **a mixture of two populations and the board has been ranking their sum** — the same
shape as t695-697 (CLUSTERS.md's top rows re-measured to zero) and t780-783 (the board's named cohort
was an artefact three ticks running).

**The fix, BUILT at t912 (one tick later).** `diff_page` has both maps in hand, so the comparison is
one line — `manuk.len() >= chrome.len()` — and an absent key on a page where our map is not smaller
books as `unaligned` with its own ranked row (`unaligned key (we drew as many): <tag>`) instead of
feeding the `missing box` total.

It is **not an exoneration**, and the wording carries that: *our map is not smaller, so this absence
is not evidence of a dropped box.* Still a divergence, still counted, certificate arithmetic
unchanged — the same discipline `TreeDivergence` uses.

Gated by `an_absence_is_only_a_missing_box_when_our_map_is_smaller`, **both directions**: our map
smaller must STILL book `missing` (a change that relabelled every absence would empty the board's top
row and look like progress), our map shifted by one key books `unaligned`, and an EQUAL count takes
the `unaligned` reading because `>=` and `>` disagree at that boundary. RED-proven by pinning the
comparison to `false`.

⚠ **The re-ranked board arrives with the next SWEEP, not with the commit**: the cause ranking is
computed during a crawl and the JSONL ledger bakes each divergence's kind in, so the banked t909 rows
cannot be re-split retroactively. Until that sweep runs, a `<div>` tick should be taken from the
`geometry/mis-sized` rows, which compare boxes that DID align and are therefore unaffected.

## ⚠ RETRACTED — "a gate that builds its own inputs proves the function, not the wiring" (t919, corrected t920)

t912 split the ranker's #1 cause into `missing` (our map is smaller) and `unaligned` (it is not). The
t919 sweep fired `unaligned` **zero times in 200 sites**, and this section concluded that the split
was inert — that the gate constructed its own maps and passed while the real ones never took the
branch.

**That was wrong, and one probe at the call site refuted it:**

```text
  [PROBE] diff_page sees chrome=57  manuk=437     www.naukri.com
  [PROBE] diff_page sees chrome=59  manuk=61      chat.google.com

  === G1 ROOT CAUSES — ranked by sites explained ===
     1 site(s) ·   32 hit(s)   unaligned key (we drew as many): <div>
     1 site(s) ·    3 hit(s)   unaligned key (we drew as many): <input>
```

It works, and it works under `--jobs 2 --rows-out` — the only bankable sweep configuration, where the
`--jobs N` path re-spawns itself as N children and merges their partial rows.

> **CHECK WHAT BINARY PRODUCED A SURPRISING READING BEFORE CHECKING WHAT THE CODE DID.** t919 had a
> mechanism that was unit-gated, RED-proven and green on its own tests firing zero times across 200
> sites, and went straight to *"the wiring is wrong"* — wrote this section, banked a lesson, and never
> asked the first question. The sweep's binary has since been overwritten, so which build it was
> cannot be recovered; that is the cost of not asking at the time.

**A second false finding was nearly banked on top of it.** The first parallel test ran `--jobs 2`
**without `--rows-out`** and produced zero `unaligned`, which reads as *"the parallel path is a second
implementation"* — a shape this project has genuinely found four times and would have believed.
`--jobs` *requires* `--rows-out` and bails when it is absent, so that arm never ran the parallel path
at all. **The probe measured its own missing flag.**

What survives from t919 unchanged: every metric pre-registration held, and the **t918 regression
stands completely** — it was never inferred (three solo runs current, two on the old tree, a
four-point bisect, and a revert that restored the number). That finding asked the binary question at
every step, which is the contrast worth keeping.

## Nine Chrome-captured claims are not a proof about the web (t919)

The same sweep caught a regression in the window it was measuring. `secure5.entertimeonline.com` fell
**0.872 → 0.692** on 39 elements, and the pre-committed control resolved it in one pass:

```text
  three solo runs, CURRENT binary        0.692308  0.692308  0.692308   byte-identical
  two solo runs,   t913 tree (OLD)       0.871795  0.871795
  bisect: t914 / t915 / t916 trees       0.871795 each
          t918 tree (HEAD)               0.692308   <- the regression
  with t918's layout hunk reverted       0.871795
```

t918's form-control baseline synthesis is **Chrome-exact on nine isolated fixtures, four of them
guards**, and costs 0.18 shape on a real page. The ratchet does not weigh those against each other —
*a tick that buys one face by degrading another is a trade, and trades are refused* — so the engine
hunk and its gate were both removed.

> **A fixture that refutes your hypothesis is the cheapest outcome; being refuted by the CORPUS is
> the second-cheapest and the one no fixture can substitute for.** Same shape as t853, where
> `hit_test`'s smallest-wins rule cost sixteen clickable links and was found by G6 on a real page.

The next attempt has a sharper question than the first: not *what is a control's baseline* — the nine
fixtures answer that — but *which real-page control does the formula get wrong, and why*. The named
candidate is already on the board (an input with an explicit `height:40px` reads 47 against Chrome's
46, because Chrome centres the internal editor in a taller control), and
`secure5.entertimeonline.com` is now the reproducer.

## The one-origin proxy cannot lie about the hostname (t921)

The proxy's named next lever was *"it does not follow a same-origin NAVIGATION"* —
`house.udn.com`'s entire body is `window.location.href="/house/index"` and the proxied render is 6
open tags against the live page's 932. That diagnosis was plausible and wrong.

The site's own first ten lines have it:

```html
  <script language="javascript">
  if (document.URL.indexOf("house.udn.com") != -1) {
      window.location.href = "/house/index";
  }
  </script>
```

**Under the proxy `document.URL` is `http://127.0.0.1:PORT/`, the guard is FALSE, and the page never
navigates.** Not a forwarding bug and not a probe bug — the page asks what host it is on, and the
proxy's entire purpose is to answer differently.

> **ASK WHAT THE PAGE BELIEVES**, now aimed at our own reference instrument. The one-origin proxy can
> serve any byte the site would serve and **it cannot lie about the hostname**. Every page that
> self-checks its origin — a hostname redirect, an environment switch, an analytics guard, a
> `location.host`-keyed CDN prefix — is structurally outside what this reference can measure.

That is a **limit**, not a bug. Serving under the real hostname needs DNS or TLS interception, which
is a different instrument. `renders_agree` already refuses these rows, which is correct; the value of
naming the cause is that no further tick spends itself guessing at the forwarding path.

### The cohort was checked, not generalised from one site

Of the shell rows measured at t903, `esaj.tjsp.jus.br` names its own host **20 times** in its
document (and refuses at 37 tags against 300), while `awlyaa.education.dz` and `merchant.upi9.pro`
name it **zero** times — and `merchant.upi9.pro` is precisely the row the proxy already converts.
**The rows the proxy cannot reach and the rows that mention their own hostname are the same rows.**

### What was kept anyway

An HTML response reached by navigation is a DOCUMENT, and it now gets the same treatment the entry
document gets — foreign hosts rewritten back through this origin, probe injected — instead of being
forwarded raw. Before, a proxied page that navigated was measured as an **unprobed, unrewritten**
document: the "half-built reference" this module exists to refuse, arriving through a door nobody had
checked. It is latent today because `renders_agree` refuses those rows for other reasons first. **A
latent correction with the tests green is worth keeping; claiming it moved a number would not be.**

## The mechanism works, and does not work at sweep scale (t929)

t912 split the ranker's `missing box` cause into `missing` (our map is smaller) and `unaligned` (it is
not). t919's 200-site sweep fired it **zero** times and concluded the wiring was inert. t920 refuted
that with a live two-site run and retracted the whole finding. **Both were partly right:**

```text
  2 sites, `--jobs 2 --rows-out`, live       unaligned key (we drew as many): <div> x32, <input> x3, <span> x3
  200 sites, same flags, binary build time   ZERO — while 32 sites meet the condition by the sweep's
  recorded before the run                    OWN printed counts (`ours >= oracle`, missing > 0)
```

The code path, the binary and the `--jobs` value are all now controlled, so the difference is scale:
the `--jobs N` **spawn loop** (a chunk re-spawns for its remainder until every URL has a row, each
re-spawn printing its own ranking) or something else about 100 sites in one child. Bounded question,
obvious next probe, stated rather than guessed.

> **"It works" and "it works in the artefact you read" are different claims, and a two-site probe
> cannot distinguish them.** t920 was right that the mechanism is not broken and wrong to retract the
> observation; the observation was about the sweep, and the sweep still shows zero.

### The analysis parser was mis-associating names

Splitting the log on `side-by-side:` lines pairs each site NAME with the NEXT block's numbers — this
sweep's table shows `www.agoda.com  oracle=57 ours=437`, which are naukri.com's numbers verbatim.

**The COUNT is name-independent and stands** (32 sites here, 26 at t919, 22 at t911). **The per-site
attributions in t911 do not.** Every conclusion drawn from the count survives; every sentence naming a
specific site through that parser needs re-deriving.

> **A parser that mis-associates names produces a table that is right in aggregate and wrong in every
> row** — the same shape as the instrument defects this loop hunts, in throwaway analysis code nobody
> gates, and it survived three ticks because the aggregate kept agreeing with itself.

## A shape delta is not a shape delta until both readings scored the same POPULATION (t1102)

`www.timeline.com` fell **0.4102 → 0.3179** between the t1089 and t1099 sweeps, **reproduced** on a
solo re-run (0.3180), and was carried for two ticks as the window's one unrefuted regression
candidate — the only thing standing between four Chrome-exact generated-content fixes and a clean
ratchet.

**The old-binary control settled it, and the answer was byte-identical.** `engine/` checked out at
`e527bb8b` (the commit before t1092, the only fix of the four that can reach a site with 8
block-level pseudos, no `display:none` pseudos and no counters), rebuilt release, run in the same
hour, three times each arm:

```text
                          coverage    shape     h_ovf  ovl  ro  n      instrument
   t1089 (banked)         0.978741   0.410192    454    1   18  1197    85ca9328
   t1099 (banked)         0.848733   0.317919    399    1   18  1038    85ca9328
   HEAD today   ×3        0.848733   0.317919    399    1   18  1038    85ca9328
   OLD BINARY today ×3    0.848733   0.317919    399    1   18  1038    85ca9328
```

Six decimals, every jarring count, on a binary that predates the change being accused. **The engine
is not what moved.**

### The reason was two columns to the left of the number everyone was reading

`coverage` fell 13 points and `shape_n` fell by 159 elements. `shape` is a mean over the elements
**both** engines rendered, so the two numbers are means over different samples of a page that itself
changed. Subtracting them measures the site's news cycle.

⚠⚠⚠ **A SOLO RE-RUN CANNOT SEE THIS.** Re-running the site today measures *today's* population
twice and agrees with itself perfectly — which is exactly what the t1099 protocol did, three times,
before concluding `REPRODUCES`. The solo re-run is the right tool for **churn** (a site that gives a
different answer to the same binary in the same hour) and is structurally blind to **drift** (a site
that gives a stable answer to a different page). Those are two failure modes and the loop had one
instrument.

### It is not one site — it is every headline mover in the diff

Of the 115 sites in both sweeps, 25 moved more than 2 shape points. Six flag as population changes,
and those six are **all five of the largest losses and the largest gain**:

```text
   sports.yahoo.com    -0.856   n 1693 → 3      cov 0.991 → 0.273
   www.timeline.com    -0.092   n 1197 → 1038   cov 0.979 → 0.849
   www.paypal.com      -0.090   n  534 → 429    cov 0.893 → 0.717
   mangaraw.ac         -0.067   n  733 → 873    cov 0.836 → 0.755
   pogoda.by           -0.057   n   71 → 53     cov 0.696 → 0.510
   www.aftenbladet.no  +0.131   n  999 → 622    cov 0.951 → 0.924
```

**And what is left once they are removed answers t1099's headline.** The 19 attributable movers are
**7 losses and 12 gains, net +0.830 shape points, worst single loss −0.041** — inside the ±3.7-point
spread t654 measured on an unchanged tree. On the sites where the comparison is legitimate at all,
the window moved shape **up**; the "flat metric" was a diff dominated by six rows that were never
the engine's to answer for.

### The mechanism, because a lesson is not a mechanism

`fidelity::sweep_diff` partitions every per-site delta into **instrument-changed** /
**population-changed** / **ATTRIBUTABLE**, and `manuk-wpt sweep-diff OLD.tsv NEW.tsv` prints the
three groups with the attributable one last. Thresholds: `|Δn| > 10%` of the earlier sample, or
`|Δcoverage| > 5` points, or a different `instrument` tag (the rule `shape_spreads` has enforced for
error bars since t676, applied between sweeps for the same reason).

⚠ **The thresholds are not fitted at one point** (the t1042-1045 trap). Varying them over 6× on
`dn` and 5× on `dcov` moves the partition by at most one site and catches all five top losses in 11
of 12 cells:

```text
     dn \ dcov     0.02      0.05      0.10
        0.05      9 (5/5)   8 (5/5)   8 (5/5)
        0.10      7 (5/5)   6 (5/5)   6 (5/5)     <- chosen
        0.20      7 (5/5)   6 (5/5)   5 (4/5)
        0.30      7 (5/5)   6 (5/5)   5 (4/5)
```

Two rows keep it from being a classifier that flags everything: a **control** site with a −9.0-point
drop and an intact population must come back ATTRIBUTABLE, and a site present in only one file must
not be reported at all (that is a scorability change, and printing it as a movement is how a dropped
hard site flatters a mean).

**GATE** `G_SWEEP_DIFF_POPULATION` —
`a_shape_delta_across_a_changed_population_is_not_attributable`, on the real t1089/t1099 rows.
RED-proven on both arms: disabling the population test flips timeline to `Comparable`; disabling the
instrument test flips `oldprobe` to `Comparable`.

⚠ A **latent tie** was found while gating it, the t853 shape again: two sites with equal deltas came
out of a `HashMap`, so the report's row order depended on hash iteration and the test passed alone
and failed in the suite. The sort now breaks ties on the site name.

## The `--jobs 2` sweep reads ~4 points LOW on shape, and a shape score carries no sample size (t1135)

The cadence sweep pricing t1128–t1134 came back **flat with a negative band**: corpus gauge
0.4692 → 0.4678, common-set mean Δshape −0.0026 over 107 rows, 14 down >2pt against 6 up. Read as a
headline that is *"seven fixes, no movement, and a slight loss."* Every part of the negative half is
the instrument.

### Seven "regressions" with byte-identical node counts

The down-movers are not small pages: `gismart 281→281`, `bhfudbal 684→684`, `crazyshop 1405→1405`,
`puentedemando 1132→1132`, `kuechenmomente 894→894` — the **same node count before and after**, which
is what says the page did not change. Re-run SOLO on the same binary, and against the OLD binary
(the t1133 tree), all in the same hour:

```text
                          t1127swp  t1135swp   SOLO new   SOLO old
   gismart.com              0.843     0.797      0.840      0.840
   bhfudbal.ba              0.596     0.551      0.595      0.595
   www.crazyshop.pl         0.658     0.618      0.655      0.658
   www.puentedemando.com    0.822     0.792      0.759      0.757
   developers.google.com    0.583     0.547      0.547      0.547   <- real SITE drift
```

**The two binaries agree to three decimals on every row**, so not one loss is attributable to the
engine — and the solo column recovers the t1127 value on three of them. The sweep's own concurrency
is depressing shape by ~4 points. t771 banked that `--jobs 8` costs hard sites their SCORABILITY to
wall-clock timeout and that `--jobs 2` is bankable; this says `--jobs 2` has a **shape** cost too,
smaller and quieter, and it lands in the same direction on every site at once — which is exactly the
shape of a systematic bias rather than noise.

**A `--jobs 2` row is bankable for the DENOMINATOR and is not comparable to a solo number.** The
burndown's per-site values and any solo re-measure live on different scales.

### A shape score has no sample size attached

14 of the 121 rows scored in both sweeps are computed over **≤10 nodes**, and 12 of them are frozen:
identical shape, identical `n`, Δ exactly `0.000`, sweep after sweep.

```text
   house.udn.com          n=1     shape 0.000    frozen
   allticketscol.com      n=1     shape 1.000    frozen  <- a full shape-PASS on ONE node
   dashboard.twitch.tv    n=2     shape 0.500    frozen
   booking.directferries  n=2     shape 0.500    frozen
   merchant.upi9.pro      n 47->2 shape 0.915 -> 0.500   <- 1 of 2, worth -0.415 in the band
```

`0.500` is not a fallback constant; it is **1/2, from a sample of two**. `merchant.upi9.pro` served a
shell on the sweep's request, its sample fell from 47 nodes to 2, and the resulting coin-flip was
banked with the same authority as a 1724-node row — a single row moving the whole common-set mean by
0.0035. Solo, the same binary returns **0.914894, byte-identical to t1127**.

And `allticketscol.com` at `1.000` on one node counts as a shape-pass in the M1 numerator. The gate
weights sites, and the instrument weights nodes; nothing reconciles the two.

### Correcting only the losses is the trap

Removing the two refuted losses flips the mean from −0.0024 to +0.0031. Removing the small-`n`
**gain** in the same breath (`experiencia.pichincha` +0.357 at n 7→4) puts it back to −0.0002.
*Apply the solo-rerun rule to the numbers you like.* The honest statement is that the band is FLAT
and that the window's real movement is two sites the mean cannot see.

### Scorability has a CHURN FLOOR, and the loop ranks its whole work-list on it

`scored / in-scope` is the number the board calls *"M1's hard ceiling"* — the term the throw-killer
programme exists to move. Measured across the last eight banked sweeps, its sweep-to-sweep NET
alternates in sign **every single time** while the GROSS is three-and-a-half times larger:

```text
   t1089->t1099  gained 6  lost 9   NET -3   GROSS 15
   t1099->t1109  gained 6  lost 3   NET +3   GROSS  9
   t1109->t1117  gained 5  lost 7   NET -2   GROSS 12
   t1117->t1121  gained 6  lost 4   NET +2   GROSS 10
   t1121->t1127  gained 2  lost 4   NET -2   GROSS  6
   t1127->t1135  gained 3  lost 1   NET +2   GROSS  4
   t1135->t1145  gained 2  lost 7   NET -5   GROSS  9
```

**Twenty-two rows flip `scored`↔`unscored` two or more times over those eight sweeps.** A quantity
whose sign alternates and whose gross exceeds its net by 3.5× is not measuring a trend, and every
scorability reading published since t1089 sits inside that band. The one movement in the whole record
that clears it is t786's +15.5 points (the selector-path keying fix).

**Some of the rows are deterministically bistable, not noisy.** `sports.yahoo.com` alternates with
period two across six consecutive sweeps — and it reproduces SOLO, so it is neither contention nor
the `--jobs 2` scale bias:

```text
   t1089  0.856 / n=1693   scored           t1127  0.000 / n=3   tree-divergence-1924
   t1099  0.000 / n=3      tree-divergence  t1135  0.885 / n=1724 scored
   t1109  0.881 / n=1637   scored           t1145  0.000 / n=3   tree-divergence-1714
```

That one row is worth an M1 site and ~0.9 shape points every other sweep, for free, and the loop has
been attributing the swing to whatever landed in between. Check #103 saw one half of it and filed it
as a population change; it is an oscillation.

> **Read `NET` against `GROSS` before reading a scorability delta as progress.** A net of ±2 on a
> gross of 10 is four sites disagreeing with four other sites. Rank on the MEMBERSHIP diff, name the
> members, and run the solo control on one of them — that is what turned a "−5 scorability
> regression" at t1145 into five wall-clock timeouts, a 403, and a bistable row.

### A `timeout-150s` is a claim about the CLOCK, and it is testable in one command

t1145 lost five sites to `timeout-150s` (up from 5 and 7 in the two prior sweeps to 12). Solo, on the
same binary in the same hour:

```text
                        sweep             SOLO
   www.ikea.com         timeout-150s      0.783742 / 652   <- byte-identical to t1135, ten hours earlier
   redinfor.com.pe      render-failed     0.722222 / 36
   www.ebay.com         timeout-150s      bot-wall-403     <- the origin refuses us; not a clock at all
```

A reason string that asserts a cause is a hypothesis with a test attached (check #73). One member,
one command, and a five-site "regression" stops being engine work.

## A missing-box count is a DESCENDANT count — rank by truncation points, not by tag

`MISSING_BOX` has been ranked by tag since t684 (`C3833 <div> 7544/32`). On the t1145 sweep's
highest-deficit site (`taphouse23.com`, coverage 0.424, ~2453 missing) the instrument's own output
says why that ranking is wrong. Every missing-box example carries the node **below which our tree
stops**, and all fourteen printed for that site name just three:

```text
   12  body/div:nth(4)/div:nth(1)/div:nth(4)/div:nth(1)/div:nth(1)/div:nth(2)/div:nth(1)
    1  body/div:nth(2)/…/ul:nth(1)/li:nth(5)
    1  body/div:nth(2)/…/ul:nth(1)/li:nth(2)
```

One container that was never built contributes its **whole subtree** to the count, one row per
descendant. The per-tag histogram is that subtree's inventory:

```text
   MISSING by tag: div×997  source×532  a×260  img×247  picture×245  li×121  ul×10  button×7
```

`picture×245`, `source×532` and `img×247` are the same 245 `<picture>` elements counted three times —
a `<picture>` contains its `<source>`s and its `<img>`. **Ranking by tag ranks the contents of the
largest missing container.** The parent is already printed in every example and nothing reads it.

### And the deficit does not say which LEG it belongs to

Chrome renders that page **~62,000px tall** (`[8 62022 200×200]` inside the missing subtree). That is
t267's third population, named 880 ticks earlier: *"an offset of 6822px is not a layout error — it is
CONTENT THAT NEVER RENDERED (lazy-load / IntersectionObserver / JS-driven expansion). Diagnose as a
CONTENT problem, not a geometry one."* A coverage-deficit ranking built to find layout work put a
content bug at #1.

> **Split a coverage deficit by its truncation point before spending a tick on it.** A *content* stop
> (hydration / IntersectionObserver) is the function leg; a *box-generation* stop is layout. On the
> t1145 sweep, 16 of 105 scored sites are below 0.70 coverage — large enough to be a whole unbuilt
> feed — while the 30 sites at 0.70–0.90 have deficits too small for that, which is where the layout
> work more likely is.

⚠ Corpus-wide rankings must come from the banked ROWS, not the log: the sweep log prints `e.g.` lines
for **5** sites and a root-cause section for **2**, while its cluster lines claim up to `14 site(s)`.
Re-derive from `(1 − coverage) × n/coverage` over `SWEEP-t<N>-rows.tsv`.

## `MANUK_RO_TRACE` — an inversion is reported on the PAIR, and the defect is one box (t1150)

`jarring_reading_order` counts sibling pairs the two engines order differently. Its exemplar line
prints **two paths and nothing else**, so every tick aimed at the invariant began by re-fetching the
page and hand-walking `nth-of-type` chains — against a *different frame*, because `boxes --fetch`
renders the live URL while the oracle renders a `curl` snapshot. A diff of two frames is not a diff
(t830), and on a CMS-driven page the hand-walk does not even resolve.

Everything the next question needs is already in the map: the keys are `/`-separated paths, so the
pair's rects, their `position`/`display`, and the whole ancestor chain are one lookup away.
`MANUK_RO_TRACE=1` prints them:

```text
  RO-TRACE <parent path>
      chrome  <A> [x y wxh] pos/disp   <B> [x y wxh] pos/disp
      ours    <A> [x y wxh]            <B> [x y wxh]
      chrome reads <A> first, we read <B> first   (<which axis carried the swap>)
        chrome [...]  ours [...]  dx +0 dy +0   <ancestor>
        ...                                     <-- FIRST DIVERGENCE
```

**The axis label is the diagnosis, not decoration.** `order()` is vertical-first, so:

| axis line | what it means | which subsystem |
|---|---|---|
| BLOCK in both | the two boxes swapped rows | block flow / box heights |
| BLOCK in Chrome, INLINE here | we collapsed two rows onto one | a box is too short, or a break was lost |
| INLINE in Chrome, BLOCK here | we broke one row into two | a box is too tall, or a float/wrap fired |
| INLINE in both | the boxes swapped columns | inline direction, float side, order |

`reading_order` had been ranked as one number for three sweeps while being **at least two
mechanisms**, and nothing in the output could separate them.

Same design as `MANUK_HOVF_TRACE` (t1112) and for the same reason: off by default, print-only, the
count computed *before* it, so it cannot filter a verdict. The pair to walk is chosen by whichever
box moved further from Chrome — a box lands in the wrong row because of something above it, and the
chain marks the first row where that starts.

⚠ Read it with `MANUK_RO_PARTITION=1`, which answers a different question: *how many of these
inversions involve a zero-area box, a box parked off-screen left, or an in-flow/out-of-flow pair* —
i.e. whether the count is an engine target at all before you spend a tick on which box moved.

## The divergence line has always carried the FONT, and on half the mass it says the font AGREES (t1243)

Every geometry divergence the sweep prints looks like this, and the braces are not decoration:

```text
  …/div:nth-of-type(3): [610 1384 566×566]  {Noto IKEA/16/170}
                     vs [610 1384 566×84]   {Noto IKEA/16/218}
```

The triple is `first-declared-family / used px size / ADVANCE`, where the advance is Chrome's own
`measureText` of a fixed probe string **in that element's resolved font** (`chrome.rs`, probe field 6,
added t563 for exactly this reason: *"`[74x16] vs [76x18]` is unattributable without it"*). So the two
sides of a `vs` answer, per divergence and for free, the question every shape tick has had to guess
at: **did the same text metrics produce these two boxes?**

Nobody had ever read it in aggregate. Asked of all 1,330 example lines in the t1243 CrUX sweep:

```text
   698  52.5%   font IDENTICAL          same family, same px, same advance — and a DIFFERENT box
   448  33.7%   ADVANCE differs (same family + size)
   175  13.2%   FAMILY differs
     9   0.7%   SIZE differs
```

**More than half the divergence mass is boxes whose text metrics agree with Chrome to the pixel.**
No font constant can move those; they are layout math. This is the falsification of the board's
oldest shape lever — tick-267's *"ONE shared constant (font metrics / line-height / margin /
border-box rounding) likely snaps MANY boxes into 8px tolerance at once"*, carried unchallenged for
~980 ticks — and the geometry itself says the same thing, because **a shared constant is a
DIRECTIONAL error and this mass has no direction**:

```text
  HEIGHT differs:  ours TALLER 365 · ours SHORTER 375     51% / 49%
  WIDTH  differs:  ours WIDER  202 · ours NARROWER 361    36% / 64%
```

Even the third that *does* disagree on advance is undirected (239 wider, 209 narrower, mean
ours/chrome 1.054) — that is per-site face substitution (a webfont Chrome loaded and we did not),
not one constant.

⚠ **The sampling bounds the claim, and it is stated rather than buried:** the root-cause block prints
up to THREE examples per cause, so this population is weighted toward the top causes. It is enough to
kill "one constant fixes many sites"; it is not a census of every divergence.

### The per-site cause block is a per-site ranking — the corpus one has to be rolled up

Each run prints `G1 ROOT CAUSES (§3b)` ranked by **that site's** hits, and the board asks for a
ranking by **distinct sites**. Those are different orders, and reading the per-site one as if it were
the corpus one is how `missing box` stayed first for a year: it leads on hits (10,943) because a
missing box is a *descendant* count (t911 — one dropped container bills its whole subtree), and it is
**sixth on sites** (42). Two rollups, both cheap, both now banked per sweep:

| artefact | key | what it is for |
|---|---|---|
| `SWEEP-t<N>-causes.tsv` | the full label, incl. tag and power-of-2 band | aiming a fix once the mechanism is chosen |
| `SWEEP-t<N>-mechanisms.tsv` | the label with tag and band stripped | choosing the mechanism — this is MASS |

The fine labels fragment one cause across a dozen rows (`height ~16px <div>`, `height ~256px <div>`,
…) and every row then looks small, which is precisely the shape that hides a shared root cause.

## The oracle deleted the doctype and scored us in QUIRKS MODE (t1247)

HTML's parser treats a `<!DOCTYPE>` as a doctype **only when it is the first thing in the document**.
Three probes — `capture_url_screenshot`, the `[id]` box probe, and `capture_seen_all_paths` (the one
that produces SHAPE) — each built their document the same way:

```rust
if let Some(i) = html.find("<head>") { …insert after <head>… } else { format!("{base}{html}") }
```

The `else` branch puts `<base href=…>` in front of the doctype. The doctype degrades to a
bogus-comment token and Chrome parses the page in **`BackCompat`**. In quirks mode a percentage
height walks up through auto-height ancestors to the initial containing block; in standards mode
CSS2 §10.5 computes it to `auto`:

```text
                                        through the probe   Chrome, same file, direct   ours
  inline-block, height:100%, auto CB        50x800                 50x16                50x16
  BLOCK child,  height:100%, auto CB        50x800                 50x16                50x16
  inline-block, height:50%,  auto CB        50x400                 50x16                50x16
```

So the instrument reported a **784px divergence on a row where the engine is exactly right** — and it
would have reported it forever, because a page with no literal `<head>` cannot be scored honestly by
a probe that deletes its doctype.

**It was live on the corpus, not only on fixtures.** Of 200 CrUX URLs, 183 fetched non-empty; **16
ship no literal `<head>` and 9 of those carry a doctype** — `celeb.gate.cc`, `rpsc.rajasthan.gov.in`,
`aksesjambi.com`, `littlecaesarsbcs.libellum.com.mx`, `www.hdnails.it`, `patrickmorin.com`,
`www.otomoto.pl`, `gismart.com`, `ofero.id`. ⚠ `celeb.gate.cc` is the site this loop has used as its
most stable CONTROL, and `www.otomoto.pl` is one of its largest scorers.

The fix is one `splice_head`: `<head>` if present, else **after the doctype**, else prepend — no
doctype means the author already chose quirks, so prepending is faithful. Three call sites had
written the same wrong `else` three times.

### The repair obligation, discharged rather than asserted

t1242 wrote the rule: *an observation banked from an instrument you subsequently repair does not
survive the repair, and it will not retract itself.* Every Chrome row in t1244/t1245/t1246 came
through this probe, so all four fixtures were **re-run through the repaired instrument**:

```text
  t1244-fix 61.5% → 100.0%   ·   t1244-ctl 80.0% → 100.0%
  t1245-a   90.9% → 100.0%   ·   t1245-b   72.7% → 100.0%   ·   t1247-p 11.1% → 100.0%
```

All five reach 100%, which **validates** those three fixes: had they been aimed at quirks-mode
targets, the repaired instrument would now show divergence. The rows they acted on were percentage
and `stretch` heights against a **definite** parent, which resolves identically in both modes. The
one row the bug did corrupt is exactly the one that was honestly left *"NOT ESTABLISHED"* — which is
the argument for writing that label instead of guessing.

## A vertical divergence is a SHIFT or a RESIZE, and the ledger cannot tell them apart (t1253)

The mechanism ledger ranks by where a divergence is **observed**. For the vertical axis that conflates
two different things, and the split is measurable in one pass over the sweep's own example boxes
(`dw`, `dy`, `dh` between Chrome's box and ours):

```text
  615 rows with a vertical divergence
    191   31.1%   PURE SHIFT   right size, right width, wrong Y   -> cause is ABOVE it
    114   18.5%   PURE RESIZE  right origin, right width, wrong height -> cause is INSIDE it
    306   49.8%   mixed
      4    0.7%   dy == -dh (margin-shaped)
```

**A third of the vertical mass is a consequence, not a defect.** A box that is the correct size and in
the wrong place was pushed there by something earlier in the flow — so `geometry/displaced: y`, which
the ledger ranks at 69 sites, is substantially a *symptom* cluster. The oracle already computes
`FIRST DIVERGENCE` per site; the ranking does not use it.

And the shift population is cheaper than its count: per site, count the **distinct** `dy` values.

```text
  26 of 66 sites — EVERY shifted box carries the SAME dy   -> ONE upstream box explains the whole site
  40 of 66 sites — more than one distinct dy
```

On 40% of affected sites the whole shift population is one defect repeated down the page. Those sites
need 26 fixes, not 191 — and the `dy` histogram per site is the cheapest way to find out which kind
of site you are looking at before spending a tick on it.

### Two hypotheses this pass killed, both in one command each

- **"The box is narrower, so the text wraps taller."** On the small-band height rows the **width
  agrees on 86%** of them.
- **"It is margin collapsing."** One site (`sestra.cc`: Chrome `[0 0 1200×110]`, ours
  `[0 10 1200×100]`) has the exact signature — lower by precisely what it lost in height. Across all
  615 rows that signature is **0.7%**. One site is not a class (t1235), and testing it cost less than
  the fix would have.

## A SCORABILITY regression is a ratchet question — chase it to an old-binary control, not to a shrug (t1268)

`fidelity-progress.sh` printed two warnings on the t1268 sweep, and they are the two it should print:

```text
  ⚠ SCORABILITY-REGRESSED: scored 108 -> 107 (fewer sites measurable — investigate, not progress)
  DENOMINATOR-TRAP: shape_mean 62.7->63.0 ROSE while scored 108->107 FELL — the gain is NOT real
```

A pass-count that rises while the denominator of *measurable* sites falls is the composition trap this
instrument was built to catch, so the +1.0 pt on `shape ≥ 0.75` is not progress and was not reported as
such. The **common-set band** — the mean Δshape over the 104 sites scored in *both* sweeps — read
**−0.01 points**, and `manuk-wpt sweep-diff` classified all 11 movers as NOT ATTRIBUTABLE with
**zero** in the attributable bucket. That is the reading.

### The scorability −1 is the part that cannot be waved through

"Fewer sites are measurable" is a ratchet question: if an engine change stopped a site rendering, the
tick is a regression regardless of what else it bought. The cheap first cut is to **bucket the reason
column across the common rows and look at the DIRECTION of the changes**:

```text
  GAINED an unscorable reason          LOST one (now scored)
    7info.ru           timeout-150s      mangaraw.ac    timeout-150s -> scored
    coinmarketcap.com  timeout-150s      morikoshi.net  timeout-150s -> scored
    payb.jp            timeout-150s      mayatoys.in    unreachable  -> scored
    www.cuneocronaca.it   css-starved-1
```

⭐ **Three timeouts gained and three lost IS the churn signature.** A real regression is one-directional;
a wall-clock bound flapping in both directions on a 150s budget is the bound, not the engine. What that
leaves is the one row whose shape is different — a site that was scored and is now unscorable for a
*non-timeout* reason — and that one gets the expensive answer.

### The control, and what makes it a control

```text
  SOLO, current binary        www.cuneocronaca.it  css-starved-1
  SOLO, OLD-BINARY CONTROL    www.cuneocronaca.it  css-starved-1
```

The old binary was built by checking the *suspect files* out at the pre-suspect revision
(`git checkout <sha> -- engine/js/src/{reflect_js,event_loop,lib,dom_bindings}.rs`), rebuilding release,
running in **the same hour on the same box**, then restoring and rebuilding. Identical reason ⇒ site
drift or an origin change, not ours.

⚠ **Two mechanical traps in doing this, both hit and both cheap to avoid.** `git checkout <sha> -- <paths>`
**stages** the old content, so copying the new files back leaves the index holding the old ones (`MM` in
`git status`) — finish with `git restore --staged` and verify `git diff --stat engine/` is empty before
believing the tree. And the restore must be followed by a **rebuild**, or the next measurement is taken
with the control binary still on disk.

⚠⚠ **The cost/benefit is not close.** ~10 minutes of build bought the difference between an unjustified
revert and an unexamined regression. t799's rule stands: *only a same-hour old-binary run attributes
cost*, and a per-site delta is a question until it has one.

## The reference hid its scrollbars and the engine did not — a 15px inline deficit under every sweep ever taken (t1319)

`chrome::base_flags` has passed `--hide-scrollbars` to every headless capture for the life of this
instrument, under a comment that named the exact defect it was about to cause:

> *"`--hide-scrollbars` matters: a visible scrollbar would shrink the layout viewport and shift every
> box."*

Every word true — and it shrinks the layout viewport of the **reference only**. Our engine reserves a
classic 15px gutter, as a desktop browser does. Measured on this box, one document 5000px tall:

```text
   google-chrome --headless=new --window-size=1200,887
      --hide-scrollbars     documentElement.clientWidth = 1200
      (no flag)             documentElement.clientWidth = 1185   ← what our engine computes
```

⭐ **Our 15px is Blink's 15px to the pixel.** The two engines agree; the instrument did not. Nobody
had ever asked them for the same number, because `viewport_chrome_offset` — the probe that exists to
make the reference's viewport match ours — runs a **one-line, non-overflowing** document, which has no
scrollbar to hide. It reported the inline offset as 0 and was blind to this by construction.

### What it cost, on a site the burndown already names

`ticket.jfa.jp` ships `overflow-y: scroll` in `css/common.css`. Its layout is a `width: 90%` container
with `5%` margins:

```text
                    Chrome           Manuk (before)      Manuk (after)
   ICB               1200             1185                1200
   90% container     1080             1067                1080
   5% margin           60               59                  60
```

1185 × 0.9 = 1066.5 → 1067, and 1185 × 0.05 = 59.25 → 59. No other viewport width produces both
numbers, which is what made the diagnosis arithmetic rather than a guess. The narrower column re-wraps
prose, each extra line pushes everything below it down, and the result is the *width launders into dy*
accumulation `docs/loop/PHASE0-RENDER-BURNDOWN.md` §11 ranks its near-miss band by. Same binary, same
day, only the flag differing:

```text
   ticket.jfa.jp   SHAPE 66.4% → 82.1%    parent-relative misses 216 → 115    (crosses the 0.75 bar)
```

**No engine layout code changed.**

### The fix: ONE constant decides it for BOTH engines

`chrome::REFERENCE_HIDES_SCROLLBARS` now selects the Chrome flag *and*, via
`chrome::match_reference_scrollbar_policy()`, puts our engine in the matching mode
(`manuk_layout::set_scrollbars_hidden`, an overlay-scrollbar switch that mirrors what
`--hide-scrollbars` is in Chrome: a process-wide UA metric, not a CSS property). They cannot drift,
because there is only one of them. Every command that compares against Chrome — `fidelity`, `parity`,
`interact`, `render --chrome` — calls it at entry; **the WPT suite deliberately does not**, because
WPT's expectations are written for a classic 15px scrollbar.

It stays `true`: hiding scrollbars is the honest choice for a *comparison* — it removes the
scrollbar's painted strip from the visual diff and takes a platform UA metric out of the geometry,
leaving the layout math, which is what the instrument is for.

### ⚠ The cohort is smaller than the hypothesis, and the measurement said so

The obvious reading — *"a corpus-wide 15px deficit"* — is wrong, and the sweep refuted it. Our engine
reserves the viewport gutter **only for the deterministic `overflow-y: scroll`**; the
`overflow: auto`-and-actually-overflows case is documented residue and reserves nothing. So only sites
that ship the `overflow-y: scroll` idiom were affected. Across the other band anchors
(`fragrantica`, `paypal`, `momon-ga`, `ta3lemkonline`, `razaoautomovel`) SHAPE moved by ≤0.1 points.

⚠ And that residue is now **unmeasurable by this instrument**, because with scrollbars hidden the
reference never reserves either. It needs its own WPT coverage and must not be read as absent because
the sweep is quiet.

### The gate

`G_REFERENCE_VIEWPORT_MATCHES` (`tests/wpt/src/chrome.rs`) launches the reference under the
instrument's own `base_flags` on an **overflowing** document and asserts its `clientWidth` equals
`vw − manuk_layout::scrollbar_gutter(auto)`.

⚠ It asserts **agreement, not a value**: flipping `REFERENCE_HIDES_SCROLLBARS` to `false` keeps it
green (both sides go to 1185), because the defect was never the policy — it was the policy applied to
one side. Proven RED by deleting the `match_reference_scrollbar_policy()` call, which is exactly the
state every sweep before t1319 ran in: `left: 1200.0, right: 1185.0`.

## The per-site error bar is not a constant — it ranges from 0.0 to 18.0 points (t1322)

The first complete 200-site sweep after the t1319 instrument repair threw up three apparent
regressions. All three were false, and the sequence that refuted them is the method:

```text
   serennu.com        sweep(--jobs 2) 49.2   →   serial re-run 73.8   (= its previous value)
   www.unoeste.br     five runs, same binary, same hour:
                          66.9 · 84.5 · 84.9 · 73.1 · 82.7      SPREAD 18.0 pts on 441–445 ids
   www.freesupertips  new 70.8  vs  OLD BINARY built and run in the same hour: 68.6
                          → the new code is BETTER; the drop was against a STALE baseline
```

⭐ And the control that makes the number mean something: **`oilprice.com` re-ran at 66.1 / 66.1 — a
spread of 0.0 on 654 ids.** The variance is a property of the SITE, not of the run or the machine.
Some pages are deterministic to the pixel; some swing eighteen points with nothing changed.

### What this changes

1. **A per-site claim needs the site's own error bar first** — at least two serial runs before any
   before/after is believed. A single reading on `unoeste.br` carries no information at all.
2. **Rank the worklist from the STABLE end.** A fix verified on a ±18pt site cannot be priced; the
   same fix on `oilprice.com` can.
3. ⚠ **`--jobs 2` is still contended.** t771 established that `--jobs 8` costs hard sites their
   scorability; this sweep shows `--jobs 2` costs a site 24 points of shape. A down-mover in a sweep
   is a **suspect**, never a regression, until a same-hour serial run has agreed with it.
4. ⚠ **And if it survives that, it still needs the OLD-BINARY control, same hour.** `freesupertips`
   looked like a −6.8 against the previous sweep and was a +2.2 against the previous *code* — the
   difference being five days of the live web moving underneath the measurement.

### ⚠ The tracker's own printed delta had a poisoned baseline

`scripts/fidelity-progress.sh` reported `IN-SCOPE PASS 18.7%→36.1% (+17.4 pts)`. Its comparator was
`t1275`, which is the **contended `--jobs 8` sweep** (scored 54 of 200, against 108 here). The honest
comparator is the last clean one, `t1268`: **34.6% → 36.1%, +1.5**. A trend line that includes a
known-poisoned point will hand you its recovery as progress.

## `nth-of-type` counts over ALL same-tag siblings, so one disagreed-about element takes a subtree out of the scored set (t1331)

The fidelity probe keys every element by a selector path — `tag.SIG:nth-of-type(n)/…` — where `SIG` is
an fnv hash of the element's sorted class list. ⚠⚠ **The `n` counts among ALL preceding siblings of the
same TAG, ignoring the sig.** So one element that only one engine creates shifts every later sibling
of that tag, and each shifted element's whole subtree keys differently and scores as MISSING.

`www.crazyshop.pl` is the arithmetic:

```text
    Chrome's <body> children:  17, of which  div.siiimpleToast × 2  are JS-injected notification toasts
    structural: oracle 1537 paths · ours 1505 paths · 135 "missing"
```

⭐ We have **32 fewer** paths and **135** are unmatched — so **at least 103 of the 135 are elements we
DO render, keyed differently**. The two toasts sit at the very top of `<body>`, so every later
`div:nth-of-type(n)` is off by two: `div.bottom-html`, Chrome's `div:nth-of-type(4)`, is our
`div:nth-of-type(2)`, and its 880px subtree drops out of the scored set with it.

That is **6.7% of this page's scorable elements**, and it presents as MISSING_BOX work — the one class
the burndown has repeatedly established cannot move the near-miss band.

### The fix, and its cost

Count `nth-of-type` among siblings sharing the same **(tag, sig)** — the sig is already in the key and
already decides matching, so this adds no new failure mode and removes the shift. ⚠ It changes the key
space, so **every banked sweep becomes incomparable** (t1242: an instrument you repair invalidates its
own earlier readings) and the corpus needs another re-baseline. Named rather than done in the tick
that found it.

⚠ It must be changed in **three places byte-identically** — the two probe scripts in `chrome.rs` and
`path_of` in `main.rs` — which that file's own comment already warns about: *"A path built two
different ways is two different keys, and the diff would then compare strangers."*

## The path key's counter and its comparison are COUPLED — the obvious fix collapses 26% of the paths (t1332)

t1331 measured that the sibling counter is sig-blind and that this costs real elements: on
`www.crazyshop.pl`, whose `<body>` opens with two JS-injected `div.siiimpleToast` toasts, at least 103
of the 135 "missing" boxes are elements we DO render, keyed differently. Check #125 made *"key
`nth-of-type` by (tag, sig)"* its first steer.

**It was tried, and the measurement refused it:**

```text
                           paths (oracle)   missing   SHAPE
    sig-blind counter            1537          135     68.2%     ← today
    sig-aware counter            1143           23     48.4%     ← 394 paths COLLAPSED
    oilprice.com                  667            1     85.0%  →   416 / 0 / 50.5%
```

⭐ **`strip_sigs` removes every `.SIG` from BOTH sides' keys before the comparison** — deliberately, so
a class the two engines disagree about does not unmatch an element. With a sig-AWARE counter,
stripping then maps distinct siblings onto the SAME key and they overwrite each other. The missing
count improves exactly as predicted and a quarter of the corpus's paths vanish with it.

**So the counter and the comparison are coupled.** Match sig-blind and count sig-blind (today), or
match with sigs and count with sigs (a different instrument, fragile to every class the engines
disagree about — which is precisely what `strip_sigs` exists to absorb). ⚠ The middle is incoherent,
and the middle is what a reading of the key alone suggests, which is why check #125's steer named it.

The shift is still real. The choice between the two coherent instruments is a corpus experiment —
how often does a class differ between engines, versus how often does an inserted sibling shift a
subtree — and neither number exists yet.

## `G_PATH_KEY_CONTRACT` — the "byte-identical contract" that had no checker

`chrome.rs`'s own header calls the three implementations of the path key a **byte-identical
contract**, warning that *"a path built two different ways is two different keys, and the diff would
then compare strangers."* Nothing checked it.

It now does: one fixture through Chrome's probe and through our own `path_of`, key sets compared.
Red-proven from **both live sides** — dropping the sig-awareness from `path_of` or from
`capture_seen_all_paths`'s probe makes the sets differ.

⚠ Two things this made possible and one it did not:

- `path_of`/`sig_of`/`strip_sigs` **moved from `main.rs` (the binary) into the library**, where the
  gate can reach them. They had lived in the binary, so no library gate could compare our walker to
  the reference's *without duplicating it* — which is the drift the contract exists to prevent.
- ⚠ It exercises `capture_seen_all_paths`'s probe and **not `PROBE_ALL_PATHS_JS`**: there are two JS
  copies and only one is under the contract.
- ⚠⚠ **The wall does not run it.** `verify.sh`'s `T · crate tests` list is
  `manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell` — `manuk-wpt` is
  invoked as a BINARY (parity, fidelity, hittest, bench) and its 107 unit tests never run, exactly as
  `manuk-js`'s 21 never run (t1330). Two crates, ~128 tests, including every gate that guards the
  fidelity instrument itself.

## A divergence between two TYPEFACES is not a layout divergence (t1369)

The G1 ledger keys every geometry divergence by which dimension is wrong and how wrong
(`oracle::signature_of`). That key answered *"what is a root cause?"* with the axis — and on
`www.a11yproject.com`, the burndown's worst named anchor, it therefore reported that our **width
computation** was the site's top five causes (44 of 132 divergences, `<a>` and `<li>`). It was not.
Both sides already carry the used-face signature `{family/px/advance}` that t563 added for exactly
this question, and they disagreed:

```text
  <a>  [494 4617 182×22] {anaheim/20/201}  vs  [487 4527 167×26] {anaheim/20/181}
```

### The arbitration is the FONT FILE, not either engine

Neither renderer is the authority on what a face measures — the file is. `Anaheim-Regular.woff2`
has `unitsPerEm` 2048; the twenty glyphs of the probe string `Hamburgefonstiv 0123` sum to 18540
units (**181.055px at 20px**), and its `normal` line-height is **25.78px** from all three of its
metrics tables (hhea, OS/2 typo, OS/2 win — they agree). Manuk reports 181 and 26.

Chrome reports the same 181.06 × 26.00 **when it can fetch the font**, and keeps agreeing across
every control: the bare `url()`, the `local(), url()` idiom the site actually uses, a `local()` that
does not resolve, weights 400–900 against a 400-only face (no synthetic-bold gap), and local-face
controls (`DejaVu Sans` 229.89/230, `sans-serif` 201.23/201).

**201 is Chrome's own fallback sans-serif advance.** One oracle run reports `{anaheim/20/181}` for
twelve elements and `{anaheim/20/201}` for eleven *on a single page load* — the reference
disagreeing with itself, which no engine change can answer.

⚠⚠⚠ **NEVER COMPARE FONT METRICS FROM A `file://` FIXTURE.** A cross-origin webfont is not fetched
for a `file://` document, so Chrome silently renders the fallback: `anaheim` measured 184.61 there —
byte-identical to `NoSuchFontXYZ`, to `serif`, and to `Liberation Serif`, with
`document.fonts.check('20px anaheim')` returning **false**. Two readings of a real bug were
manufactured that way before the control table caught it. Serve the fixture over a real HTTP origin
(`python3 -m http.server`) and assert `document.fonts.check(...)` in the probe.

### What changed, and what deliberately did not

`signature_of` now checks the two `{...}` suffixes first: both present and DIFFERENT ⇒ the cause is
keyed `font-resolution: <chrome> vs <ours>`. The cluster is still counted, ranked, and carries its
median and instances — it is named for its cause, not hidden.

```text
  distinct causes   56 → 40      MEAN SHAPE   43.3% → 43.3%   ← unchanged, on purpose
  top cause         geometry/mis-sized: width ~16px  →  font-resolution: …/201 vs …/181
  top LAYOUT cause  (buried at #4)                   →  missing box: <path>   9 hits
```

**A reclassification that moved the score would be a trade.** Only the diagnosis was wrong; the
anchor is still 43.3% and still failing. What changed is which defect the ledger sends the next tick
to fix.

### ⚠ The family name is a LABEL; only the advance is a MEASUREMENT (t1370)

t1369 compared whole signatures, and the very next page the instrument met produced **43 false
positives**: divergences keyed `font-resolution: Times New Roman/16/148 vs serif/16/148` — the same
148px advance at the same 16px size, differing only in what the two engines *call* the fallback they
both resolved to. That name is `getComputedStyle().fontFamily`'s first entry, and two engines naming
one generic fallback differently is not a divergence at all; it hid six real `geometry/mis-sized`
and `geometry/displaced` causes behind a font label.

The comparison is now the pair the probe actually MEASURES — `(px, advance)`, the two trailing
components. `{anaheim/20/201}` vs `{anaheim/20/181}` still separates (same name, different advance),
which is the case the classification exists for; `{Times New Roman/16/148}` vs `{serif/16/148}` no
longer does. The SIZE half is load-bearing too: dropping it would merge two sizes of one face.

⭐ **The general rule, and it is why this was found in one run:** *when a signature is part label and
part measurement, key on the measured part.* A label is what an engine chose to call something and
two engines may legitimately choose different words; a measurement is a claim about the world and
they may not.

### ⚠ The advance is an INTEGER, so ±1 is the instrument (t1373)

t1370 fixed comparing the family NAME; the remaining half was comparing the measured advance
EXACTLY. The probe reports `Math.round(ctx.measureText(PROBE).width)` on both sides, so two readings
of ONE face legitimately differ by a pixel — and that pixel became a cause. On `www.naukri.com` the
ledger keyed **`font-resolution: Inter/14/150 vs Inter/14/149`** over a divergence whose **median was
1203px**. A 1203px displacement is not explained by a typeface the two engines agree about to within
a rounding unit; the label was hiding a real geometry defect behind a font.

The comparison now allows ±1 on the advance. It cannot hide a real face difference: two faces one
pixel apart over the 20-character probe are sub-pixel PER CHARACTER, far below the 8px tolerance the
diff already uses, so they could not be the cause of a divergence at all. The case the
classification exists for is untouched — `anaheim/20/201` against `anaheim/20/181` is 20px apart.
**The SIZE still matches exactly**: it is a declared value, not a measured one, so it carries no
quantisation, and two sizes of one family are two different used faces.

⭐ **The rule this completes:** *a threshold on a measured quantity must be at least the
quantisation of the instrument that measured it.* t1369 keyed on a signature that was part label and
part measurement; t1370 dropped the label; t1373 gives the measurement the tolerance its own
rounding forces. Each pass narrowed the classification and each was found by running the instrument
on one more page.

Two guards, both mutation-proven: an **absent** signature must not compare unequal to a present one
(`fontsuffix` emits absence rather than a fabricated `{/0}`, and a row that says nothing about its
face stays a geometry cause), and the key must survive `div_to_jsonl` → `div_from_jsonl`, because
the cause lives in the instance strings and a serialisation boundary is a semantic one (t743).

## Freeze the page, and price the DIVERGENCE not the CONSTRUCT (t1380)

Two method rules, both bought by watching t1379's numbers be wrong in opposite directions on the same
afternoon.

### 1. A LIVE PAGE IS NOT AN INSTRUMENT FOR ANYTHING SMALLER THAN ITS OWN CHURN

`manuk-wpt fidelity --urls https://…` re-fetches. A news homepage changes between two fetches, and
the scored denominator moves with it — `morikoshi.net` returns **1032 / 1039 / 1032** scored across
three runs of the SAME binary. Any delta read across two such fetches is a reading of the news.

How wrong it can look: diffed live-against-live, `www.alphanews.live` reported **52 elements fixed and
62 broken**, including `m[30 130 608x0]` against Chrome's `608x461` — an entire subtree collapsed to
zero height, indistinguishable from the regression that would force a revert. Frozen, the same diff is
**0 fixed, 0 broken** and the two shape numbers agree to the digit.

**The fix costs nothing.** Freeze the page and hand both engines the same bytes:

```bash
curl -sL -A 'Mozilla/5.0 …' https://site/ -o raw.html
# insert <base href="https://site/"> right after <head> so subresources still resolve
python3 -c "..."            # writes snap.html
(cd /tmp/fx && python3 -m http.server 8791 &)
manuk-wpt fidelity --urls 'http://127.0.0.1:8791/snap.html' --shape-dump 4000
```

Both the Chrome reference and our engine then read one immutable document, so the denominator is
pinned and a 0.5-point delta becomes readable. ⚠ The `<base>` tag is load-bearing — without it the
page's own relative CSS does not resolve and you measure an unstyled document (t1367's trap, one layer
out).

**Diff the MISS PATHS, not the counts.** `--shape-dump N` prints one line per miss ending in the
element's selector path; `comm -23 old.paths new.paths` is *fixed* and `comm -13` is *broken*. A count
that stays the same can hide N fixed and N broken; the path sets cannot.

### 2. PRICE THE DIVERGENCE, NOT THE CONSTRUCT

t1379 priced a layout fix by asking **Chrome** which pages CONTAIN the construct — boxes with a
computed `aspect-ratio` whose child's height fills them — and got 14 of 117 sites, 12%, with 122
instances on one page and 69 on another. The fix then changed **not one box** on either of those two.

**Where a construct EXISTS says nothing about where OUR engine gets it WRONG.** Chrome having a
ratio box with a filling child is compatible with our engine already sizing that box correctly (the
ratio often arrives from an `<img>`'s natural size, which was always definite). A probe that runs
only in the reference measures the WEB; a price has to measure the GAP.

The honest form of the question is *"on how many pages do the two engines DISAGREE about this box"* —
which the oracle already answers: it is a shape-dump miss whose delta has the mechanism's signature.
Price from the diff, not from the reference.

## A SITE WHOSE REFERENCE RENDERS UNSTYLED IS CHARGED TO THE ENGINE, AND IT IS 4.1% OF THE CORPUS (t1344)

t858 recorded *"the one established instance is `trivago.be`: five `<link rel=stylesheet>`, **zero**
loaded by the oracle — one site, named, not a rate."* On the CrUX corpus it is **five rows**
(fr/be/pl/jp/de — one codebase, five locales), **4.1% of every scored row and 6,665 sampled ids**, all
sitting at `shape 0.11` with `coverage 0.966`.

⚠⚠⚠ **THAT PAIR IS INDISTINGUISHABLE FROM THE LAYOUT WORK THE BURNDOWN RANKS**: every box drawn,
nearly every one in the wrong place. What separates them is already printed on every run and nothing
read it:

```text
   365 hit(s)  display: inline → block                                  (<a>)   inline vs block
   365 hit(s)  font-resolution: Times New Roman/16/148 vs -apple-system/16/172  (<li>)
```

`Times New Roman` + `display: inline` on 365 anchors is a document with **no CSS applied at all**;
`-apple-system` + `block` is the page's own stylesheet, and that side is **ours**. ⭐ **When the
reference is unstyled and we are not, the score is upside down** — the engine that is RIGHT takes the
0.11. Read the `font-resolution` line before pricing any low-shape/high-coverage site.

### Four causes eliminated, so the next probe is a bisect and not a guess

```text
   the oracle's exact document (curl + spliced <base>), file://        sheets=0
   …the same, with EVERY <script> stripped                             sheets=0
   …the same, served over http://127.0.0.1 (a real origin)             sheets=0
   …the same, with --allow-file-access-from-files                      sheets=0
   a ONE-LINK control on the SAME href, same base, same flags          sheets=1   <- loads fine
```

Not the `<base>` splice (`base.href` is exactly the origin, parent `HEAD`, all 7 links resolve to real
absolute https URLs), not the `file://` origin, not scripts, not the CSS (**200 `text/css`, 36 KB** to
any UA, with or without a `Referer`). Every link has `sheet === null` in a `<head>` of **103
children**, while the identical fetch in isolation succeeds. NEXT: keep the 7 stylesheet links, delete
the other ~96 head children; if they load, the cause is the request BURST.

## THE SWEEP RUNNER MUST RE-SPAWN — `fidelity` EXITS ITS OWN PROCESS ON PURPOSE (t1344)

After a per-site timeout the tool prints *"EXITING THIS PROCESS DELIBERATELY — the row above is on
disk and **the parent re-spawns the remainder**"* and calls `exit`, skipping `JS_ShutDown()`. The
SpiderMonkey teardown message and `exit 139` that follow are **not** a crash; the tool says so itself.

⚠ A chunk runner that invokes `fidelity --urls-file <chunk>` **once** therefore stops at the first
slow site in that chunk and looks like it finished. Two attempts returned **123/200** and **157/200**
and both reported success. The runner must loop:

```sh
for attempt in $(seq 10); do
  # remaining = chunk URLs whose host has no row yet in the --rows-out file
  awk -F'\t' 'NR==FNR{if($0!~/^#/)done[$1]=1;next}
              {u=$0;sub(/^https?:\/\//,"",u);sub(/\/.*$/,"",u); if(!(u in done)) print}' "$c.tsv" "$c" > "$c.todo"
  [ -s "$c.todo" ] || break
  ./target/release/manuk-wpt fidelity --urls-file "$c.todo" --rows-out "$c.tsv"
done
```

⚠⚠ **A SWEEP AND A BUILD MUST NOT SHARE A WALL CLOCK.** The first attempt was invalid for a second,
worse reason: `manuk-wpt` was rebuilt while it ran, so its later chunks measured a different binary
from its earlier ones — an instrument that changes mid-reading, which is t1242's rule in its most
literal form. `-P 4` also survives where `-P 8` sheds chunks.
