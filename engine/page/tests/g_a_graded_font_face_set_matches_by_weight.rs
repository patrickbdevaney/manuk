//! **G_A_GRADED_FONT_FACE_SET_MATCHES_BY_WEIGHT — nine declared weights, two faces answered.**
//!
//! `manuk_text::FontKey` carried `bold: bool` at a 600 threshold, so a family shipping one file per
//! weight — which is how Google Fonts and every self-hosting pipeline deliver — could answer with at
//! most two of them. `@font-face` sets of 300/400/500/600/700 are the ordinary case, not a corner.
//!
//! ## Arbitrated against headless Chrome, on a graded local set
//!
//! ```text
//!   @font-face: 400 -> Lato-Regular · 500 -> Lato-Medium · 700 -> Lato-Bold
//!
//!            400    500    700    450
//!   Chrome   312    314    320    314
//!   before   312    312    320    312
//!   after    312    314    320    314
//! ```
//!
//! ⚠ **`450` IS THE ROW THAT SEPARATES §5.2 FROM "NEAREST NUMBER".** It is equidistant from 400 and
//! 500, and the spec is directional: inside 400–500, look UP to 500 first. Chrome takes the 500 face.
//! A distance metric ties and takes whichever came first.
//!
//! ## ⚠⚠ THIS IS THE THIRD ATTEMPT, AND THE FIRST TWO ARE WHY IT IS SHAPED THIS WAY
//!
//! * **t1492** replaced the boolean and ran §5.2 over each face's **FILE** weight. It was
//!   Chrome-byte-identical on two fixtures and **regressed `wpt css/css-fonts/variations` by 92
//!   subtests**; the assertions said why — *"matching for weight 420 should be mapped to CSSTest
//!   Weights 600"*. Refused and reverted whole.
//! * **t1493** carried the `@font-face` **DESCRIPTOR** (`manuk_css::FontFace::weight`) and matched the
//!   coarse rule over it. `variations` went 237/242 → 247/247.
//! * **This tick** does both: the numeric key, and §5.2 over the declared weight. `variations` holds
//!   at **247/247**.
//!
//! ## ⚠ AND THE SYSTEM-FONT PATH IS DELIBERATELY LEFT COARSE
//!
//! Handing `fontdb::Query` the real weight instead of `BOLD`/`NORMAL` takes `variations` from
//! **247 → 148**, measured twice each, with everything else identical. That is isolated to one line
//! and it is **not** shipped. So a system family still collapses:
//!
//! ```text
//!   system `Lato`   300   400   500   700   900
//!     Chrome        303   312   314   320   327
//!     ours          312   312   312   320   320     <- unchanged, and NOT claimed
//! ```
//!
//! ⚠ **The cause is not established.** The `variations` tests load their faces from
//! `./resources/…` through the `@font-face` path; if those loads are failing in the harness, the
//! suite is scoring FALLBACK behaviour and a coarse fallback happens to match more of the expected
//! widths. That is a hypothesis, and the next probe is to check whether those faces register at all.
//! Until then the metric binds and the line stays coarse.
//!
//! Mutations that must turn this red:
//!   1. `weight_is_closer` -> nearest-number        -> the 450 row takes the 400 face
//!   2. match the FILE weight, not the declared     -> 312 312 320 312 (t1492's shape)
//!   3. collapse the key back to a 600 threshold    -> 312 312 320 312

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
@font-face { font-family: "GradedProbe"; font-weight: 400; src: url("w400.ttf") format("truetype"); }
@font-face { font-family: "GradedProbe"; font-weight: 500; src: url("w500.ttf") format("truetype"); }
@font-face { font-family: "GradedProbe"; font-weight: 700; src: url("w700.ttf") format("truetype"); }
span { font-family: "GradedProbe", monospace; font-size: 32px; }
#a{font-weight:400} #b{font-weight:500} #c{font-weight:700} #d{font-weight:450}
</style></head><body>
<span id="a">MMMMMwwwwwiiiii</span><br><span id="b">MMMMMwwwwwiiiii</span><br>
<span id="c">MMMMMwwwwwiiiii</span><br><span id="d">MMMMMwwwwwiiiii</span>
<div id="out">-</div>
<script>window.addEventListener('load', function () {
  document.getElementById('out').textContent = ['a','b','c','d'].map(function (i) {
    return i + '=' + Math.round(document.getElementById(i).getBoundingClientRect().width); }).join(' ');
});</script></body></html>"##;

#[test]
fn a_graded_font_face_set_answers_each_weight_with_its_own_face() {
    let faces = [
        (
            "w400.ttf",
            "/usr/share/fonts/truetype/lato/Lato-Regular.ttf",
        ),
        ("w500.ttf", "/usr/share/fonts/truetype/lato/Lato-Medium.ttf"),
        ("w700.ttf", "/usr/share/fonts/truetype/lato/Lato-Bold.ttf"),
    ];
    if faces.iter().any(|(_, p)| !std::path::Path::new(p).exists()) {
        eprintln!(
            "SKIP: Lato's graded faces are not installed — this gate asserts face SELECTION and \
             cannot do so where there is nothing to select between."
        );
        return;
    }
    let dir = std::path::Path::new("/tmp/g1494");
    std::fs::create_dir_all(dir).unwrap();
    for (dst, src) in faces {
        std::fs::copy(src, dir.join(dst)).unwrap();
    }
    std::fs::write(dir.join("ff.html"), HTML).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            HTML,
            &format!("file://{}/ff.html", dir.display()),
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
    println!("GRADED @FONT-FACE: {got}  webfonts={:?}", page.webfonts());

    // ── VACUITY, both halves. All three faces must have arrived, or these are a fallback's widths;
    //    and three DISTINCT widths must appear among 400/500/700, or the set is still collapsing and
    //    the `450` row below would be passing for the wrong reason.
    assert_eq!(
        page.webfonts(),
        (3, 3),
        "VACUOUS: the graded set did not fully arrive, so its widths are a fallback's"
    );
    let w: Vec<&str> = got
        .split_whitespace()
        .filter_map(|v| v.split('=').nth(1))
        .collect();
    let mut d: Vec<&str> = w[..3].to_vec();
    d.sort_unstable();
    d.dedup();
    assert_eq!(
        d.len(),
        3,
        "VACUOUS: 400/500/700 did not select three DIFFERENT faces — a collapse repeats a width, and \
         that was the whole bug: {got:?}"
    );

    // Chrome headless, every row.
    let want = "a=312 b=314 c=320 d=314";
    assert_eq!(
        got, want,
        "\n  §5.2 is a closest-match over the DECLARED weight, and `450` is the row that separates \
         it from nearest-number: equidistant from 400 and 500, and the spec looks UP\n  want: \
         {want}\n  got:  {got}"
    );
}
