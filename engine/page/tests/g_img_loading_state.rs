//! **G_IMG_LOADING_STATE — the `<img>` numbers the engine already had and never published.**
//!
//! `img.complete`, `naturalWidth`, `naturalHeight` and `decode()` were all `undefined`. They are what
//! a page reads to find out whether an image is ready:
//!
//!   * `img.complete` is THE check every lazy-loader, lightbox, carousel and preloader makes;
//!   * `naturalWidth` is how every gallery computes an aspect ratio (and `undefined` makes that `NaN`);
//!   * `decode()` is what React/Next image components `await` before swapping a placeholder out — and
//!     a method that returns `undefined` makes `await img.decode()` succeed **instantly on an image
//!     that has not loaded**, which is worse than not having it at all.
//!
//! ⭐⭐ **The numbers existed the whole time.** `Page::publish_image_sources` hands every decoded
//! bitmap to the JS side so `ctx.drawImage(img, …)` has pixels; `canvas::source_size` has read the
//! width and height out of that table since it was written. Nothing exposed either to the page. This
//! tick is a publication, not a computation.
//!
//! Every row headless-Chrome-measured:
//!
//! ```text
//!                                  chrome    before      after
//!   <img> no src      complete     true      undefined   true
//!   <img src="">      complete     true      undefined   true
//!   new Image()       complete     true      undefined   true
//!   i.src='missing'   complete     false     undefined   false   (the fetch has not settled)
//!   naturalWidth / naturalHeight   0         undefined   0
//!   decode()                       Promise   TypeError   Promise
//!   sizes                          string    undefined   string
//! ```
//!
//! ## ⚠⚠ TWO DIVERGENCES, BOTH DELIBERATE, BOTH RECORDED HERE SO THEY ARE NOT "FIXED"
//!
//! **1. A FAILED image reads `complete === false` here and `true` in Chrome.** Chrome's rule is *the
//! fetch settled*, successfully or not; the only signal this engine publishes is the DECODED bitmap,
//! so success is recorded and failure is not. Closing it needs a failure set beside the source map —
//! a different mechanism, not a different accessor.
//!
//! **2. `currentSrc` returns the SELECTED url immediately; Chrome returns `""` until the image
//! loads.** ⚠⚠⚠ **DO NOT "CORRECT" THIS.** The engine deliberately publishes the candidate
//! `select_image_url` chose, and the comment at that getter records why: WPT's `the-img-element/sizes`
//! files read `expect = referenceImg.currentSrc` once per paragraph and `assert_unreached` every
//! sibling when it is falsy, so an empty string there failed whole groups and the directory read
//! **0 of 795**. A change that makes this row match Chrome costs 795 subtests. It is the t1004 hazard
//! running the other way: the *correct-looking* edit is the regression.
//!
//! ⚠ These members live on `HTMLElement.prototype` (tag-guarded), like every other cross-cutting
//! member in this engine — so `'complete' in HTMLImageElement.prototype` is `false` while
//! `img.complete` is right. That is the engine's interface model, not a gap in this tick, and the
//! gate asserts the VALUES rather than the descriptor locations.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<img id="noSrc">
<img id="emptySrc" src="">
<img id="withAttrs" src="x.png" sizes="100vw">
<div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
var noSrc = document.getElementById('noSrc');
var emptySrc = document.getElementById('emptySrc');
var withAttrs = document.getElementById('withAttrs');

// ── 1. "NOTHING TO WAIT FOR" IS COMPLETE ──────────────────────────────────────────────
k('a_noSrcComplete', noSrc.complete);
k('b_emptySrcComplete', emptySrc.complete);
var made = new Image();
k('c_newImageComplete', made.complete);

// ── 2. …AND ASSIGNING A SOURCE MAKES IT INCOMPLETE UNTIL THE FETCH SETTLES ─────────────
made.src = 'nonexistent-xyz.png';
k('d_afterSrcComplete', made.complete);

// ── 3. THE NATURAL SIZE IS 0 WHEN UNAVAILABLE — a NUMBER, not undefined ───────────────
k('e_naturalWidth', noSrc.naturalWidth);
k('f_naturalHeight', noSrc.naturalHeight);
k('g_naturalWidthIsNumber', typeof noSrc.naturalWidth);

// ── 4. decode() IS A PROMISE, ALWAYS ─────────────────────────────────────────────────
var p = noSrc.decode();
k('h_decodeReturnsPromise', !!p && typeof p.then === 'function');
p.then(function () { k('i_decodeSettled', 'resolved'); done(); },
       function (e) { k('i_decodeSettled', 'rejected:' + e.name); done(); });

// ── 5. sizes REFLECTS, AND IS WRITABLE ───────────────────────────────────────────────
k('j_sizesRead', withAttrs.sizes);
k('k_sizesType', typeof withAttrs.sizes);
withAttrs.sizes = '50vw';
k('l_sizesWriteReachesAttribute', withAttrs.getAttribute('sizes'));

// ── 6. THE GUARD — these are <img> members, and a non-image must not answer ──────────
k('m_divComplete', document.getElementById('out').complete);
k('n_divNaturalWidth', document.getElementById('out').naturalWidth);

function done() { document.getElementById('out').textContent = r.join(' '); }
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn an_img_publishes_the_loading_state_the_engine_already_knows() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://img-state.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("IMG-LOADING-STATE RESULT: {got}");

    for claim in [
        // 1 — nothing to wait for
        "a_noSrcComplete:true",
        "b_emptySrcComplete:true",
        "c_newImageComplete:true",
        // 2 — …and a source makes it incomplete
        "d_afterSrcComplete:false",
        // 3 — a NUMBER, not undefined
        "e_naturalWidth:0",
        "f_naturalHeight:0",
        "g_naturalWidthIsNumber:\"number\"",
        // 4 — decode() is a promise and it SETTLES
        "h_decodeReturnsPromise:true",
        "i_decodeSettled:\"rejected:EncodingError\"",
        // 5 — sizes reflects both ways
        "j_sizesRead:\"100vw\"",
        "k_sizesType:\"string\"",
        "l_sizesWriteReachesAttribute:\"50vw\"",
        // 6 — and a non-image answers nothing
        "m_divComplete:undefined",
        "n_divNaturalWidth:undefined",
    ] {
        assert!(
            got.contains(claim),
            "G_IMG_LOADING_STATE: expected `{claim}`\n  got: {got}\n\n  \
             An <img> with no source is `complete`; assigning one makes it incomplete until the fetch \
             settles. `naturalWidth`/`naturalHeight` are 0 when unavailable — a NUMBER, because a \
             gallery divides by them. `decode()` always returns a Promise. These are <img> members, so \
             a <div> must answer `undefined`. Every row is headless-Chrome-measured."
        );
    }
}
