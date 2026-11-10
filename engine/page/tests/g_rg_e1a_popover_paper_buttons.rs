//! RG.2.E1.a gate (chrome_ver=145.0.7632.116): a MUI Menu is a `z-index:1300` fixed popover holding an
//! absolute `z-index:500` Paper whose buttons are `position:relative; z-index:auto`. Chrome's
//! elementFromPoint at a button's centre returns the button (it paints in the Paper's context above the
//! Paper's background). manuk answered the Paper itself, so every control in the Beds & Baths menu was
//! `obstructed` and zilloft-2 could not apply a filter.

use manuk_text::FontContext;

const MENU: &str = r##"<!doctype html><html><head><style>
body{margin:0}
.pop{position:fixed;z-index:1300;inset:0}
.paper{position:absolute;z-index:500;left:100px;top:100px;width:400px;height:300px;background:#fff}
ul{position:relative;margin:0;padding:10px;list-style:none}
.b{position:relative;display:block;width:200px;height:40px;margin:10px}
.rd{position:relative;display:inline-block;width:34px;height:34px}.ri{position:absolute;z-index:1;opacity:0;inset:0;margin:0}.ic{position:relative;display:block;width:16px;height:16px;margin:9px}
.s{position:absolute;z-index:1;left:0;top:0;width:10px;height:10px}
</style></head><body>
<div class="pop"><div class="paper"><ul><li><button class="b" id="apply">APPLY</button></li><li><button class="b" id="x">X</button><span class="s" id="sib"></span></li><li><span class="rd"><input class="ri" id="radio" type="radio"><span class="ic"></span></span></li></ul></div></div>
<div id="out">-</div>
<script>
var r=document.getElementById('apply').getBoundingClientRect();
var e=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);
document.getElementById('out').textContent=e?(e.id||e.className):'null';
</script></body></html>"##;

#[test]
fn a_button_inside_a_nested_z_indexed_paper_is_topmost_and_lands_clear() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(MENU, "https://menu.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(page.dom().text_content(out), "apply");

    let tree = page.a11y_tree();
    let rd = tree
        .iter()
        .find(|n| n.role == manuk_a11y::Role::Radio)
        .expect("the radio");
    let l = tree.landing(rd.node, None);
    assert!(
        matches!(l, manuk_a11y::Landing::Clear { .. }),
        "the radio must be clickable, got {l:?}"
    );
    let n = tree
        .iter()
        .find(|n| n.name == "APPLY")
        .expect("the APPLY button");
    let l = tree.landing(n.node, None);
    assert!(
        matches!(l, manuk_a11y::Landing::Clear { .. }),
        "APPLY must be clickable, got {l:?}"
    );
}
