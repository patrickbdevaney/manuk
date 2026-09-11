//! **G_A_FONT_FACE_DECLARES_ITS_OWN_WEIGHT — the number §5.2 matches against was never carried.**
//!
//! CSS Fonts §5.2 matches a requested `font-weight` against the **`@font-face` block's own
//! `font-weight` DESCRIPTOR**, not against the weight recorded inside the font file. A block may
//! legitimately declare `font-weight: 600` for a file whose internal weight is 400 — subsetters,
//! self-hosting pipelines and icon fonts all do it — and the DECLARATION is what a page asking for
//! 600 must get.
//!
//! `manuk_css::FontFace` carried `family`, `srcs` and `unicode_range`, and **no weight at all**, so
//! the face registry had only ever known the file's.
//!
//! ## Why this is its own tick
//!
//! t1492 built §5.2's closest-match over each registered face's *file* weight and **regressed
//! `wpt css/css-fonts/variations` by 92 subtests**. The failing assertions named the error precisely:
//!
//! ```text
//!   Test @font-face matching for weight 420
//!     @font-face should be mapped to CSSTest Weights 600 — got the 300 face
//! ```
//!
//! *A closest-match over the wrong number is more confidently wrong than a coarse match over it.*
//! That tick was refused and reverted whole; carrying the descriptor is step 1 of its own three-step
//! plan, and the rule here is deliberately **unchanged** — still the coarse bold/not-bold test the
//! boolean key can express. Only its INPUT moves.
//!
//! ```text
//!   wpt css/css-fonts/variations
//!     clean tree, two runs      237/468   242/468
//!     with the descriptor       247/468   247/468
//! ```
//!
//! ## The fixture inverts the two, which is the only way to tell them apart
//!
//! Two blocks in one family: the one declaring **400** points at the **BOLD** file, and the one
//! declaring **700** points at the **REGULAR** file. Lato-Regular measures 312 for the probe string
//! and Lato-Bold 320, so:
//!
//! ```text
//!   Chrome    a(400)=320   b(700)=312     <- the DECLARATION wins, inverted from the files
//!   before    a(400)=312   b(700)=320     <- the FILE wins
//! ```
//!
//! A fixture whose declarations agreed with its files could not distinguish the two at all.
//!
//! ⚠ **`None` means "the block did not say", and falls back to the file's weight** — which is the old
//! behaviour exactly. The change is confined to blocks that do declare, so no page that was right
//! becomes wrong.
//!
//! ⚠ **A REVERSED RANGE IS INVALID, NOT SWAPPED.** `font-weight: 900 100` yields `None`, because the
//! spec says such a descriptor is invalid — that is what `css-fonts/variations`'s
//! `font-descriptor-range-reversed` tests are about, and silently swapping would pass them for the
//! wrong reason.
//!
//! Mutations that must turn this red:
//!   1. drop the `"font-weight"` arm from the descriptor parser  -> a=312 b=320 (the file wins)
//!   2. drop `ff.weight` at the `register_named_font` call site  -> a=312 b=320
//!   3. `Some((_, hi)) => hi >= 600` -> read the file's weight    -> a=312 b=320
//!   4. swap a reversed range instead of rejecting it             -> the parse arm below

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
/* DELIBERATELY INVERTED: the block declaring 400 points at the BOLD file, and vice versa. */
@font-face { font-family: "Swapped"; font-weight: 400; src: url("bold.ttf") format("truetype"); }
@font-face { font-family: "Swapped"; font-weight: 700; src: url("reg.ttf") format("truetype"); }
span { font-family: "Swapped", monospace; font-size: 32px; }
#a{font-weight:400} #b{font-weight:700}
</style></head><body>
<span id="a">MMMMMwwwwwiiiii</span><br><span id="b">MMMMMwwwwwiiiii</span>
<div id="out">-</div>
<script>window.addEventListener('load', function () {
  document.getElementById('out').textContent = ['a','b'].map(function (i) {
    return i + '=' + Math.round(document.getElementById(i).getBoundingClientRect().width); }).join(' ');
});</script></body></html>"##;

#[test]
fn a_font_face_block_matches_on_the_weight_it_declares() {
    // ── The descriptor parser, exercised directly. The rendered arm below cannot reach the reversed
    //    or keyword forms, and a parser is where a spec detail is cheapest to pin.
    let faces = |css: &str| manuk_css::Stylesheet::parse(css).font_faces().to_vec();
    let w = |css: &str| faces(css).first().and_then(|f| f.weight);
    let src = |d: &str| format!("@font-face {{ font-family: F; {d} src: url(x.ttf); }}");
    assert_eq!(w(&src("font-weight: 600;")), Some((600, 600)));
    assert_eq!(w(&src("font-weight: 100 900;")), Some((100, 900)));
    assert_eq!(w(&src("font-weight: normal;")), Some((400, 400)));
    assert_eq!(w(&src("font-weight: bold;")), Some((700, 700)));
    assert_eq!(
        w(&src("font-weight: 900 100;")),
        None,
        "a REVERSED range is INVALID per spec, not silently swapped — `font-descriptor-range-reversed` \
         is a test, and swapping would pass it for the wrong reason"
    );
    assert_eq!(
        w(&src("")),
        None,
        "\"the block did not say\" and \"the block said 400\" are different facts, and only the first \
         may fall back to the file's own weight"
    );

    let dir = std::path::Path::new("/tmp/g1493");
    std::fs::create_dir_all(dir).unwrap();
    let (reg, bold) = (
        "/usr/share/fonts/truetype/lato/Lato-Regular.ttf",
        "/usr/share/fonts/truetype/lato/Lato-Bold.ttf",
    );
    if !std::path::Path::new(reg).exists() || !std::path::Path::new(bold).exists() {
        eprintln!(
            "SKIP: Lato Regular/Bold are not installed — the rendered arm needs two faces of \
                   measurably different width to tell the declaration from the file."
        );
        return;
    }
    std::fs::copy(reg, dir.join("reg.ttf")).unwrap();
    std::fs::copy(bold, dir.join("bold.ttf")).unwrap();
    std::fs::write(dir.join("d.html"), HTML).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            HTML,
            &format!("file://{}/d.html", dir.display()),
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DECLARED WEIGHT: {got}  webfonts={:?}", page.webfonts());

    // ── VACUITY. Both faces must have arrived, or this is a fallback's widths and the inversion
    //    below would be meaningless.
    assert_eq!(
        page.webfonts(),
        (2, 2),
        "VACUOUS: both @font-face blocks must deliver a usable face, or the widths are a fallback's"
    );

    // Chrome headless. `a` asks for 400 and must get the BOLD file, because that is the file the
    // block declaring 400 points at. Inverted from the files — which is the whole point.
    let want = "a=320 b=312";
    assert_eq!(
        got, want,
        "\n  §5.2 matches the @font-face block's DECLARED font-weight, not the weight inside the \
         file it points at\n  want: {want}\n  got:  {got}"
    );
}
