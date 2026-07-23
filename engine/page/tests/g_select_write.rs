//! **G_SELECT_WRITE — the `<select>` write API: `add` / `remove(index)` + HTMLOptionsCollection methods.**
//!
//! Two silent wrongs in the everyday JS-driven `<select>` population path (country pickers,
//! dependent dropdowns, "add another" rows). Each claim is a way this goes RED:
//!
//!   * `select.add(option[, before])` was `undefined` — the primary way to insert an option threw.
//!   * `select.remove(0)` DETACHED THE WHOLE SELECT: with no own `remove(index)`, the call fell through
//!     to the inherited `ChildNode.remove()`, which ignores the argument and tore the control out of its
//!     form. `remove(0)` must remove `options[0]` and leave the select where it is.
//!   * `select.options.namedItem` / `.add` / `.remove` (HTMLOptionsCollection) were absent.
//!
//! And the two invariants the fix must NOT break: `select.remove()` with NO argument keeps the legacy
//! detach-self overload, and `div.remove()` still detaches an ordinary element.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<form id="f1"><select id="sel">
<option id="o0" value="a">A</option>
<option id="o1" value="b">B</option>
<option id="o2" value="c" name="cee">C</option>
</select></form>
<select id="sel2"><option>x</option></select>
<div id="gone">G</div>
<div id="out">-</div><script>
var r=[];
function t(k,v){ r.push(k+':'+v); }
var sel=document.getElementById('sel');
// remove(index) removes THAT option and leaves the select in its form
sel.remove(0);
t('removeOpt', sel.options.length+'/'+(sel.parentNode?sel.parentNode.id:'DETACHED')+'/'+sel.options[0].id);
// options.namedItem
t('named', sel.options.namedItem('cee') ? sel.options.namedItem('cee').id : 'null');
// select.add(option) appends
var no=document.createElement('option'); no.id='added'; no.value='z'; sel.add(no);
t('add', sel.options.length+'/'+sel.options[sel.options.length-1].id);
// select.add(option, before) inserts before the reference option
var nb=document.createElement('option'); nb.id='before'; sel.add(nb, sel.options[0]);
t('addBefore', sel.options[0].id);
// options.add exists (HTMLOptionsCollection)
t('optsAdd', typeof sel.options.add);
// a <div> is NOT a select — no add method (Chrome parity)
t('divAdd', document.createElement('div').add===undefined?'undefined':'DEFINED');
// INVARIANT 1: select.remove() with no arg keeps legacy detach-self
var s2=document.getElementById('sel2'); s2.remove();
t('selfDetach', s2.parentNode===null?'detached':'STILL');
// INVARIANT 2: div.remove() still detaches an ordinary element
var g=document.getElementById('gone'); g.remove();
t('divRemove', document.getElementById('gone')===null?'gone':'STILL');
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn select_add_remove_and_options_collection_write() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://select-write.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "removeOpt:2/f1/o1", // remove(0) removed option 0, select still in f1, first is now o1
        "named:o2",          // namedItem by name= attribute
        "add:3/added",       // add(option) appended
        "addBefore:before",  // add(option, ref) inserted before ref
        "optsAdd:function",  // HTMLOptionsCollection.add
        "divAdd:undefined",  // a <div> has no .add
        "selfDetach:detached", // select.remove() (no arg) still detaches self
        "divRemove:gone",    // div.remove() still works
    ] {
        assert!(
            got.contains(claim),
            "G_SELECT_WRITE: expected {claim} in {got:?}\n  \
             select.add/remove(index) + HTMLOptionsCollection.add/remove/namedItem must work, and \
             remove(index) must remove the OPTION (not detach the select), without breaking the \
             argument-less ChildNode.remove() overload."
        );
    }
}
