//! **G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE — a decoded image keeps its own size across every
//! re-cascade, and the content below it stays below it.**
//!
//! An image's natural size is the one geometry input that is **not in any stylesheet.** It arrives
//! from the network long after the cascade that has to lay it out, so it was written straight into
//! the cascade's *output* — once, by `apply_images` — and every later cascade rebuilt that map from
//! the stylesheets and erased it. Measured on a page with no CSS at all and one 41×23 image:
//!
//! ```text
//!   after load_async   (image applied)     width=Px(41)  ar=Some(1.78)   rect  41×23
//!   after finish_loading (re-cascaded)     width=Auto    ar=None         rect 784×0
//! ```
//!
//! **784×0 is the full content width and no height.** The picture occupies no space, and everything
//! below it slides up into the space it should have taken. And it never recovered: the image phase's
//! own per-node dedup means a second pass has nothing to re-apply, so the size was applied exactly
//! once and whichever cascade ran after it was final. A page whose images are its content lays out as
//! a stack of zero-height strips — which is the picture-shaped-hole signature the fidelity sweep
//! records, and it is invisible to coverage, because every one of those elements is present.
//!
//! **What this gate asserts, and why it is three things.** That the image is its own size; that the
//! paragraph after it is pushed *below* it (the user-visible consequence — a size nothing consumes is
//! not a fixed size); and that both hold again after a **second, independent** re-cascade trigger.
//! The last is not belt-and-braces: the style map is rebuilt at more than a dozen call sites, and a
//! rule with N implementations is not proven by the one a gate happens to touch — the same lesson
//! tick 654 paid for with eight of them.
//!
//! Hermetic: one loopback socket. The external sheet is there to *guarantee* a re-cascade happens
//! (its own effect is asserted as a precondition), so this gate cannot pass by never reaching the bug.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

const IMG_W: u32 = 41;
const IMG_H: u32 = 23;
/// Nothing like the viewport, so "the sheet cascaded" is unmistakable.
const MARKER_WIDTH: i64 = 321;

const HTML: &str = r##"<!doctype html><html><head>
<link rel="stylesheet" href="site.css">
</head><body>
<img id="pic" src="a.bmp">
<p id="after">the paragraph that must stay BELOW the picture</p>
<div id="marker">proves the external sheet cascaded</div>
<div id="hit">click me</div>
<div id="out">-</div>
<script>
  // A resolved `fetch()` that MUTATES the DOM — the shape of every data-driven page, and one of the
  // re-cascade triggers.
  fetch('data.json').then(function (r) { return r.text(); }).then(function (t) {
    document.getElementById('out').textContent = 'fetched:' + t.trim();
  });
  document.getElementById('hit').addEventListener('click', function () {
    document.getElementById('out').textContent += ' clicked';
  });
</script>
</body></html>"##;

const CSS: &str = "#marker { width: 321px; height: 10px; }\n";
const DATA: &str = "ok";

/// A 24-bit BMP of exactly `w` × `h`, built by hand — no image-encoder dev-dependency and no CRC to
/// get wrong. `image` sniffs the `BM` magic, so the extension is courtesy only.
fn bmp(w: u32, h: u32) -> Vec<u8> {
    let row = ((w * 3 + 3) / 4) * 4;
    let pixels = (row * h) as usize;
    let mut v = Vec::with_capacity(54 + pixels);
    v.extend_from_slice(b"BM");
    v.extend_from_slice(&((54 + pixels) as u32).to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&54u32.to_le_bytes());
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&(w as i32).to_le_bytes());
    v.extend_from_slice(&(h as i32).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&24u16.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&(pixels as u32).to_le_bytes());
    v.extend_from_slice(&2835i32.to_le_bytes());
    v.extend_from_slice(&2835i32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.resize(54 + pixels, 0x7f);
    v
}

fn rect_of(page: &manuk_page::Page, id: &str) -> Option<(i64, i64, i64, i64)> {
    let root = page.dom().root();
    let node = *manuk_css::query_selector_all(page.dom(), root, id).first()?;
    page.root_box.node_rects(page.dom()).get(&node).map(|r| {
        (
            r.x.round() as i64,
            r.y.round() as i64,
            r.width.round() as i64,
            r.height.round() as i64,
        )
    })
}

/// The image's laid-out `(width, height)`.
fn pic_size(page: &manuk_page::Page) -> Option<(i64, i64)> {
    rect_of(page, "#pic").map(|(_, _, w, h)| (w, h))
}

#[test]
fn a_decoded_image_keeps_its_own_size_across_every_recascade() {
    let tmp = std::env::temp_dir().join(format!("manuk-imgnat-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    unsafe { std::env::set_var("MANUK_LOAD_BUDGET_MS", "12000") };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, body, ctype): (u16, Vec<u8>, &str) = match path.as_str() {
                    "/site.css" => (200, CSS.as_bytes().to_vec(), "text/css"),
                    "/data.json" => (200, DATA.as_bytes().to_vec(), "text/plain"),
                    "/a.bmp" => (200, bmp(IMG_W, IMG_H), "image/bmp"),
                    _ => (404, b"not found".to_vec(), "text/plain"),
                };
                let head = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    if status == 200 { "OK" } else { "Not Found" },
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(&body);
                let _ = sock.flush();
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{addr}/index.html");
    let fonts = FontContext::new();
    let mut page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        // The external-CSS phase and the page-fetch pump both live here. A gate that stops at
        // `load_async` never reaches a re-cascade at all and would pass by not looking — the image
        // is correctly sized at that point, which is exactly what made this bug invisible.
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    println!(
        "NATURAL-SIZE PROBE: pic={:?} after={:?} marker={:?}",
        rect_of(&page, "#pic"),
        rect_of(&page, "#after"),
        rect_of(&page, "#marker"),
    );

    // ── PRECONDITIONS. Without these a green could mean "the image never arrived" or "no re-cascade
    //    ever ran", and both are greens about the empty case.
    assert_eq!(
        rect_of(&page, "#marker").map(|(_, _, w, _)| w),
        Some(MARKER_WIDTH),
        "the gate's own fixture failed: the external sheet did not cascade, so this run never \
         rebuilt the style map and never exercised the defect."
    );
    let dom = page.dom();
    let out_node = manuk_css::query_selector_all(dom, dom.root(), "#out")[0];
    let text = dom.text_content(out_node);
    assert!(
        text.contains("fetched:ok"),
        "the gate's own trigger did not fire — the page's fetch never resolved into the DOM, so the \
         second re-cascade never happened.\n  out: {text}"
    );

    // ── THE CLAIM, part 1: the picture is its own size.
    assert_eq!(
        pic_size(&page),
        Some((IMG_W as i64, IMG_H as i64)),
        "G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE: after the document was re-cascaded, the image is no \
         longer {IMG_W}×{IMG_H}. Its intrinsic size lives in NO stylesheet, so a cascade that \
         rebuilds the style map erases it and the picture becomes a full-width strip of zero height. \
         Restate it between the cascade and the layout (`apply_natural_sizes`), at every site that \
         rebuilds the map — it is a STANDING input to layout, not an event that happened once."
    );

    // ── THE CLAIM, part 2: and something CONSUMES that size. A width the layout ignores is not a
    //    fixed bug, and asserting only the image's own box would not tell the two apart.
    let (_, pic_y, _, pic_h) = rect_of(&page, "#pic").expect("the image has a box");
    let (_, after_y, _, _) = rect_of(&page, "#after").expect("the paragraph has a box");
    assert!(
        after_y >= pic_y + pic_h,
        "the paragraph starts at y={after_y}, inside or above the image's own box \
         (y={pic_y}, height={pic_h}) — the image's height is not reaching layout, so every element \
         below a picture slides up into it."
    );

    // ── THE CLAIM, part 3: a SECOND, independent re-cascade trigger. The style map is rebuilt at
    //    more than a dozen call sites; one rule with N implementations is not proven by one of them.
    let dom = page.dom();
    let hit = manuk_css::query_selector_all(dom, dom.root(), "#hit")[0];
    page.dispatch_click(hit, &fonts, 800.0);
    let dom = page.dom();
    let out_node = manuk_css::query_selector_all(dom, dom.root(), "#out")[0];
    let after_txt = dom.text_content(out_node);
    assert!(
        after_txt.contains("clicked"),
        "the click trigger did not fire — the handler never ran, so this half asserts nothing.\n  \
         out: {after_txt}"
    );
    assert_eq!(
        pic_size(&page),
        Some((IMG_W as i64, IMG_H as i64)),
        "G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE: the CLICK path erased the image's natural size. \
         Every path that rebuilds the style map has to restate it, not just the one the first half \
         of this gate exercises."
    );
}
