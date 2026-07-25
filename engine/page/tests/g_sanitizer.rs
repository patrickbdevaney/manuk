//! **G_SANITIZER — the Sanitizer API (`Element.setHTML` / `Element.setHTMLUnsafe`).**
//!
//! The safe way to inject untrusted markup — a comment body, a CMS-authored field, pasted rich text.
//! `el.innerHTML = untrusted` is an XSS hole; `el.setHTML(untrusted)` is the modern replacement that
//! parses the string AND removes the parts that turn markup into code. It was ABSENT, so a page that
//! reached for it got `el.setHTML is not a function` and either crashed the injection path or fell
//! back to the unsafe one.
//!
//! The gate has real teeth — a stub that just aliases `innerHTML` FAILS it:
//!   * `script-gone` — a `<script>` in the string must NOT survive in the DOM (`setHTML` != innerHTML).
//!   * `handler-gone` — an `onerror=`/`onclick=` content attribute is stripped from every element
//!     (`<img src=x onerror=...>` is the canonical payload).
//!   * `jsurl-gone` — a `javascript:` href is removed.
//!   * `safe-kept` — ordinary markup (`<b>`, text, a normal `href`) is PRESERVED — sanitizing is not
//!     deleting everything.
//!   * `unsafe-keeps-script` — `setHTMLUnsafe` is the explicit opt-out and DOES keep the `<script>`
//!     element (it is `innerHTML` with an honest name), proving the two are genuinely different.
//!
//! Proven RED: alias `setHTML` to the `innerHTML` setter (skip `sanitize_subtree`) and `script-gone`
//! / `handler-gone` / `jsurl-gone` all fail; drop the whole registration and every claim throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="safe"></div>
<div id="unsafe"></div>
<div id="cfg"></div>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }

var payload = '<b>bold</b>' +
              '<script>window.__pwned = 1;<' + '/script>' +
              '<img src="x" onerror="window.__pwned = 2">' +
              '<a href="javascript:window.__pwned=3" id="lnk">click</a>' +
              '<a href="/safe/path" id="ok">ok</a>';

try {
  var safe = document.getElementById('safe');
  safe.setHTML(payload);

  // the script element must be gone from the sanitized subtree
  push('script-gone:' + (safe.getElementsByTagName('script').length === 0));

  // the event-handler attribute must be stripped off the <img>
  var img = safe.getElementsByTagName('img')[0];
  push('handler-gone:' + (img != null && !img.hasAttribute('onerror')));

  // the javascript: href must be removed, but a normal href kept
  var lnk = safe.querySelector('#lnk');
  var ok = safe.querySelector('#ok');
  push('jsurl-gone:' + (lnk != null && !lnk.hasAttribute('href')));
  push('safe-kept:' + (safe.getElementsByTagName('b').length === 1 &&
                       ok != null && ok.getAttribute('href') === '/safe/path'));

  // setHTMLUnsafe is the opt-out: it KEEPS the script element (innerHTML with an honest name).
  var un = document.getElementById('unsafe');
  un.setHTMLUnsafe(payload);
  push('unsafe-keeps-script:' + (un.getElementsByTagName('script').length === 1));

  // Sanitizer CONFIG: removeElements is a block-list applied ON TOP of the baseline. With
  // removeElements:['img'] the <img> is removed too, the safe <b> is KEPT, and the baseline still
  // strips <script>. RED-prover: if the config is ignored the <img> survives (cfg-removes-img fails).
  var cfg = document.getElementById('cfg');
  cfg.setHTML(payload, { sanitizer: { removeElements: ['img'] } });
  push('cfg-removes-img:' + (cfg.getElementsByTagName('img').length === 0));
  push('cfg-keeps-safe:' + (cfg.getElementsByTagName('b').length === 1));
  push('cfg-baseline-still:' + (cfg.getElementsByTagName('script').length === 0));
} catch (e) {
  push('THREW:' + e);
}
document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

#[test]
fn set_html_sanitizes_and_set_html_unsafe_does_not() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://sanitize.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SANITIZER RESULT: {got}");

    for claim in [
        "script-gone:true", // <script> removed — the whole point of setHTML over innerHTML
        "handler-gone:true", // onerror= stripped from <img>
        "jsurl-gone:true",  // javascript: href removed
        "safe-kept:true",   // <b> and a normal href preserved — not delete-everything
        "unsafe-keeps-script:true", // setHTMLUnsafe is the real opt-out, genuinely different
        "cfg-removes-img:true", // Sanitizer config removeElements block-list removes the <img>
        "cfg-keeps-safe:true", // …while keeping safe <b> (block-list, not delete-everything)
        "cfg-baseline-still:true", // …and the always-on baseline still strips <script>
    ] {
        assert!(
            got.contains(claim),
            "G_SANITIZER: expected `{claim}`\n  got: {got}\n\n  \
             `Element.setHTML` must parse AND sanitize (strip <script>, on* handlers, javascript: \
             URLs) while keeping safe markup; `setHTMLUnsafe` must be the opt-out that keeps them. A \
             stub that aliases either to innerHTML fails the `*-gone` teeth."
        );
    }
}
