//! **G_EVENT_SURFACE — `once`, the legacy aliases, and `createEvent`. All silent when missing.**
//!
//! Propagation was already right: bubbling, capture, `stopPropagation`, `target`/`currentTarget`, and the
//! `dispatchEvent` return value. What was missing was everything *around* it, and every one of the gaps
//! failed **silently**:
//!
//! * **`{once: true}` fired every time, forever.** The native read the third argument as a bare boolean,
//!   so an options *object* meant `capture: false` and `once` was **dropped on the floor**. It is one of
//!   the most common options in modern code and its failure is completely invisible — the handler simply
//!   keeps running.
//! * **`e.returnValue` and `e.cancelBubble` were `undefined`.** They are IE-era aliases the spec kept
//!   *because the web never stopped using them*: jQuery's event normalisation, Google Analytics, and a
//!   great deal of ordinary handler code. `if (e.returnValue === false)` was always false, and
//!   `e.cancelBubble = true` set a junk property that stopped nothing.
//! * **`document.createEvent` did not exist**, so `createEvent is not a function` took the whole script
//!   with it. It had been deferred for fear of an infinite dispatch loop — but that loop was never in
//!   `createEvent`. It was a frozen `timeStamp`, and that is long fixed.
//! * a listener could be registered **twice**, and an `{once}` removal mid-dispatch **skipped** the next
//!   listener, because the invocation loop indexed a live array while mutating it.
//!
//! `dom/events` went **102/401 → 145/412**.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div><div id="a"><i id="b"></i></div>
<script>
  var R = [];
  var a = document.getElementById('a'), b = document.getElementById('b');

  // ── 1. `once` must fire exactly once. It fired forever.
  var n = 0;
  b.addEventListener('o', function () { n++; }, { once: true });
  b.dispatchEvent(new Event('o'));
  b.dispatchEvent(new Event('o'));
  R.push('once:' + n);

  // ── 2. An options OBJECT must still select the capture phase. Read as a bare boolean it was `false`.
  var order = [];
  a.addEventListener('c', function () { order.push('cap'); }, { capture: true });
  b.addEventListener('c', function () { order.push('tgt'); });
  b.dispatchEvent(new Event('c', { bubbles: true }));
  R.push('capOpt:' + order.join(','));

  // ── 3. The same callback must not be added twice (spec).
  var d = 0;
  function once(){ d++; }
  b.addEventListener('d', once);
  b.addEventListener('d', once);
  b.dispatchEvent(new Event('d'));
  R.push('dedupe:' + d);

  // ── 4. Legacy aliases. `undefined` here means every `if (e.returnValue === false)` is dead code.
  var e1 = new Event('x', { cancelable: true });
  R.push('rv0:' + e1.returnValue);
  e1.preventDefault();
  R.push('rv1:' + e1.returnValue);

  var e2 = new Event('y', { bubbles: true });
  var seen = [];
  a.addEventListener('y', function () { seen.push('a'); });
  b.addEventListener('y', function (ev) { seen.push('b'); ev.cancelBubble = true; });
  b.dispatchEvent(e2);
  R.push('cancelBubble:' + (seen.join(',') || 'none'));

  // ── 5. createEvent + initEvent — the pre-constructor path jQuery and GA still take.
  var e3 = document.createEvent('Event');
  e3.initEvent('z', true, true);
  var z = 0;
  a.addEventListener('z', function () { z++; });
  b.dispatchEvent(e3);
  R.push('createEvent:' + e3.type + ',' + e3.bubbles + ',' + z);

  // ── 6. A handler may be an OBJECT with `handleEvent` — the EventListener interface.
  var h = 0;
  b.addEventListener('h', { handleEvent: function () { h++; } });
  b.dispatchEvent(new Event('h'));
  R.push('handleEvent:' + h);

  // ── 7. **A PASSIVE listener's preventDefault() does NOTHING.** That is the whole contract: the page
  //       promises not to cancel, so the browser may scroll without waiting for the handler. We recorded
  //       `passive` and honoured it nowhere — so a passive touch handler could still cancel the scroll,
  //       which is the exact jank the flag exists to prevent.
  var pe = new Event('p', { cancelable: true });
  b.addEventListener('p', function (ev) { ev.preventDefault(); }, { passive: true });
  b.dispatchEvent(pe);
  R.push('passive:' + pe.defaultPrevented);

  // …and a NON-passive listener still cancels.
  var ae = new Event('a', { cancelable: true });
  b.addEventListener('a', function (ev) { ev.preventDefault(); });
  b.dispatchEvent(ae);
  R.push('active:' + ae.defaultPrevented);

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn event_options_legacy_aliases_and_create_event_all_work() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ev.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "once:1",
            "`{once: true}` fired EVERY TIME. The third argument was read as a bare boolean, so an \
             options object meant capture:false and `once` was dropped on the floor — silently",
        ),
        ("capOpt:cap,tgt", "`{capture: true}` must select the capture phase, not just the boolean form"),
        ("dedupe:1", "the same callback added twice is added once (spec)"),
        ("rv0:true", "`returnValue` was `undefined`, so `if (e.returnValue === false)` was dead code"),
        ("rv1:false", "…and preventDefault must flip it"),
        (
            "cancelBubble:b",
            "`e.cancelBubble = true` must STOP propagation — it used to set a junk property that stopped \
             nothing, so the handler on the parent still ran",
        ),
        (
            "createEvent:z,true,1",
            "`document.createEvent` + `initEvent` — jQuery's trigger and Google Analytics take this path, \
             and `createEvent is not a function` takes the whole script with it. It was deferred over a \
             feared dispatch loop; that loop was a frozen timeStamp, and it is fixed",
        ),
        ("handleEvent:1", "a listener may be an OBJECT with a handleEvent method — the EventListener interface"),
        (
            "passive:false",
            "**a PASSIVE listener's preventDefault() does NOTHING.** The page promises not to cancel, so \
             the browser may scroll without waiting for it. We recorded `passive` and honoured it \
             nowhere — a passive touch handler could still cancel the scroll, which is the exact jank \
             the flag exists to prevent",
        ),
        ("active:true", "…while a non-passive listener still cancels, or the flag would mean nothing"),
    ] {
        assert!(
            got.contains(claim),
            "G_EVENT_SURFACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
