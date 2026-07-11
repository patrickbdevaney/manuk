# REPOMAP 03 — Text: font matching/fallback, shaping, bidi, line-breaking, rasterization

Comparative map of how Blink, Gecko, WebKit, Ladybird, and the Rust `parley`
stack implement the text pipeline, to guide Manuk's lean-Rust swash-based engine.

---

## 1. Scope & sources

| Engine | Paths (all under `/home/patrickd/manuk/`) |
|---|---|
| **Blink** | `chromium/third_party/blink/renderer/platform/fonts/` and `.../fonts/shaping/` (RunSegmenter, HarfBuzzShaper, ShapeResult{,View}, NGShapeCache, FrameShapeCache, ShapingLineBreaker, FontFallbackIterator, FontCache) |
| **Gecko** | `firefox/gfx/thebes/` (gfxFont, gfxTextRun, gfxHarfBuzzShaper, gfxGraphiteShaper, gfxScriptItemizer, gfxFontFeatures); `firefox/intl/lwbrk/` (LineBreaker, LineBreakCache, Segmenter); `firefox/intl/bidi/` |
| **WebKit** | `WebKit/Source/WebCore/platform/graphics/` (FontCascade{Fonts,Cache}, ComplexTextController, SystemFallbackFontCache) + `.../graphics/harfbuzz/`, `.../coretext/`, `.../skia/`; `WebKit/Source/WebCore/platform/text/` (BidiResolver, Hyphenation, UnicodeBidi) |
| **Ladybird** | `ladybird/Libraries/LibGfx/TextLayout.{h,cpp}`, `FontCascadeList.{h,cpp}`, `Font/` (Typeface, TypefaceSkia); shaping via HarfBuzz, raster via Skia |
| **Rust refs** | `parley/parley/src/{shape,layout,resolve,analysis}/`, `parley/fontique/src/` (fallback, generic, scan). Note: parley now shapes with **harfrust** (pure-Rust HarfBuzz port) + **skrifa/read-fonts** + **icu4x**, not swash's shaper |
| **Manuk (subject)** | `engine/text/src/lib.rs` (swash `ShapeContext`/`ScaleContext`, fontdb, unicode-bidi); web-font fetch in `engine/page/src/lib.rs:181` |

---

## 2. Per-engine approach

### Blink (Chromium) — the reference architecture

- **Run segmentation.** `RunSegmenter` (`shaping/run_segmenter.h:26`) combines
  `ScriptRunIterator`, `OrientationIterator`, `SmallCapsIterator`, and
  `SymbolsIterator` into one pass that yields `RunSegmenterRange{start,end,
  script, render_orientation, font_fallback_priority}`. Segmentation (script +
  orientation + emoji/symbol priority) happens **before** shaping and before
  font selection — this is the clean layering Manuk's flat `segment()` lacks.
- **Font fallback.** `FontFallbackIterator` (`font_fallback_iterator.h:22`) is a
  single-use iterator walked by `HarfBuzzShaper`: CSS list → `@font-face` ranges
  → system fallback. `NeedsHintList()` optimization asks whether the next font
  needs the full unmatched-character hint list (segmented/unicode-range fonts) or
  just one char — avoiding cost on platforms (Win/Android) whose system-fallback
  APIs need a character cluster. `FontCache::FallbackFontForCharacter` /
  `PlatformFallbackFontForCharacter` (`font_cache.h:106,253`) hit the OS.
- **Shaping.** `HarfBuzzShaper` drives HarfBuzz per segment, re-running the
  fallback iterator for uncovered (`.notdef`) glyph ranges. Output is
  `ShapeResult` (`shaping/shape_result.h:134`, GC'd), storing per-character data
  incl. **safe-to-break** flags harvested from HarfBuzz cluster flags
  (`shape_result_cursor.cc:129`, `SetSafeToBreakBefore`).
- **Shaping caches (three tiers — Blink's key innovation):**
  1. `NGShapeCache` (`ng_shape_cache.h`) — keyed on
     `{text, start/end offset, locale, font-features, direction}` (`ShapeCacheKey`),
     word/small-run granularity; `can_cache` guards non-idempotent reuse.
  2. `FrameShapeCache` (`frame_shape_cache.h`, 2025) — per-`Font`, per-frame LRU
     of both `PlainTextNode`s and `ShapeResult`+ink-bounds, purged on
     `DidSwitchFrame()`. Caches paragraph nodes, not just words.
  3. `ShapeResultView` (`shape_result_view.h:25`) — **read-only composite of
     views** into paragraph-level shape results. Lines reference slices of the
     paragraph shape (see the ASCII diagram at `:35`) with no copy or reshape.
- **Line breaking.** `ShapingLineBreaker` (`shaping_line_breaker.h:26`) shapes
  the whole paragraph once, finds the candidate break by available width, scans
  back to a valid UAX#14 opportunity (`LazyLineBreakIterator`, `Hyphenation`),
  and **only reshapes the line edges when the break is not HarfBuzz-safe**. This
  "shape-once, reshape-edges-only" design is the highest-leverage idea for Manuk.

### Gecko (Firefox)

- **Itemization.** `gfxScriptItemizer` (UAX#24 script runs) feeds `gfxFontGroup`,
  which shapes with `gfxHarfBuzzShaper` (or `gfxGraphiteShaper` for Graphite
  fonts — a capability no other engine here has).
- **Word cache (Gecko's signature optimization).** `gfxFont` holds a
  per-font `mWordCache` (`gfxFont.h:2299`) of `gfxShapedWord` (`:1313`) keyed by
  `WordCacheKey{text ptr, length, hash, flags}` (`:2228`). Words up to
  `WordCacheCharLimit()` are shaped once and reused everywhere; entries expire on
  a timer (`kShapedWordCacheMaxAge = 3`, `WordCacheExpirationTimer`). Long runs
  bypass via `ShapeTextWithoutWordCache` (`:2182`).
- **Text run.** `gfxTextRun` (`gfxTextRun.h:94`) partitions text into `GlyphRun`s
  (`:504`) each carrying one `gfxFont` + match type; most runs are a single
  GlyphRun. `GlyphRunIterator` walks them for painting.
- **Line breaking.** `firefox/intl/lwbrk/LineBreaker.h:16` (static UAX#14/#29
  engine) backed by **icu4x** `LineSegmenter`/`LineBreakIteratorUtf16`
  (`intl/lwbrk/Segmenter.h`). `LineBreakCache` (`LineBreakCache.h:44`) is a fixed
  MRU (prime size 4093) because breaking Thai/Khmer is slow; the cache key
  includes the segmenter flags (`:24`). Gecko is the clearest model for how to
  cache line-break results, not just shaping results.

### WebKit

- **FontCascade.** `FontCascadeFonts` (`FontCascadeFonts.cpp`) realizes fallback
  ranges lazily: `realizeFallbackRangesAt(...)` (`:200`) builds `FontRanges` per
  fallback index; `glyphDataForCharacter` (`:82`) resolves a char through them;
  `glyphDataForSystemFallback` (`:369`) calls
  `systemFallbackFontForCharacterCluster` (Core Text / fontconfig) and is memoized
  by `SystemFallbackFontCache`. Distinct upright/vertical orientation fonts are
  resolved for vertical text (`:323`,`:336`).
- **Complex text.** `ComplexTextController` (`ComplexTextController.h:59`) has a
  3-level hierarchy: `m_run` → per-font "Lines" → `ComplexTextRun`s
  (`:184` comment). Platform backends: `ComplexTextControllerHarfBuzz.cpp`
  (`hb_shape` at `:386`, per-run buffer reset), CoreText, Uniscribe (Win), Skia.
  Vector capacities are tuned from Arabic-Wikipedia statistics (`:193`).
- **Line breaking / bidi.** `platform/text/BidiResolver.h` (template UBA#9
  implementation reused by layout), `Hyphenation.cpp`, ICU-backed `TextBoundaries`.

### Ladybird

- **Simplest production design; leans on Skia + HarfBuzz.** `shape_text()`
  (`LibGfx/TextLayout.cpp:126`) walks the string, picking a font per code point
  via `FontCascadeList::font_for_code_point` (`FontCascadeList.h:72`, with
  `TriggerPendingLoads` for pending `@font-face`), grouping into runs that switch
  font. `setup_text_shaping` (`:165`) builds an `hb_buffer`, guesses segment
  properties, calls `hb_shape` (`:204`). Output `GlyphRun`/`DrawGlyph`
  (`TextLayout.h:24`) caches a Skia `SkTextBlob` per scale (`ensure_text_blob`,
  `:56`) — raster/paint caching lives in the blob, not a coverage cache.
- Fallback list is built by unicode-range (`add(font, Vector<UnicodeRange>)`);
  bidi handled at LibWeb layer feeding `TextType::{Ltr,Rtl}` into shaping.

### parley / fontique (Rust reference — closest to Manuk)

- **Shaping backend.** `parley/src/shape/mod.rs` shapes with **harfrust**
  (pure-Rust HarfBuzz) over **skrifa** `FontRef`, segmenting with a swash-derived
  cluster loop (`:292`). It maintains **three shaping caches** (`shape/mod.rs:29`):
  `shape_data_cache` (per font blob+index, `ShapeDataKey`), `shape_instance_cache`
  (per variation instance), and `shape_plan_cache` (per script/lang/feature plan)
  — mirroring HarfBuzz's own face/font/plan split. This is the parley design most
  worth borrowing conceptually.
- **Fallback.** `fontique/src/fallback.rs` maps `FallbackKey{script, locale}` →
  `[FamilyId]` (data-driven, per-script + per-locale), replacing Manuk's flat
  `FALLBACK_FAMILIES` const list. `fontique/src/scan.rs` + `impl_fontconfig.rs`
  do system discovery (fontdb's role).
- **Analysis / layout.** `parley/src/analysis/` does bidi (icu) + script
  itemization; `layout/line_break.rs` implements UAX#14 line breaking;
  `layout/{run,line,cluster}.rs` build the reusable line structure. Deps pull
  `icu_normalizer`/`icu_properties` (`parley/Cargo.toml:47`).

---

## 3. Manuk today (honest state)

Pipeline in `engine/text/src/lib.rs`:

- **Discovery/matching:** `fontdb` system load; `resolve_family()` (`:272`) maps
  CSS `font-family` lists → generics / named / heuristic (mono/serif substring).
  `@font-face` registered by CSS name via `register_named_font` (`:232`),
  bold/italic variant picked in `face_id` (`:318`).
- **Segmentation + fallback:** `segment()` (`:510`) splits into maximal
  same-face runs using `resolve_face()` (`:452`) — primary face if it covers the
  char (swash charmap, memoized in `coverage`), else first covering face from the
  fixed `FALLBACK_FAMILIES` list (`:120`). Per-char coverage only; no
  script/locale awareness.
- **Shaping:** swash `ShapeContext` per run with LTR/RTL direction (`shape_run`,
  `:525`). Bidi via `unicode_bidi::BidiInfo` visual runs (`shape()`, `:558`).
  Kerning/ligatures/complex scripts come free from swash.
- **Caching:** `measure_cache` LRU (advance width by `{font,size,text}`, `:160`)
  and `glyph_cache` LRU (raster coverage by `{face,size,glyph,subpixel bucket}`,
  `:165`). **No shaped-glyph-run cache** — `shape()` re-runs HarfBuzz every call.
- **Raster:** swash `ScaleContext` with hinting, 4 subpixel buckets (`:602`),
  color glyphs via `Source::ColorBitmap`/`ColorOutline` then `Outline`
  (`:623`) — COLR/CBDT emoji supported.

**Gaps (honest):**
1. **WOFF2 unsupported** — `engine/page/src/lib.rs:181` only passes raw sfnt
   (`\x00\x01\x00\x00`/`true`/`OTTO`/`ttcf`); WOFF/WOFF2 silently dropped. Most
   real web fonts ship WOFF2 → they just don't load.
2. **No shaped-run/word cache.** Only advance widths and glyph bitmaps are
   cached; the glyph *layout* (`ShapedRun`) is recomputed per paint/measure.
3. **Line-break quality.** No UAX#14 line breaker in this module at all — breaking
   lives elsewhere and is presumably whitespace-based; no Thai/Khmer/CJK breaking,
   no hyphenation, no safe-to-break reshape avoidance.
4. **No `font-feature-settings` / `font-variation-settings`** plumbed into swash
   (no small-caps, no arbitrary OT features, **no variable-font axes**).
5. **Fallback is script-blind** — a fixed global family list, not
   `{script,locale}`-keyed; wrong CJK face for JP-vs-SC-vs-KR ambiguity, no
   language-tag influence.
6. **Emoji ZWJ sequences / variation selectors** — per-char coverage +
   `charmap().map(ch)` breaks multi-codepoint emoji clusters and VS15/VS16
   presentation selection (needs `FontFallbackPriority`-style emoji itemization).
7. **No vertical text / `writing-mode`** (no orientation itemization).
8. Single-threaded (`Rc`/`RefCell`) — fine for focused tab, noted in code.

---

## 4. Fold-in recommendations (ranked by leverage)

**Verdict on parley:** the **swash stack is sufficient** — do **not** adopt
parley wholesale. Manuk already has the same primitives (skrifa/swash fonts,
a HB-class shaper, fontdb, unicode-bidi) with less coupling. Borrow parley's
*ideas* (three-tier shape cache, `{script,locale}` fallback map, UAX#14 breaker)
selectively. Adopting parley wholesale would pull icu4x + fontique + harfrust and
re-plumb layout — large surface, against the lean mandate. Reserve parley as the
drop-in the module doc (`lib.rs:6`) already anticipates only if complex-script
line layout becomes a sink.

1. **WOFF2 decompression (highest value, smallest scope).** Without it real
   pages fall back to system fonts. There *is* a working path — vendor a Rust
   brotli decompressor + a small WOFF2 table reconstructor (or FFI to
   `woff2`/`brotli` C). Fix at `engine/page/src/lib.rs:184`. Plain WOFF (zlib) is
   trivial and should land first.
2. **Shaped-run cache (word cache).** Add an LRU keyed like Gecko's
   `WordCacheKey` / Blink's `ShapeCacheKey` — `{face, size, features, direction,
   text}` → `ShapedRun`. Layout measures the same words many times per reflow;
   this is the single biggest CPU win and reuses the existing cache pattern.
3. **UAX#14 line breaking + safe-to-break reshape.** Adopt `icu_segmenter` (or
   `unicode-linebreak`) for break opportunities, and harvest swash/HarfBuzz
   unsafe-to-break cluster flags so lines only reshape at their edges
   (Blink `ShapingLineBreaker` model). Unlocks CJK/Thai and correct wrapping.
4. **Script/locale-aware fallback.** Replace `FALLBACK_FAMILIES` (`lib.rs:120`)
   with a `{script, lang}`-keyed map (fontique's `fallback.rs` is a ready model)
   so CJK disambiguation and language tags work.
5. **Emoji itemization.** Add a `FontFallbackPriority::{Text,Emoji}` pass (Blink
   `SymbolsIterator`) so VS15/VS16 and ZWJ clusters segment before per-char
   coverage — shape emoji clusters whole against the color face.
6. **Feature/variation plumbing.** Thread `font-feature-settings` and
   `font-variation-settings` axes into the swash `ShapeContext`/`ScaleContext`
   builders (swash supports both); enables variable fonts + small-caps.
7. **ShapeResultView-style slicing (later).** Once a paragraph shape cache exists,
   let lines reference sub-ranges instead of copying — memory + reshape savings.

**BLOAT to avoid:**
- Don't add a Graphite shaper (Gecko-only, near-zero web use).
- Don't build a multi-backend `ComplexTextController` hierarchy (WebKit/Blink
  need it for CoreText/Uniscribe/Skia; Manuk has one backend — swash).
- Don't port Blink's GC'd `ShapeResult` object graph or per-frame
  `FrameShapeCache`/prefinalizer machinery — a plain LRU suffices single-threaded.
- Skip vertical writing-mode until a concrete need; it multiplies itemization,
  orientation fonts, and raster paths.
- Don't pull icu4x just for properties if `unicode-*` crates cover the need.

---

## 5. Open questions for frontier research

1. **WOFF2 in pure Rust** — is there (or can we cheaply build) a maintained
   pure-Rust brotli + WOFF2 reconstructor, or is a vetted C FFI the pragmatic
   lean choice? This gates real-web font parity.
2. **Cache granularity** — word-level (Gecko) vs. paragraph-node + view (Blink
   FrameShapeCache/ShapeResultView): which wins for Manuk's single-tab,
   repaint-heavy loop without GC infrastructure?
3. **Safe-to-break exposure** — does swash surface HarfBuzz unsafe-to-break
   cluster flags, or must Manuk move to harfrust to get them for edge-only
   reshaping?
4. **Fallback data source** — ship a static `{script,locale}→family` table
   (fontique-style) or query fontconfig/CoreText/DirectWrite per platform? Trade
   determinism/binary-size vs. OS fidelity.
5. **Emoji cluster correctness** — cheapest correct path for ZWJ sequences and
   VS15/VS16 without importing a full emoji-segmentation table.
6. **Variable-font instancing** — cache scaled instances (parley's
   `shape_instance_cache`) vs. re-instancing per draw; memory vs. CPU.
