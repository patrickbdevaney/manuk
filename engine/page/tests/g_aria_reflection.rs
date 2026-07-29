//! **G_ARIA_REFLECTION — `el.ariaLabel = 'Close'` did nothing, and the whole ARIA IDL surface was
//! absent: 0 of 42 properties.**
//!
//! Chrome-measured on one fixture: **42 of 42 present**, reflecting both ways. Here: **0 of 42**, so
//! `el.ariaLabel = 'Close'` created a plain JS own-property, `getAttribute('aria-label')` stayed
//! `null`, and the accessibility tree never heard about it.
//!
//! **This is I3 territory, not a conformance detail.** The semantic model — *"a first-class
//! accessibility tree, load-bearing, never allowed to lag the renderer"* — is the project's stated
//! moat, and the ARIA IDL properties are how a modern component library *writes* to it. React,
//! Vue, Radix, Headless UI and every design system set `el.ariaExpanded = true` on a disclosure
//! rather than `setAttribute`, because the property is the typed, minifier-friendly, framework-bound
//! form. Absent, every one of those writes lands on a JS object and the agent (and the screen
//! reader) sees the state from before the interaction.
//!
//! ⚠⚠ **The name mapping is NOT camelCase→kebab-case, and assuming it is produces a plausible,
//! wrong implementation.** Chrome-measured:
//!
//! ```text
//!   ariaValueNow          -> aria-valuenow             NOT aria-value-now
//!   ariaPosInSet          -> aria-posinset             NOT aria-pos-in-set
//!   ariaRoleDescription   -> aria-roledescription      NOT aria-role-description
//!   ariaMultiSelectable   -> aria-multiselectable      NOT aria-multi-selectable
//!   role                  -> role                      (no prefix at all)
//! ```
//!
//! An auto-derived mapping passes every "is the property there?" test and writes attributes no
//! accessibility tree reads. Assertion (3) is that mutation, pinned.
//!
//! ⚠ These are `DOMString?` — **absent is `null`, not `""`** — and the distinction is load-bearing:
//! `el.ariaChecked ?? computeDefault()` and `if (el.role === null)` are how a library asks *"did the
//! author set this?"*, and `""` answers **yes** to both. Chrome-measured: absent → `null`
//! (`typeof` `object`), `= null` **removes** the attribute, `= ''` sets it present-and-empty.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="t">t</div>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     var NAMES = ['role','ariaAtomic','ariaAutoComplete','ariaBrailleLabel',
       'ariaBrailleRoleDescription','ariaBusy','ariaChecked','ariaColCount','ariaColIndex',
       'ariaColSpan','ariaCurrent','ariaDescription','ariaDisabled','ariaExpanded','ariaHasPopup',
       'ariaHidden','ariaInvalid','ariaKeyShortcuts','ariaLabel','ariaLevel','ariaLive','ariaModal',
       'ariaMultiLine','ariaMultiSelectable','ariaOrientation','ariaPlaceholder','ariaPosInSet',
       'ariaPressed','ariaReadOnly','ariaRelevant','ariaRequired','ariaRoleDescription',
       'ariaRowCount','ariaRowIndex','ariaRowSpan','ariaSelected','ariaSetSize','ariaSort',
       'ariaValueMax','ariaValueMin','ariaValueNow','ariaValueText'];
     var t = document.getElementById('t');
     var present = 0;
     for (var i = 0; i < NAMES.length; i++) { if (NAMES[i] in t) present++; }

     var fresh = document.createElement('div');
     var parts = ['present=' + present + '/' + NAMES.length];

     // Both directions, on the property a component library writes most.
     t.ariaLabel = 'Close';
     parts.push('fwd=' + t.getAttribute('aria-label'));
     t.setAttribute('aria-expanded', 'true');
     parts.push('back=' + t.ariaExpanded);

     // The mapping, on the four that a camelCase→kebab derivation gets WRONG.
     var m = document.createElement('div');
     m.ariaValueNow = '5'; m.ariaPosInSet = '2';
     m.ariaRoleDescription = 'r'; m.ariaMultiSelectable = 'true';
     parts.push('map=' + [m.getAttribute('aria-valuenow'), m.getAttribute('aria-posinset'),
                          m.getAttribute('aria-roledescription'),
                          m.getAttribute('aria-multiselectable')].join(','));
     // …and `role` carries NO prefix.
     m.role = 'button';
     parts.push('role=' + m.getAttribute('role') + '/' + String(m.getAttribute('aria-role')));

     // Nullability, all three states.
     parts.push('absent=' + String(fresh.ariaLabel) + '/' + typeof fresh.ariaLabel);
     fresh.ariaLabel = 'x'; fresh.ariaLabel = null;
     parts.push('setNull=' + fresh.hasAttribute('aria-label'));
     fresh.ariaLabel = '';
     parts.push('setEmpty=' + fresh.hasAttribute('aria-label') + '/' +
                JSON.stringify(fresh.getAttribute('aria-label')));

     document.getElementById('out').textContent = parts.join(' ');
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
fn the_aria_idl_properties_reflect_to_their_attributes() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://aria.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("ARIA-REFLECTION {got}");
    let has = |s: &str| got.contains(s);

    // (1) **All of them, not a popular subset.** Chrome has 42/42; a partial implementation is the
    // failure mode here, because a library that finds `ariaLabel` present reasonably assumes the rest
    // and gets a silent no-op on `ariaSetSize`. RED: remove the ARIA block from the reflect table →
    // `present=0/42`, which is what shipped.
    assert!(
        has("present=42/42"),
        "the ARIA IDL surface is incomplete — got {got:?}. A partial set is worse than none: a page \
         that finds `ariaLabel` assumes `ariaSetSize` and gets a silent no-op."
    );

    // (2) **Both directions.** A getter-only implementation satisfies (1) and never writes.
    assert!(
        has("fwd=Close") && has("back=true"),
        "ARIA properties must reflect BOTH ways — got {got:?}"
    );

    // (3) **THE MAPPING, and it is the mutation that matters.** camelCase→kebab is the obvious
    // derivation and it is wrong for every multi-word name: `aria-value-now` is not an attribute any
    // accessibility tree reads. RED: derive the attribute name instead of tabling it → `map=,,,`
    // (four nulls) while (1) and (2) still pass.
    assert!(
        has("map=5,2,r,true"),
        "the attribute names are wrong. ariaValueNow -> aria-valuenow (NOT aria-value-now), \
         ariaPosInSet -> aria-posinset, ariaRoleDescription -> aria-roledescription, \
         ariaMultiSelectable -> aria-multiselectable — got {got:?}"
    );
    assert!(
        has("role=button/null"),
        "`role` reflects to `role` with NO `aria-` prefix — got {got:?}"
    );

    // (4) **`DOMString?` — three distinct states.** RED: use the plain `string` type → absent reads
    // `""` instead of `null`, and `= null` writes the literal string `"null"` into an attribute a
    // screen reader would then announce.
    assert!(
        has("absent=null/object") && has("setNull=false") && has("setEmpty=true/\"\""),
        "nullable-string semantics are wrong: absent must be `null` (not `\"\"`), `= null` must \
         REMOVE the attribute, and `= ''` must leave it present and empty — got {got:?}"
    );
}
