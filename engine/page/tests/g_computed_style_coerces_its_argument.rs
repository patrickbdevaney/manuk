//! # G_COMPUTED_STYLE_COERCES_ITS_ARGUMENT — `getPropertyValue(x)` must not THROW when `x` is not a string
//!
//! **The failure this gate exists for: `getComputedStyle(el).getPropertyValue(0)` threw
//! `TypeError: p.charCodeAt is not a function`** — and a TypeError in a property read takes the
//! rest of the script with it.
//!
//! The computed-style object's `getPropertyValue` began by testing for a custom property:
//!
//! ```js
//! getPropertyValue: function (p) {
//!   if (p.charCodeAt(0) === 45 && p.charCodeAt(1) === 45) { … }   // "--" prefix?
//! ```
//!
//! `charCodeAt` only exists on strings. Per CSSOM the parameter is a `CSSOMString`, and WebIDL
//! **converts** whatever it is handed before the method body ever runs — so a number, `null` or an
//! object is a well-defined call that returns `""`, not an exception. This is the throw class:
//! the caller was not doing anything wrong, and the failure is not local to the property being
//! asked for.
//!
//! It matters because iterating a property list is normal: `props.forEach(p => cs.getPropertyValue(p))`
//! over an array that contains an index, a `null` hole, or a `String` wrapper is enough.
//!
//! ⚠ **The LIVE `el.style` path was measured and is NOT affected** — it already coerces. The bug is
//! specific to the computed-style object, and the gate asserts the live path too so a future
//! "unification" cannot regress the half that was already right.
//!
//! ## Honest size
//!
//! `css/css-values` **1697 → 1705 (+8)**, not the +40 the error-message count suggested: the 40
//! messages sat inside files whose other assertions still fail for unrelated reasons
//! (`calc-size()`, `random-item()` — unshipped CSS Values 5). **Message count is not flip count**,
//! the same gap t1190 recorded.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | remove the `p=String(p)` coercion | RED — `num:THREW null:THREW obj:THREW bool:THREW`, while `named`, `dashed` and every live-path claim stay green |
//!
//! ⚠ The probe also confirms why `wrapper` is worth its own claim: a `String` OBJECT passes even
//! WITHOUT the fix, because it really does have `charCodeAt`. It is the claim that discriminates
//! `String(p)` from a `typeof p === 'string'` guard, and only the latter would break it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="t" style="color: rgb(1, 2, 3)">x</div>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var cs = getComputedStyle(document.getElementById('t'));

    // ── 1. A non-string argument is CONVERTED, not rejected.
    try { p('num:[' + cs.getPropertyValue(0) + ']'); } catch (e) { p('num:THREW'); }
    try { p('null:[' + cs.getPropertyValue(null) + ']'); } catch (e) { p('null:THREW'); }
    try { p('obj:[' + cs.getPropertyValue({}) + ']'); } catch (e) { p('obj:THREW'); }
    try { p('bool:[' + cs.getPropertyValue(true) + ']'); } catch (e) { p('bool:THREW'); }

    // ── 2. A String OBJECT is a string after conversion — the case that makes `typeof p === 'string'`
    //    the WRONG guard and `String(p)` the right one.
    try { p('wrapper:[' + cs.getPropertyValue(new String('color')) + ']'); }
    catch (e) { p('wrapper:THREW'); }

    // ── 3. THE RATCHET CLAUSE — real lookups must be untouched by the coercion.
    p('named:' + cs.getPropertyValue('color'));
    p('unknown:[' + cs.getPropertyValue('no-such-property') + ']');
    p('dashed:[' + cs.getPropertyValue('--nope') + ']');

    // ── 4. The LIVE style path was already correct and must STAY correct.
    var st = document.getElementById('t').style;
    try { p('liveNum:[' + st.getPropertyValue(0) + ']'); } catch (e) { p('liveNum:THREW'); }
    try { p('liveRemove:[' + st.removeProperty(0) + ']'); } catch (e) { p('liveRemove:THREW'); }
    try { p('livePrio:[' + st.getPropertyPriority(0) + ']'); } catch (e) { p('livePrio:THREW'); }
    p('liveNamed:' + st.getPropertyValue('color'));
  </script>
</body></html>"##;

#[test]
fn get_property_value_converts_its_argument_to_a_string() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom2.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COERCE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_COMPUTED_STYLE_COERCES_ITS_ARGUMENT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "num:[]",
        "THE LOAD-BEARING CLAIM. `getPropertyValue(0)` threw `p.charCodeAt is not a function` — the \
         method called a String method on whatever it was handed. WebIDL converts a CSSOMString \
         parameter before the body runs, so this is a well-defined call returning the empty string. \
         A TypeError here takes the rest of the script with it",
    ),
    ("null:[]", "`null` converts to the string \"null\", which matches no property"),
    ("obj:[]", "and an object converts via toString — no argument type may throw"),
    ("bool:[]", "a fourth type, so the claim cannot be met by special-casing numbers"),
    (
        "wrapper:[rgb(1, 2, 3)]",
        "⚠ A `String` OBJECT converts to a real string and must RESOLVE, not merely avoid \
         throwing. This is why the fix is `String(p)` and not a `typeof p === 'string'` guard — \
         that guard would make this return the empty string and look like a pass",
    ),
    (
        "named:rgb(1, 2, 3)",
        "THE RATCHET CLAUSE. An ordinary named lookup is unchanged by the coercion",
    ),
    ("unknown:[]", "an unknown property is still the empty string, not an error"),
    (
        "dashed:[]",
        "and the custom-property branch — the `--` test that needed `charCodeAt` in the first \
         place — still works on a real string",
    ),
    (
        "liveNum:[]",
        "the LIVE `el.style` path was measured and already coerced correctly. Asserted here so a \
         later unification of the two paths cannot regress the half that was right",
    ),
    ("liveRemove:[]", "same for removeProperty"),
    ("livePrio:[]", "same for getPropertyPriority"),
    ("liveNamed:rgb(1, 2, 3)", "and the live path still answers a real lookup"),
];
