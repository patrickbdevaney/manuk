//! **G_CASCADE_LOGICAL_PHYSICAL — `margin-left` and `margin-inline-start` are ONE property group
//! in the cascade, and the winner is decided by cascade priority, not by which spelling it used.**
//!
//! A logical property and its physical counterpart are separate `LonghandId`s that resolve to the
//! **same physical side**. CSS Logical Properties §Cascading says they cascade *together*: the
//! declaration that wins on (origin, layer, specificity, order) sets the side, whichever way it is
//! spelled.
//!
//! **Ours resolved every such conflict backwards — the LOWEST-priority declaration won, 7 of 7.**
//! Measured against Chrome, `margin-left` read out of `getComputedStyle` (`/tmp/logical3.html`,
//! tick 998):
//!
//! ```text
//!                                                                    Chrome    ours
//!   #r1 { margin-inline: 20px } #r1 { margin: 0 }                    ml=0      ml=20
//!   #r2 { margin: 0 } #r2 { margin-inline: 20px }                    ml=20     ml=0
//!   #r3 { margin-inline-start: 20px } #r3 { margin-left: 5px }       ml=5      ml=20
//!   #r5.t { margin-left: 5px } #r5 { margin-inline-start: 20px }     ml=5      ml=20
//! ```
//!
//! ## Why this is a reset bug, not a writing-modes bug
//!
//! `* { margin: 0 }` and `* { padding: 0 }` are the first two lines of Tailwind's preflight, of
//! Normalize, and of every hand-rolled reset since 2004 — and **any rule spelled logically was
//! immune to them.** Every RTL-aware design system emits `margin-inline-*` / `padding-inline-*` on
//! LTR pages too, because that is what the shorthand compiles to. So the reset lost to exactly the
//! thing it exists to remove, on a left-to-right page, with no vertical writing mode anywhere.
//!
//! It is also the reason `<fieldset>`'s UA margin could not be written logically at tick 996: the
//! defect was found while writing a UA rule from scratch, which is not something this loop does
//! often.
//!
//! ## The mechanism, and why `push` could not see it
//!
//! `stylo_engine.rs` merges every winning rule's declarations into ONE
//! `PropertyDeclarationBlock` and hands it to `Stylist::compute_for_declarations`. That entry point
//! iterates the block **forward** (not the `next_back()` walk the rule-tree path uses) and
//! `Cascade::apply_one_longhand` is **first-seen-wins on the id AFTER `to_physical()`**. So the
//! block must be built highest-priority-first.
//!
//! It was built ascending, and that was right for every property whose two declarations share a
//! `LonghandId` — purely because `PropertyDeclarationBlock::push` de-duplicates on `id()` and moves
//! the newcomer to the end, collapsing them to one declaration. A logical/physical pair has two
//! *different* ids, so `push` collapsed nothing, both survived, and first-seen-wins handed the win
//! to the one that had been pushed first: the loser.
//!
//! **Ascending order was never correct — it was undetectable.** Only the case `push` cannot collapse
//! exposes it.
//!
//! ## The RED probe (run, not imagined)
//!
//! Reverting `merge_ascending` to `for (decl, imp) in ascending { merged.push(...) }` — the ascending
//! push it replaced — flips `r1`, `r2`, `r3`, `r4`, `r5`, `r6`, `blockSize` and `insetPhysicalLast`
//! and leaves every control green. ⚠ A gate written only from the reset direction (`r1` alone) is
//! **half a gate**: the mirror rows `r2`/`r4`/`r6`, where the LOGICAL declaration is the one that
//! must win, are what refutes the plausible wrong fix — *"physical always beats logical"* — which
//! would pass the reset rows and be a different bug.

use manuk_text::FontContext;

/// Every row is a conflict between a logical and a physical declaration for the SAME side, and
/// nothing else varies. `r8`/`r9` are single-declaration controls: if either moves, the fixture is
/// measuring the property's support rather than the cascade.
const HTML: &str = r##"<!doctype html><html><head><style>
  /* physical declared LAST, same selector — the reset case */
  #r1 { margin-inline: 20px; }
  #r1 { margin: 0; }
  /* logical declared LAST — the MIRROR. A "physical always wins" fix fails here. */
  #r2 { margin: 0; }
  #r2 { margin-inline: 20px; }
  /* longhand pair, physical last */
  #r3 { margin-inline-start: 20px; }
  #r3 { margin-left: 5px; }
  /* longhand pair, logical last */
  #r4 { margin-left: 5px; }
  #r4 { margin-inline-start: 20px; }
  /* SPECIFICITY, not order: the physical rule is declared FIRST and must still win */
  #r5.t { margin-left: 5px; }
  #r5 { margin-inline-start: 20px; }
  /* …and the mirror: the LOGICAL rule is declared first and must still win on specificity */
  #r6.t { margin-inline-start: 20px; }
  #r6 { margin-left: 5px; }
  /* padding, so the finding is not margin-only */
  #r7 { padding-inline-start: 20px; }
  #r7 { padding: 0; }
  /* size: `block-size` vs `height`, physical last */
  #bs { block-size: 40px; }
  #bs { height: 12px; }
  /* inset on an absolutely positioned box, physical last */
  #in { position: absolute; inset-inline-start: 20px; left: 5px; }
  /* controls — ONE declaration each */
  #r8 { margin-left: 20px; }
  #r9 { margin-inline-start: 20px; }
</style></head><body>
<div id="r1" class="t">1</div><div id="r2" class="t">2</div><div id="r3" class="t">3</div>
<div id="r4" class="t">4</div><div id="r5" class="t">5</div><div id="r6" class="t">6</div>
<div id="r7" class="t">7</div><div id="bs" class="t">bs</div><div id="in" class="t">in</div>
<div id="r8" class="t">8</div><div id="r9" class="t">9</div>
</body></html>"##;

fn style_of<'p>(page: &'p manuk_page::Page, sel: &str) -> &'p manuk_css::ComputedStyle {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"))
}

fn assert_px(got: f32, want: f32, what: &str, why: &str) {
    assert!(
        (got - want).abs() < 0.51,
        "G_CASCADE_LOGICAL_PHYSICAL: {what} — expected {want}px, got {got}px.\n  {why}"
    );
}

#[test]
fn a_logical_and_a_physical_declaration_cascade_as_one_property() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://logical.test/", &fonts, 800.0);
    let ml = |sel: &str| style_of(&page, sel).margin.left.resolve(0.0, 0.0);

    // ── CONTROLS FIRST. Both spellings must work in isolation, or every row below is measuring
    //    property support rather than cascade order.
    assert_px(
        ml("#r8"),
        20.0,
        "control: `margin-left: 20px` alone",
        "the physical longhand, uncontested",
    );
    assert_px(
        ml("#r9"),
        20.0,
        "control: `margin-inline-start: 20px` alone",
        "the logical longhand, uncontested — it reaches layout, so nothing below is about support",
    );

    // ── 1. THE RESET DIRECTION. The physical declaration is later and must win.
    assert_px(
        ml("#r1"),
        0.0,
        "`#r1 { margin-inline: 20px }` then `#r1 { margin: 0 }`",
        "`* { margin: 0 }` is the first line of every CSS reset on the web, and any rule spelled \
         logically was immune to it — the reset lost to the thing it exists to remove",
    );
    assert_px(
        ml("#r3"),
        5.0,
        "`margin-inline-start: 20px` then `margin-left: 5px`",
        "the same conflict between two LONGHANDS, so the finding is not about shorthand expansion",
    );

    // ── 2. THE MIRROR, and it is the half that refutes the plausible wrong fix. "Physical beats
    //    logical" would pass every row above and fail every row here.
    assert_px(
        ml("#r2"),
        20.0,
        "`#r2 { margin: 0 }` then `#r2 { margin-inline: 20px }`",
        "the LOGICAL declaration is later, so it wins. A rule that always preferred the physical \
         spelling would pass claim 1 and be a different bug of the same size",
    );
    assert_px(
        ml("#r4"),
        20.0,
        "`margin-left: 5px` then `margin-inline-start: 20px`",
        "the longhand mirror",
    );

    // ── 3. IT IS DECIDED BY PRIORITY, NOT ORDER. In both rows the winner is declared FIRST and
    //    wins on specificity — which is what makes this a cascade claim rather than a source-order
    //    one, and what a "reverse the push order" fix gets wrong in exactly one of the two.
    assert_px(
        ml("#r5"),
        5.0,
        "`#r5.t { margin-left: 5px }` (0,2,0) vs a LATER `#r5 { margin-inline-start: 20px }` (0,1,0)",
        "specificity is consulted across the two spellings — the physical rule is earlier in the \
         sheet and still wins",
    );
    assert_px(
        ml("#r6"),
        20.0,
        "`#r6.t { margin-inline-start: 20px }` (0,2,0) vs a LATER `#r6 { margin-left: 5px }` (0,1,0)",
        "and symmetrically, with the logical rule as the more specific one",
    );

    // ── 4. NOT A MARGIN-ONLY DEFECT. Every logical group goes through the same merge.
    assert_px(
        style_of(&page, "#r7").padding.left.resolve(0.0, 0.0),
        0.0,
        "`padding-inline-start: 20px` then `padding: 0`",
        "`* { padding: 0 }` is the reset's SECOND line and was immune the same way",
    );
    assert_px(
        style_of(&page, "#bs").height.resolve(0.0, 0.0),
        12.0,
        "`block-size: 40px` then `height: 12px`",
        "the sizing group — a logical size that outlives a physical one changes the box, not just \
         its offset, and cascades a height error down the whole subtree",
    );
    assert_px(
        style_of(&page, "#in").inset.left.resolve(0.0, 0.0),
        5.0,
        "`inset-inline-start: 20px; left: 5px` in ONE declaration block",
        "the inset group, and within a single rule — so the merge is wrong per-block too, not only \
         across rules",
    );
}
