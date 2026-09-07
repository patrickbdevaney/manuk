# The base URL was the disagreement

t1476 found two of our own instruments giving different answers about our own layout of one document,
and recorded it as unsettled because a SHAPE column two instruments cannot agree on cannot direct
work. Constitution check #142 made settling it steer #1.

It was the base URL.

```text
  fetch_html("https://europa.eu/")  →  final_url = https://european-union.europa.eu/select-language?…

  probe with base "https://europa.eu/"        ul [d=block 1184x36]   ← looked like Chrome's number
  probe with base = the REAL final_url        ul [d=block  200x48]   ← matches fidelity exactly
  fidelity                                    ul [17 32    200x48]
```

**`fidelity` was right.** It uses the post-redirect `final_url` that `fetch_html` returns; my probe
passed the pre-redirect URL as the base, so every relative subresource resolved against the wrong
origin and the page laid out differently. Same bytes, different document.

⭐ **A redirect makes "the URL" two different values, and only one of them is the base.** Any probe
that fetches with one and lays out with the other is measuring a page that does not exist. `curl -L`
hides this by design: it follows the chain and hands back the final *body* while you keep holding the
original *URL*.

## What it re-validates

The whole SHAPE ranking stands, including the row it was found on:

```text
  europa.eu             cov 99.2   SHAPE  0.0
  developer.mozilla.org cov 67.8   SHAPE 35.6
  discuss.python.org    cov 100.0  SHAPE 36.6
```

## And the thing it turned up on the way

Chasing europa.eu's zero, `document.styleSheets` read `0` — which looked like "no CSS loaded" and
would have explained SHAPE 0.0 entirely. It does not. Measured on a fixture where the sheet demonstrably
applies:

```text
                                   Chrome                    Manuk
  <style> + <link>, both loading    sheets=2 [STYLE,LINK]     sheets=1 [STYLE]
  the linked rule's effect          color: rgb(1,2,3)         color: rgb(1,2,3)   ← identical
  link.sheet                        object                    undefined
```

⭐⭐ **The CSS loads and applies; only the CSSOM view of it is missing.** `document.styleSheets` is
built by scanning `getElementsByTagName('style')`, so every external `<link rel=stylesheet>` is
invisible to it, and `<link>.sheet` is `undefined`. Every theme switcher, CSS-in-JS runtime and
`sheet.disabled` toggler iterates that list and sees only the inline sheets.

⚠ **And the `sheets=0` reading proved nothing until it was controlled.** A count that is zero because
the *view* is empty looks exactly like a count that is zero because the *thing* is absent. One
fixture where the sheet visibly changes a colour separates them — and without it this tick would have
"explained" SHAPE 0.0 with a stylesheet-loading failure that does not exist.

See also [[placement-is-the-weak-axis-ranked]], [[cssom-views-and-terminators]].
