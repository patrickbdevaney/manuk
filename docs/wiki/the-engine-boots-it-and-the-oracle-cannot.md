# The engine boots it and the oracle cannot

*Tick 1502. Three explanations refuted, and the striking fact they leave behind.*

## The fact

```text
  comix.to
    our engine     BOOT: clean · 2,610 boxes
    Chrome, live   ~1,258 divs
    Chrome, proxied      4 tags     <- the reference we score against
    the row        oracle-module-shell — UNSCORED, counted against the bar
```

**Our engine renders this page and the reference cannot be built.** The row is counted as a refusal —
correctly, under the fixed-denominator rule — but it is a ceiling of the *measurement method*, not of
the browser. `allticketscol.com` is the same shape: `BOOT: clean` here, 168 proxied tags against a live
page carrying 245 divs.

That is worth stating plainly, because every reading of `METHOD` since t1485 has treated these rows as
work waiting to be done. For at least two of them the browser's half is already done.

## Three explanations, all refuted

**1. The app's runtime `fetch`es use ABSOLUTE same-origin URLs**, which the proxy cannot rewrite
because they live in JS and not in markup. This is the mechanism the instrument's own t880 note
predicts (*"the booted app's own same-origin fetch() is still cross-origin"*).

```text
  comix.to's entry bundle, 199 KB:  occurrences of `https://comix.to`  ->  0
                                    location.origin ×3 · location.hostname ×1
```

**Refuted.** The bundle builds its URLs from `location.origin`, which under the proxy *is* the proxy,
and which the proxy forwards upstream.

**2. `upgrade-insecure-requests`**, which would rewrite every proxied `http://127.0.0.1` subresource
back to `https` and send it to the live origin. **Refuted** — absent from the document and the headers
on all three sites.

**3. A page-supplied `<base href>`** pointing at the live origin, which would defeat one origin
entirely and which `rewrite_document` does not touch (it strips `<meta>` CSP and nothing else).
`allticketscol.com` does carry one:

```text
  <base href="/">
```

**Refuted** — it is *relative*, so under the proxy it resolves to the proxy's own root. It is the one
shape of `<base>` that is harmless.

## What that leaves

Nothing cheap. The remaining candidates all require running the **proxied** document in Chrome and
reading what it says, and **no instrument does that today**: `one_origin_reference` invokes Chrome with
`--dump-dom` and compares open-tag counts, so a proxied app that throws on line one is indistinguishable
from one that renders four tags on purpose.

⚠ The next step is therefore an instrument, not a fix: **capture the proxied render's console.**
`report_probe_absence` (t1487) is the precedent — it replaced a guessed cause with three
distinguishable observations, and the same move is available here for a strictly smaller cost than
another round of hypotheses.

⚠ And the honest framing for the ledger: **two of these rows are unmeasurable rather than unrendered.**
The fixed-denominator rule says they still count against the bar, and it is right — but a backlog that
ranks them as engine work is pointing at the wrong half.
