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

/// **THE TWO NUMBERS WPT'S `fuzzy` ANNOTATION IS DEFINED IN** — `(maxDifference, totalPixels)`:
/// the largest per-CHANNEL difference over any pixel that differs at all, and how many pixels
/// differ. Both are what the spec compares against the test's own declared allowance.
///
/// ⚠ Per CHANNEL, not per pixel: WPT's `maxDifference` is `max(|Δr|,|Δg|,|Δb|,|Δa|)`, so a pixel
/// one step off on one channel is a difference of 1, not of 1/255 and not of 3.
pub fn pixel_fuzz(a: &[u8], b: &[u8]) -> (u32, u32) {
    if a.is_empty() || a.len() != b.len() {
        return (255, u32::MAX);
    }
    let (mut maxd, mut count) = (0u32, 0u32);
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if p == q {
            continue;
        }
        count += 1;
        let d = (0..4)
            .map(|i| p[i].abs_diff(q[i]) as u32)
            .max()
            .unwrap_or(0);
        maxd = maxd.max(d);
    }
    (maxd, count)
}

/// **A TEST'S OWN DECLARED PIXEL ALLOWANCE** — `<meta name=fuzzy content="maxDifference=0-2;
/// totalPixels=0-100">`, WPT's mechanism for a reftest whose reference cannot be byte-identical
/// (antialiasing on a rotated edge, a gradient's dithering).
///
/// ⚠⚠⚠ **HONOURING IT IS CONFORMANCE; A BLANKET TOLERANCE WOULD BE LOOSENING THE BAR.** The
/// difference is who chose the number: here it is the test AUTHOR, per test, checked into WPT, and
/// a test with no annotation stays byte-exact. That distinction is the whole reason this is
/// implemented as a parser rather than as a threshold — the board's standing rule is *never loosen
/// the exit to make it move*, and a default fuzz would do exactly that to 6,263 tests at once.
///
/// Both keys are RANGES and both bounds are inclusive: `0-2` permits 0, 1 or 2. A bare number is a
/// range of itself. The optional `<ref-url>:` prefix selects which reference the allowance applies
/// to; we take the unprefixed form and ignore prefixed ones rather than guess, because a wrong
/// association would apply another reference's tolerance to this one.
pub fn parse_fuzzy(html: &str) -> Option<((u32, u32), (u32, u32))> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(i) = lower[from..].find("name=") {
        let at = from + i;
        let rest = &lower[at + 5..];
        let name_ok = rest.starts_with("fuzzy")
            || rest.starts_with("\"fuzzy\"")
            || rest.starts_with("'fuzzy'");
        // The tag this attribute belongs to, bounded so a stray `name=` in text cannot run away.
        let tag_end = lower[at..].find('>').map(|e| at + e).unwrap_or(lower.len());
        if name_ok {
            if let Some(c) = lower[at..tag_end].find("content=") {
                let v = &lower[at + c + 8..tag_end];
                let v = v.trim_start();
                let val = match v.chars().next() {
                    Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                    _ => v.split_whitespace().next().unwrap_or(""),
                };
                if let Some(f) = parse_fuzzy_value(val) {
                    return Some(f);
                }
            }
        }
        from = at + 5;
    }
    None
}

/// The `content` value itself: `maxDifference=0-2;totalPixels=0-100`, either order, whitespace
/// anywhere. Returns `((maxdiff_lo, maxdiff_hi), (pixels_lo, pixels_hi))`.
fn parse_fuzzy_value(v: &str) -> Option<((u32, u32), (u32, u32))> {
    // A `<ref>:` prefix scopes the allowance to one reference; we do not model per-ref allowances,
    // and applying another reference's tolerance to this one would be worse than applying none.
    if v.contains(':') {
        return None;
    }
    let range = |s: &str| -> Option<(u32, u32)> {
        let s = s.trim();
        match s.split_once('-') {
            Some((lo, hi)) => Some((lo.trim().parse().ok()?, hi.trim().parse().ok()?)),
            None => {
                let n: u32 = s.parse().ok()?;
                Some((n, n))
            }
        }
    };
    let (mut md, mut tp) = (None, None);
    for part in v.split(';') {
        let (k, val) = part.split_once('=')?;
        match k.trim() {
            "maxdifference" => md = range(val),
            "totalpixels" => tp = range(val),
            _ => return None,
        }
    }
    // ⚠ One key alone is legal in WPT and means the other is unconstrained. Modelling that as
    // `(0, MAX)` rather than refusing keeps the common single-key annotations working.
    Some((md.unwrap_or((0, u32::MAX)), tp.unwrap_or((0, u32::MAX))))
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
        match run_one(&path, fonts, &rt, wpt_dir) {
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

fn run_one(
    test: &Path,
    fonts: &FontContext,
    rt: &tokio::runtime::Runtime,
    wpt_root: &Path,
) -> RefOutcome {
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
    let Some(ref_path) = resolve_sibling(test, &href, wpt_root) else {
        return RefOutcome::Skip("reference path unresolved".into());
    };
    let Ok(ref_content) = std::fs::read_to_string(&ref_path) else {
        return RefOutcome::Skip("reference unreadable".into());
    };

    let test_px = render(&content, test, fonts, rt);
    let ref_px = render(&ref_content, &ref_path, fonts, rt);
    let equal = test_px == ref_px;
    // ⚠ **FUZZY APPLIES TO A `match` REFERENCE ONLY.** A `mismatch` asserts the two renders are
    // DIFFERENT, and an allowance there would say "different by at least a little", which is not
    // what the annotation means and not a thing WPT defines. Byte-exact inequality stays the test.
    let (maxd, npix) = pixel_fuzz(&test_px, &ref_px);
    let fuzz_ok = kind == RefKind::Match
        && !equal
        && parse_fuzzy(&content).is_some_and(|((mlo, mhi), (plo, phi))| {
            (mlo..=mhi).contains(&maxd) && (plo..=phi).contains(&npix)
        });
    let pass = if kind == RefKind::Mismatch {
        !equal
    } else {
        equal || fuzz_ok
    };
    if pass {
        RefOutcome::Pass
    } else {
        RefOutcome::Fail(format!(
            "{} render {}{}",
            if kind == RefKind::Mismatch {
                "mismatch"
            } else {
                "match"
            },
            if equal { "identical" } else { "differs" },
            // ⚠⚠⚠ **THE NEAR-MISS DATUM, ON EVERY FAILURE, BECAUSE THE STEER'S PREMISE NEEDS A
            // NUMBER.** The observer's t1155 steer says visually-correct pages fail on 1px
            // antialiasing. Only SIX tests in `css/CSS2` carry a `fuzzy` annotation, so honouring it
            // cannot be the answer by itself — but whether byte-exactness is COSTING us is
            // answerable, and it is answerable from the failures we already produce. Printing
            // `(maxdiff, pixels)` on every failing match turns the whole suite into the histogram
            // that decides it: a tail at `maxdiff<=2` is an antialiasing story, a tail at
            // `maxdiff=255` is a layout story, and nobody has looked.
            if kind == RefKind::Match && !equal {
                format!(" [maxdiff {maxd}, {npix}px]")
            } else {
                String::new()
            }
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

/// Resolve a `rel=match` href to a path on disk.
///
/// ⚠⚠⚠ **A SERVER-ROOT-ABSOLUTE HREF IS NOT A SIBLING, AND `Path::join` SILENTLY AGREED.**
/// `dir.join("/css/CSS2/x.xht")` **discards the base entirely** and yields `/css/CSS2/x.xht` at the
/// FILESYSTEM root, which does not exist — so 14 reftests were skipped as *"reference unreadable"*
/// while their reference sat in the checkout. WPT serves its corpus over HTTP, where a leading `/`
/// means the *server* root; on disk that is the WPT checkout root, which is why this needs
/// `wpt_root` and could not be fixed inside a function that only knew the test's directory.
///
/// ⚠⚠ **THE NUMBER IS 14, NOT 254, AND THE DIFFERENCE IS THE WHOLE LESSON OF t1091.** All 254
/// skips share the reason string `reference unreadable`, which reads like one bug; **239 of them
/// are a genuinely absent `wpt/css/reference/`** (this checkout is partial — `wpt/fonts/` is
/// missing too) and only these 14 are ours. A reason string is a property of the READER, and
/// grouping by it groups causes together; the check is one `[ -f ]` per row.
fn resolve_sibling(test: &Path, href: &str, wpt_root: &Path) -> Option<PathBuf> {
    if href.contains("://") {
        return None; // absolute/external reference — out of scope for now
    }
    let dir = test.parent()?;
    // Strip any query/fragment; join relative.
    let clean = href.split(['?', '#']).next().unwrap_or(href);
    if let Some(rooted) = clean.strip_prefix('/') {
        // `..` inside the corpus is fine — `Path::join` handles it lexically and the OS resolves it;
        // what is NOT fine is letting a leading `/` escape to the filesystem root.
        return Some(wpt_root.join(rooted));
    }
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

    /// # G_REFTEST_ROOT_ABSOLUTE_REF — a leading `/` is the SERVER root, not the filesystem root
    ///
    /// WPT serves its corpus over HTTP, so `<link rel="match" href="/css/CSS2/x.xht">` means the
    /// *server* root — on disk, the checkout root. `Path::join` with an absolute argument
    /// **discards the base entirely**, so those references resolved to `/css/CSS2/x.xht` at the
    /// filesystem root, did not exist, and 14 reftests were skipped as *"reference unreadable"*
    /// while their reference sat in the checkout. `css/CSS2` 3,854 → 3,858, and
    /// `reference unreadable` 254 → 240.
    ///
    /// ⚠⚠ **THE RELATIVE CASE IS THE OTHER HALF AND IT MUST NOT CHANGE.** `../../reference/x.xht`
    /// was ACCUSED of this bug at t1091 and is innocent: `Path::join` handles `..` lexically and
    /// the OS resolves it. Those 239 skips are a genuinely absent `wpt/css/reference/` — this
    /// checkout is partial. An implementation that "fixed" both would rewrite a path that was
    /// already right.
    ///
    /// To watch it go RED: delete the `strip_prefix('/')` arm — `css/CSS2/lists` goes
    /// `79 passed / 8 failed / 206 skipped` back to `76 / 6 / 211`.
    #[test]
    fn a_root_absolute_reference_resolves_against_the_wpt_root() {
        let root = Path::new("/wpt");
        let test = Path::new("/wpt/css/CSS2/abspos/t.xht");
        assert_eq!(
            resolve_sibling(test, "/css/CSS2/reference/ref.xht", root),
            Some(PathBuf::from("/wpt/css/CSS2/reference/ref.xht")),
            "a leading `/` is the SERVER root — WPT serves the corpus over HTTP — so it resolves \
             against the checkout, not against `/`. `Path::join` discards the base for an absolute \
             argument, which is how 14 reftests were skipped with their reference on disk."
        );
        assert_eq!(
            resolve_sibling(test, "sib-ref.xht", root),
            Some(PathBuf::from("/wpt/css/CSS2/abspos/sib-ref.xht")),
            "…and the ordinary sibling case is untouched"
        );
        assert_eq!(
            resolve_sibling(test, "../../reference/ref.xht", root),
            Some(PathBuf::from(
                "/wpt/css/CSS2/abspos/../../reference/ref.xht"
            )),
            "…and `..` is left alone: it was ACCUSED of this bug at t1091 and is innocent — \
             `Path::join` handles it lexically and the OS resolves it. Those 239 skips are an \
             absent `wpt/css/reference/`, i.e. a partial checkout, not a path bug."
        );
    }

    /// # G_REFTEST_INSTALLS_AHEM — the suite's RULER, in the suite's own house style
    ///
    /// A CSS 2.1 test states its expected geometry by laying text out in **Ahem**, whose every
    /// glyph is exactly `1em × 1em`, and drawing the same rectangle in the reference with a
    /// `background-color`. This is that pattern in miniature: `font: 100px/1 Ahem` and the letter
    /// **WPT'S `fuzzy` ANNOTATION IS THE TEST AUTHOR'S ALLOWANCE, AND HONOURING IT IS CONFORMANCE.**
    ///
    /// The runner compared byte-exact RGBA, so a reftest whose reference *cannot* be byte-identical
    /// — antialiasing on a rotated edge, a gradient's dithering — was unpassable by construction,
    /// exactly as an unloaded reference PNG was (t1088). WPT's answer is a per-test annotation:
    /// `<meta name=fuzzy content="maxDifference=0-2;totalPixels=0-100">`.
    ///
    /// ⚠⚠⚠ **PRICED BEFORE BUILDING, AND THE PRICE REFUSES THE HEADLINE.** Only **6** files in
    /// `css/CSS2` carry the annotation (282 across all of `css/`, 425 in the whole checkout), so
    /// honouring it is worth at most six tests on the suite this loop runs — it is NOT the
    /// explanation for the layout plateau, and the tick that assumed it was would have been the
    /// fifth green mutation of the window. What the tick DOES buy is recorded beside it: every
    /// failing `match` now prints `[maxdiff N, Mpx]`, which turns the whole 6,263-file suite into
    /// the histogram that decides whether byte-exactness costs anything at all.
    ///
    /// ⚠⚠ **AND THE DISTINCTION THAT KEEPS THIS FROM BEING A LOOSENED BAR:** the number is chosen by
    /// the test AUTHOR, per test, checked into WPT. A test with no annotation stays byte-exact. A
    /// blanket tolerance would move 6,263 tests at once on a number this loop picked for itself,
    /// which is the board's standing *never loosen the exit to make it move*.
    ///
    /// RED, run: drop `|| fuzz_ok` in `run_one` — the annotated near-miss row goes back to FAIL.
    #[test]
    fn a_tests_own_fuzzy_allowance_is_honoured_and_a_bare_test_stays_byte_exact() {
        // The annotation, in the spellings WPT actually ships.
        assert_eq!(
            super::parse_fuzzy(
                r#"<meta name=fuzzy content="maxDifference=0-2;totalPixels=0-100">"#
            ),
            Some(((0, 2), (0, 100))),
            "the canonical form"
        );
        assert_eq!(
            super::parse_fuzzy(
                r#"<meta name="fuzzy" content="totalPixels=0-2;maxDifference=0-1">"#
            ),
            Some(((0, 1), (0, 2))),
            "quoted name, and the two keys in EITHER order — 22 files in css/ use this one"
        );
        assert_eq!(
            super::parse_fuzzy(
                r#"<meta name=fuzzy content="maxDifference=0-1; totalPixels=0-4400">"#
            ),
            Some(((0, 1), (0, 4400))),
            "whitespace after the semicolon — 8 files in css/ use this one"
        );
        assert_eq!(
            super::parse_fuzzy(r#"<meta name=fuzzy content="maxDifference=3">"#),
            Some(((3, 3), (0, u32::MAX))),
            "a bare number is a range of itself, and an absent key is unconstrained"
        );
        // ⚠ The negative rows: a page with no annotation, and a per-reference allowance we decline
        // to guess at rather than apply to the wrong reference.
        assert_eq!(super::parse_fuzzy("<html><body>x</body></html>"), None);
        assert_eq!(
            super::parse_fuzzy(r#"<meta name=fuzzy content="ref.html:maxDifference=0-2">"#),
            None,
            "a <ref-url>: prefix scopes the allowance; applying it blind is worse than not applying it"
        );

        // The measurement itself, per CHANNEL and counting PIXELS.
        let a = vec![10u8, 20, 30, 255, 10, 20, 30, 255];
        let mut b = a.clone();
        b[1] = 22; // one pixel, one channel, off by 2
        assert_eq!(
            super::pixel_fuzz(&a, &b),
            (2, 1),
            "maxdiff is per-channel; one pixel differs"
        );
        assert_eq!(
            super::pixel_fuzz(&a, &a),
            (0, 0),
            "identical buffers have no fuzz"
        );

        // ── AND THE TWO ENDS OF THE ALLOWANCE, so the range is not read as a ceiling only.
        let within = |((mlo, mhi), (plo, phi)): ((u32, u32), (u32, u32)), (m, p): (u32, u32)| {
            (mlo..=mhi).contains(&m) && (plo..=phi).contains(&p)
        };
        let allow = super::parse_fuzzy(
            r#"<meta name=fuzzy content="maxDifference=0-2;totalPixels=0-100">"#,
        )
        .expect("parsed");
        assert!(
            within(allow, (2, 1)),
            "a 2-step difference on 1 pixel is INSIDE 0-2 / 0-100"
        );
        assert!(!within(allow, (3, 1)), "…and 3 steps is outside it");
        assert!(
            !within(allow, (1, 101)),
            "…as is 101 pixels, however small each difference"
        );
    }

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
