//! **G_EVENT_DISPATCH_STATE — `dispatchEvent` must throw `InvalidStateError` for an event that is not
//! ready to be dispatched.**
//!
//! DOM §`dispatchEvent`: *"If event's dispatch flag is set, or its initialized flag is not set, then
//! throw an InvalidStateError."* Two real states hit this:
//!
//! * **Uninitialized.** `document.createEvent("Event")` hands back an event whose type is `""` and whose
//!   initialized flag is unset — `initEvent` must run before it may be dispatched. Every legacy library
//!   that builds events the pre-constructor way (jQuery's `trigger`, Google Analytics) relies on this
//!   ordering, and a `dispatchEvent` that silently *succeeds* on an uninitialized event dispatches an
//!   empty-typed event to nobody and reports success — the worst failure shape.
//! * **In flight.** Re-dispatching the *same* event object from inside one of its own listeners is a
//!   re-entrancy the spec forbids, because the event's target/phase state is mid-mutation.
//!
//! The subtle part is the plumbing, not the rule: `el.dispatchEvent` is a native, and the native used to
//! `unwrap_or(false)` the internal dispatch — **swallowing the thrown `InvalidStateError` into a benign
//! `false`**, so `assert_throws_dom` saw no throw. The native now propagates the pending exception.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];

  // 1. An uninitialized createEvent() event dispatches to InvalidStateError.
  var e = document.createEvent('Event');
  R.push('type:' + JSON.stringify(e.type));            // "" before initEvent
  try { document.dispatchEvent(e); R.push('uninit:NOTHROW'); }
  catch (ex) { R.push('uninit:' + ex.name); }          // InvalidStateError

  // 2. After initEvent it dispatches normally (the flag is cleared).
  e.initEvent('foo', false, false);
  var ok = document.dispatchEvent(e);
  R.push('init:' + ok);                                // true — no throw

  // 3. Re-dispatching the same event WHILE it is in flight throws InvalidStateError.
  var d = document.createElement('div');
  var reentrant = 'NONE';
  d.addEventListener('foo', function () {
    try { d.dispatchEvent(e); } catch (ex) { reentrant = ex.name; }
  });
  d.dispatchEvent(e);
  R.push('reentrant:' + reentrant);                    // InvalidStateError

  // 4. …and once dispatch completes, the same event is dispatchable again (flag cleared).
  R.push('again:' + document.dispatchEvent(e));         // true

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn dispatch_event_throws_invalidstate_for_uninitialized_and_in_flight_events() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://event.test/d/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("type:\"\"", "a createEvent() event is uninitialized — its type is the empty string until initEvent"),
        ("uninit:InvalidStateError", "dispatching an uninitialized event throws InvalidStateError, and the native must PROPAGATE it, not swallow it into `false`"),
        ("init:true", "after initEvent the event dispatches normally and returns true"),
        ("reentrant:InvalidStateError", "re-dispatching an event from inside its own listener (dispatch flag set) throws InvalidStateError"),
        ("again:true", "once dispatch completes the flag clears and the event is dispatchable again"),
    ] {
        assert!(
            got.contains(claim),
            "G_EVENT_DISPATCH_STATE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
