//! **G_ONE_HIT_TEST_NOT_TWO — `elementFromPoint` and the accessibility tree's `hit_test` answered
//! the same question differently, because only one of them knew about layers.**
//!
//! ⭐⭐⭐ Two implementations of one rule — *what is on top at this point?* — and they disagreed on
//! the same page:
//!
//! ```text
//!                                      Chrome    a11y hit_test    elementFromPoint
//!   auto-z overlay over in-flow link     b1           b1  (t1465)      l1   ✗
//!   z-index:5 overlay                    b2           b2              l2   ✗
//! ```
//!
//! The a11y path folded a per-node layer map; `doc_element_from_point` was a **flat scan over the
//! layout rects resolving by smallest area only** — it consulted `pointer-events` and SVG
//! paintedness but never a layer at all. So an overlay with an *explicit* `z-index: 5` took the
//! click in the agent's tree and did not in the web API. This is the shape this repo keeps finding
//! (t1402's activation behaviour, t1403's `<summary>` toggle, t1356's CSSOM views): **the tests of
//! each implementation are evidence about that one only.**
//!
//! The fix is a JOIN, not a mirror. `manuk_css::stacking_layer` is now the single definition, folded
//! down the ancestor chain by both callers, with `TOP_LAYER_Z` moved beside it — `elementFromPoint`
//! lives in `manuk_js`, which cannot see `manuk_page`, and **a constant only one of two
//! implementations can reach is how they drift apart in the first place.**
//!
//! Chrome-measured, as `elementFromPoint / elementsFromPoint[0]` at each link's centre:
//!
//! ```text
//!                                          Chrome     before      after
//!   l1  auto-z overlay over in-flow link    b1/b1      l1/l1      b1/b1
//!   l2  z-index:5 overlay                   b2/b2      l2/l2      b2/b2
//!   l3  z-index:-1 underlay                 l3/l3      l3/l3      l3/l3   ✓
//!   l4  no overlay             CONTROL      l4/l4      l4/l4      l4/l4   ✓
//!   l6  4-deep auto chain vs z-index:1      z1/z1      l6/l6      z1/z1
//! ```
//!
//! ⭐⭐ **THE PLURAL IS HALF THE POINT.** `doc_elements_from_point`'s own doc comment states the
//! contract *"`elementsFromPoint(x,y)[0]` must equal `elementFromPoint(x,y)`"* — so teaching only the
//! singular about layers would have broken the invariant the plural was written to hold. Every row
//! asserts both, which is why the fixture prints them as a pair.
//!
//! ⚠ `l3` is the row that keeps the layer term honest: a `z-index: -1` underlay must stay under, so
//! this is not "positioned wins" but an ordering.
//!
//! Mutations that must turn this red:
//!   1. drop the layer term from the singular   → l1, l2, l6 read the link
//!   2. drop it from the plural's sort          → the pair disagrees on l1, l2, l6
//!   3. `stacking_layer` returns `parent` for   → l1 and l6 read the link (the t1465 bug, now
//!      a positioned `auto`                        reached through the shared rule)

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0; font: 16px/1.4 monospace }
.row { position: relative; width: 300px; height: 60px; margin: 4px; background: #eef; overflow: hidden }
.row a { display: block; width: 200px; height: 30px; background: #cfc }
.over  { position: absolute; left: 0; top: 0; width: 300px; height: 60px }
.zover { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: 5 }
.under { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: -1 }
.nest  { position: relative }
.deep  { position: absolute; left: 0; top: 0; width: 300px; height: 60px }
.z1    { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: 1 }
</style></head><body>
<div class="row" id="r1"><a href="/a" id="l1">Auto over</a><div class="over" id="b1"></div></div>
<div class="row" id="r2"><a href="/b" id="l2">Z over</a><div class="zover" id="b2"></div></div>
<div class="row" id="r3"><a href="/c" id="l3">Negative</a><div class="under" id="b3"></div></div>
<div class="row" id="r4"><a href="/d" id="l4">No over</a></div>
<div class="row" id="r6">
  <div class="nest"><div class="nest"><div class="nest"><div class="deep" id="deep"></div></div></div></div>
  <a href="/f" id="l6">Deep</a><div class="z1" id="z1"></div></div>
<div id="out">-</div>
<script>
function hit(k){var r=document.getElementById(k).getBoundingClientRect();
 var x=r.left+r.width/2,y=r.top+r.height/2;
 var e=document.elementFromPoint(x,y), all=document.elementsFromPoint(x,y);
 var first=all&&all.length?(all[0].id||all[0].tagName.toLowerCase()):'-';
 return k+'='+(e?(e.id||e.tagName.toLowerCase()):'null')+'/'+first;}
document.getElementById('out').textContent=['l1','l2','l3','l4','l6'].map(hit).join(' ');
</script></body></html>"##;

#[test]
fn element_from_point_resolves_by_layer_then_depth() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://onehit.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ONE HIT: {got}");

    // ── VACUITY. The control row must resolve at all, or every row below is measuring whether
    //    elementFromPoint works rather than which LAYER it picks.
    assert!(
        got.contains("l4=l4/l4"),
        "VACUOUS: a link with nothing on top of it does not resolve to itself — got {got:?}"
    );

    // Chrome headless, singular and plural-head for all five rows.
    let want = "l1=b1/b1 l2=b2/b2 l3=l3/l3 l4=l4/l4 l6=z1/z1";
    assert_eq!(
        got, want,
        "\n  elementFromPoint must resolve by LAYER, then depth — and the plural's head must agree\n\
           want: {want}\n  got:  {got}"
    );
}
