//! **G_JS_BASELINE_2024 — Promise.withResolvers + Set methods (the Baseline-2024 JS the surface
//! audit added as unknown).**
//!
//! Surface Audit #26 (tick 528) added these as `unknown` on the standing rule that SpiderMonkey is
//! recent enough that recent-Baseline JS is likely ALREADY present — the stale-pessimistic pattern
//! that has paid out a dozen times. This gate MEASURES the guess and, if it holds, pins it: an
//! `unknown` becomes a `gated` capability with a demonstrated way to go red.
//!
//! - **`Promise.withResolvers()`** (Baseline 2024) — the deferred pattern without the
//!   executor-closure dance; modern async libraries and frameworks construct their deferreds this way.
//!   Returns `{ promise, resolve, reject }`, and resolving settles the promise.
//! - **Set methods** (Baseline 2024) — `union` / `intersection` / `difference` / `isSubsetOf`: set
//!   algebra without hand-rolled loops, used across dedup/permission/tag-filtering code.
//!
//! **RED, run:** this gate goes red the moment either feature is absent — on an engine without
//! `Set.prototype.intersection`, `a.intersection(b)` throws and `setint:2` never appears; without
//! `Promise.withResolvers`, the destructure throws and `pwr:true` never appears. That absence IS the
//! failure mode this pin guards against (a SpiderMonkey downgrade, or the guess having been wrong).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var out = [];
    // Promise.withResolvers — the destructured deferred.
    try {
      var d = Promise.withResolvers();
      var ok = (typeof d.resolve === 'function') && (typeof d.reject === 'function')
               && (d.promise instanceof Promise);
      out.push('pwr:' + ok);
    } catch (e) { out.push('pwr:absent'); }

    // Set algebra.
    try {
      var a = new Set([1, 2, 3]);
      var b = new Set([2, 3, 4]);
      out.push('int:' + a.intersection(b).size);   // {2,3} -> 2
      out.push('uni:' + a.union(b).size);           // {1,2,3,4} -> 4
      out.push('diff:' + a.difference(b).size);     // {1} -> 1
      out.push('sub:' + new Set([2, 3]).isSubsetOf(a)); // true
    } catch (e) { out.push('set:absent'); }

    document.getElementById('out').textContent = out.join(' ');
  </script>
</body></html>"##;

#[test]
fn promise_withresolvers_and_set_methods_are_present() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://js.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("JS BASELINE 2024 PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_JS_BASELINE_2024: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "pwr:true",
        "Promise.withResolvers() returns a { promise, resolve, reject } triple with a real Promise — \
         the deferred pattern modern async libraries build on",
    ),
    (
        "int:2",
        "Set.prototype.intersection keeps the shared members {2,3}",
    ),
    (
        "uni:4",
        "Set.prototype.union is the merged set {1,2,3,4}",
    ),
    (
        "diff:1",
        "Set.prototype.difference keeps only {1}",
    ),
    (
        "sub:true",
        "Set.prototype.isSubsetOf reports {2,3} is contained in {1,2,3}",
    ),
];
