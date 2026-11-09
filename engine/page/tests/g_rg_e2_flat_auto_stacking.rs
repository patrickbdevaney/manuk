//! RG.E2 gate (chrome_ver=145.0.7632.116): `z-index: auto` positioning is FLAT (not parent+1 per nested
//! relative wrapper). staynb's Search button (absolute, after a MUI field two relative
//! wrappers deep) was refused `OCCLUDED by textbox "Who"`; Chrome's elementFromPoint returns the button.
//! RED under `MANUK_STACK_MUTATE=1` (the old cumulative `parent + 1`). Its own file: one SpiderMonkey
//! runtime per test process.

use manuk_text::FontContext;

/// RG.E2: a MUI search bar. The Who field is two `position:relative` wrappers deep and the Search
/// button is `position:absolute`, z-index auto, AFTER it, smaller. Chrome: the button is on top.
/// With `parent + 1` stacking the nested field outranked it and staynb's Search was `OCCLUDED`.
const SEARCHBAR: &str = r##"<!doctype html><html><head><style>
body{margin:0}
.box{position:relative;width:300px;height:60px}
.tf{position:relative;width:300px;height:60px;overflow:hidden}
.base{position:relative;width:300px;height:60px}
.inp{display:block;width:268px;height:50px}
.go{position:absolute;right:0;top:5px;width:50px;height:50px}
</style></head><body>
<div class="box"><div class="tf"><div class="base"><input class="inp" id="who"></div></div><button class="go" id="go">S</button></div>
<div id="out">-</div>
<script>
var r=document.getElementById('go').getBoundingClientRect();
var e=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);
document.getElementById('out').textContent=e?(e.id||e.tagName.toLowerCase()):'null';
</script></body></html>"##;

#[test]
fn a_later_positioned_sibling_beats_a_nested_positioned_field() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(SEARCHBAR, "https://searchbar.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(
        page.dom().text_content(out),
        "go",
        "the Search button must be topmost, as in Chrome 145"
    );
}
