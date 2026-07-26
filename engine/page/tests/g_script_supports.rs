//! **G_SCRIPT_SUPPORTS — the static feature-detect a page calls to decide how to load its own code,
//! and its absence is a THROW at exactly the moment the page was trying to degrade gracefully.**
//!
//! `HTMLScriptElement.supports(type)` is how a page chooses between an ES-module bundle and a classic
//! fallback, or decides whether emitting an import map is worthwhile. Calling a static that does not
//! exist is `TypeError: HTMLScriptElement.supports is not a function` — so the page does not take the
//! fallback branch, it **dies at the feature-detect**. That is the worst possible failure for a call
//! whose whole purpose is to let a page degrade.
//!
//! Third rung of `www.welt.de`'s chain, after t612's `innerText` setter and t613's XHR EventTarget.
//!
//! **The answers must be TRUE, not flattering.** `speculationrules` is a prefetch hint we do not
//! implement, and a page asking about it is *asking to be told no* so it can prefetch by itself. A
//! wrong `true` there is the honest-answer trap in its purest form: the page would then do nothing,
//! and neither would we. [[honest-answer-is-not-a-fixed-answer]]

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
<script>
  var R = [];

  // ── It EXISTS, and calling it does not throw. This is welt.de's rung.
  (function () {
    'use strict';
    try {
      HTMLScriptElement.supports('module');
      R.push('threw:false');
    } catch (e) { R.push('threw:true'); }
  })();

  R.push('isFn:' + (typeof HTMLScriptElement.supports));
  // STATIC, on the interface object — not an instance method. A page calls it without a <script>.
  R.push('onInstance:' + (typeof document.createElement('script').supports));

  // ── The answers, each of which this engine can actually back.
  R.push('classic:' + HTMLScriptElement.supports('classic'));
  R.push('module:' + HTMLScriptElement.supports('module'));
  R.push('importmap:' + HTMLScriptElement.supports('importmap'));

  // ── And the honest NO. A capability we do not have must answer false, or the page skips the
  // fallback it was about to write for itself.
  R.push('speculationrules:' + HTMLScriptElement.supports('speculationrules'));
  R.push('nonsense:' + HTMLScriptElement.supports('definitely-not-a-script-type'));

  // ── It returns a real boolean, not a truthy string — `=== true` is how feature detects are written.
  R.push('strictTrue:' + (HTMLScriptElement.supports('module') === true));
  R.push('strictFalse:' + (HTMLScriptElement.supports('nope') === false));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"#;

#[test]
fn script_supports_answers_truthfully_and_does_not_throw() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://s.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // the headline: the feature-detect must not be the thing that breaks the page
        "threw:false",
        "isFn:function",
        // static on the interface object, absent on instances
        "onInstance:undefined",
        // the three we can genuinely back — classic scripts, ES modules (t512-516), import maps
        "classic:true",
        "module:true",
        "importmap:true",
        // the honest NO — a page asking about speculationrules is asking to be told no
        "speculationrules:false",
        "nonsense:false",
        // real booleans, because `=== true` is how these are actually written
        "strictTrue:true",
        "strictFalse:true",
    ] {
        assert!(
            got.contains(claim),
            "G_SCRIPT_SUPPORTS: expected `{claim}`\n  got: {got}\n\n  \
             `HTMLScriptElement.supports` is a page's way of asking how to load its own code. If it \
             does not exist the call is a TypeError and the page dies AT THE FEATURE DETECT, instead \
             of taking the fallback it was reaching for. And `speculationrules:false` is load-bearing \
             in the other direction: answering `true` for something we do not implement means the page \
             stops prefetching and we never start — a capability claimed is a capability nobody \
             provides."
        );
    }
}
