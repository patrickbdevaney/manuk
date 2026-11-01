//! **`new FontFace(family, src).load()` must run and settle** (W4.C1.a). It was an inert constructor
//! with no `load`, so a page's "load the webfont, then reveal" chain threw and the reveal never ran
//! (c1_settle `font-swap`: 0/3 against Chrome 3/3). Scope: the face is fetched and settles; its bytes
//! are not handed to the shaper.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div>
<script>
 window.addEventListener('load', function () {
   var o = document.getElementById('out'), parts = [];
   function T(n, f) { try { parts.push(n + '=' + f()); } catch (e) { parts.push(n + '=THROW:' + e.name + ':' + e.message); } }
   var f, g;
   T('type', function () { return typeof FontFace; });
   T('ctor', function () { f = new FontFace('X', 'url(/nope.woff2)'); return f.family + '/' + f.status; });
   T('loadfn', function () { return typeof f.load; });
   T('abuf', function () { g = new FontFace('Y', new ArrayBuffer(8)); return g.status; });
   T('instanceof', function () { return f instanceof FontFace; });
   o.textContent = parts.join(' '); var n = 0;
   function done(tag) { parts.push(tag); n++; o.textContent = parts.join(' '); }
   g.load().then(function (v) { done('bufLoaded=' + (v === g) + '/' + g.status); }, function () { done('bufRejected'); });
   f.load().then(function () { done('urlLoaded'); }, function (e) { done('urlRejected=' + e.name + '/' + f.status); });
 });
</script></body></html>"#;

#[test]
fn fontface_constructs_and_load_settles() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://fonts.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    let got = page.dom().text_content(hits[0]);
    println!("FONTFACE {got}");
    // RED: restore the inert constructor -> `loadfn=undefined` / ctor throws.
    assert!(
        got.contains("type=function")
            && got.contains("ctor=X/unloaded")
            && got.contains("loadfn=function"),
        "{got}"
    );
    assert!(
        got.contains("abuf=unloaded") && got.contains("instanceof=true"),
        "{got}"
    );
    // An ArrayBuffer source loads without a network; an unreachable url settles as a rejection, never hangs.
    assert!(
        got.contains("bufLoaded=true/loaded"),
        "buffer-sourced load() must resolve with the face: {got}"
    );
    assert!(
        got.contains("urlRejected=NetworkError/error") || got.contains("urlLoaded"),
        "url load() must settle: {got}"
    );
}
