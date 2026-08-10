//! Upstream WPT **reftest** runner (P0.3).
//!
//! A reftest is a test file carrying `<link rel="match" href="ref.html">` (or
//! `mismatch`). We render both the test and its reference with the Manuk CPU
//! pipeline and compare pixels: `match` passes iff identical, `mismatch` passes iff
//! different. Deterministic CPU raster makes exact comparison meaningful.
//!
//! Pinned corpus: check out `web-platform-tests/wpt` at the commit named in
//! `IMPLEMENTATION.md` (currently `7f6164e…`, 2026-07-09) so pass counts are
//! meaningful. Tests needing JS/testharness or external resources we don't yet load
//! are **skipped** (honest — not counted as pass).

use std::path::{Path, PathBuf};

use manuk_page::Page;
use manuk_text::FontContext;

use crate::Report;

/// WPT default reference viewport.
const VW: u32 = 800;
const VH: u32 = 600;

/// Screenshot-diff harness (P0.6): fraction (0.0..=1.0) of RGBA pixels that differ
/// between two equal-size buffers. Used for tolerance-based visual comparison —
/// e.g. a real page vs a real Chrome render.
///
/// Note: supplying the Chrome reference needs a headless-Chrome step (in CI or
/// locally); this crate provides the *comparison*, and `render_page_rgba` produces
/// the Manuk side. The Chrome-capture wiring is a follow-up (no Chrome in the dev
/// sandbox).
pub fn pixel_delta(a: &[u8], b: &[u8]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 1.0;
    }
    let total = a.len() / 4;
    let diff = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p != q)
        .count();
    diff as f64 / total.max(1) as f64
}

/// Render a page's RGBA (the Manuk side of a screenshot diff).
pub fn render_page_rgba(html: &str, url: &str, fonts: &FontContext, w: u32, h: u32) -> Vec<u8> {
    Page::load(html, url, fonts, w as f32)
        .paint(fonts, w, h)
        .rgba_bytes()
        .to_vec()
}

/// The WPT Ahem face, WOFF2-compressed — the same 1,624-byte fixture `manuk-page`'s webfont gates
/// serve over HTTP. See [`install_ahem`] for why a reftest run cannot mean anything without it.
const AHEM_WOFF2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../engine/text/tests/fixtures/Ahem.woff2"
));

/// ⚠⚠⚠ **AHEM IS THE SUITE'S RULER, AND IT WAS NOT INSTALLED.**
///
/// A CSS 2.1 test does not compare two renderings of prose — it lays text out in **Ahem**, a face
/// whose every glyph is exactly `1em × 1em` with an `0.8em` ascent and `0.2em` descent, so a line
/// box's geometry becomes an arithmetic fact the reference can draw with a `background-color`
/// block. `linebox/line-height-102.xht` is the house style entire: `font: 20px/1 Ahem`,
/// `width: 1em`, and *"the 2 vertical black stripes have the same height"*.
///
/// Substitute any other face and the test measures that face's metrics instead, so it can only
/// fail. The suite declares the dependency itself — `<meta name="flags" content="ahem">` — and:
///
/// ```text
///   1,406 css/CSS2 files declare  flags: ahem
///   1,090 of those are REFTESTS   = 17.4% of the suite's 6,263
///     786 …whose reference does NOT use Ahem   → unpassable by construction
///     295 …whose reference DOES use Ahem       → both sides wrong TODAY (see below)
/// ```
///
/// ⚠⚠ **THE 295 ARE t1088's `backgrounds` TRAP.** Where the reference uses Ahem too, both sides
/// rendering in the fallback face is a *cancellation* that can read as agreement, and installing
/// the ruler can take such a test from an accidental pass to an honest fail. That is a truer
/// number, not a regression — but it is only visible if the run is read per-directory, which is
/// why the tick that added this banked all twelve.
///
/// ⚠⚠⚠ **AND IT IS NOT A MISSING `@font-face` FETCH.** 1,640 of the suite's 1,707
/// `rel="stylesheet"` links point at `/fonts/ahem.css`, so *"the runner fetches no external
/// stylesheets"* looks like the mechanism — and it is not, because **`wpt/fonts/` is not in this
/// checkout at all**. There is nothing at that URL to fetch. WPT's own runner requirement is that
/// **Ahem be installed on the host**, and [`FontContext::register_font`] is that: the face enters
/// `fontdb` under its own internal family name, exactly as `fc-cache` would have put it there.
///
/// Scoped to the reftest runner on purpose. A test font must never reach a real page, so this is
/// **not** in `FontContext::new`.
fn install_ahem(fonts: &FontContext) {
    match manuk_text::decode_woff2(AHEM_WOFF2) {
        Some(sfnt) => fonts.register_font(sfnt),
        // Loud, because every ahem-flagged test would otherwise fail for a reason the report cannot
        // name — which is the state this function exists to end.
        None => eprintln!(
            "⚠ Ahem.woff2 did not decode — every `flags: ahem` test (17.4% of css/CSS2) will \
             measure the fallback face and fail"
        ),
    }
}

/// Run every reftest under `wpt_dir/subdir`, returning a [`Report`].
pub fn run_reftests(wpt_dir: &Path, subdir: &str, fonts: &FontContext) -> Report {
    install_ahem(fonts);
    let mut report = Report::default();
    let root = wpt_dir.join(subdir);
    let mut files = Vec::new();
    collect_html(&root, &mut files);
    files.sort();

    // ONE runtime for the whole run — `render` needs an async context to fetch the `<img>`
    // resources a reference is built from (see there), and building a multi-thread runtime per
    // file would cost more than the raster.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for reftest subresource loading");

    for path in files {
        let name = path
            .strip_prefix(wpt_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        match run_one(&path, fonts, &rt) {
            RefOutcome::Pass => report.push(&name, Ok(())),
            RefOutcome::Fail(why) => report.push(&name, Err::<(), String>(why)),
            RefOutcome::Skip(why) => report.skip(&name, &why),
        }
    }
    report
}

enum RefOutcome {
    Pass,
    Fail(String),
    Skip(String),
}

fn run_one(test: &Path, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> RefOutcome {
    let Ok(content) = std::fs::read_to_string(test) else {
        return RefOutcome::Skip("unreadable".into());
    };
    // testharness.js / scripted tests need a JS runtime we don't wire by default.
    if content.contains("testharness") || content.contains("<script") {
        return RefOutcome::Skip("needs JS/testharness".into());
    }
    let Some((kind, href)) = find_ref_link(&content) else {
        return RefOutcome::Skip("not a reftest (no rel=match/mismatch)".into());
    };
    let Some(ref_path) = resolve_sibling(test, &href) else {
        return RefOutcome::Skip("reference path unresolved".into());
    };
    let Ok(ref_content) = std::fs::read_to_string(&ref_path) else {
        return RefOutcome::Skip("reference unreadable".into());
    };

    let test_px = render(&content, test, fonts, rt);
    let ref_px = render(&ref_content, &ref_path, fonts, rt);
    let equal = test_px == ref_px;
    let pass = if kind == RefKind::Mismatch {
        !equal
    } else {
        equal
    };
    if pass {
        RefOutcome::Pass
    } else {
        RefOutcome::Fail(format!(
            "{} render {}",
            if kind == RefKind::Mismatch {
                "mismatch"
            } else {
                "match"
            },
            if equal { "identical" } else { "differs" }
        ))
    }
}

/// ⚠⚠⚠ **A REFERENCE IS A DOCUMENT, AND 1,230 OF THEM ARE BUILT OUT OF `<img>`.** This used to be
/// `Page::load(…).paint(…)` — the SYNC path, which parses and lays out but fetches **no
/// subresources**. The CSS 2.1 suite's house style is to draw the expected result with coloured
/// swatch PNGs (`support/blue15x15.png`, `support/swatch-orange.png`), so those references painted
/// two blank boxes while the test painted the real thing, and the comparison could only ever say
/// *"render differs"*. Measured on `css/CSS2/positioning/right-004.xht`, the pixel row where the
/// borders belong:
///
/// ```text
///   reference, sync path    …white white white white white…       <- the swatches never loaded
///   reference, with images  …blue blue blue orange orange…
///   the TEST, either way    …blue blue blue orange orange…        <- the engine was always right
/// ```
///
/// **All 50 RTL `right-*` tests failed, and so did every other one in the family** — a 100% failure
/// rate that reads exactly like a broken engine primitive and was an unloaded PNG. The scale:
/// **1,230 of `css/CSS2`'s 6,263 reftests (19.6%) have a reference containing `<img>`**, spread
/// across `normal-flow` 276, `backgrounds` 236, `positioning` 220, `borders` 130, `linebox` 111,
/// `floats-clear` 90 and `bidi-text` 48. Every one of them was unpassable by construction.
///
/// ⚠ **ONE phase is added and no more.** `Page::load` stays: no JS (scripted tests are skipped
/// above), and **no external stylesheets** — that is a second, separate absence with its own
/// number, and bundling it here would make this measurement unattributable.
///
/// ⚠ The `timeout` bounds a test that points at the network. Local `file://` reads complete in
/// microseconds, so it never fires on the corpus; it exists so one stray absolute URL cannot hang a
/// 6,000-file run, and a fetch it cancels leaves the page exactly as the old sync path rendered it.
fn render(html: &str, path: &Path, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> Vec<u8> {
    let url = format!("file://{}", path.display());
    let mut page = Page::load(html, &url, fonts, VW as f32);
    rt.block_on(async {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            page.fetch_and_apply_images(fonts, VW as f32).await;
            page.fetch_and_apply_background_images().await;
        })
        .await;
    });
    let canvas = page.paint(fonts, VW, VH);
    canvas.rgba_bytes().to_vec()
}

#[derive(PartialEq)]
enum RefKind {
    Match,
    Mismatch,
}

/// Find the first `<link rel="match|mismatch" href="…">` via a lightweight scan.
fn find_ref_link(html: &str) -> Option<(RefKind, String)> {
    let dom = manuk_html::parse(html);
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) != Some("link") {
            continue;
        }
        let el = dom.element(n)?;
        let rel = el.attr("rel").unwrap_or("").to_ascii_lowercase();
        let kind = if rel == "match" {
            RefKind::Match
        } else if rel == "mismatch" {
            RefKind::Mismatch
        } else {
            continue;
        };
        if let Some(href) = el.attr("href") {
            return Some((kind, href.to_string()));
        }
    }
    None
}

fn resolve_sibling(test: &Path, href: &str) -> Option<PathBuf> {
    if href.contains("://") {
        return None; // absolute/external reference — out of scope for now
    }
    let dir = test.parent()?;
    // Strip any query/fragment; join relative.
    let clean = href.split(['?', '#']).next().unwrap_or(href);
    Some(dir.join(clean))
}

fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_html(&p, out);
        } else if matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("html" | "xht" | "xhtml" | "htm")
        ) {
            // Skip obvious reference files (named *-ref.*) as *tests* — they are
            // targets, discovered via their test's match link.
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !stem.ends_with("-ref") && !stem.ends_with("-notref") {
                out.push(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_delta_and_render() {
        let fonts = FontContext::new();
        let a = render_page_rgba("<body><h1>A</h1></body>", "x", &fonts, 200, 100);
        let a2 = render_page_rgba("<body><h1>A</h1></body>", "x", &fonts, 200, 100);
        let b = render_page_rgba("<body><h1>B different</h1></body>", "x", &fonts, 200, 100);
        assert_eq!(pixel_delta(&a, &a2), 0.0, "identical renders → 0 delta");
        assert!(
            pixel_delta(&a, &b) > 0.0,
            "different renders → nonzero delta"
        );
        assert_eq!(pixel_delta(&a, &[]), 1.0, "size mismatch → full delta");
    }

    /// # G_REFTEST_LOADS_BITMAPS — a reference is a DOCUMENT, and 1,230 of them are drawn with PNGs
    ///
    /// The CSS 2.1 suite's house style is to draw the expected result out of coloured swatch images
    /// (`support/blue15x15.png`, `support/swatch-orange.png`) — as an `<img>` in the reference and,
    /// in the `backgrounds` chapter, as a `background-image` in the test. [`render`] used the SYNC
    /// `Page::load`, which fetches no subresources, so those documents painted blank boxes and the
    /// comparison could only ever say *"render differs"*. **1,230 of `css/CSS2`'s 6,263 reftests
    /// (19.6%) have a reference containing `<img>`** and every one was unpassable by construction.
    ///
    /// ⚠⚠⚠ **BOTH KINDS OR NEITHER.** Loading `<img>` alone is not half a fix, it is a DIFFERENT
    /// bias: `backgrounds` went **184 → 123** on that intermediate build, because its tests draw
    /// with `background-image` while its references draw with `<img>`, and both being blank was a
    /// *cancellation* that read as agreement. Loading both took it to **220**. `positioning` shows
    /// the same effect mirrored (339 on the `<img>`-only build, 314 with both, against 187 before).
    /// This test therefore asserts **one pixel of each kind**, and either half alone fails it.
    ///
    /// To watch it go RED: delete either `fetch_and_apply_images` or
    /// `fetch_and_apply_background_images` from [`render`] — the corresponding pixel reads white.
    #[test]
    fn a_reftest_render_loads_the_bitmaps_its_document_references() {
        // A 1×1 blue PNG, written to disk so this exercises the real `file://` fetch the corpus
        // needs — a `data:` URI would pass without the fetch path working at all.
        const BLUE_PNG: [u8; 69] = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0x60, 0x60, 0xf8, 0x0f, 0x00, 0x01, 0x03, 0x01, 0x00, 0x08, 0x89, 0xc2,
            0xec, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let dir = std::env::temp_dir().join("manuk-reftest-bitmaps");
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("blue.png"), BLUE_PNG).expect("write png");
        let doc = dir.join("doc.html");
        std::fs::write(
            &doc,
            r#"<!doctype html><html><body style="margin:0">
<div style="width:20px;height:20px;background-image:url(blue.png)"></div>
<img src="blue.png" width="20" height="20" style="display:block">
</body></html>"#,
        )
        .expect("write doc");

        let fonts = FontContext::new();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let html = std::fs::read_to_string(&doc).expect("read doc");
        let px = render(&html, &doc, &fonts, &rt);
        let at = |x: u32, y: u32| -> (u8, u8, u8) {
            let i = ((y * VW + x) * 4) as usize;
            (px[i], px[i + 1], px[i + 2])
        };
        assert_eq!(
            at(10, 10),
            (0, 0, 255),
            "a `background-image: url(...)` must be fetched and painted — this is the half whose \
             absence took `css/CSS2/backgrounds` DOWN 61 tests when only `<img>` was loaded"
        );
        assert_eq!(
            at(10, 30),
            (0, 0, 255),
            "…and an `<img>`, which is how 1,230 CSS 2.1 references draw their expected result"
        );
    }

    /// # G_REFTEST_INSTALLS_AHEM — the suite's RULER, in the suite's own house style
    ///
    /// A CSS 2.1 test states its expected geometry by laying text out in **Ahem**, whose every
    /// glyph is exactly `1em × 1em`, and drawing the same rectangle in the reference with a
    /// `background-color`. This is that pattern in miniature: `font: 100px/1 Ahem` and the letter
    /// `X` must paint **a solid 100×100 square**, byte-identical to a reference `<div>` of that
    /// size and colour. No other face can land there by accident — measured on the fallback, the
    /// same document inks about 3% of that box.
    ///
    /// It runs through [`run_reftests`] rather than [`render`] on purpose: the defect being gated
    /// is a **missing call**, so the gate has to exercise the call site. **To watch it go RED:
    /// delete `install_ahem(fonts)` from [`run_reftests`]** — the `X` renders in the fallback face
    /// and the mini-suite reports `0 passed, 1 failed`.
    ///
    /// **1,090 of `css/CSS2`'s 6,263 reftests (17.4%) declare `flags: ahem`**, and `fc-list` has no
    /// Ahem on this host.
    #[test]
    fn a_reftest_run_installs_the_ahem_face_the_suite_measures_with() {
        let root = std::env::temp_dir().join("manuk-reftest-ahem");
        let suite = root.join("mini");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&suite).expect("temp suite dir");
        std::fs::write(
            suite.join("ahem-em-box.html"),
            r#"<!doctype html><html><head><meta name="flags" content="ahem">
<link rel="match" href="ahem-em-box-ref.html"></head>
<body style="margin:0"><div style="font:100px/1 Ahem;color:rgb(0,0,255)">X</div></body></html>"#,
        )
        .expect("write test");
        std::fs::write(
            suite.join("ahem-em-box-ref.html"),
            r#"<!doctype html><html><body style="margin:0">
<div style="width:100px;height:100px;background:rgb(0,0,255)"></div></body></html>"#,
        )
        .expect("write reference");

        let fonts = FontContext::new();
        let report = run_reftests(&root, "mini", &fonts);
        assert!(
            report.all_passed(),
            "a `font: 100px/1 Ahem` X must paint the solid 100x100 em box its reference draws with \
             `background-color` — that equivalence IS the CSS 2.1 house style, and it is how 1,090 \
             reftests (17.4% of css/CSS2) state their expected geometry. Without the face installed \
             the fallback glyph inks ~3% of the box.\n{}",
            report.summary()
        );
    }
}
