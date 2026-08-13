//! # G_CREATE_EVENT_ALIASES — `createEvent` accepted EVERY name, so it could not be feature-detected
//!
//! `document.createEvent(iface)` was implemented as `g[String(iface)] || g.Event`, and both halves of
//! that expression are wrong.
//!
//! ## 1. The fallback accepted everything, so the throw a page listens for never came
//!
//! `document.createEvent('NotAnEvent')` returned a plain `Event`. DOM §createEvent says **throw
//! `NotSupportedError`** — and the reason that matters is not conformance:
//!
//! > **the throw is how a page feature-detects an event interface.** An engine that never throws
//! > answers *"supported"* for every name it is handed, so the detecting library takes the modern
//! > branch and gets an `Event` with the wrong prototype — a value of the right *type* that fails at
//! > the first `instanceof`.
//!
//! Same shape as the selector feature-detect at t1200 and jQuery's `support.cors`: **ask what a
//! library BELIEVES, not what it can detect.** 152 `dom` subtests assert exactly this throw.
//!
//! ## 2. The lookup was case-SENSITIVE, and the plural aliases have no global at all
//!
//! DOM §createEvent matches its argument **ASCII-case-insensitively** against a **fixed table**, so
//! `mouseevent` and `MOUSEEVENT` are both `MouseEvent`. And five entries in that table are *aliases
//! with no interface of their own* — `Events`, `HTMLEvents`, `SVGEvents` → `Event`, `MouseEvents` →
//! `MouseEvent`, `UIEvents` → `UIEvent`. `g['MouseEvents']` is `undefined`, so **every one of them
//! fell through to `|| g.Event` and came back as a plain `Event`.**
//!
//! That is not an archaeological detail: `MouseEvents` and `HTMLEvents` are the spellings jQuery's
//! `trigger` and Google Analytics actually emit, so a synthesised click was arriving as a bare
//! `Event` — no `clientX`, no `button`, and `instanceof MouseEvent` false — on every page that
//! drives one through the legacy API.
//!
//! ## The honest edge, asserted rather than hidden
//!
//! Three table entries name interfaces this engine does not implement (`TextEvent`,
//! `DeviceMotionEvent`, `DeviceOrientationEvent`), and `TouchEvent` is in the table **only when the
//! engine exposes it** — which is what the spec's own *"if the UA supports legacy touch events"*
//! clause means. For all of those `createEvent` throws `NotSupportedError`, which is the truthful
//! answer: we do not have the interface, and a page asking is entitled to hear so rather than
//! receive an `Event` wearing the name.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function proto(a) {
      try {
        var e = document.createEvent(a);
        var names = ['Event', 'MouseEvent', 'UIEvent', 'CustomEvent', 'KeyboardEvent', 'FocusEvent'];
        for (var i = 0; i < names.length; i++) {
          if (typeof window[names[i]] === 'function' &&
              Object.getPrototypeOf(e) === window[names[i]].prototype) { return names[i]; }
        }
        return 'OTHER';
      } catch (err) { return 'THROW:' + err.name; }
    }

    // ── 1. THE CUT: a name that is not in the table must THROW, and throw the right thing.
    p('unknown:' + proto('NotAnEvent'));
    p('nearMiss:' + proto('Eventss'));
    p('empty:' + proto(''));

    // ── 2. THE PLURAL ALIASES — no global of their own, so the old `g[iface]` lookup missed every
    //    one and handed back a bare `Event`. These are the spellings jQuery and GA actually emit.
    p('mouseEvents:' + proto('MouseEvents'));
    p('uiEvents:' + proto('UIEvents'));
    p('htmlEvents:' + proto('HTMLEvents'));
    p('svgEvents:' + proto('SVGEvents'));
    p('eventsAlias:' + proto('Events'));

    // ── 3. CASE-INSENSITIVE, which the old lookup was not.
    p('lower:' + proto('mouseevent'));
    p('upper:' + proto('MOUSEEVENT'));
    p('mixed:' + proto('MoUsEeVeNt'));

    // ── 4. THE RATCHET: the names that already worked must still work, and the event must still be
    //    created UNINITIALIZED (empty type) — the property `initEvent` exists to clear.
    p('event:' + proto('Event'));
    p('mouseEvent:' + proto('MouseEvent'));
    p('uiEvent:' + proto('UIEvent'));
    p('custom:' + proto('CustomEvent'));
    var e = document.createEvent('Event');
    p('uninitType:' + JSON.stringify(e.type));
    p('uninitTarget:' + e.target);
    p('uninitBubbles:' + e.bubbles + '/' + e.cancelable + '/' + e.defaultPrevented);
    e.initEvent('ping', true, true);
    p('afterInit:' + e.type + '/' + e.bubbles);
    var got = 'no';
    document.addEventListener('ping', function () { got = 'yes'; });
    document.dispatchEvent(e);
    p('dispatches:' + got);
  </script>
</body></html>"##;

#[test]
fn create_event_maps_the_fixed_alias_table_and_throws_for_everything_else() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://createevent.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CREATE-EVENT: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_CREATE_EVENT_ALIASES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "unknown:THROW:NotSupportedError",
        "THE LOAD-BEARING CLAIM. `createEvent('NotAnEvent')` returned a plain `Event`. The throw is \
         how a page FEATURE-DETECTS an event interface — an engine that never throws answers \
         'supported' for every name, so the library takes the modern branch and gets an `Event` with \
         the wrong prototype",
    ),
    (
        "nearMiss:THROW:NotSupportedError",
        "a pluralised non-alias (`Eventss`) is not in the table either — WPT asserts this shape \
         explicitly for every singular entry",
    ),
    ("empty:THROW:NotSupportedError", "the empty string is not an alias"),
    (
        "mouseEvents:MouseEvent",
        "⚠ THE CLAIM THE OLD LOOKUP COULD NOT PASS. `MouseEvents` is an ALIAS with no global of its \
         own, so `g['MouseEvents']` was `undefined` and it fell to `|| g.Event` — a synthesised click \
         arriving as a bare `Event` with no `clientX`, no `button`, and `instanceof MouseEvent` \
         false. It is the spelling jQuery's `trigger` and Google Analytics actually emit",
    ),
    ("uiEvents:UIEvent", "same alias shape, `UIEvents` → `UIEvent`"),
    ("htmlEvents:Event", "`HTMLEvents` → `Event`, the legacy spelling"),
    ("svgEvents:Event", "`SVGEvents` → `Event`"),
    ("eventsAlias:Event", "`Events` → `Event` — correct BEFORE only by accident, because the fallback happened to be `Event`"),
    (
        "lower:MouseEvent",
        "⚠ DOM §createEvent matches ASCII-CASE-INSENSITIVELY. The old lookup was a plain property \
         read, so `mouseevent` missed and fell back to `Event`",
    ),
    ("upper:MouseEvent", "and `MOUSEEVENT`"),
    ("mixed:MouseEvent", "and any mixture"),
    ("event:Event", "THE RATCHET. The name that always worked"),
    ("mouseEvent:MouseEvent", "THE RATCHET. The exact-case interface name"),
    ("uiEvent:UIEvent", "THE RATCHET"),
    ("custom:CustomEvent", "THE RATCHET"),
    (
        "uninitType:\"\"",
        "THE RATCHET. The event is created UNINITIALIZED — an empty type until `initEvent` runs. \
         Tightening the alias table must not change what the accepted names produce",
    ),
    ("uninitTarget:null", "THE RATCHET. `target` is null before dispatch"),
    (
        "uninitBubbles:false/false/false",
        "THE RATCHET. `bubbles`/`cancelable`/`defaultPrevented` all start false",
    ),
    (
        "afterInit:ping/true",
        "THE RATCHET. `initEvent` still initialises the event it is given",
    ),
    (
        "dispatches:yes",
        "THE RATCHET, and the whole point of the API: an event built this way must still REACH a \
         listener. A gate that only proved the throws would pass with `createEvent` deleted",
    ),
];
