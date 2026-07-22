//! **G_CURRENT_SCRIPT — `document.currentScript` is the EXECUTING `<script>` element.**
//!
//! The tick-401 re-keyed oracle's third named okta error:
//! `TypeError: can't access property "hasAttribute", l.stubScriptElement is null`. Every
//! bundler's chunk loader opens with the same move — stash `document.currentScript` to find its
//! own tag, its `nonce`, its `data-*` config, and the base URL to load sibling chunks from
//! (webpack's `publicPath: "auto"` is literally this). Ours was hardcoded `null`, with a doc
//! comment correctly arguing null beats undefined — but null is the spec's answer only OUTSIDE
//! classic script execution; DURING it, the spec says it IS the element whose script is running.
//!
//! Teeth: identity, per script — each of two scripts must see ITSELF (asserted against its own
//! `getElementById`, so a stale "last script" or "first script" answer fails), its attributes
//! must be readable (`hasAttribute('data-cfg')` — the okta call, verbatim), and a
//! `type="module"` script must still read `null` (modules are never currentScript, per spec).
//!
//! Proven RED: with the hardcoded null, `s1-self`/`s2-self` read `null-during` and the gate
//! names the chunk-loader pattern it breaks.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script id="s1" data-cfg="alpha">
  window.r = [];
  var cs1 = document.currentScript;
  window.r.push('s1-self:' + (cs1 === document.getElementById('s1')));
  window.r.push('s1-tag:' + (cs1 && cs1.tagName));
  window.r.push('s1-attr:' + (cs1 && cs1.hasAttribute('data-cfg') && cs1.getAttribute('data-cfg')));
</script>
<script id="s2">
  var cs2 = document.currentScript;
  window.r.push('s2-self:' + (cs2 === document.getElementById('s2')));
  window.r.push('s2-not-s1:' + (cs2 !== document.getElementById('s1')));
</script>
<script type="module" id="m1">
  window.r.push('module-null:' + (document.currentScript === null));
  document.getElementById('out').textContent = window.r.join(' ');
</script>
</body></html>"##;

#[test]
fn current_script_is_the_executing_script_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CURRENT-SCRIPT RESULT: {got}");

    for claim in [
        "s1-self:true",     // the executing script sees ITSELF —
        "s1-tag:SCRIPT",    // — as a real element reflector,
        "s1-attr:alpha",    // with its attributes readable (the okta hasAttribute call)
        "s2-self:true",     // per-script tracking: the second script sees the SECOND element
        "s2-not-s1:true",   // not a stale first answer
        "module-null:true", // modules are never currentScript, per spec
    ] {
        assert!(
            got.contains(claim),
            "G_CURRENT_SCRIPT: expected `{claim}`\n  got: {got}\n\n  \
             During classic script execution `document.currentScript` must be the executing \
             <script> element (bundler chunk loaders stash it for their own tag, nonce and base \
             URL); outside execution and inside modules it is null."
        );
    }
}
