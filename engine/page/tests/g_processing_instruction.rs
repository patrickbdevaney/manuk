//! **G_PROCESSING_INSTRUCTION — `document.createProcessingInstruction`, the PI node, and its validity.**
//!
//! `ProcessingInstruction` (`<?target data?>`, `nodeType` 7) is a `CharacterData` node carrying a
//! `target` (its `nodeName`) and a `data` body. The factory did not exist, so ~88 `dom/nodes` WPT
//! subtests failed not on a wrong value but on `createProcessingInstruction is not a function` — the
//! test threw before its first assertion and every later line (`.target`, `.data`, `cloneNode`) died on
//! `undefined`. Each assertion below is one spec rule the factory + node now satisfy:
//!
//! * **the factory returns a real PI** — `target`/`data`/`nodeName`/`nodeType` 7, and it is a `Node`.
//! * **CharacterData semantics** — `data` is settable; `nodeValue` equals `data` (it read `null` before,
//!   because the `nodeValue` getter only knew Text nodes — a latent Comment/PI bug this closes too).
//! * **pre-mint validity** (`InvalidCharacterError`) — a target that is not a valid XML `Name`, and a
//!   `data` containing the PI-close sequence `?>`. A colon in the target is a *valid* Name (`xml:fail`).
//! * **it lives in the tree** — appended, it is a child whose HTML serialization is `<?target data>`.
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip
//! the tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** before the factory existed the script threw at the first
//! `createProcessingInstruction(...)`, leaving `#out` at its `-` sentinel — RED. The full method turns
//! it GREEN. (`instanceof ProcessingInstruction` is *not* asserted: every node reflector shares one flat
//! `Node.prototype`, so per-interface `instanceof` awaits the member-tiering tick — named, not hidden.)

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"></div>
<script>
  var R = [];
  function ck(l, g) { R.push(l + ':' + g); }
  function thrown(fn){ try { fn(); return 'NO_THROW'; } catch(e){ return e.name; } }

  var pi = document.createProcessingInstruction('xml-stylesheet', 'href="a.css"');
  ck('target', pi.target);
  ck('data', pi.data);
  ck('nodeName', pi.nodeName);
  ck('nodeType', pi.nodeType);
  ck('isNode', pi instanceof Node);
  ck('owner', pi.ownerDocument === document);
  ck('nodeValue', pi.nodeValue);
  pi.data = 'x=1'; ck('setData', pi.data);

  // ── Pre-mint validity (InvalidCharacterError) ──
  ck('badData', thrown(function(){ document.createProcessingInstruction('A', 'a?>b'); }));
  ck('badTarget', thrown(function(){ document.createProcessingInstruction('0', 'x'); }));
  ck('validColon', thrown(function(){ document.createProcessingInstruction('xml:fail', 'x'); }));

  // ── It lives in the tree and serializes as <?target data> ──
  var host = document.getElementById('host');
  host.appendChild(document.createProcessingInstruction('foo', 'bar'));
  ck('childTarget', host.firstChild.target);
  ck('serial', host.innerHTML);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn processing_instruction_is_a_real_character_data_node_with_validity() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pi.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "target:xml-stylesheet",
            "the PI target is the first argument",
        ),
        ("data:href=\"a.css\"", "the PI data is the second argument"),
        ("nodeName:xml-stylesheet", "a PI's nodeName IS its target"),
        (
            "nodeType:7",
            "PROCESSING_INSTRUCTION_NODE = 7 (it fell through to 8/comment before)",
        ),
        ("isNode:true", "a PI is a Node"),
        (
            "owner:true",
            "the PI's ownerDocument is the creating document",
        ),
        (
            "nodeValue:href=\"a.css\"",
            "nodeValue equals data for a CharacterData node (read null before)",
        ),
        ("setData:x=1", "data is settable (CharacterData)"),
        (
            "badData:InvalidCharacterError",
            "data containing '?>' is an InvalidCharacterError",
        ),
        (
            "badTarget:InvalidCharacterError",
            "a target that is not a valid XML Name throws",
        ),
        (
            "validColon:NO_THROW",
            "a colon is valid in a Name, so 'xml:fail' is a legal target",
        ),
        ("childTarget:foo", "an appended PI is a child of its parent"),
        (
            "serial:<?foo bar>",
            "a PI serializes to HTML as <?target data> (single '>' close)",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_PROCESSING_INSTRUCTION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
