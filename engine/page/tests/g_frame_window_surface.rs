//! # G_FRAME_WINDOW_SURFACE — an `<iframe>`'s window had TWO properties, and one of them was `location`
//!
//! Measured, inside the harness, on a real loaded frame:
//!
//! ```text
//!   HAVE    [document, location]
//!   MISSING [DOMException, Node, Element, HTMLElement, Event, Document,
//!            NodeFilter, Range, getComputedStyle, window, self, Object, Array, Function]
//! ```
//!
//! `contentWindow` was a hand-rolled object literal carrying `document`, `frameElement`, `location`
//! and three no-ops. **Every platform interface object vanished at the frame boundary** — and a
//! script inside a frame, or reaching into one, addresses the platform through `d.defaultView`.
//!
//! The concentrated cost is one line of WPT's own harness:
//!
//! ```js
//!   assert_throws_dom("SyntaxError", root.ownerDocument.defaultView.DOMException, () => …)
//! ```
//!
//! `defaultView.DOMException` was `undefined`, so `assert_throws_dom` died reading `.name` off it —
//! **204 `dom` and 76 `css/selectors` subtests failed before any behaviour was tested.** The tests
//! were not failing; they could not be *stated*.
//!
//! ## Inheriting the parent's globals is the TRUTH here, not a pretence
//!
//! This engine gives a frame its own **document** (`contentDocument` identity, its own arena) but
//! **not its own JS realm** — that is written down in `iframe_js`'s module docs as a stated limit.
//! One realm means `frameWin.Node` and `Node` really *are* the same object, and
//! `e instanceof frameWin.DOMException` really *is* the right answer for an exception this realm
//! threw. So the frame window now inherits the parent global rather than being empty, and
//! `sameConstructor` below asserts the identity rather than merely the presence — because if a
//! separate realm ever lands, that claim is exactly what must change.
//!
//! ## Why a Proxy and not a prototype chain — two things a prototype cannot do
//!
//! 1. **`getComputedStyle` must stay ABSENT, not shadowed by `undefined`.** A property that exists
//!    and answers `undefined` is a feature-detection trap. Its absence is *deliberate and reasoned*:
//!    `STYLES_PTR` is a single thread-local holding ONE page's style map, so a frame node looked up
//!    there returns the **parent's** style. Exposing it converts a documented absence into a
//!    silently wrong answer. `'getComputedStyle' in frameWin` must be **false**.
//! 2. **A write must land on the frame's OWN object**, and `Object.keys` must not enumerate the
//!    parent global. Every embed stashes a ready flag or a message-port handle on its window; those
//!    must not become properties of the top-level page.

use manuk_text::FontContext;

/// The parent. The frame's content is rendered in by `render_iframe` below rather than fetched —
/// `srcdoc` is not wired to this path, and a fixture whose frame never loads makes every claim here
/// vacuously true, which is what `frameLoaded` guards against.
const PARENT: &str = r#"<!doctype html><html><body>
  <iframe src="https://embed.test/c" id="f"></iframe>
  <div id="out">-</div>
  <script>window.__parentRan = 1;</script>
</body></html>"#;

/// ⚠ The child hides `#inner` **with its own stylesheet**, and the parent says nothing about that
/// node. That asymmetry is the entire point: a `getComputedStyle` that resolves a frame `NodeId`
/// against the PARENT's style map answers `visible` — a wrong answer of the right type — and only a
/// fixture where the two documents disagree can tell the two implementations apart.
const CHILD: &str = r#"<!doctype html><html><head><style>#inner { visibility: hidden; }</style></head>
<body><p id="inner">hello</p></body></html>"#;

const PROBE: &str = r#"
  var R = [];
  function p(s) {
    R.push(s);
    var sc = document.getElementById('__fws__');
    if (!sc) { sc = document.createElement('script'); sc.id = '__fws__'; sc.type = 'application/json';
               document.documentElement.appendChild(sc); }
    sc.textContent = R.join(' ');
  }
  var f = document.getElementById('f');
  var w = f.contentWindow, d = f.contentDocument;

  p('frameLoaded:' + (!!w && !!d));

  // ── 1. THE PLATFORM IS THERE AT ALL. Each of these was `undefined` on a frame window.
  var names = ['DOMException', 'Node', 'Element', 'HTMLElement', 'Event', 'Document',
               'NodeFilter', 'Range', 'Object', 'Array', 'Function', 'setTimeout'];
  var missing = names.filter(function (n) { return !w || typeof w[n] === 'undefined'; });
  p('missing[' + missing.join(',') + ']');

  // ── 2. IDENTITY, not merely presence. One realm ⇒ the SAME objects, and that is the claim a
  //    future per-frame realm would have to change.
  p('sameConstructor:' + (w.DOMException === DOMException && w.Node === Node));

  // ── 3. The properties a prototype chain would have answered with the PARENT'S value.
  p('windowSelf:' + (w.window === w) + '/' + (w.self === w));
  p('parentTop:' + (w.parent === globalThis) + '/' + (w.top === globalThis));

  // ── 4. `getComputedStyle` THROUGH THE FRAME'S WINDOW, ANSWERING ABOUT THE FRAME'S OWN ELEMENT.
  //    Withheld until t1202 because `STYLES_PTR` held ONE page's style map, so a frame node resolved
  //    against the PARENT's styles. Now arena-aware — and the claim is not that the property exists
  //    but that its ANSWER is the child's, which is the only version a wrong lookup fails.
  p('gcsPresent:' + (typeof w.getComputedStyle));
  p('gcsChildAnswer:' + w.getComputedStyle(d.getElementById('inner')).visibility);
  p('gcsNotParent:' + (w.getComputedStyle(d.getElementById('inner')).visibility
                       !== getComputedStyle(document.getElementById('out')).visibility));

  // ── 5. A WRITE MUST NOT ESCAPE INTO THE PARENT GLOBAL.
  w.__manukFrameFlag = 42;
  p('writeIsolated:' + (w.__manukFrameFlag === 42) + '/' + (typeof globalThis.__manukFrameFlag));
  p('keysOwnOnly:' + (Object.keys(w).indexOf('setTimeout') < 0));

  // ── 6. THE RATCHET. Everything the window already did must still work.
  p('docIdentity:' + (w.document === d));
  p('defaultViewRoundTrip:' + (d.defaultView === w));
  p('windowIdentity:' + (f.contentWindow === w));
  p('frameElement:' + (w.frameElement === f));
  p('locationStill:' + (typeof w.location.href === 'string'));
  p('postMessageStill:' + (typeof w.postMessage));
  p('innerReadable:' + (d.getElementById('inner') ? d.getElementById('inner').textContent : 'NONE'));
  p('divHasNoWindow:' + (typeof document.getElementById('out').contentWindow));
"#;

#[test]
fn a_frames_window_carries_the_platform_and_still_hides_what_it_cannot_answer() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(PARENT, "https://parent.test/", &fonts, 900.0);
    let root = page.dom().root();
    let fnode = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    page.render_iframe(fnode, CHILD, "https://embed.test/c", &fonts, 0);
    page.eval_for_test(PROBE);
    let dom = page.dom();
    let out = manuk_css::query_selector_all(dom, dom.root(), "#__fws__");
    let got = out
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default();
    println!("FRAME-WINDOW: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_FRAME_WINDOW_SURFACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "frameLoaded:true",
        "VACUITY: with no frame window or document, every claim below is satisfied by nothing \
         happening",
    ),
    (
        "missing[]",
        "⚠ THE LOAD-BEARING CLAIM. Every one of these twelve was `undefined` on a frame window, so \
         a script inside a frame — or reaching into one through `d.defaultView` — found no platform \
         at all. WPT's own `assert_throws_dom(type, root.ownerDocument.defaultView.DOMException, fn)` \
         died reading `.name` off `undefined`: 204 `dom` + 76 `css/selectors` subtests could not be \
         STATED, let alone pass",
    ),
    (
        "sameConstructor:true",
        "IDENTITY, not presence. This engine gives a frame its own document but NOT its own JS \
         realm, so `frameWin.Node` and `Node` genuinely are one object — inheriting them is the \
         truth about this engine, not a pretence. ⚠ If a per-frame realm ever lands, THIS is the \
         claim that must change, which is why it is asserted rather than assumed",
    ),
    (
        "windowSelf:true/true",
        "`w.window` and `w.self` must be the FRAME's window. A prototype chain would have answered \
         both with the parent's, which is the single most-read property on a window object",
    ),
    (
        "parentTop:true/true",
        "`parent`/`top` point at the containing global — how a framed script asks 'am I embedded'",
    ),
    (
        "gcsPresent:function",
        "`getComputedStyle` reaches the frame window. Withheld until t1202 — see the next claim for \
         why its mere presence is not the interesting half",
    ),
    (
        "gcsChildAnswer:hidden",
        "⚠⚠ THE LOAD-BEARING HALF. The child's own stylesheet hides `#inner`; the PARENT's styles \
         say nothing about that node. Before t1202 the lookup had one style map (`STYLES_PTR`) and \
         resolved a frame `NodeId` against the parent's — a wrong answer of the RIGHT TYPE, which is \
         why the property was withheld rather than shipped. Asserting the ANSWER, not the presence, \
         is what makes an arena-blind lookup fail this",
    ),
    (
        "gcsNotParent:true",
        "and the two documents must DISAGREE, so a lookup that silently reads the parent's map \
         cannot pass by coincidence",
    ),
    (
        "writeIsolated:true/undefined",
        "⚠ A write to the frame's window must NOT create a property on the parent global. Every \
         embed stashes a ready flag or a message-port handle on its own window",
    ),
    (
        "keysOwnOnly:true",
        "`Object.keys(frameWin)` must enumerate the frame's own properties, not the entire parent \
         global — the other thing a prototype chain gets wrong",
    ),
    (
        "docIdentity:true",
        "THE RATCHET. `contentWindow.document === contentDocument` (t1193)",
    ),
    (
        "defaultViewRoundTrip:true",
        "THE RATCHET. `contentDocument.defaultView === contentWindow` — the round trip WPT's \
         harness actually walks",
    ),
    (
        "windowIdentity:true",
        "THE RATCHET. ONE window object per frame — it used to be rebuilt on every read, so anything \
         a script stashed was written to an object thrown away on the next line",
    ),
    ("frameElement:true", "THE RATCHET. The frame's own element"),
    ("locationStill:true", "THE RATCHET. `location.href` still answers"),
    ("postMessageStill:function", "THE RATCHET. The no-op is still callable"),
    (
        "innerReadable:hello",
        "THE RATCHET. The frame's DOM is still readable through its own document — the capability \
         all of this hangs off",
    ),
    (
        "divHasNoWindow:undefined",
        "THE RATCHET. Only elements with a nested browsing context have `contentWindow`. A `<div>` \
         must not grow one — `if ('contentWindow' in el)` is a real feature detect and must take the \
         right branch on every ordinary element",
    ),
];
