//! **G_CONTENTEDITABLE_QUERY — the editability QUERY surface: `contentEditable` / `isContentEditable` / `designMode`.**
//!
//! Brick 1 of the rich-editing subsystem (PHASE0-BOUNDED-REMAINDER Tier-1 #3). Every rich-text editor
//! DETECTS its editable host before it initialises — ProseMirror / Slate / Draft / TinyMCE / CKEditor all
//! branch on `el.isContentEditable` — and it was `undefined` (falsy) on a `<div contenteditable>`, so an
//! editor mount and every `contenteditable`-detection library read the element as plain, non-editable.
//! This makes DETECTION honest. It does NOT claim the keystroke/`execCommand` editing PATH (separately,
//! honestly, still absent) — so the gate asserts reflection + computed inheritance, never "typing works".
//!
//! Teeth (a stub returning a constant fails several at once):
//!   * `ce-idl` — `el.contentEditable` reflects the enumerated attribute: 'true' | 'false' |
//!     'plaintext-only' | 'inherit' (absent).
//!   * `is-true` / `is-false` — `isContentEditable` is the computed boolean for an explicit host.
//!   * `inherit-down` — a plain child of a `contenteditable` host is itself editable (the ancestor WALK).
//!   * `false-blocks` — a `contenteditable=false` island inside an editable host is NOT editable (explicit
//!     false wins over an editable ancestor).
//!   * `set-idl` — setting `el.contentEditable='true'` writes the content attribute (reflection round-trips).
//!   * `designmode` — `document.designMode='on'` makes an otherwise-plain element editable; 'off' restores.
//!
//! Proven RED: delete the shim and `present` reads `undefined` while `isContentEditable` is missing.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="host" contenteditable><p id="inner">x</p><div id="island" contenteditable="false"><span id="locked">y</span></div></div>
<div id="plain">z</div>
<textarea id="pt" contenteditable="plaintext-only"></textarea>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function $(id) { return document.getElementById(id); }
try {
  push('present:' + (typeof $('plain').isContentEditable === 'boolean'));
  // contentEditable IDL reflects the enumerated attribute.
  push('ce-idl:' + ($('host').contentEditable === 'true' &&
                    $('plain').contentEditable === 'inherit' &&
                    $('island').contentEditable === 'false' &&
                    $('pt').contentEditable === 'plaintext-only'));
  // isContentEditable — the computed boolean.
  push('is-true:' + ($('host').isContentEditable === true));
  push('is-false:' + ($('plain').isContentEditable === false));
  // Inheritance: a plain descendant of an editable host is editable.
  push('inherit-down:' + ($('inner').isContentEditable === true));
  // An explicit contenteditable=false island inside the host is NOT editable, and neither is its child.
  push('false-blocks:' + ($('island').isContentEditable === false &&
                          $('locked').isContentEditable === false));
  // Setting the IDL property round-trips to the content attribute.
  $('plain').contentEditable = 'true';
  push('set-idl:' + ($('plain').getAttribute('contenteditable') === 'true' &&
                     $('plain').isContentEditable === true));
  $('plain').contentEditable = 'inherit';
  push('set-inherit:' + ($('plain').hasAttribute('contenteditable') === false));
  // designMode makes an otherwise-plain element editable, and restores.
  document.designMode = 'on';
  push('dm-on:' + (document.designMode === 'on' && $('plain').isContentEditable === true));
  document.designMode = 'off';
  push('dm-off:' + (document.designMode === 'off' && $('plain').isContentEditable === false));
} catch (e) {
  push('THREW:' + e);
}
$('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn contenteditable_query_surface_reflects_and_inherits() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://contenteditable.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CONTENTEDITABLE-QUERY RESULT: {got}");

    for claim in [
        "present:true",
        "ce-idl:true",
        "is-true:true",
        "is-false:true",
        "inherit-down:true",
        "false-blocks:true",
        "set-idl:true",
        "set-inherit:true",
        "dm-on:true",
        "dm-off:true",
    ] {
        assert!(
            got.contains(claim),
            "G_CONTENTEDITABLE_QUERY: expected `{claim}`\n  got: {got}\n\n  \
             The editability query surface must reflect `contentEditable`, compute `isContentEditable` up \
             the ancestor chain (explicit false wins), and honour `document.designMode`. Reflection only — \
             the editing path is a later brick."
        );
    }
}
