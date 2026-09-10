# Both sides must follow the same redirect

*Tick 1487. Three sites went from unscored to scored, and one of them clears the render bar.*

## What t1486 left behind

t1486 taught the **engine** to follow `<meta http-equiv="refresh">`. It deliberately left the
**instrument** alone, and named that as residue: `manuk-wpt` fetches a document with `curl -sL`,
which follows `Location:` and knows nothing about `<meta>`, then splices in a probe and hands the
result to Chrome — **which does follow the refresh**, navigating to the destination and taking the
probe's output with the document it left.

That produced two separate wrongs:

* the row was filed `probe-blocked`, whose stated cause was a CSP that does not exist;
* and once the fetch learned to follow the redirect, **Chrome scored the destination while our
  engine still scored the stub** — the two sides diffing different documents, which is the exact
  asymmetry `Unmeasurable::CssStarved`'s own rule forbids: *refuse when the comparison is ASYMMETRIC.*

## The result

```text
                                        before          after
  www.datacareservices.com              probe-blocked   structural 98.7%   SHAPE 76.9%   ← clears 0.75
  secure.paymentech.com                 probe-blocked   structural 93.7%   SHAPE  3.0%
  linxonline.co.pierce.wa.us            probe-blocked   structural 100.0%  SHAPE 45.2%
```

Three sites converted from unscored to scored on the in-scope corpus, and the denominator moved for
the right reason — not because a hard site was dropped, but because a site that was always
measurable finally is.

## One parser, not two

The grammar is `manuk_page::parse_meta_refresh` and the URL join is `manuk_page::resolve_url` — the
same functions the engine navigates by, and the same ones `g_a_meta_refresh_is_a_redirect` pins
against headless Chrome. A second copy in the instrument is how the instrument and the engine come to
disagree about what page they are looking at, which is the class of bug this whole thread has been
about.

Both followers carry the same three guards, for the reason the shell's own comment states: a
declarative refresh is the easiest infinite loop on the web and nothing about it looks like one. Hop
bound; self-target refusal; delay floor.

## And `probe-blocked` now reports what was SEEN

The old text asserted *"the document loaded but its own Content-Security-Policy blocked the injected
probe"*. Zero of the eight sites carrying that tag have a CSP. The replacement is a pure function
over the dump with three distinguishable observations and **no diagnosis**:

```text
  ww1.goojara.to           the probe element is ABSENT from the dump
  venus.zeronline.cloud    the probe element is ABSENT from the dump
  xhdesign.today           Chrome returned almost no document (145 bytes)
  swiftspinus.com          Chrome returned almost no document (0 bytes)
```

**`probe-blocked` is at least three different things and none of them is CSP.** `swiftspinus.com`
returns *zero bytes* from Chrome — that is not a blocked probe, it is a browser that produced nothing,
and it had been sitting under a label that said otherwise.

The unit test's vacuity arm is that none of the three observations may contain the string
`content-security-policy`: the whole finding was that the old text named a mechanism it had never
checked, and nothing else stops it drifting back.

## Gated

`chrome::meta_refresh_and_probe_absence_tests` — resolution against the document's own directory,
case-insensitive `http-equiv`, the delay reported rather than swallowed, and four refusals including
**a CSP `<meta>` must not be read as a redirect** (CSP is the only `http-equiv` this engine used to
handle, so confusing the two is the available mistake). Red under four mutations.
