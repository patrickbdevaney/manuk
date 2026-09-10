//! **G_A_DECLARED_WEBFONT_THAT_NEVER_ARRIVES_IS_COUNTED — a family this page claimed and could not
//! deliver.**
//!
//! CSS Fonts' shadowing rule is about the DECLARATION: once a document says
//! `@font-face { font-family: "Open Sans" }`, a locally-installed `Open Sans` is **shadowed for that
//! document**, and if every `src` fails the family yields no usable face. This engine implements that
//! deliberately — `declare_webfont_family`'s own comment records why, at t559/t560: declaring only on
//! success let a failed download be masked by a same-named local face, *"which looked like the wrong
//! font rather than like a failure"*, and cost 19 SHAPE points on `martinfowler.com`.
//!
//! The consequence is that **a failed webfont is silent by construction.** The page renders in a
//! fallback, still reporting the family it asked for, and nothing anywhere counted it.
//!
//! ## Why it was built (t1489) — a survey that needed a number instead of a guess
//!
//! Surveying the near-bar cohort (shape 55–75%) by the `font` field the oracle already carried:
//!
//! ```text
//!   562 of 1,170 near-bar shape misses — 48% — are on elements where the FAMILY and the SIZE
//!   agree and the measured ADVANCE does not:
//!       payb.jp         Noto Sans JP/16   chrome 168  ours 158
//!       puentedemando   Google Sans/16    chrome 168  ours 161
//!       pivaldi         Open Sans/14      chrome 141  ours 149
//! ```
//!
//! ⚠ **AND A CONTROLLED FIXTURE REFUTED THE OBVIOUS READING.** One page, one Google Fonts webfont,
//! one system stack, one monospace: `w=155 s=142 m=144` against Chrome's `w=154 s=142 m=144`. **Our
//! text metrics are right, including on a webfont that loads.** So the divergence is *which face each
//! engine ends up with* — and a declared-but-undelivered family is the one thing that makes that
//! diverge without any error anywhere.
//!
//! ## What this gate pins
//!
//! 1. **A face that ARRIVES is counted as loaded.** Without a positive row the counter is satisfied
//!    by an engine that loads nothing.
//! 2. **A face that 404s is DECLARED and NOT loaded** — it must appear in the denominator, because
//!    the whole point is that it shadowed a local face and cost the page its metrics.
//! 3. **A page with no `@font-face` reports `(0, 0)`**, not a phantom.
//!
//! Mutations that must turn this red:
//!   1. count `declared` only on success   -> the failing face vanishes from the denominator
//!   2. count `loaded` unconditionally     -> the 404 reads as delivered
//!   3. count with `+=` instead of a keyed set -> the second style round doubles the denominator

use manuk_text::FontContext;

#[test]
fn a_webfont_that_arrives_and_one_that_does_not_are_both_counted() {
    let dir = std::path::Path::new("/tmp/g1489");
    std::fs::create_dir_all(dir).unwrap();
    assert!(
        dir.join("ok.ttf").exists(),
        "fixture precondition: a real font file must be present at /tmp/g1489/ok.ttf"
    );
    let html = r##"<!doctype html><html><head><meta charset=utf-8><style>
@font-face { font-family: "ArrivesFam"; src: url("ok.ttf") format("truetype"); }
@font-face { font-family: "MissingFam"; src: url("no-such-font-1489.ttf") format("truetype"); }
</style></head><body><p style="font-family: ArrivesFam">a</p>
<p style="font-family: MissingFam">b</p></body></html>"##;
    std::fs::write(dir.join("page.html"), html).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            html,
            &format!("file://{}/page.html", dir.display()),
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let (declared, loaded) = page.webfonts();
    println!("WEBFONTS: {loaded} of {declared}");

    // ── VACUITY, and it is the half that matters. `loaded < declared` is trivially satisfied by an
    //    engine that never loads anything, which is the failure mode a naive counter would hide.
    assert_eq!(
        loaded, 1,
        "VACUOUS: the face that DOES arrive was not counted as loaded, so `{loaded} of {declared}` \
         is describing an engine that fetched nothing rather than one that fetched one of two"
    );
    assert_eq!(
        declared, 2,
        "a face whose `src` 404s must still be DECLARED — it shadowed any local family of the same \
         name (CSS Fonts' rule, and this engine implements it on purpose), so leaving it out of the \
         denominator is exactly the silence the counter exists to break: got {loaded} of {declared}"
    );

    // ── **A SECOND STYLE ROUND MUST NOT MOVE THE COUNT.** `fetch_and_apply_stylesheets` re-visits
    //    every `@font-face` block on later rounds; `claim_webfont_src` short-circuits the FETCH but
    //    the declaration runs again, so a `+=` counter inflates the DENOMINATOR once per round while
    //    the numerator cannot follow. That is not hypothetical — `pivaldi.restoplace.ws` first read
    //    "20 of 122" for a page with 61 blocks, and this arm is why the count is keyed per face.
    //
    //    ⚠ This arm exists because mutation 3 was INERT against the single-round fixture above.
    //    Asking why exposed that neither `assign` nor `accumulate` was right.
    let page = rt.block_on(async {
        let mut p = page;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    assert_eq!(
        page.webfonts(),
        (2, 1),
        "a second style round re-declares every @font-face and can re-fetch none of them, so a \
         counter drifts where a keyed set does not: got {:?}",
        page.webfonts()
    );

    // …and a document with no `@font-face` at all reports a real zero, not a phantom. Loaded second
    // on the same thread, which is also where an accumulating counter would leak if it were global.
    let bare = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            "<!doctype html><html><body><p>x</p></body></html>",
            "https://bare.test/",
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    assert_eq!(
        bare.webfonts(),
        (0, 0),
        "a page that declares no @font-face must report (0, 0) — a non-zero here is the previous \
         document's count, and the instrument would then attribute one page's fallback to another"
    );
}
