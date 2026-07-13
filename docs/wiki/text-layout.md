# TEXT AND LAYOUT — fonts, shaping, measurement

## `shrink_to_fit` is INTRINSIC — it cannot depend on available width

So it must be cached. Recomputing max-content on every call cost bbc **260ms → 168ms** when fixed. *A
quantity that is by definition independent of its input is a cache waiting to be found.*

## Shaped-run caching: word-level for hit rate, run-level fallback for correctness

Firefox and Blink both cache shaped runs; the standard granularity is **word-level** (split on word
boundaries). **The known caveat:** per-word shaping breaks OpenType **contextual** features that need
cross-word context — so word-level needs a run-level fallback for such scripts/features.

Cache key: **font identity + size + run text + script/direction/lang + features.**

**Honest measured result:** on fully *diverse* text the win is ~neutral (tuple-key `String`
construction offsets the saved metrics, and parse/cascade dominate). **The win concentrates on repeated
runs, tables, shrink-to-fit's multi-pass, and resize relayout.**

## Decoded images: LRU + a BYTE budget, not an entry count

Chromium's `cc/tiles/image_decode_cache` uses LRU over *discardable* memory, freeable under pressure.
An entry-count cap is a proxy for the thing that actually matters and it is a bad one.

## A video frame IS a `DecodedImage`

Playing a video is **swapping the `Rc` in the map the poster already occupies** and calling
`request_redraw`. **No new paint code.** This is why media collapses into ticks rather than a
subsystem — *and it is only true because the poster work landed first.*

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## Answering `MinContent` with max-content means no flex item containing a paragraph can EVER shrink

**Taffy uses an item's min-content answer as its automatic minimum size.** A measure seam that folds
`AvailableSpace::MinContent` and `MaxContent` into the same answer therefore **pins every flex item at its
longest-line width.**

Measured on rust-lang.org's three-column row in a 1128px container: Chromium `x=29 w=344 / x=421 w=344 /
x=813 w=344`; ours **`w=1128` each, with two columns entirely off the viewport** — which *looked* like the
columns were stacking, and sent the investigation into media queries for an hour.

**CSS2 §10.3.5 is `min(max-content, max(available, min-content))`.** Min-content is cheap to define
exactly: **lay the subtree out at ~zero available width; the widest fragment that survives IS the longest
unbreakable run.** It costs ~16% of layout on a flex-saturated page and is worth it.

## Measuring max-content by laying out at a huge available width lies in THREE different ways

`shrink_to_fit` probing at width `1e6` breaks whenever anything reacts to available space:

1. **A block-level child FILLS its container**, so its rect comes back ≈1e6 — the first flex item measures
   to the whole container and collapses its siblings to zero (*a flex card row rendered as ONE full-width
   card*).
2. **`max-width` clamps the container back down and `justify-content:center` centres content inside THAT** —
   a 32px icon button measured **234px**, and `margin:auto` measured **500,532px**.
3. **Centred text measures the CENTRING SLACK.**

Ask **taffy** for a flex/grid container's max-content directly. And **max-content is INTRINSIC — it cannot
depend on available width — so cache it** (bbc layout 260ms → 168ms).

## A percentage width on a flex item resolves TWICE unless the assigned width is distinguished from the containing-block width

Handing taffy's assigned width into `layout_block` as `cw` — the name used everywhere else for
*containing-block* width — makes the item's own `width: 30%` resolve against it **a second time**: 30% of a
1000px row becomes **30% of 300 = 90px**. **The used width is the SQUARE of the intended one**, and children
compound it.

**It survived every existing test because the two commonest values are exactly the two that are immune:**
`auto` has nothing to re-resolve, and 100% of 100% is still 100%. **Only an in-between percentage (30/70,
50/50 — how most page layouts are actually built) exposes it.**

## A replaced element's auto height comes from used width × intrinsic ratio, not from the image's pixels

So `img { max-width: 100% }` — **the most common reset on the web** — narrowed the box and **left the height
alone, stretching every responsive image.**

## `font-size: 0` makes swash emit UNSCALED font-unit outlines — 1000–1500px bitmaps per glyph

Asked to rasterize at 0px, swash falls back to the face's **unscaled outline in font units** and returns
bitmaps like **1227×1450 per glyph**, which the blitter then floods with the run's text colour. One
`font-size: 0` word buried old.reddit's post titles under **~27,000px of flat grey**.

**`font-size: 0` is not exotic** — it is the standard trick for killing the whitespace gap between
`inline-block`s, and half of the image-replacement recipe (`text-indent:-9999px; font-size:0`) on logos and
icon buttons. **Any rasteriser needs a guard on glyph bitmaps larger than a few multiples of the font
size.**

Separately: **a unitless zero is a valid CSS length**, so a parser handling only `Dimension` and
`Percentage` tokens drops `font-size: 0` entirely and the size stays **inherited**.

> **Every instrument said the bug was impossible** — it was in no display item, no decoded image, no rect —
> **because it was a LETTER.** *When every instrument agrees a bug cannot exist, they are all sampling the
> same layer. Bisect the layer below.*

## Chromium never asks fontconfig for a bare generic family — it asks for **Arial** and **Times New Roman**

`fontdb`'s defaults are the **Windows** names, usually absent on Linux, so `font-family: sans-serif` landed
on an arbitrary fallback. **The instinct — "ask `fc-match sans-serif`" — is ALSO wrong**: that returns
**Noto Sans**, and **Noto's line box is 1.362em against Liberation's 1.150em**, so **every line on every
page comes out 18% too tall.**

Chrome asks for the Arial/Times names, which fontconfig substitutes with the metric-compatible
**Liberation** faces. Matching that order makes advances exact (ten `i` at 100px: default 277.84 / serif
277.84 / sans 222.17 / mono 602.06 — matched to the pixel).

**Two corollaries:** **Chrome's default font is a SERIF**, and **Chrome's default monospace size is 13px,
not 16px** — which is why `<code>` famously renders smaller than its prose. Using 16px made every code block
on the web **23% too large**.

> This is why an apparently-correct font fix took the conformance wall **72/72 → 69/72** and *looked* like a
> metrics-engine problem — the reading was "adopt Skrifa", when **the real bug was the preference list.**
> Adopting Skrifa would have replaced a working metrics engine and **left the bug in place.**

## `font-family` was never mapped from the cascade AT ALL — and it wore a font-metrics costume for ticks

Every page rendered in one fallback face regardless of its CSS. **Every "font metrics divergence" the oracle
had ever reported was this bug in disguise**: we were not *mismeasuring* the font, we were **not using it**.

## `line-height: normal` comes from the font's ascent + descent + lineGap

Multiplying font-size by 1.2 is the wrong height for **every line box on every page** and is a first-order
source of vertical drift. **The subtler half:** you need a flag recording whether the author *authored* the
value, **inherited alongside the number** — otherwise an explicit `line-height: 20px` gets silently
overridden by the face's metrics, and two cascades disagree about every line box on the page.

## A block box inside an inline must BLOCKIFY the inline (CSS2 §9.2.1.1)

Deciding `has_block` from **direct children only** sends an inline that wraps a block down the pure-inline
path, where **the block's TEXT is harvested as inline words and its BOX is discarded** — text flows, but
background/padding/border **vanish**. This is ubiquitous in real markup (`<div>` inside `<a>`/`<span>`/a
custom element). Recurse through inline-only descent; **inline-block/flex/table are ATOMIC and do not
propagate.**

## Empty inline elements have GEOMETRY, and pages depend on it

An empty `<span id=…>` anchor produced no box and no fragment, so **it did not exist**. Chrome gives it
width 0 and a line-height-tall rect. One Wikipedia article carried **1,079 spans and 298 anchors** in that
state — **98% of everything the coverage probe reported missing on that page.**

## Two ways content is laid out perfectly and still cannot be SEEN

- **Every `position: absolute` element with no insets was DELETED.** Its static position needs flow's
  cursor; flow discarded it, and the abs pass had nothing to place against. *That is every React portal
  root, every JS-positioned dropdown, every `.sr-only` node.* GitHub coverage **91.4% → 97.8%** on this
  alone.
- **Anonymous boxes were stranded in stacking layer 0**, because `z` and `clip` are keyed by `NodeId` and a
  **synthesised box has no node** — so the anonymous box holding a `z-index`'d element's **text** sorted
  below that element's own background and was **painted over**.

## Inline elements produce no layout box — so `<a>` and `<button>` had NO GEOMETRY AT ALL

Inline text becomes `TextFragment`s inside the containing block's inline formatting context, which **dropped
node identity** — meaning **exactly the elements you click had no rect.** Fix: `TextFragment` carries
`node: Option<NodeId>` (deepest element ancestor) + `width` (the advance, so a run's rect needs no
re-measure); rect computation **unions block boxes ∪ inline runs** and propagates each run up into its
element ancestors, so `<a><em>x</em></a>` gives `<a>` a rect. An inline split across lines gets the **union**
of its runs.

## WOFF2 needs no C++ — and neither swash nor skrifa will decompress it for you

**Two load-bearing negatives:** **swash does not decompress WOFF2** (it operates on already-decompressed
sfnt), and **skrifa/fontations do not either.** Without a shim, **most real web fonts silently drop**,
because the font entry point only accepts raw sfnt magic.

The hard part — reconstructing the **glyf/loca transform** (`UIntBase128`/`255UInt16` varints; the
point/flag/contour/bbox/instruction streams; rebuilding head/maxp/hhea/loca) — exists in maintained pure
Rust. Depend on Dropbox's **`brotli-decompressor`** (decode-only, safe), **not** the heavier full `brotli`
crate; **the stock `woff2` 0.2/0.3 crate is abandoned and does not build.**

## A shaped-run cache key must EXCLUDE pixel size

**Shaping is size-independent up to hinting** — advances can simply be scaled afterward. Key like
cosmic-text's `ShapeRunKey`: `{text, font family+weight+style+features, script, direction, language}`.
**Including the size silently multiplies the cache by every font-size on the page for zero correctness
benefit.**

## The char-based glyph model cannot express ligatures or complex scripts

`GlyphPos { ch, x }` is a **dead end** — a shaped run is not characters with x-offsets; any real shaper
produces `{glyph_id, font_id, x, y}` runs. **Parley was evaluated and REJECTED for wholesale adoption** (it
pulls icu4x + fontique + harfrust and re-plumbs layout); borrow its *ideas* (three-tier shape cache,
`{script, locale}` fallback, UAX#14 breaker) on top of the existing swash stack.

## Skrifa is the library Chromium itself ships

Skrifa (Google's Fontations family) is the **literal metrics/outline/hinting library Chrome ships**,
replacing FreeType, with extensive pre-ship pixel-comparison against it. **HarfBuzz** is the shaping layer
both Chromium and Firefox use. **This is the one explicit exception to "read the algorithm, never the
library"** — font metrics are the dominant source of persistent sub-pixel drift, so adopting Skrifa collapses
an open-ended subsystem into a bounded integration. *Rasterization stays local (tiny-skia); Skrifa
deliberately does not rasterize.*

## `tiny-skia` has no Gaussian blur

Box-shadow's soft edge must be built from **stacked concentric rounded rects with a quadratic alpha
falloff**, and rounded rects from a Bézier path with **k = 0.5523**; damage boxes grow by `blur`. Inset
shadows, multiple shadows and spread should map to **`None` rather than to a wrong shadow.**
