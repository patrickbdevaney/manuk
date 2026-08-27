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

## A Private-Use-Area codepoint may not use a GENERIC family — and that is the whole icon-font web

`FontContext::resolve_family` answers *"which face does this element use?"* for the element as a whole,
and stops at the **first** entry of the `font-family` list it can honour. That is right for text and
wrong for exactly one block of Unicode. css-fonts-4 §char-handling-issues:

> *"If a given character is a Private-Use-Area Unicode codepoint, user agents must only match font
> families named in the font-family list that are **not generic families**. If none of the families
> named in the font-family list contain a glyph for that codepoint, user agents must display some form
> of missing glyph symbol for that character **rather than attempting installed font fallback**."*

**PUA (U+E000–U+F8FF and the two supplementary planes) is where every icon font lives** — Font Awesome,
Material Icons, Bootstrap Icons, and every in-house glyph set. A PUA codepoint has no agreed meaning, so
another family's glyph at the same address is not a worse rendering of the same character: it is a
different character. `font-family: sans-serif, "Font Awesome 6 Free"` therefore rendered whatever the
system sans keeps at U+F007 — and when the webfont failed to load, the ordinary CJK/emoji fallback found
*something*, so a 404 and a success looked the same on screen.

**The fix is a SECOND family on the key, not a change to `resolve_family`.** `FontKey::pua_family` is the
interned id of the first **non-generic** family in the same list, computed beside `family` in
`layout::text_style` (and in canvas's `fillText` key) and consumed by the per-character `resolve_face`.
Three things about the shape:

- **It is part of the KEY, not a side-table.** The shaped-run cache is keyed on `FontKey`; two elements
  that differ only in which family their icons may use must not share a cache entry.
- **`first_non_generic_family` is deliberately not a flag on `resolve_family`.** That function's whole
  contract is *"the first entry we can honour, generics included"*, and the two answers differ for
  exactly the stacks this rule is about. One function behind a boolean would make every caller decide a
  spec question it does not know it is being asked.
- **Both clauses land together.** Choosing the right family but still allowing installed-font fallback
  leaves the failed-webfont case rendering a wrong glyph; suppressing fallback without choosing the
  right family leaves the working case rendering a wrong glyph. Either half alone is still *a different
  character, presented as a fallback*.

⚠ **It was unmeasurable until the WPT testharness leg got its ruler in the same tick.**
`css/css-fonts/matching/font-unicode-PUA.html` compares `serif, …, 'Ahem'` against `'Ahem'`; with no
Ahem installed **both arms fell back to serif and agreed**, so the suite reported a pass over a real
defect. See `conformance-and-oracles.md`, *"A suite has a ruler"*.

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

**CLOSED AT t915 for `super` and `sub`** — see the calibration below. Still named, not asserted:
`middle` 26 against 25. The keyword offsets reuse the constants the atomic path already uses (`ascent * 0.35`,
`ascent * 0.15`) — shared through one `valign_text_shift` so the two implementations cannot drift —
and they approximate what Chrome derives from the font's `OS/2` superscript offsets. Calibrating
against those metrics is its own tick. `vertical-align: <length>` / `<percentage>` is a third job:
the enum has eight keyword variants and no length, so `10px` parses to `Baseline`.

### Both halves in one change

Growing the line box without moving the glyphs would make every `<sup>` line taller with its text
still on the baseline — a metric win bought with a visible regression, which is a trade, and trades
are refused. `valign_text_shift` is called from `line_metrics` to size the line and from the
placement loop to move the baseline.


## The offset is the PARENT's font size × 0.375 (t915)

t914 shipped `vertical-align` on text using the atomic arms' constants (`ascent × 0.35`,
`× 0.15`) and named the residual. One experiment found the actual rule — Chrome's growth over the
same line without the shift:

```text
                                          Chrome   t914
  super, 16px                               +6       +5
  super, 24px                               +9       +8
  super, 32px                              +12      +10
  sub,   16px                               +4       +2
  sub,   24px                               +6       +3
  super, 16px at line-height: 3             +6       +5
```

`6/16 = 9/24 = 12/32 = 0.375` exactly, and `4/16 = 6/24 = 0.25`. **The `line-height: 3` row settles
the other half: the offset does not move with the line box.** It is the font size — and per CSS 2.1
§10.8.1 (*"an appropriate offset for superscripts of the PARENT's baseline"*) the parent's, which is
measurable rather than doctrinal: a `<sup>` at `font-size: smaller` raises by the same amount as a
full-size span.

### The strut tuple gained a member rather than the constant gaining a fudge

`strut_ascent × 0.405` reproduces every row above **for this font**, and bakes its ascent/em ratio
(0.927) into a constant that would be wrong for the next one. `close_line`'s strut is now
`(ascent, descent, line_height, font_size)` and the offset is derived from the size the spec names.

> **A constant fitted to one font is a measurement of that font.** When two derivations agree on the
> data you have, prefer the one whose inputs the spec names.


## A formula that degenerates to a no-op on the common case reads as implemented (t916)

`vertical-align: text-top` was implemented as `strut_ascent - a` — **exactly zero whenever the
fragment and the strut share a font**, which is the case for nearly every `<span>` on the web. The
arm existed, was reachable after t914, and did nothing. Chrome grows the line by 3px.

CSS 2.1 §10.8.1 aligns **two different boxes**, and that is the whole bug:

* the **content area** is `ascent + descent` — the glyphs;
* the **inline box** is `line-height` tall — the glyphs **plus half-leading above and below**.

`text-top` aligns the top of the aligned subtree's *inline box* with the top of the parent's *content
area*. At `line-height: 1.5` on 16px text the half-leading is ~2.5px, so the fragment shifts DOWN by
that and the line grows below. The old formula compared two ascents and never saw the leading.

```text
                              Chrome   before   after
  vertical-align: text-top      27       24       27
  vertical-align: text-bottom   28       24       28
```

Use the **same floored** `half_leading` the line itself uses: a shift computed against a different
rounding than the line it moves within lands the box outside the box it asked for.

> **A no-op formula is worse than a missing one.** A missing arm is visible; a formula that cancels
> to zero on the common case reads as implemented in every review, passes every same-font fixture,
> and only a differential against a real engine can see it.

### The family, four ticks on

```text
                                     Chrome   t913   t914   t915   t916
  super                                30      24     29     30     30
  sub                                  28      24     26     28     28
  text-top                             27      24     24     24     27
  text-bottom                          28      24     24     24     28
  top / plain / super-on-10px (CTRL)   24      24     24     24     24
  middle                               25      24     26     26     26   <- open
  <sup> / <sub>                        27      24     24     24     24   <- open
  10px / -10px / 50%                 34/34/36  24     24     24     24   <- unrepresentable
```

`<sup>`/`<sub>`'s own box is byte-exact (18×15 against a 21×17 control) and the offset is verified at
three font sizes, so the remaining 3px is how a SMALLER fragment's half-leading folds into the line —
the one row of this family a further formula tick should not guess at.


## `vertical-align: <length>` parsed to `baseline` and vanished (t922)

The `VerticalAlign` enum had eight keyword variants and no length, so the parser's `_ =>` arm
swallowed both the length and percentage forms into `Baseline`. Not dropped with a warning, not
stored and ignored — **parsed to a different, valid value**, which is the shape this project rates
most dangerous. `vertical-align: -2px` is the standard idiom for nudging an inline icon against its
label, and it was silently a no-op for the whole life of the engine.

```text
                                     Chrome   before   after
  vertical-align: 10px                 34       24       34
  vertical-align: -10px                34       24       34
  vertical-align: 50%                  36       24       36
```

**The percentage is of the element's OWN `line-height`** (CSS 2.1 §10.8.1) — not the strut's, and not
the font size. In a 16px/1.5 fixture those are 24 and 16, and Chrome's 36 is `24 + 0.5 × 24`;
resolving at parse time against the font size would give 32 and look close enough to bank. That is
why the variant keeps a ratio and resolves in layout.

Three real match sites: the enum and its parser, the `line_metrics` atomic arms plus the `box_top`
placement arms that mirror them, and the computed-style serialisation. The text path needed two lines.

### The family, five ticks on

```text
                                     Chrome   t913   t914   t915   t916   t922
  super / sub                        30/28    24/24  29/26  30/28  30/28  30/28
  text-top / text-bottom             27/28    24/24  24/24  24/24  27/28  27/28
  10px / -10px / 50%               34/34/36     24     24     24     24  34/34/36
  four CONTROLS                        24      24     24     24     24     24
  middle                               25      24     26     26     26     26   <- open, 1px
  <sup> / <sub>                        27      24     24     24     24     24   <- open, 3px
```

Eleven of fourteen exact, from thirteen-of-fourteen wrong. What remains is two rounding questions —
and `<sup>`'s 3px is the same quantity t916 had to get exactly right for `text-top` (how a smaller
fragment's half-leading folds into the line), so it is a measurement rather than a formula to try.


## A property RECOVERED from the second UA sheet is OVERWRITTEN by it, not falling back to it (t923)

`<sup>` rendered at the right size and on the wrong baseline. Twelve mixed-font cases said the
half-leading arithmetic was already exact — an authored `<span style="font-size:13.333px;
vertical-align:super">` grows its line to Chrome's 27 — while `<sup>`, the same size and the same
raise from the UA sheet, stayed at 24. `getComputedStyle` closed it: Chrome resolves both `<sup>` and
`font-size: smaller` to 13.3333px/20px, and so do we; our `<sup>`'s box is **36×15, byte-identical**.

`vertical_align` is one of the handful of properties `stylo_engine.rs` **recovers from
MinimalCascade** into the Stylo map (stylo 0.19 exposes no computed longhand for it), and the recovery
is an unconditional `cs.vertical_align = m.vertical_align`. t914 added
`sup { vertical-align: super; font-size: smaller }` to the **Stylo** sheet only, so the size came from
Stylo and was right, and the alignment came from MinimalCascade — which had never heard of `<sup>` —
and its `Baseline` was written **over Stylo's correct `super`**.

> **For a recovered property the minimal sheet is not a fallback, it is the AUTHORITY.** Adding a rule
> to the Stylo sheet alone is worse than not adding it: the element now differs from its authored
> equivalent in exactly one property, which is the hardest shape to see.

The file already carried the warning — *"Keep in lockstep with the UA sheet in `stylo_engine.rs`"* —
and the drift happened anyway, nine ticks later, in a tick that read that file.

### The guard that outlives the fix

The gate takes the whole mixed-font fixture as a **lockstep guard**: a `<sup>` and an authored span at
the same size and alignment must produce the same line. If the two sheets drift again, those twelve
claims disagree while every keyword claim still passes.

## An inline box contributes leading even when it holds no text of its own (tick 934 measured · tick 935 LANDED)

**CSS 2.1 §10.8 is unconditional**: every inline box contributes its `line-height` to the line box,
whether or not it *directly* contains text. Ours contributes only through the fragments its text
produces, so a wrapper `<span>` with a larger `font-size` is invisible to the line box.

```text
   div is font:16px/1.5 (strut 24), 400px wide            Chrome    ours
   <span 24px>outer24</span>                     CONTROL    36        36   ✓
   <span 24px><span 12px>nested</span></span>               36        24   ✗
   <span 12px>small only</span>                  CONTROL    24        24   ✓
   <span 24px>big</span> and 16px                CONTROL    36        36   ✓
   plain 16                                      CONTROL    24        24   ✓
   line-height:normal + the nested pair                     28        18   ✗
   <span 24px;line-height:1.5><span 12px>…</span></span>    36        24   ✗
   <span 24px><span 12px>n</span>x</span>        CONTROL    36        36   ✓
```

**The discriminator is the last row.** Give the outer span one character of its own text and we are
correct; take it away and it stops existing for line-height purposes. A metrics, font or rounding
error would move the CONTROL rows too — this is **structural**.

**Where it lives.** The line box's height is folded over `LineFrag`s plus the strut
(`engine/layout/src/lib.rs:7545-7560`, `.fold(strut.0.round(), f32::max)`), and `LineFrag`s come from
`InlineItem`s, which `collect_inline_node` (`:6218`) emits only for text, atomics, spacers and breaks.
**An inline element wrapping another inline element emits no item at all**, so it never reaches the
fold.

**The fix, as landed at t935.** `InlineItem::Spacer` gained a `leading` field, kept SEPARATE from
`report_height`. Those two were the same number, and that conflation is exactly why a wrapper could
not be expressed: a padded edge must report a tall rect and contribute **zero** leading (§10.6.1),
while a text-less wrapper is the mirror image — full leading, **no rect at all**. The wrapper carrier
is emitted with `node: None` (metrics, never geometry) and `holds_line: false`:

* it must **not** resurrect the t760 empty-wrapper rule — `<div><span></span></div>` is 0 tall in
  Chrome, and the early return at `:7527` (`!line.iter().any(|f| f.content_bearing)`) is what
  delivers that;
* it must **not** enter the `vertical-align` metrics the way an atomic would — `:7549` filters
  `atomic_h <= 0.0` for exactly that reason.

The change is idempotent where we are already correct, because the fold is a `max` and a text-bearing
element already contributes through its own words. That is why the five CONTROL rows above are the
gate's real content.

**Why it is worth more than its size.** This is the shape of nearly every icon, badge and typographic
wrapper on the web (`<span class="h1"><span class="txt">…</span></span>`, `<a><span>label</span></a>`
across font sizes, `<span class="icon"><i></i></span>`). It is a line-height error, therefore a **dy**
error, and one wrong line box cascades down every element below it.

⚠⚠ **The burndown calls this family "already Chrome-correct, do NOT re-grind."** §3 lists
*half-leading / strut / vertical-align* as verified in source. That verdict is true **for the cases it
was measured on** — every CONTROL row above is one of them — and not true of this clause, which no
fixture had reached. **A "do not re-grind" entry is a statement about a measurement, not about a
subsystem.**

### Also measured: a `nowrap` inline in `overflow:hidden` reports a CLAMPED width

`white-space:nowrap; overflow:hidden; text-overflow:ellipsis` on a 400px box with a run needing 544:
Chrome reports the span at **544.28** (it overflows and is clipped), we report **399**. The height is
17 in both, so we are not wrapping — we are truncating the reported box to its container. That is the
`getBoundingClientRect` of every truncated table cell, nav label and card title. A different mechanism
from the line box; named rather than folded in, because a tick fixing both could attribute neither.

### The negative result from the same sweep

Twenty of twenty-two composed inline cases were Chrome-exact: `line-height` as a number, a 32px inline
raising its line, `line-height:0`, `vertical-align:super` and a negative length, an `<img>` and an
`inline-block` on the line, inline padding/border/margin, `text-align:justify`, `text-indent`,
`word-spacing` + `letter-spacing`, `sub`/`sup`, `2em` sizing, `text-transform`, an RTL run (x=359,
exact), and `white-space:pre` with leading and trailing spaces. Recorded so the next tick does not
re-sweep the family.

### As landed (t935) — every line box and every y byte-identical to Chrome

```text
   div is font:16px/1.5 (strut 24)                     Chrome   before   after
   <span 24px>outer24</span>                 (CONTROL)   36       36      36
   <span 24px><span 12px>nested</span></span>            36       24      36
   <span 12px>small only</span>              (CONTROL)   24       24      24
   <span 24px>big</span> and 16px            (CONTROL)   36       36      36
   plain 16                                  (CONTROL)   24       24      24
   line-height:normal + the nested pair                  28       18      28
   <span 24;line-height:1.5><span 12>…</span></span>     36       24      36
   <span 24px><span 12px>n</span>x</span>    (CONTROL)   36       36      36
```

**The cascade is gone, which is the actual prize.** In the 22-case sweep that found the defect, the
seven elements below the wrapper were up to 12.8px adrift and are now exact.

⚠⚠⚠ **THE TRAP: `ComputedStyle::line_height` IS NOT THE LINE HEIGHT.** It holds the raw
`1.2 × font-size` fallback for `line-height: normal`; `text_style` derives it from the FACE
(`round(ascent + descent + lineGap)`), which is what every other item on the line uses and what
Chrome lays out with — 18 against 19.2 at 16px. The first draft read the raw field and
`manuk-layout`'s `a_button_centres_its_content_vertically_in_its_content_box` caught it on the first
run, because that test derives its expectation from the auto-height button's own height. **One rule,
two implementations**; route through `text_style`.

⚠⚠ **A tolerance can make a gate unfalsifiable in the exact quantity it tests.** The RED proof for
the wrong-field version came back GREEN: it produces **28.800003** against Chrome's 28, and the
gate's ordinary **1.01** tolerance waves that through. The `line-height:normal` row alone now carries
**0.5**. A proof that does not go red is not a proof.

### The residue, pinned at OUR number (t935) — CLOSED at t939, and it was ONE BRANCH

The line box is now the right HEIGHT and the inner text sits at the wrong BASELINE inside it: Chrome
puts the 12px text **15px** below the line box top, we put it **6px** below — the new leading all
landed below the baseline, where Chrome half-leads it.

t935 contributes the element's `line_height` and **not** its ascent/descent, deliberately: those are
the metrics `vertical-align: middle / text-top / text-bottom / sub / super` are defined against —
*the parent's font, never the aligned box's own* — and the fold at `layout/lib.rs:7549` filters
atomics out for exactly that reason. Feeding a nested span's ascent in without re-deriving that rule
trades a `dy` cascade for a `vertical-align` regression.

**The two errors have different blast radii, and that is the whole argument for splitting them.** The
height error cascaded down the page; the baseline error is contained to one line, and every element
below it is now Chrome-exact.

**CLOSED (t939).** `close_line` places a fragment as a real inline box *about the baseline*
(`above = ascent + half_leading`) **only when it has metrics**; without them it falls to
`min_h_down`, a floor that grows the line **downward**:

```rust
   } else if f.ascent > 0.0 || f.descent > 0.0 {   // placed about the baseline
   } else { min_h_down = min_h_down.max(f.style.line_height); }   // grows downward
```

t935 gave the wrapper its `leading` and no `(ascent, descent)`, so it took the second arm and the
entire new leading landed below the baseline — the line box the right HEIGHT with everything on it
9px too high. Giving the wrapper its own metrics routes it through the arm the engine has had since
t695.

```text
   12px text inside a 24px text-less wrapper
                              Chrome    t935    t939
     line box height            36       36      36
     inner text, below top      15        6      15
```

**The padded-edge and `<br>` spacers must KEEP the downward floor, and they do by construction:**
`metrics: None` leaves them on the `min_h_down` arm byte-for-byte. They hold a line open and have no
baseline of their own — correct for a padding edge (§10.6.1) and for a `<br>`'s reporter. The
discriminator is the metrics themselves, so there is no new predicate to get wrong.

**Ranked, not stumbled on.** The t936 sweep says `reading_order` blocks **9 of the 10 cheapest M1
crossings** (sites already over the shape bar, failing only jarring), and t871-874's standing finding
is that a reading-order symptom is upstream geometry. `<a><i></i><span>label</span></a>` is the shape
this moves. ⚠ That this *does* move those nine sites is a **prediction**, not a result — only the
next sweep can answer it.

## A TAB has no width — its advance is an OUTPUT of the pen (t959–t962)

**The defect, and it is not a rounding error.** A `\t` in a preserved-whitespace run contributed
**zero advance**. `ab\tcd` was laid out as `abcd`; `a\tb\tc\td` measured **31px against Chrome's
240.8 — eight times too narrow**. `tab-size` looked like the missing thing (a surface audit filed it
as a missing CSS property) and was the *smaller* half: there was nothing for it to scale.

```text
   monospace 16px, shrink-to-fit <pre>        Chrome     before
   "ab\tcd"                    (tab-size 8)     96.3       31
   "ab\tcd"   tab-size:4                        57.8       31
   "ab\tcd"   tab-size:0                        38.5       31
   "a\tb\tc\td"                (tab-size 8)    240.8       31
   "ab cd"    an ordinary SPACE     CONTROL     48.2       39   <- always worked
```

**THE RULE, and the row that discriminates it.** A tab advances the pen to the next multiple of
`tab-size × the space advance` **that is strictly greater than where the pen is**. `tab-size` defaults
to 8; `tab-size: 0` advances nothing.

The discriminator is `tab-size: 2` on `"ab\tcd"`: `"ab"` sits *exactly on* a two-space stop and the
tab must still move, to the next one — so `tab-size:2` and `tab-size:4` measure the **same 6
characters for two different reasons**. An implementation that rounds the pen UP to a multiple passes
the `tab-size:4` row and fails only that one; one that inserts `tab-size` spaces passes both and fails
the multi-tab row. A gate for this must carry all three.

⚠⚠⚠ **AND THIS IS WHY IT IS NOT A ONE-LINE FIX — the engine had two separate places that assume an
advance is a CONSTANT, and a tab's is not.** Three ticks were spent finding the second one:

```rust
   // engine/text/src/lib.rs — position-INDEPENDENT and cached by text alone
   pub fn measure(&self, text: &str, key: FontKey, size: f32) -> f32 {
       let ck: RunKey = (key, size.to_bits(), false, text.to_owned());

   // engine/layout/src/lib.rs — the ADVANCE is fixed before `x` exists; only the BUILDER gets x
   let (advance, space_w, est_h, no_wrap, make_frag): (…, Box<dyn FnOnce(f32) -> LineFrag>)
```

* **`measure()` cannot hold it.** Its result depends only on `(font, size, text)`, which is what makes
  the cache sound. `"ab\tcd"` and `"a\tcd"` do not differ by one character's width — the tab absorbs
  the difference — so any width for `\t` inside `measure` is wrong for every run that does not start
  at column 0, and it would silently poison a cache keyed on text.
* **`InlineItem`'s advance/builder split cannot hold it either.** The placement loop consumes
  `advance` to decide wrapping and to move the pen; by the time `x` is known the width is already
  fixed.

**So the tab is placed by its own branch, before the tuple, exactly as `Break` is** — a newline is not
a character with a width either, and it is the same shape for the same reason:

```rust
   if let InlineItem::Tab { stop, style, node, no_wrap } = item {
       let x = if cur.is_empty() { … } else { pen };
       let adv = if stop > 0.0 { ((x / stop).floor() + 1.0) * stop - x } else { 0.0 };
       …
       pen = x + adv;
       prev_no_wrap = no_wrap;   // a `pre` run's next word must stay as unbreakable as its last
       continue;
   }
```

**`TabSize` is an enum and not a number, deliberately.** CSS gives `tab-size` two incompatible units:
a `<number>` is a count of **space advances in the run's own font** — not a length until a font is in
hand — and a `<length>` is absolute. The property is *inherited* and is set on `body`/`pre` far more
often than on the element that renders the tab, so collapsing the number to px at parse time bakes in
the parse-time font size and is wrong for everything downstream. `G_TAB_STOP` pins this with the same
markup at 16px and 32px: the stop must double when the font does.

**Where a text error becomes a LAYOUT error.** The tab fragment's `width` IS its advance, so
`content_right_extent` (which reads `x + width` and never re-measures the text) counts it, and a
shrink-to-fit box around tabbed text stops hugging one tab too tightly. That is the standing
"container-WIDTH errors LAUNDER into dy" mechanism: a narrower run re-wraps its prose, the line count
changes, and a whole-subtree height error follows.

**Safety.** A run with no tab in it goes through `split('\t')` and produces exactly the one `Word` it
always did, so every existing preformatted line is byte-identical. `pre-line` still collapses tabs,
which is what the spec asks for.

## A list-box `<select>` is sized by ROWS, and the row is NOT a line box (t958, t963)

**A `<select multiple>` or `<select size=N>` is a sized scrolling list box, not a dropdown**, and the
engine had no branch for either attribute — `form_control_text`'s `"select"` arm returns the selected
option and nothing computed a row count. Every multi-select rendered **one line tall**, and because a
control's height displaces everything after it, the content below ten controls landed **288px too
high**. Filter sidebars, admin forms and faceted search are where these live.

```text
   HTML's DISPLAY SIZE            rows = size          when size >= 1
                                       = 4            when `multiple` and no usable size
                                       = 1            otherwise
   the LIST-BOX model applies only when rows > 1
   content height  =  rows x (1.2 x font-size + 1)     border/padding add as normal
```

⚠⚠⚠ **THE ROW HEIGHT IS NOT THE FONT'S LINE BOX, and the two are close enough at one font size to
hide it.** Chrome-measured at six sizes, residual ≤ 0.03px:

```text
   size=3, sans-serif      9px   15.5px    16px     17px    20px     32px
     Chrome, border box  37.39    60.78   62.60    66.17   77.00   120.20
     (h - 2) / rows      11.80    19.59    20.19    21.39   25.00    39.40
     1.2 x size + 1      11.80    19.60    20.20    21.40   25.00    39.40
```

A 16px sans-serif line box is **18** in Chrome and 18 here; a 16px list-box row is **20.2**. The row
metric is also **font-family independent** (16px monospace gives the same 62.6) and **immune to
`line-height`** (`line-height: 40px` leaves it at 62.6) — Chrome forces its own, so a fix derived
from our text metrics is wrong for a reason no single-size fixture reveals. Substituting the line box
is a 4px error over two rows at 16px, which any reasonable tolerance would pass.

⚠⚠ **`<select multiple size=1>` IS THE DROPDOWN'S HEIGHT (21), NOT A ONE-ROW LIST BOX (22.2).** That
is why the model is gated on `rows > 1` rather than on the presence of `multiple`. Measured.

⚠⚠⚠ **THE WIDTH IS A `widest option + 6` RULE, AND IT WAS NEVER A MULTI-SELECT BUG (t964).** A
`<select>` renders ONE option and RESERVES room for all of them — open a country picker showing
"Chad" and every entry must fit without the box moving. We sized every select to the option it
happened to be displaying, so **the ordinary dropdown was wrong too**: 62 against Chrome's 76.

```text
   16px sans-serif    alpha 39.16   gamma 53.36   quickbrownfox 102.28

                                                Chrome   before   after
     list box  alpha..eps          size=4        59.36     45      59
     list box  alpha..eps          size=10       59.36     45      59    scrolling is irrelevant
     list box  alpha+quickbrownfox size=2       108.28     45     108
     list box  ONE option "a"      size=3        14.91     15      15    unmoved
     DROPDOWN  alpha+quickbrownfox              125.00     62     125    the real headline
```

The engine's missing term is exactly `widest − shown` — a RESERVE added to the extent that already
contains the rendered option, which is why it is a difference and not the whole width (adding
`widest` whole double-counts to 98.5 against 59.36). The `6` is the control's own border and option
padding, which our UA already contributes.

⚠⚠ **THE TWO HALVES ARE INSEPARABLE, measured in both directions.** Dropping the arrow for a list
box *without* sizing to the widest option triples the total width error (44.2px → 81.4px across six
controls); keeping it *with* the widest-option reserve gives 76.36 against 59.36. The arrow is real
physics and the old width error was silently compensating for it, so either change alone trades a
right answer for a wrong reason.

⚠⚠ **THERE IS NO SCROLLBAR TERM, and the prediction that there was one was an artefact of the
comparison.** t963 wrote that a ~6px term appears when the option count exceeds the row count. Five
options in FOUR rows measures 59.36 and five options in TEN rows measures 59.36. The gap being
explained was the difference between `alpha` (rendered) and `gamma` (widest) — **the residual
invented to explain the gap WAS the gap.**

⚠ **`appearance: none` takes the WIDGET off the control, not the OPTIONS.** Only the arrow term is
conditional; a restyled design-system select still has to hold its own entries.

⚠ **Measured and deliberately NOT modelled: `<select multiple size=1>` is 95 in Chrome** where every
formula here predicts ~62. Nine of ten controls on the t963 fixture are exact and this one is not. A
corner with no real-web population is not worth a constant fitted to one number.

⚠ **Chrome's UA gives an unstyled `<select>` its OWN ~13.333px font**, not the inherited one: an
unstyled 4-row list box is **70**, not 82.8, and the same law reproduces it exactly
(`4 x (1.2 x 13.333 + 1) + 2 = 70`). We inherit the parent's size. Any fixture that measures a select
without setting `font-size` is measuring that defect as well, and will blame whatever it was actually
testing.

## A replaced element's baseline is its bottom margin edge, and `<img>` was right by ACCIDENT (t967)

**An inline `<svg>` made its line box 30px where Chrome gives 20, and sat 14px down inside it. An
`<img>` of the identical 16×16 size was correct in both engines, on the same kind of line, in the
same fixture.** That control is the identification.

```text
   16px sans-serif block, one 16x16 thing on the line
                                          Chrome   before   after
     <div><svg 16x16></div>                 20       30       20
       ...the svg's own y inside it          0       14        0
     <div><img 16x16></div>                 20       20       20    <- THE CONTROL
     <div><svg display:block></div>         16       16       16
     <div><svg vertical-align:top></div>    18       18       18
```

⚠⚠⚠ **THE ARITHMETIC NAMES THE VALUE.** The svg's box reported a baseline of **0** — its own TOP —
so all 16px hung *below* the baseline and the line came to `strut ascent 14 + 16 = 30`, with the
glyph pushed 14px down. Both numbers are measured, not inferred.

⚠⚠ **THE RULE WAS RIGHT AND ITS DOMAIN WAS WRONG, which is exactly why it survived so long.** CSS 2.1
§10.8.1 gives an inline-block the baseline of its *last in-flow line box*, falling back to the bottom
margin edge when it has none. That is a real rule, Chrome-measured, and it was applied to **every**
atomic inline:

```rust
   // engine/layout/src/lib.rs — collect_inline_node, the Atomic push site
   let own_baseline = if !is_replaced                       // <- t967
       && matches!(s.overflow_x, Overflow::Visible)
       && matches!(s.overflow_y, Overflow::Visible)
   { last_line_baseline(&r.boxx).map(|b| r.margin_top + (b - r.boxx.rect.y)) } else { None };
```

**A replaced element has no in-flow line boxes by definition** — what it displays is not a line — so
running the search on one asks our internal box structure a question the spec never asks it. `<svg>`
answers because, unlike `<img>`, it has element children to build a box out of. **`<img>` took the
fallback every time by accident, and that is why it looked correct.**

**The guard is a NARROWING, not a removal**, and `G_REPLACED_BASELINE` pins that in both directions:
widen it to every atomic and a text-bearing `display:inline-block` goes to 22 where Chrome gives
19.19 — the §10.8.1 main clause undone along with its exception.

**Why it is worth a tick.** Inline `<svg>` is on **34.5% of the CrUX-trend corpus**
(`docs/loop/CORPUS-CONSTRUCTS.md`) — nav bars, toolbars, chips, every icon on the modern web. A line
box 10px too tall drags everything below it down the page.

⚠ **STILL OPEN: the icon INSIDE a `<button>`.** `<div><button><svg/></button></div>` is **32 against
Chrome's 24** after this fix, with the button 10px below its own wrapper's top. Same shape one level
up — an inline-block's baseline when its last line box holds a vertically-aligned replaced item — and
a different computation. Do not conflate the two.

⚠ **AND THE METHOD NOTE THAT COST AN AUDIT.** Surface audit #37 attributed this to form controls
because it derived wrapper heights as *the next control's `y` minus this one's* — the `<div>`s had no
ids. Every diverging row contained an `<svg>` and every clean row did not, which the inferred quantity
could not show. **Put an id on the box you intend to talk about.**

### …and the OTHER half: the line box that CONTAINS the icon (t968 specified, t970 LANDED)

t967 fixed the line an inline `<svg>` sits **on**. This is every inline-block that **contains** one —
the icon-button shape, 23.4% of the burndown corpus.

```text
                                            Chrome   before   after
   <span inline-block><svg 16x16></span>      20       34       20
   <span inline-block><img 16x16></span>      20       24       20
   <div><button><svg 16x16></button></div>    26       36       24
   <div><a href><svg/></a></div>              20       20       20   control, unmoved
   y of #end, after eight rows                186      232      179
```

⚠⚠⚠ **TWO SYMPTOMS, ONE BUG, AND THE `<img>` ROW PROVES IT.** `layout_children` files atomics as
**sibling boxes**, so §10.8.1's baseline search walks them — and a replaced kid answered in two
different wrong ways depending on whether it had a subtree:

```text
   span <InlineBlock>  [0 168 16x20]        span <InlineBlock>  [0 188 16x20]
     svg <Inline>      [0 168 16x16]          img <Inline>      [0 188 16x16]
       rect <Inline>   [0 168 16x16]        (nothing below)
```

* **`<svg>`** — the search descended and believed the `<rect>` fragment at the box's own **top**:
  baseline 0, all 20px below the line's baseline, `strut ascent 14.5 + 20 ≈ 34`.
* **`<img>`** — the search found nothing, returned `None`, and the caller took the *"no in-flow line
  boxes"* fallback: the inline-block's own bottom edge (20) rather than the image's (16), `20 + 3.5
  ≈ 24`. **The fallback taken on a line box that EXISTS.**

**Both are the same error — asking a replaced element's INSIDE a question about line boxes** — and it
is t967's rule one level up:

```rust
   BoxContent::Block(kids) => kids.iter().rev().find_map(|k| {
       let replaced = k.node.is_some_and(|n| matches!(dom.tag_name(n),
           Some("img"|"canvas"|"video"|"svg"|"object"|"embed"|"iframe")));
       if replaced { Some(k.rect.y + k.rect.height) } else { last_line_baseline(k, dom) }
   }),
```

⚠⚠ **"SKIP THE SUBTREE" IS NOT THE RULE — "CONTRIBUTE YOUR OWN BOTTOM EDGE" IS, and the 4px between
them was measured, not reasoned.** t968 predicted that applying t967's rule here would reach 24 and
leave every row 4px short. It does: the gate's RED-proof B runs a replaced kid returning `None` and
reads exactly 24. Returning the edge reaches Chrome's 20.

⚠ **The `<button>` rows land at 24 against Chrome's 26 and that is NOT this rule's residue** — every
button in the fixture is 2px short, including the text-only one this never touches. It is the
pre-existing UA control-height difference (our controls are 22 where Chrome gives 24), named on
`<select>` at t963.

⚠ **And a method note: t968 deferred this as "a MISSING INPUT, not a missing guard" — a change to the
contract every inline item goes through — after reading `last_line_baseline` alone.** Reading one
function further, to where atomics rejoin the tree, showed they were already in the walk. **A
deferral is a prediction about work not yet done, and this one was tested and was half right.**

## `text-indent` moves the line's START edge; it was charged as a leading fragment (t988)

`text-indent` shifts the first line box's inline-**start** edge. Its **end** edge is the container's,
so the line is `indent` px narrower *and* begins `indent` px further in. The line layout reduced
`line_avail` by the indent **and** started the first fragment's `x` at `text_indent`, while
`line_left` never moved — so the wrap test `pen + space + advance > line_avail`, with `pen` already
carrying the indent, **charged it twice**, and alignment was computed against a band whose left edge
was in the wrong place.

```text
                                                        Chrome     before      after
  text-indent:20px in an 80px box, marker on LINE 2      [  0]      [ 29]      [  0]
  text-indent:20px + text-align:center, 400px box        [196]      [186]      [196]
  text-indent:20px + text-align:right,  400px box        [371]      [351]      [371]
 ── controls ──
  marker on LINE 1 · nested marker · 10% of 400 · -9999px · align with no indent   unchanged
```

### The break-point symptom is invisible to every obvious instrument

With `text-indent: 20px` in an 80px box, Chrome breaks `aa bb / cc` and we broke `aa / bb cc`: **two
lines either way**, so the block's height matches, the container's width matches, and every character
is present. Only the x of a marker on the *second* line reveals it. The twenty-row text battery that
found this could see it only as a 20px width difference on an inline box's union — the union of the
fragments — and a dedicated probe was needed to say what had actually happened.

> **A wrap-point error that does not change the line COUNT is invisible to height, width and text
> content at once.** Put a marker on the second line.

### The alignment rows are the evidence the model is right, not a bonus

Two models explain the break-point symptom equally well, and only one survives being combined with
`text-align`:

```text
  "the indent is a leading space"      -> centre at (400−29)/2      = 186     WRONG
  "start edge moves in, end edge does not"
                                       -> centre at 20 + (380−29)/2 = 196     Chrome
                                       -> right  at 20 + (380−29)   = 371     Chrome
```

A fix aimed only at the break point could have been written either way and passed. **Two models that
agree on the symptom you noticed are separated by the property you did not think to combine it with.**

A negative indent still widens the line and carries it off-screen (`text-indent: -9999px`, the
image-replacement idiom), because both terms flip sign together — clamping the indent at zero "to be
safe" fails that control and nothing else.

Gated by `G_TEXT_INDENT_EDGES`. RED-proven three ways, each read off the whole fixture. ⚠ A fourth —
applying the indent to *every* line — **does not fire**, and the reason is structural: wrapped lines
never reach the `cur.is_empty()` block that reads `first_line`, because the break branch sets
`line_left`/`line_avail` directly. The "first line only" behaviour is enforced by the break path.

## A HORIZONTAL-only frame left the inline's box on the LINE, not on its content (t994)

t851 established that a non-replaced inline's box is its own **content area**, resolved per axis. That
held for a bare inline and for one with an all-sides border. It did **not** hold for a frame on the
inline axis only — which is the overwhelmingly common inline decoration: `<code>`, `<kbd>`, a padded
`<a>` chip or pill, a badge span, a syntax-highlighted token.

```text
                                         Chrome            before            after
  <span>y</span>                     [10,  2, 10, 19]  [10,  2, 10, 19]   unchanged
  <span background>y</span>          [10,  2, 10, 19]  [10,  2, 10, 19]   unchanged
  <span border-left:12px>y</span>    [10,  2, 22, 19]  [10,  0, 22, 21]  [10, 2, 22, 19]
  <span padding-left:12px>y</span>   [10,  2, 22, 19]  [10,  0, 22, 21]  [10, 2, 22, 19]
  <span border:12px>y</span>         [10,-10, 34, 43]  [10,-10, 34, 43]   unchanged
```

**The inline axis was right in every row** — the frame advanced the pen correctly, `width` is 22
before and after. Only the *vertical* report went wrong, and only when the vertical frame was zero:
`collect_inline_node` computed it behind `if pad_t > 0.0 || pad_b > 0.0`, so a horizontal-only frame
emitted an edge spacer carrying **no vertical report at all**. That spacer fell back to the line box,
and an inline's box is the union of its fragments — so a line-box spacer (0..21) unioned with the
element's own word (2..21) came out **0..21**: two pixels of half-leading too high and two too tall,
on a box whose background is painted.

> **A conditional that guards a computation by the axis it happens to be about is how a per-axis rule
> loses one axis.** The vertical report was written *for* vertical padding, so it was gated *on*
> vertical padding — and the box it produces is the right answer for any framed inline, because with
> zero vertical padding its two terms simply add nothing.

The all-sides case was already right — `border: 12px` has a non-zero `pad_t` and took the same branch
— which is what made this narrow, and why a fixture built from `padding: 10px 20px` (the row that
motivated the original code) could not see it.

Gated by `G_INLINE_FRAME_BOX`. RED-proven two ways: restoring the vertical-only condition returns the
two horizontal-only rows to height 21 with the other three passing, and using the line height rather
than `ascent + descent` for the report gives all three framed rows height 24. ⚠ Dropping `pad_r` from
the new condition does **not** go red, because every framed row in the fixture has a LEFT edge —
recorded as a NON-red: that half of the condition is reasoned, not measured.

An old-binary A/B on the four-site anchor panel was byte-identical (87.4% mean shape, all four jarring
invariants unchanged), and the 20-row text battery and 16-row borders battery both re-ran with no new
divergence.

## The same property, two implementations, and a comment saying they could not diverge

`vertical-align` is implemented twice in `close_line`: once for TEXT fragments (`valign_text_shift`)
and once for ATOMIC ones — inline-blocks, images, anything with a box of its own. The two arms must
agree, because they answer the same question about the same line, and `valign_text_shift`'s own doc
said they did:

> *"The keyword constants are the ones the ATOMIC arms in `line_metrics` already use — deliberately
> shared, so the two implementations of `vertical-align` cannot drift apart."*

They had drifted, and in the two keywords whose constants are hardest to guess:

```text
                text arm (measured)        atomic arm (guessed)
   sub          parent_font x 0.25         ascent x 0.15
   super        parent_font x 0.375        ascent x 0.35
```

Different constants, against a *different quantity*. Chrome-measured on a 40×20 inline-block on a
`16px/20px monospace` line, as an offset from the same box's `baseline` placement:

```text
                 Chrome     before      after
   sub          +4.19px    +2.00px    +4.00px
   super        -6.33px    -5.00px    -6.00px
```

The text arm's constants were measured at three font sizes and proven independent of `line-height`
(t9xx); the atomic arm was never revisited. **A comment asserting that two implementations cannot
diverge is the same shape as a comment asserting a UA sheet is kept in lockstep — it cannot go red**,
and this project has now been bitten by that shape three times (the twin UA sheets at t851, the twin
cascades at t1006, this).

### ⚠ The fixture bug that made the first run report 20 of 20 exact

A `vertical-align` fixture whose aligned box is the **tallest thing on its line** measures nothing:
the line box grows to the box, and `top`, `bottom`, `baseline`, `sub` and `super` all put it in the
same place. The first version of this battery returned twenty exact rows while testing no alignment
at all. **The line needs something taller than the box under test** — here a 6×60 inline-block strut
— and that strut is the difference between a fixture and a decoration.

Priced on the burndown corpus, HTML + linked stylesheets: `vertical-align: sub|super` is declared by
**22/171 = 12.9%**, and `<sup>`/`<sub>` markup appears in 4/171 = 2.3%; the union is 14.6%.

## `::first-letter` is absent, and it is 10.5% of the CSS 2.1 suite's remaining failures (t1077)

Re-ranking `css/CSS2`'s `selectors` chapter (380 failures) gives an answer that is not a ranking so
much as a single name:

```text
   339  first-letter-punctuation        8  first-letter-selector      3  first-letter-nested
     9  first-line-pseudo               5  first-letter-quote         4  lang-selector
```

**~354 of the suite's 3,374 remaining failures — 10.5% — are one unimplemented pseudo-element**, and
339 of them are the single rule that leading punctuation joins the first letter (CSS 2.1 §5.12.1),
which the suite enumerates across Unicode punctuation classes.

Measured, `p::first-letter { font-size: 32px }` over six paragraphs including quoted, parenthesised
and guillemet openings:

```text
   Chrome   every paragraph 38 tall — the 32px letter raises the first line box
   ours     every paragraph 19 tall — the rule matches nothing
```

`::first-letter` and `::first-line` are **not in the `Pseudo` enum, not parsed, and not laid out** —
`grep` finds zero occurrences in `engine/css` and `engine/layout`. A selector using one fails
`parse_compound` and the rule is dropped, which is the safe failure and also a silent one.

⚠ **And the map had no row for it at all** — not `missing`, not `unknown`, *absent*. So the loop could
not say it was unbuilt, which is the same reactive-map hole t1054 found for the layout primitives:
`::before`/`::after` are gated and heavily measured, and the two pseudo-elements CSS 2.1 defines
*beside* them were never named.

### Why it is a subsystem and not a tick

`::before`/`::after` generate a box **around** content that the box tree already knows how to hold.
`::first-letter` and `::first-line` are the only CSS 2.1 pseudo-elements that must generate a box
**inside an inline formatting context, over a range the line breaker discovers** — you cannot know
what the first line contains until you have laid it out, and styling the first letter with a larger
font changes the line's height, which changes what fits on it. That circularity is the feature.

The decomposition, in the order the suite rewards:

1. **Parse** `::first-line` / `::first-letter` into `Pseudo`, and let the cascade produce a style for
   them (the `PseudoIndex` that already serves `::before`/`::after` is the place).
2. **`::first-letter`'s range**: the first typographic letter unit of the first formatted line, plus
   any immediately *preceding* punctuation — `"`, `(`, `«`, `¡` — and, per CSS Text, any immediately
   following punctuation. The 339-test family is this rule and nothing else, so it is the half that
   pays.
3. **Box generation**: an anonymous inline box over that range, carrying the pseudo's style, sized
   before the line is broken so its font-size participates in the line box's height.
4. **`::first-line`** last: it applies to a range the line breaker *produces*, so it needs a second
   pass, and it is 9 tests rather than 339.

## `::first-letter` is a RANGE over `InlineItem`s, and UAX #14 had already cut it in two (t1078)

The tick after the one above. t1077 priced `::first-letter` as a subsystem and ordered the work
parse → range → box generation → `::first-line`. Two of those four steps turned out to be already
paid for, and the one that was not is the one nobody had ranked.

**Step 1 was free, and the reason is worth saying out loud: the parse was never the problem.**
Stylo's *servo* build has `PseudoElement::FirstLetter` (`servo/selector_parser.rs`, right beside a
comment reading *"If/when :first-letter is added…"* that is now out of date), and the `selectors`
crate's `is_css2_pseudo_element` accepts the CSS2 single-colon spelling `div:first-letter` that
every one of the suite's tests uses. So the rule parsed, matched, and cascaded correctly for 1,077
ticks — and was then thrown away, because `PseudoIndex::collect` asked `sel.pseudo_element()` a
question with exactly two answers in it:

```rust
    Some(&Pe::Before) => Some(false),
    Some(&Pe::After)  => Some(true),
    _                 => None,          // <- ::first-letter fell in here, silently
```

⚠⚠⚠ **This is the "absence" shape again (t1071), at its cheapest**: the capability was one arm of one
`match` away, and no code search for `first_letter` in `engine/css` could ever have found it,
because the thing to search for was the *third bucket that does not exist*. Enumerate what the
dependency can express, not what our own source mentions.

**Step 3 was free too**, and for a reason the spec hands over: every one of the 339 punctuation
tests pairs `div:first-letter { … }` with a reference file that writes the expected rendering **by
hand, as a `<span>`**:

```html
    test:  <div>)T)est</div>
    ref:   <div><span>)T)</span>est</div>
```

A first-letter box **is** an inline run with its own style. Building a distinct box type would have
had to re-derive line metrics, baseline alignment and painting a second time and agree with the
first to the pixel — which for a byte-exact reftest is not a simplification, it is the failure mode.
Splitting the first `InlineItem::Word` puts test and reference on the *same code path*.

### The step that was not free, and the row that found it

Step 2, the range. §5.12.1 in the spec's own words:

> Punctuation (i.e. characters defined in Unicode in the "open" (Ps), "close" (Pe), "initial" (Pi),
> "final" (Pf) and "other" (Po) punctuation classes) that precedes or follows the first letter
> should be included.

Note what the five classes leave out: **`Pd` (the dashes) and `Pc` (`_`) are not on the list**, so an
em dash is not skipped over as leading punctuation — it *becomes* the first letter. Guessing with
`char::is_ascii_punctuation()` would have been right for `(` and wrong for `—`, `_` and every
non-ASCII quote the suite enumerates, which is the axis these tests sweep.

The first implementation resolved that range inside a single word. It passed `)Alpha`, `(Alpha`,
`[Alpha`, `.Alpha` — and failed `}Bravo` and `!India`:

```text
    )Test    rows 31–63    the first letter is 36px         PASS
    {Test    rows 31–60                                     PASS
    ]Test    rows 31–60                                     PASS
    }Test    rows 11–23    the whole div is 16px            FAIL
    !Test    rows 11–21    the whole div is 16px            FAIL
```

⚠⚠⚠ **Same Unicode class, opposite result, is the signature of a SECOND mechanism.** `)` and `]` are
UAX #14 line-break class **`CP`**, and `(`/`[`/`{` are **`OP`**; both forbid a break before the
letter that follows. But `}` is **`CL`** and `!` is **`EX`**, which *permit* one — so
`unicode-linebreak` had already handed layout **two words**, and the first held no letter at all.
`first_letter_len("}")` is correctly `0`, and the code above it gave up.

`)` and `}` are both `Pe`. Nothing about the CSS classes predicts the split; only the line breaker
does. **196 of the 339 punctuation tests were on the wrong side of it** — more than half the prize
sitting behind a mechanism that belongs to a different specification.

So the range is resolved over the **concatenation of consecutive words that no white space
separates**, then applied back across them: whole words inside the range are restyled, and the one
word the range ends inside is split. Every word after the first also becomes `no_wrap`, because the
whole point of the range is that `}` and the `T` the line breaker separated it from are one
typographic unit and must not end up on two lines.

### The negative row, which the suite provides

CSS 2.1: the first letter must be the *first thing* on the first formatted line. Finding the first
word by **searching** the item list happily steps over an `<img>` and reddens the `F` of
`<div><img/>Filler Text</div>` — which `first-letter-selector-002` asserts must not happen, and
which is the one test this tick briefly regressed. Searching became a **walk** that gives up at the
first content-bearing non-word, stepping over only what occupies no line of its own: a `Spacer` (an
inline's padding edge), an `AbsPseudo` (out of flow, zero advance), a `Tab`, and white space.

### Measured

Headless Chrome at 20px monospace (12px advance) with `div:first-letter { background: red }`, the
range read straight off the screenshot as a character count:

```text
                     Chrome   ours
   )A)lpha             3        3     leading AND trailing punctuation
   }Bravo              2        2     the UAX #14 CL case
   Charlie             1        1     the plain case
   —Echo               1        1     Pd is not one of the five classes
   «Golf               2        2     Pi
   !India              2        2     Po, and the other UAX #14 case (EX)
```

```text
   css/CSS2/selectors     85 passed / 380 failed   ->   436 passed / 29 failed
   css/CSS2 (whole)     2272 passed / 3374 failed  ->  2624 passed / 3022 failed
```

Zero tests that passed before fail after. Gated by `G_FIRST_LETTER`, RED-proven four ways (drop the
`Pe`/`Po` arms; resolve within one word; delete the replaced-element bail; drop the `PseudoIndex`
bucket) — and the single-word mutation is the one that matters, because it leaves `)A)lpha` and
`Charlie` **green** while `}Bravo` and `!India` go red. A gate that could not tell those apart would
have banked half the feature as all of it.

### What is left, named rather than discovered later

Eight tests, three mechanisms: the letter inside a child `<span>` inherits from the *originating*
element rather than from the span that holds it; `::before` content as the first letter; and
propagation into the first in-flow **block** descendant (`<div id=x><div>Text</div></div>`).
`::first-line` is untouched and is now the only CSS 2.1 pseudo-element with no implementation at
all — and unlike `::first-letter` it cannot be reached by asking the existing parse a new question,
because Stylo's servo build has no `FirstLine` variant to ask about.

## A soft wrap opportunity at a space belongs to the element that CONTAINS the space (t1105)

`layout_inline` decided a break with `!(no_wrap && prev_no_wrap)` — *"forbid a break when the tokens
on both sides are `nowrap`"*. That is right **within** one element and wrong the moment two `nowrap`
elements are **siblings**: each is its own nowrap run, and the white space between them belongs to
their **parent**. CSS Text §3 governs a soft wrap opportunity by the white-space of the element
containing the space, not by the tokens flanking it.

Six rows, Chrome-measured, in a 300px container at 16px monospace:

```text
   r1  <div> <span nowrap>…</span> <span nowrap>…</span> ×5     Chrome 300x38   ours 300x19   ✗
   r2  <div> <span>…</span> ×5                       CONTROL    Chrome 300x38   ours 300x38   ✓
   r3  r1 with display:inline-block on the spans                Chrome 300x38   ours 300x19   ✗
   r4  <div nowrap> <span>…</span> ×3                CONTROL    Chrome 300x19   ours 300x19   ✓
   r5  <ul><li display:inline nowrap> (no inter-tag ws)         Chrome 300x19   ours 300x19   ✓
   r6  the r1 shape inside a <td>                               Chrome 296x40   ours 407x21   ✗
```

**r4 is what makes this a fix and not a loosening**: a container that is *itself* `nowrap` must still
refuse every break, and it does — its spaces are inside it. r5 is an accidental control worth keeping:
with no white space between the `</li><li>` there is no break opportunity for either engine.

### What it costs, and how it reaches a table

`.hlist` is on every Wikipedia navbox; the same shape is every breadcrumb trail, tag row, chip list
and wrapping toolbar whose ITEMS are individually `nowrap`. The damage is not the line — it is the
**intrinsic size**. Content that never wraps makes a cell's min-content its whole unwrapped width,
and no ancestor can shrink below that:

```text
   the innermost <td>      Chrome  241 x 532        ours  4274 x 28     ← 28px is ONE LINE
   its wrapping table              395 x 846              4428 x 209
   the outer cell                  397 x 848              4430 x 211
```

⚠ **Read the height, not just the width.** Two ticks looked at `4430 vs 397` as a *table-width* bug
and refuted two table hypotheses against Chrome-exact fixtures. The `28` is what identified it.

### The fix is known, was measured, and is REFUSED — the residue is a second defect

Threading the space's own `white-space` through the inline items and breaking when the SPACE is not
nowrap turns all six rows Chrome-exact, keeps `manuk-layout` at 154/154, and moves `css/CSS2`
**+2 / −0** (one of the two is `white-space-007`). On wikipedia, with the population intact
(coverage 0.991404 → 0.991406, n 4844 → 4845):

```text
   shape          0.664740 → 0.674097     +0.94
   h_overflow          364 → 0            the entire horizontal overflow
   overlap              52 → 316          ✗
   reading_order        70 → 183          ✗
```

**And the residue is distributed, which is why it is not landed.** `MANUK_RO_PARTITION` reports the
183 inversions as **36 distinct containers** (biggest 34, top three 73) — not one mis-laid row, which
would have been ~325 pairs from a single 26-sibling group and would have been the whole story. So the
break *opportunity* is necessary and **not sufficient**: once content wraps, *where* we break must
agree with Chrome, and on 36 containers it does not.

The four-invariant aggregate is nearly flat (−364 +263 +113 = +12) and shape rises. It is still
refused: **a redistribution that cannot be accounted for is not evidence that it is safe.** The next
brick is to take one of the 36 containers, diff our line breaks against Chrome's inside it, and land
the break-opportunity change **with** that fix rather than before it.

### The 36 containers are NOT the break rule — three hypotheses, three fixtures, three negatives (t1106)

Taking one of the containers the candidate fix disturbed: Wikipedia's `.hlist` sidebar
(`section[1]/table[3]/tr[10]/td[1]`), four `<li display:inline>` links in a `<ul>`. Asked of Chrome
rather than assumed, **every one of them computes `white-space: normal`** — so the nowrap rule this
whole arc began from does not apply to this container at all.

```text
                    Chrome                 ours
   td            [0  0 319x65]          [0   0 311x45]
   ul            [8  0 304x59]          [7   0 296x39]     ← one line shorter
   li1           [83 1 139x16]          [0   1 130x16]
   li2           [0 21 304x16]          [130 1 300x16]     ← ours on LINE 1, Chrome's on line 2
   li3           [71 40 64x16]          [72  1 358x36]     ← ours spans two lines; a UNION rect
```

`li1 + li2` is **430px on one line in a 296px box.** The overlap and reading-order pairs are a
*consequence* of that single over-full line, not of a break landing in the wrong place.

Three plausible causes, each killed by a fixture:

| Hypothesis | Fixture | Result |
|---|---|---|
| whitespace between inline `<li>`s is dropped | `<ul><li>a</li> <li>b</li></ul>` ± the space, ± a forced wrap | `ul` 300x19 / 300x19 / 300x38 — **Chrome-exact on all rows** |
| `text-align:center` does not reach inline `<li>`s | centred `ul`/`div`, on the block and on the list | `li` x=102 w=48 — **Chrome-exact on all four rows** |
| the break opportunity is still missing here | — | these `li` are `white-space: normal`; the rule cannot apply |

⚠ **The centring row is worth keeping for the reason it was suspected.** Chrome's `li1` sits at x=83
in a 304px `ul` and ours at x=0, which reads exactly like a missing `text-align: center` — and our
engine centres the fixture to the pixel. **A 430px line in a 296px box has no centring to do.** The
symptom named the wrong organ for the second time in this arc: t1103's route named *tables*, this one
named *alignment*, and both were downstream of a wrap.

**Still open, as a question:** why do a 130px item and a 300px item share a 296px line when the
placement loop tests `pen + space_w + advance > line_avail`? They are laid ADJACENT
(`li2.x == li1.width` exactly), so no space item separates them in our stream — while the synthetic
fixture produces the gap correctly. The difference is on the real page: `.hlist` adds
`li::after { content: " · " }` between every pair, and removing that is the first bisection step.

## Generated content on a NESTED INLINE element was dropped entirely — half the corpus declares one (t1107)

The question the previous entry left open answers itself: **`li::after { content: " · " }` was not
rendering at all.** Two adjacent `<li>` shared a line with no gap because there was nothing between
them — the separator, and with it the only white space on the line, never entered the flow.

`::before` / `::after` could only be materialised from `collect_inline_group`'s `owner` — *the
element whose inline formatting context the run **is***. A pseudo on anything nested INSIDE that
context rendered nowhere, and nested inline elements are where the web actually hangs them:

```text
   q::before / q::after                 quotation marks
   a[href]::after                       the print stylesheet's URL
   abbr[title]::after                   the expansion
   label::before                        the custom checkbox / radio (every design system)
   .breadcrumb > li + li::before        the separator idiom this whole arc started from
   .hlist li::after                     every Wikipedia navbox
```

**85 of 169 corpus pages (50%) declare a `::before`/`::after` whose selector's subject is an
inline-by-default element**, against 130 of 169 (77%) that declare one at all. Measured with the
HTML + linked-stylesheet crawl of `docs/loop/CORPUS-CONSTRUCTS.md` (the join key computed once, as
that file's own warning demands).

### The measurement, both engines, `16px/1 monospace` in a 300px box

`span::after { content: " | " }`, **no white space between the tags** — the `.hlist` markup:

```text
                                                        Chrome    before    after
  <div><span>alpha</span><span>bravo</span>   ::after    77.08     48.00     76.80  ✓
  <ul><li display:inline>  (the .hlist idiom) ::after    77.08     48.00     76.80  ✓
  <div><span>alpha</span></div>, span alone   ::before   67.44     48.00     76.80  ~
  <div>alpha</div>, pseudo on the DIV         CONTROL    ok        ok        ok     ✓
  narrow 100px, 4 inline <li> with separators           100x64    100x32    100x64  ✓
  the same 4 <li>, no pseudo at all           CONTROL   100x16    100x32    100x32  ✗
```

**The width is billed to the ELEMENT, not merely painted on the line** — Chrome makes the `<li>`
itself wider by its separator, which is what turns this from missing ink into a `shape` term. On
Wikipedia's `.hlist` sidebar our `li1` read 130px against Chrome's 139, and 9px is exactly one
`" · "` at 14px.

⚠ **Row 3 is named, not asserted.** At a line edge Chrome collapses the separator's outer
collapsible space away and we do not, because generated content is emitted as ONE unbreakable word
with its spaces baked in. That is a ≤1-space error on the last item of a line. Splitting the outer
spaces out into real `space_before` / `pending_space` fixes it and *breaks the mid-line rows*, because
Chrome bills a trailing collapsible space INTO the preceding inline's rect and our fragments cannot
represent that. Exact on the common case beat approximate on both.

⚠⚠ **Row 6 is a SECOND, INDEPENDENT DEFECT the fix makes louder, and it is the next brick.**
`<li>alpha</li><li>bravo</li>` with no white space anywhere has **no soft wrap opportunity**, and
Chrome duly puts all four on one line and overflows. We break anyway: the placement loop's only
break guard is `breakable = !(no_wrap && prev_no_wrap)`, which never asks whether a SPACE is
present. This is the exact twin of t1105 — that entry found we refuse a break where one exists; this
one finds we take a break where none does — and the two are one rule with one missing term.

### What it bought, and the one place it cost

Old binary and new, same hour, `--rows-out` per site:

```text
                     coverage       shape            h_ovf  overlap  r-order  dead_target
  en.wikipedia.org   .991406→.991404  .660888→.723163  365→395  52→63   70→76   80→0
  www.a11yproject    .964602 (same)   .394495→.412844    1→1    0→0      1→1     0→0
  doc.rust-lang.org  byte-identical on every column
  news.ycombinator   byte-identical on every column

  css/CSS2, per-TEST state diff      3863 → 3890      +27 GAINED, 0 LOST
      generated-content +21 · tables/table-anonymous-objects +4 · selectors/first-letter-quote +1
      · syntax/case-sensitive +1
  manuk-layout                       155/155
```

**`dead_target` 80 → 0 is the largest single term.** Those were links whose entire visible content
was a pseudo: with the content absent the box was degenerate and the agent had nothing to click.
This is an I3 win, not a rendering one, and the render metric could not have found it.

The three jarring counters rise on wikipedia and the rise is **attributed, not shrugged at**: the
sweep's own second h-overflow exemplar is `…/td/table/tbody/tr/td/div/ul/li:nth-of-type(11)`, an
inline `<li>` in a nested navbox — the t1105 container, now carrying the separator text it was
missing, on a line the sibling-`nowrap` rule still refuses to break. No certificate term regresses
and one improves (dead-target clean 3/4 → 4/4); every counter that rises was already unclean on that
site. The residue is row 6's rule and t1105's, which are the same rule.

## A soft wrap opportunity is a property of the GAP — and the breaker had to be able to go back to one (t1108)

The placement loop decided breakability from the items *flanking* a position:

```rust
let breakable = !(no_wrap && prev_no_wrap);   // "forbid a break when both sides are nowrap"
```

That never asks whether there is a space at the position at all, and it was wrong in **both**
directions at once. Two ticks found the two halves separately — t1105 found we *refuse* a break where
one exists, t1107's battery found we *take* a break where none does — and they are one rule with one
missing term.

### The battery: 20 rows, both engines, `16px/1 monospace`

```text
                                                             Chrome   before   after
  A. IS THERE AN OPPORTUNITY AT ALL
   a1  100px <span>alpha</span><span>bravo</span>… no ws     1 line   2        1  ✓
   a2  CONTROL the same WITH source white space              3        3        3
   a3  CONTROL one unbreakable token                         1        1        1
   a4  100px alpha<span>bravo</span>delta          no ws     1        2        1  ✓
  B. WHOSE `white-space` GOVERNS
   b1  300px <span nowrap>a</span> <span nowrap>b</span> ×4  2        1        2  ✓
   b2  CONTROL the container is itself nowrap                1        1        1
   b3  CONTROL plain spans, spaces between                   2        2        2
   b4  CONTROL a space INSIDE a nowrap span                  1        1        1
   b5  CONTROL nowrap span, then ordinary text               2        2        2
  C. ATOMIC INLINES — an opportunity with no space, and we already agreed
   c1  adjacent inline-blocks, no ws                         2        2        2
   c2  CONTROL the same with ws                              3        3        3
   c3  adjacent <img>, no ws                                 2        2        2
   c4  CONTROL the same with ws                              2        2        2
  D. GENERATED CONTENT (t1107)
   d1  100px inline <li> ::after{content:" | "}              4        4        4
   d2  CONTROL ::after{content:"|"} — content, no space      1        4        1  ✓
  E. THE OTHER OPPORTUNITIES — measured so the restriction cannot silently kill them
   e1  hyphens                                               4        4        4
   e2  CJK                                                   4        4        4
   e3  soft hyphen U+00AD                                    3        2        2  ✗ (pre-existing)
   e4  <wbr>                                                 2        2        2
   e5  overflow-wrap:break-word                              2        2        2
```

19 of 20 exact. `e3` was already failing and is unrelated: we do not break at U+00AD at all.

### Restricting the opportunities exposed a forward-only greedy breaker

While *every* position was breakable, the first item that did not fit was itself a legal break, so a
forward-only loop was exactly right. The moment `break_before` says *"there is no white space here"*,
the item that overflows is routinely **not** a legal break and the line must be cut **behind** it.
`css/CSS2` named this in seven tests, every one of which had been passing on that accident:

```text
  text/white-space-004 · -processing-013 · -052   `XX  XX` at pre-wrap in 5em — the opportunity is
                                                  the preserved space run, the overflow arrives one
                                                  token later
  css1/c5502-imrgn-r-000 · c5504 · c5507 · c5509  `x <span style=margin-right:4em>x</span>x` — the
                                                  only space is before the span and the overflow
                                                  arrives at the final `x`, three items on
```

So the loop records `last_brk`, the index in the current line of the last legal cut, and on an
overflow at a non-breakable item it splits the line there, closes the head and re-lays the tail on a
fresh band. Every tail fragment moves by the same delta — the x of the first one — because nothing
has been committed to them yet: `close_line` has not run, so there is no alignment, justification or
atomic translation to undo.

### Three details the restriction made load-bearing

- **The opportunity is AFTER a preserved space run, not before it.** Putting it before cuts
  `XX  XX` in a 5em box into `XX` / `  XX`, an 80px second line where Chrome gives 40.
- **An inline element's edge spacer consumes the space's WIDTH and must not consume the
  OPPORTUNITY** — the position moves in front of the element's first word, it does not vanish.
  Clearing both cost the four `css1` inline margin/padding reftests.
- **`<wbr>` had to start saying what it is.** A zero-width soft wrap opportunity worked by accident
  while every position was one.

### What it bought

Old binary and new, same hour, four-site panel:

```text
                     coverage         shape            h_ovf   overlap  r-order  dead
  en.wikipedia.org  .991402→.991404  .722899→.787572  394→0    63→95    76→79    0→0
  news.ycombinator  1.000000 (same)  .772388→.797264    0→0     0→0      0→0     0→0
  www.a11yproject   byte-identical on every column
  doc.rust-lang.org byte-identical on every column

  css/CSS2, per-TEST state diff       3890 → 3899      +9 GAINED, 0 LOST
  manuk-layout                        156/156
```

**`shape ≥ 0.75` goes from 1 of 4 sites to 3 of 4** — wikipedia 0.7876 and news.ycombinator 0.7973
both cross the certificate bar — and **wikipedia's entire horizontal overflow, 394 elements, is
gone**. Together with t1107 the anchor's shape has moved 0.6609 → 0.7876 in two ticks.

⚠ **The `overlap` rise is attributed, not shrugged at.** The sweep's own exemplar is two `<li>` of
one navbox `<ul>` — the `.hlist` container this arc has been working. On a controlled wrapping hlist
both engines put every item on the same line as each other; what differs is that each **line-end item
is exactly one collapsible space too wide** (67 vs Chrome's 58 at 16px monospace), because generated
content is one word with its spaces baked in. That is t1107's named row-3 residue, and it now has a
corpus price on it: it re-groups items near a line edge, and `node_rects` reports a multi-line inline
as a UNION rect, so a re-grouping reads as a sibling collision. **Collapsing a generated separator's
outer spaces at a line edge is the next brick.**

## Generated content is TEXT, so its white space collapses — and an empty block-level generated box is not an inline word (t1110)

t1107 let a nested inline's `::before`/`::after` into the flow and billed its `content` string
verbatim. Two things that could not matter while the content rendered nowhere immediately did.
Chrome-measured, `<div><span>alpha</span>bravo</div>` at `16px/1 monospace`:

```text
                                                          Chrome   t1108   after
   span::after{content:" "}                                 57.81   58      58    ✓
   span::after{content:"  "}            two spaces          57.81   67      58    ← collapse
   span::after{content:" ";display:table}   the CLEARFIX    48.17   58      48    ← empty box
   span::after{content:" "} + white-space:pre               57.81   58      58    ✓
   no pseudo                            CONTROL             48.17   48      48    ✓
   span::after{content:" "} + a source space after it       57.81   58      58    ✓
   <div>a<span class=cf></span>b</div>   the span's width     0      19       0   ← empty box
```

`content:" "; display:table` is **Bootstrap's clearfix**. Every `.cf` / `.clearfix` inline was
carrying ~19px of phantom width, which displaces its siblings and reads out as overlap. The
float-containment half of the idiom is untouched: it is read from the block's own `::after`, not
from the inline path.

### Two bugs in the fix, both caught by the suite and neither by reasoning

- **Rust's `char::is_whitespace` is true for U+00A0** and CSS's collapsible set is space, tab and
  newline. The first collapse used `split_whitespace()` and silently ate the trailing `\A0` out of
  `content: "Filler text\A0"` — `css/CSS2/generated-content/before-content-display-005` and `-007`
  both went red.
- **`Display::Table` cannot be added to `generated_box_is_block_level`**, because `engine/css` maps
  *both* `table` and `inline-table` to that one variant. Adding it gained `-006` and lost `-007` — a
  wash that trades a right answer for a wrong one. The clearfix suppression tests `display` itself
  instead, which needs no such distinction because its content is empty. **The cascade conflation is
  the real defect and is now on the map.**

### The gate had a VACUOUS ROW and the mutation is what found it

The first NBSP assertion was *"`content:"x\A0"` bills more than bare"* — and collapsing an NBSP to a
plain space bills **exactly the same width**, so the mutation that swapped in Rust's whitespace set
left the row GREEN. A run of **two** NBSPs is the discriminator: two characters under the CSS set,
one under Rust's. *Run every mutation; a green one is the finding.*

### What it is worth, measured honestly

```text
   css/CSS2 per-TEST state diff     3899 → 3900      +1 GAINED, 0 LOST
   manuk-layout                     157/157
   en.wikipedia.org                 byte-identical to t1108 (n 4844, shape 0.7876, overlap 95)
   sestra.cc                        inside its own run-to-run band (overlap 15,15 vs 14,15)
```

⚠ **A single A/B run said +5.7 shape on wikipedia and −10 overlap on sestra.cc. Interleaved repeat
runs of BOTH binaries say both are ZERO.** The wikipedia "gain" came with the population moving
4844 → 4498, which is the tell; re-running the two binaries alternately produced byte-identical rows
for both. Two favourable headlines, killed by the rule that is supposed to kill them.

## The two sites nearest the M1 bar, and three hypotheses that died (t1111)

The t1109 sweep's cheapest tier is *"shape already ≥0.75, blocked ONLY by jarring"*, and its three
cheapest members were `sports.yahoo.com` (ONE reading-order pair), `hnhbkis.edu.in` and
`www.marktplaats.nl` (TWO h-overflow elements each). Re-measured against the live sites:

```text
   sports.yahoo.com     UNSCORED  tree-divergence-1738   ← the work-list item EVAPORATED
   hnhbkis.edu.in       shape 0.932   h_overflow 2       stable across t1089 / t1099 / t1109
   www.marktplaats.nl   shape 0.962   h_overflow 2       stable across t1089 / t1099 / t1109
```

⚠ **A sweep's work-list has a SHELF LIFE.** `sports.yahoo.com` was one reading-order pair from M1
when the sweep ran and is not scorable at all two ticks later. Re-measure a named site before
spending a tick on it.

### The exemplars, and what they are not

```text
   hnhbkis.edu.in    …/div[4]/div[1] and its <img>   right 1221 > vw 1200   (21px)
   www.marktplaats.nl  …/form/…/i                    right 500083 > vw 1200
```

`500083` is the tell: `shrink_to_fit` measures a subtree at a **1e6** available width, and a centring
context puts an ordinary box at x≈500,000 there. The extent code already discards that artifact in
two places (`FILL_SENTINEL`, `SLACK`). So the hypothesis was that the same artifact reaches a real
box's POSITION. Three ways for that to happen were tested and all three are dead:

1. **An `position:absolute` child of a shrink-to-fit box takes its static position from the probe.**
   Twelve rows, four containers (flex + `justify-content:center`, inline-block, flex + `text-align:
   center`, plain block), each with a `position:relative` wrapper and an inset-less absolute `<i>`:
   **byte-identical to Chrome on all twelve.** The idiom the site actually uses already works.
2. **`static_pos` is written during an intrinsic probe.** It *is* — proved by replacing the write
   with a `panic!` and watching the existing gate
   `an_out_of_flow_childs_static_position_survives_its_containers_translate` fire. So the pollution
   path is **live, not dead code**. But the real pass overwrites it there, and guarding the write
   (`if self.intrinsic_probe.get() { return }`) leaves `www.marktplaats.nl` **byte-identical** —
   same shape, same h_overflow 2. Not the mechanism.
3. …and because it is not the mechanism, the guard was **reverted**. It is principled and the suite
   stays green at 157/157, but no fixture and no site can be made to go red without it, and a change
   that cannot be shown to change anything is not bankable here.

**What is still open:** a real box on that page is positioned at x≈500,000, and neither the abspos
static-position table nor the extent heuristics put it there. The next probe should find *which* box
carries the offset — the `<i>`'s own rect, an ancestor's, or a fragment's — before another mechanism
is guessed at.

## `MANUK_HOVF_TRACE` — the overflow is reported on the symptom, and the defect is an ancestor (t1112)

t1111 spent a whole tick guessing three mechanisms for `www.marktplaats.nl`'s `<i>` at **right
500083** and refuted all three, because the exemplar line names the SYMPTOM's path and nothing else.
A box is pushed out by something *above* it, and the instrument's keys are `/`-separated paths — so
the ancestor chain is just the prefix set, and printing both engines' rects down that chain shows the
first row where they part company. That row is the defect's address.

`MANUK_HOVF_TRACE=1` on the fidelity sweep, off by default, print-only (the count is computed before
it and never filtered). It localised both of the corpus's nearest-to-M1 sites on the first run:

```text
   www.marktplaats.nl — every ancestor within tolerance, then:
      chrome [658 93   24 24]   ours [651 93 499432 24]   dx -7  dw +499408   <-- i
   hnhbkis.edu.in — every ancestor EXACT (dx 0, dw 0), then:
      chrome [882 1254 198 288] ours [741 1254 480 288]   dx -141 dw +282     <-- div
      chrome [882 1300 198 196] ours [741 1254 480 474]                       <-- its <img>
```

**Two different defects, each now one element wide instead of a fourteen-element path.**

- `marktplaats`: the `<i>`'s **own width is 499,432** — a `shrink_to_fit` measuring width (1e6, of
  which centring slack is ~500,000) reaching an element's USED WIDTH. Its position is fine; every
  ancestor is fine. This is a font-icon `<i>` in a search form.
- `hnhbkis`: a `<div>` **480px wide inside a 230px-wide parent**, with its `<img>` at the image's
  natural 480×474 where Chrome gives 198×196. Every single ancestor is byte-exact, which is what
  makes it a one-box defect rather than a cascade.

⚠ **And the obvious reading of the second one is already refuted.** The idiom is Tailwind's
`class="w-full h-full object-cover"`, and reduced — a 480×474 image inside a `230×431` card with
`padding:16px` and a `height:288px` inner box — **our engine is Chrome-exact at 198×288**, both for
the plain case and with `position:absolute; inset:0`. Whatever makes the live page take the natural
size, it is not that rule failing generically.

## A flex item loses its specified `width` when the container is shrink-to-fit AND a sibling grows (t1113)

Following t1112's trace to `www.marktplaats.nl`'s `<i>`, the chase found a different, much more
common defect on the way. Two conditions, both required, Chrome-measured at `16px/1 monospace` with
`.icon{width:24px;height:24px}`:

```text
                                                             Chrome   ours
   r1  inline-block > flex > span[flex:1] + i.icon             24      10   ✗
   r2  inline-block > flex > span          + i.icon  CONTROL   24      24   ✓
   r3  …span[flex:1 1 auto] + i.icon                           24      10   ✗
   r4  …span[flex-grow:1]   + i.icon                           24      10   ✗
   r5  width:300px  > flex > span[flex:1] + i.icon   CONTROL   24      24   ✓
```

**The rule: when a flex container is shrink-to-fit and ANY item is growable, the OTHER items lose
their specified `width` and fall back to their content size.** `10` is the width of the `@` glyph.
r2 and r5 are what make it a two-condition rule rather than a general flex bug — remove either the
growable sibling or the indefinite container and the row is exact.

⚠ **And it is not `<i>`, not inline-block, and not flex generally** — three earlier fixtures cleared
all of those. Eight rows of `display:inline-block; width:24px` on `<i>`/`<span>`/`<em>`/`<b>`, empty
and non-empty, inline and in a class: **Chrome-exact on all eight.** Six rows of a `width:24px` flex
item in a definite-width row, with and without a `flex:1` sibling, as `<i>`/`<span>`/`<div>` and
inline-block: **Chrome-exact on all six.** The defect needs the *combination*.

### Reach, and where the code is

`display:flex` is **46% of the burndown corpus** (a floor — it greps inline CSS only), and
"text grows, icon stays fixed" inside an auto-width container is the shape of every toolbar, nav bar,
search field, card header and chip row on the modern web.

Flex is delegated to **Taffy**, so the defect is in what we hand it: the measure closure at
`layout_flex_or_grid` maps `AvailableSpace::MaxContent → None` for the known width and then measures
the item, and it is that measurement — not Taffy's distribution — that must still honour the item's
own `width`. The next tick has the five rows above as its gate.

## A definite `width` IS the box's intrinsic contribution — both of them (t1114)

t1113 localised the rule; this is the fix. `min_content_width` and `max_content_width_uncached` both
lay the subtree out and measure how far the **children** reach, and neither asked the box itself — so
`width:24px` on a box containing a 10px glyph reported **10** for both. CSS Sizing §5.1: a box with a
definite preferred size contributes exactly that size, and content wider than it simply overflows.

```text
                                                         Chrome   before   after
   inline-block > flex > span[flex:1]        + i.icon      24      10       24
   inline-block > flex > span[flex:1 1 auto] + i.icon      24      10       24
   inline-block > flex > span[flex-grow:1]   + i.icon      24      10       24
   inline-block > flex > span                + i.icon CTRL 24      24       24
   width:300px  > flex > span[flex:1]        + i.icon CTRL 24      24       24
```

⚠⚠⚠ **FIXING MAX-CONTENT ALONE CHANGES NOTHING, AND THAT IS THE TRAP.** `shrink_to_fit` returns
`pref.min(avail.max(min_content))`, so a min-content of 9.6 pulls the 24px item straight back down
the moment a growable sibling squeezes it. Both halves, or neither — and the engine's own
`MANUK_TRACE_INTRINSIC` is what said so, printing `avail=24 -> 24.0` (fixed) directly above
`avail=0 -> 9.6` (not).

Only `Dim::Px` qualifies. A percentage against an indefinite constraint is not definite and must fall
through to the content measurement; so must a `calc()` with a percentage term. The value returned is
a CONTENT width, because that is what every caller consumes — `shrink_to_fit` hands it straight to
`layout_children` — so a `border-box` width has its own padding and border removed.

### What it is worth, stated exactly

```text
   css/CSS2 per-TEST state diff   3900 → 3902   +2 GAINED, 0 LOST
       bidi-text/direction-applies-to-015 · tables/anonymous-table-box-width-001
   manuk-layout                   158/158
   four-site panel, TWO interleaved runs of each binary:  every column IDENTICAL
```

**The corpus movement is zero and is reported as zero.** `www.marktplaats.nl` and `hnhbkis.edu.in`
are still `h_overflow 2`; wikipedia and news.ycombinator are byte-identical across both binaries and
both rounds. The fix is right by spec and moves the suite; it does not move these four sites, and
check #104's finding — a construct's frequency is not its leverage — applies to this one too.

⚠ **The residue, measured:** the flex CONTAINER's own max-content is still short by the fixed item.
Our shrink-to-fit `inline-block > flex > span[flex:1] + i.icon` comes out **48** where Chrome says
**72** — the items are now each exact and their sum is not. That is Taffy's contribution handling for
a `flex-basis: 0` item, one level up from this fix, and it is the next brick.

### Two more gate rows were written and deleted

A `box-sizing:border-box` row and a `width:50%` negative both stayed **GREEN** under mutations that
should have killed them: `layout_html`'s `MinimalCascade` does not carry the properties they turn on,
and `shrink_to_fit`'s `min` squeezes the percentage row back to its glyph either way. **A row that
cannot go red is not coverage.** Third time this session that running every mutation caught a fake
assertion — the rule earns its keep.

## A flex child FILLS the 1e6 measuring width, and the slack heuristic then throws away its items (t1115)

t1114 left the flex container's own max-content short by exactly the fixed item — 48 where Chrome
says 72. One variable separates the rows, Chrome-measured, `16px/1 monospace`, `.icon{width:24px}`
inside `inline-block > flex`:

```text
                                          Chrome   ours
   span[flex:1]        + i.icon             72      48   ✗
   span[flex:1 1 auto] + i.icon             72      48   ✗
   span                + i.icon   CONTROL   72      72   ✓
   span[flex:0 1 auto] + i.icon   CONTROL   72      72   ✓
```

**`flex-grow` is the single discriminator** — and the mechanism is not in the flex algorithm at all.
The engine's own `MANUK_TRACE_INTRINSIC=k1`:

```text
   [max-content] #k1 pref=48.2
       child Some("div") [0 0 1000000x24]
```

The inline-block measures its max-content by laying its subtree out at **1e6**. The flex row, being
block-level, FILLS that width; the `flex:1` span grows to a million pixels and the 24px icon is
carried out to x≈999,976. `content_right_extent` then does exactly what it was built to do — discard
a box that filled the measuring width, and discard an offset larger than `SLACK` — and recurses into
the inline text, finding the span's 48.2 and never seeing the icon at all. **Every step is the
heuristic working as designed; the composition loses a real item.**

A non-growing item cannot scatter, which is why the two controls are exact and why `flex-grow` looks
like the cause.

**The fix is to stop measuring an exploded layout**: a flex/grid child that filled the measuring
width should be asked for its OWN max-content — `max_content_width_uncached` already has that branch
(it delegates to taffy) and reaches it only when the node ITSELF is flex/grid, never when a child is.
⚠ Note the branch that *does* exist is not reached at all here: instrumenting
`taffy_tree::max_content_width` printed nothing on these fixtures.

### The fix: a filled flex box answers for itself (t1116)

Stop walking it. `content_right_extent` gains a `flex_max_content` callback: a box that filled the
measuring width AND is a flex/grid container reports its own max-content instead of having its
scattered children summed.

```text
                                                    Chrome    before   after
   span[flex:1]        + i.icon                      72.17     48       72
   span[flex:1 1 auto] + i.icon                      72.17     48       72
   span                + i.icon           CONTROL    72.17     72       72
   span[flex:0 1 auto] + i.icon           CONTROL    72.17     72       72
   the flex row with `padding: 0 10px`               92.17     48       92
   the flex row with `border-left:3;border-right:7`  82.17     48       82
   span[flex:1] + TWO fixed icons                    96.17     48       96
```

⚠ **The padding and border rows are not decoration.** The answer is a CONTENT width and `rect.x` is
the BORDER-box edge, so the box's own frame has to come back on — and the LEFT half especially,
because that is the half which normally arrives through the descendants we have just stopped walking.
The first cut came out 10 short on `padding: 0 10px` and 3 short on `border-left: 3px`.

```text
   css/CSS2 per-TEST state diff   3902 → 3902   0 changed
   manuk-layout                   159/159
   panel, TWO interleaved runs of each binary:
     hnhbkis.edu.in    shape 0.9274 → 0.9316   (+0.42 pts, both runs, same n=234)
     en.wikipedia.org  0.7877/0.7880 → 0.7870/0.7909   — bands overlap, no signal
     news.ycombinator / www.marktplaats            byte-identical
```

One site moves reproducibly and small; nothing regresses. Stated as measured.

## An atomic inline IS a line box, and the bare ones were invisible to the search (t1131)

CSS 2.1 §10.8.1 gives an `inline-block` its baseline in one sentence: **the baseline of its last
in-flow line box**, falling back to the **bottom margin edge** only when it has *no in-flow line
boxes* or its `overflow` is not `visible`. Both clauses were implemented. The DOMAIN of the search
was not.

`last_line_baseline` walks a box's children back-to-front looking for something with a baseline. A
replaced kid answers with its own bottom margin edge and is never searched (t967 — asking an `<img>`
what is inside it is a question the spec never asks). Every other kid was recursed into. So a kid
that was **itself an atomic inline holding no text** — an icon `<span>` sized entirely by CSS, an
empty `inline-flex` chip, a `display:table-cell` spacer — recursed, found no text fragment, returned
`None`, and the OUTER box concluded it had no line boxes at all and took the fallback.

It is off by exactly one strut descender, and only on the line whose ONLY occupant is a box:

```text
   <div>                                              Chrome    before    after
     <span display:inline-block 100x20>                 24        28        24
     ...nested one level deeper again                   24        32        24
     ...display:inline-flex / display:table-cell        24        28        24
     ...the inner atomic at overflow:hidden             24        28        24
     ...the inner atomic with margin-bottom:10px        34        44        34
     the OUTER box at overflow:hidden           CTRL    28        28        28
     TEXT beside the atomic on the same line    CTRL    24        24        24
     an empty inline-block, nothing nested      CTRL    24        24        24
```

**The controls are the diagnosis.** A line carrying text found a fragment and was right; a *bare*
atomic with nothing wrapping it took the fallback and was right by accident. Only the composition —
a box inside a box — reaches the wrong branch, and that is why it survived: every simple form of it
worked.

### It is one rule with two implementations, and both had it

`last_line_baseline` answers §10.8.1 for inline layout; `first_line_baseline` answers CSS Box
Alignment §9 for a flex or grid item. A baseline-aligned flex item whose only content was a bare
atomic put its sibling 4px low for the same reason. They now share `kid_own_baseline`, and the
display list that decides *what is atomic* is shared with the box collector as `is_atomic_inline` —
because the collector decides which children BECOME atomics and the search decides which children
COUNT as lines, and if those two lists ever disagree the search walks straight past a line box that
exists.

The composition is worth stating because it looks like an inconsistency: computing a container's
**first** baseline asks each atomic on that line for **its last** line box. Two different questions —
*which line box is first* and *where that line box's baseline is* — and §10.8.1 answers only the
second. Chrome-measured: beside a 30px-wide `inline-block` holding two lines of `16px/20px`
monospace, a baseline-aligned sibling lands at `dy 21` (the atomic's second line, baseline 36), not
at 16.

### The non-`visible` clause is NOT gated on being atomic, and a WPT test says so

The first version of this fix corrected the atomic arm alone and took `css/CSS2` from 3907 to
**3906**. `css/CSS2/linebox/baseline-block-with-overflow-001` pairs an `overflow:hidden` **block**
child against an `overflow:hidden` **inline-block** child and asserts they render identically —
whichever it is, the search stops there and takes its bottom margin edge. Both arms had been wrong
together, so the test passed on the cancellation; correcting one arm made the test's reference right
and its subject stale. **A whole-suite pass-SET diff caught it and the headline count would not
have**, because a −1 hides inside a 3,907 as easily as a +1 does.

### What it does NOT buy, and why that is the interesting half

Fourteen CrUX-trend sites are **byte-identical** across the fix (same hour, old binary rebuilt from
the checked-out tree; the one apparent mover was the site drifting under two runs of the SAME
binary). The reach is bounded by how often a line's only occupant is a **non-replaced** atomic, and
the two dominant icon idioms both dodge it: an `<svg>` icon is replaced and already took the
bottom-margin-edge branch, and an `<i class="fa">` carries a generated glyph, which is a text
fragment. What is left — CSS-background icon spans, spacers, empty chips — is real, is Chrome-exact
now, and is below what the shape metric resolves.

## `line-height: 0` is a value, and the strut's two halves cancel (t1132)

A line box's STRUT is the containing block's own font metrics folded into every line the block
produces. It contributes `ascent + half_leading` above the baseline and `line_height - ascent -
half_leading` below it, where `half_leading = floor((line_height - (ascent + descent)) / 2)`.

At `line-height: 0` that half-leading is **negative**, and the two halves cancel to exactly zero.
That is not a degenerate case — it is the entire point of the idiom. `line-height: 0` is the
standard reset for the whitespace between `inline-block`s, and the standard wrapper for an icon or
a sprite: **109 of the 373 stylesheets the burndown corpus loads declare it.**

`line_metrics` computed the first half that way and the second half through a guard:

```rust
let above = strut.ascent.round() + hl_s;
let below = if strut.line_height > 0.0 { strut.line_height - strut.ascent.round() - hl_s }
            else                       { strut.descent.round() };   // <- the defect
```

So a declared `line-height: 0` fell into the `else` and handed back the **raw font descent** — a
descender the `above` line had already subtracted. Every line under such a container was ~3px too
tall, and a text-only one 8px too tall.

```text
                                                    Chrome    before    after
   line-height:0, text only                           0         8         0
   line-height:0, one 100x20 inline-block            20        23        20
   ...the same plus text on the line                 20        23        20
   ...the same with a 60px-tall atomic               60        63        60
   ...with an <img> in place of the atomic           20        23        20
   line-height 10/16/20/30/40px               CTRL  10/16/20/30/40 — exact BEFORE and after
```

### Two things this fixture says that the failing row alone does not

**The ladder is why this is one clause and not a re-derivation.** Five non-zero line-heights were
already Chrome-exact, and every single failure was the `line-height: 0` row of its family. The
number being wrong makes the half-leading arithmetic the obvious suspect — and a change there would
have moved all six rows of every family.

**The `else` was never guarding what it looked like it was guarding.** It reads as a guard for a
MISSING strut (a caller with no block style in hand). A missing strut is `(0, 0, 0, 0)`, and both
forms compute `0` for it. The only input that reached the branch was a *declared* zero.

And the tell was ten lines away the whole time: the **text-fragment** arm of the same function has
never carried the guard (`below.max(line_height - a - hl - sh)`). One rule, two implementations, and
the special case on only one of them — the same shape as t1131 one function over.

`css/CSS2` **3907 → 3948, +41 and 0 lost** (36 in `linebox/`, three `normal-flow/inline-*-height`,
two `margin-collapse`, one `floats-placement`); `css-flexbox` 309 → 311; grid / position / sizing /
text / display pass-sets identical.

### The suite counts drift across hours, and only a same-hour set diff attributes

t1131 banked `flexbox 310 / grid 211`. The same committed source, rebuilt an hour later, reads
`309 / 210` — while each binary is internally stable (two runs, byte-identical pass sets). So the
reftest suite carries a ~1-test across-hour drift that a within-hour repeat cannot see. Comparing
today's count against a number in an old journal entry can manufacture a ±1 that is not yours;
comparing two same-hour pass SETS cannot.

## A `<br>` is a BREAK, not an inline box on the line it ends (t1137)

Its fragment carried `ascent = descent = 0` with `style.line_height` set to the `<br>` **element's
own** line-height, which sends it down `close_line`'s metric-less arm —
`min_h_down = min_h_down.max(f.style.line_height)`, a **floor on the line box**. So a `<br>` made the
line it terminates taller than that line's own content, at three magnitudes:

```text
                                                chrome   before   after
  One<br>two                                      36       37       36
  One<br style="line-height:40px">two             36       58       36
  One<br style="font-size:40px">two               36       66       36
  4 lines / 8 lines by <br>                     72/144   76/152   72/144
  One<br style="line-height:0">two         CTRL   36       36       36
  the same two lines by WRAPPING           CTRL   36       36       36
  One<span style="line-height:40px">x</span>two CTRL 40     40       40
  white-space:pre newline                  CTRL   36       36       36
```

**Chrome answers 36 to every `<br>` row and 40 to the `<span>` row.** That is the whole rule: an
inline box's `line-height` grows its line, and a break's does not.

### The wrapped control is what identifies the mechanism

The `<br>` ladder alone (18 · 37 · 56 · 76 · 152 against Chrome's 18 · 36 · 54 · 72 · 144) reads as a
`line-height: normal` constant that is ~1px too big, and the font-size ladder agrees — 10px, 13.333px,
14px, 20px, 24px and 32px were all +1 at two lines. **Every one of those readings points at the
strut, and the strut was never wrong.** The row that says so is the same two lines produced by
WRAPPING instead: exact, at every count, before and after. A battery that walks only the construct
that showed the symptom will name the wrong organ.

### And its own rect is the font's CONTENT AREA

Chrome reports `0 × 17` for a `<br>` at 16px/normal in every row above — including
`line-height:40px` and `font-size:40px` — and at `dy 6` inside a 30px line, where the 17 sits at the
half-leading. We reported `0 × 19 / 40 / 48 / 30` at `dy 0`.

Both defects fall out of one correction: the `<br>` fragment is a **zero-width copy of the STRUT**
(the containing block's font metrics and line-height) rather than a box built from the `<br>`'s own
style. The strut is already folded into every line, so the copy cannot grow one; and having real
`ascent`/`descent` puts it on `close_line`'s text arm, which is what gives it a content area at the
right offset. The box stays — `getBoundingClientRect` on a `<br>` is how caret and editor libraries
find line ends (t380).

### The gate that was pinning the engine to the bug

`a_table_cells_baseline_alignment_aligns_the_first_lines_of_its_row` asserted
`h_two > h_one + shift` — *"the row grows by the baseline shift AND a whole extra line."* That is
**reasoned, not measured**, and it is wrong: the second line drops into space the baseline shift has
already opened under the tall cell. Chrome, on the gate's own markup: **37 · 51 · 69** for one, two
and three lines. The inequality was satisfied only by the `<br>` inflation (52), and it went red the
moment the defect was fixed — t1004's shape exactly. Replaced with the measured pair: the row grows,
and the THIRD line (which lands below the shift, where nothing absorbs it) adds exactly one full line.

### ⚠ A GREEN mutation, recorded rather than acted on

Deleting `close_line`'s metric-less arm outright leaves this gate — and all 169 `manuk-layout`
tests — **green**, with a text-bearing control and again with an empty-inline one. An empty
`<span style="line-height:40px">`'s 40 does not come from that arm (its reporter fragment carries real
metrics), so after this tick the `<br>` was very likely the arm's only real occupant. That is a note
for a later tick, not a deletion in this one: *"no test covers it"* is how a real behaviour gets
removed.

**Same-hour HEAD-binary control, pass-SET diff:** `css/CSS2` **3963 → 3973 (+10), zero losses** —
`white-space-processing-016/017/018`, `white-space-008`, `block-in-inline-first-line-001`,
`table-anonymous-objects-212`, and four `*-applies-to-017/008/015` rows. Six batteries, 180 rows:
`lines` 30/30 · `lines2` 19/19 · `tblbr` 4/4 · `tcell` 45/45 (was 41/45 — the `<br>` fix closed
t1134's four residual rows too) · `tcell2` 58/59 · `lines3` 18/23.

⚠ **Residue, named:** a MIXED-FONT-SIZE line is still 1px too tall — `One<span
style="font-size:40px">x</span>two` is 46 against Chrome's 45, with the span at `dy 1` against 0.
Untouched by this tick and identical before it; the fold of independently-rounded ascents is the
place to look.

## `line-height: normal` rounds the PARTS, not the SUM — a constant fitted at one SIZE (t1138)

The rule was `(ascent + descent + gap).round()`. It is `ascent.round() + descent.round() +
gap.round()`. Chrome, Liberation Sans, one line, `line-height: normal`:

```text
             8   11   16   22   24   26   36   38   40   44   46   56   72   96  128
  Chrome     9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
  before     9   13   18   25   28   30   41   44   46   51   53   64   83  110  147
  after      9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
```

A 44-row ladder — 36 sizes plus serif and monospace at four sizes each — reads **44/44 after and
30/44 before**. The serif and monospace rows are the independent confirmation: different metric
tables, not re-fitted.

### Why it survived, and what the old doc got wrong

The old `height()` argued the point explicitly: *"rounding each term first gives 14 + 3 = 17 for
Liberation where Chrome says 18 — a rule that looks equally plausible written down and is wrong on
the very first face."* **14 + 3 omits the gap.** `round(0.523) = 1` puts it back, and 14 + 3 + 1 = 18
— the same answer. At 16px the two rules cannot be told apart, and 16px is where the whole comparison
was made.

The doc also leaned on breadth: *"verified against real Chrome on three faces. Three is the point:
one face cannot distinguish this rule from rounding the parts separately."* Three faces, **all at
16px**. Varying the face does not separate these rules. Varying the SIZE does. This is the
constants-fitted-at-one-point class (t1042-1046) with the parameter held fixed being the font size,
and the counter-example that justified the wrong branch having a term missing.

### Why no ranking could have found it

Every miss is **±1, in both directions** — +1 at 11, −1 at 22, −1 at 26, −1 at 36, +1 at 38, +1 at
40, +1 at 44, −1 at 46, −1 at 56, +1 at 72 — and 30 of 36 sizes agreed. That is a rounding-mode
**scatter**, not a drift. Every search this project has run for the placement near-miss has looked for
*one shared constant that snaps many boxes into tolerance at once* (the t267 lever, restated on the
board ever since); a defect whose sign alternates with the fractional part of a scaled metric is
invisible to it, and it cancels in any mean.

⚠ **The CSS 2.1 suite cannot see this fix at all.** `css/CSS2` is byte-identical across the change —
3973 passed, 1687 failed, and the pass SETS are identical — because its reftests use Ahem or run at
16px, the one size where both rules agree. A suite that reports zero is not saying the fix is worth
zero; it is saying the suite does not exercise the parameter. Priced the other way, `line-height:
normal` at a heading size is on every page that has an `<h1>`.

### The gate guards against being re-fitted

`line_height_normal_rounds_each_metric_and_then_sums` asserts the arithmetic (not a host font's
numbers, so it does not depend on which faces are installed) and additionally requires that **at
least ten of its rows actively separate the two rounding rules** — if a later edit narrows the ladder
back toward sizes where both agree, the test fails rather than silently passing. 16, 24, 96 and 128
are kept as the rows where they agree, as controls. RED-proven twice: restoring `round(sum)` fails at
11px, and dropping the gap term fails at 16px — the old doc's own mistake, now a red test.

## UAX #14 is Chrome-exact on 26 of 27 rows — and the 27th was `word-break: keep-all` (t1140)

Taken as a PROBE first, because audit #54's steer is *"grep the map before a capability tick"* and the
map's `partial` row said *"UAX #14 line breaking — the Unicode algorithm rather than a
simplification."* A 27-row battery at a 120px width — hyphen / non-breaking hyphen / em-dash /
en-dash, solidus, URLs, grouped numbers and currency, version strings, `&nbsp;`, U+200B, CJK per
ideograph, CJK brackets, sentence and bracket punctuation, `word-break: break-all`,
`overflow-wrap: break-word`, `white-space: nowrap`, and the unbreakable-token overflow — reads
**26/27 Chrome-exact**.

**That negative result is most of the tick's value.** The break-opportunity surface is done; the row
was carried as `partial` on a class that has been correct for a long time.

### The 27th

`word-break: keep-all` was parsed into `WordBreak::KeepAll` and then thrown away —
`break_segments` had no access to the style — so it behaved as `normal`. Invisible for Latin, which
never breaks mid-word anyway, and a whole rewrap for CJK, which breaks at every ideograph.

```text
                                                    chrome   before   after
  日本語のテキストが折り返される       keep-all         20       60       20
  中文文本应该在任意字符处换行显示     keep-all         20       60       20
  日本語text日本語text日本語           keep-all         20       40       20
  한국어 텍스트는 어절 단위로…  CTRL  keep-all         60       60       60
  alpha-bravo-charlie-delta      CTRL  keep-all         40       40       40
  alphabravo<U+200B>charliedelta CTRL  keep-all         40       40       40
  alphabravo<wbr>charliedelta    CTRL  keep-all         40       40       40
  supercalifragilistic…          CTRL  keep-all         20       20       20
  the same three CJK rows with break-all / break-word / anywhere  60 in both
```

CSS Text §5.1: *"implicit soft wrap opportunities between typographic letter units — classes NU, AL,
AI and ID — are suppressed."* **The control rows are what make it a predicate on the two CHARACTERS
the opportunity sits between, rather than "never break inside a word."** Spaces still break — Korean
is written with them, and that is what the property is FOR. Hyphens still break (class BA/HY is not a
letter unit). A zero-width space still breaks. And the CJK↔Latin boundary in row three *is* a letter
unit on both sides, which is what Chrome's 20 there pins.

The over-fix — suppressing every interior opportunity — is a RED-proven mutation: it takes the hyphen
row from 40 to 20.

**Same-hour HEAD-binary control, pass-SET diff:** `css/CSS2` **3973 → 3973, zero gains, zero
losses** — the suite has no `keep-all` reftest, which is precisely why the row sat `partial` with a
gate that could not see it. Ten batteries, 304 rows, 4 differ (three sub-pixel advance widths, one
`display:none` instrument row).

## A `data:` URI contains a SEMICOLON, and the declaration splitter cut every one in half (t1143)

`parse_declarations` was `text.split(';')`. A `data:` URI carries one:

```text
  src: url(data:font/ttf;base64,AAAA…) format("truetype")
       └────────── fragment 1 ──────┘└──────── fragment 2 ────────┘
```

Fragment 1 has an unterminated `url(`, so `parse_font_face_block` finds no source and **drops the
whole `@font-face`**; fragment 2 is not a declaration at all. Face harvesting runs through this
parser *whichever engine computes the styles*, so the failure is live on the shipping Stylo path —
which is why the measured symptom is a font and not a background.

Chrome-measured on a `file://` fixture, one 147KB TrueType face declared three ways and used as
`font-family: <face>, monospace` so a failure falls back visibly:

```text
                                         chrome    before   after
  src: url(go.ttf)                CTRL    126.56     127      127
  src: url("go.ttf")              CTRL    126.56     127      127
  src: url(data:font/ttf;base64,…)        126.56     145      127
  font-family: monospace          CTRL    144.5      145      145
  font-family: NoSuchFace,monospace CTRL  144.5      145      145
```

### The control row is what named the organ, and the first probe got it backwards

The first battery declared its web font ONLY as a `data:` URI. Every `ProbeGo` row came back
byte-identical to the monospace fallback, and the honest-looking conclusion was *"web fonts do not
load"* — which is very nearly what the map's `partial` row already said. Adding the `url(go.ttf)`
arm inverted it in one run: **web fonts have always loaded; only the `data:` form fails.** A fixture
that cannot express the negative case reports the wrong subject as broken — the same shape as t1042's
`data:`-URI trap, one punctuation mark over.

### Priced before building

```text
  data: payload in an @font-face src ...... 17 of the 166 @font-face pages  (10%)
  ;-bearing data: URI in any CSS url() .... 89 of 761 corpus files          (11.7%)
    of which data:image/svg+xml; ........... 1053 occurrences
```

⚠ The `background-image` half of that population is **not** affected on the live path: Stylo parses
declarations correctly, and only face harvesting comes through this parser. The claim is the font
half, which is measured; the rest is the reason to fix the splitter properly rather than special-case
`src`.

The splitter now tracks `(`/`)` depth and `"`/`'` quoting, so a `;` inside a function or a string is
not a separator. The over-fix — never splitting on `;` — is a RED-proven mutation: it collapses
`color: red; background: blue url(a.png); margin: 0` from three declarations to one.

**Same-hour HEAD-binary control, pass-SET diff:** `css/CSS2` **3973 → 3973, zero gains, zero
losses** — the suite has no `data:`-URI `@font-face` test. Thirteen batteries, 347 rows, 8 differ,
all pre-existing (sub-pixel advance widths, one `display:none` instrument row, and the `@media
(scripting)` / `ex`-vs-monospace rows t1142 priced and deferred).

## A box that is WIDER *and* TALLER is a FACE, and the diff reports the computed family (t1151)

A placement error makes a box *move*. A sizing error makes it wrong on one axis. **A box that is
wider and taller at the same time is neither** — extra width fits MORE text per line, so it cannot
also produce extra lines unless the glyphs themselves are wider. That signature means a different
**face** (or a different used size), and it is the cheapest discriminator available in a box dump:

```text
  www.kuechenmomente.de   chrome [90x18] {Raleway/18}   ours [103x26] {Raleway/18}   +14% w, +44% h
  www.lyreco.com          chrome h3 [747x42]            ours h3 [759x84]             +12px, +2 lines
  www.jatekshop.eu        chrome [91x64]                ours [91x79]                 +1 wrapped line
```

⚠ **`Seen.font` cannot confirm it.** The field was added (t562) precisely so a 2px divergence could
read as *"Chromium used Face A at 13px, we used Face B at 14px"* — but **both sides are populated
from the COMPUTED style**, i.e. the family the cascade asked for. When neither engine can load a
webfont and each falls back to a different local face, the column prints `{Raleway/18}` on both
sides: **agreement, in the exact column built to detect disagreement.**

Refuted while looking for the cause, so nobody re-derives them — all three Chrome-exact:

| idiom | verdict |
|---|---|
| the "bulletproof" `@font-face`: a bare `.eot` `src`, then a second `src` with the real `format()` list | ✅ `parse_font_face_block` collects urls from *every* `src` in order; `manuk-page` tries each in turn, so an undecodable `.eot` costs nothing |
| a PARENT-relative `url(../ext/fonts/x.woff2)` from a sheet in a subdirectory | ✅ resolved against the **stylesheet's** base, not the document's |
| an unresolvable family | ✅ both engines fall back alike on a *local* stack |

So before spending a tick on a wider-and-taller box: the layout is probably right. Ask which face
actually rasterized it, and note that today nothing in the pipeline can answer that.

### Measure the face, do not name it — `canvas.measureText` is the channel `getComputedStyle` lacks (t1153)

The section above ends at *"nothing in the pipeline can answer which face rasterized this box."* It
can now, and the fix is not a new API — it is asking a different question. **`getComputedStyle`
cannot report the used face's NAME**, which is true and was recorded in `Seen.font`'s own comment for
588 ticks. **It does not follow that the face cannot be MEASURED**: set `ctx.font` on a canvas from
the element's own computed style and `measureText` returns the advance the *used* face produces.

Both probes now emit `{family/px/ADVANCE}` for one fixed mixed-width ASCII string
(`Hamburgefonstiv 0123`), measured in each element's own resolved font — Chrome's via canvas, ours
via `FontContext::measure` with a `FontKey` built exactly as `layout::text_style` builds it, so the
number is the one the layout used rather than a second opinion. Cached per distinct font, so it is
one measure per face, not per element. A rejected canvas font string leaves `ctx.font` unchanged, so
a sentinel round-trip reports `0` (absence) instead of the previous element's number.

First reading, and it re-ranks a leg of the burndown:

```text
                          declared          CHROME   OURS    ours is
  kuechenmomente.de       Raleway/18          166     240     +45%
  jatekshop.eu            fira_sansbook/14    129     140     +8.5%
  lyreco.com              Lyreco Renner/18    174     184     +5.7%
  ───────────────────────────────────────────────────────────────────
  kuechenmomente.de       -apple-system/10    102     102      0      <- CONTROL
```

**The control is the half that makes it a measurement.** `-apple-system` is unresolvable in both
engines on Linux, both fall back alike, and the advance matches to the pixel — so the probe is
comparable and the divergences above are real. Where the family is a **webfont**, Chrome has the
face and we do not, every text box is that much wider, prose re-wraps, the line count changes, and
the error arrives downstream as `dy` — scored as *shape*.

⚠ Adding this moves `instrument_tag()` (it hashes the probes' own text), so rows banked after it
cannot be silently diffed against older ones. Designed behaviour: a step change in the instrument is
not an error bar on the subject.

### One hundred `@font-face` rules for one family — `unicode-range` (t1154)

The Google-Fonts CSS block, inlined, is the commonest webfont delivery on the web, and it is
**subsetted by codepoint**. `www.kuechenmomente.de` ships **170 `@font-face` rules, 100 of them named
`Raleway`** — weights {400, 700} × styles {normal, italic} × ~13 `unicode-range` subsets — with the
Cyrillic and Vietnamese blocks *first* in source order and Latin further down.

`unicode-range` has **zero occurrences in `engine/`**. `manuk_css::FontFace` is `{ family, srcs }`
(`engine/css/src/lib.rs:2608`), so the loader walks all hundred blocks under one name and hands each
arriving face to `FontContext::register_named_font`, where `face_id` selects on **weight and style
only**. A Cyrillic subset and the Latin subset are indistinguishable to that search, and a face
picked for weight 400 / normal may have no Latin glyphs to shape with — so the run falls back and
every box on the page is measured in the wrong face.

⚠ **The performance half is the same bug.** With no `unicode-range` there is no reason not to fetch
all hundred subsets, so a page Chrome serves with *one* woff2 can cost a hundred requests against a
render deadline. `unicode-range` is not only how the right face is chosen — it is how the other
ninety-nine are never asked for.

⚠ **This is NOT the `src` parsing.** The bulletproof `.eot`-first list, parent-relative
`url(../…)` and the fallback path are each Chrome-exact (t1151). Fetching and registering a face
works. The missing descriptor is the one that says **which** face.

Acceptance test, and it only exists because the face-advance probe landed first (t1153):
`{Raleway/18}` advance **240 → 166**, `fira_sansbook/14` **140 → 129**, `Lyreco Renner/18`
**184 → 174**, and the `-apple-system/10/102` control unmoved.

#### What `unicode-range` fixed, and what it did not (t1155)

Landed: the descriptor is parsed (explicit ranges, bare codepoints, and the `U+4??` **wildcard**,
which is a range — reading it literally would silently restore the old behaviour for the faces that
use the commonest short spelling), and `engine/page` **skips the fetch** for a block whose range
covers none of the document's codepoints. `G_WEBFONT_UNICODE_RANGE` counts requests: **4 → 1**.

⚠ **An unparseable component invalidates the whole descriptor** (CSS Fonts §4.5) → `None` → *"all
codepoints"*. A range we cannot read makes a face a CANDIDATE; dropping just the bad component would
narrow coverage on a guess and could hide the one face a page needs.

⚠ **The skip happens AFTER `declare_webfont_family`.** CSS Fonts' shadowing rule is about the
declaration (t561): the family is claimed by the document whether or not this subset is wanted, or a
locally-installed same-named face would mask it. Only the fetch is skipped.

**And the pre-registered acceptance test did not move:** `Raleway/18` is still 240 against Chrome's
166, `fira_sansbook/14` 140 vs 129, `Lyreco Renner/18` 184 vs 174. A reduced four-subset family
(Cyrillic first, Latin last) lays out in Ahem at exactly 100px **with the skip and without it** —
`face_id`'s per-glyph fallback already reaches a face that has the glyphs. So on a page where the
right face *arrives*, selection was never the failure, and the remaining question is whether it
arrives at all: the fetch, its timing against the render deadline, or the format.

## swash reads `size(0)` as FONT UNITS, and `font-size: 0` is a RESET, not an edge case (tick 1160)

`ShaperBuilder::size` and `ScalerBuilder::size` both **default to `0`**, documented as *"equivalent
to the units per em of the font"*. Passing a CSS `font-size: 0` straight through therefore did not
ask for a zero-width run — it asked for the run measured in the font's own **design units**, and
swash answered truthfully. **One space came back 569px.**

**Why that is a layout bug and not a curiosity.** `font-size: 0` on a container is the pre-flexbox
reset for the whitespace between `inline-block` children:

```css
.grid { font-size: 0 }                                  /* kill the inter-column space   */
.col  { display: inline-block; font-size: 16px }        /* put the text size back         */
```

The whole point of the idiom is that the space between two columns collapses to nothing. With a
569px space between every pair, **every grid built this way stacked one column per line** — the
layout the idiom exists to prevent. The `font-size:0` / `line-height:0` reset is in **109 of the 373
stylesheets** the burndown corpus loads.

**The ladder is what names the organ, because the symptom is indistinguishable from a wrap.** Two
`inline-block`s of 40px and 70px:

```text
                                            CHROME    before      after
   font-size:0    40 + sp + 70   in 230px   1 line    2 lines     1 line
   font-size:0    10 + sp + 10   in 230px   1 line    2 lines     1 line   <- NOT an overflow (20px!)
   font-size:0    40 + nbsp + 70 in 230px   1 line    2 lines     1 line   <- NOT a break opportunity
   font-size:0    40 + 70, no space         1 line    1 line      1 line   CTRL
   font-size:1px  40 + sp + 70              1 line    1 line      1 line   CTRL — the edge is EXACTLY 0
   font-size:16px 40 + sp + 70              1 line    1 line      1 line   CTRL
```

The `10 + 10` row rules out an overflow — 20px of content in a 230px box. The `&nbsp;` row rules out
a break-opportunity decision: **a non-breaking space must never break**, so whatever forced the line
was not reading break opportunities at all. The `1px` row says the boundary is *exactly zero*, not
"small". What is left is the advance, which is where the inline trace was pointed and where it
printed `space=569.0`.

**Clamped at `shape_run`, the one funnel**, because `measure` and `shape_bidi` both bottom out
there; fixing either call site would have left the other holding font units — the *one rule, N
implementations* shape this repo has paid for at t720, t1027, t1131 and t1134.

⚠⚠⚠ **The raster half was nearly waved through as a green mutation, and measuring it said 1.9 MB.**
`rasterize` hands the same `size` to swash's *scaler*, which shares the convention. The reasoning
*"at size 0 `shape_bidi` yields no glyphs, so guarding it is dead code"* is comfortable and was not
checked. The measurement:

```text
   rasterize("A", size=0)   ->  1358 x 1409   1,913,422 bytes   ...and CACHED in the glyph LRU
   rasterize("A", size=16)  ->    11 x   12         132 bytes
```

That is the actual root of tick 15's `font-size: 0` "glyph-shaped continents". `rasterize` is a
`pub` entry point; its only other protection is that callers happen to iterate glyphs a same-size
shaping produced. The guard is on the **question** (*we never meant to ask for font units*), not on
the output size — *"how big is too big"* is a per-face heuristic, while the convention is a fact
about the API.

⚠⚠⚠ **This section's own tick-15 entry, sixty lines up, has named the convention the whole time** —
and prescribed *"any rasteriser needs a guard on glyph bitmaps larger than a few multiples of the
font size"*. **No guard was ever built.** Tick 15 fixed the symptom's other cause (the parser dropped
a unitless `0`, so the size stayed inherited), and the seam kept answering in font units for another
eleven hundred ticks — with the measure half never implicated at all. **A documented mechanism with
no gate is an unmeasured claim.**

**Measured** against a same-hour old binary, pass-sets diffed: `css/CSS2` **3973 → 3974**, and the
one test that flipped is `css/CSS2/linebox/line-breaking-font-size-zero-001.html` — the suite's own
name for the defect. `css-flexbox`, `css-grid`, `css-sizing` and `css-text` pass-sets byte-identical.

Gated by `a_zero_font_size_measures_zero_and_not_the_fonts_design_units` (manuk-text, with the 1px
and 16px control rows that make the boundary *exactly* zero) and by the 96-cell layout battery
`intrinsic_keywords_and_the_font_size_zero_inline_block_grid`. Both RED-proven by deleting the
early return.

## The band's next anchor is FONT-limited, not layout-limited — and a six-row control is what says so (t1333)

`www.hdnails.it` sits at 60.5–62.2% SHAPE on 1,076 ids (its own error bar, two runs). Its top
mechanism is **102 `<span>` widths off by ~8–13px**, and the oracle's absolute example carries the
font annotation both engines measured:

```text
    span(1)/span(1)/span(2)/span(1)
      Chrome  [166 650 61x21]  {Favorit Std/19/187}
      ours    [169 393 74x19]  {Favorit Std/19/204}
```

Same family, same size, **different advance** — 74 against 61, 21% wide. That is the signature of a
different USED FACE, not of different layout arithmetic, and the next three histogram bars (div
heights at ~256px and ~128px) are what a 21% text-width error does to wrapping.

### The control that turns a hypothesis into a classification

⚠ *"Our text metrics are wrong"* and *"Chrome loaded a font we did not"* produce the same box, so the
divergence alone cannot tell them apart. Six strings, one 19px line, measured in both engines:

```text
    Helvetica, Arial, sans-serif     132     132
    Arial                            132     132
    'Times New Roman', serif       122.4   122.4
    monospace                      194.5   194.5
    sans-serif  (AVWiljMM 0123)    133.7   133.7
    'Favorit Std', Helvetica       132     132        ← both FALL BACK identically
```

⭐ **Six of six identical, including the fallback row.** Our shaping, our face selection and our
advance accumulation agree with Chrome to a tenth of a pixel on every system family. So the anchor's
102 hits are **a webfont Chrome has and we do not** — the family name comes from computed style (both
say `Favorit Std`) while the face behind it differs.

**The classification is the deliverable:** this anchor is not layout work. Sending layout ticks at it
would be the burndown's own *"MISSING_BOX cannot move this band"* mistake in a different costume, and
the six-row control is what makes that statement rather than a guess.

## `letter-spacing` dropped every unit it did not know, and a dropped value read as `normal` (t1371)

`letter-spacing` was one of the handful of properties `stylo_engine.rs` recovers from
`MinimalCascade` after Stylo has run — "because Stylo's servo build exposes them as a
`Spacing<Length>` we'd otherwise map by hand". The mapping is four lines. The cost of not writing
them was **every font-relative unit**, because `MinimalCascade` resolves the length through
`values::dimension_to_px`, which maps **`"em" | "rem"` to the same arm** and returns `None` for
anything else — and for this property `None` means *zero spacing*, i.e. a declaration that vanishes
without a trace and renders identically to `normal`.

Chrome-measured, `font-size: 20px` monospace, `Hamburgefonstiv 0123` (20 chars), root 16px:

```text
                                 Chrome used   Chrome box   before   after
  normal                            —             240.83     241      241   CTRL
  2px                               2px           280.83     281      281   CTRL
  -1px                             -1px           220.83     221      221   CTRL
  .1em  (font-size longhand)        2px           280.83     281      281
  .1em  (after a `font:` shorthand) 2px           280.83     273      281   KEY
  .15ch                             1.80615px     276.95     241      277   KEY
  .1rem (root 16px)                 1.6px         272.83     281      273   KEY
  .1em at font-size 40px            4px           561.64     562      562
```

⭐ **Three different ways to get the BASIS wrong, and one fix answers all of them.** `ch` was a unit
the resolver had never heard of. `rem` was a unit it had heard of and aliased to `em`, so it used
the element's 20px where the root's 16px was meant — **the one row whose old value was too LARGE**,
which is why *"we drop spacing"* was never the whole story. And `.1em` after a `font:` shorthand was
the right unit against the wrong number: that cascade had not established the basis, so it resolved
against the inherited 16px.

The fix is to stop resolving it twice. `stylo_map` takes Stylo's own computed `LetterSpacing` — a
`LengthPercentage` Stylo has already resolved against the correct bases, the same machinery whose
`width: 40ch` and `max-width: 50ch` are Chrome-exact today.

⚠⚠ **THIS IS THE FOURTH PROPERTY CAUGHT BEING RECOVERED FROM A CASCADE THAT COMPUTES IT IN A
DIFFERENT CONTEXT** — t923 (`sup`'s `vertical-align`), t1366 (`<td>`'s), t1368 (`align`'s), now this.
**A recovery is a second implementation, and a second implementation of a UNIT is a second answer.**
Before adding a recovery, ask what CONTEXT the other cascade will compute the value in.

**Named residue, measured and not fixed:** `ex` is now applied but on the wrong basis — `.2ex` at
20px monospace is 2.2px in Chrome (the face's real x-height) and 2.0px here, because Stylo's servo
build resolves `ex` as a flat `0.5em` with no font metrics wired. It was DROPPED before (241) and is
281 now against Chrome's 284.83: strictly closer, still wrong, deliberately not asserted.
**`word-spacing` is inert in LAYOUT** — its value now comes from Stylo alongside `letter-spacing`,
but `word-spacing: 10px` on `a b c d` measures 114.30 in Chrome and 84 here, the same as `normal`.
`manuk-layout` reads `style.word_spacing` and the advance never changes, so that gap is downstream
of the cascade.

## `word-spacing` and the path that has no separate space (t1372)

`word-spacing` worked everywhere except the content that preserves its spacing on purpose. In the
WRAPPING path an inline run never contains a space — the inter-word space is its own item and
`space_before` pays both spacings for it. Under `white-space: pre` there is no such split: the
preserved spaces travel INSIDE the run's text, so that arm never runs and the property was dropped
for code blocks, ASCII tables, terminal transcripts and `<pre>`-formatted logs.

⭐ **`letter-spacing` was never dropped there, and that is why this read as a `word-spacing` bug.**
The run's width already pays `letter_spacing` once per CHARACTER, and a space is a character, so the
`pre` path looked half-correct — one of the two spacings survived it. **The defect was the PATH, not
the property**; the fix adds a separator term to the run's own width, which is zero in the wrapping
path (a run there holds no separator) and therefore cannot double-pay it.

```text
   font: 20px/1.2 monospace, `a b c d`             Chrome   before   after
     word-spacing:10px                              114.30    114     114   CTRL (already right)
     white-space:pre; word-spacing:10px             114.30     84     114
     white-space:pre; word-spacing:1ch              120.42     84     120
     white-space:pre; 10px, U+00A0 separators       114.30     84     114
     white-space:pre; 10px, U+2003 separators        84.30     84      84   ← NOT a separator
     white-space:pre; 10px + letter-spacing:2px     128.30    104     128
```

⚠⚠ **THE SEPARATOR SET IS MEASURED, NOT READ OFF THE SPEC.** U+0020 SPACE and U+00A0 NO-BREAK SPACE
each take the full spacing; **U+2003 EM SPACE does not.** CSS Text 3 lists more word-separator
characters than Chrome charges, and the obvious implementation — *"charge the spacing for every
whitespace character in the run"* — passes every other row and widens every em space and every tab
on every `<pre>` on the web.

⚠ CORRECTING THE RECORD: t1371 stated that "`word-spacing` is inert in LAYOUT". That is wrong. Its
fixture set `white-space: pre` on every row to keep the advance measurable, so it exercised only the
one path where the property was dropped; `word-spacing: 10px` on ordinary wrapping text was already
Chrome-exact at 114.30. **A fixture that fixes one variable to make a measurement possible has also
fixed it for the conclusion.**
