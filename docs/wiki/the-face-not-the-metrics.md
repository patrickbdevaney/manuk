# The face, not the metrics

*Tick 1489. Two hypotheses refused, and the second only became visible because an inert mutation
forced the instrument to be fixed first.*

## The survey

t1488 narrowed the near-bar shape gap to *sizing inside agreed layout modes*. `Seen::font` is
`family/px/advance`, already carried and already discarded by `--shape-dump`. Printing it:

```text
  562 of 1,170 near-bar shape misses — 48% — are on elements where the FAMILY and the SIZE
  agree and the measured ADVANCE does not:

      payb.jp         Noto Sans JP/16   chrome 168  ours 158     ours 6.0% narrower
      puentedemando   Google Sans/16    chrome 168  ours 161     ours 4.2% narrower
      pivaldi         Open Sans/14      chrome 141  ours 149     ours 5.7% WIDER
      serennu         (no disagreement at all — 0 of 16 misses)
```

The two advances are **independent measurements of one probe string**: Chrome's `measureText` on the
element's resolved font against this engine's own `fonts.measure`. Not a plumbing artefact.

## Hypothesis 1: our text metrics are wrong — REFUTED

One page, one Google Fonts webfont, one system stack, one monospace:

```text
  ours    w=155  s=142  m=144
  Chrome  w=154  s=142  m=144
```

**Our metrics match Chrome to one pixel, including on a webfont that loads.** So the divergence is
*which face each engine ends up with*, not how we measure it.

## Hypothesis 2: the webfonts do not arrive — REFUTED, and the refutation needed a fixed instrument

A declared `@font-face` family **shadows** a locally-installed one of the same name — CSS Fonts' rule,
implemented here on purpose (t559/t560: declaring only on success let a failed download be masked by a
same-named local face and cost 19 SHAPE points on `martinfowler.com`). So a failed webfont is *silent
by construction*: the page renders in a fallback, still reporting the family it asked for.

`Page::webfonts()` was built to count it, and the first reading was **`pivaldi: 20 of 122`** — which
looked like the answer.

⚠⚠ **IT WAS AN ARTEFACT OF MY OWN COUNTER, AND AN INERT MUTATION IS WHAT EXPOSED IT.** Mutation 3
(`assign` vs `accumulate`) did not go red, because a single-round fixture cannot tell them apart.
Asking why showed that **neither was right**: `fetch_and_apply_stylesheets` re-visits every
`@font-face` block on later style rounds, `claim_webfont_src` short-circuits the FETCH but not the
declaration, so a `+=` counter inflates the denominator once per round while the numerator cannot
follow. Keyed per face (`family` + first `src`), idempotent across rounds:

```text
  payb.jp          14 of 15        SHAPE MISS FONT 420 of 432
  puentedemando    16 of 21        266 of 290
  pivaldi          20 of 20        50 of 97          <- "20 of 122" was the bug
  restaurantguru    7 of 7         16 of 168
  razaoautomovel   11 of 12        13 of 338
```

**Delivery is near-perfect and the advance still disagrees.** Had the counter shipped uncorrected,
this tick would have concluded that webfont loading was the problem — and it is not.

*A mutation that does not go red is a question about the fixture* (t1424, t1239), and here the answer
was a defect in the thing being tested.

## What is left, and it is stated as untested

The faces load, our metrics are right, and real sites still diverge 4–6% in **both** directions. The
strongest remaining candidate is **which face within the family**: `manuk_text::FontKey` carries
`bold: bool` at a 600 threshold, so weights 300/400/500 collapse to one face and 600/700/800 to
another — while Google Fonts ships a separate file per weight and Chrome picks the real one. A page
set in weight 500 would get our 400 face and Chrome's 500.

⚠ **Not confirmed here.** The obvious fixture is inconclusive: Chrome's own per-weight advances
bucketed at exactly 300/400/500=151 and 600/700/800=156, which is the *fallback* bucketing and means
Chrome measured before the font settled. Confirming it needs a fixture that proves the face is live at
measure time — a `document.fonts.ready` await, or a local `@font-face` per weight served from disk.

## Landed

```
  tests/wpt/main.rs      --shape-dump prints the DISPLAY and FONT pair per miss, plus a tally of each
  engine/page/lib.rs     Page::webfonts() — declared vs delivered faces, keyed per face
  gate  g_a_declared_webfont_that_never_arrives_is_counted   RED under 3 mutations, including the
        second-style-round arm that the inert mutation demanded
```
