//! # G_WEBFONT_UNICODE_RANGE — one hundred `@font-face` rules for one family, and only one is asked for
//!
//! **The inlined Google-Fonts block is the commonest webfont delivery on the web, and it is subsetted
//! by codepoint.** `www.kuechenmomente.de` ships **170 `@font-face` rules, 100 of them named
//! `Raleway`** — weights {400,700} × styles {normal,italic} × ~13 `unicode-range` subsets — with the
//! **Cyrillic and Vietnamese blocks first in source order** and Latin further down.
//!
//! `unicode-range` had **zero occurrences in `engine/`** (t1154), so the loader walked all hundred
//! blocks under one name and fetched every one of them: a page Chrome serves with a single woff2 cost
//! up to a hundred requests, against a render deadline, and the faces that did arrive landed in one
//! family list where `FontContext::face_id` selects on weight and style alone.
//!
//! This gate pins the half that is measurable from inside: **the requests that are never made.** The
//! server counts, and the count is the assertion — three of the four declared subsets cannot serve a
//! single codepoint in the document, and a browser that fetches them is doing work it can prove is
//! useless before it starts.
//!
//! ⚠ **What this gate deliberately does NOT claim.** It does not assert that the skip changes which
//! FACE is selected. On a reduced fixture it does not: `face_id`'s per-glyph fallback already lands on
//! a face that has the glyphs, so the same text measures the same either way, and t1155 severed the
//! skip and measured exactly that. Claiming selection here would be the vacuous half of a real gate.
//! The selection half of `unicode-range` is unbuilt and named as such.
//!
//! ⚠ **And it does not claim to fix the three corpus sites.** `kuechenmomente` / `jatekshop` /
//! `lyreco` still measure their fallback advances (240 / 140 / 184 against Chrome's 166 / 129 / 174),
//! so whatever keeps those faces out of our layout is downstream of this. That is recorded in the
//! journal as an unmoved pre-registered acceptance test, not smuggled into a gate that passes.
//!
//! RED, run: delete the `unicode_range` skip in `engine/page`'s webfont loader — the count goes 1 → 4.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use manuk_text::FontContext;

/// Serves the WPT Ahem face for any request, and **counts the requests**.
fn counting_font_origin() -> (String, Arc<AtomicUsize>) {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../text/tests/fixtures/Ahem.woff2"
    ))
    .expect("Ahem fixture");
    let hits = Arc::new(AtomicUsize::new(0));
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    let counter = Arc::clone(&hits);
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let b = bytes.clone();
            let counter = Arc::clone(&counter);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                counter.fetch_add(1, Ordering::SeqCst);
                let mut h = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: font/woff2\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    b.len()
                )
                .into_bytes();
                h.extend_from_slice(&b);
                let _ = s.write_all(&h);
            });
        }
    });
    (format!("http://{addr}"), hits)
}

#[test]
fn only_the_subset_the_document_can_use_is_fetched() {
    let fonts = FontContext::new();
    let (origin, hits) = counting_font_origin();

    // The Google-Fonts shape, reduced: ONE family, four subsets, the Latin one LAST — because in the
    // real block Cyrillic and Vietnamese come first, and a loader that stops at the first match would
    // pass this test for the wrong reason if Latin led.
    //
    // The document's text is pure ASCII, so exactly one of these four can serve a codepoint in it.
    let html = format!(
        r#"<!doctype html><html><head><style>
             @font-face {{ font-family: 'Subsetted'; src: url({origin}/cyrillic-ext.woff2) format('woff2');
               unicode-range: U+0460-052F, U+1C80-1C8A, U+20B4, U+2DE0-2DFF, U+A640-A69F, U+FE2E-FE2F; }}
             @font-face {{ font-family: 'Subsetted'; src: url({origin}/vietnamese.woff2) format('woff2');
               unicode-range: U+0102-0103, U+0110-0111, U+1EA0-1EF9, U+20AB; }}
             @font-face {{ font-family: 'Subsetted'; src: url({origin}/cyrillic.woff2) format('woff2');
               unicode-range: U+0301, U+0400-045F, U+0490-0491, U+04B0-04B1, U+2116; }}
             @font-face {{ font-family: 'Subsetted'; src: url({origin}/latin.woff2) format('woff2');
               unicode-range: U+0000-00FF, U+0131, U+2000-206F; }}
             #a {{ font-family: 'Subsetted'; font-size: 20px; }}
             span {{ display: inline-block; }}
           </style></head><body>
             <div><span id="a">XXXXX</span></div>
           </body></html>"#
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        800.0,
    ));
    rt.block_on(page.finish_loading(&fonts, 800.0));

    let n = hits.load(Ordering::SeqCst);
    assert_eq!(
        n, 1,
        "G_WEBFONT_UNICODE_RANGE: {n} of the 4 declared subsets were fetched, and only ONE of them \
         (`latin.woff2`) can serve a codepoint in a document whose text is `XXXXX`.\n\n  \
         `unicode-range` is not decoration on the Google-Fonts block — it is the descriptor that says \
         which face. kuechenmomente.de declares ONE HUNDRED subsets under the single name `Raleway`; \
         at {n} requests each that is a page Chrome serves with one woff2 and we serve with a hundred, \
         against a render deadline. The skip lives in `engine/page`'s webfont loader, immediately \
         AFTER `declare_webfont_family` — the family must still be claimed (CSS Fonts' shadowing rule, \
         t561), it is only the FETCH that is skipped."
    );

    // …and the one that WAS fetched must be the usable one, or the count above is satisfied by
    // fetching nothing useful. Ahem's every glyph is exactly 1em, so 5 chars at 20px is exactly 100px
    // and no fallback lands there by accident.
    let dom = page.dom();
    let root = dom.root();
    let rects = page.root_box.node_rects(dom);
    let n = manuk_css::query_selector_all(dom, root, "#a")[0];
    let w = rects.get(&n).map(|r| r.width).unwrap_or(-1.0);
    assert!(
        (w - 100.0).abs() < 0.5,
        "G_WEBFONT_UNICODE_RANGE is HALF-SATISFIED: exactly one subset was fetched, but the text \
         measured {w}px rather than Ahem's exact 100px — so the request that survived was not the one \
         the document needed. A count with no layout assertion beside it can be passed by skipping \
         everything."
    );
}
