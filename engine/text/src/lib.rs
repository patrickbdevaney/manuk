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
}

impl Default for FontKey {
    fn default() -> Self {
        FontKey {
            family: FontFamily::SansSerif,
            bold: false,
            italic: false,
        }
    }
}

/// Vertical metrics of a font at a given size, in px. `descent` is a positive
/// magnitude below the baseline.
#[derive(Clone, Copy, Debug, Default)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
}

impl LineMetrics {
    /// Total line box height for `line-height: normal` — **`ascent + descent + gap`, ROUNDED TO A
    /// WHOLE PIXEL**, because that is the number Chrome lays out with.
    ///
    /// A line box is not a place to be more precise than the engine you are being compared
    /// against. Keeping the sum fractional made every line ~0.4px taller than Chrome's: invisible
    /// on one line, and **cumulative down the document**, which is exactly the FID-SWEEP near-miss
    /// signature (`mdx=0` with `mdy` growing with content density — wikipedia 45px over ~110 line
    /// boxes).
    ///
    /// Verified against real Chrome on three faces. Three is the point: one face cannot distinguish
    /// this rule from rounding the parts separately.
    ///
    /// ```text
    ///                   ascent  descent    gap     sum    → us   Chrome
    /// Liberation Sans   14.484    3.391  0.523  18.398      18     18
    /// DejaVu Sans       14.854    3.773  0      18.627      19     19
    /// Noto Sans         17.104    4.688  0      21.792      22     22
    /// ```
    ///
    /// **Round the SUM, not the parts.** Rounding each term first gives 14+3=17 for Liberation
    /// where Chrome says 18 — a rule that looks equally plausible written down and is wrong on the
    /// very first face. That mistake was made here and caught only by re-measuring after the edit.
    ///
    /// Advance WIDTHS are never rounded: Chrome positions glyphs subpixel horizontally, and the
    /// sweep already measures our horizontal placement as exact (`mdx=0`).
    pub fn height(&self) -> f32 {
        (self.ascent + self.descent + self.line_gap).round()
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
    /// Discovered fallback faces (lazy); per-`(face, char)` coverage memo.
    fallbacks: RefCell<Option<Vec<FaceId>>>,
    coverage: RefCell<HashMap<(FaceId, char), bool>>,
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
        FontContext {
            db: RefCell::new(db),
            cache: RefCell::new(HashMap::new()),
            faces: RefCell::new(Vec::new()),
            face_by_dbid: RefCell::new(HashMap::new()),
            primary_of: RefCell::new(HashMap::new()),
            fallbacks: RefCell::new(None),
            coverage: RefCell::new(HashMap::new()),
            family_names: RefCell::new(Vec::new()),
            family_ids: RefCell::new(HashMap::new()),
            webfonts: RefCell::new(HashMap::new()),
            declared_webfonts: RefCell::new(std::collections::HashSet::new()),
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
                "sans-serif" | "system-ui" | "ui-sans-serif" | "-apple-system"
                | "blinkmacsystemfont" => return FontFamily::SansSerif,
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
    fn resolve_face(&self, ch: char, primary: FaceId) -> FaceId {
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
            for (face, script, run) in self.segment(text, primary) {
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
    fn segment(&self, text: &str, primary: FaceId) -> Vec<(FaceId, Script, String)> {
        use swash::text::Codepoint;
        let mut runs: Vec<(FaceId, Script, String)> = Vec::new();
        for ch in text.chars() {
            let face = self.resolve_face(ch, primary);
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
                for (face, script, run) in self.segment(sub, primary) {
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
            family: ctx.resolve_family(families),
            bold,
            italic,
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
            family: ctx.resolve_family(families),
            bold,
            italic,
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
            family: ctx.resolve_family(families),
            bold,
            italic,
        };
        ctx.cap_height(key, size_px)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
