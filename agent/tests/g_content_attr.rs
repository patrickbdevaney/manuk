//! **G_CONTENT_ATTR — `content: attr(href)` met its element on one cascade and not the other.**
//!
//! `a::after { content: " (" attr(href) ")" }` — printing a link's target after its text — is the
//! idiom `attr()` exists for, and it is on **14 of 39** sampled CrUX sites, the highest-priced row
//! in t1369's sweep of stylo's pref gates.
//!
//! ⭐⭐⭐ **THE CAUSE IS A SIGNATURE, NOT A MISSING FEATURE.** `ContentPart::Text`'s own doc has
//! always said an `attr()` is *"already resolved against the element (that one CAN be resolved in
//! the cascade — the attribute is right there on the element)"* — and on the Stylo path it is, by a
//! mapper that holds the element. `MinimalCascade` could not: `apply_declaration` takes a
//! `&Declaration` and a parent font size, **with no element in sight**, so `content: attr(data-x)`
//! silently produced nothing on that cascade while the shipping path rendered it.
//!
//! That is the twin-drift class for the fourth time in a week — t1361 (`font-size` clobbering an
//! inherited `line-height`), t1364 (`table { border-spacing: 2px }` in one UA sheet), t1369 (`/`
//! alt-text parsed by one cascade) — and the rule it keeps proving is the one this file's float half
//! already states: *a cascade that disagrees with its twin is the `<source>` bug again.*
//!
//! The term now survives the value parser unresolved (`ContentPart::Attr`) and meets its element one
//! layer out in `cascade_node`.
//!
//! ## ⚠ AND THE HALF-FIX THAT WOULD HAVE PASSED A LESSER GATE
//!
//! Resolving only the ELEMENT's own `content` fixes the case nobody writes. **`attr()` is almost
//! never on an element's `content` — it is on a pseudo's**, and a pseudo is cascaded by
//! `cascade_pseudo` into `s.before` / `s.after`, which are separate `ComputedStyle`s. The first
//! version of this fix did exactly that and the `attr()` row of `g_ax_name_content_alt` stayed red.
//! Both are resolved now, and the pseudo is resolved **against its ORIGINATING element** — a pseudo
//! has no attributes of its own.
//!
//! ## Chrome-measured (`16px/24px monospace`)
//!
//! ```text
//!   <a href="/docs">link</a>  with  ::after { content: " (" attr(href) ")" }   115.59
//!   <span data-x="VAL">x</span>     ::before { content: attr(data-x) }          38.55
//!   <span>x</span>                  ::before { content: attr(data-missing) }     9.64   NEGATIVE
//!   <span>x</span>                  ::before { content: "[" attr(data-missing) "]" }  28.91
//! ```
//!
//! ⚠ **The last two rows are CSS 2.1 §12.2 and they are the reason a miss is the EMPTY STRING rather
//! than a dropped term.** Row 3 is 9.64 — one character, the `x` — so a missing attribute contributes
//! nothing *visible*; but row 4 is 28.91, three characters, so the brackets around it **still
//! render**. An implementation that drops the whole declaration on a missing attribute passes row 3
//! and fails row 4, and `a::after{content:" ("attr(href)")"}` on an `<a>` with no `href` is exactly
//! that case.
//!
//! PROVEN RED by three mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
.w{width:600px;margin:0 0 8px 0}
#e1::after{content:" (" attr(href) ")";}
#e2::before{content:attr(data-x);}
#e3::before{content:attr(data-missing);}
#e4::before{content:"[" attr(data-missing) "]";}
</style></head><body>
<div class="w"><a id="e1" href="/docs">link</a></div>
<div class="w"><span id="e2" data-x="VAL">x</span></div>
<div class="w"><span id="e3">x</span></div>
<div class="w"><span id="e4">x</span></div>
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
fn content_attr_meets_its_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ca.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.5;

    // ── VACUITY. The attributes must exist, or "it resolves attr()" is a claim about terms that
    // were never in the fixture.
    assert_eq!(
        page.dom()
            .element(by_id(&page, "e1"))
            .and_then(|e| e.attr("href")),
        Some("/docs"),
        "VACUOUS: #e1 has no href, so its attr() row proves nothing"
    );
    assert_eq!(
        page.dom()
            .element(by_id(&page, "e3"))
            .and_then(|e| e.attr("data-missing")),
        None,
        "VACUOUS: #e3's attribute exists after all, so the missing-attribute rows are not about a \
         missing attribute"
    );

    let rows: &[(&str, f32, &str)] = &[
        ("e1", 115.59, "`a::after{content:\" (\" attr(href) \")\"}` — the idiom attr() exists for"),
        ("e2", 38.55, "a bare attr() as the whole declaration"),
        ("e3", 9.64, "NEGATIVE — a missing attribute contributes the EMPTY string, so only the `x` is drawn"),
        ("e4", 28.91, "…and the literals AROUND it still render. An implementation that drops the whole declaration on a missing attribute passes e3 and fails this"),
    ];
    for (id, want, why) in rows {
        let got = width(&page, id);
        assert!(
            near(got, *want),
            "G_CONTENT_ATTR #{id}: Chrome renders this {want} wide, got {got}.\n  {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `parse_content_parts` drops the `attr(` branch (the pre-tick behaviour)
//       -> e1 reads ~67 and e2 ~9.6; e3 stays green, which is what shows the NEGATIVE row cannot
//          carry this gate on its own.
// N2  resolve only the ELEMENT's `content`, not the pseudo's (the half-fix)
//       -> every row fails, because in this fixture every `attr()` is on a pseudo. That is the
//          shape the first version of the t1372 fix actually had.
// N3  treat a missing attribute as a dropped TERM rather than the empty string
//       -> e3 stays green and e4 reads 9.64 instead of 28.91: the brackets vanish with the value.
