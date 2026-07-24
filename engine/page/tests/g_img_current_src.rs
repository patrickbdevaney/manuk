//! **G_IMG_CURRENT_SRC — `<img>.currentSrc` reports the resolved URL of the image actually loaded.**
//!
//! `currentSrc` is the read-only IDL property that tells a script WHICH image resource an `<img>` is
//! displaying — an absolute URL. Lazy-load, lightbox, gallery and analytics libraries read it on every
//! image (to skip re-fetching one already shown, to build a full-size link, to log what loaded). It was
//! absent: `'currentSrc' in img` was `false`, so those reads returned `undefined` and the library either
//! threw or silently mis-behaved.
//!
//! This engine loads an `<img>`'s `src` attribute directly (it does not yet do srcset/`<picture>`
//! candidate selection for the bitmap — see `Page::pending_image_urls`), so the honest `currentSrc` is
//! precisely the **resolved `src`**: the absolute URL of the resource we actually load. Reporting our own
//! loaded URL is truthful; diverging from Chrome's srcset pick is a *separate* responsive-images gap, not
//! a `currentSrc` defect.
//!
//! Four things to prove:
//! 1. `img.currentSrc` is the **absolute** URL resolved from a relative `src` (not the raw attribute);
//! 2. it is the empty string when the image has no source to select (no `src`);
//! 3. it is **read-only** — assigning to it does not change what it reports;
//! 4. it does not exist as a real value on non-image elements (a `<div>` reads `undefined`).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<img id="im" src="pics/a-320.png">
<img id="none" alt="no source">
<div id="dv"></div>
<div id="out">-</div>
<script>
  var R = [];
  var im = document.getElementById('im');
  R.push('has:' + ('currentSrc' in im));
  R.push('abs:' + (im.currentSrc === 'https://cdn.test/gallery/pics/a-320.png' ? 'OK' : im.currentSrc));
  var none = document.getElementById('none');
  R.push('empty:' + (none.currentSrc === '' ? 'OK' : JSON.stringify(none.currentSrc)));
  // read-only: assignment must be ignored (getter-only accessor)
  try { im.currentSrc = 'https://evil.test/x.png'; } catch (e) {}
  R.push('ro:' + (im.currentSrc === 'https://cdn.test/gallery/pics/a-320.png' ? 'OK' : im.currentSrc));
  var dv = document.getElementById('dv');
  R.push('div:' + (dv.currentSrc === undefined ? 'UNDEF' : JSON.stringify(dv.currentSrc)));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_img_current_src_reports_the_resolved_loaded_url_read_only() {
    let fonts = FontContext::new();
    // Base URL under a subpath so a relative `src` must be RESOLVED, not echoed.
    let page = manuk_page::Page::load(HTML, "https://cdn.test/gallery/index.html", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("has:true", "`currentSrc` must exist on an <img> — it was absent, so libraries read `undefined`"),
        (
            "abs:OK",
            "currentSrc is the ABSOLUTE URL resolved from the relative `src` against the document base, \
             not the raw `pics/a-320.png` attribute",
        ),
        (
            "empty:OK",
            "with no `src` there is no source to select, so currentSrc is the empty string (spec), not a \
             stray value",
        ),
        (
            "ro:OK",
            "currentSrc is read-only — assigning to it is ignored (getter-only accessor), it still \
             reports the loaded URL",
        ),
        (
            "div:UNDEF",
            "a non-image element has no image resource, so its currentSrc reads `undefined` rather than \
             resolving some stray `src`",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_IMG_CURRENT_SRC: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
