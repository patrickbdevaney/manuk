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

## Shrink-to-fit content extent must include the child's RIGHT margin (margin box, not border box)

`content_right_extent` measures a block's max-content/shrink-to-fit width by walking children and taking the
rightmost edge. It counted `rect.x + rect.width` (border-box right) — but `rect.x` already includes the
child's LEFT margin, so omitting the RIGHT margin makes the box-model asymmetric and the extent short by one
margin. A flex item wrapping `<p width:100 margin:10>` measured 110 instead of 120 (its content's margin
box). Fix: add `px_margin_right(node)` (percentage/auto → 0 for an intrinsic measure; negatives don't extend
the border-box edge, so clamp ≥ 0) at each `content_right_extent` box visit. Affects every shrink-to-fit
path — flex/grid items, inline-block, floats, table cells.

## text-transform — rendered casing without touching the DOM text (tick 182)

`text-transform: uppercase` is everywhere — nav bars, buttons, section headings, table headers; and
`capitalize` on titles. It was **unimplemented** (0 hits in the engine), so text rendered in its source
casing: a `text-transform:uppercase` button whose textContent is "Submit" rendered "Submit", not
"SUBMIT". A visible divergence on a large fraction of styled pages.

Mechanism (css + layout):
- **css** — `TextTransform` enum (`None`/`Uppercase`/`Lowercase`/`Capitalize`), an **inherited**
  `Style::text_transform` (copied from the parent in the MinimalCascade inheritance step, beside
  `white_space`), parsed from the `text-transform` property, added to the style-change-detection set,
  and recovered from MinimalCascade on the shipping **Stylo** path (Stylo's servo build exposes it as
  a bitflags type we would otherwise map by hand).
- **layout** — `apply_text_transform(raw, cs.text_transform) -> Cow<str>` at the point a text node
  becomes inline words (`collect_inline_node`, the `NodeData::Text` arm). The RENDERED run is re-cased
  (and therefore measured at its new width — no separate metrics bug) while the **DOM text is
  untouched**: `dom.text_content` still returns the author's string, so JS reads what the author wrote.
  `None` borrows the input (zero-alloc); the casing modes allocate. Unicode casing is honoured
  (`ß`→`SS`, locale-independent `to_uppercase`/`to_lowercase`); `Capitalize` upper-cases the first
  cased letter of each whitespace-delimited word — the common-case approximation of the spec's "first
  typographic letter unit".

**Gate.** `text_transform_recases_rendered_text_only` (engine/layout): unit (Submit→SUBMIT,
HELLO→hello, "hello world"→"Hello World", straße→STRASSE) + E2E (inherited uppercase nav renders HOME;
a child `text-transform:none` island stays "Keep"; `dom.text_content` still contains "home"). RED vs
the no-transform baseline. css+layout suites green (layout 72→73), HANG/CRASH 0. Residue:
`full-width`/`full-size-kana` keywords; the spec's exact grapheme-cluster word boundary for capitalize
(digits/punctuation-prefixed words); `letter-spacing`/`word-spacing` are separate unbuilt properties.

## overflow-wrap / word-break — char-level breaking of an unbreakable token (tick 183)

A single unbreakable token — a long URL, a 64-char commit hash, an unspaced foreign string, an API key
— has no whitespace and no UAX-14 opportunity (hyphen / soft-hyphen / U+200B / CJK) for `break_segments`
to split at, so it stays one word and the line-filler lets it overflow its column, pushing the layout
sideways (the classic "long link blows out a narrow sidebar"). `overflow-wrap: break-word` — with its
legacy alias `word-wrap: break-word`, and the CJK/code cousin `word-break: break-all` — is the
everywhere fix: break the token at an arbitrary character so it wraps. It was **unimplemented** (0 hits).

Mechanism (css + layout):
- **css** — `OverflowWrap` (`Normal`/`BreakWord`/`Anywhere`) parsed from `overflow-wrap` **and** the
  legacy `word-wrap` (same computed value); `WordBreak` (`Normal`/`BreakAll`/`KeepAll`) from
  `word-break`. Both **inherited** (copied in the MinimalCascade inheritance step beside `white_space`
  / `text_transform`), added to the style-change set, and recovered from MinimalCascade on the shipping
  **Stylo** path (servo build models them as keyword enums we don't consume directly).
- **layout** — a derived predicate `break_word = overflow_wrap ∈ {BreakWord, Anywhere} ||
  word_break == BreakAll` is computed in `collect_inline_node` and carried on `InlineItem::Word`. The
  actual split happens in `break_overwide_words`, a pre-pass at the head of `layout_inline` where the
  content width `cw` and font metrics are both known: any `break_word` word whose measured width
  exceeds `cw` is greedily split at char boundaries into chunks that each fit `cw` (never an empty
  chunk — a single glyph wider than `cw` is an accepted unbreakable overflow), emitted as ordinary
  breakable words so the existing line-filler wraps them across lines. Only over-wide break-word words
  are rewritten; every other item passes through untouched, so the whitespace/UAX-14 path and the
  parity gate are unmoved. The split is lossless (chunks concatenate back to the original token) and
  only the first chunk keeps the token's leading space.

**Gate.** `overflow_wrap_break_word_wraps_long_token` (engine/layout): a 60-char token in a 100px
column — control (`overflow-wrap:normal`) leaves one fragment >100px (overflows); `break-word` splits
into >1 fragment each ≤100px and losslessly; `word-break:break-all` reaches the same breaking. RED vs
the no-char-break baseline. css+layout suites green (layout 73→74), HANG/CRASH 0. Residue:
`word-break:break-all` breaking a word that *would* still fit later in the line (we only split words
wider than a full line); `overflow-wrap:anywhere`'s smaller min-content contribution; `line-break`
and `hyphens`.

## letter-spacing / word-spacing — tracking a run's advance in measure and paint (tick 184)

`letter-spacing` (inter-character tracking) and `word-spacing` (extra inter-word space) are on a large
slice of styled UI — tracked uppercase nav bars, buttons, small-caps labels, kickers/eyebrows, hero
headings — and pair directly with `text-transform:uppercase` (tick 182). Both were **unimplemented** (0
hits): a tracked run measured and painted at its untracked width, so its box was too narrow and its
glyphs too tight wherever the design asked for tracking.

Mechanism (css + layout + paint):
- **css** — `ComputedStyle::{letter_spacing, word_spacing}: f32` (px), parsed from the two properties
  via `values::parse_length_px` (`normal` and anything unparseable → 0; `em` resolves against this
  element's font size). Both **inherited** (copied in the MinimalCascade inheritance step beside
  `white_space`/`text_transform`), added to the style-change set, recovered from MinimalCascade on the
  shipping **Stylo** path (servo build exposes them as a `Spacing<Length>` we don't consume directly).
- **layout** — carried on `TextStyle`. A word's measured width gains `letter_spacing × char_count`
  (trailing tracking included, matching Chrome — so a word of *n* chars reserves *n*×ls, the last of
  which is the trailing gap); each inter-word space gains `word_spacing`. `close_line` (alignment slack)
  and `inline_extent` (min/max-content) switched from re-measuring the fragment text to the stored
  `f.width`, which already carries the tracking (and equals `measure(text)` when spacing is 0).
- **paint** — `draw_text` offsets glyph *i* by `i × letter_spacing` past its shaped pen, exactly
  mirroring the layout width bump so a tracked run measures and paints in step.

**Safety.** The computed default is `0`, at which shaping, measurement, alignment and paint are
byte-identical to before — so all existing content and every parity/WPT number is unmoved and the
ratchet cannot regress. Only an explicitly-tracked run changes.

**Gate.** `letter_and_word_spacing_widen_runs` (engine/layout): `letter-spacing:4px` adds exactly 20px
to the 5-char word "hello"; `word-spacing:10px` pushes the second word of "aa bb" right by 10px. RED vs
the no-tracking baseline (both deltas 0). css+layout+paint green (layout 74→75), HANG/CRASH 0. Residue:
`word-spacing` inside a `pre` run's internal spaces; per-grapheme-cluster tracking for
ligatures/combining marks (we count chars — exact for the Latin common case); negative letter-spacing
is honoured arithmetically but not clamped against a zero-width run.

## text-overflow: ellipsis — truncating a clipped single line (tick 186)

`text-overflow: ellipsis` — always paired with `white-space: nowrap` and `overflow: hidden` — is one of
the most common idioms in real UIs: a card/list title, nav/tab label, table cell, file name or chat
preview that must fit one line and end in `…` rather than being cut mid-glyph. It was **unimplemented**
(0 hits): the box just clipped its content at the edge, slicing a word in half with no ellipsis.

Mechanism (css + layout):
- **css** — `TextOverflow { Clip, Ellipsis }` (non-inherited, default `Clip`) parsed from
  `text-overflow` (a 1–2-value property; `ellipsis` in either slot → Ellipsis), recovered from
  MinimalCascade on the shipping **Stylo** path.
- **layout** — after `layout_inline` of a *pure inline-formatting-context* block, if the box
  `text-overflow:ellipsis` AND clips (`overflow` ≠ `visible`) AND doesn't wrap (`nowrap`/`pre`) AND its
  single line's right edge exceeds `cx + cw`, `apply_text_overflow_ellipsis` runs: keep the fragments
  whose right edge is ≤ `cutoff = cx + cw − width('…')`; the fragment straddling `cutoff` is cut by
  `truncate_to_width` (longest char-boundary prefix fitting the remaining budget); the rest are dropped;
  an `…` fragment is appended at the anchor. The ellipsis inherits the style/owner of the last kept run.

**Safety.** A line that fits is returned untouched and `clip` is a no-op, so no box without an actual
overflow changes — the default path is byte-identical and every parity/WPT number holds; only a
genuinely-overflowing ellipsis box renders differently (which is the whole point).

**Gate.** `text_overflow_ellipsis_truncates_clipped_line` (engine/layout): a long title in an 80px
`nowrap; overflow:hidden; text-overflow:ellipsis` box renders truncated text ending in `…` whose kept
part is a proper prefix of the original; the `clip` control keeps the full run with no `…`. RED vs the
no-truncation baseline. css+layout green (layout 75→76), HANG/CRASH 0. Residue: only the pure-inline
path (mixed block/float lines not yet truncated); `-webkit-line-clamp` multi-line clamp; the line-start
(leading) ellipsis value; char- not grapheme-cluster boundaries.
