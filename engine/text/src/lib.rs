//! manuk-text — font discovery, metrics, shaping, and glyph rasterization.
//!
//! CLAUDE.md's text stack is Parley + swash + fontdb. This first pass uses `fontdb`
//! for system font discovery and `fontdue` (a swash-family rasterizer) for metrics
//! and glyph bitmaps, giving a robust, headless-verifiable Latin text path. Parley's
//! higher-level line breaker + bidi + complex-script shaping is the drop-in upgrade
//! behind the [`FontContext`] API — layout and paint only depend on the shapes and
//! metrics returned here, not on the shaper.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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

/// A rasterized glyph: its fontdue metrics and an 8-bit coverage bitmap
/// (`width * height`, row-major, top-to-bottom).
pub struct GlyphBitmap {
    pub metrics: fontdue::Metrics,
    pub coverage: Vec<u8>,
}

/// Owns the font database and a cache of rasterizer-ready faces.
///
/// Single-threaded by design (uses `Rc`/`RefCell`) — the focused-tab pipeline runs
/// on one thread. A `Send` variant for the compositor's background tiers is a small
/// change (swap to `Arc`/`Mutex`).
pub struct FontContext {
    db: fontdb::Database,
    cache: RefCell<HashMap<FontKey, Option<Rc<fontdue::Font>>>>,
}

impl FontContext {
    /// Build a context populated with the system's installed fonts.
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        FontContext {
            db,
            cache: RefCell::new(HashMap::new()),
        }
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

    fn load(&self, key: FontKey) -> Option<Rc<fontdue::Font>> {
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
        // Fall back to any face if the specific query misses.
        let id = self
            .db
            .query(&query)
            .or_else(|| self.db.faces().next().map(|f| f.id))?;
        let font = self.db.with_face_data(id, |data, index| {
            let settings = fontdue::FontSettings {
                collection_index: index,
                ..fontdue::FontSettings::default()
            };
            fontdue::Font::from_bytes(data, settings).ok()
        })??;
        Some(Rc::new(font))
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

    /// Advance width of a run of text at `size` px.
    pub fn measure(&self, text: &str, key: FontKey, size: f32) -> f32 {
        match self.font(key) {
            Some(font) => text
                .chars()
                .map(|c| font.metrics(c, size).advance_width)
                .sum(),
            // No font: estimate with a monospace-ish average.
            None => text.chars().count() as f32 * size * 0.5,
        }
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

    /// Rasterize a single glyph to an 8-bit coverage bitmap.
    pub fn rasterize(&self, ch: char, key: FontKey, size: f32) -> Option<GlyphBitmap> {
        let font = self.font(key)?;
        let (metrics, coverage) = font.rasterize(ch, size);
        Some(GlyphBitmap { metrics, coverage })
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
        let g = ctx.rasterize('W', key, 32.0).expect("raster W");
        assert!(g.metrics.width > 0 && !g.coverage.is_empty());
    }
}
