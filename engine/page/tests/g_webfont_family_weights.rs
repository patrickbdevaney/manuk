//! **G_WEBFONT_FAMILY_WEIGHTS — a family declares four faces and only ONE of them is ever fetched.**
//!
//! A real site does not declare one `@font-face` per family. It declares one per **weight and
//! style**, all under the same `font-family` name — that is what the "self-host your Google font"
//! download produces, and it is the default shape on the open web:
//!
//! ```css
//! @font-face{font-family:Noto Serif;font-weight:400;font-style:normal; src:url(…regular.woff2)}
//! @font-face{font-family:Noto Serif;font-weight:400;font-style:italic; src:url(…italic.woff2)}
//! @font-face{font-family:Noto Serif;font-weight:700;font-style:normal; src:url(…700.woff2)}
//! @font-face{font-family:Noto Serif;font-weight:700;font-style:italic; src:url(…700italic.woff2)}
//! ```
//!
//! (verbatim from `https://www.a11yproject.com/css/screen.min.css`.)
//!
//! The registration loop in `fetch_and_apply_stylesheets` keyed its idempotence check on the
//! **family**:
//!
//! ```rust
//! if fonts.has_webfont_face(&ff.family) { continue; }
//! ```
//!
//! so the first block registered `regular` and the other three were never fetched. **Every bold and
//! every italic run on the page was then measured and painted in the REGULAR face** — and there is
//! no synthetic bold anywhere in `engine/text`, so this is not a slightly-wrong weight: bold text
//! had byte-identical advances to regular text.
//!
//! ⚠ **The consumer for the missing faces was already built, and could never fire.**
//! `FontContext::face_id` searches the family's registered ids for the matching weight/style —
//! *"picking the bold/italic variant when present"*, in its own comment — over a `Vec<ID>` that
//! `register_named_font` extends. Storage and selector both done; the producer delivered one face,
//! forever, so the search was dead code and `ids.first()` was the only reachable path. The guard's
//! *purpose* (this function re-runs after every round of dynamic scripts, and a new face forces a
//! full-document relayout) is real; only its KEY was wrong, and the fix keys on the resolved src URL
//! so a re-run is still a no-op.
//!
//! **The second claim, same block: a relative `src` resolves against the STYLESHEET, not the
//! document.** The loop is iterating `sources` and holds the sheet's own URL, then throws it away
//! and calls `resolve_url(&self.final_url, src)`. `/assets/css/main.css` + `url(../fonts/x.ttf)` is
//! `/assets/fonts/x.ttf`; against the document it is `/fonts/x.ttf`, which 404s and falls back
//! silently. This fixture serves the sheet from `/assets/css/` and answers **404 for `/fonts/`**, so
//! the document-relative resolution cannot pass by accident.
//!
//! **The assertion is a control, not a constant.** `#ctl` names a single-face family whose one src
//! is the *same bold bytes*, so `#bold.width == #ctl.width` says "the bold face was used" without
//! hard-coding a metric — and `#bold.width != #reg.width` is the vacuity guard that stops the whole
//! thing passing when every span shares one face.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

const REG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../demo/fonts/LiberationSans-Regular.ttf"
);
const BOLD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../demo/fonts/LiberationSans-Bold.ttf"
);

/// The sheet lives at `/assets/css/main.css` and the faces at `/assets/fonts/*.ttf`, reachable only
/// by resolving `url(../fonts/…)` against the SHEET. Anything under `/fonts/` — where a
/// document-relative resolution lands — is a 404.
fn origin() -> String {
    let reg = std::fs::read(REG).expect("LiberationSans-Regular.ttf fixture");
    let bold = std::fs::read(BOLD).expect("LiberationSans-Bold.ttf fixture");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let (reg, bold) = (reg.clone(), bold.clone());
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

                let body: Option<(&str, Vec<u8>)> = match path.as_str() {
                    "/assets/css/main.css" => {
                        // Two faces, ONE family — the shape every self-hosted webfont ships.
                        let css = "\
@font-face{font-family:'TwoFace';font-weight:400;src:url(../fonts/reg.ttf) format('truetype')}\n\
@font-face{font-family:'TwoFace';font-weight:700;src:url(../fonts/bold.ttf) format('truetype')}\n\
@font-face{font-family:'OnlyBold';font-weight:400;src:url(../fonts/bold.ttf) format('truetype')}\n\
span{display:inline-block;font-size:40px}\n\
#reg{font-family:'TwoFace';font-weight:400}\n\
#bold{font-family:'TwoFace';font-weight:700}\n\
#ctl{font-family:'OnlyBold'}\n";
                        Some(("text/css", css.as_bytes().to_vec()))
                    }
                    "/assets/fonts/reg.ttf" => Some(("font/ttf", reg)),
                    "/assets/fonts/bold.ttf" => Some(("font/ttf", bold)),
                    _ => None, // includes /fonts/* — the document-relative miss
                };

                match body {
                    Some((ct, b)) => {
                        let mut h = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n",
                            b.len()
                        )
                        .into_bytes();
                        h.extend_from_slice(&b);
                        let _ = s.write_all(&h);
                    }
                    None => {
                        let _ = s.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn every_declared_face_of_a_family_is_registered_not_just_the_first() {
    let fonts = FontContext::new();
    let origin = origin();

    // The same string in all three spans: the only variable is which FACE measured it.
    let html = format!(
        r#"<!doctype html><html><head>
             <link rel="stylesheet" href="{origin}/assets/css/main.css">
           </head><body>
             <div><span id="reg">Handgloves</span></div>
             <div><span id="bold">Handgloves</span></div>
             <div><span id="ctl">Handgloves</span></div>
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

    let reg = width("#reg");
    let bold = width("#bold");
    let ctl = width("#ctl");

    // VACUITY GUARD FIRST — if the sheet or the faces never arrived, every span shares one fallback
    // and the real assertion below would be comparing a number to itself. `#ctl` is a one-face
    // family, so it is also the proof that `../fonts/…` resolved against the SHEET: nothing under
    // `/fonts/` exists on this server.
    assert!(
        reg > 0.0 && bold > 0.0 && ctl > 0.0,
        "G_WEBFONT_FAMILY_WEIGHTS is VACUOUS: a span has no box (reg={reg} bold={bold} ctl={ctl})"
    );
    assert!(
        (ctl - reg).abs() > 1.0,
        "G_WEBFONT_FAMILY_WEIGHTS is VACUOUS: the bold face (#ctl, {ctl}px) and the regular face \
         (#reg, {reg}px) measure the same, so `#bold == #ctl` cannot tell them apart.\n\n  \
         Either both fell back to one system face — which is what a relative `src` resolved against \
         the DOCUMENT instead of the SHEET looks like, since this server 404s everything under \
         /fonts/ — or the two fixture files are not actually different faces."
    );

    // THE claim: the family's 700 face was fetched and selected.
    assert!(
        (bold - ctl).abs() < 0.5,
        "G_WEBFONT_FAMILY_WEIGHTS: bold text in a two-face @font-face family measured {bold}px; the \
         same bytes reached through a single-face family measure {ctl}px.\n\n  \
         {bold}px == the REGULAR face ({reg}px) means the family's second `@font-face` block was \
         never fetched: the registration loop keyed its idempotence check on the FAMILY \
         (`has_webfont_face(&ff.family)`), so the first face registered and every later weight and \
         style of the same family hit `continue`. Every bold and italic run on such a page is then \
         painted in the regular face — there is no synthetic bold to hide it. The guard is needed \
         (this function re-runs per script round and a new face forces a full relayout); key it on \
         the resolved SRC URL, which is stable across re-runs and distinct per face."
    );
}
