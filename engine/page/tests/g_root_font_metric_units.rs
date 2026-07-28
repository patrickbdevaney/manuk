//! **G_ROOT_FONT_METRIC_UNITS — `rcap` / `rch` / `rex` / `ric` measured the DEVICE'S DEFAULT font,
//! because nothing ever told Stylo which element was the root.**
//!
//! This is the third member of a set of three, and tick 722 named it in writing before it was
//! measured. When the root element is cascaded, Stylo's own `matching.rs` updates **three** things:
//!
//! ```text
//!   device.set_root_font_size(…)      // rem            <- we had this
//!   device.set_root_line_height(…)    // rlh            <- landed t722
//!   device.set_root_style(…) + update_root_font_metrics()   // rcap/rch/rex/ric   <- this gate
//! ```
//!
//! `update_root_font_metrics` reads `device.root_style`, and **nothing in this engine ever wrote
//! that field**, so every root-relative font-metric unit resolved against the device's default style
//! instead of the document's root element.
//!
//! Chrome-measured, root `font: 32px sans-serif`, element `font: 16px sans-serif`:
//!
//! ```text
//!             CHROME   BEFORE   AFTER          element-relative twin (already exact)
//!   10rch       178      80      178             10ch   = 89
//!   10rex       169      73      169             10ex   = 85
//!   10rcap      220     105      220             10cap  = 110
//! ```
//!
//! ⚠ **Every element-relative unit exact and every root-relative one wrong is the signature of a
//! root that was never published** — not of a broken metric. The element twins are asserted here for
//! that reason: they are the over-correction guard, and they are what identified the shape in the
//! first place.
//!
//! ⚠ Separate and still open (also true of `lh`/`rlh`): `CSS.supports('width','10rch')` answers
//! **false** here and **true** in Chrome, for units that demonstrably work — a false *negative*, so
//! a page that guards them takes its fallback for no reason.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><style>
 html { font: 32px sans-serif }
 body { margin:0; font: 16px sans-serif; width: 800px }
 #ch  { width: 10ch  }   #rch  { width: 10rch  }
 #ex  { width: 10ex  }   #rex  { width: 10rex  }
 #cap { width: 10cap }   #rcap { width: 10rcap }
</style></head><body>
 <div id="ch">c</div><div id="rch">c</div>
 <div id="ex">e</div><div id="rex">e</div>
 <div id="cap">p</div><div id="rcap">p</div>
</body></html>"#;

fn width(page: &manuk_page::Page, sel: &str) -> f32 {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    let rects = page.root_box.node_rects(page.dom());
    rects
        .get(&hits[0])
        .unwrap_or_else(|| panic!("{sel} has no box"))
        .width
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn root_relative_font_metric_units_measure_the_root_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://rfm.test/", &fonts, 800.0);

    let (ch, rch) = (width(&page, "#ch"), width(&page, "#rch"));
    let (ex, rex) = (width(&page, "#ex"), width(&page, "#rex"));
    let (cap, rcap) = (width(&page, "#cap"), width(&page, "#rcap"));
    println!("ROOT-FONT-METRICS ch={ch} rch={rch} ex={ex} rex={rex} cap={cap} rcap={rcap}");

    // (1) **The element-relative twins, which already worked** — the over-correction guard. A fix
    // that pointed every metric unit at the root would satisfy every `r*` assertion below and break
    // these three. Chrome measures 89 / 85 / 110 on this fixture.
    for (name, got, want) in [("ch", ch, 89.0), ("ex", ex, 85.0), ("cap", cap, 110.0)] {
        assert!(
            (got - want).abs() < 2.0,
            "10{name} must stay the ELEMENT's metric ({want}) — got {got}. If this moved, the fix \
             pointed the element-relative units at the root as well."
        );
    }

    // (2) **The root-relative units.** RED: delete the `set_root_style` call in `cascade_via_stylo`
    // → 80 / 73 / 105, the device's default style. Chrome measures 178 / 169 / 220.
    for (name, got, want) in [
        ("rch", rch, 178.0),
        ("rex", rex, 169.0),
        ("rcap", rcap, 220.0),
    ] {
        assert!(
            (got - want).abs() < 2.0,
            "10{name} must measure the ROOT element's 32px font ({want}) — got {got}. A value near \
             the element's own twin means the root style was never published to the cascade device."
        );
    }

    // (3) **The relationship, not just the numbers.** The root is exactly 2x the element's font
    // size, so each root-relative unit must be ~2x its twin. This survives a font-stack change that
    // would move all six absolute values together, which the assertions above would not.
    for (name, r, e) in [("rch", rch, ch), ("rex", rex, ex), ("rcap", rcap, cap)] {
        let ratio = r / e;
        assert!(
            (ratio - 2.0).abs() < 0.05,
            "10{name} / 10{} must be 2.0 (the root's font is 32px, the element's 16px) — got \
             {ratio:.3}",
            &name[1..]
        );
    }
}
