//! **G_HAS_CASCADE_ORDER — `:has()` rules keep their cascade order ACROSS stylesheets.**
//!
//! Stylo's servo build discards `:has()` rules at parse (`parse_has() -> false`), so they are recovered
//! by a supplement that runs our own selector engine as a second pass. **13% of the corpus uses
//! `:has()`**, so this is not an edge.
//!
//! The supplement used to walk, *per element*, every rule of every `:has()`-carrying sheet —
//! re-evaluating each rule's `@media` and re-asking each selector whether it was relative, work whose
//! answer cannot change between elements. Tick 580 hoisted that out into one pass per cascade
//! (`collect_relative_rules`). The hoist is a pure performance change and must therefore be **provably
//! behaviour-preserving**, and its one real hazard is *ordering*: source order was previously implicit
//! in "sheet by sheet, rule by rule", and is now an explicit `order` number that has to reproduce it.
//!
//! So this gate asserts the thing the refactor could break and nothing else could catch:
//!
//! 1. **A later sheet wins at equal specificity.** Two sheets both give `.box:has(> img)` a colour; the
//!    second must win. The fixture places sheet 1's competing rule at within-sheet index **3** and
//!    sheet 2's at index **0**, deliberately: drop the per-sheet stride on `order` and sheet 2's 0 sorts
//!    before sheet 1's 3, so the *earlier* sheet wins. **The first version of this fixture put both at
//!    index 0 and could not detect that at all** — a stable sort preserves emission order and hid the
//!    defect, so the RED patch left the gate green. An assertion whose fixture cannot reach the
//!    mechanism is green for a reason unrelated to the claim (the t573 lesson, met again while writing
//!    the gate meant to catch it).
//! 2. **Specificity still outranks source order.** A more specific `:has()` rule in the FIRST sheet must
//!    beat a weaker one in the second — the guard against "fix ordering by making it purely positional".
//! 3. **`!important` still outranks both.**
//! 4. **The rules still actually apply, and only to matching elements** — the control, without which
//!    every assertion above could pass on a page where `:has()` does nothing at all.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head>
<style>
  /* SHEET 1 — the competing rule is deliberately at a HIGH within-sheet index. */
  .filler-a { margin: 0 }
  .filler-b { padding: 0 }
  .filler-c { border: 0 }
  .box:has(> img) { color: rgb(1, 0, 0); }      /* rule index 3 */
  #sp.box:has(> img) { color: rgb(0, 0, 1); }   /* higher specificity, EARLIER sheet */
  .imp:has(> img) { color: rgb(9, 9, 9) !important; }
</style>
<style>
  /* SHEET 2 — later in document order, and its competing rule is at index 0. Without a per-sheet
     stride on `order`, sheet 2's index 0 sorts BEFORE sheet 1's index 3 and the EARLIER sheet wins,
     which is the bug. Equal indices would not detect it: a stable sort preserves emission order and
     hides the defect, which is exactly what the first version of this fixture did. */
  .box:has(> img) { color: rgb(2, 0, 0); }      /* rule index 0 */
  #sp.box { color: rgb(50, 50, 50); }
  .imp:has(> img) { color: rgb(7, 7, 7); }
</style>
</head><body>
<div class="box" id="later"><img src="x.png"></div>
<div class="box" id="sp"><img src="x.png"></div>
<div class="box imp" id="imp"><img src="x.png"></div>
<div class="box" id="nomatch"><span>no img here</span></div>
</body></html>"##;

fn rgb(page: &manuk_page::Page, sel: &str) -> (u8, u8, u8) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let c = page
        .styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"))
        .color;
    (c.r, c.g, c.b)
}

#[test]
fn has_rules_keep_their_cascade_order_across_sheets() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://has.test/", &fonts, 800.0);

    // 4. THE CONTROL FIRST: if `:has()` applies to nothing, every ordering claim below is vacuous.
    assert_ne!(
        rgb(&page, "#later"),
        (0, 0, 0),
        "G_HAS_CASCADE_ORDER: the `:has()` supplement applied NOTHING — Stylo discards these rules at \
         parse, so with the supplement broken every assertion in this gate would pass vacuously"
    );
    assert_eq!(
        rgb(&page, "#nomatch"),
        (0, 0, 0),
        "G_HAS_CASCADE_ORDER: `.box:has(> img)` must NOT match a `.box` with no `<img>` child — a \
         supplement that applies its rules to everything would satisfy claim 1 and be far worse"
    );

    // 1. Later sheet wins at equal specificity.
    assert_eq!(
        rgb(&page, "#later"),
        (2, 0, 0),
        "G_HAS_CASCADE_ORDER: two sheets give `.box:has(> img)` the same specificity, so the LATER \
         one wins. Source order used to be implicit in the sheet-by-sheet walk; hoisting the \
         collection out of the per-element loop made it an explicit number, and if that number does \
         not carry a per-sheet stride then rule 0 of sheet 1 ties with rule 0 of sheet 2"
    );

    // 2. Specificity still beats source order — including across sheets, in the losing direction.
    assert_eq!(
        rgb(&page, "#sp"),
        (0, 0, 1),
        "G_HAS_CASCADE_ORDER: `#sp.box:has(> img)` is in the EARLIER sheet but is more specific, so \
         it must beat the later `.box:has(> img)`. This is the guard against repairing order by \
         making the sort purely positional"
    );

    // 3. `!important` still outranks both.
    assert_eq!(
        rgb(&page, "#imp"),
        (9, 9, 9),
        "G_HAS_CASCADE_ORDER: an `!important` `:has()` declaration in the earlier sheet must beat the \
         normal one in the later sheet"
    );
}
