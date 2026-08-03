# THE FIDELITY INSTRUMENT — what it can and cannot see

## `shell-only-N` is the oracle rendering ONE CURL'd FILE, so every RELATIVE bundle 404s (t856)

`shell-only-N` says **the ORACLE built fewer than `CERT_MIN_SHAPE_SAMPLE` elements** — it is a claim
about Chrome's count, never ours. The board's #1 (the scorability ceiling) names it as part of
*"29 of 130 in-scope sites do not render at all"*, so what it actually measures decides whether those
are engine ticks or nothing at all.

**The oracle's document is one `curl` of the URL** (`chrome::fetch_document`), written to
`/tmp/manuk-fetch-*.body` and rendered by Chrome from `file://`. Measured across the whole 12-site
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

⚠⚠⚠ **THE MECHANISM, AND IT IS NOT THE ORIGIN.** From `file:///tmp/x.html`, a relative
`src="main-5UYZQ2ZL.js"` (allticketscol, Angular) resolves to `file:///tmp/main-5UYZQ2ZL.js` and a
root-relative `src="/esaj/_next/static/chunks/…"` (esaj, Next.js) to `file:///esaj/…`. **Both 404, so
the bundle never runs.** Absolute URLs still load — `vk.com` goes `0 → 215` on exactly that, which is
why the shortfall varies per site instead of being uniform, and is the tell that separates this cause
from a blanket scheme restriction.

⚠⚠⚠ **TWO COMMENTS IN THIS ONE FILE ASSERTED CONTRADICTORY CAUSES, AND THE TRUE CAUSE WAS A THIRD
THING NEITHER TESTED.** `unscoreable_reason` said *"its `file://` copy has a `null` origin, so a
JS-rendered page never builds"*. `Unmeasurable::ShellOnly`'s own docs, 1,300 lines above, record t674
**killing that exact claim** by serving the identical document over `http://127.0.0.1` and getting a
byte-identical dump. t674's experiment was sound; its conclusion was over-broad — **serving the same
single file over localhost 404s `/esaj/_next/…` just as hard**, so it could not distinguish *"the
origin blocks the fetch"* from *"the files are not there."* A refuted cause left standing in a second
comment is worse than no comment: it is a wrong answer with a citation.

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

### The ranked fix, and its trade — stated, not taken

Give the saved document a **`<base href="ORIGINAL_URL">`** before handing it to Chrome. Relative and
root-relative subresources then resolve against the real origin over https, the document itself stays
a byte-stable local file, and ten sites become visible to the instrument.

⚠ **The trade is real and belongs to whoever sets the instrument's contract:** the oracle's render
would depend on live subresources, so *"the document snapshot is cached, so three repeats are three
renders of the same bytes"* (`fidelity.rs:1263`, asserted by a determinism test) weakens to *the same
HTML with whatever the CDN served this minute*. That is a change to what the certificate MEANS, not
just to what it can reach, so it is named here and not smuggled into a measurement tick.

⚠ **Until it lands, do not spend throw-killer ticks on this cohort.** Ten of the twelve have no
engine defect visible in this data at all — the pages render in Chrome and our own score is
`coverage 1.000000` against a one-element reference. Working them would be optimising against an
artefact, which is the failure this loop has now been caught by four times.
