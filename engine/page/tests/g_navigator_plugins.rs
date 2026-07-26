//! **G_NAVIGATOR_PLUGINS — `navigator.plugins` and `navigator.mimeTypes` exist and enumerate.**
//!
//! Measured by surface audit #32 (tick 588) off the Blink use-counter dump: `NavigatorPlugins` is
//! **32.54% of page loads**, `NavigatorMimeTypes` 12.46%. Our navigator literal shipped `vendor`,
//! `vendorSub`, `productSub`, `deviceMemory`, `hardwareConcurrency`, `maxTouchPoints` and
//! `webdriver` — and **not these two**.
//!
//! The failure mode is the one the code comment beside `navigator.vendor` already argues, verbatim:
//!
//! > *"`vendor` was UNDEFINED, and it is one of the handful of things a UA-sniffing bundle reads on
//! > its first line. `navigator.vendor.indexOf('Apple')` on `undefined` is a TypeError that takes the
//! > rest of the bundle with it — and sniffing code is, by nature, the code that runs before anything
//! > else."*
//!
//! `navigator.plugins.length` on `undefined` is exactly that TypeError, in exactly that code. The
//! argument was made and then applied to one property.
//!
//! ## What honest looks like here, and why it is not "pretend to have Flash"
//!
//! Since Chrome 93 the spec **hard-codes** `navigator.plugins` to five fixed PDF-viewer entries on
//! every desktop browser, precisely so it stops being a fingerprinting surface — the list is no
//! longer a report of what is installed, it is a **constant the spec requires**. So returning it is
//! not a claim about this engine's plugin support; it is the spec-mandated value, and returning
//! `undefined` instead is the divergence.
//!
//! We do render PDFs? No — and that is why `application/pdf` is **not** claimed in
//! `mimeTypes`-as-capability terms anywhere else in the map. This gate asserts the *enumeration
//! surface* a sniffer walks, which is a different question from whether a `<embed type=application/pdf>`
//! displays. Keeping those apart is the point: the honest answer to "can a page enumerate plugins?"
//! is yes, and the honest answer to "do you render PDF?" stays no.
//!
//! Claims: both collections exist, have a `length`, are indexable, support `namedItem`, iterate, and
//! their entries carry the `name`/`filename`/`description` a sniffer reads — and `refresh()` exists
//! and does not throw, because legacy code calls it unconditionally.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body><div id="out">-</div><script>
  var R = [];
  function k(n, v) { R.push(n + ':' + v); }
  try {
    k('plugins', typeof navigator.plugins);
    k('len', navigator.plugins.length > 0);
    k('idx', typeof navigator.plugins[0] === 'object' && navigator.plugins[0] !== null);
    k('name', typeof navigator.plugins[0].name === 'string' && navigator.plugins[0].name.length > 0);
    k('file', typeof navigator.plugins[0].filename === 'string');
    k('desc', typeof navigator.plugins[0].description === 'string');
    // The canonical sniff: `namedItem` by the spec-mandated name.
    k('named', navigator.plugins.namedItem('PDF Viewer') !== null);
    // Legacy code calls this unconditionally; it must exist and be harmless.
    navigator.plugins.refresh();
    k('refresh', true);
    k('mimes', typeof navigator.mimeTypes);
    k('mimeLen', navigator.mimeTypes.length > 0);
    k('mimeNamed', navigator.mimeTypes['application/pdf'] !== undefined ||
                   navigator.mimeTypes.namedItem('application/pdf') !== null);
    k('mimeType', typeof navigator.mimeTypes[0].type === 'string');
    // A for-loop over the collection is how every enumeration-based sniff walks it.
    var seen = 0;
    for (var i = 0; i < navigator.plugins.length; i++) { if (navigator.plugins[i].name) { seen++; } }
    k('walk', seen === navigator.plugins.length);
  } catch (e) { k('THREW', e); }
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn navigator_plugins_and_mime_types_enumerate() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nav.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("NAVIGATOR PLUGINS: {got}");

    for (claim, why) in [
        (
            "plugins:object",
            "`navigator.plugins.length` on `undefined` is a TypeError in the code that runs FIRST — \
             the identical argument the comment beside `navigator.vendor` already makes, applied to \
             the property it was written about and not to this one. 32.5% of page loads read it",
        ),
        ("len:true", "the spec hard-codes five PDF-viewer entries — an empty list is not the spec value"),
        ("idx:true", "indexable: `navigator.plugins[0]` is how a sniffer reaches an entry"),
        ("name:true", "…and `.name` is the string it compares"),
        ("file:true", "`.filename` is read by the older sniffs"),
        ("desc:true", "`.description` likewise"),
        (
            "named:true",
            "`namedItem('PDF Viewer')` — the canonical modern check, and the name the spec mandates",
        ),
        (
            "refresh:true",
            "legacy code calls `plugins.refresh()` unconditionally; it must exist and be harmless",
        ),
        ("mimes:object", "`navigator.mimeTypes` is the sibling collection, 12.5% of page loads"),
        ("mimeLen:true", "…and it is not empty"),
        ("mimeNamed:true", "…and `application/pdf` is reachable by name, which is what is looked up"),
        ("mimeType:true", "…and an entry carries `.type`"),
        (
            "walk:true",
            "a `for` loop over `length` reaches every entry — enumeration is how the older sniffs \
             work, and a collection that indexes but does not walk would satisfy every claim above",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_NAVIGATOR_PLUGINS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
