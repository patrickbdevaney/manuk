//! **G_DETAILS — a closed disclosure hides its contents, and clicking the summary opens it.**
//!
//! `<details>`/`<summary>` is the web's standard "show more", and it is **pure UA behaviour**:
//! GitHub's folded diffs and collapsed review threads, MDN's collapsible sections, every docs
//! site's FAQ. None of them ship a line of script for it — the browser is the whole implementation.
//!
//! Before this, `details` and `summary` appeared **nowhere** in the engine. Both consequences are
//! bad, and the first is worse than it sounds:
//!
//! - every collapsible on the web rendered **permanently expanded**. Not a cosmetic difference: a
//!   page of collapsed sections becomes a wall of everything at once, the summary stops meaning
//!   anything, and a long GitHub thread renders every folded diff inline.
//! - clicking the summary did **nothing**, so a section could never be opened *or* closed — and for
//!   an agent driving the page, "click Show more" was unactionable.
//!
//! ⚠ **This gate exercises the SHIPPING (Stylo) cascade**, so the rule it actually falsifies is the
//! `details > *:not(summary)` pair in `stylo_engine.rs`'s `UA_CSS`. The `MinimalCascade` mirror in
//! `css/src/lib.rs` (`cascade_node`) is kept in lockstep **by convention, not by this test** — the
//! same standing hazard `<dialog>` carries, and the reason both sites say so in a comment. Two
//! cascades disagreeing about whether a section renders is the `<source>` bug all over again.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<details id="shut">
  <summary id="s1">Show the diff</summary>
  <p id="hidden-body">SECRET</p>
</details>
<details id="already" open>
  <summary id="s2">Already open</summary>
  <p id="open-body">VISIBLE</p>
</details>
</body></html>"##;

/// Is this node rendered at all? A `display:none` element produces no box, so it has no rect.
fn is_rendered(page: &manuk_page::Page, sel: &str) -> bool {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    let Some(&n) = hits.first() else {
        return false;
    };
    page.node_rects().get(&n).is_some_and(|r| r.height > 0.0)
}

#[test]
fn a_closed_disclosure_hides_its_body_and_the_summary_toggles_it() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://details.test/", &fonts, 800.0);

    // ── 1. A CLOSED <details> RENDERS ONLY ITS SUMMARY. ───────────────────────────────────────
    assert!(
        is_rendered(&page, "#s1"),
        "G_DETAILS: the summary of a CLOSED <details> must still render — it is the label the user \
         clicks. Hiding it would make the disclosure unopenable."
    );
    assert!(
        !is_rendered(&page, "#hidden-body"),
        "G_DETAILS: the body of a CLOSED <details> rendered. Every collapsible on GitHub and MDN is \
         permanently expanded — a page of folded sections becomes a wall of everything at once."
    );

    // ── 2. `open` RENDERS THE BODY. ───────────────────────────────────────────────────────────
    //
    // Without this the rule above would be indistinguishable from "details never renders its
    // children", which would be an equally broken engine that passes assertion 1.
    assert!(
        is_rendered(&page, "#open-body"),
        "G_DETAILS: the body of a <details open> did NOT render. The `open` attribute is the whole \
         mechanism — hiding the body unconditionally passes the closed-case check while making the \
         element useless."
    );

    // ── 3. CLICKING THE SUMMARY OPENS IT. ─────────────────────────────────────────────────────
    let root = page.dom().root();
    let s1 = manuk_css::query_selector_all(page.dom(), root, "#s1")[0];
    page.dispatch_click(s1, &fonts, 800.0);
    assert!(
        is_rendered(&page, "#hidden-body"),
        "G_DETAILS: clicking the summary did not open the disclosure. This is UA behaviour with no \
         script behind it, so if the engine does not do it, nothing does — and 'click Show more' is \
         unactionable for a user and an agent alike."
    );

    // ── 4. AND CLICKING AGAIN CLOSES IT. ──────────────────────────────────────────────────────
    //
    // A one-way open would pass assertion 3 and still be wrong: the widget is a TOGGLE.
    page.dispatch_click(s1, &fonts, 800.0);
    assert!(
        !is_rendered(&page, "#hidden-body"),
        "G_DETAILS: a second click did not close the disclosure. It is a toggle, not a one-way \
         reveal — a one-way open passes the opening check and still leaves the widget broken."
    );
}
