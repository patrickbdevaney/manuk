//! **G_TRANSITION_REALTIME — an ordinary `transition: background-color 2s linear` (delay 0) now
//! interpolates against a real clock instead of jumping to its target.**
//!
//! Before: `animation::time_ms` never moved and the sampler only ran for a NEGATIVE delay, so a
//! class change put the element straight on its end value (root cause:
//! `docs/sprint/w6/J1b-drain-cause.md`). Now `transition::set_realtime_clock_ms` drives a per-node
//! running table captured at the style change.
//!
//! RED, run: make `sample_realtime` capture `start = 0` instead of `clock` — the t=1000 cascade then
//! reads elapsed 1000 either way, but the clock-0 capture is what the stubbed variant changes
//! when the change is seen at clock 500 (this test changes the class at 500): mid reads the target.

use manuk_css::stylo_engine::cascade_via_stylo;
use manuk_css::MinimalCascade;

const HTML: &str = r#"<!doctype html><html><head><style>
 #a { background-color: rgb(0,0,0); transition: background-color 2s linear; }
 #a.on { background-color: rgb(200,100,50); }
 #b { background-color: rgb(0,0,0); }
 #b.on { background-color: rgb(200,100,50); }
</style></head><body><div id="a"></div><div id="b"></div></body></html>"#;

fn bg(dom: &manuk_dom::Dom, sheets: &[manuk_css::Stylesheet], id: &str) -> (u8, u8, u8) {
    let map = cascade_via_stylo(dom, sheets, 800.0, 600.0);
    let n = dom.get_element_by_id(dom.root(), id).unwrap();
    let c = map[&n].background_color.expect("bg");
    (c.r, c.g, c.b)
}

#[test]
fn a_class_change_transition_interpolates_on_the_realtime_clock() {
    let mut dom = manuk_html::parse(HTML);
    let sheets = MinimalCascade::collect_style_elements(&dom);
    manuk_css::transition::set_realtime_clock_ms(Some(0.0));
    assert_eq!(bg(&dom, &sheets, "a"), (0, 0, 0));
    // The style change is seen at 500ms.
    manuk_css::transition::set_realtime_clock_ms(Some(500.0));
    let a = dom.get_element_by_id(dom.root(), "a").unwrap();
    let b = dom.get_element_by_id(dom.root(), "b").unwrap();
    dom.set_attr(a, "class", "on");
    dom.set_attr(b, "class", "on");
    let start = bg(&dom, &sheets, "a");
    assert_eq!(start, (0, 0, 0), "elapsed 0 reads the from value");
    // 1000ms into a 2s linear transition (clock 1500): halfway.
    manuk_css::transition::set_realtime_clock_ms(Some(1500.0));
    let mid = bg(&dom, &sheets, "a");
    println!("TRANSITION-REALTIME mid={mid:?}");
    assert!(
        mid.0 > 0 && mid.0 < 200 && (mid.0 as i32 - 100).abs() <= 2,
        "mid must be strictly between from and target (~100), got {mid:?}"
    );
    // CONTROL: no transition declared -> target immediately.
    assert_eq!(bg(&dom, &sheets, "b"), (200, 100, 50));
    // 2100ms after the change (clock 2600): finished.
    manuk_css::transition::set_realtime_clock_ms(Some(2600.0));
    assert_eq!(
        bg(&dom, &sheets, "a"),
        (200, 100, 50),
        "past the end = target"
    );
    manuk_css::transition::set_realtime_clock_ms(None);
}
