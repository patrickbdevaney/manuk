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

/// Which generic font family to resolve. Mapped to concrete faces via `fontdb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
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

/// One placed glyph within a shaped run. `x` is the pen offset from the run origin.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPos {
    pub ch: char,
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
    pub coverage: Vec<u8>,
}

/// Owns the font database and a cache of rasterizer-ready faces.
///
/// Single-threaded by design (uses `Rc`/`RefCell`) — the focused-tab pipeline runs
/// on one thread. A `Send` variant for the compositor's background tiers is a small
/// change (swap to `Arc`/`Mutex`).
/// Key for the shaped-run/measure cache: `(font, quantized size bits, run text)`.
type RunKey = (FontKey, u32, String);
/// Key for the glyph raster cache: `(font, size bits, glyph char, subpixel bucket 0..4)`.
type GlyphKey = (FontKey, u32, char, u8);

/// Owned font-file bytes + face index, so a swash `FontRef` (which borrows the data) can be
/// built on demand for rasterization.
struct FaceData {
    data: Vec<u8>,
    index: u32,
}

/// Number of horizontal subpixel positions a glyph is cached at (quarter-pixel).
const SUBPIXEL_BUCKETS: u8 = 4;

pub struct FontContext {
    db: fontdb::Database,
    cache: RefCell<HashMap<FontKey, Option<Rc<fontdue::Font>>>>,
    /// Owned face bytes per key, for building swash `FontRef`s during rasterization.
    face_data: RefCell<HashMap<FontKey, Option<Rc<FaceData>>>>,
    /// swash's reusable scaling context (glyph rasterization). `RefCell` because scaling
    /// takes `&mut`; single-threaded like the rest of the context.
    scale_ctx: RefCell<swash::scale::ScaleContext>,
    /// Bounded LRU cache of measured run widths (A3 shaped-run cache). Layout measures
    /// the same words repeatedly (per line and in shrink-to-fit's multiple passes), so
    /// caching the advance width skips re-running per-glyph metrics.
    measure_cache: RefCell<LruCache<RunKey, f32>>,
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

impl FontContext {
    /// Build a context populated with the system's installed fonts.
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        FontContext {
            db,
            cache: RefCell::new(HashMap::new()),
            face_data: RefCell::new(HashMap::new()),
            scale_ctx: RefCell::new(swash::scale::ScaleContext::new()),
            measure_cache: RefCell::new(LruCache::new(
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
        self.db.len()
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

    /// Resolve the fontdb face id for `key` (specific query, else any face).
    fn face_id(&self, key: FontKey) -> Option<fontdb::ID> {
        let family = match key.family {
            FontFamily::SansSerif => fontdb::Family::SansSerif,
            FontFamily::Serif => fontdb::Family::Serif,
            FontFamily::Monospace => fontdb::Family::Monospace,
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
            .query(&query)
            .or_else(|| self.db.faces().next().map(|f| f.id))
    }

    fn load(&self, key: FontKey) -> Option<Rc<fontdue::Font>> {
        let id = self.face_id(key)?;
        let font = self.db.with_face_data(id, |data, index| {
            let settings = fontdue::FontSettings {
                collection_index: index,
                ..fontdue::FontSettings::default()
            };
            fontdue::Font::from_bytes(data, settings).ok()
        })??;
        Some(Rc::new(font))
    }

    /// Resolve (and cache) the owned face bytes for `key` — needed to build a swash
    /// `FontRef` for rasterization.
    fn face_bytes(&self, key: FontKey) -> Option<Rc<FaceData>> {
        if let Some(hit) = self.face_data.borrow().get(&key) {
            return hit.clone();
        }
        let loaded = self.face_id(key).and_then(|id| {
            self.db.with_face_data(id, |data, index| {
                Rc::new(FaceData {
                    data: data.to_vec(),
                    index,
                })
            })
        });
        self.face_data.borrow_mut().insert(key, loaded.clone());
        loaded
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
        let w = match self.font(key) {
            Some(font) => text
                .chars()
                .map(|c| font.metrics(c, size).advance_width)
                .sum(),
            // No font: estimate with a monospace-ish average.
            None => text.chars().count() as f32 * size * 0.5,
        };
        self.misses.set(self.misses.get() + 1);
        self.measure_cache.borrow_mut().put(ck, w);
        w
    }

    /// Shape a text run: place each glyph at its accumulated pen position.
    ///
    /// Latin-only, left-to-right, no kerning/ligatures yet (that is Parley's remit).
    pub fn shape(&self, text: &str, key: FontKey, size: f32) -> ShapedRun {
        let metrics = self.line_metrics(key, size);
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen = 0.0f32;
        match self.font(key) {
            Some(font) => {
                for ch in text.chars() {
                    glyphs.push(GlyphPos { ch, x: pen });
                    pen += font.metrics(ch, size).advance_width;
                }
            }
            None => {
                let adv = size * 0.5;
                for ch in text.chars() {
                    glyphs.push(GlyphPos { ch, x: pen });
                    pen += adv;
                }
            }
        }
        ShapedRun {
            glyphs,
            width: pen,
            metrics,
        }
    }

    /// Rasterize a single glyph (via swash) to an 8-bit coverage bitmap, at the horizontal
    /// subpixel offset `subpixel_x` (its fractional part is quantized into
    /// [`SUBPIXEL_BUCKETS`] positions). Cached by `(font, size, glyph, bucket)` so repeated
    /// draws — every frame, every scroll/caret tick — are a lookup, while crisp subpixel
    /// placement is preserved across the quarter-pixel buckets.
    pub fn rasterize(
        &self,
        ch: char,
        key: FontKey,
        size: f32,
        subpixel_x: f32,
    ) -> Option<Rc<GlyphBitmap>> {
        let frac = subpixel_x - subpixel_x.floor();
        let bucket = ((frac * SUBPIXEL_BUCKETS as f32).round() as u8) % SUBPIXEL_BUCKETS;
        let gk = (key, size.to_bits(), ch, bucket);
        if let Some(hit) = self.glyph_cache.borrow_mut().get(&gk) {
            return Some(hit.clone());
        }

        let face = self.face_bytes(key)?;
        let font = swash::FontRef::from_index(&face.data, face.index as usize)?;
        let glyph_id = font.charmap().map(ch);

        let mut ctx = self.scale_ctx.borrow_mut();
        let mut scaler = ctx.builder(font).size(size).hint(true).build();
        let offset = swash::zeno::Vector::new(bucket as f32 / SUBPIXEL_BUCKETS as f32, 0.0);
        let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
            .format(swash::zeno::Format::Alpha)
            .offset(offset)
            .render(&mut scaler, glyph_id)?;

        let bmp = Rc::new(GlyphBitmap {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            coverage: image.data,
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
        let g = ctx.rasterize('W', key, 32.0, 0.0).expect("raster W");
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
