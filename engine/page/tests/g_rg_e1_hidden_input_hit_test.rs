//! RG.2.E1 gate (chrome_ver=145.0.7632.116): a MUI radio is a visually hidden `<input>` (absolute, opacity 0,
//! z-index 1, filling its `position:relative` root) over an icon span. Chrome's elementFromPoint at the
//! radio's centre returns the input itself, so a click is not occluded; manuk answered the icon, which the
//! agent then reported as `OCCLUDED: ... covered by generic ""` (dashdish-11, zilloft-2). RED under
//! `MANUK_NESTED_Z_MUTATE=1` (the old absolute `n * 1024 + 1`).

use manuk_text::FontContext;

const RADIO: &str = r##"<!doctype html><html><head><style>
body{margin:0}
.modal{position:fixed;z-index:1300;top:0;left:0;right:0;bottom:0}
.scr{position:absolute;overflow:auto;left:50%;top:50%;width:550px;height:300px;transform:translate(-275px,-150px)}
.grp{display:flex;flex-direction:column;padding:20px}
label{display:inline-flex;align-items:center;vertical-align:middle;cursor:pointer}
.root{display:inline-flex;position:relative;padding:9px;box-sizing:border-box;width:42px;height:42px;align-items:center;justify-content:center}
.inp{cursor:inherit;position:absolute;opacity:0;width:100%;height:100%;top:0;left:0;margin:0;padding:0;z-index:1}
.icon{position:relative;display:flex;width:24px;height:24px}
.icon svg{width:24px;height:24px}
.dot{position:absolute;left:0;transform:scale(0)}
.rip{overflow:hidden;pointer-events:none;position:absolute;z-index:0;inset:0px;border-radius:inherit}
</style></head><body>
<div class="modal"><div class="scr"><div class="grp"><label><span class="root"><input class="inp" id="r" type="radio" name="s" value="m"><span class="icon" id="icon"><svg viewBox="0 0 24 24"></svg><svg class="dot" viewBox="0 0 24 24"></svg></span><span class="rip" id="rip"></span></span><span id="t">Medium</span></label></div></div></div>
<div id="out">-</div>
<script>
var r=document.getElementById('r').getBoundingClientRect();
var e=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);
document.getElementById('out').textContent=e?(e.id||e.tagName.toLowerCase()):'null';
</script></body></html>"##;

#[test]
fn an_opacity_zero_input_over_its_icon_is_topmost_and_lands_clear() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(RADIO, "https://radio.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    assert_eq!(
        page.dom().text_content(out),
        "r",
        "the input must be topmost, as in Chrome 145"
    );

    // The agent's own hit-test (a11y tree) must agree: act on the radio is not OCCLUDED.
    let tree = page.a11y_tree();
    let n = tree
        .iter()
        .find(|n| n.role == manuk_a11y::Role::Radio)
        .expect("a radio");
    let l = tree.landing(n.node, None);
    assert!(
        matches!(l, manuk_a11y::Landing::Clear { .. }),
        "radio must be clickable, got {l:?}"
    );
}
