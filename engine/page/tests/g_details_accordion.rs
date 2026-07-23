//! **G_DETAILS_ACCORDION — a named `<details>` group is EXCLUSIVE: opening one closes the others.**
//!
//! `<details name="…">` is the platform accordion (HTML Living Standard, Baseline 2024). Give a set
//! of disclosures the same `name` and the browser guarantees **at most one is open at a time**:
//! opening one auto-closes whichever sibling in the group was open. FAQ accordions, docs sidebars,
//! GitHub's settings panels and every "only one section expanded" UI now ship this with **no script
//! at all** — the browser is the whole implementation, exactly like the summary-click toggle itself.
//!
//! Before this, a named group behaved like a pile of independent disclosures: clicking a second
//! summary opened it while leaving the first open, so a five-item FAQ could sit fully expanded — the
//! precise "wall of everything at once" the plain `<details>` gate exists to prevent, one level up.
//!
//! Two invariants, and the second is what stops a cheap wrong fix:
//!   1. **Exclusivity** — opening `#b` (same `name` as the open `#a`) closes `#a`, and the `toggle`
//!      event fires on `#a` so a lazy-loaded panel knows it was collapsed.
//!   2. **Scoping by name** — a details with a *different* `name` (`#c`) is NOT in the group and is
//!      unaffected. A fix that closes *all other* open details regardless of name passes invariant 1
//!      and breaks every page with two independent accordions.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<details id="a" name="faq" open><summary id="sa">A</summary><p id="ba">BODY-A</p></details>
<details id="b" name="faq"><summary id="sb">B</summary><p id="bb">BODY-B</p></details>
<details id="c" name="other"><summary id="sc">C</summary><p id="bc">BODY-C</p></details>
<div id="out">-</div>
<script>
  window.__aToggles = 0;
  document.getElementById('a').addEventListener('toggle', function () { window.__aToggles++; });
</script>
</body></html>"##;

/// Rendered at all? A `display:none` element (a closed disclosure's body) produces no box.
fn is_rendered(page: &manuk_page::Page, sel: &str) -> bool {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    let Some(&n) = hits.first() else {
        return false;
    };
    page.node_rects().get(&n).is_some_and(|r| r.height > 0.0)
}

fn click(page: &mut manuk_page::Page, fonts: &FontContext, sel: &str) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
    page.dispatch_click(n, fonts, 800.0);
}

#[test]
fn opening_a_named_details_closes_its_group_siblings_but_not_other_groups() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://accordion.test/", &fonts, 800.0);

    // Baseline: only #a (the one carrying `open`) shows its body.
    assert!(
        is_rendered(&page, "#ba"),
        "G_DETAILS_ACCORDION: <details open> body must render."
    );
    assert!(
        !is_rendered(&page, "#bb"),
        "G_DETAILS_ACCORDION: a closed <details> body must not render."
    );

    // ── 1. EXCLUSIVITY — open #b (same name="faq") and #a must auto-close. ─────────────────────
    click(&mut page, &fonts, "#sb");
    assert!(
        is_rendered(&page, "#bb"),
        "G_DETAILS_ACCORDION: clicking #b's summary did not open it."
    );
    assert!(
        !is_rendered(&page, "#ba"),
        "G_DETAILS_ACCORDION: opening #b left its same-name sibling #a open. A `<details name>` group \
         is EXCLUSIVE — at most one open at a time. Without this, a named FAQ accordion expands into a \
         wall of every section at once, which is the whole reason the group has a name."
    );

    // The auto-closed #a must have fired `toggle` — a lazy panel listens for it to know it collapsed.
    // #a started open and carried no click, so the ONE toggle it can have seen is the exclusivity close.
    page.eval_for_test("document.getElementById('out').textContent = String(window.__aToggles);");
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let toggles = page.dom().text_content(out);
    assert!(
        toggles.trim() == "1",
        "G_DETAILS_ACCORDION: #a fired {toggles:?} toggle events, expected 1. Exclusivity must dispatch \
         `toggle` on the sibling it auto-closes, or a lazy-loaded panel never learns it was collapsed."
    );

    // ── 2. SCOPING — #c has a DIFFERENT name; it is not in the group. ──────────────────────────
    click(&mut page, &fonts, "#sc");
    assert!(
        is_rendered(&page, "#bc"),
        "G_DETAILS_ACCORDION: clicking #c's summary did not open it."
    );
    assert!(
        is_rendered(&page, "#bb"),
        "G_DETAILS_ACCORDION: opening #c (name=\"other\") closed #b (name=\"faq\"). Exclusivity is \
         scoped BY NAME — closing every other open details regardless of name breaks any page with two \
         independent accordions."
    );
}
