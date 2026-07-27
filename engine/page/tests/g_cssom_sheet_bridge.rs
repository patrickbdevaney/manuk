//! **G_CSSOM_SHEET_BRIDGE — a rule inserted through `styleEl.sheet.insertRule()` must reach the
//! cascade and move the box.**
//!
//! This is the CSS-in-JS runtime path: styled-components, emotion and every `<style>`-injecting
//! library create a `<style>`, take its `.sheet`, and call `insertRule` once per generated class. It
//! is how a large fraction of the app web gets its styling at all.
//!
//! **It was `undefined`, and `undefined` is not the spec's absent value.** `HTMLStyleElement.sheet` is
//! typed `CSSStyleSheet?`, so the guard every consumer writes is `if (el.sheet === null)`. Against
//! `undefined` that guard is **false** and the code proceeds into the thing it just checked for.
//! Measured at tick 663:
//!
//! ```text
//!   typeof el.sheet  undefined   ·  el.sheet === null  false   ·  typeof CSSStyleSheet  function
//!   document.styleSheets.length / el.sheet.cssRules / insertRule   ALL THREW TypeError
//! ```
//!
//! `typeof CSSStyleSheet === "function"` was **already true** — the false-presence shape, where every
//! feature detect passes and the page walks into the gap one line later. `www.agoda.com` renders blank
//! behind exactly this: its located stack (tick 662) reads `insertRules → getTag → this.sheet`, then
//! `.length` on `undefined`.
//!
//! **The claim that matters is the CASCADE, not the API.** An object with an `insertRule` that returns
//! cleanly and changes nothing would satisfy every shape test a library performs and still render the
//! wrong page — which is the failure this project names first in its own lesson list (*a gate that does
//! not measure what the user feels reports green while the user suffers*). So the load-bearing
//! assertion here is a **box width**, and the rule that produces it is inserted at runtime into a
//! `<style>` that did not exist when the document was parsed.
//!
//! Hermetic: one inline document, no network.

use manuk_text::FontContext;

/// A width no default, no UA rule and no other rule in the fixture produces.
const INJECTED_WIDTH: i64 = 456;
/// The width the authored sheet gives before anything is injected.
const AUTHORED_WIDTH: i64 = 111;

const HTML: &str = r##"<!doctype html><html><head><style id="authored">#a { width: 111px }
@media screen { #m { width: 50px } }
#c { width: 33px }</style></head><body>
<div id="a">x</div><div id="m">y</div><div id="c">z</div>
<script>
  function mark(id) { var d = document.createElement('div'); d.id = id; document.body.appendChild(d); }
  var authored = document.getElementById('authored');

  // 1. The surface exists at all, and the tag guard holds.
  mark('T__typeof_' + (typeof authored.sheet));
  mark('T__identity_' + (authored.sheet === authored.sheet));
  mark('T__divsheet_' + (typeof document.getElementById('a').sheet));

  // 2. cssRules counts a nested @media block as ONE rule, not three. A naive `split('}')` gets the
  //    right answer for flat sheets and silently shreds every responsive one.
  mark('T__rules_' + authored.sheet.cssRules.length);
  mark('T__selector_' + (authored.sheet.cssRules[0].selectorText === '#a'));

  // 3. document.styleSheets before anything is injected — the baseline for the liveness check below.
  mark('T__docsheets_before_' + document.styleSheets.length);

  // 4. THE CSS-IN-JS PATH: a <style> that did not exist at parse time, a rule inserted through its
  //    sheet, and the box must move.
  var injected = document.createElement('style');
  document.head.appendChild(injected);
  injected.sheet.insertRule('#a { width: 456px }', injected.sheet.cssRules.length);

  // 4b. …and the list is LIVE: a <style> appended after load is in it, with no cache to invalidate.
  //     Read AFTER the injection on purpose — the first draft of this gate read it before, reported
  //     1, and was asserting 2. The gate caught its own fixture.
  mark('T__docsheets_after_' + document.styleSheets.length);

  // 5. deleteRule must un-cascade, or "it cascaded" could mean "text was appended and never removed".
  injected.sheet.insertRule('#c { width: 999px }', injected.sheet.cssRules.length);
  injected.sheet.deleteRule(injected.sheet.cssRules.length - 1);

  // 6. Past-the-end insertRule THROWS, per spec — a CSS-in-JS runtime uses that to discover its own
  //    bookkeeping is wrong, and silently clamping would hide a library bug inside a browser bug.
  try { injected.sheet.insertRule('#c{width:1px}', 99); mark('T__oob_NOTHROWN'); }
  catch (e) { mark('T__oob_threw'); }
</script>
</body></html>"##;

/// Every `#id` in the laid-out tree, with its width.
fn boxes(page: &manuk_page::Page) -> std::collections::HashMap<String, i64> {
    let rects = page.root_box.node_rects(page.dom());
    let mut out = std::collections::HashMap::new();
    for n in page.dom().flat_descendants(page.dom().root()) {
        if let Some(el) = page.dom().element(n) {
            if let Some(id) = el.attr("id") {
                if let Some(r) = rects.get(&n) {
                    out.insert(id.to_string(), r.width.round() as i64);
                }
            }
        }
    }
    out
}

#[test]
fn a_rule_inserted_through_the_sheet_reaches_the_cascade() {
    let tmp = std::env::temp_dir().join(format!("manuk-cssom-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://example.invalid/index.html", &fonts, 800.0);
    let b = boxes(&page);
    let mut marks: Vec<&String> = b.keys().filter(|k| k.starts_with("T__")).collect();
    marks.sort();
    println!("CSSOM PROBE: {marks:?}");
    let has = |m: &str| b.contains_key(m);

    // ── PRECONDITION: the script ran at all. Without this every assertion below is about a page
    //    whose JavaScript never executed, which would fail for the wrong reason and read as this one.
    assert!(
        marks.iter().any(|m| m.starts_with("T__typeof_")),
        "the gate's own fixture failed: no probe marks exist, so the page's script never ran and \
         nothing below is a measurement of the CSSOM.\n  marks: {marks:?}"
    );

    // ── THE SURFACE.
    assert!(
        has("T__typeof_object"),
        "`styleEl.sheet` is not an object. It was `undefined` — and `undefined` is not the spec's \
         absent value, so `if (el.sheet === null)` passes and the page dies one line later on the \
         thing it just checked for.\n  marks: {marks:?}"
    );
    assert!(
        has("T__identity_true"),
        "`el.sheet !== el.sheet` — a fresh object per read. Every CSSOM consumer assumes identity, \
         and a library that stashes bookkeeping on the sheet loses it.\n  marks: {marks:?}"
    );
    assert!(
        has("T__divsheet_undefined"),
        "a non-`<style>` element reports a sheet — the tag guard is not holding, so `<div>.sheet` \
         hands back an object for an element that has no stylesheet.\n  marks: {marks:?}"
    );
    assert!(
        has("T__rules_3") && has("T__selector_true"),
        "`cssRules` does not report the authored sheet's THREE rules with `#a` first. A nested \
         `@media` block is ONE rule: a naive close-brace split gets flat sheets right and shreds \
         every responsive one, which is why the splitter tracks brace DEPTH.\n  marks: {marks:?}"
    );
    assert!(
        has("T__docsheets_before_1") && has("T__docsheets_after_2"),
        "`document.styleSheets` is not a LIVE list: it must report the one authored sheet before the \
         injection and BOTH after it, with nothing invalidating a cache in between. It was \
         `undefined`, so reading `.length` THREW rather than reporting a number.\n  marks: {marks:?}"
    );
    assert!(
        has("T__oob_threw"),
        "`insertRule` past the end did not throw. The spec throws IndexSizeError there and a \
         CSS-in-JS runtime uses it to discover its own bookkeeping is wrong; clamping silently hides \
         a library bug inside a browser bug.\n  marks: {marks:?}"
    );

    // ── THE CLAIM, and it is a BOX, not an API shape. An `insertRule` that returns cleanly and
    //    changes nothing would satisfy every shape test above and still render the wrong page.
    assert_eq!(
        b.get("a").copied(),
        Some(INJECTED_WIDTH),
        "G_CSSOM_SHEET_BRIDGE: `#a` is not {INJECTED_WIDTH}px, so a rule inserted at runtime through \
         `styleEl.sheet.insertRule()` — into a `<style>` that did not exist when the document was \
         parsed — never reached the cascade. That is the CSS-in-JS path: styled-components, emotion \
         and every `<style>`-injecting library style the app this way, and `www.agoda.com` renders \
         blank without it."
    );

    // ── AND deleteRule MUST UN-CASCADE, or "the rule applied" could just mean "text was appended".
    assert_eq!(
        b.get("c").copied(),
        Some(33),
        "`#c` is not back at its authored 33px — `deleteRule` did not remove the 999px rule it was \
         given, so the bridge can add style and not take it away, and every CSS-in-JS unmount leaks \
         its rules forever."
    );
    // The authored sheet is untouched by all of this — a bridge that rewrote the wrong element's
    // text would still pass everything above.
    assert_eq!(
        b.get("m").copied(),
        Some(50),
        "the authored sheet's `@media` rule stopped applying — the bridge rewrote a sheet it was not \
         asked to touch."
    );
    assert_ne!(
        b.get("a").copied(),
        Some(AUTHORED_WIDTH),
        "`#a` still has its authored width, which means the injected rule lost the cascade rather \
         than reaching it."
    );
}
