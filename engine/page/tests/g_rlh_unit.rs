//! **G_RLH_UNIT — `rlh` resolved against neither the root NOR the element, but against the INITIAL
//! line-height, and the map called the capability `works` for 212 ticks.**
//!
//! `rem` is root-relative and this engine has always known it: the cascade calls
//! `Device::set_root_font_size` the moment `<html>` is cascaded, with a comment explaining that
//! `html{font-size:62.5%}` — the "1rem = 10px" idiom half the web is built on — silently breaks
//! without it. **`rlh` is root-relative in exactly the same way and nothing set it.** Stylo's own
//! `matching.rs` sets the two together, four lines apart, under *"Update root font size for rem
//! units"* and *"Update root line height for rlh units"*. We had the first.
//!
//! Chrome-measured (surface audit #44, t721), root `line-height:2` on `16px`, element
//! `line-height:20px`:
//!
//! ```text
//!                       CHROME    MANUK before    MANUK after
//!    width:  5lh          100         100            100
//!    height: 5lh          100         100            100
//!    width:  5rlh         160          96            160
//!    height: 5rlh         160          96            160
//! ```
//!
//! `96 = 5 × 19.2 = 5 × (16 × 1.2)` — the **initial `normal` line-height**. Not the root's `32px`,
//! not the element's `20px`: initial-relative, which is the one answer no author can predict and the
//! one that looks plausible in a screenshot.
//!
//! ⚠⚠ **The ledger said `works` from tick 509 to tick 721**, because the probe behind that row tests
//! `width:5lh` and *nothing else* — and its own receipt says so: *"`rlh` is the root-relative sibling
//! on the identical Stylo path … **not separately geometry-tested**"*. The untested half was the
//! broken half. **A probe that tests one property has measured one property**; the row's name did the
//! over-claiming, not the probe.
//!
//! ⚠ Named residue: `Device::calc_line_height` in this servo build returns **0** for
//! `line-height: normal` (*"TODO: compute `normal` from the font metrics"*), so a root that never
//! states a line-height leaves `rlh` at zero. That is honest and it is not a regression — the value
//! it replaces was wrong for *every* root, including that one. Asserted below so it cannot drift
//! silently.
//!
//! ⚠ Separate, still open, deliberately NOT asserted as working here: `CSS.supports('width','5lh')`
//! answers **false** in this engine and **true** in Chrome, for a unit that demonstrably works — a
//! false *negative*, the mirror of this project's usual false-presence hazard, so a page guarding
//! `lh` takes its fallback for no reason. `lh`/`rlh` reached Baseline **Widely Available** in May
//! 2026, which is when authors stop guarding.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
 html { font: 16px/2 sans-serif }
 body { margin:0; font: 16px/1.5 sans-serif; width: 800px }
 #w1 { line-height:20px; width:  5lh  }
 #w2 { line-height:20px; width:  5rlh }
 #h1 { line-height:20px; height: 5lh  }
 #h2 { line-height:20px; height: 5rlh }
 /* A root with NO line-height: the named residue. Its own subtree is measured in the second doc. */
</style></head><body>
 <div id="w1">w</div><div id="w2">w</div><div id="h1">h</div><div id="h2">h</div>
</body></html>"#;

/// Same document with a root that never states a line-height — `Device::calc_line_height` returns 0
/// for `normal` in this build, so `rlh` is 0 here. Pinned, not celebrated.
const HTML_NORMAL_ROOT: &str = r#"<!doctype html><html><head><style>
 body { margin:0; width: 800px }
 #r { width: 5rlh }
</style></head><body><div id="r">r</div></body></html>"#;

fn rect(page: &manuk_page::Page, sel: &str) -> [f32; 4] {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    let rects = page.root_box.node_rects(page.dom());
    let r = rects
        .get(&hits[0])
        .unwrap_or_else(|| panic!("{sel} has no box"));
    [r.x, r.y, r.width, r.height]
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn rlh_resolves_against_the_root_line_height_not_the_initial_one() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rlh.test/", &fonts, 800.0);

    let w1 = rect(&page, "#w1")[2];
    let w2 = rect(&page, "#w2")[2];
    let h1 = rect(&page, "#h1")[3];
    let h2 = rect(&page, "#h2")[3];
    println!("RLH  w1(5lh)={w1} w2(5rlh)={w2} h1(5lh)={h1} h2(5rlh)={h2}");

    // (1) **`lh` is the element's line-height** — the half that already worked, asserted as the
    // over-correction guard: a fix that pointed BOTH units at the root would break this and pass
    // everything else. RED: make `lh` root-relative -> 160.
    assert!(
        (w1 - 100.0).abs() < 1.0,
        "width:5lh must be 5 x the ELEMENT's 20px line-height = 100 — got {w1}"
    );
    assert!(
        (h1 - 100.0).abs() < 1.0,
        "height:5lh must be 100 — got {h1}"
    );

    // (2) **`rlh` is the ROOT's line-height** — 5 x (16 x 2) = 160. RED: delete the
    // `set_root_line_height` call in `cascade_via_stylo` -> 96, the initial `normal` line-height.
    // Chrome measures 160 on this exact fixture.
    assert!(
        (w2 - 160.0).abs() < 1.0,
        "width:5rlh must be 5 x the ROOT's 32px line-height = 160, not the element's (100) and not \
         the initial normal (96) — got {w2}"
    );
    assert!(
        (h2 - 160.0).abs() < 1.0,
        "height:5rlh must be 160 — got {h2}. A unit that is right for `width` and wrong for \
         `height` is how this bug survived: the probe that declared it `works` tested only `width`."
    );

    // (3) **THE NAMED RESIDUE, PINNED.** `line-height: normal` on the root yields 0 in this servo
    // build, so `5rlh` is 0 and the block falls back to its auto width (the 800px container). If
    // this ever reads 96, the initial-relative behaviour is back; if it reads a real multiple of the
    // font's normal line-height, `calc_line_height` learned to compute `normal` and this assertion
    // should be replaced with the Chrome value.
    let page2 = manuk_page::Page::load(HTML_NORMAL_ROOT, "https://rlh.test/", &fonts, 800.0);
    let r = rect(&page2, "#r")[2];
    println!("RLH  normal-root 5rlh width={r}");
    assert!(
        r < 1.0 || (r - 800.0).abs() < 1.0,
        "with `line-height: normal` on the root, this build's calc_line_height returns 0, so 5rlh \
         is 0 — got {r}. A value near 96 means the initial-relative bug is back."
    );
}
