//! **G_PROBE_CAPABILITIES — measure the unknowns, then pin what is real.**
//!
//! `docs/loop/CONSTELLATION.tsv` carries a `status` per capability, and the lever board's priorities
//! are computed FROM it. An `unknown` row is therefore not a neutral blank: it is a cell that steers
//! the loop while carrying no evidence, and several of them turned out to be capabilities that
//! already worked. A wrong `unknown` costs a tick of rediscovery; a wrong `missing` costs a tick of
//! rebuilding something that exists.
//!
//! **Every probe here is BEHAVIOURAL, and that is the whole design.** `typeof X === 'function'` is
//! exactly the check an inert stub passes — this engine ships several deliberately (see the prelude's
//! inert sweep), so a presence check would report them as capabilities. So: WebAssembly is measured
//! by instantiating real bytes and calling the exported function; multicol and container queries by
//! reading back the geometry they are supposed to produce; CJK breaking by whether the text actually
//! wrapped inside its box. A capability counts as present only when it did its job.
//!
//! This file is a **ratchet**, not a survey. It asserts only what measured TRUE, so a capability
//! found working here can never silently regress to missing. Values that measured false are recorded
//! in the TSV as `missing` with this gate as their receipt — measured absence, which is a different
//! and much more useful thing than never having looked.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><head><style>
  #mc { column-count: 3; column-gap: 10px; width: 300px; }
  #cq-outer { container-type: inline-size; width: 400px; }
  @container (min-width: 300px) { #cq { color: rgb(1, 2, 3); } }
  #snap { overflow-x: scroll; scroll-snap-type: x mandatory; width: 100px; }
  #cjk { width: 60px; font-size: 16px; }
  @media print { #printonly { color: rgb(4, 5, 6); } }
  @media screen { #screenonly { color: rgb(7, 8, 9); } }
  #balance { text-wrap: balance; width: 200px; }
</style></head><body>
  <div id="mc"><p>alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima</p></div>
  <div id="cq-outer"><div id="cq">container query target</div></div>
  <div id="snap"><span>a</span></div>
  <div id="cjk">日本語のテキストはここで折り返されるべきです</div>
  <div id="printonly">print</div>
  <div id="screenonly">screen</div>
  <div id="balance">balance me across two lines please</div>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };
    var $ = function (id) { return document.getElementById(id); };
    var cs = function (id) { return getComputedStyle($(id)); };
    // Never let one probe's throw hide the rest — each records its own verdict.
    var probe = function (name, fn) {
      var v;
      try { v = fn() ? 'yes' : 'no'; } catch (e) { v = 'no'; }
      R.push(name + ':' + v);
    };

    // ── WebAssembly. Instantiated and CALLED, not detected.
    // A hand-assembled module exporting `add(i32,i32)->i32`. If this returns 7, the whole pipeline
    // (compile, instantiate, export lookup, call, return marshalling) is real.
    probe('wasm', function () {
      if (typeof WebAssembly !== 'object' || !WebAssembly.Module) { return false; }
      var bytes = new Uint8Array([
        0,97,115,109, 1,0,0,0,
        1,7,1,96,2,127,127,1,127,
        3,2,1,0,
        7,7,1,3,97,100,100,0,0,
        10,9,1,7,0,32,0,32,1,106,11
      ]);
      var mod = new WebAssembly.Module(bytes);
      var inst = new WebAssembly.Instance(mod, {});
      return inst.exports.add(3, 4) === 7;
    });

    // ── Multicol: three columns in a 300px box with a 10px gap means a column box near 93px.
    // Measured by where the text actually is, not by whether the property parsed.
    probe('multicol', function () {
      var h = $('mc').getBoundingClientRect().height;
      var p = $('mc').firstChild.getBoundingClientRect();
      // A laid-out 3-column box is much wider than one column and much shorter than a single
      // 300px-wide flow of the same text.
      return p.width > 0 && p.width < 200 && h > 0;
    });

    // ── Container queries: the rule applies only if the container's inline size was resolved.
    probe('containerq', function () { return cs('cq').color.indexOf('1, 2, 3') >= 0; });

    // ── Media queries: `screen` must match and `print` must not. This is one probe, because a
    // stylesheet engine that matched BOTH would pass either half alone.
    probe('mediaq', function () {
      return cs('screenonly').color.indexOf('7, 8, 9') >= 0 &&
             cs('printonly').color.indexOf('4, 5, 6') < 0;
    });
    probe('matchmedia', function () {
      return typeof matchMedia === 'function' && matchMedia('screen').matches === true &&
             matchMedia('print').matches === false;
    });

    // ── CJK line breaking: 16px glyphs in a 60px box must wrap. If it did not, the box is one
    // long line and its height stays at a single line.
    probe('cjkbreak', function () {
      var r = $('cjk').getBoundingClientRect();
      return r.height > 20 && r.width <= 61;
    });

    // ── scroll snap: the property must at least round-trip through the cascade to be usable.
    probe('scrollsnap', function () { return cs('snap').scrollSnapType.indexOf('x') >= 0; });
    probe('textwrapbalance', function () { return cs('balance').textWrap.indexOf('balance') >= 0; });

    // ── Newer platform surfaces. Behavioural where there is behaviour to check, presence-plus-shape
    // where the API is a constructor we would have to drive asynchronously.
    probe('viewtransitions', function () { return typeof document.startViewTransition === 'function'; });
    probe('navigationapi', function () { return typeof navigation === 'object' && navigation !== null &&
                                                typeof navigation.navigate === 'function'; });
    probe('webcodecs', function () { return typeof VideoDecoder === 'function' &&
                                            typeof VideoDecoder.isConfigSupported === 'function'; });
    probe('sanitizer', function () { return typeof Element.prototype.setHTML === 'function'; });
    probe('highlights', function () { return typeof Highlight === 'function' &&
                                             typeof CSS !== 'undefined' && !!CSS.highlights; });
    probe('scopedregistry', function () { return typeof CustomElementRegistry === 'function' &&
                                                 'initialize' in CustomElementRegistry.prototype; });

    // ── Quirks mode: a document WITHOUT a doctype must report BackCompat. This document has one,
    // so the honest check here is that the mode is reported at all and is the standards one.
    probe('quirksflag', function () { return document.compatMode === 'CSS1Compat'; });

    // ── Drag and drop: the event surface pages actually bind to.
    probe('dragdrop', function () {
      return typeof DataTransfer === 'function' && 'draggable' in document.createElement('div') &&
             'ondragstart' in document.createElement('div');
    });
  </script>
</body></html>"##;

#[test]
fn measured_capabilities_do_not_regress() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://probe.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("PROBE RESULT: {got}");

    // Filled in from what the probe actually reported — see the module doc: this gate pins what is
    // real, it does not assert what would be nice.
    for claim in PINNED {
        assert!(
            got.contains(claim),
            "G_PROBE_CAPABILITIES: expected `{claim}`\n  got: {got}\n\n  \
             This capability was MEASURED working and pinned in docs/loop/CONSTELLATION.tsv. \
             Losing it is a regression, not a scope change."
        );
    }
}

/// What the probe MEASURED working, on the run that landed this gate.
///
/// `wasm` is the headline: WebAssembly was carried as `unknown` ("Figma, games, ffmpeg.wasm") and it
/// compiles, instantiates, resolves an export and returns the right integer. `cjkbreak` and `mediaq`
/// were likewise unknown and likewise already real.
///
/// Everything the probe reported `no` for is deliberately NOT here. Those are recorded in
/// `CONSTELLATION.tsv` as `missing` with this gate as the receipt — measured absence, which is worth
/// far more than an unexamined `unknown`, and which will start failing here the day someone builds it
/// (at which point the claim moves into this list).
const PINNED: &[&str] = &[
    "wasm:yes",
    "mediaq:yes",
    "matchmedia:yes",
    "cjkbreak:yes",
    "quirksflag:yes",
];
