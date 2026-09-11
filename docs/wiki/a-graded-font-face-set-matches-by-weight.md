# A graded `@font-face` set matches by weight

*Tick 1494. Third attempt, and the two refusals before it are why this one is shaped the way it is.*

## The gap

`manuk_text::FontKey` carried `bold: bool` at a 600 threshold, so a family shipping one file per
weight — which is how Google Fonts and every self-hosting pipeline deliver — could answer with at
most two of them. A `@font-face` set of 300/400/500/600/700 is the ordinary case, not a corner.

```text
  @font-face: 400 -> Lato-Regular · 500 -> Lato-Medium · 700 -> Lato-Bold

           400    500    700    450
  Chrome   312    314    320    314
  before   312    312    320    312
  after    312    314    320    314
```

⚠ **`450` is the row that separates §5.2 from "nearest number".** It is equidistant from 400 and 500,
and the spec is directional: inside 400–500, look UP to 500 first. Chrome takes the 500 face; a
distance metric ties and takes whichever came first.

## Three attempts, and the shape of this one is the record of the first two

* **t1492** replaced the boolean and ran §5.2 over each face's **FILE** weight. Chrome-byte-identical
  on two fixtures, and it **regressed `wpt css/css-fonts/variations` by 92 subtests**. The assertions
  said why — *"matching for weight 420 should be mapped to CSSTest Weights 600"*. Refused, reverted
  whole, with a three-step plan.
* **t1493** carried the `@font-face` **descriptor** (`manuk_css::FontFace::weight`) and matched the
  old coarse rule over it. `variations` went **237/242 → 247/247**.
* **This tick** does both — the numeric key and §5.2 over the declared weight. `variations` holds at
  **247/247**; `css/css-fonts` is **4092/7552**, its banked mark, twice.

## ⚠ And the system-font path is deliberately left coarse

Handing `fontdb::Query` the real weight instead of `BOLD`/`NORMAL` takes `variations` from
**247 → 148**, measured twice each, with everything else in the tree identical. Isolated to one line,
and **not shipped**. So a system family still collapses, and that is stated rather than claimed:

```text
  system `Lato`   300   400   500   700   900
    Chrome        303   312   314   320   327
    ours          312   312   312   320   320     <- unchanged, NOT claimed
```

⚠ **The cause is not established.** The `variations` tests load their faces from `./resources/…`
through the `@font-face` path; if those loads fail in the harness, the suite scores FALLBACK behaviour
and a coarse fallback happens to match more of the expected widths. That is a hypothesis. The next
probe is whether those faces register at all — `Page::webfonts()` answers it directly, and it exists.
Until then the metric binds and the line stays coarse.

## What the gates cover, and one that had to move

`g_a_graded_font_face_set_matches_by_weight` — the four Chrome rows, with **two** vacuity arms: all
three faces must arrive, and 400/500/700 must select three *distinct* widths (a collapse repeats one).
Red under the nearest-number mutation and under collapsing the key at its layout source.

⚠ A third mutation — *match the FILE weight, not the declared* — was **inert against this gate**,
because its declared weights agree with its files. It is red against
`g_a_font_face_declares_its_own_weight` (t1493), whose fixture deliberately **inverts** them. *A
mutation that does not go red is a question about the fixture*, and the answer here was that the two
gates cover different halves and the mutation belongs to the other one.

⚠ And collapsing the key had to be mutated at **both** `FontKey` construction sites in
`engine/layout` — mutating one left the gate green, which says the two sites serve different paths and
this fixture reaches only one of them.
