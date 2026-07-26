//! **G_IFACE_SURFACE — the platform's interface objects exist, and answer `instanceof` correctly.**
//!
//! A browser exposes ~180 *interface objects* on `globalThis` — `HTMLMetaElement`, `Navigator`,
//! `HTMLTableCellElement`, `CanvasRenderingContext2D`. They are not decoration. Reading an absent one
//! is a **`ReferenceError`, and a `ReferenceError` kills the frame that read it.** It does not degrade,
//! it does not return `undefined`, and there is no `?.` a page can write to survive it.
//!
//! ## What this gate was built from: a top-1k site that rendered NOTHING
//!
//! `www.welt.de` is a HEAD site of the certification corpus (`docs/bench/corpus-v2.tsv`). Measured at
//! tick 608 it scored **0.0% coverage — 3,242 of 3,243 elements missing** — and the console said why:
//!
//! ```text
//! ReferenceError: HTMLMetaElement is not defined
//! ReferenceError: Navigator is not defined
//! ERROR page.console: Failed to load website due to adblock: Loader aborted: HTMLMetaElement is not defined
//! ```
//!
//! The site's loader probes interface objects. The probe threw, the site concluded it was being
//! **ad-blocked**, and it aborted its own boot. The page did not render *badly*; it rendered *nothing*,
//! from one absent global. That is the whole failure mode, and it is invisible to a box-diff — the
//! coverage number says "we render 0.0% of this site" and names no cause.
//!
//! A probe of the 183 interface objects a browser exposes found **63 absent here**. The 21 that existed
//! had each been added by whichever test happened to need it — a list shaped by *this loop's attention*
//! rather than by the platform, which is the same defect shape the tick-591 surface audit found in the
//! capability map.
//!
//! ## The rule this gate enforces, and why the negative half is not padding
//!
//! > **An interface object is defined IFF the thing it names exists in this engine.**
//!
//! `OffscreenCanvas` is therefore **deliberately absent** — `getContext` has no offscreen tier, so a
//! page's `'OffscreenCanvas' in window` must keep getting the honest answer `false`. This gate asserts
//! that absence, so a later tick cannot quietly turn the list into a *claim* instead of a *fact*. A
//! stub that names a capability we lack defeats feature-detection and is worse than the gap
//! (`DAILY-DRIVER-CERTIFICATION.md` §1; this repo's `honest-answer-is-not-a-fixed-answer` law).
//!
//! The **nine** `NEG_*` claims are the other half of the same discipline: an over-broad predicate is a
//! **wrong answer**, not a generous one. `<cite>` is plain `HTMLElement` in the spec, not `HTMLQuoteElement`;
//! a custom element (`<my-widget>`) is `HTMLElement`, not `HTMLUnknownElement`. Without those, a
//! predicate that simply returned `true` would pass every positive claim in this file.
//!
//! ## Named residue — NOT fixed here, and recorded so it is not mistaken for done
//!
//! * `CanvasRenderingContext2D.prototype` is **not in a context's prototype chain** (`getContext`
//!   builds a fresh object carrying own methods), so patching `…prototype.fillText` is accepted and
//!   **inert**. The interface object is honest — canvas 2D really does rasterize — but that patch path
//!   does not work yet. It needs the context to become a real reflector, which is its own tick.
//! * Still absent, each blocked on a capability rather than on this list: `OffscreenCanvas` (no
//!   offscreen tier — the honest negative above), `IDBFactory`/`IDBDatabase`/`IDBRequest`,
//!   `TextTrack`/`TextTrackCue`/`VTTCue`, `DOMStringMap`, `MessageEvent`. Their predicates need a
//!   distinguishing shape this tick did not establish; adding the name without one would be guessing.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta id="m" charset="utf-8"></head><body>
<p id="p">x</p><h3 id="h">y</h3><ul id="u"><li id="li">z</li></ul>
<table id="t"><caption id="cap">c</caption><thead id="th"><tr id="tr"><td id="td">1</td><th id="hd">2</th></tr></thead></table>
<fieldset id="fs"><legend id="lg">L</legend></fieldset><blockquote id="bq">q</blockquote>
<pre id="pre">p</pre><br id="br"><hr id="hr"><canvas id="c"></canvas>
<div id="out">-</div>
<script>
  var R = [], p = function (k, v) { R.push(k + '=' + v); };
  var $ = function (i) { return document.getElementById(i); };

  // ── THE POINT: the two globals that aborted welt.de's boot must simply be readable.
  p('meta_iface', typeof HTMLMetaElement);
  p('nav_iface', typeof Navigator);

  // ── POSITIVE: an element is an instance of ITS interface.
  p('meta', $('m') instanceof HTMLMetaElement);
  p('para', $('p') instanceof HTMLParagraphElement);
  p('heading', $('h') instanceof HTMLHeadingElement);
  p('li', $('li') instanceof HTMLLIElement);
  p('td', $('td') instanceof HTMLTableCellElement);
  p('th', $('hd') instanceof HTMLTableCellElement);
  p('thead', $('th') instanceof HTMLTableSectionElement);
  p('caption', $('cap') instanceof HTMLTableCaptionElement);
  p('fieldset', $('fs') instanceof HTMLFieldSetElement);
  p('quote', $('bq') instanceof HTMLQuoteElement);
  p('pre', $('pre') instanceof HTMLPreElement);

  // ── NEGATIVE: a WRONG tag must not match. Without these, `function(){return true}` passes above.
  p('NEG_div_para', $('out') instanceof HTMLParagraphElement);
  p('NEG_p_heading', $('p') instanceof HTMLHeadingElement);
  p('NEG_tr_cell', $('tr') instanceof HTMLTableCellElement);
  p('NEG_cite_quote', document.createElement('cite') instanceof HTMLQuoteElement);

  // ── The non-element singletons — identity, which is exact.
  p('navigator', navigator instanceof Navigator);
  p('performance', performance instanceof Performance);
  p('localStorage', localStorage instanceof Storage);
  p('sessionStorage', sessionStorage instanceof Storage);
  p('customElements', customElements instanceof CustomElementRegistry);
  p('crypto', crypto instanceof Crypto);
  p('subtle', crypto.subtle instanceof SubtleCrypto);
  p('impl', document.implementation instanceof DOMImplementation);
  p('NEG_nav_storage', navigator instanceof Storage);

  // ── CharacterData's third member, XHR's base, the canvas family, SVG.
  p('pi', document.createProcessingInstruction('t','d') instanceof ProcessingInstruction);
  p('NEG_text_pi', document.createTextNode('a') instanceof ProcessingInstruction);
  p('xhrbase', new XMLHttpRequest() instanceof XMLHttpRequestEventTarget);
  p('ctx2d', $('c').getContext('2d') instanceof CanvasRenderingContext2D);
  p('gradient', $('c').getContext('2d').createLinearGradient(0,0,1,1) instanceof CanvasGradient);
  p('NEG_ctx_gradient', $('c').getContext('2d') instanceof CanvasGradient);
  p('svgpath', document.createElementNS('http://www.w3.org/2000/svg','path') instanceof SVGPathElement);

  // ── HTMLUnknownElement: an undefined tag is unknown; a CUSTOM element is not.
  p('unknown', document.createElement('foobar') instanceof HTMLUnknownElement);
  p('NEG_custom_unknown', document.createElement('my-widget') instanceof HTMLUnknownElement);
  p('NEG_div_unknown', $('out') instanceof HTMLUnknownElement);

  // ── THE HONEST NEGATIVE. We have no offscreen canvas tier; saying so must stay true.
  p('offscreen_absent', typeof OffscreenCanvas === 'undefined');

  $('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn platform_interface_objects_exist_and_answer_instanceof() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://iface.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // VACUITY GUARD: if the script never ran, `got` is the placeholder and every `contains` below
    // would be vacuously checked against nothing. A gate whose fixture silently failed to execute
    // reports green forever — this project has booked that shape more than once.
    assert!(
        got.contains("meta_iface=") && got.len() > 200,
        "VACUITY GUARD: the fixture script did not run to completion — every claim below would be \
         measuring a blank string, not the engine. got: {got:?}"
    );

    for (claim, why) in [
        (
            "meta_iface=function",
            "THE POINT: `HTMLMetaElement` must be readable. Its absence is a ReferenceError that \
             aborted www.welt.de's loader outright (it concluded it was ad-blocked), leaving 3,242 \
             of 3,243 elements unrendered",
        ),
        (
            "nav_iface=function",
            "THE POINT: `Navigator` must be readable — the second global welt.de's loader threw on",
        ),
        ("meta=true", "a <meta> is an HTMLMetaElement"),
        ("para=true", "a <p> is an HTMLParagraphElement"),
        ("heading=true", "an <h3> is an HTMLHeadingElement (one interface over h1-h6)"),
        ("li=true", "an <li> is an HTMLLIElement"),
        ("td=true", "a <td> is an HTMLTableCellElement"),
        ("th=true", "a <th> is an HTMLTableCellElement too — one interface, two tags"),
        ("thead=true", "a <thead> is an HTMLTableSectionElement"),
        ("caption=true", "a <caption> is an HTMLTableCaptionElement"),
        ("fieldset=true", "a <fieldset> is an HTMLFieldSetElement"),
        ("quote=true", "a <blockquote> is an HTMLQuoteElement"),
        ("pre=true", "a <pre> is an HTMLPreElement"),
        (
            "NEG_div_para=false",
            "a <div> is NOT an HTMLParagraphElement — an over-broad predicate is a WRONG answer, and \
             without this claim a test that always returns true would pass every positive above",
        ),
        ("NEG_p_heading=false", "a <p> is NOT an HTMLHeadingElement"),
        ("NEG_tr_cell=false", "a <tr> is NOT an HTMLTableCellElement"),
        (
            "NEG_cite_quote=false",
            "a <cite> is NOT an HTMLQuoteElement — the spec gives it plain HTMLElement, and only \
             <blockquote>/<q> are quotes",
        ),
        ("navigator=true", "`navigator instanceof Navigator`"),
        ("performance=true", "`performance instanceof Performance`"),
        (
            "localStorage=true",
            "`localStorage instanceof Storage` — how a page tells a real storage object from a \
             polyfilled {} before trusting it with a quota check",
        ),
        ("sessionStorage=true", "Storage has TWO instances, and both must answer"),
        ("customElements=true", "`customElements instanceof CustomElementRegistry`"),
        ("crypto=true", "`crypto instanceof Crypto`"),
        ("subtle=true", "`crypto.subtle instanceof SubtleCrypto`"),
        (
            "impl=true",
            "`document.implementation instanceof DOMImplementation` — duck-typed, because it is \
             per-document and identity against the main document's would be wrong for an iframe's",
        ),
        ("NEG_nav_storage=false", "navigator is NOT a Storage"),
        (
            "pi=true",
            "a ProcessingInstruction (nodeType 7) completes the CharacterData family alongside Text \
             and Comment, which were already here",
        ),
        ("NEG_text_pi=false", "a Text node is NOT a ProcessingInstruction"),
        (
            "xhrbase=true",
            "every XMLHttpRequest is an XMLHttpRequestEventTarget — its WebIDL base",
        ),
        (
            "ctx2d=true",
            "a 2D context is a CanvasRenderingContext2D. Canvas 2D genuinely rasterizes here \
             (G_CANVAS), so naming its interface is a TRUE statement — note the prototype-patch \
             residue named in this file's header",
        ),
        ("gradient=true", "a gradient is a CanvasGradient"),
        ("NEG_ctx_gradient=false", "a context is NOT a CanvasGradient"),
        ("svgpath=true", "an SVG <path> is an SVGPathElement (SVG tag names stay lowercase)"),
        (
            "unknown=true",
            "`document.createElement('foobar')` is an HTMLUnknownElement — the interface of a tag \
             the HTML spec does not define",
        ),
        (
            "NEG_custom_unknown=false",
            "a CUSTOM element (<my-widget>, a name with a dash) is an HTMLElement, NOT an \
             HTMLUnknownElement. Getting this backwards is the one way the predicate can be \
             actively wrong",
        ),
        ("NEG_div_unknown=false", "a <div> is a known tag, so not unknown"),
        (
            "offscreen_absent=true",
            "**THE HONEST NEGATIVE.** `getContext` has no offscreen tier, so `OffscreenCanvas` must \
             stay undefined and `'OffscreenCanvas' in window` must keep answering false. An \
             interface object is defined IFF the thing it names EXISTS; a stub naming a capability \
             we lack defeats feature-detection and is worse than the gap. If a later tick adds the \
             name without the capability, this claim is what stops it",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_IFACE_SURFACE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
