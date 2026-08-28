//! manuk-text — font discovery, metrics, shaping, and glyph rasterization.
//!
//! CLAUDE.md's text stack is Parley + swash + fontdb. This first pass uses `fontdb`
//! for system font discovery and `fontdue` (a swash-family rasterizer) for metrics
//! and glyph bitmaps, giving a robust, headless-verifiable Latin text path. Parley's
//! higher-level line breaker + bidi + complex-script shaping is the drop-in upgrade
//! behind the [`FontContext`] API — layout and paint only depend on the shapes and
//! metrics returned here, not on the shaper.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::rc::Rc;

use lru::LruCache;
use swash::text::Script;

pub mod woff2;
pub use woff2::{decode_webfont, decode_woff1, decode_woff2};

/// Which font family to resolve. `Named` carries an interned id into the [`FontContext`]
/// family-name registry (a specific installed or `@font-face`-registered family); the rest
/// are the CSS generics. Mapped to concrete faces via `fontdb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
    Named(u32),
}

/// A resolved-font lookup key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontKey {
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    /// ⚠⚠⚠ **THE FAMILY A PRIVATE-USE-AREA CODEPOINT IS ALLOWED TO USE, WHICH IS NOT `family`.**
    ///
    /// css-fonts-4 §char-handling-issues: *"If a given character is a Private-Use-Area Unicode
    /// codepoint, user agents must only match font families named in the font-family list that are
    /// **not generic families**."* [`FontContext::resolve_family`] answers for the element as a
    /// whole and returns at the **first** entry it can honour — so `font-family: sans-serif,
    /// "Font Awesome"` resolves to `SansSerif`, and every icon in the U+E000–U+F8FF block is then
    /// drawn from whatever glyph the system sans face happens to have at that codepoint. That is
    /// the *wrong icon*, silently, rather than a missing-glyph box.
    ///
    /// This is the interned id of the **first non-generic family in the same list** — computed once
    /// beside `family`, carried on the key so it reaches the per-character face resolution in
    /// [`FontContext::segment`], and `None` when the list has no non-generic entry (in which case
    /// the spec's answer is the missing-glyph symbol, not a fallback face).
    ///
    /// It is part of the KEY, not a side-table, because it changes which glyphs a run produces and
    /// the shaped-run cache is keyed on this struct. Two elements that differ only here must not
    /// share a cache entry.
    pub pua_family: Option<u32>,
}

impl Default for FontKey {
    fn default() -> Self {
        FontKey {
            family: FontFamily::SansSerif,
            bold: false,
            italic: false,
            pua_family: None,
        }
    }
}

/// Is `ch` in one of Unicode's three Private Use Areas?
///
/// css-fonts-4 singles these out because a PUA codepoint has **no agreed meaning**: the only font
/// that can render it correctly is one the page named itself. A generic family's glyph at the same
/// codepoint is not a worse rendering of the same character — it is a different character.
pub fn is_private_use(ch: char) -> bool {
    matches!(ch as u32, 0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD)
}

/// Vertical metrics of a font at a given size, in px. `descent` is a positive
/// magnitude below the baseline.
#[derive(Clone, Copy, Debug, Default)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
}

/// One face's own `line-height: normal` box ASCENT — its rounded ascent plus the half-leading CSS
/// 2.1 §10.8.1 hands to the font (`(normal_height − content_area) / 2`, floored the way the strut
/// arithmetic in layout floors it). This is the quantity that UNIONS across the faces of a mixed
/// run; the raw metrics do not (see [`FontContext::line_metrics_for_text`]).
fn box_ascent(m: &LineMetrics) -> f32 {
    let a = m.ascent.round();
    let content = a + m.descent.round();
    a + ((m.height() - content) / 2.0).floor()
}

impl LineMetrics {
    /// Total line box height for `line-height: normal` — **each of `ascent`, `descent` and `gap`
    /// ROUNDED TO A WHOLE PIXEL AND THEN SUMMED**, because that is the number Chrome lays out with.
    ///
    /// A line box is not a place to be more precise than the engine you are being compared
    /// against. Keeping the sum fractional made every line ~0.4px taller than Chrome's: invisible
    /// on one line, and **cumulative down the document**, which is exactly the FID-SWEEP near-miss
    /// signature (`mdx=0` with `mdy` growing with content density — wikipedia 45px over ~110 line
    /// boxes). That much was right the first time; WHERE the rounding goes was not.
    ///
    /// ⚠⚠⚠ **THIS RULE WAS FITTED AT ONE FONT SIZE AND ITS COUNTER-EXAMPLE DROPPED A TERM.** The
    /// version above this one computed `(ascent + descent + gap).round()` and its doc argued the
    /// point explicitly — *"rounding each term first gives 14 + 3 = 17 for Liberation where Chrome
    /// says 18"*. **14 + 3 omits the gap**, and `round(0.523) = 1` puts it back: 14 + 3 + 1 = 18,
    /// the same answer, which is why 16px could not tell the two rules apart. Neither could the
    /// three FACES the doc leaned on — *"three is the point"* — because they were all measured at
    /// one size. **Varying the face does not separate these rules; varying the SIZE does.**
    ///
    /// A 36-size ladder against real Chrome, Liberation Sans, `line-height: normal`, one line:
    ///
    /// ```text
    ///                8   11   16   22   24   26   36   38   40   44   46   56   72   96  128
    ///   Chrome       9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
    ///   round(sum)   9   13   18   25   28   30   41   44   46   51   53   64   83  110  147
    ///   round parts  9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
    /// ```
    ///
    /// **15 of 15, and the old rule misses 10 of them — in BOTH directions**, which is the tell
    /// that it was a rounding-mode error rather than a wrong constant. 30 of the 36 sizes agreed
    /// before, and every disagreement was ±1: a scatter, not a drift, and therefore invisible to
    /// any ranking that looks for a shared constant.
    ///
    /// Advance WIDTHS are never rounded: Chrome positions glyphs subpixel horizontally, and the
    /// sweep already measures our horizontal placement as exact (`mdx=0`).
    pub fn height(&self) -> f32 {
        self.ascent.round() + self.descent.round() + self.line_gap.round()
    }

    /// The CSS **content area** of an inline box (CSS 2.1 §10.6.1) — `round(ascent) + round(descent)`,
    /// with **no line gap and no dependence on `line-height` whatsoever**.
    ///
    /// This is what `getBoundingClientRect()` reports for a non-replaced inline element, and it is a
    /// *different number and a different rounding rule* from [`height`] above. Getting the two
    /// confused is the whole bug this exists to fix: we were reporting every `<a>`, `<span>` and
    /// `<em>` as `line_height` tall, anchored at the line-box top — so on a `line-height: 1.6` page
    /// every inline element came out ~6px too tall and ~3px too high, on every line, everywhere.
    ///
    /// **The two rules are genuinely opposite, and that is not a typo.** `line-height: normal`
    /// rounds the SUM (tick 269, measured: Liberation 14.484+3.391+0.523 → 18, and rounding the
    /// parts gives 17, which is wrong). The content area rounds the PARTS. Verified against real
    /// Chrome across 2 faces × 8 sizes = 16 points, with no exception:
    ///
    /// ```text
    ///                  size   ascent  descent   round+round   Chrome
    /// Liberation Sans   14px  12.672    2.966      13+3 = 16      16
    /// Liberation Sans   16px  14.484    3.391      14+3 = 17      17   ← +2px size, +1px box
    /// Liberation Sans   32px  28.969    6.781      29+7 = 36      36
    /// DejaVu Sans       16px  14.852    3.773      15+4 = 19      19
    /// DejaVu Sans       32px  29.703    7.547      30+8 = 38      38
    /// ```
    ///
    /// The 14px→16 / 16px→17 pair is the discriminator: a single ratio (or rounding the sum) cannot
    /// produce a 1px growth across a 2px size step. Only per-part rounding does.
    pub fn content_height(&self) -> f32 {
        self.ascent.round() + self.descent.round()
    }
}

/// An index into the [`FontContext`] face registry (a resolved face: a `FontKey`'s primary
/// face, or a fallback face chosen per-glyph for coverage the primary lacks).
pub type FaceId = u32;

/// One placed glyph within a shaped run: a font glyph id (not a `char` — after shaping,
/// ligatures/complex scripts break the one-char-one-glyph assumption), the `face` it was
/// shaped/rasterized from (per-glyph fallback), at pen offset `x`.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPos {
    pub glyph_id: u16,
    pub face: FaceId,
    pub x: f32,
}

/// The result of shaping a text run at a size: placed glyphs + measured extents.
#[derive(Clone, Debug, Default)]
pub struct ShapedRun {
    pub glyphs: Vec<GlyphPos>,
    pub width: f32,
    pub metrics: LineMetrics,
}

/// A rasterized glyph (via swash): placement offsets + an 8-bit coverage bitmap
/// (`width * height`, row-major, top-to-bottom).
///
/// `left` is the horizontal offset from the pen origin to the bitmap's left edge; `top` is
/// the distance from the baseline **up** to the bitmap's top edge (so in screen space, with
/// y growing down, the bitmap's top y is `baseline - top`).
pub struct GlyphBitmap {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    /// `is_color == false`: 8-bit alpha coverage (`width*height`). `is_color == true`:
    /// straight-alpha RGBA (`width*height*4`) for a color/emoji glyph.
    pub coverage: Vec<u8>,
    pub is_color: bool,
}

/// Owns the font database and a cache of rasterizer-ready faces.
///
/// Single-threaded by design (uses `Rc`/`RefCell`) — the focused-tab pipeline runs
/// on one thread. A `Send` variant for the compositor's background tiers is a small
/// change (swap to `Arc`/`Mutex`).
/// Key for the shaped-run/measure cache: `(font, quantized size bits, base RTL, run text)`.
///
/// ⚠ The base direction is part of the key. The same string under an LTR base and an RTL base
/// resolves to a **different visual order**, so omitting it would serve one paragraph's ordering to
/// the other — a cache hit that silently returns correctly-shaped glyphs in the wrong places.
type RunKey = (FontKey, u32, bool, String);
/// Key for the glyph raster cache: `(face, size bits, glyph id, subpixel bucket 0..4)`.
type GlyphKey = (FaceId, u32, u16, u8);

/// Owned font-file bytes + face index, so a swash `FontRef` (which borrows the data) can be
/// built on demand for shaping/rasterization.
struct FaceData {
    data: Vec<u8>,
    index: u32,
}

/// Family names tried (in order) as per-glyph fallback faces for coverage the primary font
/// lacks (CJK, emoji, symbols, Arabic/Hebrew). Only the ones actually installed are used.
const FALLBACK_FAMILIES: &[&str] = &[
    "Noto Color Emoji",
    "Noto Sans CJK JP",
    "Noto Sans CJK SC",
    "Noto Sans CJK KR",
    "Noto Sans Symbols2",
    "Noto Sans Arabic",
    "Noto Sans Hebrew",
    "Noto Sans Devanagari",
    "DejaVu Sans",
];

/// Number of horizontal subpixel positions a glyph is cached at (quarter-pixel).
const SUBPIXEL_BUCKETS: u8 = 4;

pub struct FontContext {
    db: RefCell<fontdb::Database>,
    cache: RefCell<HashMap<FontKey, Option<Rc<fontdue::Font>>>>,
    /// The face registry: interned faces indexed by [`FaceId`], deduped by fontdb id.
    faces: RefCell<Vec<Rc<FaceData>>>,
    face_by_dbid: RefCell<HashMap<fontdb::ID, FaceId>>,
    /// `FontKey` → its primary [`FaceId`] (the resolved family/weight/style face).
    primary_of: RefCell<HashMap<FontKey, Option<FaceId>>>,
    /// A fontdue face per interned [`FaceId`], so a FALLBACK face can be asked for its vertical
    /// metrics through the same reader the primary uses. Built lazily; a face that fails to parse
    /// memoises as `None` so it is not retried per line box.
    face_fonts: RefCell<HashMap<FaceId, Option<Rc<fontdue::Font>>>>,
    /// Discovered fallback faces (lazy); per-`(face, char)` coverage memo.
    fallbacks: RefCell<Option<Vec<FaceId>>>,
    coverage: RefCell<HashMap<(FaceId, char), bool>>,
    /// Per face: does it cover ALL of printable ASCII? One bool, memoised, so the ASCII fast path
    /// in [`FontContext::line_metrics_for_text`] costs a single lookup rather than a per-character
    /// walk of `coverage` on every text fragment of every Latin page.
    ascii_coverage: RefCell<HashMap<FaceId, bool>>,
    /// Interned named font families (id ↔ lowercase name), for `FontFamily::Named`.
    family_names: RefCell<Vec<String>>,
    family_ids: RefCell<HashMap<String, u32>>,
    /// `@font-face` family name (lowercase) → the registered face ids, so a web font
    /// resolves under its CSS-declared name even if the file's internal name differs.
    webfonts: RefCell<HashMap<String, Vec<fontdb::ID>>>,
    /// **Every family name an `@font-face` rule DECLARED**, lowercased — whether or not any of its
    /// `src`s actually loaded.
    ///
    /// This is the CSS Fonts **shadowing** rule, and it needs the declaration rather than the load:
    /// once a document declares `@font-face { font-family: "Open Sans" }`, a locally-installed
    /// `Open Sans` is shadowed *for that document*. If every `src` fails, the family yields **no
    /// usable face** and matching continues to the NEXT entry in the `font-family` list — it does not
    /// silently fall back to the same-named local face.
    ///
    /// Measured consequence of getting this wrong (t559/t560): `martinfowler.com` declares
    /// `Open Sans, sans-serif` and `Open Sans` happens to be installed on the box, so a failed webfont
    /// load was masked by the local face and the page went from 68.2% to 49.2% SHAPE against a Chromium
    /// that had loaded the real webfont. **A failed download must look failed, not like a different
    /// font.**
    declared_webfonts: RefCell<std::collections::HashSet<String>>,
    /// **Every `@font-face` `src` URL this context has already tried**, absolute and resolved.
    ///
    /// The page layer needs an idempotence key because `fetch_and_apply_stylesheets` re-runs after
    /// every round of dynamic scripts, and a newly registered face forces a full-document relayout —
    /// so without one the same font is re-fetched and the whole document re-laid-out per round.
    ///
    /// ⚠ **That key must be the SRC, not the family.** A real site declares one `@font-face` block
    /// per weight and style, all under one family name (regular / italic / 700 / 700-italic — the
    /// shape every self-hosted Google font ships). Keyed on the family, the first block registered
    /// and the other three were skipped, so **every bold and italic run on the page was measured in
    /// the regular face** — and there is no synthetic bold here to soften it. The src URL is stable
    /// across re-runs (same idempotence) and distinct per face (all four now load).
    ///
    /// Recorded on the ATTEMPT, not on success: a 404 that is retried every script round is the same
    /// per-round cost the key exists to prevent.
    attempted_webfont_srcs: RefCell<std::collections::HashSet<String>>,
    /// **The platform UI font — the face `system-ui` names, which is NOT `sans-serif`'s face.**
    ///
    /// `None` only if none of the candidates is installed, in which case `system-ui` falls back to
    /// the sans generic (the old behaviour) rather than to nothing.
    ///
    /// ⚠ These are two different fonts on purpose, and conflating them is a **line-height** bug, not
    /// a glyph-shape nicety. `sans-serif` deliberately resolves to Arial→Liberation Sans because that
    /// is the family *Chrome itself* asks for (see `resolve_generic_families`), whose line box is
    /// 1.150em. `system-ui` goes to the platform UI font — `fc-match system-ui` answers **Noto Sans**
    /// here, and Chromium returns exactly that — whose line box is 1.375em. Measured, 16px "source":
    /// Chrome `50.23x22`, ours `48x18`. **Every line on the page was 4px short.**
    system_ui_family: Option<String>,
    /// swash's reusable scaling context (glyph rasterization). `RefCell` because scaling
    /// takes `&mut`; single-threaded like the rest of the context.
    scale_ctx: RefCell<swash::scale::ScaleContext>,
    /// swash's reusable shaping context (kerning/ligatures/complex scripts).
    shape_ctx: RefCell<swash::shape::ShapeContext>,
    /// Bounded LRU cache of measured run widths (A3 shaped-run cache). Layout measures
    /// the same words repeatedly (per line and in shrink-to-fit's multiple passes), so
    /// caching the advance width skips re-running per-glyph metrics.
    measure_cache: RefCell<LruCache<RunKey, f32>>,
    /// Bounded LRU cache of fully **shaped** runs (glyph ids + positions). Painting shapes
    /// the same runs it already measured, and a scroll re-paint re-shapes every visible run;
    /// caching the whole `ShapedRun` turns that into a clone of a small glyph vector instead
    /// of re-running bidi + swash shaping.
    shape_cache: RefCell<LruCache<RunKey, ShapedRun>>,
    /// Bounded LRU cache of rasterized glyph coverage bitmaps. Painting re-draws the same
    /// glyphs every frame (and every scroll/caret tick repaints the whole viewport), so
    /// rasterizing each glyph fresh each time was the dominant text-paint cost. Cache the
    /// coverage bitmap behind an `Rc` so repeated draws are a hash lookup + clone.
    glyph_cache: RefCell<LruCache<GlyphKey, Rc<GlyphBitmap>>>,
    hits: Cell<u64>,
    misses: Cell<u64>,
}

/// Default capacity (entries) of the shaped-run cache.
const MEASURE_CACHE_CAP: usize = 8192;
/// Default capacity (entries) of the glyph raster cache. Distinct (font,size,char)
/// triples on a page are modest; this bounds memory while covering the visible set.
const GLYPH_CACHE_CAP: usize = 8192;

/// Point `fontdb`'s generic families at the faces the system actually resolves them to.
///
/// The preference lists mirror the order a fontconfig-configured Linux system reports for
/// `sans-serif` / `serif` / `monospace` — the first installed entry wins, exactly as `fc-match`
/// would answer. An explicit `MANUK_FONT_{SANS,SERIF,MONO}` overrides, so a divergence traced to
/// font choice can be pinned without a rebuild.
fn resolve_generic_families(db: &mut fontdb::Database) {
    fn first_installed(db: &fontdb::Database, candidates: &[&str]) -> Option<String> {
        for name in candidates {
            let found = db.faces().any(|f| {
                f.families
                    .iter()
                    .any(|(fam, _)| fam.eq_ignore_ascii_case(name))
            });
            if found {
                return Some((*name).to_string());
            }
        }
        None
    }

    let pick = |db: &fontdb::Database, env: &str, candidates: &[&str]| -> Option<String> {
        std::env::var(env)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| first_installed(db, candidates))
    };

    if let Some(f) = pick(
        db,
        "MANUK_FONT_SANS",
        // **Chrome's order, not fontconfig's.** `fc-match sans-serif` answers "Noto Sans" on this
        // machine — but Chrome never asks fontconfig for the bare generic. It asks for its own
        // default family, *Arial*, and fontconfig substitutes the metric-compatible Liberation Sans.
        // The difference is not cosmetic: Noto's line box is 1.362em against Liberation's 1.150em,
        // an 18% error on the height of every line on every page. Measured against Chromium, not
        // assumed. This is why the naive `fc-match` list turned the box-parity wall red — it was
        // red for a real reason.
        &[
            "Arial",
            "Liberation Sans",
            "Arimo",
            "Helvetica",
            "DejaVu Sans",
            "Noto Sans",
            "FreeSans",
        ],
    ) {
        db.set_sans_serif_family(f);
    }
    if let Some(f) = pick(
        db,
        "MANUK_FONT_SERIF",
        // Chrome's default serif is *Times New Roman* → Liberation Serif.
        &[
            "Times New Roman",
            "Liberation Serif",
            "Tinos",
            "DejaVu Serif",
            "Noto Serif",
            "FreeSerif",
        ],
    ) {
        db.set_serif_family(f);
    }
    let mono = pick(
        db,
        "MANUK_FONT_MONO",
        &[
            "DejaVu Sans Mono",
            "Noto Sans Mono",
            "Liberation Mono",
            "Courier New",
            "FreeMono",
        ],
    );
    if std::env::var("MANUK_FONT_DEBUG").is_ok() {
        eprintln!(
            "[fonts] sans={:?} serif={:?} mono={:?}",
            db.family_name(&fontdb::Family::SansSerif),
            db.family_name(&fontdb::Family::Serif),
            mono
        );
    }
    if let Some(f) = mono {
        db.set_monospace_family(f);
    }
}

/// Resolve **`system-ui`** — the platform's own UI font, which is a *different family* from
/// `sans-serif` and must not be aliased onto it.
///
/// `fontdb` has no `set_system_ui_family`, so the resolved name is stored on the `FontContext` and
/// reached as a named family. The candidate order is fontconfig's answer for `system-ui` on a
/// desktop Linux box — verified, not guessed: `fc-match system-ui` reports **Noto Sans** here and
/// Chromium's `system-ui` measures exactly Noto Sans (16px "source" → `50.23x22`). `Cantarell` and
/// `Ubuntu` come next because they are the GNOME and Ubuntu desktop UI fonts respectively, and a box
/// carrying one of those is a box whose UI font it is.
///
/// ⚠ The sans list must NOT be reused here even though it ends in the same fonts: it deliberately
/// starts at `Arial`/`Liberation Sans` because that is what *Chrome* asks for when a page says
/// `sans-serif`. Starting `system-ui` there would reproduce the very bug this function exists to fix.
/// `MANUK_FONT_SYSTEM_UI` overrides, like its three siblings.
fn resolve_system_ui_family(db: &fontdb::Database) -> Option<String> {
    let installed = |name: &str| {
        db.faces().any(|f| {
            f.families
                .iter()
                .any(|(fam, _)| fam.eq_ignore_ascii_case(name))
        })
    };
    if let Ok(v) = std::env::var("MANUK_FONT_SYSTEM_UI") {
        if !v.is_empty() {
            return Some(v);
        }
    }
    [
        "Noto Sans",
        "Cantarell",
        "Ubuntu",
        "DejaVu Sans",
        "Liberation Sans",
        "Arial",
    ]
    .into_iter()
    .find(|n| installed(n))
    .map(str::to_string)
}

impl FontContext {
    /// Build a context populated with the system's installed fonts.
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // **Resolve the generic families the way the SYSTEM does.**
        //
        // `fontdb`'s defaults are `Arial` / `Times New Roman` / `Courier New` — Windows names that
        // are usually absent on Linux, so `font-family: sans-serif` silently landed on whatever the
        // query happened to fall back to. Chromium asks fontconfig, gets `Noto Sans` here, and every
        // width it measures is a Noto Sans width. We were measuring a *different font's* widths for
        // every string on every page: the same sentence came out 305px for us and 317px for Chrome,
        // so every line wrapped at a different word and every box below it moved.
        //
        // Font metrics are the dominant source of persistent placement drift, and this — not the
        // metrics engine — is where it starts. Pick the same physical faces the system's own
        // resolver picks, in its own preference order.
        //
        // **Un-flagged, and the flag's red wall turned out to be telling the truth.**
        //
        // This was held back because turning it on took box-parity from 72/72 to 69/72 on `valign`
        // and `white-space-nowrap` — both LINE-HEIGHT probes. The reading at the time was "font
        // selection is right, the metrics under it are wrong; adopt Skrifa (METHODOLOGY Part 15)".
        //
        // That reading was wrong, and measuring Chromium instead of reasoning about it said so. The
        // metrics engine was never the problem. The preference lists were: they were built from
        // `fc-match <generic>`, which answers "Noto Sans" here — and Chrome never asks fontconfig
        // for the bare generic. It asks for its own default family (Arial / Times New Roman), which
        // fontconfig substitutes to the metric-compatible Liberation faces. Noto's line box is
        // 1.362em against Liberation's 1.150em, so every line on every page came out 18% too tall,
        // and two line-height probes noticed.
        //
        // Fix the lists and the same change is green. Skrifa would have replaced a metrics engine
        // that was working, and left the real bug in place. The wall was not an obstacle to route
        // around; it was the finding.
        resolve_generic_families(&mut db);
        let system_ui_family = resolve_system_ui_family(&db);
        if std::env::var("MANUK_FONT_DEBUG").is_ok() {
            eprintln!("[fonts] system-ui={system_ui_family:?}");
        }
        FontContext {
            system_ui_family,
            db: RefCell::new(db),
            cache: RefCell::new(HashMap::new()),
            faces: RefCell::new(Vec::new()),
            face_by_dbid: RefCell::new(HashMap::new()),
            primary_of: RefCell::new(HashMap::new()),
            face_fonts: RefCell::new(HashMap::new()),
            fallbacks: RefCell::new(None),
            coverage: RefCell::new(HashMap::new()),
            ascii_coverage: RefCell::new(HashMap::new()),
            family_names: RefCell::new(Vec::new()),
            family_ids: RefCell::new(HashMap::new()),
            webfonts: RefCell::new(HashMap::new()),
            declared_webfonts: RefCell::new(std::collections::HashSet::new()),
            attempted_webfont_srcs: RefCell::new(std::collections::HashSet::new()),
            scale_ctx: RefCell::new(swash::scale::ScaleContext::new()),
            shape_ctx: RefCell::new(swash::shape::ShapeContext::new()),
            measure_cache: RefCell::new(LruCache::new(
                NonZeroUsize::new(MEASURE_CACHE_CAP).unwrap(),
            )),
            shape_cache: RefCell::new(LruCache::new(NonZeroUsize::new(MEASURE_CACHE_CAP).unwrap())),
            glyph_cache: RefCell::new(LruCache::new(NonZeroUsize::new(GLYPH_CACHE_CAP).unwrap())),
            hits: Cell::new(0),
            misses: Cell::new(0),
        }
    }

    /// `(hits, misses)` on the shaped-run cache — for perf assertions / diagnostics.
    pub fn measure_cache_stats(&self) -> (u64, u64) {
        (self.hits.get(), self.misses.get())
    }

    /// Number of faces discovered — 0 means no system fonts were found.
    pub fn face_count(&self) -> usize {
        self.db.borrow().len()
    }

    /// Is a face for **exactly** this family present, with no fallback in the answer?
    ///
    /// [`resolve_family`] and [`font`] cannot answer this: both fall back to sans-serif when the
    /// name is unknown, so *"does `font-family: Ahem` work"* comes back `true` on a host that has
    /// never seen Ahem. The WPT runners need the un-fallen-back answer to make installing the
    /// suite's ruler idempotent, and a caller that wants to know whether a family is genuinely
    /// available — rather than merely resolvable — has had no way to ask.
    ///
    /// [`resolve_family`]: FontContext::resolve_family
    /// [`font`]: FontContext::font
    pub fn has_family(&self, name: &str) -> bool {
        self.db
            .borrow()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
    }

    /// Resolve (and cache) a fontdue face for `key`, or `None` if unavailable.
    pub fn font(&self, key: FontKey) -> Option<Rc<fontdue::Font>> {
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.clone();
        }
        let loaded = self.load(key);
        self.cache.borrow_mut().insert(key, loaded.clone());
        loaded
    }

    /// Register a downloaded font (no CSS family alias — matched by its internal name).
    pub fn register_font(&self, data: Vec<u8>) {
        self.db.borrow_mut().load_font_data(data);
    }

    /// Register a downloaded `@font-face` font under its CSS-declared `family` name, so
    /// `font-family: family` resolves to it regardless of the file's internal name.
    /// Record that an `@font-face` rule DECLARED this family, independently of whether its `src`
    /// loaded. Call it for every `@font-face` rule the document has, before attempting the fetch —
    /// the shadowing rule is about the declaration, not the download (see [`declared_webfonts`]).
    ///
    /// [`declared_webfonts`]: FontContext::declared_webfonts
    pub fn declare_webfont_family(&self, family: &str) {
        let n = family.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
        if !n.is_empty() {
            self.declared_webfonts.borrow_mut().insert(n);
        }
    }

    /// Has a face already been REGISTERED (not merely declared) for this `@font-face` family?
    ///
    /// The page layer needs this to answer "is this download new?", and the answer is load-bearing
    /// twice over: `fetch_and_apply_stylesheets` runs again after **every** round of dynamic scripts,
    /// so without it the same font is re-fetched and re-registered each round — and, since a newly
    /// registered face now forces a relayout, that would mean a full-document relayout per script
    /// round, which is precisely the waste the relayout guard exists to prevent.
    pub fn has_webfont_face(&self, family: &str) -> bool {
        self.webfonts
            .borrow()
            .get(&family.to_ascii_lowercase())
            .is_some_and(|ids| !ids.is_empty())
    }

    /// Claim one `@font-face` block for fetching: `true` the first time this `(family, url)` pair is
    /// seen, `false` on every later call. This is the per-face idempotence key described on
    /// [`attempted_webfont_srcs`] — it replaces a per-FAMILY check that skipped every weight and
    /// style after the first.
    ///
    /// ⚠ **The family is part of the key, not decoration.** Two different families legitimately name
    /// the same file — an alias, a `Foo`/`Foo Text` pair, or a build that points several declarations
    /// at one subsetted face. Keyed on the URL alone, the second family silently gets no face at all,
    /// which trades one bug for a narrower one.
    ///
    /// [`attempted_webfont_srcs`]: FontContext::attempted_webfont_srcs
    pub fn claim_webfont_src(&self, family: &str, url: &str) -> bool {
        self.attempted_webfont_srcs
            .borrow_mut()
            .insert(format!("{}\u{1}{url}", family.to_ascii_lowercase()))
    }

    pub fn register_named_font(&self, family: &str, data: Vec<u8>) {
        let before: std::collections::HashSet<fontdb::ID> =
            self.db.borrow().faces().map(|f| f.id).collect();
        self.db.borrow_mut().load_font_data(data);
        let new_ids: Vec<fontdb::ID> = self
            .db
            .borrow()
            .faces()
            .map(|f| f.id)
            .filter(|id| !before.contains(id))
            .collect();
        if !new_ids.is_empty() {
            self.webfonts
                .borrow_mut()
                .entry(family.to_ascii_lowercase())
                .or_default()
                .extend(new_ids);
        }
    }

    /// Intern a lowercase family name, returning its stable id.
    /// Intern a CSS family name: **deduped case-INSENSITIVELY, stored case-PRESERVINGLY.**
    ///
    /// Those are two different jobs and this function used to do only the first — it pushed the
    /// lowercased key into `family_names`, so `family_name_of` handed `face_id` a lowercase string and
    /// `fontdb::Family::Name` (case-SENSITIVE) missed, fell through to `Family::SansSerif`, and **every
    /// named family resolved to the same face.** t557 fixed the DETECTION in `resolve_family`; the case
    /// was then discarded one line later, here, which is why the measured widths did not move at all
    /// (five families, five distinct `Named(...)` ids, one `FaceId(0)`, one width). *A fix upstream of a
    /// lossy step is not a fix.*
    ///
    /// So: the lowercased form remains the dedup KEY (CSS family matching is case-insensitive, and
    /// `font-family: ARIAL` and `font-family: Arial` must intern to one id), and the ORIGINAL string is
    /// what gets stored and handed back to the font database.
    fn intern_family(&self, name: &str) -> u32 {
        let key = name.to_ascii_lowercase();
        if let Some(&id) = self.family_ids.borrow().get(&key) {
            return id;
        }
        let mut names = self.family_names.borrow_mut();
        let id = names.len() as u32;
        names.push(name.to_string());
        self.family_ids.borrow_mut().insert(key, id);
        id
    }

    fn family_name_of(&self, id: u32) -> Option<String> {
        self.family_names.borrow().get(id as usize).cloned()
    }

    /// Resolve a CSS `font-family` list to a `FontFamily` we can load: the first entry that
    /// is a generic keyword, an installed/`@font-face` face by that exact name, or a known
    /// named→generic mapping (Courier→mono, Times→serif). Defaults to sans-serif.
    pub fn resolve_family(&self, names: &[String]) -> FontFamily {
        for raw in names {
            // ── THE NAME KEEPS ITS CASE FOR THE FACE QUERY (tick 557).
            //
            // `fontdb::Family::Name` matching is **case-SENSITIVE**, and this loop used to lowercase the
            // family before querying — so `Family::Name("dejavu sans")` returned `None` while
            // `Family::Name("DejaVu Sans")` returns the face, and **no named system family ever
            // resolved.** Measured at t556: `"DejaVu Sans"`, `"Noto Sans"` and a deliberately
            // non-existent `"NoSuchFontXYZ"` all rendered the same 330px wide here, where Chromium gave
            // 374 / 348 / 299 — three different faces. Every named family on the web was silently
            // substituted, which is one defect producing BOTH corpus symptoms: sign-changing text
            // advances (a substituted face has different per-glyph widths) and a constant line-box height
            // delta (different ascent+descent).
            //
            // So `orig` (trimmed, unquoted, ORIGINAL case) goes to fontdb and is what gets interned;
            // `n` (lowercased) stays the key for the generic keywords and the `@font-face` map, which is
            // keyed lowercase by CSS's case-insensitive family matching.
            let orig = raw.trim().trim_matches(['"', '\'']);
            let n = orig.to_ascii_lowercase();
            match n.as_str() {
                "sans-serif" => return FontFamily::SansSerif,
                // ── `system-ui` IS NOT `sans-serif`, AND THE MACOS ALIASES ARE NOT GENERICS AT ALL.
                //
                // These five names used to share this arm and return `SansSerif`, which broke every
                // modern font stack in the same way: they all sit at the FRONT of one, so returning a
                // generic here SHORT-CIRCUITED the whole list and the font Chrome actually picks was
                // never reached. Measured against Chromium, 16px "source" — all four of these stacks
                // came out `48x18` for us:
                //
                //   Bootstrap 5  `system-ui,-apple-system,…`            Chrome `50.23x22` (Noto Sans)
                //   GitHub       `-apple-system,…,"Noto Sans",…`        Chrome `50.23x22` (4th entry)
                //   Tailwind     `ui-sans-serif,system-ui,sans-serif`   Chrome `50.23x22` (2nd entry)
                //   Bootstrap 4  `-apple-system,…,Roboto,…`            Chrome `48.34x19` (**Roboto**)
                //
                // `system-ui` resolves to the platform UI font (`fc-match system-ui` → Noto Sans, and
                // Chromium agrees). `ui-sans-serif` is its CSS-Fonts-4 spelling and resolves the same
                // way — Chrome does not recognise it and falls back to its default font, so a page
                // using it ALONE diverges; every real stack pairs it with `system-ui`, where the two
                // readings agree, and being spec-correct beats reproducing a gap.
                //
                // ⚠ `-apple-system` / `BlinkMacSystemFont` are Blink's **macOS-only** aliases for San
                // Francisco. They are not families on Linux, and Chrome here treats them as unknown
                // and moves on — which is why Bootstrap 4 lands on Roboto. So they get NO arm: they
                // fall through to the named-family path, fail to match, and continue to the next
                // entry, exactly as Chrome does. **On macOS the correct answer is the system UI font,
                // so this wants a platform-conditional alias** — named here rather than pretended
                // otherwise, because the Linux answer is the one that is measured.
                "system-ui" | "ui-sans-serif" => {
                    return match &self.system_ui_family {
                        // ⚠ The ORIGINAL case, never lowered: `face_id` re-queries fontdb with this
                        // exact string and `fontdb::Family::Name` matching is case-SENSITIVE (t557).
                        // Lowering it here would resolve `system-ui` to nothing at all — the same
                        // miss, one call later.
                        Some(f) => FontFamily::Named(self.intern_family(f)),
                        // Nothing installed to be the UI font: the old behaviour, which is at least
                        // a real face, rather than no font at all.
                        None => FontFamily::SansSerif,
                    };
                }
                "serif" | "ui-serif" | "cursive" | "fantasy" => return FontFamily::Serif,
                "monospace" | "ui-monospace" => return FontFamily::Monospace,
                "" => continue,
                _ => {
                    // An @font-face-registered family wins under its CSS name.
                    if self.webfonts.borrow().contains_key(&n) {
                        return FontFamily::Named(self.intern_family(&n));
                    }
                    // ── SHADOWING (CSS Fonts, tick 561). If the document DECLARED this family via
                    // `@font-face` and no face loaded for it, the family has no usable face and
                    // matching must continue to the NEXT `font-family` entry. It must NOT fall back to
                    // a same-named LOCAL face: that is how a failed download starts looking like a
                    // different font instead of like a failure. (Measured at t559/t560 —
                    // `martinfowler.com` declares `Open Sans, sans-serif`, `Open Sans` is installed on
                    // this box, and masking the failed webfont with the local face cost 19 points of
                    // SHAPE against a Chromium that had the real one.)
                    if self.declared_webfonts.borrow().contains(&n) {
                        continue;
                    }
                    // A named family: use it only if fontdb actually has a face whose family
                    // name matches (so unknown names fall through to hints / next entry).
                    // ⚠ `orig`, not `n` — see the case note above; this query is case-sensitive.
                    let q = fontdb::Query {
                        families: &[fontdb::Family::Name(orig)],
                        weight: fontdb::Weight::NORMAL,
                        stretch: fontdb::Stretch::Normal,
                        style: fontdb::Style::Normal,
                    };
                    let db = self.db.borrow();
                    let matched = db.query(&q).is_some_and(|id| {
                        db.face(id).is_some_and(|f| {
                            f.families
                                .iter()
                                .any(|(fam, _)| fam.eq_ignore_ascii_case(&n))
                        })
                    });
                    if matched {
                        // Intern the ORIGINAL case: `face_id` re-queries fontdb with this string, so
                        // lowering it here would reintroduce the same miss one call later.
                        return FontFamily::Named(self.intern_family(orig));
                    }
                    if n.contains("mono") || n.contains("courier") || n.contains("consol") {
                        return FontFamily::Monospace;
                    }
                    if n.contains("times")
                        || n.contains("georgia")
                        || n.contains("serif")
                        || n.contains("garamond")
                        || n.contains("palatino")
                    {
                        return FontFamily::Serif;
                    }
                }
            }
        }
        FontFamily::SansSerif
    }

    /// The **first non-generic family in `names` that has a real face** — the only thing a
    /// Private-Use-Area codepoint is allowed to be drawn with (css-fonts-4
    /// §char-handling-issues). `None` when the list names no such family, which is the spec's
    /// "display some form of missing glyph symbol" case rather than a fallback.
    ///
    /// Deliberately NOT a variant of [`resolve_family`]: that function's whole contract is *"the
    /// first entry we can honour, generics included"*, and the two answers differ for exactly the
    /// stacks this rule is about. Sharing one function behind a flag would make every caller
    /// decide a spec question it does not know it is being asked.
    ///
    /// [`resolve_family`]: FontContext::resolve_family
    pub fn first_non_generic_family(&self, names: &[String]) -> Option<u32> {
        for raw in names {
            let orig = raw.trim().trim_matches(['"', '\'']);
            let n = orig.to_ascii_lowercase();
            // The generics, plus the CSS-Fonts-4 spellings `resolve_family` honours. `emoji`,
            // `math` and `fangsong` are generics we do not implement — they must still be SKIPPED
            // here, or a page listing one before its icon font would pick it as the PUA family and
            // then fail to resolve it to anything.
            if matches!(
                n.as_str(),
                "" | "serif"
                    | "sans-serif"
                    | "monospace"
                    | "cursive"
                    | "fantasy"
                    | "system-ui"
                    | "ui-serif"
                    | "ui-sans-serif"
                    | "ui-monospace"
                    | "ui-rounded"
                    | "emoji"
                    | "math"
                    | "fangsong"
            ) {
                continue;
            }
            // An `@font-face` family wins under its CSS name, exactly as in `resolve_family`.
            if self.webfonts.borrow().contains_key(&n) {
                return Some(self.intern_family(&n));
            }
            // A DECLARED-but-unloaded `@font-face` family shadows any same-named local face, so
            // matching continues to the next entry (t561) — the same rule, for the same reason.
            if self.declared_webfonts.borrow().contains(&n) {
                continue;
            }
            // ⚠ `orig`, not `n`: `fontdb::Family::Name` matching is case-SENSITIVE (t557).
            let q = fontdb::Query {
                families: &[fontdb::Family::Name(orig)],
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            };
            let db = self.db.borrow();
            let matched = db.query(&q).is_some_and(|id| {
                db.face(id).is_some_and(|f| {
                    f.families
                        .iter()
                        .any(|(fam, _)| fam.eq_ignore_ascii_case(&n))
                })
            });
            drop(db);
            if matched {
                return Some(self.intern_family(orig));
            }
        }
        None
    }

    /// The **name of the family this stack actually resolves to** — the answer to *"which face did you
    /// use?"*, which a box on its own cannot give.
    ///
    /// Returns the family name for a `Named(...)` resolution and the generic keyword otherwise, so a
    /// diff can say `Open Sans/13` vs `sans-serif/14` instead of leaving a 2px height divergence
    /// unattributable (t562/t563). `None` only when the list resolves to nothing nameable, which the
    /// caller renders as `?` rather than as a guess.
    pub fn resolved_family_name(&self, names: &[String]) -> Option<String> {
        match self.resolve_family(names) {
            FontFamily::Named(id) => self.family_name_of(id),
            FontFamily::SansSerif => Some("sans-serif".to_string()),
            FontFamily::Serif => Some("serif".to_string()),
            FontFamily::Monospace => Some("monospace".to_string()),
        }
    }

    /// Resolve the fontdb face id for `key` (specific query, else any face).
    fn face_id(&self, key: FontKey) -> Option<fontdb::ID> {
        let named = match key.family {
            FontFamily::Named(id) => self.family_name_of(id),
            _ => None,
        };
        // An @font-face family resolves directly to its registered face ids (bypassing the
        // internal-name query), picking the bold/italic variant when present.
        if let Some(n) = &named {
            // The `@font-face` map is keyed by the CSS name LOWERCASED (family matching is
            // case-insensitive in CSS), while `named` now preserves the author's case for fontdb's
            // case-sensitive query. Lower it here, or a webfont declared `"Fira Sans"` stops resolving.
            let lower = n.to_ascii_lowercase();
            if let Some(ids) = self.webfonts.borrow().get(&lower) {
                if let Some(&id) = ids.iter().find(|&&id| {
                    self.db.borrow().face(id).is_some_and(|f| {
                        (f.weight == fontdb::Weight::BOLD) == key.bold
                            && (f.style != fontdb::Style::Normal) == key.italic
                    })
                }) {
                    return Some(id);
                }
                return ids.first().copied();
            }
        }
        let family = match key.family {
            FontFamily::SansSerif => fontdb::Family::SansSerif,
            FontFamily::Serif => fontdb::Family::Serif,
            FontFamily::Monospace => fontdb::Family::Monospace,
            FontFamily::Named(_) => match &named {
                Some(n) => fontdb::Family::Name(n),
                None => fontdb::Family::SansSerif,
            },
        };
        let query = fontdb::Query {
            families: &[family, fontdb::Family::SansSerif],
            weight: if key.bold {
                fontdb::Weight::BOLD
            } else {
                fontdb::Weight::NORMAL
            },
            stretch: fontdb::Stretch::Normal,
            style: if key.italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        };
        self.db
            .borrow()
            .query(&query)
            .or_else(|| self.db.borrow().faces().next().map(|f| f.id))
    }

    fn load(&self, key: FontKey) -> Option<Rc<fontdue::Font>> {
        let id = self.face_id(key)?;
        let font = self.db.borrow().with_face_data(id, |data, index| {
            let settings = fontdue::FontSettings {
                collection_index: index,
                ..fontdue::FontSettings::default()
            };
            fontdue::Font::from_bytes(data, settings).ok()
        })??;
        Some(Rc::new(font))
    }

    /// Intern a fontdb face into the registry, returning its stable [`FaceId`] (deduped).
    fn intern(&self, dbid: fontdb::ID) -> Option<FaceId> {
        if let Some(&fid) = self.face_by_dbid.borrow().get(&dbid) {
            return Some(fid);
        }
        let fd = self.db.borrow().with_face_data(dbid, |data, index| {
            Rc::new(FaceData {
                data: data.to_vec(),
                index,
            })
        })?;
        let mut faces = self.faces.borrow_mut();
        let fid = faces.len() as FaceId;
        faces.push(fd);
        self.face_by_dbid.borrow_mut().insert(dbid, fid);
        Some(fid)
    }

    /// The primary [`FaceId`] for `key` (resolved family/weight/style), cached.
    fn primary_face(&self, key: FontKey) -> Option<FaceId> {
        if let Some(&hit) = self.primary_of.borrow().get(&key) {
            return hit;
        }
        let fid = self.face_id(key).and_then(|id| self.intern(id));
        self.primary_of.borrow_mut().insert(key, fid);
        fid
    }

    fn face(&self, id: FaceId) -> Option<Rc<FaceData>> {
        self.faces.borrow().get(id as usize).cloned()
    }

    /// The **x-height** in px for `key` at `size` — the OS/2 `sxHeight` of the resolved
    /// primary face, scaled to the font size. This is the CSS **`ex`** unit, and it is read
    /// from the SAME face the shaper lays glyphs with (via swash's `Metrics`, the design-unit
    /// value Chrome also uses), so `ex` and the rendered lowercase text agree. `None` when no
    /// face resolves or the face declares no x-height (leaving the spec `0.5em` fallback).
    pub fn x_height(&self, key: FontKey, size: f32) -> Option<f32> {
        let fid = self.primary_face(key)?;
        let fd = self.face(fid)?;
        let font = swash::FontRef::from_index(&fd.data, fd.index as usize)?;
        let xh = font.metrics(&[]).scale(size).x_height;
        (xh > 0.0).then_some(xh)
    }

    /// The **cap-height** in px for `key` at `size` — the OS/2 `sCapHeight` of the resolved
    /// primary face, scaled to the font size. This is the CSS **`cap`** unit, read from the same
    /// face the shaper draws with (swash's `Metrics.cap_height`). `None` when no face resolves or
    /// the face declares no cap-height (leaving Stylo's `cap = ascent` fallback).
    pub fn cap_height(&self, key: FontKey, size: f32) -> Option<f32> {
        let fid = self.primary_face(key)?;
        let fd = self.face(fid)?;
        let font = swash::FontRef::from_index(&fd.data, fd.index as usize)?;
        let ch = font.metrics(&[]).scale(size).cap_height;
        (ch > 0.0).then_some(ch)
    }

    /// The installed fallback faces, discovered once (lazy).
    fn fallback_faces(&self) -> Vec<FaceId> {
        if let Some(fbs) = self.fallbacks.borrow().as_ref() {
            return fbs.clone();
        }
        let mut out = Vec::new();
        for name in FALLBACK_FAMILIES {
            let q = fontdb::Query {
                families: &[fontdb::Family::Name(name)],
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            };
            let found = self.db.borrow().query(&q);
            if let Some(fid) = found.and_then(|id| self.intern(id)) {
                if !out.contains(&fid) {
                    out.push(fid);
                }
            }
        }
        *self.fallbacks.borrow_mut() = Some(out.clone());
        out
    }

    /// Whether `face` covers EVERY printable ASCII codepoint (cached, one bool per face).
    ///
    /// This exists for one reason: [`line_metrics_for_text`] must walk a run's characters to find
    /// the faces it draws from, and the overwhelming majority of runs on the web are ASCII in a
    /// face that covers ASCII — a case where the answer is known in advance. `str::is_ascii` is a
    /// word-at-a-time scan and this is one hash lookup, so an English page pays neither the
    /// per-character `coverage` probe nor a second allocation.
    ///
    /// ⚠ It is a real question, not a formality: an ICON font is a face that covers almost no
    /// ASCII, and `font-family: FontAwesome` on ASCII text genuinely falls back.
    ///
    /// [`line_metrics_for_text`]: FontContext::line_metrics_for_text
    fn face_covers_ascii(&self, face: FaceId) -> bool {
        if let Some(&hit) = self.ascii_coverage.borrow().get(&face) {
            return hit;
        }
        let all = (0x20u8..=0x7e).all(|b| self.face_covers(face, b as char));
        self.ascii_coverage.borrow_mut().insert(face, all);
        all
    }

    /// Whether `face` has a glyph for `ch` (cached).
    fn face_covers(&self, face: FaceId, ch: char) -> bool {
        if let Some(&hit) = self.coverage.borrow().get(&(face, ch)) {
            return hit;
        }
        let covered = self
            .face(face)
            .and_then(|fd| {
                swash::FontRef::from_index(&fd.data, fd.index as usize)
                    .map(|f| f.charmap().map(ch) != 0)
            })
            .unwrap_or(false);
        self.coverage.borrow_mut().insert((face, ch), covered);
        covered
    }

    /// Resolve which face to shape/render `ch` with: the primary if it has the glyph, else
    /// the first fallback face that does (CJK/emoji/symbols), else the primary (`.notdef`).
    ///
    /// ⚠⚠⚠ **A PRIVATE-USE-AREA CODEPOINT TAKES NEITHER PATH.** css-fonts-4
    /// §char-handling-issues carves it out twice over: only **non-generic** families in the
    /// author's list may match it, and *"if none of the families named in the font-family list
    /// contain a glyph for that codepoint, user agents must display some form of missing glyph
    /// symbol for that character **rather than attempting installed font fallback**"*. Both halves
    /// matter and both are here, because either one alone renders a *different character* and
    /// calls it a fallback: PUA is where every icon font lives, so the failure is Font Awesome's
    /// U+F007 coming out as whatever the system serif keeps at that address.
    fn resolve_face(&self, ch: char, primary: FaceId, pua: Option<FaceId>) -> FaceId {
        if is_private_use(ch) {
            // Only the author's first non-generic family, and no installed-font fallback after it.
            // `primary` here is the missing-glyph carrier, which is exactly the spec's remedy.
            return match pua {
                Some(p) if self.face_covers(p, ch) => p,
                _ => primary,
            };
        }
        if ch.is_whitespace() || ch.is_control() || self.face_covers(primary, ch) {
            return primary;
        }
        for fb in self.fallback_faces() {
            if fb != primary && self.face_covers(fb, ch) {
                return fb;
            }
        }
        primary
    }

    /// The face a Private-Use-Area codepoint may be drawn with, per [`FontKey::pua_family`].
    /// `None` when the list named no non-generic family, or when that family has no face.
    fn pua_face(&self, key: FontKey) -> Option<FaceId> {
        let id = key.pua_family?;
        // Same weight/style as the run: an icon font ships one face, but a text family named as
        // the first non-generic entry may well ship four, and picking the roman for a bold run
        // would change the advance.
        self.primary_face(FontKey {
            family: FontFamily::Named(id),
            bold: key.bold,
            italic: key.italic,
            pua_family: None,
        })
    }

    /// Vertical line metrics for `key` at `size` px. Falls back to a reasonable
    /// estimate when no font is available.
    /// Raw, UNROUNDED face metrics. The rounding that matches Chrome belongs to the **line box**,
    /// not to these values — see [`LineMetrics::height`] and `text_style` in layout.
    pub fn line_metrics(&self, key: FontKey, size: f32) -> LineMetrics {
        if let Some(font) = self.font(key) {
            if let Some(m) = font.horizontal_line_metrics(size) {
                return LineMetrics {
                    ascent: m.ascent,
                    descent: -m.descent, // fontdue descent is negative (below baseline)
                    line_gap: m.line_gap,
                };
            }
        }
        LineMetrics {
            ascent: size * 0.8,
            descent: size * 0.2,
            line_gap: 0.0,
        }
    }

    /// A fontdue face for an interned [`FaceId`] — the same construction [`load`] uses, so the
    /// metrics read out of it are the SAME numbers the primary path reads. Cached per face.
    ///
    /// [`load`]: FontContext::load
    fn face_font(&self, fid: FaceId) -> Option<Rc<fontdue::Font>> {
        if let Some(hit) = self.face_fonts.borrow().get(&fid) {
            return hit.clone();
        }
        let built = self.face(fid).and_then(|fd| {
            let settings = fontdue::FontSettings {
                collection_index: fd.index,
                ..fontdue::FontSettings::default()
            };
            fontdue::Font::from_bytes(&fd.data[..], settings)
                .ok()
                .map(Rc::new)
        });
        self.face_fonts.borrow_mut().insert(fid, built.clone());
        built
    }

    /// Vertical line metrics for `key` at `size` **as used by this actual RUN OF TEXT** — the
    /// primary face's metrics UNIONED with those of every fallback face the run's characters
    /// resolve to.
    ///
    /// ⚠⚠⚠ **THIS IS THE DIFFERENCE BETWEEN A CJK PAGE BEING THE RIGHT HEIGHT AND BEING 25% SHORT,
    /// AND IT IS INVISIBLE IN EVERY LATIN FIXTURE.** `line-height: normal` is the FONT's business
    /// (see [`LineMetrics::height`]) — but *which* font is the run's business, and a run whose
    /// author family is `sans-serif` still draws its CJK glyphs from a CJK face, because
    /// [`resolve_face`] falls back per glyph. Asking only the PRIMARY face gives DejaVu Sans's
    /// metrics for a line made entirely of Noto Sans CJK glyphs.
    ///
    /// Chrome-measured, `font: 16px sans-serif`, `line-height: normal`, one line, 272px block:
    ///
    /// ```text
    ///   content                     family                  Chrome   primary-only
    ///   "Latin only text"           sans-serif                  18     18   ✓
    ///   "1980年代アイドル・歌手 (65)"  sans-serif                  24     18   ✗ 6px SHORT
    ///   "1980年代アイドル・歌手 (65)"  serif                       24     18   ✗
    ///   "松田聖子"                    "Noto Sans CJK JP"          24     24   ✓ named, no fallback
    ///   "Latin only text"           "Noto Sans CJK JP"          24     24   ✓ the FONT, not the script
    /// ```
    ///
    /// The last two rows are the control that settles the mechanism: it is not "CJK text is taller",
    /// it is "**the face the glyphs actually came from is taller**" — Latin text in a CJK face is 24
    /// too. A rule keyed on the SCRIPT would pass the third row and fail the fifth.
    ///
    /// [`resolve_face`]: FontContext::resolve_face
    pub fn line_metrics_for_text(&self, key: FontKey, size: f32, text: &str) -> LineMetrics {
        let base = self.line_metrics(key, size);
        let Some(primary) = self.primary_face(key) else {
            return base;
        };
        // The fast path, and it is the one nearly every run on the web takes: ASCII text in a face
        // that covers ASCII can never leave its primary, so there is nothing to union.
        if text.is_ascii() && self.face_covers_ascii(primary) {
            return base;
        }
        let pua = self.pua_face(key);
        // Small and linear: a run draws from one or two faces in practice, and the loop stops
        // growing the set as soon as every fallback face has been seen once.
        let mut seen: Vec<FaceId> = Vec::new();
        for ch in text.chars() {
            // Whitespace and controls are laid out by the primary face (see `resolve_face`), so
            // they can never introduce a face and are skipped before the coverage memo is touched.
            if ch.is_whitespace() || ch.is_control() {
                continue;
            }
            let f = self.resolve_face(ch, primary, pua);
            if f != primary && !seen.contains(&f) {
                seen.push(f);
            }
        }
        // ⭐ **THE CONTROL ARM IS FREE, AND IT IS THE REASON THIS RETURNS EARLY RATHER THAN
        // UNIONING WITH ITSELF.** A run drawn entirely by its own primary face takes this line and
        // gets the BYTE-IDENTICAL `LineMetrics` the engine has always used — same fractional
        // ascent, same descent, same baseline. Every Latin page on the corpus is therefore
        // untouched by construction, not by hope, and the union arithmetic below only ever runs on
        // a run that actually crossed a face boundary.
        if seen.is_empty() {
            return base;
        }
        // ⚠⚠⚠ **A COMPONENT-WISE MAX OF RAW METRICS IS THE WRONG UNION, AND IT IS OFF BY EXACTLY
        // ONE PIXEL — WHICH IS THE HARDEST SIZE OF ERROR TO NOTICE.** Taking
        // `max(ascent) + max(descent) + max(line_gap)` over the faces gave **25** for
        // `16px sans-serif` CJK where Chrome says 24, because DejaVu Sans's `lineGap` of 0.523
        // rounds to 1 and Noto Sans CJK's is 0 — a term contributed by a face that draws not one
        // glyph on the line.
        //
        // The unit that composes is not the raw metric, it is each face's own FINISHED line box:
        // its `normal` height with its own leading already distributed (CSS 2.1 §10.8.1 — the
        // half-leading belongs to the font, and `A + halfLeading` / `D + halfLeading` are what the
        // line box unions). Measured over the two faces at 16px:
        //
        // ```text
        //   face                 a      d      gap    height   halfLead   box asc   box desc
        //   DejaVu Sans        14.48   3.39   0.52      18         0         14         4
        //   Noto Sans CJK JP   18.56   4.61   0.00      24         0         19         5
        //   union                                       24                   19         5   ✓ Chrome
        //   (component-wise max of raw metrics)          25                              ✗
        // ```
        let mut asc = box_ascent(&base);
        let mut desc = base.height() - asc;
        for f in seen {
            let Some(font) = self.face_font(f) else {
                continue;
            };
            let Some(m) = font.horizontal_line_metrics(size) else {
                continue;
            };
            let fm = LineMetrics {
                ascent: m.ascent,
                descent: -m.descent, // fontdue descent is negative (below baseline)
                line_gap: m.line_gap,
            };
            let a = box_ascent(&fm);
            asc = asc.max(a);
            desc = desc.max(fm.height() - a);
        }
        // `line_gap: 0` because the leading is already inside `asc`/`desc` — `height()` must not
        // add it a second time, and the caller's `half_leading` arithmetic must find none left to
        // distribute.
        LineMetrics {
            ascent: asc,
            descent: desc,
            line_gap: 0.0,
        }
    }

    /// Advance width of a run of text at `size` px. Cached (A3 shaped-run cache): a
    /// repeat measure of the same `(text, font, size)` is an LRU hit that skips the
    /// per-glyph metrics.
    pub fn measure(&self, text: &str, key: FontKey, size: f32) -> f32 {
        // Base direction is irrelevant to WIDTH — bidi reorders runs, it does not resize them — so
        // measurement pins it at `false` and both directions share one cache entry.
        let ck: RunKey = (key, size.to_bits(), false, text.to_owned());
        if let Some(&w) = self.measure_cache.borrow_mut().get(&ck) {
            self.hits.set(self.hits.get() + 1);
            return w;
        }
        // Shape each fallback-segmented run and sum advances (no glyph vec is built).
        let mut total = None;
        if let Some(primary) = self.primary_face(key) {
            // Width is order-independent, so measure without bidi reordering.
            let mut pen = 0.0f32;
            for (face, script, run) in self.segment(text, primary, self.pua_face(key)) {
                self.shape_run(&run, face, script, size, false, |g, _| pen += g.advance);
            }
            total = Some(pen);
        }
        let w = total.unwrap_or_else(|| text.chars().count() as f32 * size * 0.5);
        self.misses.set(self.misses.get() + 1);
        self.measure_cache.borrow_mut().put(ck, w);
        w
    }

    /// Split `text` into maximal runs sharing a resolved face (primary + per-glyph
    /// fallback), so each run can be shaped by a single font.
    fn segment(
        &self,
        text: &str,
        primary: FaceId,
        pua: Option<FaceId>,
    ) -> Vec<(FaceId, Script, String)> {
        use swash::text::Codepoint;
        let mut runs: Vec<(FaceId, Script, String)> = Vec::new();
        for ch in text.chars() {
            let face = self.resolve_face(ch, primary, pua);
            let script = ch.script();
            // `Common` (spaces, digits, most punctuation) and `Inherited` (combining marks) carry
            // no script of their own. Opening a new run for them would cut a word in half — and an
            // Arabic word split at its own comma stops joining across the cut, which is exactly the
            // disconnected-letterforms bug this segmentation exists to prevent. So they EXTEND the
            // run in progress, and only start one (as Latin) when nothing precedes them.
            let neutral = matches!(script, Script::Common | Script::Inherited | Script::Unknown);
            match runs.last_mut() {
                Some((f, s, buf)) if *f == face && (neutral || *s == script) => buf.push(ch),
                _ => runs.push((
                    face,
                    if neutral { Script::Latin } else { script },
                    ch.to_string(),
                )),
            }
        }
        runs
    }

    /// Shape one same-face run via swash, invoking `emit(glyph, x_offset)` per glyph with
    /// the pen advancing; returns the run's total advance width. `rtl` sets the shaping
    /// direction (glyphs are still emitted in visual, left-to-right pen order).
    fn shape_run(
        &self,
        text: &str,
        face: FaceId,
        script: Script,
        size: f32,
        rtl: bool,
        mut emit: impl FnMut(&swash::shape::cluster::Glyph, f32),
    ) -> f32 {
        // ⚠⚠⚠ **SWASH READS `size(0.0)` AS *"FONT UNITS"*, NOT AS ZERO** — its shaper and its scaler
        // both default to `0`, and both document that as *"equivalent to the units per em of the
        // font"*. So `font-size: 0` did not ask for a zero-width run; it asked for the run measured
        // in the font's own design units, and a single space came back **569px**.
        //
        // That is not an obscure value: `font-size: 0` on a container is THE pre-flexbox reset for
        // the whitespace between `inline-block` children (`.grid{font-size:0}` + `.col{display:
        // inline-block;font-size:16px}`), and the whole point of it is that the inter-column space
        // collapses to nothing. With a 569px space between every pair of columns, every such grid
        // stacked one column per line — the layout the idiom exists to prevent.
        //
        // ```text
        //                                            CHROME   BEFORE   AFTER
        //   40px + space + 70px inline-blocks, 230px   1 line  2 lines  1 line
        //   the space's own advance at font-size:0       0      569       0
        // ```
        //
        // Nothing above this clamps, and it is deliberately clamped HERE rather than at each call
        // site: this is the one function that knows what swash was asked, and `measure`, `shape` and
        // the glyph scaler all funnel through the same convention (t720's "one rule, N
        // implementations" — the second implementation is what the tick after this one pays for).
        // A `&nbsp;` broke the line too, which is what proves it was never a break-opportunity
        // decision: an advance is an advance whether or not a break may fall at it.
        if !(size > 0.0) {
            return 0.0;
        }
        let Some(fd) = self.face(face) else {
            return 0.0;
        };
        let Some(font) = swash::FontRef::from_index(&fd.data, fd.index as usize) else {
            return 0.0;
        };
        let dir = if rtl {
            swash::shape::Direction::RightToLeft
        } else {
            swash::shape::Direction::LeftToRight
        };
        let mut ctx = self.shape_ctx.borrow_mut();
        // **The script is what selects the OpenType feature set**, and swash defaults it to
        // `Latin` — so before this, every run on the web was shaped as Latin. Latin needs no
        // joining, no reordering and no conjunct formation, so those features were never applied:
        // Arabic came out as disconnected isolated letterforms (`مرحبا` as five unjoined shapes),
        // and Devanagari as one glyph per codepoint with matras unreordered and conjuncts unformed.
        // Both are *legible-looking* to someone who does not read the script — which is why this
        // survived: nothing was missing, nothing was `.notdef`, the text was simply wrong.
        let mut shaper = ctx
            .builder(font)
            .size(size)
            .script(script)
            .direction(dir)
            .build();
        shaper.add_str(text);
        let mut pen = 0.0f32;
        shaper.shape_with(|cluster| {
            for g in cluster.glyphs {
                emit(g, pen);
                pen += g.advance;
            }
        });
        pen
    }

    /// Shape a text run with kerning/ligatures/complex-script support **and per-glyph font
    /// fallback** (swash), placing each resulting glyph (by glyph id + face) at its pen
    /// position. Runs of characters the primary font lacks are shaped with a fallback face.
    pub fn shape(&self, text: &str, key: FontKey, size: f32) -> ShapedRun {
        self.shape_bidi(text, key, size, false)
    }

    /// Shape a run against an explicit **bidi base direction** (`base_rtl`), which is the paragraph's
    /// resolved `direction` / `dir` — not a property of the text itself.
    ///
    /// The base level is what the Unicode Bidi Algorithm resolves every other run against, so it
    /// decides where a trailing period lands, which end the line starts from, and how embedded Latin
    /// and numbers are ordered inside Arabic or Hebrew. With the wrong base every character is
    /// present and correctly shaped, and the line reads backwards.
    pub fn shape_bidi(&self, text: &str, key: FontKey, size: f32, base_rtl: bool) -> ShapedRun {
        let ck: RunKey = (key, size.to_bits(), base_rtl, text.to_owned());
        if let Some(cached) = self.shape_cache.borrow_mut().get(&ck) {
            return cached.clone();
        }
        let metrics = self.line_metrics(key, size);
        let Some(primary) = self.primary_face(key) else {
            return ShapedRun {
                glyphs: Vec::new(),
                width: text.chars().count() as f32 * size * 0.5,
                metrics,
            };
        };
        let pua = self.pua_face(key);
        let mut glyphs = Vec::new();
        let mut pen = 0.0f32;
        // Bidi: reorder the text into visual runs against the PARAGRAPH's base level, then within
        // each run face-segment and shape with that run's own direction. An LTR base over pure-LTR
        // text yields a single LTR run, identical to the non-bidi path.
        let base = if base_rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let info = unicode_bidi::BidiInfo::new(text, Some(base));
        for para in &info.paragraphs {
            let (levels, vruns) = info.visual_runs(para, para.range.clone());
            for vr in vruns {
                let rtl = levels[vr.start].is_rtl();
                let sub = &text[vr.clone()];
                for (face, script, run) in self.segment(sub, primary, pua) {
                    let advance = self.shape_run(&run, face, script, size, rtl, |g, x| {
                        glyphs.push(GlyphPos {
                            glyph_id: g.id,
                            face,
                            x: pen + x + g.x,
                        });
                    });
                    pen += advance;
                }
            }
        }
        let run = ShapedRun {
            glyphs,
            width: pen,
            metrics,
        };
        self.shape_cache.borrow_mut().put(ck, run.clone());
        run
    }

    /// Rasterize a single glyph (via swash) to an 8-bit coverage bitmap, at the horizontal
    /// subpixel offset `subpixel_x` (its fractional part is quantized into
    /// [`SUBPIXEL_BUCKETS`] positions). Cached by `(font, size, glyph, bucket)` so repeated
    /// draws — every frame, every scroll/caret tick — are a lookup, while crisp subpixel
    /// placement is preserved across the quarter-pixel buckets.
    pub fn rasterize(
        &self,
        glyph_id: u16,
        face: FaceId,
        size: f32,
        subpixel_x: f32,
    ) -> Option<Rc<GlyphBitmap>> {
        let frac = subpixel_x - subpixel_x.floor();
        let bucket = ((frac * SUBPIXEL_BUCKETS as f32).round() as u8) % SUBPIXEL_BUCKETS;
        let gk = (face, size.to_bits(), glyph_id, bucket);
        if let Some(hit) = self.glyph_cache.borrow_mut().get(&gk) {
            return Some(hit.clone());
        }

        // ⚠⚠⚠ **THE SCALER SHARES THE SHAPER'S CONVENTION, AND HERE IT COSTS 1.9 MB A GLYPH.**
        // `ScalerBuilder::size` also defaults to `0` = *"the units per em of the font"*, so a
        // request at `font-size: 0` renders the **unscaled outline**: measured, one `A` comes back
        // **1358x1409, 1,913,422 bytes** against 11x12 / 132 bytes at 16px — and it is then held in
        // the glyph LRU. This is the root of tick 15's `font-size: 0` "glyph-shaped continents",
        // where one word buried old.reddit's post titles under ~27,000px of flat grey.
        //
        // ⚠ **`docs/wiki/text-layout.md` HAS NAMED THIS CONVENTION SINCE TICK 15** — *"any rasteriser
        // needs a guard on glyph bitmaps larger than a few multiples of the font size"* — and no such
        // guard was ever built. What was fixed then was the SYMPTOM's other cause (the parser dropped
        // a unitless `0` so the size stayed inherited); the seam kept answering in font units for
        // another eleven hundred ticks. A documented mechanism with no gate is an unmeasured claim.
        //
        // The guard is on the QUESTION, not on the output size: *"how big is too big"* is a
        // heuristic that has to be tuned per face, while *"we never meant to ask for font units"* is
        // a fact about the API. It is not redundant with `shape_run`'s clamp — this is a `pub` entry
        // point, and its protection would otherwise be that every caller happens to iterate glyphs
        // that a same-size shaping produced.
        if !(size > 0.0) {
            return None;
        }
        let fd = self.face(face)?;
        let font = swash::FontRef::from_index(&fd.data, fd.index as usize)?;

        let mut ctx = self.scale_ctx.borrow_mut();
        let mut scaler = ctx.builder(font).size(size).hint(true).build();
        let offset = swash::zeno::Vector::new(bucket as f32 / SUBPIXEL_BUCKETS as f32, 0.0);
        // Prefer a color bitmap/outline (emoji), then a plain alpha outline.
        let image = swash::scale::Render::new(&[
            swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            swash::scale::Source::ColorOutline(0),
            swash::scale::Source::Outline,
        ])
        .offset(offset)
        .render(&mut scaler, glyph_id)?;

        let is_color = matches!(image.content, swash::scale::image::Content::Color);
        let bmp = Rc::new(GlyphBitmap {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            coverage: image.data,
            is_color,
        });
        self.glyph_cache.borrow_mut().put(gk, bmp.clone());
        Some(bmp)
    }
}

impl Default for FontContext {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    /// A process-lazy, per-thread [`FontContext`] used **only** for CSS unit metrics
    /// (the `ch` unit's `0`-glyph advance). Building a `FontContext` scans the system
    /// font database, so it is kept off the hot path behind this thread-local: the first
    /// `ch` query on a thread pays the scan once, every later query is a `measure_cache`
    /// hit. It carries only system + generic faces — it is **not** the page's context and
    /// has no `@font-face` webfont registrations, which is why [`zero_advance_px`] is
    /// exact for generic/installed families and falls back through the generics for an
    /// unregistered webfont name (the same answer Chrome gives when the webfont is absent).
    static METRICS_CTX: FontContext = FontContext::new();
}

/// The **UAX #9 resolved embedding level of every BYTE** of `text`, as one paragraph whose base
/// direction is `base_rtl`.
///
/// `shape_bidi` already runs the bidirectional algorithm *inside* one text run, so the glyphs of a
/// single Arabic or Hebrew word come out in the right visual order. That is only half of UAX #9:
/// rule **L2** reorders the *whole line*, and a line is made of INLINE BOXES — a footer's twenty
/// `<a>` elements, an `<em>` inside a sentence, an `inline-block` chip — which the shaper never
/// sees because each is measured and placed separately. Layout owns that half, and it needs the
/// levels the shaper computes internally. Exposing them here keeps ONE bidi implementation for the
/// engine: a second copy in the layout crate would be free to disagree with the glyphs it places.
///
/// Byte-indexed (not char-indexed) because the caller's natural key is a `&str` slice offset, and a
/// level is constant across a code point's bytes.
pub fn bidi_levels(text: &str, base_rtl: bool) -> Vec<u8> {
    let base = if base_rtl {
        unicode_bidi::Level::rtl()
    } else {
        unicode_bidi::Level::ltr()
    };
    unicode_bidi::BidiInfo::new(text, Some(base))
        .levels
        .iter()
        .map(|l| l.number())
        .collect()
}

/// The advance width, in px, of the `0` (ZERO) glyph for the font that `families` /
/// `bold` / `italic` resolve to at `size_px` — i.e. the value of the CSS **`ch`** unit.
///
/// This is measured through the SAME shaper (`FontContext::measure`) that layout uses to
/// place text, so `N` characters of a monospace run occupy exactly `N * zero_advance_px`
/// and a `width: Nch` box comes out identical to the text it is sized for — which is the
/// whole point of `ch` and what a constant `0.5em`/`0.6em` approximation can never
/// guarantee to the pixel. Resolution mirrors `layout::text_style`'s `FontKey`
/// construction (`resolve_family` + `weight >= 600` + `italic`) so the metric and the
/// glyphs never disagree.
pub fn zero_advance_px(families: &[String], bold: bool, italic: bool, size_px: f32) -> f32 {
    METRICS_CTX.with(|ctx| {
        let key = FontKey {
            // These three measure `0`, the x-height and the cap-height — never a Private-Use-Area
            // codepoint — so the PUA family is genuinely absent here, not merely unsupplied.
            family: ctx.resolve_family(families),
            bold,
            italic,
            pua_family: None,
        };
        ctx.measure("0", key, size_px)
    })
}

/// The x-height in px (CSS **`ex`** unit) for the font `families`/`bold`/`italic` resolve to
/// at `size_px`, read off the shared metrics context (see [`zero_advance_px`]). `None` leaves
/// Stylo's spec `ex = 0.5em` fallback in place — the honest answer for a face that declares no
/// x-height or a family that does not resolve.
pub fn x_height_px(families: &[String], bold: bool, italic: bool, size_px: f32) -> Option<f32> {
    METRICS_CTX.with(|ctx| {
        let key = FontKey {
            // These three measure `0`, the x-height and the cap-height — never a Private-Use-Area
            // codepoint — so the PUA family is genuinely absent here, not merely unsupplied.
            family: ctx.resolve_family(families),
            bold,
            italic,
            pua_family: None,
        };
        ctx.x_height(key, size_px)
    })
}

/// The cap-height in px (CSS **`cap`** unit) for the font `families`/`bold`/`italic` resolve to
/// at `size_px`, read off the shared metrics context. `None` leaves Stylo's `cap = ascent`
/// fallback — which, until this fix supplied a real cap-height, was `0` (the provider never set
/// `ascent`), so a `cap`-sized box collapsed to nothing.
pub fn cap_height_px(families: &[String], bold: bool, italic: bool, size_px: f32) -> Option<f32> {
    METRICS_CTX.with(|ctx| {
        let key = FontKey {
            // These three measure `0`, the x-height and the cap-height — never a Private-Use-Area
            // codepoint — so the PUA family is genuinely absent here, not merely unsupplied.
            family: ctx.resolve_family(families),
            bold,
            italic,
            pua_family: None,
        };
        ctx.cap_height(key, size_px)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # G_FONT_SIZE_ZERO — swash reads `size(0.0)` as **font units**, and `font-size: 0` is a reset
    ///
    /// `ShaperBuilder::size` and `ScalerBuilder::size` both default to `0`, documented as
    /// *"equivalent to the units per em of the font"*. So passing a CSS `font-size: 0` straight
    /// through did not ask for a zero-width run — it asked for the run measured in the font's own
    /// **design units**, and swash answered truthfully. One space came back **569px**.
    ///
    /// ⚠⚠⚠ **`font-size: 0` IS NOT AN EDGE CASE — it is the pre-flexbox reset for the whitespace
    /// between `inline-block` children**, the idiom `.grid{font-size:0}` + `.col{display:inline-block;
    /// font-size:16px}`, whose entire purpose is that the inter-column space collapses to nothing.
    /// With a 569px space between every pair of columns, every such grid stacked **one column per
    /// line** — the exact layout the idiom exists to prevent.
    ///
    /// ⚠⚠ **THE LADDER IS WHAT NAMES THE ORGAN, because the symptom looks like a wrap.** Two
    /// `inline-block`s of 40px and 70px in a 230px box:
    ///
    /// ```text
    ///                                     CHROME    ours before   ours after
    ///   font-size:0    40 + sp + 70        1 line     2 lines       1 line
    ///   font-size:0    10 + sp + 10        1 line     2 lines       1 line   <- NOT an overflow
    ///   font-size:0    40 + &nbsp; + 70    1 line     2 lines       1 line   <- NOT a break opp.
    ///   font-size:0    40 + 70, no space   1 line     1 line        1 line   CTRL
    ///   font-size:1px  40 + sp + 70        1 line     1 line        1 line   CTRL — EXACTLY zero
    ///   font-size:16px 40 + sp + 70        1 line     1 line        1 line   CTRL
    /// ```
    ///
    /// The `10 + 10` row rules out an overflow (20px in a 230px box), the `&nbsp;` row rules out a
    /// break-opportunity decision (a non-breaking space must never break), and the `1px` row says
    /// the boundary is **exactly zero** rather than "small". What is left is the advance itself.
    ///
    /// ⚠ **CLAMPED AT `shape_run`, THE ONE FUNNEL**, because `measure` and `shape_bidi` both bottom
    /// out there — fixing it at either call site would leave the other holding font units. The
    /// GREEN mutation is recorded rather than acted on: `rasterize` passes the same `size` to
    /// swash's *scaler*, which shares the convention, but it is only ever reached with glyphs that
    /// `shape_bidi` produced — and at size 0 there are now none. Guarding it too would be dead code
    /// that passes forever, which is the definition of vacuous coverage (STATUS.md, `G_POOL_ISOLATION`).
    ///
    /// To watch it go RED: delete the `size > 0.0` early return in `shape_run`.
    #[test]
    fn a_zero_font_size_measures_zero_and_not_the_fonts_design_units() {
        let fonts = FontContext::new();
        let key = FontKey::default();
        // The subject: at font-size 0 every advance is zero, whatever the string.
        for text in [" ", "\u{a0}", "hello there world", ""] {
            assert_eq!(
                fonts.measure(text, key, 0.0),
                0.0,
                "font-size:0 must measure zero for {text:?}, not the font's design units"
            );
            assert!(
                fonts.shape(text, key, 0.0).glyphs.is_empty(),
                "font-size:0 must shape to no glyphs for {text:?}"
            );
        }
        // CONTROL — the boundary is EXACTLY zero, not "small". A 1px space is a real, positive
        // advance, and a 16px one is larger still. If the guard ever became `size < 1.0` or a
        // clamp-to-minimum, these rows go red.
        let one = fonts.measure(" ", key, 1.0);
        let sixteen = fonts.measure(" ", key, 16.0);
        assert!(
            one > 0.0 && sixteen > one,
            "a positive font-size must still measure positively and scale: 1px={one} 16px={sixteen}"
        );
        // CONTROL — the design-unit answer is what swash gives back when it is asked for it, and it
        // is enormous compared with any plausible px advance. This row is why the bug was invisible
        // as a "rounding" or "metrics" problem: it is off by the em scale, not by a pixel.
        assert!(
            sixteen < 100.0,
            "a 16px space is a px advance, not a design-unit one: {sixteen}"
        );
        // ⚠⚠⚠ THE RASTER HALF, measured rather than assumed. Without its guard, `rasterize` at
        // size 0 returns a **1358x1409 bitmap, 1,913,422 bytes**, and caches it — against 11x12 and
        // 132 bytes at 16px. `docs/wiki/text-layout.md` has named this convention since tick 15 and
        // no guard was ever built for it.
        let run = fonts.shape("A", key, 16.0);
        let g = run.glyphs[0];
        assert!(
            fonts.rasterize(g.glyph_id, g.face, 0.0, 0.0).is_none(),
            "rasterizing at font-size 0 must be refused, not answered with the unscaled outline"
        );
        let at16 = fonts
            .rasterize(g.glyph_id, g.face, 16.0, 0.0)
            .expect("a 16px glyph rasterizes");
        assert!(
            at16.width < 64 && at16.height < 64,
            "a 16px glyph is a 16px-ish bitmap, not a design-unit one: {}x{}",
            at16.width,
            at16.height
        );
    }

    /// **`line-height: normal` rounds the PARTS, not the SUM — and the old rule was fitted at one
    /// font size while its counter-example dropped a term.**
    ///
    /// `(a + d + g).round()` and `a.round() + d.round() + g.round()` agree at 16px for Liberation
    /// Sans (18 either way, because `round(0.523) = 1` restores exactly what the doc's *"14 + 3 =
    /// 17"* argument had left out), and they agree on the other two faces the old doc leaned on —
    /// all three of which were measured at that same 16px. **Varying the FACE does not separate
    /// these rules. Varying the SIZE does.**
    ///
    /// Chrome (`--headless=new --hide-scrollbars`), Liberation Sans, one line, `line-height:normal`:
    ///
    /// ```text
    ///                8   11   16   22   24   26   36   38   40   44   46   56   72   96  128
    ///   Chrome       9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
    ///   before       9   13   18   25   28   30   41   44   46   51   53   64   83  110  147
    ///   after        9   12   18   26   28   31   42   43   45   50   54   65   82  110  147
    /// ```
    ///
    /// A 44-row ladder (36 sizes plus serif and monospace at four sizes each) reads **44/44 after
    /// and 30/44 before**, with every miss ±1 in BOTH directions — a rounding-mode error, not a
    /// wrong constant, which is why no "one shared constant snaps many boxes into tolerance" search
    /// could ever have found it.
    ///
    /// This test asserts the ARITHMETIC rather than a font's numbers, so it does not depend on which
    /// faces the host has installed — the metric values are the ones the ladder above was measured
    /// against.
    ///
    /// RED, run: restore `(ascent + descent + line_gap).round()` in [`LineMetrics::height`].
    #[test]
    fn line_height_normal_rounds_each_metric_and_then_sums() {
        // Liberation Sans's per-em metrics, scaled — the face the Chrome ladder was taken on.
        let (a, d, g) = (14.484f32 / 16.0, 3.391f32 / 16.0, 0.523f32 / 16.0);
        let at = |size: f32| LineMetrics {
            ascent: a * size,
            descent: d * size,
            line_gap: g * size,
        };
        // (size, what Chrome lays out with)
        let chrome = [
            (8.0, 9.0),
            (11.0, 12.0),
            (16.0, 18.0),
            (22.0, 26.0),
            (24.0, 28.0),
            (26.0, 31.0),
            (36.0, 42.0),
            (38.0, 43.0),
            (40.0, 45.0),
            (44.0, 50.0),
            (46.0, 54.0),
            (56.0, 65.0),
            (72.0, 82.0),
            (96.0, 110.0),
            (128.0, 147.0),
        ];
        let mut separated = 0;
        for (size, want) in chrome {
            let lm = at(size);
            assert_eq!(
                lm.height(),
                want,
                "`line-height: normal` at {size}px must be {want} (Chrome);                  a={:.3} d={:.3} g={:.3}",
                lm.ascent,
                lm.descent,
                lm.line_gap
            );
            // How many of these rows can tell the two rounding rules apart at all.
            if (lm.ascent + lm.descent + lm.line_gap).round() != want {
                separated += 1;
            }
        }
        // ⚠ The guard against re-fitting this at one point: if a future edit narrows the ladder to
        //   sizes where both rules agree, the test still passes and proves nothing. 16px, 24px,
        //   96px and 128px are exactly such rows, and they are kept as CONTROLS — but at least ten
        //   rows must actively separate the two rules or this test has stopped testing.
        assert!(
            separated >= 10,
            "only {separated} of these sizes distinguish `round(parts)` from `round(sum)` — the              ladder has lost its discriminating power and must be widened, not trusted"
        );
        // The line GAP is a term in its own right, and dropping it is the exact mistake the old
        // doc's counter-example made: 14 + 3 is 17, and Chrome says 18.
        let lib16 = at(16.0);
        assert_eq!(
            lib16.ascent.round() + lib16.descent.round(),
            17.0,
            "without the gap the 16px row is 17 — which is why the old doc concluded that rounding              the parts was wrong, at the one size where it could not tell"
        );
    }

    #[test]
    fn context_loads_and_measures() {
        let ctx = FontContext::new();
        // These assertions assume the test host has at least one system font,
        // which holds on standard Linux/macOS/Windows dev images.
        if ctx.face_count() == 0 {
            eprintln!("no system fonts; skipping metric assertions");
            return;
        }
        let key = FontKey::default();
        let w_hello = ctx.measure("Hello", key, 16.0);
        let w_hi = ctx.measure("Hi", key, 16.0);
        assert!(w_hello > w_hi, "longer text should be wider");
        let lm = ctx.line_metrics(key, 16.0);
        assert!(lm.ascent > 0.0 && lm.descent >= 0.0);
        let run = ctx.shape("Hi", key, 16.0);
        assert_eq!(run.glyphs.len(), 2);
        assert!(run.width > 0.0);
        // Rasterize the first shaped glyph of "W" (by its resolved face).
        let wrun = ctx.shape("W", key, 32.0);
        let gp = wrun.glyphs[0];
        let g = ctx
            .rasterize(gp.glyph_id, gp.face, 32.0, 0.0)
            .expect("raster W");
        assert!(g.width > 0 && !g.coverage.is_empty());
    }

    #[test]
    fn measure_cache_hits_on_repeat() {
        let ctx = FontContext::new();
        let key = FontKey::default();

        // First measure of each of two distinct runs → two misses.
        let a = ctx.measure("the", key, 16.0);
        let b = ctx.measure("quick", key, 16.0);
        let (h0, m0) = ctx.measure_cache_stats();
        assert_eq!((h0, m0), (0, 2), "two distinct runs are two misses");

        // Re-measuring the same runs → hits, identical results.
        assert_eq!(ctx.measure("the", key, 16.0), a);
        assert_eq!(ctx.measure("quick", key, 16.0), b);
        let (h1, _m1) = ctx.measure_cache_stats();
        assert_eq!(h1, 2, "repeat measures are cache hits");

        // A different size is a distinct key (miss), not a stale hit.
        let _ = ctx.measure("the", key, 24.0);
        let (h2, m2) = ctx.measure_cache_stats();
        assert_eq!(h2, 2, "different size does not falsely hit");
        assert_eq!(m2, 3);
    }

    /// **A named, installed family must resolve to THAT family — `fontdb`'s name query is
    /// case-SENSITIVE.**
    ///
    /// `resolve_family` lowercased the family before querying fontdb, so
    /// `Family::Name("dejavu sans")` returned `None` while `Family::Name("DejaVu Sans")` returns the
    /// face — and **no named system family ever resolved.** Measured against Chromium at t556 on one
    /// 44-character string: `"DejaVu Sans"` 374px, `"Noto Sans"` 348px, a deliberately absent
    /// `"NoSuchFontXYZ"` 299px — three different faces; we rendered all three at **330px**. That single
    /// defect produced both corpus-wide symptoms the t549–t555 sweeps chased: sign-changing text advances
    /// (a substituted face has different per-glyph widths, so the error goes either way depending on the
    /// string) and a **constant** line-box height delta (a substituted face has different
    /// ascent+descent).
    ///
    /// RED PROOF: lowercase the name in the fontdb query and the two installed families collapse onto
    /// the same resolution as the absent one.
    #[test]
    fn a_named_installed_family_resolves_to_that_family_not_a_fallback() {
        let fonts = FontContext::new();
        if fonts.face_count() == 0 {
            eprintln!("no system fonts on this box — skipping (an absent measurement, not a pass)");
            return;
        }
        // Pick two families that are ACTUALLY installed here, by their real (mixed-case) names, so the
        // test measures resolution rather than the font list.
        let installed: Vec<String> = {
            let db = fonts.db.borrow();
            let mut v: Vec<String> = db
                .faces()
                .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
                .filter(|n| n.chars().any(|c| c.is_ascii_uppercase()))
                .collect();
            v.sort();
            v.dedup();
            v
        };
        if installed.len() < 2 {
            eprintln!("fewer than two mixed-case families installed — skipping");
            return;
        }
        let absent = FontFamily::Named(0); // never interned; only used for the shape of the assert below
        let _ = absent;

        for name in installed.iter().take(6) {
            let got = fonts.resolve_family(&[name.clone()]);
            match got {
                FontFamily::Named(id) => {
                    let back = fonts.family_name_of(id).unwrap_or_default();
                    assert!(
                        back.eq_ignore_ascii_case(name),
                        "resolving {name:?} must yield THAT family, got {back:?}"
                    );
                }
                other => panic!(
                    "{name:?} IS installed on this box and must resolve to Named(...), not {other:?} —                      falling back substitutes a face with different advances and a different                      ascent+descent, which is exactly the corpus-wide text divergence measured at t556"
                ),
            }
        }

        // A family that is NOT installed must still fall back (the fix must not make every string a
        // "match"), and it must NOT resolve to the same thing as an installed one.
        let bogus = fonts.resolve_family(&["NoSuchFontXYZ".to_string()]);
        assert!(
            !matches!(bogus, FontFamily::Named(_)),
            "an absent family must fall back, not resolve: {bogus:?}"
        );
    }

    /// **Distinct named families must MEASURE distinctly — the advance has to follow the resolution.**
    ///
    /// t557 made `resolve_family` return a distinct `Named(...)` for each installed family and the
    /// rendered widths did not move by a single pixel: five families, five ids, **one `FaceId(0)` and one
    /// width (330px)**. The case was thrown away one line later by `intern_family`, so `face_id`
    /// re-queried the case-sensitive `fontdb::Family::Name` with a lowercased string, missed, and fell
    /// back to `Family::SansSerif` for all of them. *A fix upstream of a lossy step is not a fix* — and
    /// resolution-level assertions could not see it, which is why this one measures the WIDTH.
    #[test]
    fn distinct_named_families_measure_distinctly() {
        let f = FontContext::new();
        if f.face_count() == 0 {
            eprintln!("no system fonts — skipping (an absent measurement, not a pass)");
            return;
        }
        // Families whose real names differ in case AND that are installed here. Take a few and require
        // that at least two of them measure differently — identical widths across unrelated faces is the
        // signature of the fallback collapse.
        let installed: Vec<String> = {
            let db = f.db.borrow();
            let mut v: Vec<String> = db
                .faces()
                .filter_map(|x| x.families.first().map(|(n, _)| n.clone()))
                .filter(|n| n.chars().any(|c| c.is_ascii_uppercase()))
                .collect();
            v.sort();
            v.dedup();
            v
        };
        if installed.len() < 4 {
            eprintln!("too few mixed-case families installed — skipping");
            return;
        }
        const S: &str = "Announcing Rust 1.90.0 and the metrics probe";
        let mut widths = Vec::new();
        let mut faces = Vec::new();
        for name in installed.iter() {
            let family = f.resolve_family(&[name.clone()]);
            let key = FontKey {
                family,
                bold: false,
                italic: false,
                pua_family: None,
            };
            faces.push(f.primary_face(key));
            widths.push(f.measure(S, key, 16.0));
        }
        let distinct_faces: std::collections::BTreeSet<_> = faces.iter().collect();
        assert!(
            distinct_faces.len() > 1,
            "every one of {} installed families resolved to the SAME face — the advance is not \
             following the resolution, which is the fallback collapse this test exists for",
            installed.len()
        );
        let mut uniq: Vec<u32> = widths.iter().map(|w| w.to_bits()).collect();
        uniq.sort_unstable();
        uniq.dedup();
        assert!(
            uniq.len() > 1,
            "all {} families measured the SAME width ({:.1}px) — a substituted face has different \
             per-glyph advances, so identical widths across unrelated faces means the named family is \
             being ignored somewhere downstream of resolution",
            widths.len(),
            widths[0]
        );
    }

    /// **A DECLARED `@font-face` family whose `src` failed must fall through to the next stack entry —
    /// never to a same-named LOCAL face.**
    ///
    /// CSS Fonts' shadowing rule, and the reason it matters is measured: `martinfowler.com` declares
    /// `font-family: Open Sans, sans-serif` and loads Open Sans from Google Fonts; `Open Sans` also
    /// happens to be installed on this box (13 faces). Once t557/t558 made named families resolve, a
    /// failed webfont load was silently masked by the local face and the page fell **68.2% → 49.2%**
    /// SHAPE against a Chromium that had loaded the real webfont. **A failed download must look failed,
    /// not like a different font.**
    #[test]
    fn a_declared_webfont_family_shadows_the_local_face_of_the_same_name() {
        let f = FontContext::new();
        if f.face_count() == 0 {
            eprintln!("no system fonts — skipping (an absent measurement, not a pass)");
            return;
        }
        // Pick a family that IS installed here, so the test is about shadowing rather than about
        // whether the box happens to have the font.
        let local: Option<String> = {
            let db = f.db.borrow();
            let mut found = None;
            for face in db.faces() {
                if let Some((n, _)) = face.families.first() {
                    if n.chars().any(|c| c.is_ascii_uppercase()) {
                        found = Some(n.clone());
                        break;
                    }
                }
            }
            found
        };
        let Some(local) = local else {
            eprintln!("no mixed-case family installed — skipping");
            return;
        };

        // Without any declaration, the local face is the right answer.
        let before = f.resolve_family(&[local.clone(), "sans-serif".to_string()]);
        assert!(
            matches!(before, FontFamily::Named(_)),
            "{local:?} is installed and undeclared, so it must resolve to the local face: {before:?}"
        );

        // Declare it as an `@font-face` family and load NOTHING for it (the failed-src case).
        f.declare_webfont_family(&local);
        let after = f.resolve_family(&[local.clone(), "sans-serif".to_string()]);
        assert_eq!(
            after,
            FontFamily::SansSerif,
            "once `@font-face` DECLARES {local:?}, the local face of that name is SHADOWED for this \
             document — a failed src must fall through to the NEXT stack entry (sans-serif), not be \
             masked by a same-named local font"
        );

        // And a declaration that DID load must still win (shadowing is not suppression).
        let f2 = FontContext::new();
        f2.declare_webfont_family("MyWebFont");
        // No bytes registered -> falls through; with bytes it would resolve. The fall-through half is
        // what this asserts, since registering real font bytes needs a fixture.
        assert_eq!(
            f2.resolve_family(&["MyWebFont".to_string(), "serif".to_string()]),
            FontFamily::Serif,
            "a declared-but-unloaded family falls through to the next entry, whatever that entry is"
        );
    }

    /// **`system-ui` is a DIFFERENT FONT from `sans-serif`, and the macOS aliases are not generics —
    /// so neither may short-circuit a font stack.**
    ///
    /// Five names shared one arm and all returned `SansSerif`. Every one of them sits at the FRONT of
    /// a real-world stack, so returning a generic there ended the search and the family Chrome
    /// actually picks was never reached. Measured against live Chromium, 16px `"source"` — all four of
    /// these came out `48x18` here:
    ///
    /// | stack | Chrome | was |
    /// |---|---|---|
    /// | `system-ui` | `50.23x22` (Noto Sans) | `48x18` |
    /// | Bootstrap 5 `system-ui,-apple-system,…` | `50.23x22` | `48x18` |
    /// | GitHub `-apple-system,…,"Noto Sans",…` | `50.23x22` (4th entry) | `48x18` |
    /// | Tailwind `ui-sans-serif,system-ui,sans-serif` | `50.23x22` (2nd entry) | `48x18` |
    /// | Bootstrap 4 `-apple-system,…,Roboto,…` | `48.34x19` (**Roboto**) | `48x18` |
    ///
    /// The **line box** is the load-bearing half: 22 vs 18 is 4px on *every line of the page*, and a
    /// line height is a `dy` term, so it charges every line below it too.
    ///
    /// Asserted against the context's OWN resolved UI family rather than a hardcoded "Noto Sans", so
    /// the test states the invariant on any box; it self-skips where a box cannot tell the two apart.
    ///
    /// RED, run: restore the single arm
    /// `"sans-serif" | "system-ui" | "ui-sans-serif" | "-apple-system" | "blinkmacsystemfont" =>
    /// FontFamily::SansSerif` — the first assertion fails (`system-ui` == `sans-serif`), and so does
    /// the Bootstrap-4 one (the stack short-circuits instead of reaching Roboto).
    #[test]
    fn system_ui_is_not_the_sans_generic_and_the_macos_aliases_do_not_short_circuit() {
        let f = FontContext::new();
        let fam = |names: &[&str]| {
            f.resolve_family(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        let Some(ui) = f.system_ui_family.clone() else {
            eprintln!("no UI-font candidate installed — nothing to distinguish; skipping");
            return;
        };
        let sysui = fam(&["system-ui"]);
        let sans = fam(&["sans-serif"]);

        // 1. `system-ui` resolves to the platform UI family by NAME, not to the sans generic.
        assert_eq!(
            sysui,
            fam(&[&ui]),
            "`system-ui` must resolve to the platform UI family ({ui})"
        );
        // Only meaningful where the UI font and the sans generic are actually different faces —
        // which is the whole point, but a minimal container may install only one of them.
        let key = |family| FontKey {
            family,
            bold: false,
            italic: false,
            pua_family: None,
        };
        if f.primary_face(key(sysui)) != f.primary_face(key(sans)) {
            assert_ne!(
                sysui, sans,
                "`system-ui` and `sans-serif` are different families and must not be aliased: \
                 aliasing them is a line-box error on every line of the page"
            );
            // The measured consequence, not just the identity: a different face means a different
            // `line-height: normal` and a different advance. Chrome: 22 vs 18, 50.23 vs 48.03.
            let lh = |fam| f.line_metrics(key(fam), 16.0).height();
            assert_ne!(
                lh(sysui),
                lh(sans),
                "the two faces must produce different `line-height: normal` box heights \
                 (Chrome measured 22 for system-ui vs 18 for sans-serif at 16px)"
            );
        }

        // 2. Tailwind: `ui-sans-serif` is the same UI font, so the stack lands there either way.
        assert_eq!(
            fam(&["ui-sans-serif", "system-ui", "sans-serif"]),
            sysui,
            "the Tailwind stack must resolve to the UI font"
        );
        // 3. Bootstrap 5 / GitHub reach the UI font too — GitHub only by *skipping* three aliases.
        assert_eq!(
            fam(&[
                "system-ui",
                "-apple-system",
                "Segoe UI",
                "Roboto",
                "sans-serif"
            ]),
            sysui,
            "Bootstrap 5's stack starts at system-ui"
        );

        // 4. ⚠ THE ONE THAT PROVES THE SHORT-CIRCUIT IS GONE. `-apple-system` and
        //    `BlinkMacSystemFont` are macOS-only aliases and are not families here, so Bootstrap 4's
        //    stack must walk PAST them to the first installed entry — Chrome lands on Roboto.
        let roboto_installed = fam(&["Roboto"]) != fam(&["NoSuchFontXYZ"]);
        if roboto_installed {
            assert_eq!(
                fam(&[
                    "-apple-system",
                    "BlinkMacSystemFont",
                    "Segoe UI",
                    "Roboto",
                    "Helvetica Neue",
                    "Arial",
                    "sans-serif"
                ]),
                fam(&["Roboto"]),
                "the macOS aliases must NOT short-circuit the stack — Bootstrap 4 lands on Roboto \
                 (Chrome measured 48.34x19, and we measured 48x18 while this bug stood)"
            );
        }
        // …and they must not resolve to the UI font either, which would be the same bug wearing a
        // new name: on THIS platform they name nothing.
        assert_ne!(
            fam(&["-apple-system", "Roboto"]),
            sysui,
            "`-apple-system` names no family on Linux; it must fall through, not become system-ui"
        );
    }

    /// ⚠⚠⚠ **A PRIVATE-USE-AREA CODEPOINT MAY NOT BE DRAWN FROM A GENERIC FAMILY, and every icon
    /// font on the web lives in that block.**
    ///
    /// css-fonts-4 §char-handling-issues: *"If a given character is a Private-Use-Area Unicode
    /// codepoint, user agents must only match font families named in the font-family list that are
    /// not generic families. If none of the families named in the font-family list contain a glyph
    /// for that codepoint, user agents must display some form of missing glyph symbol for that
    /// character rather than attempting installed font fallback."*
    ///
    /// [`FontContext::resolve_family`] answers for the whole element and stops at the FIRST entry
    /// it can honour, so `font-family: serif, Icons` resolved to `Serif` and U+F000 came out as
    /// whatever the system serif keeps at that address — **a different character, silently**, and
    /// the case where the icon font failed to load was indistinguishable from the case where it
    /// worked. Found by `css/css-fonts/matching/font-unicode-PUA.html`, which could only see it
    /// once t1183 installed Ahem on the testharness leg: with no Ahem, BOTH arms of its comparison
    /// fell back to serif and the test passed by cancellation.
    ///
    /// **Self-calibrating — there is no expectation column.** Every row states an EQUALITY between
    /// two measurements taken on this host, so it means the same thing on a box with different
    /// fonts installed. Three of the four rows are CONTROLS.
    ///
    /// **To watch it go RED: delete the `is_private_use` arm from `resolve_face`** — row 1 falls
    /// back to the serif advance and rows 2–4 are unmoved, which is what makes them controls.
    #[test]
    fn a_private_use_codepoint_matches_only_the_non_generic_families_the_author_named() {
        const PUA: &str = "\u{F000}";
        let f = FontContext::new();
        let Some(sfnt) = decode_woff2(include_bytes!("../tests/fixtures/Ahem.woff2")) else {
            panic!("the Ahem fixture must decode — it is the ruler this test measures with");
        };
        f.register_font(sfnt);
        assert!(
            f.has_family("Ahem"),
            "Ahem must register under its own name"
        );

        let names = |v: &[&str]| -> Vec<String> { v.iter().map(|s| s.to_string()).collect() };
        let key = |v: &[&str]| FontKey {
            family: f.resolve_family(&names(v)),
            bold: false,
            italic: false,
            pua_family: f.first_non_generic_family(&names(v)),
        };
        let w = |v: &[&str], text: &str| f.measure(text, key(v), 100.0);

        // Ahem's em box is the ruler: every glyph is exactly 1em wide. If the fixture and the
        // generic happen to agree at this codepoint the test cannot discriminate, and saying so
        // beats a green that means nothing.
        let ahem_pua = w(&["Ahem"], PUA);
        let serif_pua = w(&["serif"], PUA);
        assert_ne!(
            ahem_pua, serif_pua,
            "the probe is blind on this host: Ahem and serif measure U+F000 identically"
        );

        // 1. THE DEFECT. A generic first, the author's real family after it.
        assert_eq!(
            w(&["serif", "Ahem"], PUA),
            ahem_pua,
            "`font-family: serif, Ahem` must draw U+F000 from Ahem — the generic is not eligible \
             for a Private-Use-Area codepoint (css-fonts-4 char-handling-issues)"
        );

        // 2. CONTROL — a NON-PUA character in the same list is untouched: `serif` wins, as it must.
        assert_eq!(
            w(&["serif", "Ahem"], "X"),
            w(&["serif"], "X"),
            "the PUA rule must not touch ordinary text: `serif, Ahem` still measures X in serif"
        );

        // 3. CONTROL — the named family already first. Unchanged, and it is the common icon-font
        //    shape (`font-family: "Font Awesome", sans-serif`), so a regression here would be the
        //    fix breaking the case that already worked.
        assert_eq!(
            w(&["Ahem", "serif"], PUA),
            ahem_pua,
            "`Ahem, serif` already resolved to Ahem and must keep doing so"
        );

        // 4. THE SECOND CLAUSE — no installed-font fallback. With no non-generic family named,
        //    the codepoint gets the primary's missing-glyph symbol, NOT a rummage through every
        //    installed face for something that happens to have a glyph there.
        let serif_primary = f
            .primary_face(key(&["serif"]))
            .expect("serif must resolve to a face");
        assert_eq!(
            f.resolve_face('\u{F000}', serif_primary, None),
            serif_primary,
            "a PUA codepoint with no author-named family must show the missing-glyph symbol, not \
             an installed-font fallback"
        );
        // ...and the control that keeps row 4 honest: an ordinary uncovered character DOES still
        // fall back, so row 4 is measuring the PUA carve-out and not a dead fallback path.
        let cjk = '\u{4E00}';
        if f.fallback_faces().iter().any(|&fb| f.face_covers(fb, cjk)) {
            assert_ne!(
                f.resolve_face(cjk, serif_primary, None),
                serif_primary,
                "non-PUA fallback must still work — otherwise row 4 proves nothing"
            );
        }
    }
}
