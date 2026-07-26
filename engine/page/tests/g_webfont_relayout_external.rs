//! **G_WEBFONT_RELAYOUT (external-stylesheet half) — the same claim on the path where the `@font-face`
//! arrives in an EXTERNAL sheet.**
//!
//! Split into its own file because a `PageContext` is per-PROCESS (two `#[test]`s in one binary
//! SIGSEGV). It is a separate CASE, not a duplicate: with an external sheet `count > 0`, so control
//! reaches `apply_stylesheets` — which correctly answers `RestyleDamage::None`, because a new FACE
//! changes metrics and not the cascade's inputs. The early return then had to learn about the font
//! too, or this path keeps the fallback layout for the same reason the inline path did.
//!
//! See `g_webfont_relayout.rs` for the full account.
//!
//! Surface audit #34 (t618) found that `@font-face` had **no gate anywhere in the repo** while the map
//! claimed it `works`. It does not, on the path most real sites take.
//!
//! Every layer in isolation was correct. `manuk-text` decodes the WOFF2, registers the face and
//! measures Ahem at **exactly 100.0px** for 5 characters at 20px. What was missing sat one level up:
//!
//! ```rust
//! if count > 0 || self.dom.has_dirty() {   // external CSS arrived, or a script mutated the tree
//! ```
//!
//! That guard exists for a real reason — an unconditional re-cascade costs a full extra pass, 257ms a
//! go on bbc.co.uk — but it lists **two** reasons to re-lay-out and there are **three**. A font that
//! has just arrived changes the advance of every glyph in the document, so every line box, wrap point
//! and content height computed with the fallback face is stale. **An optimisation guard is only as
//! correct as its list of inputs.**
//!
//! **Why the inline case is the one that matters:** `@font-face` in an inline `<style>` means
//! `count == 0` and a clean tree, so the whole thing took the `else` branch and kept the fallback
//! layout. That is exactly what every "inline your critical CSS" build produces, which is to say most
//! performance-tuned sites — and it is invisible to any test whose stylesheet is external.
//!
//! **Ahem is the font because its metrics are a proof, not a comparison.** Every Ahem glyph is a
//! filled square of exactly 1em advance, so N characters at Ms px is exactly `N*M` px and nothing
//! else. A fallback cannot coincidentally produce that number; the assertion is `== 100.0`, not
//! "different from before".

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves the WPT Ahem face as `font/woff2` for any request.
fn font_origin() -> String {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../text/tests/fixtures/Ahem.woff2"
    ))
    .expect("Ahem fixture");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let b = bytes.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                if path.ends_with(".css") {
                    let css = "@font-face { font-family: 'AhemTest'; src: url(ahem.woff2) format('woff2'); }\n\
                               #a { font-family: 'AhemTest'; font-size: 20px; }\n\
                               #b { font-family: serif; font-size: 20px; }\n\
                               span { display: inline-block; }\n";
                    let _ = s.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{css}",
                            css.len()
                        )
                        .as_bytes(),
                    );
                    return;
                }
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
    format!("http://{addr}")
}

#[test]
fn a_downloaded_webfont_from_an_external_sheet_is_laid_out_with() {
    let fonts = FontContext::new();
    let origin = font_origin();

    // The `@font-face` lives in an EXTERNAL sheet, so `count > 0` and the early-return branch is the
    // one under test.
    let html = format!(
        r#"<!doctype html><html><head>
             <link rel="stylesheet" href="{origin}/site.css">
           </head><body>
             <div><span id="a">XXXXX</span></div>
             <div><span id="b">XXXXX</span></div>
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

    let dom = page.dom();
    let root = dom.root();
    let rects = page.root_box.node_rects(dom);
    let width = |sel: &str| -> f32 {
        let n = manuk_css::query_selector_all(dom, root, sel)[0];
        rects.get(&n).map(|r| r.width).unwrap_or(-1.0)
    };

    let ahem = width("#a");
    let fallback = width("#b");

    // THE claim: Ahem's advance is exactly 1em, so 5 chars at 20px is exactly 100px. Only the real
    // face can produce that number.
    assert!(
        (ahem - 100.0).abs() < 0.5,
        "G_WEBFONT_RELAYOUT(external): text in a downloaded @font-face family measured {ahem}px, not 100px.\n\n  \
         Ahem's every glyph is exactly 1em wide, so 5 characters at font-size:20px is exactly 100px \
         and no fallback face can land there by accident. {ahem}px means the document was laid out \
         with the FALLBACK and never re-laid-out once the font arrived. The font itself is fine — \
         manuk-text decodes, registers and measures it at exactly 100.0 — so a failure here is the \
         relayout guard, not the font stack: it listed `external CSS arrived` and `the tree is dirty` \
         as the reasons to re-cascade, and a newly-registered face is a third one."
    );

    // …and the fallback must NOT accidentally be 100px, or the assertion above proves nothing about
    // which face was used. This is the vacuity guard.
    assert!(
        (fallback - 100.0).abs() > 1.0,
        "G_WEBFONT_RELAYOUT(external) is VACUOUS: the serif fallback also measured {fallback}px, so `== 100` \
         cannot distinguish the web font from the default face. Pick a size/string where they differ."
    );
}
