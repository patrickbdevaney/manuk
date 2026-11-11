//! RG.3.0.a gate (chrome_ver=145.0.7632.116): an auto-positioned button inside a nested z-indexed popup
//! stays above that popup's in-flow rows. staynb's "Add Adults guest" (a z-index:100 popup three contexts
//! deep) fell to the slice root under the popup's own layer and read `OCCLUDED ... covered by generic ""`;
//! Chrome's elementFromPoint returns the button. RED under `MANUK_AUTO_SLICE_MUTATE=1` (the old fall-back).

use manuk_text::FontContext;

const POPUP: &str = r##"<!doctype html><html><head><style>
body{margin:0}
.a{position:fixed;z-index:10;top:0;left:0;width:400px;height:300px}
.b{position:relative;z-index:1;width:400px;height:300px}
.c{position:absolute;z-index:100;top:20px;left:20px;width:300px;height:200px;background:#fff}
.row{position:relative;height:42px}
.lbl{width:200px;height:42px}
.add{position:absolute;right:0;top:0;width:39px;height:39px}
</style></head><body>
<div class="a"><div class="b"><div class="c"><div class="row"><div class="lbl"></div><button class="add" id="add">+</button></div></div></div></div>
<div id="out">-</div>
<script>
var r=document.getElementById('add').getBoundingClientRect();
var e=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);
document.getElementById('out').textContent=e?(e.id||e.tagName.toLowerCase()):'null';
</script></body></html>"##;

#[test]
fn an_auto_button_in_a_nested_z_popup_is_topmost_and_lands_clear() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(POPUP, "https://popup.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(
        page.dom().text_content(out),
        "add",
        "the button must be topmost, as in Chrome 145"
    );

    let tree = page.a11y_tree();
    let n = tree
        .iter()
        .find(|n| n.role == manuk_a11y::Role::Button)
        .expect("a button");
    let l = tree.landing(n.node, None);
    assert!(
        matches!(l, manuk_a11y::Landing::Clear { .. }),
        "button must be clickable, got {l:?}"
    );
}
