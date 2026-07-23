//! **G_FORM_ELEMENTS — `form.elements` is a live `HTMLFormControlsCollection` with named access.**
//!
//! `form.elements` was `undefined` ENTIRELY. `for (var i=0;i<form.elements.length;i++)` — the shape
//! every form-serialization / validation library uses — threw `can't access property "length",
//! form.elements is undefined`, and `form.elements['field']` / `.namedItem('field')` were the same
//! throw. It is a legacy platform object like `HTMLCollection`, with two form-specific rules that a
//! naive re-use of the plain collection would get wrong, so each is a way this goes RED:
//!
//!   * Members are the LISTED controls in tree order (button/fieldset/input/object/output/select/
//!     textarea) — EXCEPT `input[type=image]`, which the collection omits. `len` and `noimg` prove it.
//!   * Indexed (`elements[0]`, `.item(1)`) and named (`elements['role']` by name, `elements['extra']`
//!     by id, `.namedItem('username')`) access resolve against the controls.
//!   * The named getter returns a **`RadioNodeList`** when >1 control shares a name — a radio group —
//!     and that list's `.value` is the CHECKED radio's value (read) and selects the matching radio
//!     (write). Without it `form.elements.plan.value` silently returns the FIRST radio, not the
//!     selected one.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <form id="f">
      <input name="username" value="a">
      <input name="password" type="password">
      <select name="role"><option>x</option></select>
      <textarea name="bio"></textarea>
      <input type="radio" name="plan" value="free">
      <input type="radio" name="plan" value="pro" checked>
      <input type="image" name="img" src="x.png">
      <button id="btn">go</button>
      <input id="extra" type="text">
    </form>
    <div id="out">-</div><script>
    var r = [];
    var f = document.getElementById('f');
    r.push('type:' + typeof f.elements);
    r.push('len:' + f.elements.length);               // 9 controls minus the image input
    r.push('idx0:' + f.elements[0].tagName);          // indexed access, tree order
    r.push('item1:' + f.elements.item(1).type);       // .item(i)
    r.push('named:' + f.elements.namedItem('username').value);
    r.push('byname:' + f.elements['role'].tagName);   // named access by `name`
    r.push('byid:' + f.elements['extra'].tagName);    // named access by `id`
    var plan = f.elements['plan'];
    r.push('radioctor:' + (plan instanceof RadioNodeList));
    r.push('radiolen:' + plan.length);
    r.push('radioval:' + plan.value);                 // the CHECKED radio's value
    plan.value = 'free';
    r.push('radioset:' + f.elements['plan'].value);   // writing .value selects that radio
    r.push('noimg:' + (f.elements.namedItem('img') === null ? 'omitted' : 'PRESENT'));
    r.push('ctor:' + (f.elements instanceof HTMLFormControlsCollection));
    document.getElementById('out').textContent = r.join(' ');
    </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn form_elements_is_a_controls_collection_with_named_access() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://form-elements.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "type:object",    // form.elements exists and is an object, not undefined
        "len:8",          // 9 listed controls minus input[type=image]
        "idx0:INPUT",     // indexed access, in tree order
        "item1:password", // .item(i)
        "named:a",        // namedItem by `name`
        "byname:SELECT",  // named access by `name` attribute
        "byid:INPUT",     // named access by `id` attribute
        "radioctor:true", // a same-named group is a RadioNodeList
        "radiolen:2",     // both radios in the list
        "radioval:pro",   // .value is the CHECKED radio's value
        "radioset:free",  // writing .value selects the matching radio
        "noimg:omitted",  // input[type=image] is NOT a member
        "ctor:true",      // the collection is an HTMLFormControlsCollection
    ] {
        assert!(
            got.contains(claim),
            "G_FORM_ELEMENTS: expected {claim} in {got:?}\n  \
             form.elements must be a live HTMLFormControlsCollection (indexed/item/length + named \
             access by name/id) whose named getter returns a RadioNodeList for a radio group — its \
             absence throws `form.elements is undefined` in every form-serialization library."
        );
    }
}
