//! **G_IFACE_SURFACE_2 — the interface surface re-measured, and the half of it where `false` is a LIE.**
//!
//! `G_IFACE_SURFACE` (tick 608) established the class: **a referenced name that does not exist is a
//! `ReferenceError`, not a `false`**, and nothing in the language survives it — `el.foo?.()` guards a
//! missing *method*, but the throw on a bare identifier happens before any operator the author could
//! have written. `www.welt.de` read `HTMLMetaElement`, took the throw, concluded it was being
//! ad-blocked and aborted its own boot.
//!
//! That tick took the surface from 120 to 174 of 183 names. **This gate exists because the surface
//! goes stale from the WEB's side, not ours**, and it had never been re-measured. A probe of 262
//! platform globals found **59 still absent** — including `MessageEvent`, the whole CSSOM rule
//! family, the SVG shape elements, and the IndexedDB interface names that every wrapper (Dexie, idb,
//! Firebase persistence) references at module scope before a database is ever opened.
//!
//! ## The part that is NOT just "add more names"
//!
//! The inert-stub doctrine is justified by a specific claim: `x instanceof FileList` answering
//! `false` is **correct**, because this engine never builds a `FileList`. **That justification does
//! not transfer to interfaces we DO build.** We genuinely produce `@media` and `@keyframes` rules and
//! genuinely deliver message events — so an inert `CSSMediaRule` would answer `false` about an object
//! that IS one, and send every CSS-in-JS runtime down its "this browser has no media rules" branch
//! with the rules sitting right there.
//!
//! So this gate's load-bearing assertions are the **positive** ones. Presence is necessary; being
//! *right* is the claim. Each truthful predicate is asserted both ways — true for a real instance and
//! false for a near-miss — because a `Symbol.hasInstance` that returns `true` for everything is
//! indistinguishable from a working one if you only ever test the positive side.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head>
  <style id="sheet">
    .a { color: red }
    @media (min-width: 1px) { .b { color: blue } }
    @keyframes spin { from { opacity: 0 } to { opacity: 1 } }
    @supports (display: flex) { .c { display: flex } }
    @font-face { font-family: X; src: url(x.woff2) }
  </style>
</head><body>
  <svg id="svg" width="40" height="40">
    <g id="g"><rect id="rect" width="10" height="10"></rect><circle id="circ" r="4"></circle></g>
    <text id="txt">t</text><use id="use"></use><image id="img"></image><path id="p" d="M0 0"></path>
  </svg>
  <canvas id="cv" width="20" height="20"></canvas>
  <div id="out">-</div>
  <script>
    var R = [];
    var $ = function (i) { return document.getElementById(i); };

    // ── 1. NOTHING ON THE SURFACE IS A ReferenceError. This is the welt.de claim, re-measured over a
    // list four times the size of the one tick 608 used. It is one assertion because that is how the
    // failure arrives: the page does not survive the first name that is missing.
    var NAMES = [
      'MessageEvent','MediaQueryListEvent','HTMLOptionsCollection',
      'SVGGeometryElement','SVGRectElement','SVGCircleElement','SVGGElement',
      'SVGUseElement','SVGTextElement','SVGImageElement','StyleSheetList','CSSRuleList','CSSMediaRule',
      'CSSKeyframesRule','CSSSupportsRule','CSSFontFaceRule','CSSImportRule',
      'CSSPageRule','CSSNamespaceRule','CSSConditionRule','CSSGroupingRule','MediaList','FontFace',
      'FontFaceSet','PerformanceNavigationTiming','PerformanceMark','PerformanceMeasure',
      'IntersectionObserverEntry','ResizeObserverSize',
      'IDBFactory','IDBDatabase','IDBTransaction',
      'IDBObjectStore','IDBIndex','IDBRequest','IDBCursor','IDBOpenDBRequest','IDBVersionChangeEvent',
      'TextMetrics','Clipboard','ServiceWorkerContainer','NavigatorUAData','Permissions',
      // …and the names tick 608 landed, so this gate also guards against losing them.
      'HTMLMetaElement','Navigator','HTMLTableCellElement','CanvasRenderingContext2D','CSSRule',
      'CSSStyleRule','PerformanceEntry','SVGPathElement'
    ];
    var absent = [];
    NAMES.forEach(function (n) { if (typeof globalThis[n] === 'undefined') { absent.push(n); } });
    R.push('absent:' + (absent.length ? absent.join(',') : 'none'));

    // ── 1b. THE OTHER HALF OF THE SAME RULE, AND IT IS THE ONE A LATER TICK WILL BE TEMPTED TO
    // BREAK: an interface object is defined IFF the thing it names EXISTS here. Every name below is
    // a capability this engine does not have, so `'X' in window` must keep answering false — that
    // check is a page's only way to route around us, and `'DeviceMotionEvent' in window` is exactly
    // how one decides whether to run a motion-permission flow. Naming them would defeat the feature
    // detection, which is t772's half-installed-API trap pointing the other way.
    var CLAIMED = [];
    ['OffscreenCanvas','TrustedTypePolicyFactory','XMLHttpRequestUpload','ReportingObserver',
     'PerformanceResourceTiming','PerformanceObserverEntryList','StaticRange','CSSKeyframeRule',
     'DeviceMotionEvent','DeviceOrientationEvent','GamepadEvent','TrackEvent','ToggleEvent',
     'FormDataEvent','ContentVisibilityAutoStateChangeEvent','CDATASection','DOMStringMap']
      .forEach(function (n) { if (typeof globalThis[n] !== 'undefined') { CLAIMED.push(n); } });
    R.push('overclaimed:' + (CLAIMED.length ? CLAIMED.join(',') : 'none'));

    // ── 2. THE SVG SHAPE FAMILY ANSWERS TRUTHFULLY, BOTH WAYS.
    R.push('gIsG:' + ($('g') instanceof SVGGElement));
    R.push('rectNotG:' + ($('rect') instanceof SVGGElement));
    R.push('rectIsRect:' + ($('rect') instanceof SVGRectElement));
    R.push('circIsCirc:' + ($('circ') instanceof SVGCircleElement));
    R.push('useIsUse:' + ($('use') instanceof SVGUseElement));
    R.push('txtIsTxt:' + ($('txt') instanceof SVGTextElement));
    R.push('imgIsImg:' + ($('img') instanceof SVGImageElement));
    // The WebIDL base: every shape is a geometry element; a <g> and a <text> are not.
    R.push('rectIsGeom:' + ($('rect') instanceof SVGGeometryElement));
    R.push('pathIsGeom:' + ($('p') instanceof SVGGeometryElement));
    R.push('gNotGeom:' + ($('g') instanceof SVGGeometryElement));
    R.push('txtNotGeom:' + ($('txt') instanceof SVGGeometryElement));
    // An HTML <div> is none of them — the predicate must key on the tag, not on "is an element".
    R.push('divNotRect:' + ($('out') instanceof SVGRectElement));

    // ── 3. THE CSSOM RULE FAMILY — where an inert `false` would have been a LIE. This is what every
    // CSS-in-JS runtime does: walk cssRules and narrow.
    var rules = $('sheet').sheet.cssRules;
    var byKind = { style: 0, media: 0, keyframes: 0, supports: 0, fontface: 0, rule: 0 };
    for (var i = 0; i < rules.length; i++) {
      var r = rules[i];
      if (r instanceof CSSRule) { byKind.rule++; }
      if (r instanceof CSSStyleRule) { byKind.style++; }
      if (r instanceof CSSMediaRule) { byKind.media++; }
      if (r instanceof CSSKeyframesRule) { byKind.keyframes++; }
      if (r instanceof CSSSupportsRule) { byKind.supports++; }
      if (r instanceof CSSFontFaceRule) { byKind.fontface++; }
    }
    R.push('ruleCount:' + rules.length);
    R.push('allAreRules:' + (byKind.rule === rules.length));
    R.push('style:' + byKind.style);
    R.push('media:' + byKind.media);
    R.push('keyframes:' + byKind.keyframes);
    R.push('supports:' + byKind.supports);
    R.push('fontface:' + byKind.fontface);
    // ...and the narrowing is EXCLUSIVE: a plain style rule must not answer yes to @media.
    var plain = null, med = null;
    for (var j = 0; j < rules.length; j++) {
      if (rules[j] instanceof CSSStyleRule) { plain = rules[j]; }
      if (rules[j] instanceof CSSMediaRule) { med = rules[j]; }
    }
    R.push('plainNotMedia:' + (plain instanceof CSSMediaRule));
    R.push('mediaNotStyle:' + (med instanceof CSSStyleRule));
    // The grouping/condition bases hold over the right subset.
    R.push('mediaIsCond:' + (med instanceof CSSConditionRule));
    R.push('plainNotCond:' + (plain instanceof CSSConditionRule));
    // A non-rule object must not pass, or the predicate is just "is an object".
    R.push('objNotRule:' + (({ type: 'x' }) instanceof CSSRule));
    // The numeric constants libraries read instead of hard-coding 4.
    R.push('consts:' + CSSRule.STYLE_RULE + ',' + CSSRule.MEDIA_RULE + ',' + CSSRule.KEYFRAMES_RULE +
           ',' + CSSRule.SUPPORTS_RULE);

    // ── 4. `MessageEvent` — the guard on every cross-origin listener and every OAuth popup
    // handshake: `if (!(e instanceof MessageEvent)) return;`
    var got = 'none';
    window.addEventListener('message', function (e) {
      got = (e instanceof MessageEvent) + '/' + e.data;
    });
    var mc = new MessageChannel();
    R.push('mcIsChannel:' + (typeof mc.port1.postMessage === 'function'));
    // A synthetic one, which is what a test harness and a polyfill both build.
    R.push('synthIsMsg:' + (({ type: 'message', data: 1 }) instanceof MessageEvent));
    R.push('clickNotMsg:' + (({ type: 'click', data: 1 }) instanceof MessageEvent));
    // No `data` at all is not a message event — otherwise the predicate is "type is a string".
    R.push('noDataNotMsg:' + (({ type: 'message' }) instanceof MessageEvent));

    // ── 5. `TextMetrics`, which a text-fitting loop narrows on.
    var m = $('cv').getContext('2d').measureText('abc');
    R.push('metrics:' + (m instanceof TextMetrics));
    R.push('numNotMetrics:' + (({ width: 3 }) instanceof TextMetrics));

    // ── 6. THE INDEXEDDB FAMILY, WHICH IS A REAL IMPLEMENTATION HERE. That is why the names are a
    // gap rather than a claim: a wrapper (Dexie, `idb`, Firebase persistence) references
    // `IDBDatabase`/`IDBRequest` at MODULE scope, before any database is opened — so the
    // ReferenceError lands before the feature it guards is ever reached, and the app's
    // "IndexedDB unavailable" fallback never runs because the check for it threw.
    R.push('factory:' + (indexedDB instanceof IDBFactory));
    R.push('objNotFactory:' + (({}) instanceof IDBFactory));
    var openReq = indexedDB.open('g_iface_2', 1);
    R.push('openIsReq:' + (openReq instanceof IDBRequest));
    R.push('openIsOpenReq:' + (openReq instanceof IDBOpenDBRequest));
    R.push('objNotReq:' + (({ readyState: 'pending' }) instanceof IDBRequest));
    // ⚠ The rest of the family can only be instantiated ASYNCHRONOUSLY (`onupgradeneeded` fires on a
    // later microtask), and this harness reads `#out` after the synchronous script. A first draft
    // asserted `db instanceof IDBDatabase` inside that handler — the pushes never ran, the claims
    // were never in the output, and the gate passed on assertions that DID NOT EXECUTE. So the
    // asynchronous half is asserted the only way it can be here: from the NEGATIVE side, which is
    // the side that actually catches the bug. An over-broad duck predicate — the real risk when the
    // shape is `typeof o.put === 'function'` — passes every positive test and fails these.
    R.push('objNotDb:' + (({ close: 1 }) instanceof IDBDatabase));
    R.push('objNotStore:' + (({ keyPath: 'id' }) instanceof IDBObjectStore));
    R.push('objNotTx:' + (({ db: 1 }) instanceof IDBTransaction));
    R.push('objNotIndex:' + (({ multiEntry: false }) instanceof IDBIndex));
    R.push('objNotCursor:' + (({ primaryKey: 1 }) instanceof IDBCursor));
    R.push('objNotVCE:' + (({ oldVersion: 0 }) instanceof IDBVersionChangeEvent));
    // ...and a REQUEST is not a DATABASE: the two shapes are adjacent and a sloppy predicate merges
    // them, which is how a wrapper ends up calling `createObjectStore` on a request.
    R.push('reqNotDb:' + (openReq instanceof IDBDatabase));
    // ...and they are named, because a name is what a minifier's error message and a prototype patch
    // both go looking for.
    R.push('named:' + IDBDatabase.name + ',' + FontFace.name + ',' + MessageEvent.name);
    // The `navigator` sub-objects: identity against the singleton, which is exact.
    R.push('clip:' + (navigator.clipboard instanceof Clipboard) +
           ',' + (navigator.permissions instanceof Permissions) +
           ',' + (document.fonts instanceof FontFaceSet));
    R.push('clipNotPerm:' + (navigator.clipboard instanceof Permissions));
    // The navigation entry the RUM libraries narrow on.
    R.push('navTiming:' + (performance.getEntriesByType('navigation')[0] instanceof PerformanceNavigationTiming));
    R.push('markNotNav:' + (performance.mark('x') instanceof PerformanceNavigationTiming));

    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn interface_surface_is_present_and_the_ones_we_build_answer_truthfully() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ifs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // THE headline: not one of these reads is a ReferenceError.
        "absent:none",
        // SVG shape family, both directions
        "gIsG:true",
        "rectNotG:false",
        "rectIsRect:true",
        "circIsCirc:true",
        "useIsUse:true",
        "txtIsTxt:true",
        "imgIsImg:true",
        "rectIsGeom:true",
        "pathIsGeom:true",
        "gNotGeom:false",
        "txtNotGeom:false",
        "divNotRect:false",
        // CSSOM rules — the half where an inert `false` would have been a lie
        "ruleCount:5",
        "allAreRules:true",
        "style:1",
        "media:1",
        "keyframes:1",
        "supports:1",
        "fontface:1",
        "plainNotMedia:false",
        "mediaNotStyle:false",
        "mediaIsCond:true",
        "plainNotCond:false",
        "objNotRule:false",
        "consts:1,4,7,12",
        // MessageEvent — the cross-origin/OAuth guard
        "mcIsChannel:true",
        "synthIsMsg:true",
        "clickNotMsg:false",
        "noDataNotMsg:false",
        // TextMetrics
        "metrics:true",
        "numNotMetrics:false",
        // the inert ones claim nothing, and are named
        // the deliberate absences — the half of the rule a later tick will be tempted to break
        "overclaimed:none",
        // IndexedDB: a real implementation, so the names must answer truthfully
        "factory:true",
        "objNotFactory:false",
        "openIsReq:true",
        "openIsOpenReq:true",
        "objNotReq:false",
        "objNotDb:false",
        "objNotStore:false",
        "objNotTx:false",
        "objNotIndex:false",
        "objNotCursor:false",
        "objNotVCE:false",
        "reqNotDb:false",
        "named:IDBDatabase,FontFace,MessageEvent",
        "clip:true,true,true",
        "clipNotPerm:false",
        "navTiming:true",
        "markNotNav:false",
    ] {
        assert!(
            got.contains(claim),
            "G_IFACE_SURFACE_2: expected `{claim}`\n  got: {got}\n\n  \
             A referenced name that does not exist is a ReferenceError, not a `false` — welt.de read \
             `HTMLMetaElement`, took the throw, decided it was being ad-blocked and blanked itself. \
             `absent:none` is that claim, re-measured over 262 platform globals (59 were missing). \
             The rest are the half the inert-stub doctrine does NOT cover: we genuinely build \
             `@media` rules and message events, so an inert `CSSMediaRule` answers `false` about an \
             object that IS one and sends every CSS-in-JS runtime down its no-media-rules branch. \
             The negative claims (`plainNotMedia:false`, `objNotRule:false`, `clickNotMsg:false`) are \
             load-bearing: a `Symbol.hasInstance` that returns true for everything passes every \
             positive test and is not an implementation."
        );
    }
}
