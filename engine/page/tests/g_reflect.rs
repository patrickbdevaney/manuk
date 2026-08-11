//! **G_REFLECT — HTML attribute reflection. The single largest gap in the platform, by a factor of five.**
//!
//! `a.href`. `input.disabled`. `img.width`. `td.colSpan`. Every one is a **reflected IDL attribute** — a
//! property that *is* a view over a content attribute, with the HTML spec's type coercion in between.
//! They were **all `undefined`**.
//!
//! Measured, not guessed: histogramming all 47,226 failing subtest messages in `html/dom` showed **~38,000
//! of them (80%) are this one mechanism.** The entire `dom/` suite that ten ticks were spent on is 6,484
//! subtests. This is five times that, behind a single generic feature.
//!
//! **And it is how ordinary page code touches the DOM.** `if (input.disabled)` reading `undefined` does not
//! throw — it silently takes the wrong branch.
//!
//! The assertions below are the *rules*, not the table, because the rules are the spec:
//!
//! * **boolean is PRESENCE, not value.** `el.disabled = false` must **remove** the attribute. Writing the
//!   string `"false"` — what stringifying does — leaves the element disabled with no way to tell.
//! * **URL resolves against the base.** `a.href` on `<a href="x">` is absolute.
//! * **an invalid `unsigned long` falls back to the default** — it is *not* clamped to zero.
//! * **`limited unsigned long`**: `colspan="0"` is invalid, so `colSpan` reads back as **1**.
//! * **an absent string is `""`**, never `null`.
//! * and the accessor is **inert on the wrong element** — `div.disabled` is `undefined`, not `false`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<a id="a" href="x" target="_blank"></a>
<input id="i" disabled maxlength="5">
<img id="m" width="300" height="-5">
<img id="lz" loading="lazy"><img id="bg" loading="BOGUS"><iframe id="fr" loading="lazy"></iframe>
<table><tr><td id="t" colspan="0"></td></tr></table>
<script>
  var R = [];
  var a = document.getElementById('a'), i = document.getElementById('i');
  var m = document.getElementById('m'), t = document.getElementById('t');

  R.push('str:' + a.target);
  R.push('absent:' + JSON.stringify(a.rel));          // "" — never null
  R.push('url:' + a.href);                            // resolved against the base
  R.push('bool:' + i.disabled);
  i.disabled = false;
  R.push('boolOff:' + i.hasAttribute('disabled'));    // REMOVED, not set to "false"
  i.disabled = true;
  R.push('boolOn:' + i.getAttribute('disabled'));     // present, empty value
  R.push('long:' + i.maxLength);
  R.push('ulong:' + m.width);
  m.width = 200;
  R.push('setBack:' + m.getAttribute('width'));       // the IDL set reaches the attribute
  R.push('negFallback:' + m.height);                  // height="-5" is invalid → default 0, NOT clamped
  R.push('limited:' + t.colSpan);                     // colspan="0" is invalid → 1
  R.push('inert:' + String(document.createElement('div').disabled));

  // ⚠⚠⚠ `loading` was on <video>/<audio> and MISSING from <img>/<iframe> — the two elements the
  //     platform puts it on, and `loading="lazy"` is on 49 of 385 corpus pages (12.7%). Chrome
  //     returns the legacy `auto` for both the missing and the invalid value, and `auto` is what is
  //     implemented (the north star makes a structural divergence from Chromium the bug).
  R.push('imgLoad:' + document.getElementById('lz').loading);       // "lazy"
  R.push('ifrLoad:' + document.getElementById('fr').loading);       // "lazy"
  R.push('imgLoadDflt:' + document.getElementById('m').loading);    // no attribute -> "auto"
  R.push('imgLoadBad:' + document.getElementById('bg').loading);    // invalid      -> "auto"
  document.getElementById('m').loading = 'lazy';
  R.push('imgLoadSet:' + document.getElementById('m').getAttribute('loading'));
  // …and it must stay INERT on the elements the platform does NOT put it on.
  R.push('vidLoad:' + String(document.createElement('video').loading));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn html_attributes_reflect_with_the_specs_type_coercion() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://reflect.test/p/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("str:_blank", "a plain string attribute reflected — every one of these was `undefined`"),
        ("absent:\"\"", "an ABSENT string reflects as `\"\"`, never `null`"),
        (
            "url:https://reflect.test/p/x",
            "a URL attribute is RESOLVED against the document base. `a.href` on `<a href=\"x\">` is \
             absolute in every browser, and code comparing it to `\"x\"` never worked",
        ),
        ("bool:true", "boolean reflection is `hasAttribute` — `disabled=\"\"` is true"),
        (
            "boolOff:false",
            "**`el.disabled = false` must REMOVE the attribute.** Stringifying writes `\"false\"`, which \
             leaves the element disabled and gives the page no way to tell",
        ),
        ("boolOn:", "…and setting it true writes presence with an empty value"),
        ("long:5", "an integer attribute is parsed"),
        ("ulong:300", "…and so is an unsigned one"),
        ("setBack:200", "**the IDL SET must reach the content attribute** — 13,724 subtests said it did not"),
        (
            "negFallback:0",
            "a NEGATIVE value in an unsigned field is INVALID and falls back to the default. It is *not* \
             clamped to zero, which is the intuitive-and-wrong thing to do",
        ),
        (
            "limited:1",
            "`limited unsigned long`: `colspan=\"0\"` is invalid, so `colSpan` reads back as **1**, not 0",
        ),
        ("imgLoad:lazy", "`<img loading=\"lazy\">.loading` — the commonest lazy-load idiom on the web, and the IDL attribute did not exist"),
        ("ifrLoad:lazy", "…and the same on `<iframe>`, the other element the platform puts it on"),
        (
            "imgLoadDflt:auto",
            "with no attribute Chrome returns the legacy `auto`, not the spec's `eager` — measured, \
             and implemented because a structural divergence from Chromium is the bug",
        ),
        ("imgLoadBad:auto", "an invalid keyword falls back to the same value as a missing one"),
        ("imgLoadSet:lazy", "and the IDL SET reaches the content attribute"),
        (
            "vidLoad:undefined",
            "**`<video>` has no `loading` and we invented one** — a FALSE PRESENCE, which is the \
             direction that makes a feature-detecting page take the unsupported branch",
        ),
        (
            "inert:undefined",
            "and the accessor is INERT on an element that does not reflect it — `div.disabled` is \
             `undefined`, not `false`. A shared prototype must not hand every element every attribute",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_REFLECT: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
