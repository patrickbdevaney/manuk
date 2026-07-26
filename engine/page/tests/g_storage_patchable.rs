//! **G_STORAGE_PATCHABLE — patching a `localStorage` method must take effect.**
//!
//! Measured at tick 586 while building the certificate's capability probe, which could not wrap
//! storage and recorded nothing:
//!
//! ```text
//! own=false | protoHas=undefined | wrapStuck=false      ← localStorage.setItem = fn  → DISCARDED
//! idbOpenWrap=true | fetchWrap=true | ioWrap=true       ← every other host object wraps fine
//! ```
//!
//! `localStorage` is a `Proxy` over a native seam, and its `set` trap reads:
//!
//! ```js
//! set: function (t, p, v) {
//!   if (typeof p === 'string' && !hasOwnProperty.call(t, p)) { __storage('set', area, p, v); }
//!   return true;                    // ← a method name falls through here and is DROPPED
//! }
//! ```
//!
//! So `localStorage.foo = 'bar'` correctly stores an item, and `localStorage.setItem = fn` is
//! **accepted and thrown away** — `return true` tells the assignment it succeeded. In a real browser
//! the methods live on `Storage.prototype`, so assigning one creates an **own property that shadows
//! it**, and subsequent calls run the replacement.
//!
//! ## Why this is a capability and not a curiosity
//!
//! **Patching storage is one of the most common things a real page does**, and every one of these
//! installs silently and then never runs:
//!
//! - **private-mode / quota fallbacks** — wrap `setItem`, catch `QuotaExceededError`, fall back to
//!   an in-memory shim. Safari private mode made this idiom universal.
//! - **SSR and hydration guards** — replace storage with a no-op during server-side or pre-hydration
//!   render so shared code does not touch a missing API.
//! - **analytics and session libraries** — wrap `setItem` to mirror writes, namespace keys, or expire
//!   them.
//! - **test doubles** — every `jest.spyOn(localStorage, 'setItem')` in a page's own test bundle.
//!
//! The failure is the worst shape available: **no error, no warning, and the original behaviour
//! continues** — so the page appears to work until the case the wrapper existed for arrives.
//!
//! Claims: a method assignment shadows and is CALLED; `delete` restores the original; a non-method
//! assignment still writes to storage (the guard — the trap's original purpose must survive); and
//! ordinary storage round-trips are untouched.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body><div id="out">-</div><script>
  var R = [];
  var real = localStorage.setItem;
  // 1. Patch a method — it must stick AND be the thing that runs.
  var calls = [];
  localStorage.setItem = function (k, v) { calls.push(k); return real.call(localStorage, k, v); };
  R.push('stuck:' + (localStorage.setItem !== real));
  localStorage.setItem('patched', '1');
  R.push('ran:' + (calls.length === 1 && calls[0] === 'patched'));
  // …and the wrapper still reached the real implementation underneath.
  R.push('wrote:' + (localStorage.getItem('patched') === '1'));
  // 2. delete restores the original method.
  delete localStorage.setItem;
  R.push('restored:' + (typeof localStorage.setItem === 'function' && localStorage.setItem !== calls.push));
  localStorage.setItem('after', '2');
  R.push('afterWrote:' + (localStorage.getItem('after') === '2'));
  R.push('noExtraCall:' + (calls.length === 1));
  // 3. THE GUARD: a NON-method assignment must still write to storage. The trap's original job is
  //    `localStorage.foo = 'bar'`, and a fix that made every assignment shadow would break it.
  localStorage.plainkey = 'plainvalue';
  R.push('plain:' + (localStorage.getItem('plainkey') === 'plainvalue'));
  // 4. The control: ordinary storage still behaves.
  localStorage.setItem('ctl', 'v');
  R.push('ctl:' + (localStorage.getItem('ctl') === 'v'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_patched_storage_method_takes_effect() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://store.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("STORAGE PATCHABLE: {got}");

    for (claim, why) in [
        (
            "stuck:true",
            "`localStorage.setItem = fn` must STICK. The Proxy's `set` trap dropped any assignment \
             whose key matched a method name and returned `true`, so the assignment reported success \
             and changed nothing — the worst available failure shape",
        ),
        (
            "ran:true",
            "…and the replacement must be what RUNS. Sticking without being called would be a \
             different lie with the same symptom",
        ),
        (
            "wrote:true",
            "…and a wrapper that delegates to the captured original must still reach real storage, \
             which is exactly what a quota/private-mode fallback does",
        ),
        (
            "restored:true",
            "`delete localStorage.setItem` must restore the original method — in a browser the \
             method lives on `Storage.prototype` and the assignment merely SHADOWS it",
        ),
        ("afterWrote:true", "…and the restored original still works"),
        (
            "noExtraCall:true",
            "…and the removed wrapper is genuinely gone, not merely hidden",
        ),
        (
            "plain:true",
            "THE GUARD: `localStorage.plainkey = 'v'` must still write to STORAGE. That is the trap's \
             original purpose, and a fix that made every assignment shadow the target would break \
             every page that uses property syntax for storage",
        ),
        ("ctl:true", "THE CONTROL: ordinary getItem/setItem is untouched"),
    ] {
        assert!(
            got.contains(claim),
            "G_STORAGE_PATCHABLE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
