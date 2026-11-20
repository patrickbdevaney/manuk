//! **G_TRANSITION_EVENTS — a CSS transition dispatches `transitionrun`/`transitionstart` when it
//! starts and `transitionend` (propertyName, elapsedTime) exactly once per property when it ends;
//! a property whose value did not change gets none.**
//!
//! RED, run: skip the `EVENTS` push for `transitionend` in `sample_realtime` (or make
//! `flush_transition_events` drain without dispatching) — the log never gets `end:`.
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 #a { background-color: rgb(0,0,0); width: 10px; transition: background-color 2s linear, width 2s linear; }
 #a.on { background-color: rgb(200,100,50); }
</style></head><body><div id="a"></div><div id="out"></div>
<script>
  var a = document.getElementById('a'); window.LOG = [];
  ['transitionrun','transitionstart','transitionend'].forEach(function(t){
    a.addEventListener(t, function(e){ LOG.push(t + ':' + e.propertyName + ':' + e.elapsedTime); });
  });
  getComputedStyle(a).backgroundColor;
  a.className = 'on';
  getComputedStyle(a).backgroundColor;
</script></body></html>"##;

#[test]
fn transition_events_fire_once_per_changed_property() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://t.test/", &fonts, 800.0);
    page.tick_transitions(&fonts, 800.0); // delivers run/start
    manuk_css::transition::skew_realtime_ms(1000.0);
    page.tick_transitions(&fonts, 800.0); // mid: no end yet
    manuk_css::transition::skew_realtime_ms(1500.0);
    page.tick_transitions(&fonts, 800.0); // past 2s: end
    page.tick_transitions(&fonts, 800.0); // nothing more
    let root = page.dom().root();
    page.eval_for_test("document.getElementById('out').textContent = window.LOG.join('|');");
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TRANSITION-EVENTS {got}");
    let ends: Vec<&str> = got
        .split('|')
        .filter(|s| s.starts_with("transitionend"))
        .collect();
    assert_eq!(ends, vec!["transitionend:background-color:2"], "{got}");
    assert!(got.contains("transitionrun:background-color:0"), "{got}");
    assert!(got.contains("transitionstart:background-color:0"), "{got}");
    assert!(
        !got.contains("width"),
        "unchanged property must not fire: {got}"
    );
    let end_pos = got.find("transitionend").unwrap();
    assert!(got.find("transitionstart").unwrap() < end_pos, "{got}");
}
