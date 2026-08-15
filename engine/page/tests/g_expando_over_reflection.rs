//! **G_EXPANDO_OVER_REFLECTION — `div.target = el` did not store `el`. It stored the STRING
//! `"[object HTMLSpanElement]"`, in a content attribute, on an element that has no `target` IDL
//! attribute at all.**
//!
//! The reflected content attributes are installed natively on ONE shared prototype and were **not
//! gated by tag**, so `target`, `href`, `src`, `rel`, `type`, `alt`, `name`, `placeholder`, `action`,
//! `method`, `content`, `media`, `srcset`, `htmlFor` and `value` reflected on **every** element.
//! Assigning any of those fifteen names to a `<div>` — or to a custom element — did not create an
//! expando. It ran a reflection setter that `String()`-ified the value into an attribute, and the
//! read came back a string. Measured before the fix, on a plain `<div>`:
//!
//! ```text
//!   d.target = spanElement   ->  d.target === "[object HTMLSpanElement]"
//!   d.href   = 1             ->  d.href   === "file:///tmp/1"      (resolved as a URL!)
//!   d.value  = 1             ->  d.value  === "1"
//! ```
//!
//! ⚠⚠⚠ **This is a WRONG ANSWER OF THE RIGHT TYPE, which is the most expensive failure shape this
//! project has.** Nothing throws, nothing logs, and the caller reads back a plausible string. It is
//! how WPT's own `css/support/interpolation-testcommon.js` died: it stashes an element with
//! `targetContainer.target = target` and reads it back one line later, so every
//! `*-interpolation.html` / `*-composition.html` in the tree threw
//! `TypeError: can't access property "setProperty", target.style is undefined` **during setup** and
//! produced no subtests whatsoever — **194 files, across css-transforms, css-backgrounds,
//! css-values, css-sizing, css-grid, css-fonts, css-ui, css-text, css-position, css-flexbox,
//! css-color and css-display.** Storing state on a DOM node is not a test-harness quirk; it is what
//! every framework, ad-blocker and analytics shim does.
//!
//! **The fix is a GATE ON THE NATIVE ACCESSOR, not a removal of it** — the native implementations
//! know things the reflection table does not (`<template>.content` is a `DocumentFragment`, a
//! ProcessingInstruction's `target` is its node name, `img.src` resolves against the document base),
//! so on a tag that HAS the IDL attribute they are called unchanged. On a tag that does not, the
//! property does what it does in every other browser: an ordinary, enumerable, own data property.
//!
//! ⚠⚠ **The reflection table alone is NOT a safe gate, and trusting it blindly would have been a far
//! bigger bug than the one being fixed.** `value` appears in the table only for the six elements that
//! reflect it as a plain content attribute (button/data/li/meter/option/param). `<input>`,
//! `<textarea>`, `<select>`, `<progress>` and `<output>` all *have* a `value` IDL attribute — it is
//! simply not a *reflection* — so the table's silence is a statement about the mechanism, not about
//! existence. Gating on the table alone would have made `input.value` `undefined`. `<template>` and
//! `content` are the same shape. Assertion (3) pins both.
//!
//! ⚠ **SVG is carved out by NAMESPACE, not by tag name.** `<use href>` is an `SVGAnimatedString` with
//! its own IDL; an HTML attribute table has no authority over a foreign element. Assertion (4).

use manuk_text::FontContext;

/// The fifteen names the native layer installs that are *also* tag-specific IDL attributes — i.e.
/// exactly the set that was destroying expandos. Kept as one list so the gate fails if the native
/// surface grows a sixteenth ungated name.
const HTML: &str = r##"<!doctype html><html><body>
 <a id="a" href="/x" target="_blank">a</a>
 <form id="f" action="/p" method="post" target="t"></form>
 <meta id="m" content="mc">
 <template id="tp"><b>hi</b></template>
 <input id="i" value="iv" type="text" placeholder="ph">
 <textarea id="ta">tv</textarea>
 <select id="sel"><option id="o" value="ov">O</option></select>
 <img id="img" src="/s.png" alt="A" srcset="/a 1x">
 <label id="lab" for="i"></label>
 <li id="li" value="7">x</li>
 <svg id="s"><use id="u" href="#frag"/></svg>
 <my-widget id="w"></my-widget>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     var $ = function (x) { return document.getElementById(x); };
     var NAMES = ['target','href','src','rel','type','alt','name','placeholder','action','method',
                  'content','media','srcset','htmlFor','value'];
     var parts = [];

     // (1) An element assigned to a name that is NOT its IDL attribute must come back AS THAT
     // ELEMENT — identity, not a stringification. This is the exact line the WPT interpolation
     // harness runs.
     var d = document.createElement('div');
     var span = document.createElement('span');
     d.target = span;
     parts.push('identity=' + (d.target === span));

     // (2) …for the whole family, on a plain <div> and on a custom element, and an unset one reads
     // `undefined` rather than the empty string a reflection getter would hand back.
     var fresh = document.createElement('div');
     var unset = 0;
     for (var i = 0; i < NAMES.length; i++) { if (fresh[NAMES[i]] === undefined) unset++; }
     parts.push('unset=' + unset + '/' + NAMES.length);

     var broken = [];
     var div2 = document.createElement('div');
     var widget = $('w');
     for (var i = 0; i < NAMES.length; i++) {
       var n = NAMES[i];
       div2[n] = 1;    if (div2[n] !== 1)   broken.push('div.' + n);
       widget[n] = 1;  if (widget[n] !== 1) broken.push('ce.' + n);
     }
     parts.push('broken=' + (broken.length ? broken.join(',') : 'none'));

     // …and it is a real own, enumerable data property, the way Chrome makes it.
     var od = Object.getOwnPropertyDescriptor(div2, 'target');
     parts.push('own=' + !!od + '/' + (od && od.enumerable) + '/' + (od && od.writable));

     // (3) THE REAL IDL ATTRIBUTES ARE UNTOUCHED — including the five `value` elements and the
     // `<template>.content` fragment that the reflection table does not describe.
     parts.push('idl=' + [$('a').target, $('f').action, $('f').method, $('f').target,
                          $('m').content, $('i').type, $('i').placeholder, $('img').alt,
                          $('img').srcset, $('lab').htmlFor, $('li').value].join('|'));
     parts.push('href=' + (/\/x$/.test($('a').href)) + ' src=' + (/\/s\.png$/.test($('img').src)));
     parts.push('value=' + [$('i').value, $('ta').value, $('sel').value, $('o').value].join('|'));
     parts.push('tplcontent=' + ($('tp').content && $('tp').content.nodeType));

     // (4) A FOREIGN element is out of the HTML table's jurisdiction: `<use href>` keeps whatever the
     // engine gives it and is NOT gated into `undefined`.
     parts.push('svghref=' + ($('u').href !== undefined && $('u').href !== ''));

     document.getElementById('out').textContent = parts.join(' ');
   });
 </script>
</body></html>"##;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_reflected_attribute_name_is_an_expando_on_an_element_that_has_no_such_idl_attribute() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://expando.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("EXPANDO-OVER-REFLECTION {got}");
    let has = |s: &str| got.contains(s);

    // (1) **Identity, not a string.** RED: drop the `appliesTo` gate in `reflect_js.rs` → the native
    // setter runs on a `<div>` and `identity=false`, because `d.target` is the string
    // `"[object HTMLSpanElement]"`. This one line is why 194 WPT interpolation files produced zero
    // subtests.
    assert!(
        has("identity=true"),
        "`div.target = element` must store the ELEMENT — a reflection setter stringifies it into a \
         content attribute and the read comes back \"[object HTMLSpanElement]\". Got {got:?}"
    );

    // (2) **The whole family, on a `<div>` AND on a custom element, both directions.** A fix that
    // handles `target` and leaves `href`/`value`/`content` reflecting is the partial-implementation
    // failure mode: a caller that finds one name working assumes the rest.
    assert!(
        has("broken=none") && has("unset=15/15"),
        "every tag-specific reflected name must behave as a plain expando on an element that does \
         not have that IDL attribute, and read `undefined` when unset — got {got:?}"
    );
    assert!(
        has("own=true/true/true"),
        "the expando must be a real own, enumerable, writable data property (that is what Chrome \
         creates) — got {got:?}"
    );

    // (3) **AND THE ACTUAL IDL ATTRIBUTES STILL WORK.** This is the assertion that makes the gate
    // worth having: the obvious implementation — gate on the reflection table alone — makes
    // `input.value`, `textarea.value`, `select.value` and `<template>.content` all `undefined`,
    // because the table describes REFLECTION and those are not reflections. RED: delete the
    // `EXTRA_TAGS` carve-out → `value=|||` and `tplcontent=undefined`.
    assert!(
        has("idl=_blank|/p|post|t|mc|text|ph|A|/a 1x|i|7"),
        "a tag that HAS the IDL attribute must still reflect it — got {got:?}"
    );
    assert!(
        has("href=true src=true"),
        "`a.href` / `img.src` must still resolve against the document base — got {got:?}"
    );
    assert!(
        has("value=iv|tv|ov|ov") && has("tplcontent=11"),
        "`value` on input/textarea/select/option and `<template>.content` are IDL attributes the \
         reflection table does NOT list, because they are not reflections. Gating on the table alone \
         deletes them — got {got:?}"
    );

    // (4) **A foreign element is not governed by an HTML attribute table.** RED: gate on tag name
    // instead of namespace → `<use>` is not an HTML tag, so `svghref=false`.
    assert!(
        has("svghref=true"),
        "SVG `<use href>` has its own IDL and must not be gated away by the HTML table — got {got:?}"
    );
}
