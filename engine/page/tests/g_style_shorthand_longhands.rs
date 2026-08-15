//! **G_STYLE_SHORTHAND_LONGHANDS — a shorthand sets its longhands, and `el.style` never did it.**
//!
//! CSSOM §6.7: a declaration block exposes an IDL attribute for every supported property, and the
//! block's setter for a shorthand *"sets the longhand properties"*. So `el.style.margin = '1px 2px'`
//! followed by `el.style.marginTop` is `'1px'` in every browser.
//!
//! `el.style` here is a Proxy over the style **attribute**, parsed into a flat dict keyed by the
//! name the author wrote. A shorthand stored ONE entry under its own name, so every longhand read
//! answered `''` — through `cssText`, the IDL setter, `setProperty` and `setAttribute('style', …)`
//! alike, because all four land in the same dict.
//!
//! ⚠⚠⚠ **THE CONTROL ROW IS WHAT SAID THIS WAS NOT A GRID BUG.** It was found probing
//! `css/css-grid/parsing`, whose top failure signature is `e.style.cssText = grid …` reading back
//! `""`. The probe carried a `margin: 1px 2px` → `marginTop` control row on the theory that the
//! `grid` shorthand simply was not parsed — and the control failed identically. Layout was never
//! involved: the same document lays a `grid: 150px 100px / 200px 300px` container out correctly,
//! byte-for-byte against the longhand spelling. **The bug is every shorthand on the web**:
//! `margin`, `padding`, `border`, `background`, `font`, `flex`, `gap`, `inset`, `grid`,
//! `place-items`, `transition`, `animation`.
//!
//! ⚠ The reach is not conformance trivia. `el.style.marginTop` after setting `margin` is what
//! measurement code in every UI library reads; `getPropertyValue('border-top-width')` after `border`
//! is what layout shims read. An empty string parses as `NaN` downstream, and the caller lays out
//! against it.
//!
//! ⚠⚠ **THE EXPANSION IS STYLO'S, ASKED FOR — NOT REIMPLEMENTED.** `parse_style_attribute` has
//! ALREADY expanded the shorthand by the time it returns; the longhand values were sitting in the
//! block and nothing asked. `stylo_engine::expand_declaration` enumerates them with
//! `ShorthandId::longhands()` and serializes each through the SAME `property_value_to_css` that
//! `serialize_declaration` uses, so a longhand reads byte-identically whether the author wrote the
//! longhand or the shorthand. A second expansion table would have been a second answer.
//!
//! ⚠ It is a READ-side overlay, not a rewrite of what is stored: `cssText` still round-trips the
//! author's own text and `length`/`item(i)` still enumerate what was declared. Assertions (5) and
//! (6) pin exactly that, because a fix that expanded into storage passes (1)–(4) and silently
//! changes three other surfaces.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="a">-</div><div id="b">-</div><div id="order">-</div><div id="keep">-</div>
 <script>
   window.addEventListener('load', function () {
     var d = document.createElement('div');
     var fresh = function (css) { d.style.cssText = ''; if (css) d.style.cssText = css; return d; };

     // (1) Every route into the declaration block, one shorthand, one longhand read.
     var r = [];
     fresh('margin: 1px 2px');                       r.push(d.style.marginTop);
     fresh(); d.style.margin = '1px 2px';            r.push(d.style.marginTop);
     fresh(); d.style.setProperty('margin', '1px 2px'); r.push(d.style.marginTop);
     fresh(); d.setAttribute('style', 'margin: 1px 2px'); r.push(d.style.marginTop);
     fresh('margin: 1px 2px');                       r.push(d.style.getPropertyValue('margin-top'));
     document.getElementById('a').textContent = r.join(',');

     // (2) Shorthand FAMILIES, not just `margin` — the four-sided one, the multi-property one, the
     //     numeric one, and the grid one this was found through.
     var s = [];
     fresh('border: 1px solid red');                 s.push(d.style.borderTopWidth);
     fresh('flex: 1');                               s.push(d.style.flexGrow);
     fresh('gap: 10px 20px');                        s.push(d.style.rowGap);
     fresh('grid: 150px 100px / 200px 300px');       s.push(d.style.gridTemplateRows);
     fresh('grid-row: 1 / 3');                       s.push(d.style.gridRowStart);
     document.getElementById('b').textContent = s.join('|');

     // (3) DECLARATION ORDER — the later declaration wins, whichever spelling it is.
     var o = [];
     fresh('margin: 5px; margin-top: 9px');          o.push(d.style.marginTop);
     fresh('margin-top: 9px; margin: 5px');          o.push(d.style.marginTop);
     document.getElementById('order').textContent = o.join(',');

     // (4) What must NOT change: the shorthand still reads back, `cssText` round-trips the author's
     //     own text, `length` counts DECLARATIONS not longhands, and an unset longhand is ''.
     var k = [];
     fresh('margin: 1px 2px');                       k.push(d.style.margin);
     fresh('margin: 1px 2px');                       k.push(d.style.cssText);
     fresh('margin: 1px 2px');                       k.push(String(d.style.length));
     fresh('color: red');                            k.push('[' + d.style.marginTop + ']');
     fresh(); d.style.marginTop = '1px';             k.push(d.style.marginTop);
     document.getElementById('keep').textContent = k.join('|');
   });
 </script>
</body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_shorthand_written_to_el_style_reads_back_through_every_longhand_it_sets() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://shorthand.test/",
        &fonts,
        800.0,
    ));

    let a = text(&page, "#a");
    let b = text(&page, "#b");
    let order = text(&page, "#order");
    let keep = text(&page, "#keep");
    println!("SHORTHAND a=[{a}] b=[{b}] order=[{order}] keep=[{keep}]");

    // (1) **All four write routes reach the longhand, and so does `getPropertyValue`.**
    // RED: revert `read` to `ser(k, stripImp(parse()[k] || ''))` → `,,,,` (five empty strings).
    assert_eq!(
        a, "1px,1px,1px,1px,1px",
        "cssText / IDL setter / setProperty / setAttribute / getPropertyValue must ALL expose the \
         longhand a shorthand sets — got {a:?}. All-empty is the pre-fix behaviour: the block stored \
         one entry under `margin` and `margin-top` was a different key."
    );

    // (2) **Four unrelated shorthand families**, so this is the CSSOM rule and not a `margin` patch.
    // RED: same revert → `||||`.
    assert_eq!(
        b, "1px|1|10px|150px 100px|1",
        "border / flex / gap / grid / grid-row must each expose their longhands — got {b:?}"
    );

    // (3) **DECLARATION ORDER.** The second row is the one that fails if the read prefers a direct
    // entry over the expansion; the first fails if it prefers the expansion over a direct entry.
    // Only an in-order merge satisfies both, which is why both rows are here.
    assert_eq!(
        order, "9px,5px",
        "the LATER declaration wins whichever spelling it is: `margin:5px; margin-top:9px` is 9px \
         and `margin-top:9px; margin:5px` is 5px — got {order:?}"
    );

    // (4) **THE FOUR CONTROL ROWS — what an over-eager fix breaks.** Expanding into STORAGE instead
    // of over the read satisfies (1)–(3) and changes every one of these: the shorthand read, the
    // `cssText` round-trip, the declaration count, and the empty answer for a property nobody set.
    assert_eq!(
        keep, "1px 2px|margin: 1px 2px|1|[]|1px",
        "CONTROLS: the shorthand still reads back, `cssText` round-trips the author's own text, \
         `length` counts DECLARATIONS not longhands, an unset longhand is empty, and a directly-set \
         longhand is unaffected — got {keep:?}"
    );
}
