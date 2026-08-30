//! **G_POINTER_EVENTS_CASCADE — `pointer-events: none` was inert on the cascade every agent gate
//! runs on.**
//!
//! The field exists, Stylo's mapper sets it, and `Page::non_hittable_nodes` reads it to build the
//! a11y tree's `hittable` flag — but `MinimalCascade` never parsed the property. Measured on one
//! fixture, an overlay with `pointer-events: none` over a target:
//!
//! ```text
//!   non-hittable nodes in the a11y tree     Stylo 1     MinimalCascade 0     <- was
//! ```
//!
//! ## ⭐⭐⭐ It lands squarely on this week's own work
//!
//! t1359 DEFINED `Landing::Unreachable` as *"on screen and `pointer-events: none`"*, and t1366 made
//! the agent's drive path refuse an obstructed target and scroll to an off-screen one. Both are
//! gated in `agent/tests` — **on the cascade where the property did nothing.** The hit-test those
//! gates exercise could not see a `pointer-events` overlay at all, so the one arm of `landing` that
//! distinguishes *"unaimable"* from *"off screen"* was untestable there.
//!
//! ⚠ `manuk-a11y`'s own unit test `hit_test_passes_through_a_pointer_events_none_overlay` does not
//! catch it either: it builds the tree **by hand**, setting `hittable` directly, so it tests
//! `hit_test`'s traversal and never the cascade that feeds it. A gate that constructs its own input
//! cannot discover that the producer of that input is broken.
//!
//! ## How it was found — surface audit #79, measuring the drift instead of tripping over it
//!
//! This is the twin-cascade drift class for the **fifth** time in a week (t1361 `font-size`
//! clobbering an inherited `line-height`, t1364 `border-spacing`, t1369 the `content` alt syntax,
//! t1372 `attr()`, now this). The first four were each found by accident. #79 asked how big the
//! class is: 14 corpus stylesheets, every declared property counted, and every property name
//! checked against `MinimalCascade`'s source. `pointer-events` is declared **146 times** and appears
//! nowhere in that cascade.
//!
//! ⚠⚠ **AND THE AUDIT CAUGHT ITSELF FIRST.** Its initial extraction — a regex over `"prop" =>` match
//! arms — reported `overflow` (419 declarations), `filter` and `border-bottom` as unhandled. All
//! three ARE handled, in multi-name arms the regex could not see. `SURFACE-AUDIT.md`'s own standing
//! note says *"grep the artefact, infer the engine — has now produced a wrong number three times"*,
//! and it had just produced a fourth. The ranking was redone against every quoted string in the
//! file, which errs toward calling a property handled, and the top row was then **measured on both
//! cascades** rather than published from a grep.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_a11y::A11yNode;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0}
#under{position:absolute;left:0;top:0;width:200px;height:100px}
#over{position:absolute;left:0;top:0;width:200px;height:100px;z-index:50;pointer-events:none}
#solid{position:absolute;left:0;top:200px;width:200px;height:100px}
#cover{position:absolute;left:0;top:200px;width:200px;height:100px;z-index:50}
</style></head><body>
<button id="under">under</button><div id="over">over</div>
<button id="solid">solid</button><div id="cover">cover</div>
</body></html>"##;

fn non_hittable(tree: &A11yNode) -> usize {
    tree.iter().filter(|n| !n.hittable).count()
}

#[test]
fn pointer_events_none_reaches_the_hit_test() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pe.test/", &fonts, 800.0);
    let tree = page.a11y_tree();

    // ── VACUITY. Both overlays must have laid out on a HIGHER layer, or neither arm is about
    // anything: an overlay that is not on top cannot intercept a click whatever its pointer-events.
    assert!(
        tree.iter()
            .filter(|n| n.z >= 50 && n.bbox.is_some())
            .count()
            >= 2,
        "VACUOUS: the two overlays are not on a higher stacking layer, so nothing is covered"
    );

    // ── ARM 1 · THE DEFECT. Exactly ONE node is non-hittable — the `pointer-events: none` overlay.
    assert_eq!(
        non_hittable(&tree),
        1,
        "G_POINTER_EVENTS_CASCADE: exactly one node carries `pointer-events: none`, so the a11y \
         tree must report exactly one non-hittable node. Reading 0 means the cascade never parsed \
         the property and every hit-test on this harness is blind to it."
    );

    // ── ARM 2 · THE CLICK PASSES THROUGH. A point inside the transparent overlay must reach the
    //    button underneath it.
    let hit = tree
        .hit_test(100.0, 50.0)
        .expect("something must be at (100,50)");
    assert_eq!(
        hit.name, "under",
        "G_POINTER_EVENTS_CASCADE: a click inside a `pointer-events: none` overlay must pass \
         THROUGH to the control beneath it, got {:?}",
        hit.name
    );

    // ── ARM 3 · CONTROL — an ORDINARY overlay still intercepts. Without this row the gate passes
    //    against an engine that ignores overlays entirely, which is the opposite bug.
    let hit = tree
        .hit_test(100.0, 250.0)
        .expect("something must be at (100,250)");
    assert_ne!(
        hit.name, "solid",
        "CONTROL: an overlay WITHOUT `pointer-events: none` must still intercept the click — \
         reaching the button beneath it means overlays are being ignored altogether"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `"pointer-events"` arm from `apply_declaration` (the pre-tick behaviour)
//       -> ARM 1 reads 0 non-hittable nodes and ARM 2 hits `over`; ARM 3 stays green, which is what
//          says the fix is about the PROPERTY and not about overlay handling in general.
// N2  parse it but invert the sense (`none` => Auto, anything else => None)
//       -> ARM 1 fires FIRST, not ARM 3: inverting makes every element whose `pointer-events` is
//          not `none` non-hittable, so the count is far above 1 rather than the 0 of N1. ARM 3 would
//          fail too. ⚠ The ledger says which arm actually fired rather than the one predicted when
//          the gate was written — the count arm turns out to discriminate both directions, which is
//          worth knowing before trusting the CONTROL to carry that case alone.
