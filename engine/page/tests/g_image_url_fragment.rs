//! **G_IMAGE_URL_FRAGMENT — a fragment on an image URL is not part of the path, and leaving it on
//! meant the image did not load AT ALL.**
//!
//! `background-image: url("icons.svg#icon-home")` is the SVG sprite idiom: one file, a fragment
//! naming the piece you want. The fragment identifies a part of the resource **after** it has been
//! retrieved (RFC 3986 §3.5) and is never handed to the transport — but ours reached
//! `std::fs::read` as the literal filename `icons.svg#icon-home`, which does not exist.
//!
//! ⚠⚠⚠ **The failure was not a wrong crop — it was NO IMAGE.** That distinction is the whole reason
//! this gate asserts a decoded size on the *control* rows as well: an implementation that drops
//! every fragmented URL passes any assertion phrased as "the cropped image is not 200x200", because
//! there is no image to be 200x200 either.
//!
//! ⚠⚠ **And it reaches further than `#xywh=`.** ANY fragment did this — `#icon-home`, `#svgView(...)`,
//! a stale `#` left on a URL by a template. The `#not-a-spatial-fragment` row is in the battery for
//! exactly that reason: it must load, at its FULL natural size, because a fragment that selects
//! nothing spatial selects the whole image.
//!
//! Measured against Chrome (`red-green-200x200.svg` is a 200x200 red square with a green 100x100
//! quadrant at 100,100 — the WPT `svg/linking/reftests/support` fixture):
//!
//! ```text
//!                                              Chrome    before      after
//!   url(a.svg)                     no fragment  200x200   200x200    200x200   <- CONTROL
//!   url(a.svg#not-a-spatial-frag)               200x200   NO IMAGE   200x200
//!   url(a.svg#xywh=100,100,100,100)             100x100   NO IMAGE   100x100
//!   url(a.svg#xywh=pixel:100,100,100,100)       100x100   NO IMAGE   100x100
//!   url(a.svg#xywh=percent:50,50,50,50)         100x100   NO IMAGE   100x100
//!   url(a.svg#xywh=0,0,400,400)      clamped    200x200   NO IMAGE   200x200
//!   url(a.svg#xywh=100,100,-100,100) invalid    200x200   NO IMAGE   200x200
//!   url(a.svg#xywh=0,0,50,50&xywh=100,100,100,100)        100x100    100x100   <- last VALID wins
//! ```
//!
//! ⚠⚠⚠ **THE `invalid-after-valid` ROW EXISTS BECAUSE A MUTATION CAME BACK GREEN.** The obvious
//! assertion — *"a negative width yields the whole 200x200 image"* — does **not** distinguish
//! *discarding* the fragment from *clamping* it, because clamping a negative extent also collapses
//! the region to nothing and falls back to the whole image. Deleting the `w <= 0` guard left the
//! gate GREEN, which is a reading about the row, not a failed RED-proof: the row was asserting less
//! than its comment claimed.
//!
//! `#xywh=100,100,100,100&xywh=0,0,-50,50` separates them. If an invalid pair is *discarded* it is
//! not a candidate, so the earlier valid one wins and the answer is **100x100**; if it is accepted
//! and clamped it becomes the last candidate, collapses, and the answer is 200x200. That row goes
//! red for the mutation the `invalid` row could not see — and it is also the only row that proves an
//! invalid pair does not invalidate an earlier valid one.

use manuk_page::Page;
use manuk_text::FontContext;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <rect width="200" height="200" fill="red"/>
  <rect x="100" y="100" width="100" height="100" fill="green"/>
</svg>"#;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_fragment_on_an_image_url_is_stripped_before_the_fetch_and_applied_after_it() {
    let dir = std::env::temp_dir().join(format!("manuk-frag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let svg_path = dir.join("a.svg");
    std::fs::write(&svg_path, SVG).expect("write svg");

    // Each case gets its own element so one page load measures the whole battery.
    let cases: Vec<(&str, &str, u32, u32)> = vec![
        ("plain", "", 200, 200),
        ("nonspatial", "#icon-home", 200, 200),
        ("bare", "#xywh=100,100,100,100", 100, 100),
        ("pixel", "#xywh=pixel:100,100,100,100", 100, 100),
        ("percent", "#xywh=percent:50,50,50,50", 100, 100),
        ("clamped", "#xywh=0,0,400,400", 200, 200),
        ("invalid", "#xywh=100,100,-100,100", 200, 200),
        ("lastwins", "#xywh=0,0,50,50&xywh=100,100,100,100", 100, 100),
        (
            "invalid-after-valid",
            "#xywh=100,100,100,100&xywh=0,0,-50,50",
            100,
            100,
        ),
    ];
    let body: String = cases
        .iter()
        .map(|(id, frag, _, _)| {
            format!(
                r#"<div id="{id}" style="width:100px;height:100px;background-repeat:no-repeat;background-image:url('a.svg{frag}')"></div>"#
            )
        })
        .collect();
    let html = format!("<!doctype html><body>{body}</body>");
    let base = format!("file://{}/", dir.display());

    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // ⚠ `finish_loading` is not optional: background images are fetched by the SUBRESOURCE pass
    // (`fetch_and_apply_background_images`), which is the path that carries the URL to the decoder.
    let page = rt.block_on(async {
        let mut p = Page::load_async(&html, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let dom = page.dom();
    let mut got: Vec<String> = Vec::new();
    for (id, _, _, _) in &cases {
        let node = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(*id));
        let size = node
            .and_then(|n| page.decoded_images().get(&n))
            .map(|i| format!("{}x{}", i.width, i.height))
            .unwrap_or_else(|| "NO-IMAGE".to_string());
        got.push(format!("{id}={size}"));
    }
    let line = got.join(" ");
    println!("IMAGE-URL-FRAGMENT {line}");

    let _ = std::fs::remove_dir_all(&dir);

    for ((id, frag, w, h), _) in cases.iter().zip(0..) {
        let want = format!("{id}={w}x{h}");
        assert!(
            line.contains(&want),
            "url('a.svg{frag}') must decode to {w}x{h} — a fragment is stripped before the fetch \
             (RFC 3986 §3.5) and the SPATIAL part applied after it. `NO-IMAGE` is the pre-fix \
             answer: the fragment stayed on the filename and nothing loaded at all. got: {line}"
        );
    }
}
