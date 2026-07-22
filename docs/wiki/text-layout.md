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

## text-decoration-color — a colored underline paints in its own hue, not the text color (tick 187)

A colored decoration line — a brand/hover underline, a strikethrough price in a distinct hue, an
overline accent — is everywhere in modern design, and it is the single most common way
`text-decoration` is customised. But the paint side hardcoded the line color to the run's text color
(`fade(f.style.color)`), and the parser threw away any color token, so `text-decoration-color:red` on
blue text drew a **blue** underline: the wrong color on every link whose underline was meant to
contrast with its text.

- **css** — `TextDecoration` gains `color: Option<Rgba>` (`None` == the `currentColor` default). The
  `text-decoration` *shorthand* resets it: lines come from keyword presence, and the color is the
  first value token that `parse_color` accepts (`underline dotted red`), skipping the line/style
  keywords (`underline`/`overline`/`line-through`/`blink`/`none`/`solid`/`double`/`dotted`/`dashed`/
  `wavy`). The `text-decoration-color` *longhand* sets it directly (`currentColor` → `None`). The
  `text-decoration-line` longhand touches only the line bits, leaving a set color intact. Recovered
  wholesale from MinimalCascade on the shipping **Stylo** path (the whole `TextDecoration` is already
  recovered there — the new field rides along for free).
- **paint** — the decoration line color becomes `fade(d.color.unwrap_or(f.style.color))`: an explicit
  decoration color wins, otherwise it follows the text color exactly as before.

**Safety.** The default `None` reproduces the old `fade(f.style.color)` byte-for-byte, so every run
without a decoration color is unchanged and the ratchet cannot regress — behaviour changes only when
`text-decoration-color` (or a color in the shorthand) is actually set.

**Gate.** `text_decoration_color_overrides_text_color` (engine/paint): `.l{color:#00f;
text-decoration:underline;text-decoration-color:#f00}` emits a TextLine that is **red**, and no
TextLine is the blue text color; the control (no decoration color) defaults the underline to blue.
RED vs the hardcoded-text-color baseline (line == text color always). css+paint green, HANG/CRASH 0.
Residue: `text-decoration-style` (dotted/dashed/wavy/double still paint solid),
`text-decoration-thickness`, `text-underline-offset`, and `text-decoration-skip-ink`.

## `text-decoration-thickness` / `text-underline-offset` — a decoration line the design's own weight and position

`text-decoration-thickness` (Tailwind `decoration-2`, thick brand underlines) and
`text-underline-offset` (Tailwind `underline-offset-4`, breathing room under links) are everywhere in
modern design, but the decoration line was drawn at a **hardcoded** thickness (`font_size / 14`, so a
14px font always got a 1px hairline) at a **fixed** underline position, so `decoration-2` drew a
hairline and `underline-offset-*` did nothing — the underline crowded the text on every design that
asked it not to.

- **css** — `TextDecoration` gains `thickness: Option<f32>` (`None` == `auto`/`from-font`, the
  font-derived default) and `underline_offset: f32` (px below the default underline position, default
  0). The `text-decoration-thickness` longhand parses a length via `values::parse_length_px` (`auto`
  → `None`); `text-underline-offset` parses a length (`auto` → 0). The `text-decoration` *shorthand*
  resets `thickness` to `None` (it is a longhand of the shorthand) but **leaves `underline_offset`
  untouched** (it is *not* a longhand of `text-decoration`). Recovered wholesale from MinimalCascade on
  the shipping **Stylo** path (`cs.text_decoration = m.text_decoration` — the new fields ride along).
  Dropping the struct's `Eq` derive (an `f32` cannot be `Eq`) is safe: nothing keys a map on it.
- **paint** — thickness becomes `d.thickness.filter(|t| *t > 0.0).unwrap_or((font_size/14).max(1))`,
  and the *underline* line's y gains `+ d.underline_offset` (overline/line-through are unaffected —
  the offset is underline-only per spec).

**Safety.** `thickness: None` + `underline_offset: 0.0` (the defaults) reproduce the old thickness and
y byte-for-byte, so every run without these properties is unchanged and the ratchet cannot regress —
behaviour changes only when a thickness or offset is actually set.

**Gate.** `text_decoration_thickness_and_offset_shape_the_underline` (engine/paint): a 14px underline
defaults to a ~1px line; `text-decoration-thickness:6px` paints a 6px line; `text-underline-offset:8px`
keeps the thickness but sits the line exactly 8px below the default y. RED vs the hardcoded-thickness /
fixed-position baseline. css+paint green, HANG/CRASH 0. Residue: `text-decoration-style`
(dotted/dashed/wavy/double still paint solid), `text-decoration-skip-ink`, `from-font` exact metrics.

## The shaper must be told WHICH SCRIPT it is shaping (tick 214)

swash's `ShaperBuilder` defaults `script` to `Script::Latin`, and `shape_run` never called
`.script()`. **The script is what selects the OpenType feature set**, so every run on the web was
shaped with Latin's — which needs no joining, no reordering and no conjunct formation, so none of
those features ever ran:

- **Arabic** rendered as disconnected isolated letterforms. `init`/`medi`/`fina` never applied, so
  `مرحبا` came out as five standalone shapes instead of one joined word.
- **Devanagari** was a flat 1:1 codepoint→glyph mapping. `akhn`/`half`/`pres` never applied, so
  conjuncts never formed and the virama rendered as a visible dangling mark.
- Thai, Bengali, Tamil, Khmer and every other complex script were wrong the same way.

**Why it survived so long, and this is the transferable part.** *Nothing was missing.* No `.notdef`,
no tofu, no error, no exception, a plausible width, and the per-glyph **fallback worked correctly**
and picked exactly the right face. The text rendered as real letters from the right font that happen
to be **wrong** — which, to anyone who does not read the script, looks fine. Every instrument the
project had was pointed at *coverage* ("is there a glyph?") and this bug has perfect coverage.

The probe that found it (`engine/text/tests/probe_script_fallback.rs`) only saw it by comparing
**glyph count against codepoint count** — a cheap, script-agnostic invariant that needs no ability to
read the script. That is the reusable instrument: *for a complex script, glyphs ≠ chars is the
signal, and glyphs == chars is the bug.*

**The fix** is script-aware run segmentation. `segment()` returns `(FaceId, Script, String)` instead
of `(FaceId, String)` — a run breaks when **either** the face or the script changes — and the script
is passed to `ctx.builder(font).script(script)`.

⚠ **`Common`/`Inherited`/`Unknown` characters must EXTEND the run in progress, not open a new one.**
Spaces, digits and most punctuation carry no script of their own. If they started a new run, an
Arabic word split at its own comma would stop joining across the cut — reintroducing the same bug in
running text only, where it is hardest to spot. They only start a run (as Latin) when nothing
precedes them.

Held by `G_COMPLEX_SCRIPT` (`engine/text/tests/g_complex_script.rs`). **Both claims proven RED**
independently by removing `.script(script)`: Devanagari falls back to 6 glyphs for 6 codepoints, and
the Arabic interior letter keeps its isolated glyph id inside the word. The gate also pins Latin (5)
and CJK (4) glyph counts, because the risk script segmentation introduces is **over-splitting** — a
run cut per character shapes nothing correctly and loses kerning.

**Confirmed already working, so do not re-probe:** per-glyph font fallback itself. CJK, emoji,
Arabic, Hebrew and Devanagari all resolve real faces with zero `.notdef` (`FALLBACK_FAMILIES`). The
lever board's "CJK/emoji renders as TOFU" was a `?`, and the answer is **no** — this is the fifth
time a feature assumed missing here turned out to be built (after `localStorage`, `FormData`,
`position: sticky`, `IntersectionObserver`). **An absent measurement is not a negative measurement.**

## The bidi BASE direction — `direction: rtl` / `dir="rtl"` (tick 215)

Shaping decides which glyph; the **base level** decides where it goes. `FontContext::shape`
hard-coded that base to LTR (`BidiInfo::new(text, Some(Level::ltr()))`), so `direction: rtl` and
`dir="rtl"` — how the entire Arabic, Hebrew, Persian and Urdu web declares itself — changed nothing.
After tick 214 every character was present and correctly *shaped*, and still in the **wrong order**:
a trailing period on the wrong end of the line, an embedded Latin word or number on the wrong side
of its neighbours, short lines hugging the wrong margin.

**This is the same failure shape as tick 214, one layer up**, and worth naming as a pair: a
*coverage* instrument cannot see either. Nothing is missing, nothing is `.notdef`, the width is
plausible. Tick 214's invariant was `glyphs == chars`; this one's is **"the same string under two
bases must not shape identically"** — also script-agnostic, also needing no ability to read the text.

**The plumbing**, six touch points in `manuk-css` following the tick-183 `OverflowWrap` template
(enum · `ComputedStyle` field · default · inherit · parse · relayout-damage), plus:

- **`stylo_engine.rs` must recover it from `MinimalCascade`.** The shipping path is Stylo, whose
  servo build does not surface `direction` in a form we consume — without the recovery line the
  property works in tests and does nothing in the browser.
- **`dir="rtl"` is a presentational hint in `apply_ua_defaults`**, and it is not optional: nearly
  every RTL site sets the attribute on `<html>` rather than writing `direction: rtl` in CSS, so a
  stylesheet-only implementation reads as "RTL unsupported" on exactly the sites that need it.
- **`TextStyle.rtl`** carries it layout → paint, because visual order is resolved at shaping time.
  ⚠ Adding a field to `TextStyle` breaks constructors in **`shell/src/gui.rs`** too — grep every
  crate, not just `engine/`.
- **`RunKey` gained the base direction.** Without it the second paragraph is a cache HIT returning
  the first one's ordering: correctly-shaped glyphs in the wrong places, only sometimes.

**HTML's initial value is `ltr`, NOT content detection**, and the gate pins that. Inferring RTL from
an unmarked Arabic paragraph would look more "correct" and would be a *structural divergence from
Chromium* — which the north star calls a bug regardless of how it looks. `dir="auto"` is the opt-in
for detection and is not implemented yet.

**A measured residual, stated rather than smoothed over:** the two bases give run widths differing by
~0.89px on a 70px mixed line (~1.3%). The bases split the line into different bidi runs, so the space
between scripts is shaped in a different run and picks up a slightly different advance. Per-run
shaping is what every browser does, so this is inherent — but it matters because `measure()` is
direction-agnostic (base pinned LTR) while paint shapes with the real base. `G_BIDI_BASE` bounds it
at 3%; a real divergence would be painted text overflowing the box layout reserved for it.

Held by `G_BIDI_BASE` (`engine/text/tests/g_bidi_base_direction.rs`), **proven RED** by pinning the
base back to LTR. It also pins that pure-LTR text is byte-identical under both bases — the risk RTL
support introduces is perturbing the 99% case.

**Residue:** `dir="auto"`, `unicode-bidi` (`isolate`/`embed`/`bidi-override`), RTL `text-align`
defaulting to `right`, and RTL block-level layout (list markers, scrollbar side, `float` reversal).
This tick makes RTL text **read correctly**; it does not yet make an RTL *page* lay out mirrored.

## Canvas text is the DOM text stack, wired to a different surface

`ctx.fillText` shapes and rasterizes through **`engine/text`** — the same swash pipeline, the same
bidi reordering, the same per-glyph fallback chain, the same glyph raster cache as a paragraph. It is
a wiring job, not a second renderer, and that is the load-bearing decision: a text stack living
inside the canvas would drift from the DOM's within one tick and would have to re-learn ticks 214
(complex-script shaping) and 215 (bidi base direction) separately. Because it shares the pipeline, a
canvas draws joined Arabic, Devanagari conjuncts, CJK and emoji for free.

**The split follows the rest of `canvas.rs`.** JS owns the state machine and the string ergonomics —
the `ctx.font` CSS shorthand parse, `textAlign`/`textBaseline` pen offsets, colour resolution. Rust
receives a resolved pen origin, colour, size, family list and two style bits. One native call per
`fillText`.

### The canvas blit cannot be `manuk_paint`'s blit

`manuk_paint::blit_coverage` writes `alpha = 255`, because it composites onto an opaque page
background. A canvas is **transparent-backed** — that is exactly what lets it compose over the page —
so alpha has to accumulate (`a_out = a_src + a_dst·(1−a_src)`) in the premultiplied space `Pixmap`
stores. Reusing the opaque blit fills every glyph's bounding box with opaque fringing. Same glyph
bitmaps, necessarily different compositor.

### `measureText` returning `length * 7` was worse than an imprecise width

It is a width with **no relationship to the glyphs**, so every layout derived from it compounds the
error: centring, wrapping, column fitting, label-collision checks, terminal cell hit-testing. The
cheapest proof it is a fiction rather than an estimate: under it `IIIIIIIIII` and `WWWWWWWWWW`
measure identically. `g_canvas_text` asserts exactly that pair.

### Transforms: uniform scale is exact, rotation is the documented gap

Glyphs are rasterized from outlines *at a size*, so `ctx.scale(2,2)` genuinely renders at twice the
size rather than magnifying a bitmap. The transform is reduced to a scale (mean of the two column
norms) plus a mapped origin: text lands at the correctly transformed position, at the correctly
scaled size, **upright**. Rotation and skew are not applied to the glyph raster — wrong for rotated
axis labels, right for everything else. Closing it means an outline API on `FontContext`
(`scale_outline`) so glyphs can be filled as paths through the transform, which is its own tick.

Two smaller bounded approximations, recorded so they are not rediscovered as bugs: `maxWidth`
re-shapes at a smaller size instead of condensing horizontally (loses height with width, but keeps
the label inside the box the author reserved — overflow is the worse failure for the axis labels that
pass it); and `strokeText` renders **filled** in the stroke colour, because the raster hands back
coverage, not an outline path.

### Gate lesson: a pixel claim must re-assert that ink exists

`sparse` ("the ink is not everywhere") and `placed` ("the ink is in the right place") are both
trivially true of a **blank** canvas. Written without an explicit `n > 0`, a no-op `fillText` would
satisfy them, and the gate would print two false greens beside its real failure. Every pixel-extent
claim in `g_canvas_text` carries the ink-count conjunct for that reason.

## Canvas `drawImage` — the first operation that needs pixels flowing INWARD

### The plumbing was directional, and that is why the method was a no-op

Every canvas operation before this one draws something the *script* described: a colour, a path, a
string. `drawImage` draws something the **host** owns — the decoded bytes of an `<img>` the network
fetched. Canvas had exactly one pixel channel, `canvas_bitmaps()` → the image map the painter reads,
and it pointed **outward**. There was no way in, which is precisely what
`ctx.drawImage = function(){}  // no image source plumbing yet` was recording.

`manuk_js::publish_image_source` is the deliberate mirror of `canvas_bitmaps`, keyed by the same
`NodeId`, and `Page::publish_image_sources` calls it before each script round. A source is named by
**node id, never by handing pixels across the FFI**: a sprite sheet is megabytes and an animation
loop would copy it sixty times a second.

### Canvases and images must live in SEPARATE registries even though `Page` merges them

`drain_canvases` drops finished canvases into `self.images` alongside `<img>` — that is the trick
that lets the painter treat a canvas as a replaced element and know nothing about canvas at all. So
the obvious implementation, "look the source up in `self.images`", is wrong in a way that is very
hard to see: `CANVASES` holds **live** surfaces, `self.images` holds a **snapshot taken at the end of
the previous script round**. Under a shared map, the standard double-buffer idiom
`dst.drawImage(scratch, 0, 0)` composites the *previous frame*. Canvases are therefore excluded from
publishing, and `CANVASES` is looked up first.

### A negative extent means two different things on the two rects

On the **source** rect it merely re-anchors the same region and is otherwise a no-op. On the
**destination** rect it MIRRORS — `drawImage(img, x+w, y, -w, h)` is how a sprite sheet draws a
character facing the other way. Conflating them (normalising both, or rejecting both) leaves every
sprite in a game facing the same direction, with nothing thrown and no visual clue that an argument
was dropped.

### It is a PATTERN FILL of the destination rect, not `draw_pixmap`

tiny-skia's `draw_pixmap` takes an integer offset and cannot express the source crop, the non-uniform
dst/src scale, and the context transform simultaneously. A `Pattern` carries its own matrix, so all
three compose: the pattern maps the source crop onto the destination rect, and the fill transform
handles the rest. `SpreadMode::Pad` rather than `Repeat`, because bilinear sampling reads half a texel
past the crop at its edges and repeating wraps the opposite edge in as a one-pixel fringe.

### tiny-skia applies the fill transform to the SHADER as well as the path

This is the trap, and the gate caught it only because the claim was strong enough. `fill_path`'s
transform is concatenated onto the shader's own matrix, so the pattern matrix must be expressed purely
in user space. Pre-multiplying `xform(m)` into it as well type-checks and looks obviously correct —
and **passes a single-corner pixel assertion by accident**: the doubly-transformed sample lands
entirely off the image, `Pad` clamps every pixel to the source's top-left texel, and a flat fill of
that one colour satisfies any claim that happens to name it. The fixture's top-left is red, the naive
claim asserted red, and it went green on a completely broken draw.

The fix in the *gate*, not just the code: `xform` asserts **all four quadrants** of an asymmetric
fixture. A flat clamped fill cannot impersonate four distinct colours. The general rule — a claim
about a transformed image needs at least two distinguishable colours in each axis, or it is really
only asserting "something was painted".

### RED probes executed, not asserted (process rule 3)

  · restore `ctx.drawImage = function(){}`  → 8 of 9 claims fail (`undecoded` correctly survives:
    a no-op and a spec no-op are indistinguishable, which is the point of keeping that claim separate)
  · delete both publish hooks              → `imgblit`/`imgcrop` fail, every canvas→canvas claim
    stays GREEN — proving the two source paths are independently exercised, not one claim twice
  · fold `xform(m)` into the pattern matrix → `xform` fails on the four-quadrant claim (and passed
    on the one-corner version, which is how the latent bug was found)

### Residue

`putImageData` and `clip()` remain honest no-ops. `ImageBitmap`, `OffscreenCanvas` and `<video>` as
sources return no `__nodeId` and draw nothing — the shim skips them explicitly rather than throwing.
Canvas still keeps its own `FontContext`, so `@font-face` webfonts do not resolve inside a canvas.

## The line box is a whole number of pixels (tick 269)

The FID-SWEEP's NEAR-MISS population — `mdx=0`, `mdy` = 12/20/45/82, **growing with text density** —
had one more branch to test after tick 268, and this is the one that was load-bearing.

### The measurement

One 600px-wide, 6-line paragraph at `font: 16px sans-serif`:

```
Chrome 108px      Manuk 110.39px      →  0.4px per line box
```

Nothing was wrong with font *selection*: our metrics are Liberation Sans to four decimals, and
Chrome's `sans-serif` measures 18px per line where DejaVu gives 19 and Noto 22 — so both engines had
already picked the same face. Shaping was right, advance widths were right (`mdx=0` said so all
along). The line box was simply **fractional** — 18.398px against Chrome's 18 — and that remainder
rides on *every line box on the page*, so it compounds downward instead of staying local. Over the
~110 line boxes of a dense article it is 45px, and it displaces every element below the text.

### The rule

`line-height: normal` = **`round(ascent + descent + lineGap)`** — the sum rounded, not the parts.

### The wrong rule that looks identical

The first implementation rounded each term separately, with a confident comment citing Skia's
`SkScalarRoundToScalar`. It is wrong, and one face cannot tell:

```
                  ascent  descent    gap     sum   round(sum)  Chrome   round-each
Liberation Sans   14.484    3.391  0.523  18.398          18       18       17  ✗
DejaVu Sans       14.854    3.773  0      18.627          19       19       19  =
Noto Sans         17.104    4.688  0      21.792          22       22       22  =
```

It agrees on two faces of three and is wrong on **the one we actually ship**. It was caught only by
re-running the probe *after* the edit — the reasoning that motivated it was fluent and the arithmetic
that refuted it (`14.484.round() == 14`, not 15) took one line. Hence three faces in the table, hence
the gate asserts on a face whose `line_gap` is non-zero, and hence one assertion exists purely to
fail under the round-each rule. **A zero-gap face cannot discriminate between the two rules at all**,
so a gate built on DejaVu or Noto would have passed the broken implementation.

### What is NOT rounded

Advance widths. Chrome positions glyphs subpixel horizontally, and the sweep already measured our
horizontal placement as exact. Rounding widths would trade a fixed vertical error for a new
horizontal one — the same shape of trade as the nested-list margin in tick 268.

---

## The inline box is the CONTENT AREA, not the line box (tick 271)

The largest single systematic placement error the engine has had, and the one hardest to see
locally: **`getBoundingClientRect()` on an inline element returned the line box.**

```
<p style="font: 16px/1.6 sans-serif">before <a>link</a></p>

Chrome      <a>  y = line_top + 4    height = 17     ← the font's content area
Manuk       <a>  y = line_top        height = 25.6   ← the line box
```

Wrong in **both** coordinates, on **every `<a>`, `<span>`, `<em>`, `<strong>` and `<code>` on every
page that sets `line-height`** — which is essentially the whole web. FID-SWEEP had been showing the
signature for three ticks without it being read correctly: on wikipedia, `dw=0` (widths exact) with
`dh=+7` repeated across dozens of elements, and a median `dh=4` for the page.

### The rule (CSS 2.1 §10.6.1)

```
content_height = round(ascent) + round(descent)        ← no line gap, no line-height
half_leading   = floor((line_box_height - content_height) / 2)     ← may be NEGATIVE
content_top    = line_top + half_leading
line_box       = line-height                           ← content may OVERFLOW it
```

### The two rounding rules are opposite, and that is not a typo

`line-height: normal` rounds the **sum** (tick 269, above). The content area rounds the **parts**.
Measured against real Chrome, 2 faces × 8 sizes, no exception:

```
                 size   ascent  descent   round+round   round(sum)   Chrome
Liberation Sans  14px   12.672    2.966      13+3 = 16          16       16
Liberation Sans  16px   14.484    3.391      14+3 = 17          18   ✗   17
Liberation Sans  32px   28.969    6.781      29+7 = 36          36       36
DejaVu Sans      16px   14.852    3.773      15+4 = 19          19       19
DejaVu Sans      32px   29.703    7.547      30+8 = 38          37   ✗   38
```

The **14px→16 / 16px→17** pair is the discriminator: no single ratio and no rounded sum can grow a
box by 1px across a 2px size step. Only per-part rounding does. Tick 269 rejected per-part rounding
for the *line box* and was right to; applying that conclusion to the *content area* would have been
the natural mistake, and the sweep across sizes is what forecloses it.

### Half-leading is signed

`line-height: 1` on a 16px Liberation face is a **16px line box containing a 17px content area**.
Chrome floors the half-leading to `-1` and lets the inline overflow upward. The old code clamped it
at zero *and* took `max(line_height, ascent + descent)` for the line box — so a tight line came out
16px where Chrome says 14, and every tight paragraph on the page grew.

### Where it is stored, and why relative to the baseline

`TextFragment` carries `content_ascent` and `content_height`, and `rect()` derives
`y = baseline - content_ascent`. Storing an absolute top would have to be re-shifted by `translate`,
sticky positioning and scroll — three places that already move `baseline`, and one of them would
eventually be missed. Anchoring to the baseline makes the content area translation-invariant by
construction.

Per-**fragment**, not per-line: `<p>14px <em style="font-size:32px">x</em></p>` puts two runs on one
shared baseline with two different content areas, and Chrome reports each element its own.

### Measured effect

```
site            placement (within 8px)     median dy      median dh
old.reddit.com     17.6%  →  26.5%          60  →  12       0  →  0
en.wikipedia.org    7.2%  →   7.2%          45  →  45       4  →  0
G1 wiki snapshot   15.5%  →  15.5%          23  →  23       1  →  0
local probe        85.7%  →  100.0%          3  →   0       6  →  0
```

old.reddit's placement score moved half again — the first movement on the sweep's own metric in four
placement-targeted ticks — and the median `dh` went to **0 on every real page measured**, which is
the direct read of the fix. Wikipedia's *height* median went exact while its `dy` did not move, which
correctly separates this cause from the still-open sidebar-width narrowing (93px against Chrome's
186px) that dominates that page.

### The synthetic fragments that were riding on `line_height`

`rect()` reading `style.line_height` was load-bearing for something else entirely. Inline
padding/border **spacers** — and the empty fragment a bare `<br>` leaves — have no text and no font
(`ascent == descent == 0`) and exist only to carry an element's geometry, so they encoded their
height in `style.line_height` because that was the field `rect()` read. The content-area change made
every one of them report height 0; they fell out of `node_rects`' `width > 0 || height > 0` filter
and **vanished**, dropping G1 coverage from 100% to 67.8% (29 elements on news.ycombinator, 13 on
wikipedia).

A *placement* change caused a *coverage* regression in a gate that was not the target. The fix is a
named field — `LineFrag::report_h: Option<f32>` — rather than a font field doing double duty, so the
next change to `rect()` cannot silently delete these boxes again.

**Gate:** `inline_box_is_the_font_content_area_not_the_line_box` (manuk-layout). Proven RED on the
pre-fix code twice, on two different mechanisms independently: reverting `rect()` fails assertion 1
("got 25.6"), and reverting only the line-box `max` fails assertion 3 ("got 17, want 16"). The test
opens by asserting the installed face's content area is distinguishable from its 1.6 line box —
without that guard, a face where they coincide would make every later assertion vacuous.

## `text-transform: capitalize` titlecases the first LETTER of a word, not the first character (tick 412)

The capitalize pass cleared its "at word start" flag on **every** non-whitespace character, so any word
beginning with punctuation, a quote, or a digit lost its capital: `(hello)` stayed `(hello)`, `'twas`
stayed `'twas`, `3d` stayed `3d`. The CSS Text spec titlecases the first typographic **letter unit** of
each word — leading symbols are part of the word but are not the letter, so Chrome capitalizes past
them (`(Hello)`, `'Twas`, `3D`).

The fix stops clearing the word-start flag in the non-letter branch: leading punctuation/quotes/digits
pass through untouched and the flag survives until the first alphabetic char, which is titlecased and
only then clears the flag. Word boundaries stay whitespace-delimited (the documented common-case
approximation of UAX #29). Gated by `capitalize_skips_leading_punctuation_and_digits`, RED-proven
(restore the flag clear → `(hello) World`, not `(Hello) World`).

## `white-space: pre-wrap` PRESERVES spaces; `pre-line` COLLAPSES them — they shared one path (tick 413)

`pre-wrap` and `pre-line` were folded onto a single branch that, within each line, split on whitespace
into words separated by a single positional gap — i.e. it **collapsed runs of spaces**. That is right
for `pre-line` (preserve newlines, collapse spaces, wrap) and **wrong** for `pre-wrap`, whose defining
behaviour is that every space is significant (preserve newlines AND spaces, still wrap). So a
`<textarea>` (pre-wrap by UA default), an aligned ASCII table, or any "preformatted but still wrapping"
block reflowed into a single-spaced blob — the indentation and column alignment silently gone.

The inline model carries a space as a boolean `space_before` gap, not glyph text, so it cannot express
"three spaces" that way. The fix splits `pre-wrap` onto its own branch that emits each **maximal
whitespace run as its own measured `Word` token** (`space_before: false`, since the space is now
explicit), interleaved with the word tokens. N spaces stay N spaces, leading indentation survives, and
a soft wrap can still fall between tokens. `pre-line` keeps the collapse loop unchanged; `pre`, `normal`
and `nowrap` are untouched, so the blast radius is pre-wrap only. Gated by
`pre_wrap_preserves_spaces_while_pre_line_collapses`, RED-proven (route pre-wrap back through collapse →
`a   b` renders `ab`). Residue: trailing-whitespace *hanging* at a wrap boundary (pre-wrap lets trailing
spaces overflow rather than force a wrap) is not specially modelled — the run measures as a normal token.

## `text-indent` shifts the FIRST line box only — and it powers image replacement (tick 416)

`text-indent` was **unimplemented** — the string appeared only in a code *comment* (the
`text-indent:-9999px; font-size:0` image-replacement recipe it half-enabled). No `text_indent` field
on `ComputedStyle`, no Stylo map, no layout application. Two whole idioms silently no-op'd: prose
first-line indentation, and — more jarringly — the ubiquitous **image-replacement hack**
(`text-indent:-9999px` or `text-indent:100%` on logos and icon buttons), where "unhandled" does not
mean "no effect" but **duplicate text rendered at x≈0 on top of the background image**.

The value is an inherited length or %-of-containing-block, stored as `Dim` (so `%` resolves at layout
against the container width) and zoom-scaled. It maps through **both** cascades: `stylo_map` consumes
Stylo's `clone_text_indent().length` (the shipping path), and `MinimalCascade` parses it (the
layout-test + fallback path). Application lives in `layout_inline`: a `first_line` flag starts true and
flips false after the first `close_line`; while it is true the first fragment's inline-start `x`
becomes the indent and the first line's available width shrinks by it. A **negative** indent both
places the first glyph run off-screen-left *and* widens the available width, so the line never wraps
and sits entirely off-screen — exactly the image-replacement recipe. The key safety property: with the
default indent `0` the injected arithmetic is the IEEE identity (`x + 0.0 == x`, `w - 0.0 == w`), so
every existing line box is **byte-identical** — the path is inert until an author sets it. Gated by
`text_indent_offsets_the_first_line_only` (layout) + `text_indent_maps_through_the_stylo_cascade`
(cascade). Residue: the `hanging`/`each_line` keywords are accepted-and-ignored; anonymous mixed
block+inline runs (which already hardcode `align:left`) and form-control text pass indent 0.
