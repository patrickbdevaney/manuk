//! **G_AN_EXTERNAL_SCRIPT_KEEPS_ITS_SRC — a control flag was living in a web-facing attribute.**
//!
//! `fetch_external_scripts` fetched a `<script src>`, put the source in the element, and then called
//! `dom.remove_attr(node, "src")`. The ABSENCE of `src` was the signal `collect_inline_scripts` read
//! as *"this node has text and should run"* — so the flag and the attribute the page reads were the
//! same bit, and the page could not have it back. *A sentinel that is also a legal value is not a
//! sentinel* (t1424), one class further along: here the sentinel was a legal ABSENCE.
//!
//! ⚠ **THIS ENGINE HAD ALREADY FIXED THE SAME BUG ONE PATH OVER.** `Page::dyn_scripts_ran` exists
//! because `fetch_and_run_dynamic_scripts` removed `src` before evaluating and *"a script cannot read
//! its own URL off an attribute that is gone"* — measured then as `TypeError: Invalid URL: ` on 4 of
//! 200 CrUX sites. One rule, two implementations, and the PARSER half — the half that runs on every
//! page — was the stale one.
//!
//! ## What it cost, measured
//!
//! The t1480 boot histogram's top first-failure class over 40 CrUX sites was ONE third-party bundle
//! at a byte-identical minified offset:
//!
//! ```text
//!   OneTrustStub</S.prototype.setOTDataLayer@https://www.ikea.com/   inline#156:1:12608
//!   OneTrustStub</S.prototype.setOTDataLayer@https://www.otomoto.pl/ inline#66:1:12608
//!   TypeError: can't access property "hasAttribute", p.stubScriptElement is null
//! ```
//!
//! Two families of ordinary code break on this and nothing else does:
//!
//! * **A bundle locating its own tag** — `document.querySelector('script[src*="otSDKStub"]')`,
//!   which every consent SDK and tag manager does to read its own `data-*` configuration.
//! * **A bundle deriving its asset base** — `new URL(document.currentScript.src)`, which is
//!   literally what webpack's `publicPath: 'auto'` emits. Against `""` that does not skip, it
//!   **throws**.
//!
//! ## Arbitrated against headless Chrome, on this exact fixture
//!
//! ```text
//!                                        Chrome     before     after
//!   runs                                 1          1          1
//!   document.currentScript.src           has-src    ""         has-src
//!   …and it is ABSOLUTE                  abs        —          abs
//!   new URL(currentScript.src)           ok         THREW      ok
//!   querySelectorAll('script[src]')      1          0          1
//!   querySelectorAll('script[src*=…]')   1          0          1
//!   the data-* attribute                 abc-123    abc-123    abc-123
//! ```
//!
//! ## ⚠ `runs=1` IS THE LOAD-BEARING ROW, NOT A FORMALITY
//!
//! `Page::drain_injected_scripts` selects scripts by *"has `src` and is not in `dyn_scripts_ran`"*.
//! Restoring `src` without seeding that set makes every external script on every page fetch a second
//! time and **execute a second time** — a page's analytics, its consent gate and its router all
//! booting twice, which is a Bar-0 shape dressed as a one-line attribute change. The seed is what
//! makes this fix a fix; `runs` is what proves it.
//!
//! ⚠ **RESIDUE, NAMED.** The node now carries `src` AND a text child, where Chrome's external script
//! has `src` and `textContent === ""`. Holding the source in a side map instead of in the DOM is the
//! complete fix; this is the half the corpus actually fails on, and the gate does not claim the other.
//!
//! Mutations that must turn this red:
//!   1. restore `dom.remove_attr(node, "src")`        -> csSrc="", qsSrc=0, and newURL THREW
//!   2. drop the `!inlined.contains(&n)` exemption    -> the script never runs at all (runs=undefined)
//!   3. drop the `dyn_scripts_ran` seed               -> runs=2
//!   4. seed `dyn_scripts_ran` with EVERY script node -> (not reachable from a fixture; covered by 3)

use manuk_text::FontContext;

const JS: &str = r##"window.__runs = (window.__runs || 0) + 1;
window.__probe = window.__probe || {};
window.__probe.csSrc    = (document.currentScript && document.currentScript.src) ? 'has-src' : 'no-src';
window.__probe.absolute = (document.currentScript && /^file:\/\/.*g1481\.js$/.test(document.currentScript.src)) ? 'abs' : 'not-abs';
window.__probe.newURL   = (function () { try { new URL(document.currentScript.src); return 'ok'; } catch (e) { return 'THREW'; } })();
window.__probe.qsSrc    = document.querySelectorAll('script[src]').length;
window.__probe.qsStar   = document.querySelectorAll('script[src*="g1481.js"]').length;
window.__probe.data     = (document.currentScript && document.currentScript.getAttribute('data-domain-script')) || 'none';
"##;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8>
<script id="otSDKStub" src="g1481.js" data-domain-script="abc-123"></script>
</head><body><div id="out">-</div>
<script>
window.addEventListener('load', function () {
  var p = window.__probe || {};
  document.getElementById('out').textContent =
    'runs=' + window.__runs + ' csSrc=' + p.csSrc + ' abs=' + p.absolute + ' newURL=' + p.newURL +
    ' qsSrc=' + p.qsSrc + ' qsStar=' + p.qsStar + ' data=' + p.data;
});
</script></body></html>"##;

#[test]
fn a_fetched_external_script_still_has_the_src_the_page_reads() {
    let dir = std::path::Path::new("/tmp/g1481");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("g1481.js"), JS).unwrap();
    let path = dir.join("page.html");
    std::fs::write(&path, HTML).unwrap();

    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(
            HTML,
            &format!("file://{}", path.display()),
            &fonts,
            800.0,
        )
        .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("EXTERNAL SCRIPT IDENTITY: {got}");

    // ── VACUITY. The fetched script has to have RUN, or every field is its initial value and this
    //    gate passes on a page whose script never executed at all.
    assert!(
        got.contains("runs=1"),
        "VACUOUS: the external script did not run exactly once — every field below is then a \
         statement about nothing, and `runs=2` in particular means the fix double-boots every page: \
         got {got:?}"
    );

    // Chrome headless, every field.
    let want = "runs=1 csSrc=has-src abs=abs newURL=ok qsSrc=1 qsStar=1 data=abc-123";
    assert_eq!(
        got, want,
        "\n  a fetched external <script src> must keep the attribute the page reads: its own URL, \
         absolute, resolvable by `new URL`, and findable by an attribute selector\n  want: {want}\n  \
         got:  {got}"
    );

    // …and the ATTRIBUTE is still on the element in the DOM, not merely reflected by the binding.
    let script = manuk_css::query_selector_all(page.dom(), root, "script[src]");
    assert_eq!(
        script.len(),
        1,
        "the selector engine must see it too — `document.styleSheets`-style divergences begin \
         exactly here, with the JS view and the DOM disagreeing about one attribute"
    );
}
