//! **G_IMPORT_INLINE_STYLE — an `@import` inside an inline `<style>` was never fetched, and it is the
//! spelling Google Fonts' own "@import" tab tells you to paste.**
//!
//! t564 wired the `@import` walk over the EXTERNAL sheets — the ones a `<link rel=stylesheet>`
//! brought in — and stopped there. An import declared in a `<style>` block dropped the whole
//! imported sheet: its rules, its `@font-face`s, everything. The engine even said so on every load
//! and nothing read it — stylo's own parser logs
//! `Saw @import rule, but no way to trigger the load` once per occurrence.
//!
//! **The A/B is one line of markup apart**, `font-family:'M PLUS 1p'` at 14px against real Chrome,
//! and the external row is the CONTROL that proves the walk itself already worked:
//!
//! ```text
//!   where the @import lives          Chrome            before
//!   in an EXTERNAL sheet (<link>)    119.313 x 20      119 x 20   ✓ (t564)
//!   in an INLINE <style>             119.313 x 20      111 x 16   ✗
//! ```
//!
//! **PRICED BEFORE BUILDING**, on 73 CrUX corpus pages actually fetched: **2 carry one (2.7%)**,
//! both Google Fonts, and one of them (`pasarbokep.com`) is in the near-bar band the burndown ranks.
//! A small share of pages, and a TOTAL loss of a stylesheet on the pages that have it.
//!
//! ⚠ **AHEM IS THE FONT BECAUSE ITS METRICS ARE A PROOF, NOT A COMPARISON.** Every Ahem glyph is a
//! filled square of exactly 1em advance, so 5 characters at 20px is exactly 100px and nothing else
//! can land there. The imported sheet carries BOTH a `@font-face` and a plain rule, so this gate
//! fails if either half of an imported sheet goes missing — an import that delivered only fonts
//! would be a narrower claim than the bug.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// One origin serving three things: the imported stylesheet, the Ahem face it names, and (for the
/// control arm) an outer stylesheet that itself `@import`s the same sheet.
fn origin() -> String {
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../text/tests/fixtures/Ahem.woff2"
    ))
    .expect("Ahem fixture");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    let base = format!("http://{addr}");
    // Reached ONLY through the inline `<style>`'s `@import` — a DISTINCT url from the one the
    // external sheet imports, so removing the inline scan cannot be masked by the external route.
    let inline_imported = format!(
        "@font-face {{ font-family: 'AhemImported'; src: url({base}/ahem.woff2) format('woff2'); }}\n\
         #a {{ font-family: 'AhemImported'; font-size: 20px; }}\n\
         #r {{ width: 137px; }}\n\
         #p {{ width: 61px; }}\n"
    );
    // Reached ONLY through the external sheet's `@import` — the t564 path.
    let imported = "#o2 { width: 53px; }\n".to_string();
    let outer = format!("@import url({base}/imported.css);\n#o {{ width: 91px; }}\n");
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let (font, imported, inline_imported, outer) = (
                font.clone(),
                imported.clone(),
                inline_imported.clone(),
                outer.clone(),
            );
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let (ctype, body): (&str, Vec<u8>) = if req.contains("/ahem.woff2") {
                    ("font/woff2", font)
                } else if req.contains("/inline-imported.css") {
                    ("text/css", inline_imported.into_bytes())
                } else if req.contains("/imported.css") {
                    ("text/css", imported.into_bytes())
                } else {
                    ("text/css", outer.into_bytes())
                };
                let mut h = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                h.extend_from_slice(&body);
                let _ = s.write_all(&h);
            });
        }
    });
    base
}

#[test]
fn an_at_import_inside_an_inline_style_block_is_fetched_and_applied() {
    let fonts = FontContext::new();
    let base = origin();

    // `#a` is styled ONLY by the imported sheet, reached through an INLINE `<style>`.
    // `#o` is the CONTROL for the path that already worked at t564: the same import, one level of
    // indirection out, through a `<link rel=stylesheet>`.
    // `#b` is the vacuity guard — the same string in the fallback face.
    // `#r` proves an imported sheet's ORDINARY RULES arrive too, not just its `@font-face`.
    let html = format!(
        r#"<!doctype html><html><head>
             <link rel="stylesheet" href="{base}/outer.css">
             <style>@import url({base}/inline-imported.css);</style>
             <style>span {{ display: inline-block; }} #b {{ font-size: 20px; font-family: serif; }} #p {{ width: 29px; }}</style>
           </head><body>
             <div><span id="a">XXXXX</span></div>
             <div><span id="b">XXXXX</span></div>
             <div><span id="r">x</span></div>
             <div><span id="o">x</span></div>
             <div><span id="o2">x</span></div>
             <div><span id="p">x</span></div>
           </body></html>"#
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{base}/index.html"),
        &fonts,
        800.0,
    ));
    rt.block_on(page.finish_loading(&fonts, 800.0));

    let dom = page.dom();
    let root = dom.root();
    let rects = page.root_box.node_rects(dom);
    let width = |sel: &str| -> f32 {
        let n = manuk_css::query_selector_all(dom, root, sel)[0];
        rects.get(&n).map(|r| r.width).unwrap_or(-1.0)
    };

    let imported_font = width("#a");
    let fallback = width("#b");
    let imported_rule = width("#r");

    // ── 1. THE `@font-face` HALF. Ahem's advance is exactly 1em, so 5 chars at 20px is exactly 100.
    assert!(
        (imported_font - 100.0).abs() < 0.5,
        "G_IMPORT_INLINE_STYLE: text in a family declared by a sheet `@import`ed from an INLINE \
         <style> measured {imported_font}px, not 100px.\n\n  Ahem's every glyph is exactly 1em wide, \
         so 5 characters at font-size:20px is exactly 100px and no fallback face lands there by \
         accident. A miss here means the inline sheet's import was never fetched — the walk in \
         `fetch_and_apply_stylesheets` iterated only the EXTERNAL sheets, and stylo logged \
         `Saw @import rule, but no way to trigger the load` while the page rendered in the fallback."
    );

    // ── 2. THE ORDINARY-RULES HALF. An imported sheet is a stylesheet, not a font delivery
    // mechanism: a fix that only harvested `@font-face` out of it would pass arm 1 and fail here.
    assert!(
        (imported_rule - 137.0).abs() < 0.5,
        "an imported sheet's ORDINARY rules must apply too — `#r {{ width: 137px }}` came out \
         {imported_rule}px. 137 is chosen to be nothing the layout could produce on its own."
    );

    // ── 3. THE EXTERNAL-IMPORT PATH DELIVERS ITS RULES TOO. t564 fetched these sheets and its own
    // comment said they "must reach the CASCADE" — they did not, because `apply_stylesheets`
    // re-derives its sources FROM THE DOM and an imported sheet has no `<link>` node. Only the
    // `@font-face` scan saw them, which is exactly why the defect survived: an import that carried
    // fonts LOOKED fixed.
    let external_import_rule = width("#o2");
    assert!(
        (external_import_rule - 53.0).abs() < 0.5,
        "a sheet `@import`ed from an EXTERNAL stylesheet must have its ordinary rules cascaded — \
         `#o2 {{ width: 53px }}` came out {external_import_rule}px. This is the t564 path, and it \
         has been delivering @font-face and nothing else."
    );

    // ── 4. THE PLAIN `<link>` CONTROL. The sheet the DOM itself names must be unaffected.
    let plain_link = width("#o");
    assert!(
        (plain_link - 91.0).abs() < 0.5,
        "CONTROL: an ordinary `<link rel=stylesheet>` rule must still apply — `#o {{ width: 91px }}` \
         came out {plain_link}px"
    );

    // ── 5. THE CASCADE ORDER, and it is what makes "prepend" a claim rather than a convenience.
    // An `@import` must precede every rule in its own sheet, so the imported rules lose ties to the
    // document's own. Chrome-measured on the same shape (`#p{width:61px}` imported, `#p{width:29px}`
    // in a later inline `<style>`, equal specificity): **29**. Appending the imported sheets instead
    // gives 61 and passes every other arm here.
    let ordered = width("#p");
    assert!(
        (ordered - 29.0).abs() < 0.5,
        "an imported sheet's rules must lose a tie to the document's own sheets (Chrome: 29, the \
         later inline rule) — got {ordered}px. 61 means the imported sheets were APPENDED and now \
         override the page's own CSS."
    );

    // ── 6. THE VACUITY GUARD. If the fallback also measured 100 the first arm would prove nothing.
    assert!(
        (fallback - 100.0).abs() > 1.0,
        "G_IMPORT_INLINE_STYLE is VACUOUS: the serif fallback also measured {fallback}px, so \
         `== 100` cannot distinguish the imported face from the default one."
    );
}
