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
