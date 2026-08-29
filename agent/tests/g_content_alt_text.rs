//! **G_CONTENT_ALT_TEXT — one `content` declaration says two things, and this engine PAINTED the
//! half meant to be announced.**
//!
//! CSS Content 3 lets a single declaration carry both: everything before the `/` is **drawn**, and
//! everything after it is what a screen reader **announces**.
//!
//! ```css
//!   ::before { content: "★" / "" }              /* draw a star, announce nothing */
//!   ::before { content: "" / counter(step) }    /* announce a step number that is not drawn */
//! ```
//!
//! Both directions were wrong here, for two different reasons on the two cascades.
//!
//! ## ⭐⭐⭐ THE FOURTH PROPERTY FOUND SWITCHED OFF — AND THE FIRST ONE `longhands.toml` COULD NOT NAME
//!
//! Stylo parses the syntax, but the `/` arm of its value parser carries
//! `static_prefs::pref!("layout.css.content.alt-text.enabled")` — **inside the parser, not in
//! `longhands.toml`**. With the pref off the whole declaration is an unexpected-token error, so the
//! author's fallback line wins (and where there is no fallback, the pseudo vanishes).
//!
//! t1358's rule was *"when a CSS feature looks absent, read `longhands.toml` for a `servo_pref`
//! before concluding anything"*. That rule could not have found this one. **It generalises: the
//! gates are `static_prefs::pref!` call sites ANYWHERE in stylo, and a sweep of the crate finds 53
//! of them — this engine flips six.**
//!
//! ⚠⚠ **And the sweep's second result is what keeps the rule honest.** Of the other 47,
//! `system-ui` (13 of 39 corpus sites) and `-webkit-fill-available` (3/39) were measured against
//! Chrome and are **already correct with their prefs off**:
//!
//! ```text
//!   font-family: system-ui, "Hamburgefonstiv"   Chrome 131.61   ours 131.61
//!   font-family: monospace  (control)           Chrome 144.50   ours 144.50
//!   width: -webkit-fill-available in a 400px CB Chrome 400.00   ours 400.00
//! ```
//!
//! **An unflipped pref is not evidence that a feature is broken** — it is a place to look. Only the
//! row with a measured divergence was flipped.
//!
//! ## The other cascade did not parse it at all
//!
//! `MinimalCascade` handed the whole value to `parse_content_parts`, so `"before" / "alt"` rendered
//! as `beforealt`. The two cascades disagreeing about what a pseudo paints is the `<source>` bug
//! this project keeps re-finding (t1361 `font-size`/`line-height`, t1364 `border-spacing`), so both
//! are fixed here and this gate runs on the one the wall can see.
//!
//! ⚠ The split is at a `/` **outside a quoted string**: `content: "and/or" / "and or"` has two, and
//! taking the first renders `"and` and announces `or" / "and or"`.
//!
//! ## Chrome-measured (`16px/24px monospace`, 400px block)
//!
//! ```text
//!   content: "before" / ""                 renders "beforelabel"   105.97   <- ours was 48.17
//!   content: "before" / "alt"              renders "beforelabel"   105.97
//!   content: "and/or" / "x"                renders "and/orlabel"   105.97
//!   content: "plain"   (no slash)          renders "plainlabel"     96.34   CONTROL
//!   no ::before at all                     renders "label"          48.17   CONTROL
//! ```
//!
//! ⚠ **NAMED, MEASURED, NOT BUILT — the alt text is not yet in the accessible NAME.** This tick
//! makes the two halves *separable*; threading the alt half into `accessible_name` (the t1365
//! plumbing, whose own note said a fourth fact should become a context struct) is the next tick.
//! `accname` is flat at 432/484 across this change, deliberately.
//!
//! ⚠ **NAMED, MEASURED, NOT BUILT — white space at the pseudo/text boundary.** The WPT fixture
//! writes `content: " before "` with deliberate outer spaces, and Chrome collapses them against the
//! adjacent text: `" before " + "label" + " after "` is **18** characters wide in Chrome and **20**
//! here (173.4 vs 192.7). Exactly two spaces, and a separate mechanism from this one.
//!
//! PROVEN RED by three mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
.w{width:400px;margin:0 0 8px 0}
#e1::before{content:"before" / "";}
#e2::before{content:"before" / "alt";}
#e3::before{content:"and/or" / "x";}
#e4::before{content:"plain";}
</style></head><body>
<div class="w" id="c1"><span id="e1">label</span></div>
<div class="w" id="c2"><span id="e2">label</span></div>
<div class="w" id="c3"><span id="e3">label</span></div>
<div class="w" id="c4"><span id="e4">label</span></div>
<div class="w" id="c5"><span id="e5">label</span></div>
</body></html>"##;

fn by_id(page: &manuk_page::Page, id: &str) -> NodeId {
    let dom = page.dom();
    dom.descendants(dom.root())
        .find(|&n| {
            dom.element(n)
                .and_then(|e| e.attr("id"))
                .is_some_and(|v| v == id)
        })
        .unwrap_or_else(|| panic!("VACUOUS: no element with id={id:?}"))
}

fn width(page: &manuk_page::Page, id: &str) -> f32 {
    page.root_box
        .node_rects(page.dom())
        .get(&by_id(page, id))
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
        .width
}

#[test]
fn the_content_alt_half_is_announced_not_painted() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cat.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.5;

    // ── VACUITY. The bare `label` span must be the narrowest of the five, or the ::before content
    // is not reaching layout at all and every row below is the same measurement five times.
    let bare = width(&page, "e5");
    assert!(
        near(bare, 48.17),
        "VACUOUS: the control span with no ::before measures {bare}, not 48.2 — the fixture is not \
         laying out as expected and nothing below is a test of `content`"
    );

    // (id, Chrome width, what the row is for)
    let rows: &[(&str, f32, &str)] = &[
        ("e1", 105.97, "an EMPTY alt (`/ \"\"`) still renders the drawn half — reading 48.17 means the whole declaration was refused"),
        ("e2", 105.97, "a non-empty alt is not painted — reading ~135 means the alt was drawn too"),
        ("e3", 105.97, "the split is at a `/` OUTSIDE the quotes: `\"and/or\"` keeps its slash, so this is the SAME width as e1/e2 and a naive split('/') renders `and` and reads ~77"),
        ("e4", 96.34, "CONTROL — a declaration with no `/` is unaffected"),
        ("e5", 48.17, "CONTROL — no ::before at all"),
    ];
    for (id, want, why) in rows {
        let got = width(&page, id);
        assert!(
            near(got, *want),
            "G_CONTENT_ALT_TEXT #{id}: Chrome renders this {want} wide, got {got}.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `split_content_alt` returns `(v, None)` always (MinimalCascade's pre-tick behaviour)
//       -> e1 reads 96.3 ("before" painted, the empty alt ignored — it happens to look right),
//          e2 reads ~135 ("beforealt"), e3 reads ~154 unchanged. The e2 row is the one that fires.
// N2  split at the FIRST `/` regardless of quoting
//       -> e3 renders `and` and reads ~77 instead of 105.97. This is the only row that separates a
//          correct split from a naive `v.split('/')`.
// N3  drop the `layout.css.content.alt-text.enabled` pref flip in `cascade_via_stylo`
//       -> the STYLO path stops parsing the declaration entirely. ⚠ Not observable from THIS gate,
//          which runs on MinimalCascade (`manuk-agent` takes `manuk-page` with default features).
//          Measured directly instead, and recorded rather than dressed up: with the pref off,
//          `content: "before" / ""` renders 48.17 on the stylo path where Chrome renders 105.97.
//          That asymmetry is surface audit #78's finding — no wall runs a Stylo-path gate.
