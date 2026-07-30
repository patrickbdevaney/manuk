//! **G_HOT_DOM_NO_COMPILE — a DOM call on the frame path must not invoke the JS COMPILER.**
//!
//! Tick 768 found `record_mutation` building a source string and running `evaluate_script` **once per
//! mutated node**. On `en.wikipedia.org/wiki/Terrier` that was ~4 million parse+bytecode+`JSScript`
//! allocations during MediaWiki's boot, and the process died with SIGSEGV inside SpiderMonkey in 6 of 8
//! runs. The fix was to CALL the function instead of compiling a program that calls it.
//!
//! **This gate exists because that was a CLASS, not an instance** (`docs/wiki/js-engine.md`). The grep
//! that followed found three more, all on paths a real page hits per element or per event:
//!
//!   * `getBoundingClientRect()` — the hottest measurement call on the web (scroll handlers, sticky
//!     headers, `IntersectionObserver` polyfills, every animation library) — compiled an eight-field
//!     object literal **per call**: 20,000 calls in **131ms**, now **13ms**;
//!   * `getClientRects()` — compiled a whole IIFE with an `item()` closure per call, and was the
//!     slowest of the three: **354ms → 16ms**;
//!   * `dispatchEvent()` — compiled `__dispatchEvent(id, __pendingEvent)` **per event fired**;
//!   * `getBBox()` — same object-literal shape, and the call every charting library makes per SVG node.
//!
//! **Why a RATIO and not a millisecond budget.** There is no public hook that says *"SpiderMonkey
//! compiled something"*, so the observable is cost — but an absolute budget either flakes on a slow box
//! or is so loose it proves nothing. (It was loose first: a 1500ms budget passed at 35ms *with* the
//! defect restored, i.e. the gate could not go red — recorded because a gate that cannot fail is the
//! failure this project keeps catching.) So each hot call is measured against `element.tagName` — a
//! native property read in the same loop, same process, same machine — and the assertion is on the
//! ratio. Measured: `getBoundingClientRect` **43.7× tagName with the compile, 5.0× without**.
//!
//! RED, run: restore `format!(…)` + `eval_in_current_global` in `el_get_bounding_rect` (ratio 43.7 vs a
//! limit of 15) or in `el_get_client_rects` (ratio ~170 vs a limit of 25).
//!
//! ⚠ The correctness half is asserted FIRST and is not decoration: a "fast" `getBoundingClientRect`
//! that returns the wrong numbers, or a `dispatchEvent` that stops delivering, would pass a timing
//! assertion alone. The values below are the ones the old compiled literals produced, so this gate also
//! pins that the rewrite changed **cost only**.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="a" style="width:120px;height:40px;margin-left:10px;margin-top:5px">x</div>
<svg width="100" height="50"><rect id="r" x="5" y="6" width="30" height="20"/></svg>
<div id="out">-</div>
<script>
var out = [];
try {
  var e = document.getElementById('a');

  // ── correctness first: the exact numbers the compiled object literals used to produce.
  var r = e.getBoundingClientRect();
  out.push('rect:' + [r.x, r.y, r.width, r.height, r.left, r.top, r.right, r.bottom].join(','));
  out.push('keys:' + Object.keys(r).length);

  var list = e.getClientRects();
  out.push('rects:' + list.length + ':' + (list.item(0) ? list.item(0).width : 'none') +
           ':' + (list.item(5) === null));

  var bb = document.getElementById('r').getBBox();
  out.push('bbox:' + [bb.x, bb.y, bb.width, bb.height].join(','));

  var fired = 0;
  e.addEventListener('ping', function () { fired++; });
  e.dispatchEvent(new Event('ping'));
  out.push('fired:' + fired);

  // ── then cost, against a native-property control in the same process.
  var N = 20000, i, s = 0, t;
  t = Date.now(); for (i = 0; i < N; i++) { s += e.tagName.length; }               var tTag = Date.now() - t;
  t = Date.now(); for (i = 0; i < N; i++) { s += e.getBoundingClientRect().width; } var tRect = Date.now() - t;
  t = Date.now(); for (i = 0; i < N; i++) { s += e.getClientRects().length; }       var tList = Date.now() - t;
  t = Date.now(); for (i = 0; i < N; i++) { e.dispatchEvent(new Event('ping')); }   var tEvt = Date.now() - t;

  var base = Math.max(tTag, 1);
  // 'DIV'.length + rect.width + list.length, N times each — proves the loops actually READ the values
  // rather than being optimised into nothing.
  out.push('acc:' + (s === N * (3 + 120 + 1)));
  out.push('fired2:' + (fired === N + 1));
  out.push('rectRatio:' + (tRect / base).toFixed(1));
  out.push('listRatio:' + (tList / base).toFixed(1));
  out.push('evtRatio:' + (tEvt / base).toFixed(1));
  out.push('ms:' + tTag + '/' + tRect + '/' + tList + '/' + tEvt);
} catch (err) {
  out.push('THREW:' + err);
}
document.getElementById('out').textContent = out.join(' ');
</script></body></html>"##;

/// `getBoundingClientRect` measured **43.7×** the control with the compile and **5.0×** without.
const RECT_RATIO_LIMIT: f64 = 15.0;
/// `getClientRects` was ~170× with its IIFE compile; ~8× once it calls the once-compiled factory.
const LIST_RATIO_LIMIT: f64 = 25.0;

#[test]
fn hot_dom_calls_do_not_invoke_the_js_compiler() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://hotdom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("G_HOT_DOM RESULT: {got}");

    // ── correctness (body margin 0, so `a` is at x=10 y=5 — the numbers the old literals produced)
    for claim in [
        "rect:10,5,120,40,10,5,130,45",
        "keys:8",           // eight own enumerable fields, exactly as the object literal had
        "rects:1:120:true", // one rect, right width, and item() out of range is null
        "bbox:5,6,30,20",
        "fired:1",
        "acc:true",    // the hot loops really read the width / length they were timing
        "fired2:true", // every one of the 20,000 dispatches was delivered
    ] {
        assert!(
            got.contains(claim),
            "G_HOT_DOM_NO_COMPILE: expected `{claim}`\n  got: {got}\n\n  \
             The rewrite of getBoundingClientRect / getBBox / getClientRects / dispatchEvent off the JS \
             compiler must change COST ONLY. A wrong number here means the native object build lost a \
             field or a value, or the native call stopped delivering."
        );
    }

    let ratio = |k: &str| -> f64 {
        got.split_whitespace()
            .find_map(|t| t.strip_prefix(k))
            .and_then(|v| v.parse().ok())
            .unwrap_or(f64::MAX)
    };
    for (key, limit, what) in [
        ("rectRatio:", RECT_RATIO_LIMIT, "getBoundingClientRect"),
        ("listRatio:", LIST_RATIO_LIMIT, "getClientRects"),
    ] {
        let r = ratio(key);
        assert!(
            r <= limit,
            "G_HOT_DOM_NO_COMPILE: {what} costs {r}× a native property read (limit {limit}×).\n  \
             got: {got}\n\n  \
             This is the tick-768 defect class: a DOM call on the frame path invoking the JS COMPILER. \
             `format!` + `evaluate_script` reads like a one-liner and is a parse + bytecode compile + \
             JSScript allocation, per call. It killed the process on Wikipedia (~4M compiles during \
             MediaWiki's boot); here it shows up as a ratio against `tagName` in the same loop. Build \
             the object natively and CALL the once-compiled helper."
        );
    }
}
