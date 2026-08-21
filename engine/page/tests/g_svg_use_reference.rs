//! **G_SVG_USE_REFERENCE — the icon-sprite idiom: a `<use href="#icon">` naming a `<symbol>` in
//! another `<svg>` resolved to NOTHING. The icon was not misplaced; it was never drawn.**
//!
//! Both columns are measured on **this exact fixture** — Chrome headless on the `HTML` below with a
//! probe appended, and BEFORE from the same string on the parent commit. Not remembered, and not
//! adjusted by arithmetic from a nearly-identical page:
//!
//! ```text
//!                                              CHROME        BEFORE       AFTER
//!   <use> of a <symbol> (24-unit vb at 20px)   3,25 13x10    0,20 0x19    3,25 13x10
//!   <use> of a <defs> path, x/y offset         3,43 17x17    0,40 0x19    3,43 17x17
//!   <use> of a multi-shape <g>                 1,105 15x6    0,104 0x19   1,105 15x6
//!   <use> beside an ordinary <rect>            24,84 10x10   NO-BOX       24,84 10x10
//!   a DANGLING <use href="#nope">              0,104 0x0     0,104 0x19   0,104 0x0
//!   same-<svg> <use>, and xlink:href           2,116 8x8     0,114 0x19   2,116 8x8
//!                                              20,120 8x8    0,114 0x19   20,120 8x8
//!   ── unchanged controls ──────────────────────────────────────────────────────────
//!   the plain <rect> sharing that svg          0,80 24x24    0,80 24x24   0,80 24x24
//!   the sprite host's own <rect>               0,0 10x10     0,0 10x10    0,0 10x10
//!   <symbol>/<defs> content, svg MAPPED        0,0 0x0       NO-BOX       NO-BOX
//!   <symbol>/<defs> content, svg REFUSED       0,0 0x0       0,114 0x19   NO-BOX
//! ```
//!
//! AFTER is Chrome-identical on **every row but the last two** — the `<symbol>`/`<defs>` content,
//! which is the named residue below and predates this change. Its two BEFORE values are worth
//! keeping apart: in a `<svg>` this pass mapped, the content was already absent; in one it refused,
//! the CSS inline box stood. Same element, same markup, two different wrong answers depending on
//! whether something *else* in the subtree happened to pair — which is what a whole-`<svg>` refusal
//! does, and the reason the refusal is worth this much care.
//!
//! ## Two bugs, one reference model
//!
//! ⚠⚠ **The first is not a geometry bug at all.** `Page::decode_inline_svgs` serialises **one
//! `<svg>` element** and hands that string to usvg — and the sprite idiom puts the `<symbol>` sheet
//! in one `<svg>` and the `<use>` in another. So every such reference named an id that was not in
//! the document usvg was given. usvg drops an unresolvable `<use>`, which means **the icon did not
//! rasterise**: not a wrong box, an absent drawing, on the markup pattern that ships most of the
//! icons on the web. Every geometry number underneath it was measuring a blank. The fix injects the
//! definitions the subtree reaches outside itself, wrapped in a `<defs>` so they stay invisible.
//!
//! The second is the pairing. `svg_geometry` matches usvg's rendered leaves against the DOM's shape
//! elements **by count**, and refuses the whole `<svg>` when the counts disagree — deliberately,
//! because a mis-paired shape reports one element's bounds on another. Once `<use>` resolves, usvg
//! **expands** it: a `<use>` naming a one-path symbol emits one leaf, a two-shape `<g>` emits two,
//! and a dangling one emits none, while the DOM walk saw an element with no element children and
//! counted zero for all three. So the fix resolves the reference in the DOM the way the spec
//! defines it and counts the leaves the target will contribute — the pairing stays exact rather
//! than becoming a heuristic — and a `<use>`'s box is the **union** of its run, which is the box
//! Chrome returns for a multi-shape reference.
//!
//! ⚠ **NAMED RESIDUE, unchanged by this tick and not smoothed over:** `<symbol>`/`<defs>` content
//! gets `0x0` in Chrome and no box here. That is the non-rendered-container skip, which predates
//! this change and is its own mechanism; the `<use>` zero-box below is carried *because* it is part
//! of this one. An external-file reference (`icons.svg#check`) is a fetch this pass does not do and
//! reads as dangling.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | drop the `external_use_defs` injection | RED — every `<use>` back to `0x19` **and `c-rect` with them**, see below |
//! | drop `use_leaf_count` (`<use>` as an ordinary container) | RED — identical output to the above |
//! | count a `<use>` as exactly ONE leaf (the obvious guess) | RED — `u4=1,105 6x6`, `u5=10,105 6x6`: see below |
//! | drop the `zero` list (let a dangling `<use>` vanish) | RED — `u5=NO-BOX`, a wrong box traded for a missing one |
//!
//! ⚠⚠ **The first two mutations produce the SAME output, and each half alone is a REGRESSION.**
//! With the injection but not the counting, usvg emits leaves the DOM walk does not count; with the
//! counting but not the injection, the DOM walk counts leaves usvg never emitted. Either way the
//! pairing guard fires and refuses the **whole** `<svg>` — so `c-rect`, an ordinary `<rect>` with
//! nothing wrong with it, goes from `0,80 24x24` to `0,80 0x19` because of the `<use>` beside it.
//! Two changes, one behaviour; landing half of this would have made a sprite-using page worse.
//!
//! ⚠⚠ **And the third mutation is the reason this module refuses rather than guesses.** "One leaf
//! per `<use>`" is the obvious implementation, it passes four of the six assertions, and what it
//! does to the two it fails is the point: `u4` reports only the first of the two shapes it
//! references, and the **dangling** `u5` is handed the *circle's* bounds — a real box, plausibly
//! sized, belonging to a different element. A silently mis-attributed box is worse than the honest
//! `0x19` it replaced, which is exactly what the count-pairing guard exists to prevent.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<svg id="sprite" width="60" height="20" style="display:block">
  <symbol id="sym-check" viewBox="0 0 24 24"><path id="sym-path" d="M4 12 L10 18 L20 6"/></symbol>
  <defs>
    <path id="def-path" d="M0 0 H10 V10 H0 Z"/>
    <g id="def-g"><rect id="dg-r" x="0" y="0" width="6" height="6"/><circle id="dg-c" cx="12" cy="3" r="3"/></g>
  </defs>
  <rect id="sprite-vis" x="0" y="0" width="10" height="10"/>
</svg>
<svg id="a" width="20" height="20" viewBox="0 0 24 24" style="display:block"><use id="u1" href="#sym-check"/></svg>
<svg id="b" width="40" height="40" viewBox="0 0 24 24" style="display:block"><use id="u2" href="#def-path" x="2" y="2"/></svg>
<svg id="c" width="48" height="24" viewBox="0 0 48 24" style="display:block">
  <rect id="c-rect" x="0" y="0" width="24" height="24" fill="#ccc"/>
  <use id="u3" href="#def-path" x="24" y="4"/>
</svg>
<svg id="d" width="30" height="10" viewBox="0 0 30 10" style="display:block">
  <use id="u4" href="#def-g" x="1" y="1"/>
  <use id="u5" href="#nope"/>
</svg>
<svg id="e" width="40" height="20" viewBox="0 0 40 20" style="display:block">
  <defs><rect id="e-def" x="0" y="0" width="8" height="8"/></defs>
  <use id="u6" href="#e-def" x="2" y="2"/>
  <use id="u7" xlink:href="#e-def" x="20" y="6"/>
</svg>
</body></html>"##;

/// `id=x,y WxH` for each id, read from the layout tree the painter and the fidelity sweep both read.
fn boxes(page: &manuk_page::Page, ids: &[&str]) -> String {
    let rects = page.root_box.node_rects(page.dom());
    let root = page.dom().root();
    ids.iter()
        .map(|id| {
            let hits = manuk_css::query_selector_all(page.dom(), root, &format!("#{id}"));
            match hits.first().and_then(|n| rects.get(n)) {
                Some(r) => format!(
                    "{id}={},{} {}x{}",
                    r.x.round(),
                    r.y.round(),
                    r.width.round(),
                    r.height.round()
                ),
                None => format!("{id}=NO-BOX"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_use_reference_reports_the_geometry_of_what_it_references() {
    // ⚠⚠ **MERGED INTO ONE `#[test]` DELIBERATELY (t1342) — DO NOT SPLIT THIS BACK OUT.**
    //
    // `libtest` spawns a thread per test, including at `--test-threads=1`, and SpiderMonkey allows
    // exactly one JS thread per process: a second one silently runs no script, or SIGSEGVs outright
    // if the first is still alive. Two `#[test]`s in a `Page`-building binary therefore means at most
    // one of them was ever really checked. See `docs/wiki/js-engine.md` and
    // `g_one_js_thread_per_process.rs`. Enforced by `G_ONE_PAGE_TEST_PER_BINARY`.
    a_use_reference_actually_rasterises();
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // ⚠ `finish_loading` for the same reason `g_svg_auto_sizing` needs it: the inline-svg decode
    // that feeds this mapping runs in the SUBRESOURCE pass, so a gate that reads at load-event time
    // is reading a state the mapping has not reached. An observation has a TIME (t742).
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://svguse.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let got = boxes(
        &page,
        &[
            "sprite",
            "sprite-vis",
            "sym-path",
            "def-path",
            "dg-r",
            "a",
            "u1",
            "b",
            "u2",
            "c",
            "c-rect",
            "u3",
            "d",
            "u4",
            "u5",
            "e",
            "e-def",
            "u6",
            "u7",
        ],
    );
    println!("SVG-USE {got}");
    let has = |s: &str| got.contains(s);

    // (1) **THE ICON.** A `<use>` of a `<symbol>` reports the referenced path's bounds through the
    // symbol's own viewBox and the host svg's scale — Chrome `3,25 13x10` for a 24-unit icon drawn
    // at 20px. RED: restore `<use>` as an ordinary container → NO-BOX.
    assert!(
        has("u1=3,25 13x10"),
        "a <use> of a <symbol> must report the referenced geometry — got {got:?}"
    );

    // (2) **The x/y offset and the scale both apply**, and they are separable: `#b` draws the same
    // 10-unit path at 40/24 with a (2,2) shift.
    assert!(
        has("u2=3,43 17x17"),
        "a <use>'s x/y offset must translate the referenced bounds before the viewBox scale — \
         got {got:?}"
    );

    // (3) **A `<use>` COEXISTING WITH AN ORDINARY SHAPE — and this row is the one that proves the
    // two halves compose.** Before, `#c` mapped *successfully* and `u3` was simply absent: the
    // `<use>` was dangling for usvg too, so one DOM shape met one usvg leaf and the pairing
    // balanced by both sides being blind to the icon. The `<rect>` must keep the geometry it
    // already had while the `<use>` gains its own — a fix that only added leaves would break the
    // balance and take the `<rect>` down with it.
    assert!(
        has("c-rect=0,80 24x24") && has("u3=24,84 10x10"),
        "a plain <rect> sharing an <svg> with a <use> must keep its own geometry while the <use> \
         gains one — got {got:?}"
    );

    // (4) **A `<use>` is not one leaf.** `#def-g` holds a rect AND a circle, so the expansion emits
    // two leaves and the `<use>`'s box is their UNION (x 0..15, y 0..6, shifted by 1,1). RED: count
    // every `<use>` as exactly one leaf — the obvious guess — and `#d` mis-pairs and refuses.
    assert!(
        has("u4=1,105 15x6"),
        "a <use> of a multi-shape <g> is the UNION of the shapes it expands to — got {got:?}"
    );

    // (5) **THE CONTROL AGAINST OVER-CORRECTION, and it is the one a count-based fix gets wrong.**
    // A dangling `<use href="#nope">` renders nothing and contributes NO leaf. Assuming one leaf
    // per `<use>` desynchronises the pairing for `#d` and takes `u4` down with it.
    //
    // ⚠ **And it still has a BOX** — Chrome `0,104 0x0`, zero-area at the svg's origin. The first
    // version of this dropped it, which read as a tidy result and was a MISSING_BOX where the page
    // used to have a (wrong) one; the ledger ranks missing as the worse of the two, so the
    // zero-area box is carried deliberately. RED: drop the `zero` list → `u5=NO-BOX`.
    assert!(
        has("u5=0,104 0x0"),
        "a dangling <use> contributes no leaf and still reports a zero-area box at the svg's \
         origin, Chrome-measured — got {got:?}"
    );

    // (6) **THE SPRITE HOST STILL WORKS.** `#sprite` is a visible svg whose only rendered child is
    // one `<rect>`; `<symbol>`/`<defs>` contribute nothing. This is the control that the
    // non-rendered-container skip still lines up with usvg after the `<use>` change.
    assert!(
        has("sprite=0,0 60x20") && has("sprite-vis=0,0 10x10"),
        "a sprite host svg keeps its own rendered children — got {got:?}"
    );

    // (7) **THE OTHER COMMON SHAPE — `<defs>` and `<use>` in the SAME `<svg>`, and `xlink:href`.**
    // Every other `<use>` here is cross-`<svg>`, which is the case the injection exists for; this
    // one must work without it, and must not be injected *twice* (a duplicate id in front of usvg
    // makes the resolver pick one arbitrarily). Two `<use>`s of one definition also prove the
    // target is not consumed by the first reference. `xlink:href` is the legacy spelling still on
    // most shipped sprite sheets.
    assert!(
        has("u6=2,116 8x8") && has("u7=20,120 8x8"),
        "a <use> of a definition in its OWN <svg> must resolve without injection, twice over, and \
         through the legacy xlink:href spelling — got {got:?}"
    );

    // (8) **`<symbol>`/`<defs>` CONTENT GETS NO BOX.** Chrome says `0x0`; we say absent. The
    // difference is named in the module header rather than smoothed over — what matters here is
    // that it is no longer the `0x19` inline box, which is a wrong number of the right shape.
    assert!(
        has("sym-path=NO-BOX") && has("def-path=NO-BOX") && has("dg-r=NO-BOX"),
        "non-rendered <symbol>/<defs> content must not get a CSS inline box — got {got:?}"
    );
}

/// ⚠⚠ **THE HEADLINE CLAIM, ASSERTED ON PIXELS RATHER THAN INFERRED FROM usvg's SEMANTICS.**
///
/// "usvg drops an unresolvable `<use>`, so the icon never rasterised" is a statement about a
/// dependency's behaviour, and the whole tick rests on it. Reasoning about it is not measuring it:
/// `g_first_paint`'s own svg case exists because *"a decoded image that never reaches the display
/// list"* is a different failure from a decode that returned `None`, and either one would produce
/// the same absent icon. So this samples the centre of a `<use>`-drawn icon and requires the
/// referenced fill colour to be there.
///
/// RED-proven: drop the `external_use_defs` injection → `rgb(255,255,255)`, a blank square where
/// the icon should be. That is what every sprite-shipped icon on the web painted as.
fn a_use_reference_actually_rasterises() {
    let fonts = FontContext::new();
    let html = r##"<!doctype html><body style="margin:0">
      <svg width="0" height="0" style="display:none">
        <symbol id="ic" viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="#ff0000"/></symbol>
      </svg>
      <svg viewBox="0 0 10 10" style="width:100px;height:100px;display:block"><use href="#ic"/></svg>
    </body>"##;
    let page = manuk_page::Page::load(html, "https://svguse.test/", &fonts, 200.0);
    let canvas = page.paint(&fonts, 200, 200);
    let px = canvas.rgba_bytes();
    // The centre of the 100×100 svg box.
    let (w, x, y) = (200usize, 50usize, 50usize);
    let i = (y * w + x) * 4;
    let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
    println!("SVG-USE-PAINT rgb({r},{g},{b})");
    assert!(
        r > 200 && g < 60 && b < 60,
        "a <use> of a <symbol> defined in ANOTHER <svg> — the sprite idiom — must paint the \
         referenced fill, got rgb({r},{g},{b}). The reference is dangling in the one-element \
         serialization handed to usvg, so the icon is not drawn at all"
    );
}
