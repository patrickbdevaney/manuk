//! **G_CHARACTERDATA_IFACE — the `CharacterData` abstract base interface exists.**
//!
//! `CharacterData` is the WebIDL base of `Text` (nodeType 3), `Comment` (8), `ProcessingInstruction`
//! (7) and `CDATASection` (4). It was never installed as a global, so `node instanceof CharacterData`
//! threw a `ReferenceError` — and `dom/nodes/Document-create{TextNode,Comment}` assert exactly that as
//! their FIRST check, aborting all 12 subtests before reaching `data`/`nodeType`/`nodeName` (which were
//! already correct).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [], push = function (k, v) { R.push(k + '=' + v); };
  push('type', typeof CharacterData);                                  // function
  push('text', document.createTextNode('a') instanceof CharacterData); // true
  push('comment', document.createComment('c') instanceof CharacterData); // true
  push('pi', document.createProcessingInstruction('t', 'd') instanceof CharacterData); // true
  push('el', document.body instanceof CharacterData);                  // false
  push('chain', (document.createTextNode('a') instanceof Node) + ',' + (document.createTextNode('a') instanceof Text)); // true,true
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn characterdata_base_interface_exists() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cd.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "type=function",
            "CharacterData is installed as a global constructor",
        ),
        ("text=true", "a Text node is a CharacterData"),
        ("comment=true", "a Comment node is a CharacterData"),
        ("pi=true", "a ProcessingInstruction node is a CharacterData"),
        ("el=false", "an Element is NOT a CharacterData"),
        ("chain=true,true", "a Text node is still a Node and a Text"),
    ] {
        assert!(
            got.contains(claim),
            "G_CHARACTERDATA_IFACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
