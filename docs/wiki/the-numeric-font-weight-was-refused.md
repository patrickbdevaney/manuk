# The numeric font weight was refused

*Tick 1492. Chrome-verified on the fixture, −92 on the suite. Reverted, with the measurement.*

## What was built

`manuk_text::FontKey` carries `bold: bool` at a 600 threshold, so 300/400/500 collapse onto one face
and 600/700/800/900 onto another. t1491 priced the change and justified it: **285 of 584
font-disagreeing near-bar shape misses sit at a weight the boolean collapses.**

The change replaced the boolean with `weight: u16` across **30 construction sites in 9 files**, handed
the real number to `fontdb::Query` (which implements CSS Fonts §5.2 itself), and gave the
`@font-face` branch its own §5.2 comparator.

**It worked, and it was Chrome-byte-identical on both fixtures.**

```text
  system family, `Lato` 32px      300   400   500   700   900
    Chrome                        303   312   314   320   327
    before                        312   312   312   320   320
    after                         303   312   314   320   327

  a graded @font-face set         400   500   700   450
    Chrome                        312   314   320   314
    after                         312   314   320   314     (450 is §5.2's directional tie)
```

## Why it was refused

```text
  wpt css/css-fonts/variations
    clean tree, two runs      237/468   242/468      (a ±5 band)
    with the change           147/468   148/468      -92, far outside it
```

**A regression is never traded for a capability.** Reverted.

## What the refusal established, which a green tick would not have

⚠ **Reverting only the `@font-face` branch did not recover it** — `variations` stayed at 148. So the
cause is the SYSTEM path, `fontdb::Weight(key.weight)`, which is precisely the half that was
Chrome-verified. Two oracles disagree about one change, and *when two instruments disagree about one
number, neither is evidence* (constitution check #142).

⚠ **The `@font-face` branch was independently wrong, and that IS established.** Its first version used
CSS Fonts §5.2's closest-match over each registered face's weight, and the failing assertions named the
error exactly:

```text
  Test @font-face matching for weight 420
    @font-face should be mapped to CSSTest Weights 600 — got the 300 face
```

§5.2 matches against the **`@font-face` DESCRIPTOR's** declared weight or range, **not** the weight
inside the font file. `manuk_css::FontFace` carries `family`, `srcs` and `unicode_range` and **no
weight at all**, so this map has only ever known the file's. *A closest-match over the wrong number is
more confidently wrong than a coarse match over it.*

⚠ **And the CSSTest weight faces are not installed on this machine** (`fc-list | grep -c CSSTest` is
0) — the tests load them from `./resources/…`, through the very `@font-face` path whose descriptor is
missing. So the suite may be measuring the descriptor gap rather than the weight key. **Not
established**, and it is the first thing the next attempt must settle.

## The three-step next attempt

1. **Carry the descriptor.** Parse `font-weight` in the `@font-face` block into
   `manuk_css::FontFace`, thread it through `register_named_font`, and key the webfont map on
   `(id, lo, hi)`. This is a prerequisite for §5.2 being implementable at all on that path, and it is
   worth doing on its own evidence.
2. **Re-measure `variations` with the descriptor present**, on the clean tree. That decides whether
   the −92 is the weight key or the missing descriptor, and nothing else can.
3. **Only then re-attempt the numeric key**, with `variations` as the gate rather than the casualty.

*A revert with a full measurement is not a lost tick* (t1420).
