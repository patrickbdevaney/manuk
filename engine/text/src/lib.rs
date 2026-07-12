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
    /// Total line box height (ascent + descent + gap).
    pub fn height(&self) -> f32 {
        self.ascent + self.descent + self.line_gap
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
/// Key for the shaped-run/measure cache: `(font, quantized size bits, run text)`.
type RunKey = (FontKey, u32, String);
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
        &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Arial", "Helvetica", "FreeSans"],
    ) {
        db.set_sans_serif_family(f);
    }
    if let Some(f) = pick(
        db,
        "MANUK_FONT_SERIF",
        &["Noto Serif", "DejaVu Serif", "Liberation Serif", "Times New Roman", "FreeSerif"],
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
        // **HELD BEHIND A FLAG, deliberately.** Switching to the system's real faces is correct and
        // measurably closes the width gap (a sans-serif sentence went from 305px to Chrome's 317px)
        // — but it immediately turned the box-parity wall red (72/72 → 69/72) on `valign` and
        // `white-space-nowrap`, which are LINE-HEIGHT and ADVANCE probes. The wall is right: the
        // font selection is fixed, and the metrics computed on top of it are not.
        //
        // `swash`'s advances disagree with Chromium's by ~11% on monospace (6.9px/char where
        // Chromium measures 7.83), and our `normal` line-height is a 1.2× guess rather than the
        // font's own ascent/descent/lineGap. Both are the metrics layer, and METHODOLOGY Part 15
        // says to solve that by ADOPTING Skrifa — the library Chromium itself ships — not by
        // re-deriving advance math. Landing the selection fix on top of a broken metrics layer
        // would trade a measured regression for an unmeasured improvement, which is precisely the
        // trade the wall exists to refuse (METHODOLOGY Part 18: no gate is ever loosened to make a
        // feature land).
        //
        // Enable with MANUK_FONT_SYSTEM=1 to reproduce the measurement; remove the flag with the
        // Skrifa migration.
        if std::env::var("MANUK_FONT_SYSTEM").is_ok() {
            resolve_generic_families(&mut db);
        }
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
            scale_ctx: RefCell::new(swash::scale::ScaleContext::new()),
            shape_ctx: RefCell::new(swash::shape::ShapeContext::new()),
            measure_cache: RefCell::new(LruCache::new(
                NonZeroUsize::new(MEASURE_CACHE_CAP).unwrap(),
            )),
            shape_cache: RefCell::new(LruCache::new(
                NonZeroUsize::new(MEASURE_CACHE_CAP).unwrap(),
            )),
            glyph_cache: RefCell::new(LruCache::new(
                NonZeroUsize::new(GLYPH_CACHE_CAP).unwrap(),
            )),
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
    fn intern_family(&self, name: &str) -> u32 {
        let key = name.to_ascii_lowercase();
        if let Some(&id) = self.family_ids.borrow().get(&key) {
            return id;
        }
        let mut names = self.family_names.borrow_mut();
        let id = names.len() as u32;
        names.push(key.clone());
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
            let n = raw.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
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
                    // A named family: use it only if fontdb actually has a face whose family
                    // name matches (so unknown names fall through to hints / next entry).
                    let q = fontdb::Query {
                        families: &[fontdb::Family::Name(&n)],
                        weight: fontdb::Weight::NORMAL,
                        stretch: fontdb::Stretch::Normal,
                        style: fontdb::Style::Normal,
                    };
                    let db = self.db.borrow();
                    let matched = db.query(&q).is_some_and(|id| {
                        db.face(id).is_some_and(|f| {
                            f.families.iter().any(|(fam, _)| fam.eq_ignore_ascii_case(&n))
                        })
                    });
                    if matched {
                        return FontFamily::Named(self.intern_family(&n));
                    }
                    if n.contains("mono") || n.contains("courier") || n.contains("consol") {
                        return FontFamily::Monospace;
                    }
                    if n.contains("times") || n.contains("georgia") || n.contains("serif")
                        || n.contains("garamond") || n.contains("palatino")
                    {
                        return FontFamily::Serif;
                    }
                }
            }
        }
        FontFamily::SansSerif
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
            if let Some(ids) = self.webfonts.borrow().get(n) {
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
        self.db.borrow()
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
        let ck: RunKey = (key, size.to_bits(), text.to_owned());
        if let Some(&w) = self.measure_cache.borrow_mut().get(&ck) {
            self.hits.set(self.hits.get() + 1);
            return w;
        }
        // Shape each fallback-segmented run and sum advances (no glyph vec is built).
        let mut total = None;
        if let Some(primary) = self.primary_face(key) {
            // Width is order-independent, so measure without bidi reordering.
            let mut pen = 0.0f32;
            for (face, run) in self.segment(text, primary) {
                self.shape_run(&run, face, size, false, |g, _| pen += g.advance);
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
    fn segment(&self, text: &str, primary: FaceId) -> Vec<(FaceId, String)> {
        let mut runs: Vec<(FaceId, String)> = Vec::new();
        for ch in text.chars() {
            let face = self.resolve_face(ch, primary);
            match runs.last_mut() {
                Some((f, s)) if *f == face => s.push(ch),
                _ => runs.push((face, ch.to_string())),
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
        size: f32,
        rtl: bool,
        mut emit: impl FnMut(&swash::shape::cluster::Glyph, f32),
    ) -> f32 {
        let Some(fd) = self.face(face) else { return 0.0 };
        let Some(font) = swash::FontRef::from_index(&fd.data, fd.index as usize) else {
            return 0.0;
        };
        let dir = if rtl {
            swash::shape::Direction::RightToLeft
        } else {
            swash::shape::Direction::LeftToRight
        };
        let mut ctx = self.shape_ctx.borrow_mut();
        let mut shaper = ctx.builder(font).size(size).direction(dir).build();
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
        let ck: RunKey = (key, size.to_bits(), text.to_owned());
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
        // Bidi: reorder the text into visual runs (LTR base), then within each run
        // face-segment and shape with the run's direction. Pure-LTR text yields a single
        // LTR run identical to the non-bidi path.
        let info = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
        for para in &info.paragraphs {
            let (levels, vruns) = info.visual_runs(para, para.range.clone());
            for vr in vruns {
                let rtl = levels[vr.start].is_rtl();
                let sub = &text[vr.clone()];
                for (face, run) in self.segment(sub, primary) {
                    let advance = self.shape_run(&run, face, size, rtl, |g, x| {
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
        let g = ctx.rasterize(gp.glyph_id, gp.face, 32.0, 0.0).expect("raster W");
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
}
