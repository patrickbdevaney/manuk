//! # G_CSSRULELIST_IS_LIVE_AND_STABLE — `sheet.cssRules` is ONE list that updates, not a new one each read
//!
//! **The failure this gate exists for: `const {cssRules} = sheet;` then `sheet.insertRule(…)` and
//! `cssRules.length` is still 0 — forever.**
//!
//! `cssRules` re-derived its list from the `<style>` element's `textContent` on every read. That
//! makes it correctly **live**, and it is why `sheet.cssRules.length` looked right in every casual
//! check. But it minted a **new array object each time**, so:
//!
//! * `sheet.cssRules !== sheet.cssRules`, which the spec forbids — a `CSSRuleList` has identity;
//! * anything that **binds the list once** and reads it later sees a snapshot frozen at bind time.
//!
//! Binding it once is the normal idiom, not an exotic one. WPT's own `parsing-testcommon.js` does
//! exactly that, which is why **201 of the 392 subtests in `css/selectors/parsing` failed on
//! `assert_equals: Sheet should have 1 rule expected 1 but got 0`** — with the selector engine
//! innocent and `insertRule` working perfectly. A histogram of those failures reads as a list of
//! unsupported selectors (`.pastoral`, `body > p`, `[att]`, `h1 em`); **it is nothing of the kind,
//! and the tell is that those are the most basic selectors in CSS.** An engine that could not parse
//! `.pastoral` would not render a single page on the web.
//!
//! ## The principle was already written down — one level too shallow
//!
//! `el.sheet` is cached per element, with the comment *"ONE object per element: `el.sheet ===
//! el.sheet` is an assumption every CSSOM consumer makes"*. That is the same argument this gate
//! makes about `cssRules`, and it had not been carried down to the rule list.
//!
//! ## Both halves, because either alone is a wrong fix
//!
//! Caching the list without refreshing it makes `cssRules` **dead** — a sheet mutated through the
//! element's text (which is how `insertRule`/`deleteRule` are implemented here) would stop
//! reporting. Refreshing without caching is the bug being fixed. The gate asserts identity AND
//! liveness against every mutation path.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | restore `cssRules` to building a fresh array per read (the t1190 state) | RED — `capturedAfterInsert:0`, `identity:false`, `capturedAfterDelete:1`, `capturedAfterText:0` |
//! | cache the list but skip the in-place refresh | RED — `freshAfterInsert:0` and `capturedAfterText:0`; a dead list is not the fix |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head></head><body>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }

    var st = document.createElement('style');
    document.head.append(st);
    var sheet = st.sheet;

    // ── 1. IDENTITY. A CSSRuleList is an object, not a value.
    p('identity:' + (sheet.cssRules === sheet.cssRules));

    // ── 2. THE CAPTURED REFERENCE — WPT's `parsing-testcommon.js` idiom, transcribed.
    var cssRules = sheet.cssRules;
    p('capturedEmpty:' + cssRules.length);
    sheet.insertRule('.pastoral{}');
    p('capturedAfterInsert:' + cssRules.length);
    p('freshAfterInsert:' + sheet.cssRules.length);
    p('stillSame:' + (cssRules === sheet.cssRules));
    p('selText:' + cssRules[0].selectorText);

    // ── 3. DELETION must shrink the SAME list, not leave a stale tail.
    sheet.insertRule('.marine{}', 1);
    p('capturedTwo:' + cssRules.length);
    sheet.deleteRule(0);
    p('capturedAfterDelete:' + cssRules.length);

    // ── 4. THE ELEMENT'S TEXT IS THE SOURCE OF TRUTH, so writing it must still show through —
    //    this is the half a naive "just cache it" fix breaks. ⚠ A raw text write runs no accessor
    //    of ours, so it is observed on the next READ of the sheet; the bound list is the SAME
    //    object and reports the new count from that point on. Both are asserted.
    st.textContent = 'a{} b{} c{}';
    p('freshAfterText:' + sheet.cssRules.length);
    p('capturedAfterText:' + cssRules.length);
    p('sameAfterText:' + (cssRules === sheet.cssRules));

    // ── 5. `item()` survives the refresh, and `rules` is the same list (legacy IE alias).
    p('item:' + (cssRules.item(0) === cssRules[0]) + ':' + (cssRules.item(9) === null));
    p('rulesAlias:' + (sheet.rules === sheet.cssRules));

    // ── 6. THE RATCHET CLAUSE. Two different <style> elements must NOT share a list.
    var st2 = document.createElement('style');
    document.head.append(st2);
    st2.textContent = 'z{}';
    p('perElement:' + (st2.sheet.cssRules !== cssRules) + ':' + st2.sheet.cssRules.length);
  </script>
</body></html>"##;

#[test]
fn cssrules_is_one_list_that_updates_in_place() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CSSRULELIST: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_CSSRULELIST_IS_LIVE_AND_STABLE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "capturedAfterInsert:1",
        "THE LOAD-BEARING CLAIM, and it is WPT's `parsing-testcommon.js` idiom transcribed: bind \
         the list once, mutate the sheet, read the bound list. It reported 0 forever, which \
         presented as 201 selectors being 'invalid' — including `.pastoral` and `body > p`",
    ),
    (
        "identity:true",
        "`sheet.cssRules === sheet.cssRules`. The spec gives a CSSRuleList identity; a fresh array \
         per read fails this, and every library that stashes bookkeeping on the list loses it",
    ),
    (
        "capturedEmpty:0",
        "the bound list starts empty — stated so `capturedAfterInsert` is proven to be a CHANGE \
         rather than a constant that happened to read 1",
    ),
    (
        "freshAfterInsert:1",
        "and a fresh read agrees with the bound one. This is the claim a 'cache it and never \
         refresh' fix passes while breaking liveness — it is here to pair with capturedAfterText",
    ),
    ("stillSame:true", "identity survives a mutation, not just repeated reads"),
    (
        "selText:.pastoral",
        "the refreshed list holds real rules, not empty slots — a length that updates while the \
         contents do not would satisfy every count above",
    ),
    ("capturedTwo:2", "an indexed insert grows the same list"),
    (
        "capturedAfterDelete:1",
        "and a delete SHRINKS it. In-place refresh must truncate `length`; assigning indices \
         without truncating leaves a stale tail that reads as a rule that no longer exists",
    ),
    (
        "freshAfterText:3",
        "⚠ THE HALF A NAIVE CACHE BREAKS. The element's `textContent` is the single source of \
         truth here — `insertRule` is implemented by rewriting it — so a list cached and never \
         refreshed would report the pre-write count and be DEAD. Live and stable are both required",
    ),
    (
        "capturedAfterText:3",
        "and the BOUND list reports the new count too, because it is the same object the refresh \
         wrote through. ⚠ NAMED LIMIT, measured not assumed: a raw `textContent` write runs no \
         accessor of ours, so this becomes true at the next read of the sheet rather than at the \
         instant of the write — the two mutation paths that ARE ours (`insertRule`/`deleteRule`) \
         push the update immediately, which is what the captured-reference idiom needs",
    ),
    ("sameAfterText:true", "identity survives a source-text rewrite as well"),
    (
        "item:true:true",
        "`item()` survives the refresh and still returns null past the end (not undefined)",
    ),
    ("rulesAlias:true", "the legacy `rules` alias is the same list, not a second one"),
    (
        "perElement:true:1",
        "THE RATCHET CLAUSE. Caching must be per sheet. A single shared list would satisfy every \
         claim above and make every stylesheet on the page report the same rules",
    ),
];
