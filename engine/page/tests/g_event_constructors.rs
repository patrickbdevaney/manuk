//! **G_EVENT_CONSTRUCTORS — the typed Event subclass hierarchy: inheritance + per-interface members.**
//!
//! `Event-subclasses-constructors` asserts, for each typed event interface, both its **member set**
//! (own + every ancestor's) and its **`instanceof` chain** up to `Event`. The engine builds events as
//! flat objects, so each interface's constructor must set its ancestors' members as own properties AND
//! its prototype must chain to its parent's — otherwise `new MouseEvent() instanceof UIEvent` is false
//! and `mouseEvent.view`/`detail` are `undefined`. Before this tick only Event/MouseEvent/KeyboardEvent/
//! FocusEvent/InputEvent existed, all parent-less and missing the UIEvent members. Each assertion is one
//! rule the spec's interface graph requires:
//!
//! * **the chain** — `WheelEvent` is a `MouseEvent` is a `UIEvent` is an `Event`.
//! * **inherited members are present** — a `MouseEvent` carries UIEvent's `view`/`detail`; a
//!   `KeyboardEvent` carries them too; `relatedTarget` is a MouseEvent member.
//! * **new interfaces** — `UIEvent`, `WheelEvent`, `CompositionEvent` now exist with their members.
//! * **`view` is `Window?`** — `new UIEvent('x', {view: 7})` is a `TypeError` (a primitive is not a Window).
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip the
//! tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** before the hierarchy existed `new WheelEvent`/`new UIEvent`/`new CompositionEvent`
//! were `undefined` (a `TypeError`), so the script threw at the first and `#out` stayed at its `-`
//! sentinel — RED. The chained constructors turn it GREEN.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  function ck(l, g) { R.push(l + ':' + g); }
  function thrown(fn){ try { fn(); return 'NO_THROW'; } catch(e){ return e.name; } }

  var m = new MouseEvent('click', { clientX: 40, relatedTarget: document });
  ck('mUI', m instanceof UIEvent);
  ck('mEv', m instanceof Event);
  ck('mViewIn', 'view' in m);
  ck('mDetail', m.detail);
  ck('mRelT', m.relatedTarget === document);

  var w = new WheelEvent('wheel', { deltaX: 3.1 });
  ck('wMouse', w instanceof MouseEvent);
  ck('wUI', w instanceof UIEvent);
  ck('wDeltaX', w.deltaX);
  ck('wDeltaMode', w.deltaMode);

  var k = new KeyboardEvent('keydown', { location: 7 });
  ck('kLoc', k.location);
  ck('kViewIn', 'view' in k);

  var c = new CompositionEvent('compositionstart', { data: 'x' });
  ck('cData', c.data);
  ck('cUI', c instanceof UIEvent);

  var u = new UIEvent('x', { view: window });
  ck('uView', u.view === window);
  ck('uBadView', thrown(function(){ new UIEvent('x', { view: 7 }); }));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn typed_events_have_the_inheritance_chain_and_their_members() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ev.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "mUI:true",
            "a MouseEvent is a UIEvent (the instanceof chain)",
        ),
        ("mEv:true", "a MouseEvent is an Event"),
        (
            "mViewIn:true",
            "a MouseEvent carries UIEvent's inherited `view` member",
        ),
        (
            "mDetail:0",
            "a MouseEvent carries UIEvent's inherited `detail`, defaulting to 0",
        ),
        (
            "mRelT:true",
            "`relatedTarget` is a MouseEvent member set from the init dict",
        ),
        ("wMouse:true", "a WheelEvent is a MouseEvent"),
        (
            "wUI:true",
            "a WheelEvent is a UIEvent (chain walks two levels)",
        ),
        (
            "wDeltaX:3.1",
            "WheelEvent's `deltaX` is set from the init dict",
        ),
        ("wDeltaMode:0", "WheelEvent's `deltaMode` defaults to 0"),
        ("kLoc:7", "KeyboardEvent gained the `location` member"),
        (
            "kViewIn:true",
            "a KeyboardEvent carries UIEvent's inherited `view`",
        ),
        ("cData:x", "CompositionEvent (new interface) carries `data`"),
        ("cUI:true", "a CompositionEvent is a UIEvent"),
        ("uView:true", "UIEvent's `view` accepts a Window"),
        (
            "uBadView:TypeError",
            "UIEvent's `view` rejects a non-Window primitive with a TypeError",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_EVENT_CONSTRUCTORS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
