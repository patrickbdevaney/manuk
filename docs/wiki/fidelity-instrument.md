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

## A gate that builds its own inputs proves the function, not the wiring (t919)

t912 split the ranker's #1 cause — `missing box: <div>`, 37 sites and 2,398 hits — into `missing`
(our map is smaller) and `unaligned` (it is not), on the strength of t911's measurement that **22 of
58 sites render as many or more box-bearing paths than Chrome**. It was unit-gated in both
directions and RED-proven by pinning the comparison to `false`.

**On the next sweep, `unaligned` fired zero times in 200 sites.** The `missing box` total did not
move: 2,398 hits then, 2,389 now.

It should have fired on **26** sites, by the sweep's own printed numbers.
`compare_structure_detail` reports `probed = chrome.len()` and `mboxes.len() = manuk.len()` — the
same two quantities `diff_page` compares — and 26 sites print `ours >= oracle` with a non-zero
missing count (`naukri.com` 437 against 57; `chat.google.com` 2005 against 2004).

> **The gate constructs its own maps and passes; the sweep runs the same function on real ones and
> the branch is never taken.** A gate that builds its own inputs proves the function, not the wiring.

Third occurrence of the shape in one run: t782's correction reached the reason string and not the
ranker; t913 located the `vertical-align` defect in the consumer branch when the producer was
hard-coded; and now this. **Instrument the call site, do not infer it from a neighbouring line.**

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
