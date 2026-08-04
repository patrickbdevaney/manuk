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

## `-webkit-line-clamp: N` caps a block at N lines with a trailing `…` (tick 417)

The container half of the truncation idiom on nearly every card / product tile / article excerpt:
`display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:N; overflow:hidden`. stylo 0.19
gates `-webkit-line-clamp` to `engine="gecko"`, so the servo build never parses it — and `display:
-webkit-box` is likewise gecko-only, so a `<div>` carrying the idiom simply stays a **block** (its UA
default) and flows its text as normal block content. That is the lucky break: no `-webkit-box`
formatting context is needed for the real-world single-text-run case, so the whole feature reduces to a
post-layout truncation of the block's line boxes.

`line_clamp: Option<u16>` is a **non-inherited** box property (so it never leaks to descendants),
parsed in MinimalCascade and recovered into the shipping cascade through the same MinimalCascade merge
that carries `object-fit`/`text-overflow`/`visibility` (all of them gecko-modelled types stylo won't
surface). In the block-with-inline-children layout path, after the line boxes are built,
`apply_line_clamp` groups fragments by their shared `line_top`, keeps the first N lines, drops the rest,
and **unconditionally** forces an ellipsis onto line N — unconditional because content genuinely
continued past it (that is why there were extra lines), unlike single-line `text-overflow` which only
fires on an actual overflow. The clamped box height (bottom of line N) is returned as `h`, so siblings
below reflow up. Guarded by `overflow-y ≠ visible` (the idiom always sets `overflow:hidden`) and by
`line_clamp` being set at all — an unclamped page never enters the branch, so it is byte-identical.
Gated by `line_clamp_caps_lines_and_appends_ellipsis` (layout, RED-proven: 6 lines → 2 + `…`) and
`line_clamp_recovers_through_the_stylo_cascade` (shipping-path recovery). Residue: the `line-clamp`
shorthand's `<block-ellipsis>`/`continue` parts are ignored (bare integer only); a clamped block whose
children are themselves blocks (not the common all-inline excerpt) is not handled; true old-flexbox
`-webkit-box` child layout is out of scope.

## The `ch` unit is the font's real `0`-advance, not the `0.5em` fallback (tick 499)

`ch` is *the advance of the `0` glyph*. In the shipping Stylo cascade the `Device`'s
`FontMetricsProvider` (`StubFontMetrics` in `stylo_engine.rs`) returned `FontMetrics::default()` —
every field `None` — so Stylo took the spec's *"impossible to determine"* branch
(`zero_advance_measure_or_default`) and used `ch = 0.5em`. Meanwhile layout laid the text into the box
with the font's **true** advance from the shaper (a monospace `0` is ~`0.6em`), so a `width:10ch` box
came out `80px` at `16px` while its ten monospace chars measured `~96px` and overflowed. `max-width:65ch`
— the readable-column idiom on essentially every article — was ~17% too narrow everywhere.

**Fix (a css↔text seam, not a constant).** A constant `0.6em` cannot be trusted to the pixel: the test
`line-break-ch-unit` and every `Nch` box require `N` chars to occupy *exactly* `N·ch`, which only holds
if the metric is the SAME number the shaper places glyphs with. So `manuk-text` exposes
`zero_advance_px(families, bold, italic, size_px)` — it resolves the family exactly as
`layout::text_style`'s `FontKey` does (`resolve_family` + `weight≥600` + `italic`) and returns
`measure("0", key, size)` off a **thread-local** `FontContext` (system-font scan paid once per thread,
then a `measure_cache` hit). `query_font_metrics` extracts the family list from the Stylo `Font`
(mirroring `stylo_map`'s `font-family` extraction), calls it, and returns
`zero_advance_measure: Some(len)`. `manuk-css` gains an **optional, `--features stylo`-gated**
dependency on `manuk-text` (workspace-internal; `manuk-text` does not depend on `manuk-css`, so no
cycle; no new external crate — I2 holds).

**Scope / honesty.** Only `zero_advance_measure` is filled; `x_height`/`cap_height`/`ic_width` stay
`None`, so `ex = 0.5em`, `cap = ascent`, `ic = 1em` are unchanged — `ex` real metrics are a bounded
follow-up, and nothing that worked before changes. The thread-local context carries **system + generic
faces only**, not the page's `@font-face` registrations: `ch` is exact for generic/installed families
(the gate + `line-break-ch-unit` use `monospace`), and an unregistered webfont name falls through the
generics to a real fallback-font advance — closer to Chrome (which uses the fallback's `0` when the
webfont is absent) than the old flat `0.5em`, never a regression. Threading the page's own
`FontContext` in (for webfont-exact `ch`) is the next step. Gated by `g_ch_unit` (RED-proven: stub gives
`box:80 ref:96 eq:false real:false`; fix gives `box:96 ref:96 eq:true real:true`).

## The `ex` unit is the face's real x-height (OS/2 sxHeight), not `0.5em` (tick 500)

The sibling of the `ch` fix (tick 499), on the same `manuk-css`↔`manuk-text` seam. `StubFontMetrics`
returned `x_height: None`, so Stylo used the spec `ex = 0.5em`. Real faces sit slightly over half an em
(DejaVu/Liberation sans ≈ `0.52em`), so `ex`-sized boxes were a few percent too short — cumulative in a
form or an icon column sized in `ex`. `manuk-text::x_height_px` reads the face's **OS/2 `sxHeight`** via
`swash::FontRef::metrics(&[]).scale(size).x_height` — the same design-unit value Chrome uses — off the
same primary face the shaper draws with, and the provider returns `x_height: Some(len)`. `None` (a face
with no declared x-height, or an unresolved family) leaves the spec `0.5em` fallback untouched, so
nothing regresses. `cap_height`/`ic_width` stay `None` (both freely available from the same swash
`Metrics` — `cap` and `ic` units are the bounded next step). Gated by `g_ex_unit` (RED-proven: stub
gives `ex100:800`; fix lands ~`832`, pinned to `(810,900)` so a wrong metric — cap-height ~1150,
a whole em 1600 — fails instead of passing).

## The `cap` unit is the face's real cap-height — it used to collapse to 0px (tick 502)

Third unit on the tick-499 `manuk-css`↔`manuk-text` metrics seam, and the one that was most broken.
`cap` is the cap-height (height of a flat-topped capital). Stylo's fallback for a `None` `cap_height` is
the font's **ascent** — but the provider never set `ascent` either (it defaulted to `0`), so `cap`
resolved to **0px** and any `cap`-sized box collapsed to nothing (worse than `ch`/`ex`, which at least
had a `0.5em` fallback). `manuk-text::cap_height_px` reads the face's OS/2 `sCapHeight` via
`swash::FontRef::metrics(&[]).scale(size).cap_height` (the value Chrome uses); the provider returns
`cap_height: Some(len)`. `None` (a face with no declared cap-height) still leaves the ascent fallback.
`ascent`/`ic_width` remain unset — `ic` (ideographic advance) is intentionally not filled: it is ≈`1em`
for any face, so it cannot cleanly diverge from its own `1em` fallback and would make a non-falsifiable
gate. Gated by `g_cap_unit` (RED-proven: stub gives `cap100:0`; fix lands ~`1150`, pinned to
`(900,1600)` so a wrong/larger metric fails). This closes the *clean* font-relative-unit work (`ch`,
`ex`, `cap` real; `ic` measured-and-pinned as fallback-correct in `CONSTELLATION.tsv` tick 501).

## No named font family ever resolved — `fontdb`'s name query is case-SENSITIVE (tick 557)

Five ticks of instrument work (t551–t556) narrowed a corpus-wide text divergence to one question, and the
answer is three characters wide: `resolve_family` lowercased the family name before handing it to
`fontdb::Family::Name`, and **that query is case-sensitive.**

```
Family::Name("DejaVu Sans")  ->  Some("DejaVu Sans")
Family::Name("dejavu sans")  ->  None
```

So `matched` was **always false** for any family whose real name is not entirely lowercase — which is
essentially every font on every system. Every named family on the web fell through to the
`contains("mono")` / `contains("serif")` hints, or to `sans-serif`. Measured against Chromium on one
44-character string before the fix:

| declared | Chromium | ours |
|---|---|---|
| `"DejaVu Sans"` | 374px | **330px** |
| `"Noto Sans"` | 348px | **330px** |
| `"DejaVu Serif"` | 380px | **330px** |
| `"Liberation Mono"` | 422px | **330px** |
| `"NoSuchFontXYZ"` (absent) | 299px | **330px** |

**Two real families and a deliberately fake one, all the same width.** That is the tell for a resolution
failure rather than a metrics one, and it is why the generic stacks (`sans-serif`, `serif`, `monospace`)
measured *fine* the whole time — they never take the named path.

**Why this one defect produced two different corpus symptoms**, which is the test a root cause has to pass:
a substituted face has different **per-glyph advances** (so text widths are wrong in *both directions*
depending on the string — the ±9–22px sign-changing anchor widths) and a different **ascent+descent** (so
the line box is off by a *constant* — the +2px height on every instance). One cause, both signatures, no
residue.

**The fix:** `orig` (trimmed, unquoted, original case) goes to fontdb and is what gets interned; the
lowercased form stays the key for the generic keywords and the `@font-face` map, which is keyed lowercase
because CSS family matching is case-insensitive. `face_id` lowers the interned name again for its webfont
lookup — miss that and a webfont declared `"Fira Sans"` stops resolving, trading one bug for another.

RED-PROVEN: `a_named_installed_family_resolves_to_that_family_not_a_fallback` enumerates the mixed-case
families actually installed on the box and asserts each resolves to `Named(that family)`; restoring the
lowercase in the query fails it with *"AR PL KaitiM Big5 IS installed on this box and must resolve to
Named(...), not SansSerif"*. It also asserts an absent family still falls back, so the fix cannot be "call
everything a match".

### The SECOND defect, one line later: `intern_family` discarded the case (tick 558)

Resolution is fixed and verified — a trace at the layout call site shows
`["DejaVu Sans"] -> Named(0)`, `["Noto Sans"] -> Named(1)`, `["DejaVu Serif"] -> Named(2)`,
`["Liberation Mono"] -> Named(3)`, `["NoSuchFontXYZ"] -> SansSerif`: five declarations, five distinct
outcomes. **And the rendered widths are still 330px for all of them.** So the family now resolves and the
*advance* does not follow — `face_id`/`load`, or the measurement path, is not using the resolved face.
**Found and fixed at t558, and it is one line away from the first fix.** `intern_family` stored the
**lowercased** key in `family_names`, so `family_name_of` handed `face_id` a lowercase string,
`fontdb::Family::Name` missed *again*, and every named family fell back to `Family::SansSerif`. Dedup must
be case-INSENSITIVE — CSS family matching is, so `font-family: ARIAL` and `font-family: Arial` have to
intern to one id — but **storage must be case-PRESERVING**, and this function was doing only the first job.

**A fix upstream of a lossy step is not a fix.** t557's assertion lived at the resolution layer and could
not see this: five families, five distinct `Named(...)` ids, one `FaceId(0)`, one width. So the t558 test
measures the **WIDTH** — `distinct_named_families_measure_distinctly` enumerates the mixed-case families
actually installed on the box (417 here), requires more than one distinct `FaceId` *and* more than one
distinct measured width, and fails with *"every one of 417 installed families resolved to the SAME face"* if
the lowercase comes back. **Assert on the observable, not on the intermediate** — the intermediate was
already correct and the observable was not.

Measured end-to-end on the committed probe against live Chromium: **SHAPE 36.4% → 90.9%**, misplaced spans
**5 of 5 → 1 of 11**, and `"DejaVu Sans"` / `"Noto Sans"` / `"DejaVu Serif"` / `"Liberation Mono"` all land
within the 8px tolerance where they had shared one width.

**One residual, named rather than folded in:** `"NoSuchFontXYZ"` — for an unknown family Chromium falls back
to a *serif* default (299px) and we fall back to *sans* (330px). That is a default-family divergence, not a
resolution one. [[box-layout]]

## `@font-face` shadows a same-named local face — a failed download must look failed (tick 561)

CSS Fonts: once a document declares `@font-face { font-family: "Open Sans" }`, a locally-installed
`Open Sans` is **shadowed for that document**. If every `src` in the rule fails to load, the family yields
**no usable face** and matching continues to the **next entry in the `font-family` list** — it does *not*
fall back to the same-named local font.

We used to fall back to it, and the bug only became reachable once t557/t558 made named families resolve at
all: before that, everything fell to a generic and the question never arose. So `declare_webfont_family` is
called for **every** `@font-face` rule *before* the fetch is attempted — the rule is about the **declaration**,
not the download — and `resolve_family` skips a declared family with no loaded face rather than querying the
system for it.

**Why "declare first, fetch second" is the whole design:** declaring only on success would make a failed
download indistinguishable from a font we simply do not have, and the failure mode is not a missing glyph —
it is *a different font, silently*, which measures as a page-wide metrics divergence and reads as a layout
bug. RED-PROVEN: `a_declared_webfont_family_shadows_the_local_face_of_the_same_name` takes a family the box
actually has, asserts it resolves locally when undeclared, declares it with nothing loaded, and asserts the
resolution falls through to `sans-serif`; removing the shadowing check fails it.

⚠ **Honest scope: this is a spec fix, not the fix for the site that motivated it.** t560 diagnosed
`martinfowler.com`'s 68.2% → 49.2% SHAPE as a masked webfont failure. **It was not** — that site has no
webfont `<link>` for Open Sans at all; it names `Open Sans, sans-serif` and uses the local install, the same
face Chromium uses. With the sizes now correct (`dw=1 dh=2` where they had been ±9–22px), the page turns out
to be **displaced by dy≈82px**, and the earlier sizing error had been partially *compensating* for it.
**A score can fall because a confound was removed** — which is the third time in this arc that a mechanism
fitting the numbers was not the mechanism, and each time the fix was to read the page instead of the
distribution. [[box-layout]]

## A downloaded web font is a THIRD reason to re-lay-out (tick 619)

`fetch_and_apply_stylesheets` re-cascades on two conditions:

```rust
if count > 0 || self.dom.has_dirty() {   // external CSS arrived, or a script mutated the tree
```

Both are right, and the guard earns its keep — an unconditional re-cascade is 257ms a go on
bbc.co.uk, and the `else` branch was added because a relayout that cannot change the output is waste
with a safety story attached. But it lists **two** reasons and there are **three**: a `@font-face`
face that has just been registered changes the advance of every glyph in the document, so every line
box, wrap point and content height computed with the fallback is stale.

**An optimisation guard is only as correct as its list of inputs.**

### Every layer below it was already correct

Bisected with the WPT `Ahem` face, whose every glyph is exactly 1em wide — so `XXXXX` at 20px is
exactly 100px and no fallback can produce that number by accident:

| layer | result |
|---|---|
| WOFF2 decode (`manuk_text::decode_webfont`) | ✓ 7 tests green, incl. this fixture |
| `register_named_font` + `resolve_family` | ✓ face count +1, resolves `Named(_)` |
| `FontContext::measure` | ✓ **exactly 100.0** |
| the fetch (local server log) | ✓ `GET /ahem.woff2` |
| **the document's layout** | ✗ **66.7px** — the fallback |

The font downloaded, decoded, registered and resolved, and nothing asked the page to use it.

### ⚠ The blast radius is smaller than it looks, and it was measured

`count` is `external.len()` — **all** external stylesheets, not only those containing `@font-face`. So
any page with a single external sheet already took the relayout branch. Measured across the HEAD
corpus:

```text
  10 of 16 sites use @font-face
  10 of those 10 have it in an EXTERNAL sheet
   1 has one inline (www.welt.de) — and its fidelity is byte-for-byte unchanged by this fix
```

The broken path needs a page whose `@font-face` is inline **and** which has no external stylesheet at
all. **Measured impact on this corpus: zero sites.** The fix is correct and stays; the honest value of
the tick is the *gate*, since the map had claimed `works` on nothing.

### The fix must not reintroduce the cost it is fixing

`fetch_and_apply_stylesheets` runs again after **every** round of dynamic scripts. Setting
"a font arrived" on each successful fetch therefore re-registers the same face every round and forces
a full-document relayout every round — exactly the waste the guard exists to prevent.
`FontContext::has_webfont_face()` skips a family that already has a registered face, so the second
round does no fetch and forces no relayout.

### What is still unmeasured

`font-display` (FOUT/FOIT swap behaviour), `unicode-range` subsetting, and the `Lora`-shaped finding
from t563 — Chromium resolving a webfont where we fall back to `serif`, on a page whose sheet was
**external** and therefore not explained by this tick. The map row should read `partial`, not `works`.

## The line box has no STRUT, and that is the `dy` term (tick 690)

CSS 2.1 §10.8: *"each line box starts with a zero-width inline box with the element's font and line height
properties — the strut."* `close_line` folds `ascent`, `descent` and `line-height` over **the fragments
present**, and an atomic or synthetic `LineFrag` carries `ascent == descent == 0` by construction. So a line
whose only content is an `<img>` has **zero descent**, and nothing is reserved under the baseline.

Measured on headless Chrome and here, same fixture, `margin:0; font:16px/normal sans-serif`, a 40×40 broken
`<img>`:

```text
                                        Chrome   ours
  div > img  (default = baseline)         h=44    h=40    <- the 4px strut descent
  div > img  vertical-align:top           h=40    h=40    ✓
  div > img  display:block                h=40    h=40    ✓
```

**`top` and `block` already agree, which localises it exactly:** the atomic is *placed* correctly
(`VerticalAlign::Baseline => baseline - h`); the **line** is not opened far enough to hold what sits under
the baseline. A baseline-aligned atomic's bottom sits ON the baseline, and the baseline is not the bottom of
the line box.

This is the `dy` term tick 688 identified — correctly-sized boxes in the wrong place because something above
them is too short — and it fires on **every baseline-aligned inline image on the web**: icons, logos,
avatars, spacer gifs. On tick 689's fixture it accumulated to **32px over four images**.

⚠ **The obvious fix is wrong ALONE and was tried:** `atomic_h + descent` changes nothing on its own, because
`descent` is 0 on exactly the lines that need it.

### FIXED at tick 691 — two changes, one behaviour

1. **The strut.** `layout_inline` takes the containing block's `ComputedStyle` and folds its metrics into
   every line box as a zero-width fragment — through **`text_style`**, not the raw `ComputedStyle`, because
   that is the one function that resolves a family list to a `FontKey` and `line-height: normal` to a number.
   A strut resolved differently from the fragments would compare two notions of the same font. A caller with
   no block style passes `None` and gets a zero strut — exactly the old behaviour, so no call site changes
   meaning by accident.
2. **A baseline-aligned atomic demands `height + descent`**, because its bottom sits ON the baseline.

```text
                                    Chrome   before   after
  div > img  (default = baseline)     h=44     h=40     h=43
  div > img  vertical-align:top       h=40     h=40     h=40   <- guard
  div > img  display:block            h=40     h=40     h=40   <- guard
  p  (a plain text line)             --        h=18     h=18   <- guard
  parity                             --      72/72    72/72    <- 30 pages, the wider net
```

The **1px** residual against Chrome is a FONT-descent difference — our `sans-serif` resolves to a different
face than the reference Chrome's — not a logic one, and it sits inside the 8px SHAPE tolerance the
certificate scores on, where 4px of missing descent *per inline image* did not.

⚠⚠ **Neither half can be removed as dead code**, which is why the gate asserts the combination: tick 690
tried half 2 alone, measured no change, and reverted it — correctly, on the evidence available then. And the
three guards are load-bearing, not decoration: `top`, `block` and plain text already agreed with Chrome, so a
fix that opened *every* line box by the descent would move them too, in a way a single assertion on `w1`
could not see. `parity` was run before landing rather than discovered by the wall, because this is the
function that computes every line box in the engine.

[[subpixel-error-compounds]] [[box-layout]]

## The strut's cost: `desitales2` dy 91 → 110, and `line-height: normal` is exonerated (tick 693)

Tick 692's proving sweep showed the strut moved `dy` the right way on three sites and the **wrong way on one**:
`www.desitales2.com`'s median dy went **91 → 110**. Two things had to be established before attributing it.

**1. Is 19px a result at all?** `desitales2` is the byte-reproducible control, but `dy` — unlike SHAPE — has
never had a measured error bar. Two consecutive runs on the current tree:

```text
  run 1   SHAPE 60.6%   median dx=0 dy=110 dw=0 dh=3
  run 2   SHAPE 60.6%   median dx=0 dy=110 dw=0 dh=3
```

**Deterministic.** So the 19px is real and this session caused it. (SHAPE went 61.1% → 60.6%, −0.5 pts, which
is inside that site's recorded 2.3-pt spread — the dy is the sharper instrument here, and it is the one with
no noise.)

**2. Is it the strut's `line-height` half?** The strut folds in the block's `line-height`, so a
`line-height: normal` that resolved larger than Chrome's would inflate every line. Measured, both engines:

```text
             Chrome   ours
  16px sans    18       18
  16px serif   18       18
  13px sans    15       15
  17px -apple-system  20   20
```

**Exact agreement — exonerated.** The remaining candidate is the *descent* half applied to this site's
specific baseline-aligned atomics: either our descent for its faces differs from Chrome's, or some of those
images are not inline atomics in Chrome's box tree at all (a flex item, or a CSS `vertical-align` this fixture
did not cover). That is the next probe, and it is narrow.

⚠ **The trade, stated plainly:** the strut is +2.9 coverage points on `ikea`, −45px dy on `keirin`, −120px on
`welt.de`, +5.2 shape points on `agoda`, and **+19px dy / −0.5 shape points on `desitales2`.** No certificate
term regressed (`scored 5`, `shape ≥0.75 on 0`, both unchanged), so this is not a ratchet trade — but it is
not free either, and a lever that helps three sites and hurts a fourth has a second mechanism in it.

[[box-layout]] [[subpixel-error-compounds]]

## The half-leading belongs to each INLINE BOX, not to the line (tick 695)

CSS 2.1 §10.8 builds a line box from **two maxima taken about the baseline** — `max(distance above)`
and `max(distance below)` over every inline-level box, each box having already added **its own**
leading to its own font's ascent and descent. We did something that looks equivalent and is not: fold
`max(ascent)`, `max(descent)` and `max(line-height)` over the line, take
`line_h = max(line-height, tallest atomic)`, and then **centre the content area inside the result**.

On a line whose tallest box is the one carrying the leading, those two agree *exactly*. That is why
this survived 690 ticks: a plain paragraph — the overwhelming majority of lines on the web — comes out
byte-identical either way. They diverge the moment the tallest box on the line is an **atomic**, and
then the whole line is displaced, text included.

**Chrome-measured** (`--headless=new --dump-dom`, 1280×800, `margin:0`, `16px/normal sans-serif`, a
40×40 `<img>` followed by a `<span>`; every number relative to the div's own top):

```text
                                         Chrome   before   after
  line-height:60px      — the div          h=65     h=60     h=65
                        — the img top        0        8        0
                        — the span top      26       34       26
  vertical-align:top    — the span top       0       24        0
  vertical-align:bottom — the span top      22        0       22
  vertical-align:middle — the span top      10       16       10
  (guards)
  line-height:normal    — the div          h=44     h=43     h=44
                        — img / span top  0 / 26   0 / 26   0 / 26
  a span alone          — the div          h=18     h=18     h=18
```

Twenty-two of twenty-two probed boxes now match Chrome exactly. **The 1px on the `line-height:normal`
row was not a font difference** — tick 691 recorded it as one ("our `sans-serif` resolves to a
different face"), and it was the half-leading's rounding remainder: keeping `above + below ==
line-height` exactly for the strut puts it back where Chrome puts it.

### `top` and `bottom` are opposites, and only a fixture carrying both can see it

Both are aligned to the **line box's own edges**, which do not exist until everything else has been
placed, so both are applied after the baseline-relative maxima and both can only make the line taller.
But `top` grows it **downward** — the baseline stays where the strut put it — while `bottom` grows it
**upward**: the image pins the line's bottom edge, and the strut's descent still has to fit *under the
baseline* above that edge, so the baseline moves down and the text with it. The first version of this
fix treated them alike, passed the `top` row, and left `bottom` **22px** out.

⚠ **Their heights are identical** (both produce a 40px line box) and they differ only in where the
text inside sits, so a height assertion cannot gate this. The gate asserts positions.

### What it is worth, and the honest ledger

`img { vertical-align: middle }` and `vertical-align: bottom` are CSS-reset material and
`line-height` + an inline image is the ordinary shape of a nav bar, a card, a byline and an avatar
row — so this is ordinary-page geometry, not a corner case.

```text
  parity                          72/72 across 30 pages   (unmoved)
  layout suite                    91/91
  desitales2 (byte-reproducible control)
      structural coverage    98.7% -> 98.7%   (597 paths, 8 missing — IDENTICAL set)
      misplaced                582 ->   582   (identical)
      SHAPE                   60.6% -> 60.6%  (identical)
      absolute median dy         110 ->  127
  keirin.jp   absolute median dy  161 ->  124     SHAPE 58.8% -> 56.9%
  www.welt.de absolute median dy 2957 -> 2950
```

⚠ **The two live movers on `dy` point opposite ways, and `dy` is the metric SHAPE was built to
replace** — `placement_stats` charges one root cause N times, so a single container displaced at the
top of the page moves it by that container's error on every element below. keirin's SHAPE fall of
1.9 points sits inside that site's *recorded 3.7-point spread on an unchanged tree*, so it is not a
result either. What is not noise is the control: **every certificate term on `desitales2` is
identical, down to the missing-element set.**

⚠ Still approximate, measured here and left alone: `vertical-align: middle` resolves x-height as
`ascent / 2` where the real face is nearer `0.52 × em` (2px on this fixture), and `sub`/`super` use
0.15/0.35 constants (1–2px). All inside the 8px SHAPE tolerance, all wanting real font metrics
plumbed to `close_line` — which is a separate tick.

[[subpixel-error-compounds]] [[box-layout]]

## A family declares four faces; the loop fetched one, and resolved its URL against the wrong document (tick 747)

Two information losses in one ~20-line block (`engine/page/src/lib.rs`, the "Web fonts" section of
`fetch_and_apply_stylesheets`). Both were found by READING the block, not by a sweep row, and both
present identically to a user: *the page is in a different font and nothing says so*.

### A — the idempotence key was the FAMILY, so only the first weight ever loaded

```rust
fonts.declare_webfont_family(&ff.family);
if fonts.has_webfont_face(&ff.family) { continue; }     // <- wrong grain
```

A real site does not declare one `@font-face` per family. It declares one **per weight and style**,
all under one name — this is verbatim from `a11yproject.com/css/screen.min.css`, and it is what
every "self-host your Google font" download emits:

```css
@font-face{font-family:Noto Serif;font-weight:400;font-style:normal; src:url(…regular.woff2)}
@font-face{font-family:Noto Serif;font-weight:400;font-style:italic; src:url(…italic.woff2)}
@font-face{font-family:Noto Serif;font-weight:700;font-style:normal; src:url(…700.woff2)}
@font-face{font-family:Noto Serif;font-weight:700;font-style:italic; src:url(…700italic.woff2)}
```

The first block registered; the other three hit `continue` and were **never fetched**. Every bold and
every italic run on such a page was then measured and painted in the **regular** face — and there is
no synthetic bold anywhere in `engine/text`, so the advances were byte-identical to regular text, not
merely a bit light. Measured on the gate fixture, "Handgloves" at 40px: **211.25px (regular) where
the real bold face is 226.72px** — a 7% width error on every bold run, which re-wraps prose and turns
into a whole-line height cascade.

⚠ **The consumer was already built and could never fire.** `FontContext::face_id` searches the
family's registered ids for the matching weight/style — *"picking the bold/italic variant when
present"*, in its own comment — over a `Vec<ID>` that `register_named_font` extends. Storage and
selector both finished; the producer delivered exactly one face, forever, so the search was dead code
and the `ids.first()` fallback was the only reachable path. *The orphaned-reader shape: when a
capability looks half-built, check which half.*

**The guard's purpose was real; only its grain was wrong.** `fetch_and_apply_stylesheets` re-runs
after every round of dynamic scripts and a newly registered face forces a full-document relayout, so
an unkeyed loop costs a relayout per round. The key is now `(family, the block's first src URL)`:
stable across re-runs (same idempotence) and distinct per face (all four load).

Three details that are each a bug if got wrong:

- **Key the BLOCK, not each src.** A block lists one face in several formats
  (`url(x.woff2), url(x.woff)`) and stops at the first that works. Claiming per-src leaves `.woff`
  unclaimed, and the next script round fetches the fallback format of a face that already loaded.
- **The family belongs in the key.** Two families may legitimately point at the same file (an alias,
  a `Foo`/`Foo Text` pair). URL-only keying silently starves the second one — caught by the fixture,
  which reaches the same bold bytes through a second single-face family.
- **Claim on the ATTEMPT, not on success.** A 404 retried every script round is the same per-round
  cost the key exists to prevent.

### B — a relative `src` resolved against the DOCUMENT, not the stylesheet

```rust
let url = resolve_url(&self.final_url, src);    // self.final_url is the DOCUMENT
```

CSS Values §4.2: a relative URL in a stylesheet resolves against **the stylesheet's** URL. The
enclosing loop iterates `sources` and literally holds it (`StyleSource::External(url, _)`), then
discards it one line later.

```text
  sheet                      src                    CORRECT                 WAS
  /css/screen.min.css        url(../fonts/x.woff2)  /fonts/x.woff2          /fonts/x.woff2   (agrees)
  /assets/css/main.css       url(../fonts/x.woff2)  /assets/fonts/x.woff2   /fonts/x.woff2   404
```

The first row is why this survived: a site with `/css/` at the root resolves the same both ways, so
the nearest real page (a11yproject) is a **coincidence, not a confirmation**. The second row is the
standard Jekyll/Hugo/webpack output. An inline `<style>` correctly keeps the document as its base.

**Gate.** `engine/page/tests/g_webfont_family_weights.rs` serves the sheet from `/assets/css/` and
**404s everything under `/fonts/`**, so B cannot pass by accident, and asserts `#bold.width ==
#ctl.width != #reg.width` where `#ctl` is the same bold bytes reached through a one-face family — a
control built from the fixture rather than a hard-coded metric. RED-proven twice: **M1** restore the
family-grained key (bold measures 211.25, the regular face); **M2** restore the document-relative
base (everything 404s, all three spans equal, the vacuity guard fires first).

## `system-ui` is a different font from `sans-serif`, and an alias at the FRONT of a stack decides the whole stack (tick 749)

`resolve_family` walks the author's `font-family` list and returns on the first entry it can satisfy.
Five names shared one match arm:

```rust
"sans-serif" | "system-ui" | "ui-sans-serif" | "-apple-system" | "blinkmacsystemfont"
    => return FontFamily::SansSerif,
```

Two independent defects sit in that line.

### 1. The two generics need two different faces, and this file already said so

`resolve_generic_families` points `sans-serif` at **Arial → Liberation Sans**, and its comment explains
why that is not fontconfig's answer: *"Chrome never asks fontconfig for the bare generic. It asks for
its own default family, Arial, and fontconfig substitutes the metric-compatible Liberation Sans… Noto's
line box is 1.362em against Liberation's 1.150em, an 18% error on the height of every line on every
page."*

`system-ui` is the other thing — the **platform UI font**. `fc-match system-ui` answers **Noto Sans**
here, and Chromium's `system-ui` measures exactly Noto Sans. So the alias was inflicting precisely the
error the sans list was written to avoid, and `LineMetrics::height`'s verification table had both
numbers already:

```text
                  ascent  descent    gap     sum    → us   Chrome
Liberation Sans   14.484    3.391  0.523  18.398      18     18
Noto Sans         17.104    4.688  0      21.792      22     22
```

**Every line of a `system-ui` page was 4px short.** A line height is a `dy` term: the error is not
per-page, it is per-line, and it accumulates down the document.

Because `fontdb` has no `set_system_ui_family`, the resolved name lives on the `FontContext` and the UI
font is reached as `FontFamily::Named` — no new enum variant, so `FontKey`, the paint path and the three
caches are untouched.

⚠ **Intern the ORIGINAL case.** `face_id` re-queries fontdb with the interned string and
`fontdb::Family::Name` matching is case-SENSITIVE (tick 557). Lowercasing the UI family name here
resolves `system-ui` to *nothing* — the same miss the case fix removed, one call later.

### 2. An early match discards the rest of the author's list

This is the larger half. Each of those five names is written **first** in a real stack, so answering
"sans generic" there ends the search — the family Chrome actually picks is never reached. Measured
against live Chromium, `16px "source"`:

| stack | Chrome | was |
|---|---|---|
| `system-ui` | `50.23x22` (Noto Sans) | `48x18` |
| Bootstrap 5 `system-ui,-apple-system,…` | `50.23x22` | `48x18` |
| GitHub `-apple-system,BlinkMacSystemFont,"Segoe UI","Noto Sans",…` | `50.23x22` (**4th** entry) | `48x18` |
| Tailwind `ui-sans-serif,system-ui,sans-serif` | `50.23x22` (2nd entry) | `48x18` |
| Bootstrap 4 `-apple-system,…,Roboto,"Helvetica Neue",…` | `48.34x19` (**Roboto**) | `48x18` |

⚠ **`-apple-system` and `BlinkMacSystemFont` are Blink's macOS-only aliases for San Francisco.** They
name nothing on Linux; Chrome treats them as unknown and moves on, which is exactly why Bootstrap 4
lands on Roboto rather than on a sans generic. They therefore get **no arm at all** — they fall through
the named-family path, fail to match, and continue. On macOS the right answer is the system UI font, so
this wants a platform-conditional alias; the Linux answer is the one that is measured.

Bootstrap 4 reaching Roboto is the only assertion that proves the short-circuit is gone; the `system-ui`
assertions all pass with the short-circuit intact for the other four names.

### The reusable rule

**A `font-family` list is a fallback chain, and an over-eager match arm does not mis-resolve one family
— it makes a decision on the author's behalf about every entry that follows.** Each of these five names
looked harmlessly approximate on its own line, and the arm was wrong on 100% of the real stacks tested.
The same shape exists anywhere a lookup walks an author-ordered list of alternatives and a default is
returned from inside the loop.

**Residue, named:** a stack where *nothing* matches still falls back to sans here, where Chrome uses its
**standard font** (Times → Liberation Serif) — `"Segoe UI"` alone is `48x18` for us and `41.77x18` for
Chrome. Pre-existing, a different primitive, and its own tick.

## The collapsible set is CSS's, not Unicode's (tick 759)

White-space processing — collapsing runs, trimming edges, choosing soft-wrap opportunities — applies to
exactly five characters (CSS Text 3 §3, §4.1):

```
SPACE U+0020 · TAB U+0009 · LINE FEED U+000A · CARRIAGE RETURN U+000D · FORM FEED U+000C
```

`char::is_whitespace` implements the **Unicode `White_Space` property**, which is strictly larger. The
extra members are not exotic — they are the characters an author picks *precisely because they must not
collapse*: `U+00A0` NO-BREAK SPACE (`&nbsp;`), `U+2007` FIGURE SPACE, `U+202F` NARROW NO-BREAK SPACE, and
`U+2000`–`U+200A`.

All three collapse sites in `engine/layout` used it, so `&nbsp;` was collapsed and trimmed like a space.
The visible consequence was not a slightly-wrong width — an element whose only content was `&nbsp;` was
left with **no text, therefore no line box, therefore height 0**:

```
<div>&nbsp;</div>        Chrome 18      was 0
a&nbsp;&nbsp;&nbsp;b     Chrome 48      was 29   (collapsed to a single space)
a   b                    Chrome 29          29   (ASCII — must still collapse)
```

That last row is the other half of the rule and belongs in any gate for this: a fix that simply stopped
collapsing would be a worse bug than the one it replaced.

`text-transform: capitalize` deliberately keeps `is_whitespace` for its word-start scan — *a word
boundary is a different question from a collapsible character*, and NBSP genuinely is a word separator
for that purpose.

### The rule

**A standard-library predicate that almost means what the spec means is a bug that reads as correct
code.** `is_whitespace` is not sloppy; it is a precise implementation of a *different* specification, and
the difference is exactly the interesting cases. Nothing looks wrong at the call site, no test over ASCII
input can catch it, and the failure appears only for characters an author chose deliberately.

Grep the class — `is_whitespace`, `is_alphanumeric`, `to_lowercase`, `trim()` — each has a CSS/HTML
definition that differs from Unicode's at the edges, and the edges are where authors live.

## A line box with no content-bearing member does not exist — and the spec's own sentence is too wide (tick 761)

`<div><span></span></div>` was **19px tall here against Chrome's 0**. The strut (§10.8, tick 690) is
folded into *every* line box unconditionally, so any wrapper whose inline content collapsed to nothing
still got a full line height. CSS 2.1 §9.4.2 says it should not:

> *"Line boxes that contain no text, no preserved white space, no inline elements with non-zero margins,
> padding or borders, and no other in-flow content … must be treated as zero-height line boxes for the
> purposes of determining the positions of any elements inside of them, and must be treated as not
> existing for any other purpose."*

**The rule is about the LINE, not about the empty inline** — and that distinction is the whole tick. An
empty inline *sharing a line with text* keeps a real rect: Chrome reports the span in
`<div>text<span id=s1></span>text</div>` as **17px tall**, and fragment anchors, scroll-spy targets and
`getBoundingClientRect` on a marker span depend on it. That is exactly what `InlineItem::Spacer` was
built for, and its comment said so with a Chrome citation. The blunt fix — *"an empty inline generates no
line box"* — closes the corpus symptom and regresses the anchor case. The implemented predicate is
`any(content_bearing)` over the line's fragments, and the reporter fragments are still **emitted at zero
height** rather than dropped, so the element stays in `node_rects` (dropping them would trade a placement
error for a coverage one — the regression `LineFrag::report_h` already documents).

### Chrome is NARROWER than its own spec text, and only the measurement says so

§9.4.2's escape hatch reads *"no inline elements with non-zero margins, padding or borders"*, which
invites the general test. Measured against live Chromium, `body{margin:0;font:16px/normal sans-serif}`,
reading the **div's** height:

```text
  <div><span style="padding:4px">      </span></div>   18   <- 4px of it is HORIZONTAL
  <div><span style="padding:4px 0">    </span></div>    0   <- vertical only
  <div><span style="border-top:3px">   </span></div>    0
  <div><span style="margin-left:10px"> </span></div>    0
```

Three of those four rows have a non-zero margin/border/padding and are still **0**. What actually holds a
line open is an edge that occupies **inline flow width** — which in this engine is precisely the
`pad_l` / `pad_r` spacers, so `holds_line` is `true` on those two emission sites and `false` on the
empty-inline reporter. Writing the predicate from the spec sentence would have made three of the four
rows 18 against Chrome's 0: a fix that is *more spec-compliant* and *less correct*.

Gate: `a_line_box_with_only_empty_inlines_does_not_exist` (`engine/layout/src/lib.rs`), which asserts both
directions and all four rows above. RED-proven by mutation in both directions — `holds_line: true` on the
empty-inline spacer reads 19.2 (the corpus symptom), `holds_line: false` on the padding edges reads 0
where Chrome says 18.

## Chrome does not break after a solidus (t791)

UAX #14 offers a line-break opportunity after `/` (class SY, whose only member is U+002F), and
`unicode-linebreak` reports it faithfully. **Blink tailors it away** — a long URL overflows its box in
Chrome rather than wrapping at its path separators. We took the opportunity, so every URL, file path
and breadcrumb in body text produced a different set of line boxes.

```
  at width 120px, heights in px                       Chrome   ours (before)
  aaaa/bbbb/cccc/dddd                                   19      38
  https://example.com/very/long/path/here               19      77
  one/two three/four five/six seven/eight               77      58
  the URL again, with overflow-wrap: break-word         58      58   (unchanged, deliberately)
```

⚠ **The third row is why the first two would teach the wrong lesson.** Read alone they say *"Chrome
overflows where we wrap"*, which sounds like we are the tidier engine. On the third Chrome takes
**four** lines against our three — refusing the opportunity moves a whole token down. The error is not
a bias in one direction; it is a different set of line boxes, and every element below one inherits the
difference as `dy`.

Every other separator already agreed (`- . _ ? = & , : +`, numeric dates, CJK, soft hyphens, U+200B),
which is what makes this a one-character tailoring rather than a quarrel with the crate. The hyphen and
the zero-width space are asserted in the gate precisely so the over-broad version of this fix — *stop
breaking inside words* — cannot pass.

⚠ **`overflow-wrap: break-word` is a different path and must keep breaking.** A page that asks for the
URL to be broken still gets it. That assertion is what separates this from a regression wearing a
parity number — and it is the one number in the gate I first wrote from an assumption (77px, *"the URL
with every slash broken"*) rather than from Chrome. Chrome reads 58. **Thirteen measured numbers and
one invented one, and the invented one was in the guard.**

## A text-bearing `inline-block` sits on its own baseline (t795)

CSS 2.1 §10.8.1: the baseline of an `inline-block` is **the baseline of its last in-flow line box** —
unless it has no in-flow line boxes, or `overflow` computes to something other than `visible`, in
which case it is the bottom margin edge. We implemented only the fallback, so a text-bearing
inline-block sat entirely above the line's baseline and its line box grew by the whole strut descent.

```
                                                    Chrome   before   after
  <span style="display:inline-block">Ay</span>Ay     19.19     23      19
  …with padding:5px                                  29.19     33      29
  …with overflow:hidden                              23.38     23      23   ✓ (fallback)
  an EMPTY inline-block + text                       19.19     19      19   ✓ (fallback)
  a 20×20 empty inline-block + text                  24.19     24      24   ✓ (fallback)
```

⚠ **The three rows that already matched are the fallback cases — which is exactly why this survived
690 ticks.** The rule we implemented is a real rule; it was applied to every box instead of to the two
kinds it belongs to. A wrong rule that is right a third of the time, and silent the rest, is the
hardest shape to see from the inside.

About 4px per line, on every row of chips, nav items, badges, tags, buttons and inline lists on the
modern web — compounding down the page as `dy`. Measured: `blog.rust-lang.org` **73.7% → 99.3%**
shape on 1664 elements, `chat.google.com` **72.9% → 84.7%** (crossing the bar), `255md.com` 69.8 →
72.1, `en.wikipedia.org` 58.8 → 60.4.

⚠ **`blog.rust-lang.org` had been this loop's control site all day**, sitting at 73.6–73.7% through six
consecutive fixes, and its byte-identical reading was quoted as evidence each time. It was
byte-identical because none of those fixes touched what was actually wrong with it. **A control that
never moves is telling you about your fixes, not about the site.**

⚠ It was found by a probe built to ask a different question — sub-pixel accumulation, whose answer was
"no bug". The container height in the same output read 22 against Chrome's 18, and nobody had asked.
**A probe is worth more than its hypothesis.**

## UAX #9 rule L2: a line's inline BOXES are reordered — and having the other half of bidi is what hid it

`FontContext::shape_bidi` has run the Unicode Bidirectional Algorithm since the shaper landed:
`unicode_bidi::BidiInfo` over the run's text, visual runs, each shaped with its own direction, gated
by `engine/text`'s `g_bidi_base_direction`. That is UAX #9 applied **inside one text run**, and it is
correct.

**Rule L2 applies to a LINE**, and a line is not a run — it is a sequence of inline *boxes*: a
footer's twenty `<a>` elements, an `<em>` mid-sentence, an `inline-block` chip, each measured and
positioned separately by `layout_inline`. Nothing reordered those. So one Arabic word came out right
and twenty Arabic links came out exactly backwards, on every RTL page, for the engine's whole life.

`close_line` now runs L2 over the line's fragments, after justification (which reads the flow-order
gaps) and before the alignment offset (a uniform shift, which reordering commutes with).

Chrome-measured, `file://`, 1200×800 window, 400px containers, x relative to the container:

```text
                                       Chrome            before        after
  dir=rtl, three RTL-script <a>        370 / 343 / 312   312/343/370   370/343/312   ✗→✓
  dir=ltr, three RTL-script <a>         58 /  31 /   0     0/ 34/ 61    58/ 31/  0   ✗→✓
  dir=rtl, three LATIN <a>             303 / 334 / 364   303/334/364   303/334/364   ✓ control
```

⚠ **The third row is the difference between a bidi fix and a "reverse the links on an RTL page"
fix.** Latin text in an RTL paragraph is one LTR run at level 2: its boxes keep source order and only
the *line* is flush right. Reading the container's `direction` and reversing would get row 1 right
and rows 2 and 3 wrong. The second row is the same point from the other side — an RTL run reorders
inside an **LTR** paragraph. All three fall out of the levels, so all three are exact.

**Spaces are modelled as items, not as gaps.** The flow leaves inter-word space as the distance
between one fragment's end and the next one's start; under reordering that space is a *character*
with its own level and its own place in the visual sequence, so the permutation must carry it.
Reversing positions in place and mirroring the gaps is correct for a single level and composes
**wrongly** with two (an LTR run embedded in RTL), because the array stays in logical order while the
nested reversal has already moved its members. Slots (`Frag(i)` | `Space(w)`) make L2 the textbook
loop over an index permutation, and conserve the line's total advance exactly — so alignment,
justification and the float band still agree on the width they already agreed on.

**Inert on LTR content by construction, not by a fast path:** with no odd level on the line, L2's
`lowest_odd..=max` range is empty and no `x` is touched.

Measured, old-binary control, 20 sites, same hour: `possssno.sbs` **0.6974 → 0.8783** (crossing the
0.75 bar, and `reading_order` **524 → 1**), `www.ta3lemkonline.com` **0.5492 → 0.5733**, 17 others
flat, zero attributable regressions.

⚠ **This SUPERSEDES a refutation recorded at t837**, which read:

> *"REFUTED en route… `possssno.sbs`'s footer is horizontally MIRRORED under `<html lang="fa"
> dir="rtl">`, and **RTL is NOT the cause** — a `dir="rtl"` inline fixture measures `a1` at 492
> against Chrome's 493… Our inline base direction is Chrome-correct."*

Both halves of that sentence are true and the conclusion does not follow. Our inline base *direction*
**is** Chrome-correct — that is precisely why the line is flush right — and the t837 fixture was
row 3 above, **Latin text**, which is the one case that is supposed to keep source order. A fixture
built from the alphabet the reader types cannot ask a question about the script the page is written
in. **When probing a script-dependent behaviour, the fixture must contain that script.**

**Residue, measured here and not fixed:** a fixed-width block in an RTL containing block is flush
LEFT for us and flush RIGHT in Chrome (`#r` at Chrome `x=800` = 1200−400, ours `x=0`). CSS 2.1
§10.3.3 — the over-constrained equation ignores `margin-right` under `ltr` and **`margin-left` under
`rtl`**. One miss per block under parent-relative shape, which is why it is not the mass; it is why
an RTL page's whole sidebar sits on the wrong side.

## An inline element's box is ITS OWN content area, resolved PER AXIS (t853)

`<span class="icon"><i></i></span>` — an inline whose entire content is an **atomic** inline (a sprite
`<i>`, an `<img>`, an icon glyph in an `inline-block`) or a nested inline (`<a><em>x</em></a>`) —
emits nothing that belongs to *itself*: no `Word` (the text is the descendant's), no edge spacer (no
padding), and it is not empty so the empty-inline reporter does not fire. It arrived at `node_rects`
with **no fragment at all**, and the only geometry left to lift was the child's box.

Chrome-measured, `16px/1.2 sans-serif` (`--headless=new --dump-dom` + `getBoundingClientRect`):

```text
                                              Chrome           before
  <span><i 8x4  inline-block></i></span>      [11, 1, 8,17]    [11,11,8, 4]
  <span><i 8x40 inline-block></i></span>      [11,93, 8,17]    [11,70,8,40]
  <span 10px><b 40px>x</b></span>             [11,48,22,11]    [11,21,22,44]
  <span></span>            (truly empty)      [11,38, 0, 0]
```

**Two facts, and the second is what makes the obvious fix wrong.**

1. An inline that contributes content **has an inline box of its own** — its font's ascent + descent,
   on the line's baseline. Row 1 is 17 tall because the *span's* font says so; nothing inside it is
   17 tall.
2. That box is **not unioned with its descendants in the block axis.** Rows 2 and 3 have a 40px icon
   and a 44px `<b>` inside a 17px and an 11px parent; Chrome reports the parent unmoved and lets the
   child overflow. In the **inline** axis the opposite holds — the box *is* the advance of everything
   in it, which is why row 1 is 8 wide.

So the rule is **per axis**, the same shape as the static position (t849). Rows 2 and 3 are the ones
worth keeping in a gate: the common icon is *smaller* than its line, so a both-axes union is correct
on row 1 and only row 2 tells you it is a coincidence.

**Where each half lives.** `collect_inline_node` guarantees the element has fragments at all — two
zero-width `Spacer` reporters, one at the head and one at the tail of its items, because an inline
that wraps spans several lines and Chrome's rect runs from the first line's content top to the last
line's bottom. Both are `holds_line: false` with `report_ascent: Some(..)`, so neither brings a line
into existence, neither consumes a pending space, and neither feeds a `line_height` floor into
`close_line` — the line boxes come out byte-identical. `node_rects` then owns the axis split: an
ancestor that owns fragments takes only the **horizontal** extent of what it lifts; one that owns
none takes the whole rect, which is the pre-existing behaviour and the only thing keeping an exotic
boxless element from having no geometry at all.

⚠ **THIS IS AN I3 FIX, NOT A SHAPE FIX, AND THE DIFFERENCE IS NOT RHETORICAL.** `node_rects` is a
shared producer:

```text
  LayoutBox::node_rects()  →  manuk_a11y::build_tree_with_rects  →  A11yNode.bbox  →  the click point
```

The agent clicks the **centre of the bbox**. Reported as the 4px icon box, the click point for an
icon button was computed 3.5px low in a box 13px too short — on the single most common clickable
idiom there is. Ranked on M1 that is a sub-tolerance `shape` term the corpus cannot price; ranked on
I3 it is a mis-actuation surface, and **nothing in the burndown's ranker (`in-scope sites × dy`) can
see it** (CONSTITUTION-CHECK #72). The gate therefore asserts the a11y bbox *and* `node_rects` in the
same file: five consecutive geometry ticks passed I3 only because the producer is shared, and a fix
to the producer itself is exactly where that accident stops protecting you.

**Residue, named rather than hidden:** the reporters bound the element's first and last lines, so a
boxless inline whose content wraps across **three or more** lines reports the outer two and not any
middle line's horizontal extent. Chrome unions every fragment. One line's worth of width on a
multi-line boxless inline, and it is a strict improvement on lifting the child's box.

## `vertical-align` is implemented for atomic inlines and absent for text (t913)

Thirteen cases against a 24px baseline control, `google-chrome-stable --headless=new
--hide-scrollbars --window-size=1200,800`:

```text
                                        Chrome   ours
  plain baseline (CONTROL)                24      24    ok
  vertical-align: super                   30      24    -6
  vertical-align: sub                     28      24    -4
  <sup> / <sub>                           27      24    -3
  vertical-align: top                     24      24    ok
  vertical-align: middle                  25      24    -1
  vertical-align: 10px / -10px            34      24    -10
  vertical-align: 50%                     36      24    -12
  vertical-align: text-top                27      24    -3
  vertical-align: text-bottom             28      24    -4
  super on a font-size:10px span          24      24    ok   <- CONTROL
  super on a 10px <img>                   24      24    ok   <- CONTROL
```

**The last two rows are why this is not "add the offset".** A raised inline that still fits inside
the strut does not grow the line. The rule is CSS 2.1 §10.8's union — *the line box is the union of
the SHIFTED inline boxes and the strut* — and a shift only matters once it pushes a box past the
strut's edge. A fix that added the offset unconditionally would break both control rows.

### The cause is one `if`

`line_metrics` branches on the fragment kind:

```rust
  if f.atomic_h > 0.0 {           // image / inline-block / replaced
      let (a, b) = match f.valign { … Super => (h + ascent*0.35, -(ascent*0.35)) … };
  } else if f.ascent > 0.0 … {    // TEXT
      let (a, d) = (f.ascent.round(), f.descent.round());   // <- f.valign is never read
```

Eight match arms on one path and no mention on the other. `valign` appears at its assignment site,
the struct field, the atomic metric arms and the atomic placement arms — **nowhere else**. A text
fragment consults it neither for the line's height nor for its own paint position.

> **`<sup>` and `<sub>` are TEXT.** The case the UA stylesheet itself generates is the case that was
> never wired, which is why every atomic test passes and the whole family fails.

### Why it belongs to the `<div>`-height burndown

`<sup>`/`<sub>` carry footnotes, citations, ™/®, prices, ordinals and chemical formulae, and
`vertical-align: <length>` is the standard icon-alignment idiom. Each sits inside a `<div>` that is
then 3-12px short, and a per-line error is precisely what laundres downward through a subtree
(t743). The ranker's magnitude bands — 8px through 256px on 27-29 sites each — are what a small
per-line error looks like after it accumulates.

### The fix must be both halves at once

Growing the line box without moving the glyphs would make every `<sup>` line taller with its text
still on the baseline — a metric win bought with a visible regression, which is a trade, and trades
are refused. The correct change shifts the text fragment's box and paint origin by the same amount
and then takes the union.

## A branch that ignores a field and a field that can only hold one value look identical (t914)

t913 located the `vertical-align` defect precisely: `line_metrics`'s TEXT branch never reads
`f.valign`, while the atomic branch has eight match arms for it. **Wiring the shift into that branch
changed nothing.** Every case still read 24.

The cause is one level upstream. The line fragment for a word is built with

```rust
    valign: VerticalAlign::Baseline,   // hard-coded
```

so `f.valign` was `Baseline` for every piece of text that has ever existed, and the eight arms
downstream were **unreachable** rather than unread. The builder is a `move` closure and nobody had
captured the word's own `vertical-align`; reading it one line earlier, where the node and the styles
are both still in scope, is the fix.

> **Read the PRODUCER, not only the consumer.** A branch that ignores a field and a field that can
> only hold one value are indistinguishable from inside the branch — and only one of them is fixed by
> changing the branch. Same shape as t897, where the layout rect was already a parameter of the
> function that answered `auto`.

### `<sup>`/`<sub>` had no UA rule at all

Chrome's sheet is `sup { vertical-align: super; font-size: smaller }`. Ours had neither line, so a
footnote marker, a citation, a ™, an ordinal and every chemical formula rendered as plain baseline
text at full size. The shrink is now exact — a `<sup>`'s own box is 18×15 against a plain span's
21×17, in both engines.

### What is asserted, and what is named

Asserted (Chrome-exact): the UA shrink, and four CONTROLS that stay at 24 — a plain line,
`vertical-align: top`, and a `super` on a 10px span and a 10px `<img>`. **Those last two are the rule
itself**: a raised inline that still fits inside the strut must NOT grow the line, because CSS 2.1
§10.8 is a union and not an addition.

Named, not asserted: `super` lands 29 against Chrome's 30, `sub` 26 against 28, `middle` 26 against
25. The keyword offsets reuse the constants the atomic path already uses (`ascent * 0.35`,
`ascent * 0.15`) — shared through one `valign_text_shift` so the two implementations cannot drift —
and they approximate what Chrome derives from the font's `OS/2` superscript offsets. Calibrating
against those metrics is its own tick. `vertical-align: <length>` / `<percentage>` is a third job:
the enum has eight keyword variants and no length, so `10px` parses to `Baseline`.

### Both halves in one change

Growing the line box without moving the glyphs would make every `<sup>` line taller with its text
still on the baseline — a metric win bought with a visible regression, which is a trade, and trades
are refused. `valign_text_shift` is called from `line_metrics` to size the line and from the
placement loop to move the baseline.
