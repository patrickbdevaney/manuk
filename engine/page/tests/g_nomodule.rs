//! **G_NOMODULE — a module-capable browser must NOT run the `nomodule` half of a build.**
//!
//! ⚠⚠⚠ **THIS IS THE ANGULAR CLI SHAPE, AND RUNNING BOTH HALVES IS NOT A SLOW PAGE — IT IS A BROKEN
//! ONE.** Differential loading ships every bundle twice and lets the browser pick exactly one:
//!
//! ```html
//!   <script src="main-es2015.…js" type="module"></script>
//!   <script src="main-es5.…js"    nomodule defer></script>
//! ```
//!
//! A browser that understands `type="module"` runs the first and **skips the second**; one that does
//! not sees an unknown `type`, skips the first, and runs the second. Two mutually exclusive rules,
//! and this engine honoured only one of them — `nomodule` appeared nowhere in the tree — so it ran
//! **both halves of the same application**: two framework runtimes racing over one root element.
//!
//! Found on `pogoda.by` (t864), an Angular app that ships `runtime`/`polyfills`/`main` in both
//! flavours and rendered nothing here. The pattern is not one site's: it is what `ng build` emitted
//! by default for years, and what Vite's legacy plugin and the webpack module/nomodule recipe emit
//! today.
//!
//! **Teeth, and each is a way to get this wrong:**
//! * the `nomodule` classic script must not run — the defect itself;
//! * the `type="module"` script MUST still run — a fix that skipped both would blank the page just
//!   as thoroughly, and would pass a gate that only asserted the first claim;
//! * `nomodule` on a `type="module"` element is **inert per spec** and that script must still run —
//!   the one way the predicate can be actively wrong;
//! * a plain classic script with no `nomodule` is untouched.
//!
//! Proven RED: before the fix `r` reads `module legacy inert-nomodule plain` — the legacy bundle
//! executes alongside the modern one, which is the double-bootstrap.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>window.r = [];</script>
<script type="module">window.r.push('module');</script>
<script nomodule>window.r.push('legacy');</script>
<script type="module" nomodule>window.r.push('inert-nomodule');</script>
<script type="text/javascript" nomodule>window.r.push('legacy-explicit-mime');</script>
<script>window.r.push('plain');</script>
<script type="module">
  document.getElementById('out').textContent = window.r.join(' ');
</script>
</body></html>"##;

#[test]
fn a_module_capable_browser_skips_the_nomodule_half_of_a_differential_build() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nomodule.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("NOMODULE RESULT: {got}");

    for claim in ["module", "plain", "inert-nomodule"] {
        assert!(
            got.split_whitespace().any(|w| w == claim),
            "G_NOMODULE: `{claim}` must still RUN\n  got: {got}\n\n  \
             Skipping too much is the same blank page as skipping too little: `nomodule` applies to \
             CLASSIC scripts only, so a `type=\"module\"` element carrying it is inert per spec and \
             must still execute, and a script with no `nomodule` at all is untouched."
        );
    }
    for claim in ["legacy", "legacy-explicit-mime"] {
        assert!(
            !got.split_whitespace().any(|w| w == claim),
            "G_NOMODULE: `{claim}` must NOT run\n  got: {got}\n\n  \
             HTML's `prepare the script element` step 12: if the element has a `nomodule` content \
             attribute and its script block's type is \"classic\", return — do not fetch, do not \
             execute. This engine understands `type=\"module\"`, so running the legacy half of a \
             differential build means running BOTH halves of the same application: two framework \
             runtimes over one root element. That is the Angular CLI default output, and it is why \
             pogoda.by rendered nothing."
        );
    }
    // Order matters as much as membership, and this expectation is **Chrome's**, captured from
    // `chromium --headless --dump-dom` on this exact fixture rather than reasoned out — the first
    // draft of this line asserted the reverse and was wrong. A classic non-`defer` script executes
    // during parsing; a module is deferred to after it. So `plain` — the LAST script in the source
    // order below — runs FIRST, and the two surviving modules follow in document order.
    assert_eq!(
        got.split_whitespace().collect::<Vec<_>>(),
        vec!["plain", "module", "inert-nomodule"],
        "G_NOMODULE: `nomodule` removes members from the executable set; it must not reorder what \
         is left. Chrome on this fixture: `plain module inert-nomodule`."
    );
}
