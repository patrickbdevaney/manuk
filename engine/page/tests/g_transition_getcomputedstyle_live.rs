//! **G_TRANSITION_GETCOMPUTEDSTYLE_LIVE — with no DOM mutation between two `getComputedStyle`
//! reads, the value moves while a transition runs (real wall clock) and is stable once it ends.**
//!
//! RED, run: drop the `any_running_realtime()` term from `forced_reflow`'s early return — every read
//! after the change equals the first.
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 #a { background-color: rgb(0,0,0); transition: background-color 600ms linear; }
 #a.on { background-color: rgb(200,100,50); }
</style></head><body><div id="a"></div><div id="out"></div>
<script>
  var a = document.getElementById('a'), R = [];
  function wait(ms){ var t=Date.now(); while(Date.now()-t<ms){} }
  getComputedStyle(a).backgroundColor;
  a.className = 'on';
  R.push(getComputedStyle(a).backgroundColor);   // change seen: start
  wait(300);
  R.push(getComputedStyle(a).backgroundColor);   // ~300ms, no mutation: mid
  wait(500);
  R.push(getComputedStyle(a).backgroundColor);   // ~800ms: done
  R.push(getComputedStyle(a).backgroundColor);   // stable
  document.getElementById('out').textContent = R.join('|');
</script></body></html>"##;

#[test]
fn computed_style_moves_while_a_transition_runs() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://t.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TRANSITION-LIVE {got}");
    let v: Vec<&str> = got.split('|').collect();
    assert_eq!(v.len(), 4, "{got}");
    let red = |s: &str| s.trim_start_matches("rgb(").split(',').next().unwrap().parse::<i32>().unwrap();
    assert!(red(v[0]) <= 30, "start is near the from value: {got}");
    assert!((red(v[1]) - 100).abs() <= 45, "mid is about halfway: {got}");
    assert_ne!(v[1], v[0], "second read must differ while running: {got}");
    assert_ne!(v[1], "rgb(200, 100, 50)", "mid is strictly between: {got}");
    assert_eq!(v[2], "rgb(200, 100, 50)", "{got}");
    assert_eq!(v[3], v[2], "stable after end: {got}");
}
