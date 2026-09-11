# A `@font-face` declares its own weight

*Tick 1493. Step 1 of t1492's refusal plan, and it moves the suite that refused it.*

## The number §5.2 matches against was never carried

CSS Fonts §5.2 matches a requested `font-weight` against the **`@font-face` block's own `font-weight`
descriptor** — not against the weight recorded inside the font file. A block may legitimately declare
`font-weight: 600` for a file whose internal weight is 400; subsetters, self-hosting pipelines and
icon fonts all do it, and the **declaration** is what a page asking for 600 must get.

`manuk_css::FontFace` carried `family`, `srcs` and `unicode_range`, and **no weight at all**. The face
registry had only ever known the file's.

## Why it is its own tick

t1492 built §5.2's closest-match over each registered face's *file* weight and **regressed
`wpt css/css-fonts/variations` by 92 subtests**. The failing assertions named the error exactly:

```text
  Test @font-face matching for weight 420
    @font-face should be mapped to CSSTest Weights 600 — got the 300 face
```

*A closest-match over the wrong number is more confidently wrong than a coarse match over it.* That
tick was refused and reverted whole, with a three-step plan; this is step 1, and the matching rule
here is deliberately **unchanged** — still the coarse bold/not-bold test the boolean key can express.
Only its INPUT moves.

```text
  wpt css/css-fonts/variations
    clean tree, two runs      237/468   242/468
    with the descriptor       247/468   247/468
  wpt css/css-fonts (area)    4092/7552 — exactly its banked mark, HANG/CRASH 0
```

⚠ The sub-area is up and stable across two runs; the AREA total is flat at its mark, so the gain sits
inside the area's own noise. Both are reported rather than the flattering one.

## The fixture inverts the two, which is the only way to tell them apart

Two blocks in one family: the one declaring **400** points at the **BOLD** file, and the one declaring
**700** points at the **REGULAR** file. Lato-Regular measures 312 for the probe string, Lato-Bold 320:

```text
  Chrome    a(400)=320   b(700)=312      <- the DECLARATION wins, inverted from the files
  before    a(400)=312   b(700)=320      <- the FILE wins
```

A fixture whose declarations agreed with its files could not distinguish the two at all.

## Three details the gate pins

⚠ **`None` means "the block did not say"**, and falls back to the file's weight — the old behaviour
exactly. The change is confined to blocks that *do* declare, so no page that was right becomes wrong.
"Did not say" and "said 400" are different facts and only the first may fall back.

⚠ **A reversed range is INVALID, not swapped.** `font-weight: 900 100` yields `None`, because the spec
says so — which is what `css-fonts/variations`'s `font-descriptor-range-reversed` tests are about.
Silently swapping would pass them for the wrong reason.

⚠ **`normal` is 400 and `bold` is 700 here too.** They are the same keywords the property takes and a
sheet that writes them in the descriptor is not rare.

## Where this leaves the plan

Step 2 of t1492's plan asked whether the −92 was the weight key or the missing descriptor. **It was
the key**: with the descriptor present and the coarse rule unchanged, `variations` is *above* the
clean-tree band. Step 3 — re-attempting the numeric key — now has a prerequisite in place and
`variations` as a gate rather than a casualty.
