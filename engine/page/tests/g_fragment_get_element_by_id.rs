//! **G_FRAGMENT_GET_ELEMENT_BY_ID — `this.shadowRoot.getElementById('x')` threw, because there was
//! nowhere correct to put the method.**
//!
//! `getElementById` is `NonElementParentNode`: it lives on **Document** and **DocumentFragment**
//! (which every ShadowRoot is), and **not** on Element. This engine's reflector surfaces are
//! prototypes — `EventTarget → Node → Element → HTMLElement`, with `Document` branching off `Node` —
//! and a shadow root is a Node, so it got `Node.prototype`. Putting `getElementById` there would have
//! defined it on **every element in the document**.
//!
//! So the fix is a link, not a method: `DocumentFragment.prototype → Node.prototype`, and
//! `ShadowRoot.prototype → DocumentFragment.prototype` (the real spec hierarchy). ⚠ `doc_get_by_id`
//! itself needed **no change** — it already roots at `this_node(vp)` and walks descendants. The
//! method was correct and homeless.
//!
//! Chrome-measured, and the last two rows are the ones that make it a *link* rather than a paste:
//!
//! ```text
//!                                        CHROME     BEFORE     AFTER
//!   template.content.getElementById     function   undefined   function
//!   fragment.getElementById('ku')       U          TypeError   U
//!   shadowRoot.getElementById('si')     i          TypeError   i
//!   element.getElementById              undefined  undefined   undefined   <- must STAY undefined
//!   document.getElementById             works      works       works
//! ```
//!
//! `this.shadowRoot.getElementById(...)` is the idiom of every hand-written web component, and
//! `template.content.getElementById(...)` is how compilers address their own template before cloning
//! it.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <template id="tpl"><p id="tp">t</p></template>
 <div id="d"><span id="s">s</span></div>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     function T(n, f) { try { return n + '=' + f(); } catch (e) { return n + '=' + e.name; } }
     var tpl = document.getElementById('tpl'), d = document.getElementById('d');
     var frag = document.createDocumentFragment(), k = document.createElement('u');
     k.id = 'ku'; frag.appendChild(k);
     var h = document.createElement('div');
     var sr = h.attachShadow({ mode: 'open' });
     sr.innerHTML = '<i id="si">i</i>';
     document.getElementById('out').textContent = [
       T('tplWorks', function () { return tpl.content.getElementById('tp').textContent; }),
       T('fragWorks', function () { return frag.getElementById('ku').tagName; }),
       T('shadowWorks', function () { return sr.getElementById('si').textContent; }),
       T('miss', function () { return String(sr.getElementById('nope')); }),
       // THE CONTROL: it must NOT have leaked onto elements.
       T('elementHasNot', function () { return typeof d.getElementById; }),
       T('docStillHas', function () { return document.getElementById('d').id; }),
       // …and the fragment must still inherit everything it had from Node.prototype.
       T('shadowQS', function () { return typeof sr.querySelector; }),
       T('shadowAEL', function () { return typeof sr.addEventListener; })
     ].join(' ');
   });
 </script>
</body></html>"#;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn get_element_by_id_lives_on_fragments_and_not_on_elements() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://frag.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("FRAGMENT-GEBI {got}");
    let has = |s: &str| got.contains(s);

    // (1) **All three fragment kinds.** RED: remove the `Tier::DocumentFragment` branch → `TypeError`
    // on every one, which is what shipped.
    assert!(
        has("tplWorks=t") && has("fragWorks=U") && has("shadowWorks=i"),
        "getElementById must work on a <template>'s content, a DocumentFragment and a ShadowRoot — \
         got {got:?}"
    );
    assert!(
        has("miss=null"),
        "a miss must be `null`, not undefined or a throw — got {got:?}"
    );

    // (2) **THE CONTROL, and it is the whole reason this is a prototype LINK and not a line added to
    // `Node.prototype`.** RED: define `getElementById` on the Node tier → `elementHasNot=function`,
    // and every element in every document grows a method the spec does not give it.
    assert!(
        has("elementHasNot=undefined"),
        "`getElementById` must NOT be on elements — it is NonElementParentNode, and putting it on \
         `Node.prototype` would define it on every element in the document — got {got:?}"
    );
    assert!(
        has("docStillHas=d"),
        "document.getElementById must be unaffected — got {got:?}"
    );

    // (3) **The new link did not COST anything.** A fragment prototype inserted in the wrong place
    // would shadow `Node.prototype` rather than extend it, and these would vanish.
    assert!(
        has("shadowQS=function") && has("shadowAEL=function"),
        "a shadow root must still inherit querySelector/addEventListener from Node.prototype — \
         got {got:?}"
    );
}
