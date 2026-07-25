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
  #sg-outer { display: grid; grid-template-columns: repeat(3, 1fr); }
  #sg { display: grid; grid-column: span 3; grid-template-columns: subgrid; }
  @scope (#scope-root) { #scoped { color: rgb(10, 20, 30); } }
  #anchor-el { anchor-name: --a; }
  #anchored { position: absolute; position-anchor: --a; top: anchor(bottom); }
  #attrbox { width: attr(data-w type(<length>), 50px); }
  #sda { animation-timeline: scroll(); }
  #fs { field-sizing: content; }
</style></head><body>
  <div id="mc"><p>alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima</p></div>
  <div id="cq-outer"><div id="cq">container query target</div></div>
  <div id="snap"><span>a</span></div>
  <div id="cjk">日本語のテキストはここで折り返されるべきです</div>
  <div id="printonly">print</div>
  <div id="screenonly">screen</div>
  <div id="balance">balance me across two lines please</div>
  <div id="sg-outer"><div id="sg"><span>a</span></div></div>
  <div id="scope-root"><div id="scoped">scoped</div></div>
  <div id="anchor-el">anchor</div><div id="anchored">anchored</div>
  <div><textarea id="fsref" cols="40">ok</textarea><textarea id="fs" cols="40">ok</textarea></div>
  <div id="attrbox" data-w="120px">attr</div>
  <div id="sda">scroll driven</div>
  <div id="lhbox" style="line-height:20px; width:5lh">lh</div>
  <div id="cvon">shown</div><div id="cvoff" style="display:none">hidden</div><div id="cvvis" style="visibility:hidden">invisible</div>
  <video id="vidmuted" muted></video><video id="vidloud"></video>
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

    // ── WasmGC (final spec): a struct type + struct.new/struct.get, instantiated and CALLED.
    // Kotlin/Wasm and Dart/Flutter-web compile to exactly this class of module. The bytes were
    // validated against real Chromium (wasmgc:42) before this probe landed, so `no` here is a
    // measured engine absence, never a malformed fixture.
    probe('wasmgc', function () {
      if (typeof WebAssembly !== 'object' || !WebAssembly.Module) { return false; }
      var b = new Uint8Array([
        0,97,115,109, 1,0,0,0,
        1,9,2, 0x5f,1,0x7f,0, 0x60,0,1,0x7f,
        3,2,1,1,
        7,6,1,2,109,107,0,0,
        10,13,1, 11,0, 0x41,42, 0xfb,0,0, 0xfb,2,0,0, 0x0b
      ]);
      var inst = new WebAssembly.Instance(new WebAssembly.Module(b), {});
      return inst.exports.mk() === 42;
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

    // ── Error.stackTraceLimit (audit #13 watch): the prelude defines the PROPERTY (typeof is
    // 'number'), but the capability is TRUNCATION — set the limit to 3 and a deep recursion's
    // .stack must carry at most 3 frames. Measured, not assumed: defining the property without
    // wiring it is exactly the inert-stub shape the t195 typeof-lies lesson warns about.
    probe('stacklimit', function () {
      try {
        var old = Error.stackTraceLimit;
        Error.stackTraceLimit = 3;
        var deep = function (n) { if (n <= 0) { throw new Error('x'); } deep(n - 1); };
        var frames = -1;
        try { deep(20); } catch (e) {
          frames = String(e.stack || '').split('\n').filter(function (l) { return l.length > 0; }).length;
        }
        Error.stackTraceLimit = old;
        return frames >= 1 && frames <= 4;
      } catch (_) { return false; }
    });

    // ── field-sizing (Baseline June 2026): `content` stands the UA cols-width hint down, so the
    // control hugs its content. Both halves measured: the REFERENCE textarea keeps its ~333px
    // cols width (a broken textarea would fail here, not pass), the field-sized one shrinks.
    probe('fieldsizing', function () {
      var ref = $('fsref').getBoundingClientRect().width;
      var fs = $('fs').getBoundingClientRect().width;
      return ref > 300 && fs > 0 && fs < 150;
    });

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

    // ── Second batch (tick 230). Same rule as above: behaviour, never presence.

    // CSS features, each measured by the geometry or computed value it is supposed to produce.
    probe('subgrid', function () { return cs('sg').gridTemplateColumns.indexOf('subgrid') >= 0; });
    probe('scopedstyles', function () { return cs('scoped').color.indexOf('10, 20, 30') >= 0; });
    probe('anchorpos', function () { return cs('anchored').position === 'absolute' &&
                                            cs('anchored').top !== '' &&
                                            cs('anchored').positionAnchor !== undefined &&
                                            String(cs('anchored').positionAnchor || '').length > 0; });
    probe('attrfn', function () { return cs('attrbox').width === '120px'; });
    probe('scrolldriven', function () { return typeof ScrollTimeline === 'function' ||
                                               String(cs('sda').animationTimeline || '').indexOf('scroll') >= 0; });

    // JSPI — the WebAssembly promise-integration constructors, on top of the working wasm core.
    probe('jspi', function () {
      return typeof WebAssembly === 'object' && typeof WebAssembly.Suspending === 'function' &&
             typeof WebAssembly.promising === 'function';
    });

    // media pseudo-classes: `:muted` must SELECT the muted video and NOT the unmuted one.
    //
    // The first draft asked only that `querySelectorAll('video:muted').length >= 0`, which is true of
    // every engine that does not throw — including one that ignores the pseudo-class entirely and
    // returns nothing. A probe whose claim cannot fail measures nothing; this one fails on both an
    // engine that selects neither video and one that selects both.
    probe('mediapseudo', function () {
      try {
        var m = document.querySelectorAll('video:muted');
        return m.length === 1 && m[0].id === 'vidmuted';
      } catch (e) { return false; }
    });

    // NOT probed here: CSP enforcement. The natural test — "an inline script must be blocked by
    // `script-src 'self'`" — cannot be run FROM an inline script, because a working implementation
    // would prevent this very probe from executing. It needs an external-script harness and a real
    // response header, so it stays `unknown` rather than being given a verdict this file cannot earn.

    // ── Intl (locale-aware number/date formatting). The verified build is `mozjs/intl`, so
    // SpiderMonkey carries ICU. The de-DE assertion is the RED-prover: only real ICU locale data
    // yields `1.234,5` (dot thousands, comma decimal) — a stub, or a `noicu` build, cannot fake it.
    // This underpins every app that shows a localized price, date or number (i18n is table stakes).
    probe('intl', function () {
      if (typeof Intl !== 'object') { return false; }
      var us = new Intl.NumberFormat('en-US').format(1234.5);
      var de = new Intl.NumberFormat('de-DE').format(1234.5);
      var mon = new Intl.DateTimeFormat('en-US', { month: 'long', timeZone: 'UTC' })
                  .format(new Date(Date.UTC(2020, 0, 15)));
      return us === '1,234.5' && de === '1.234,5' && mon === 'January';
    });

    // ── Element.checkVisibility(): the "is it actually rendered?" test every UI library reinvents
    // (scroll-into-view guards, lazy-mount checks, a11y "is it on screen"). Behaviorally RED-proven:
    // a shown element is true, a display:none element is false, and `{visibilityProperty:true}` folds
    // in visibility:hidden — a stub that always-returns cannot pass all four assertions.
    probe('checkvisibility', function () {
      var on = $('cvon'), off = $('cvoff'), vis = $('cvvis');
      if (!on || typeof on.checkVisibility !== 'function') { return false; }
      return on.checkVisibility() === true &&
             off.checkVisibility() === false &&
             vis.checkVisibility() === true &&
             vis.checkVisibility({ visibilityProperty: true }) === false;
    });

    // ── Temporal (Chrome 144 / Baseline 2026): the modern date/time API replacing the `Date`
    // footguns. SpiderMonkey ships it in the verified build. RED-proven by CALENDAR ARITHMETIC a stub
    // cannot fake: 2020-01-15 + 40 days = 2020-02-24 (Jan has 31 days), that date is a Wednesday
    // (ISO dayOfWeek 3), a 25-hour Duration totals 25 hours, and PlainTime 10:30 + 45 min = 11:15.
    probe('temporal', function () {
      if (typeof Temporal === 'undefined') { return false; }
      var d = Temporal.PlainDate.from('2020-01-15');
      return d.add({ days: 40 }).toString() === '2020-02-24' &&
             d.dayOfWeek === 3 &&
             Temporal.Duration.from({ hours: 25 }).total({ unit: 'hours' }) === 25 &&
             Temporal.PlainTime.from('10:30').add({ minutes: 45 }).toString() === '11:15:00';
    });

    // ── Drag and drop: the event surface pages actually bind to.
    probe('dragdrop', function () {
      return typeof DataTransfer === 'function' && 'draggable' in document.createElement('div') &&
             'ondragstart' in document.createElement('div');
    });

    // ── content-visibility (Baseline 2025). Measured at the PARSE/@supports level, which is the
    // level a page feature-detects at: the property is compiled into this Stylo build, so
    // `@supports (content-visibility: auto)` and `CSS.supports()` answer YES honestly, and a
    // feature-detect that branches on it branches correctly. The `auto` case is visually SAFE here
    // even without the optimization — we render the content instead of skipping the off-screen part,
    // which is strictly MORE work, never wrong output. The off-screen render-SKIP optimization and
    // the `hidden` subtree collapse are layout work and are NOT claimed by this probe.
    //
    // RED-proven by a value-validation discriminator: a rubber-stamp that says yes to everything also
    // says yes to `content-visibility: floops-not-a-value`, which MUST be no. So this fails both on an
    // engine that dropped the property from the build (all three keywords go no) and on one that
    // stubbed CSS.supports to always-true (the nonsense value goes yes).
    probe('contentvis', function () {
      return CSS.supports('content-visibility', 'auto') &&
             CSS.supports('content-visibility', 'hidden') &&
             CSS.supports('content-visibility', 'visible') &&
             !CSS.supports('content-visibility', 'floops-not-a-value');
    });

    // ── contain-intrinsic-size: the size-RESERVATION half of content-visibility (reserves an
    // intrinsic height for skipped subtrees so the scrollbar does not jump). Measured absent — the
    // property is `engine="gecko"` in this servo Stylo build (zero occurrences in the generated
    // properties), so `@supports` honestly says no. Recorded, not asserted true.
    probe('containintrinsic', function () {
      return CSS.supports('contain-intrinsic-size', '200px');
    });

    // ── view-transition pseudo-classes (:active-view-transition / -type()): styling DURING a View
    // Transition by transition type (Baseline 2026). Measured via the `selector()` @supports form,
    // which asks whether this build's selector parser even knows the pseudo — no async transition to
    // drive. A gecko-only NonTSPseudoClass would parse-fail here, same fence as `:has()`/`:playing`.
    probe('vtpseudo', function () {
      return CSS.supports('selector(:active-view-transition)');
    });

    // ── navigator.cpuPerformance (Chrome 152 CPU Performance API): scheduler-aware sites may branch
    // on it. A plain data accessor, nothing to drive, so a presence check is the whole test. Chrome-
    // only and not Baseline; measured here to pin the cell rather than carry it as an unmeasured guess.
    probe('cpuperf', function () {
      return typeof navigator !== 'undefined' && typeof navigator.cpuPerformance !== 'undefined';
    });

    // ── CSS `lh` line-height unit (Baseline Widely Available May 2026), sibling of ch/ex/cap.
    // BEHAVIOURAL, not parse-only: #lhbox has `line-height:20px; width:5lh`, so a correctly-wired `lh`
    // resolves the block width to 5×20 = 100px. A fallback (unit parsed but treated as 1em=16px, the
    // ch-unit-0.5em bug's shape) yields 80px; an unparsed unit drops the declaration and the block
    // fills its 800px container. Only exactly-100 passes, so this fails on all three wrong outcomes.
    probe('lhunit', function () {
      var w = $('lhbox').getBoundingClientRect().width;
      return Math.abs(w - 100) < 2;
    });

    // ── :user-invalid / :user-valid (Baseline May 2026): the interaction-state form pseudos (turn red
    // only AFTER the user touches+leaves a field, unlike :invalid which fires on load). Measured at the
    // selector-parse level via CSS.supports(selector()) — matching needs real user interaction this
    // probe cannot drive. A gecko-only NonTSPseudoClass parse-fails here (same fence as :has()/:playing).
    probe('userinvalid', function () {
      return CSS.supports('selector(:user-invalid)') && CSS.supports('selector(:user-valid)');
    });

    // ── ToggleEvent.source (Baseline Newly Available May 2026): the invoking element on the
    // toggle/beforetoggle event (popover/dialog command-invoker wiring). The events already fire here;
    // this is the new property. Constructor + own-shape check, wrapped (ToggleEvent may be absent).
    probe('togglesource', function () {
      if (typeof ToggleEvent !== 'function') { return false; }
      try { return 'source' in new ToggleEvent('toggle'); }
      catch (e) { return 'source' in ToggleEvent.prototype; }
    });

    // ── image-rendering (Baseline Newly Available May 2026): the scaling-filter keyword
    // (pixelated/crisp-edges) that keeps pixel-art/QR/retro images crisp instead of bilinear-blurred.
    // Parse-level via CSS.supports; a bogus keyword discriminates a rubber-stamp.
    probe('imagerendering', function () {
      return CSS.supports('image-rendering', 'pixelated') &&
             CSS.supports('image-rendering', 'crisp-edges') &&
             !CSS.supports('image-rendering', 'floops-not-a-value');
    });

    // ── light-dark() CSS color function (Baseline 2024, surface audit #27): color: light-dark(A,B)
    // picks A or B by the used color-scheme — the modern way to drop the @media (prefers-color-scheme)
    // duplication. The color-scheme PROPERTY landed via a Stylo pref-flip (t464); this measures the
    // consuming FUNCTION. Parse-level via CSS.supports (resolution needs a cascaded color-scheme the
    // static probe does not drive); a bogus inner color discriminates a rubber-stamp.
    probe('lightdark', function () {
      return CSS.supports('color', 'light-dark(white, black)') &&
             !CSS.supports('color', 'light-dark(floops-not-a-color, black)');
    });

    // ── CSS Level-4 stepped/sign math round()/mod()/abs()/sign() (Baseline 2024, surface audit #27).
    // calc()/min()/max()/clamp() are older and WPT-covered; these are the newer functions modern
    // fluid-type/layout math uses. Parse-level via CSS.supports in a <length> context (sign() returns a
    // number, so it is tested inside calc()); a bogus function discriminates a rubber-stamp.
    probe('cssmath4', function () {
      return CSS.supports('width', 'round(down, 23px, 10px)') &&
             CSS.supports('width', 'mod(18px, 5px)') &&
             CSS.supports('width', 'abs(-5px)') &&
             CSS.supports('width', 'calc(sign(-3) * 10px)') &&
             !CSS.supports('width', 'floopz(1px)');
    });
  </script>

  <!-- ES-module capability, measured from INSIDE a real `<script type=module>`. Modules are deferred,
       so this runs AFTER the classic probe above and globalThis.R already exists to push into. This is
       the surface that "killed every Vite app on the internet" until import.meta.url resolved: a module
       must COMPILE + LINK + EVALUATE and expose a string import.meta.url. Self-contained modules (no
       cross-module imports) are the measured-working half pinned here. The multi-module import GRAPH is
       the recorded GAP — module_resolve_hook (dom_bindings.rs) returns a null pointer, so an
       `import {x} from './y.js'` fails at ModuleLink; a pre-fetched synchronous module registry keyed by
       resolved specifier is the follow-on subsystem, not this tick. RED-provable: break run_module or
       the metadata hook and esmmodule goes no. -->
  <script type="module">
    try {
      var ok = (typeof import.meta === 'object' && import.meta !== null &&
                typeof import.meta.url === 'string' && import.meta.url.length > 0);
      if (globalThis.R) { globalThis.R.push('esmmodule:' + (ok ? 'yes' : 'no')); }
    } catch (e) { try { if (globalThis.R) { globalThis.R.push('esmmodule:no'); } } catch (_e) {} }
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
    // Added by the tick that built it — this file's own rule: what the probe measured absent moves
    // into this list the day it starts working, and from then on its disappearance is a regression.
    "scrollsnap:yes",
    // tick 308 — document.startViewTransition now runs the update callback and settles its promises
    // (the skip-animation path), so SPA route/state changes wrapped in a transition no longer vanish.
    "viewtransitions:yes",
    // tick 309 — window.navigation (Navigation API): navigate() fires an interceptable navigate event
    // and intercept({handler}) runs client-side routing over the shared History/Location plumbing.
    "navigationapi:yes",
    // tick 344 — `:muted` media pseudo-class in the querySelector engine: `video:muted` selects the
    // muted <video> and not the loud one. querySelector-only (the servo Stylo build has no Muted
    // NonTSPseudoClass variant, so it cannot cascade — deferred with the same fence as `:has()`).
    "mediapseudo:yes",
    // tick 359 — WasmGC (final spec): struct type instantiates, struct.new/struct.get return the
    // right value through the whole pipeline. The Kotlin/Wasm + Dart/Flutter-web module class.
    // SpiderMonkey ships it on; the probe measured it rather than assuming it.
    "wasmgc:yes",
    // tick 379 — container queries: the sized cascade re-pass (layout feeds content-box sizes back
    // through TElement::query_container_size) + the @container source supplement (stylo's servo
    // build cfg-drops the at-rule) make the #cq rule apply only because #cq-outer's resolved
    // inline size (400px) crosses the (min-width: 300px) condition.
    "containerq:yes",
    // tick 400 — Error.stackTraceLimit: HONEST NO. The prelude defines the property (typeof
    // 'number' — audit #13's watch), but truncation is NOT wired: our SpiderMonkey predates the
    // Firefox-153 implementation of this V8-ism, so a limit of 3 does not cap .stack frames.
    // Measured, pinned as no per the typeof-lies rule; flips to yes WITH a mozjs bump that
    // carries it, never by retuning the probe.
    "stacklimit:no",
    // tick 388 — field-sizing: content (Baseline June 2026): recovered from MinimalCascade BEFORE
    // the presentational-hints pass so it can veto the UA cols-width hint; the probe measures the
    // reference textarea keeping its cols width AND the field-sized one hugging content.
    "fieldsizing:yes",
    // tick 418 — Intl (locale-aware number/date formatting): SpiderMonkey carries full ICU in the
    // verified `mozjs/intl` build, so `Intl.NumberFormat`/`DateTimeFormat` are real, not stubs. The
    // probe's de-DE assertion (`1.234,5`) is the RED-prover — only genuine ICU locale data produces
    // it — so this pin also guards against a silent regression to a `noicu` build. i18n is table
    // stakes: every app showing a localized price/date/number depends on it. Was carried UNKNOWN
    // (unprobed, unlisted); measured working, not assumed.
    "intl:yes",
    // tick 419 — Element.checkVisibility(): the render-visibility check UI libraries reinvent. Real,
    // not a stub — it walks the ancestor chain for display:none, rejects disconnected nodes, and folds
    // in visibility:hidden under {visibilityProperty:true}. The probe RED-proves all four behaviors, so
    // a stub (always-true or always-false) cannot pass. Was carried UNKNOWN (unprobed, unlisted).
    "checkvisibility:yes",
    // tick 428 (surface audit #16) — Temporal (Chrome 144 / Baseline 2026): the modern date/time API.
    // SpiderMonkey ships it in the verified build; the probe RED-proves it with calendar arithmetic a
    // stub cannot fabricate (2020-01-15 + 40d = 2020-02-24, ISO dayOfWeek 3, a 25h Duration totals 25h,
    // PlainTime 10:30 + 45m = 11:15). Was carried nowhere on the map; measured working, not assumed.
    "temporal:yes",
    // tick 509 — CSS `lh` line-height unit (Baseline Widely Available May 2026): ALREADY RESOLVES,
    // exactly (the stale-pessimistic rule pays again — surface audit #24 added it UNKNOWN a tick
    // earlier). `width:5lh` with `line-height:20px` measures 100px — the unit both PARSES in this
    // Stylo build and resolves against the element's used line-height, needing zero wiring (unlike
    // ch, which needed the font-metrics seam). Behavioural RED-prover: a fallback would be 80px, an
    // unparsed unit ~800px; only exactly-100 passes. `rlh` is the root-relative sibling on the same
    // Stylo line-height-relative length path (differs only by self-vs-root line-height).
    "lhunit:yes",
    // tick 510 — :user-invalid / :user-valid PARSE (selector-recognized; NOT gecko-gated, unlike the
    // :playing fence). `CSS.supports('selector(:user-invalid)')` → yes, so a form's feature-detect
    // branches correctly. Interaction-state MATCHING (turn red only after touch+blur) needs real user
    // input this static probe cannot drive — so the cell is `partial`; this pins only the parse half.
    "userinvalid:yes",
    // tick 510 — image-rendering parses + VALIDATES its values (pixelated/crisp-edges accepted, a bogus
    // keyword rejected — not a rubber-stamp). @supports answers honestly for pixel-art/QR feature-
    // detects. Whether the PAINTER honors pixelated (nearest-neighbor vs bilinear) is untested → cell
    // `partial`; this pins the parse/@supports half.
    "imagerendering:yes",
    // tick 506 — ES modules run (the self-contained half). Measured from INSIDE a real
    // `<script type=module>`: it COMPILES + LINKS + EVALUATEs and `import.meta.url` is a non-empty
    // string — the exact hook whose absence silently killed every Vite/Rollup/esbuild bundle (they
    // all emit `import.meta.url` unconditionally). RED-provable: break `run_module` or the metadata
    // hook and this goes no. The multi-module import GRAPH is a SEPARATE, still-absent capability
    // (module_resolve_hook returns null → `import from './y.js'` fails at ModuleLink) — recorded as
    // the gap on CONSTELLATION cell 169, a pre-fetched sync module registry is its follow-on subsystem.
    "esmmodule:yes",
    // tick 539 (surface audit #27) — light-dark() CSS color function (Baseline 2024) PARSES + VALIDATES
    // in this Stylo build (a bogus inner color is rejected — not a rubber-stamp). The stale-pessimistic
    // rule pays yet again: added `unknown` in audit #27 expecting it might be gecko-gated, measured
    // working a tick later. This pins the PARSE half; light-dark()'s color-scheme-dependent RESOLUTION
    // in getComputedStyle is untested by the static probe → cell `partial`. Flips to a behavioural pin
    // the day a probe drives color-scheme and reads the resolved color; never by retuning this probe.
    "lightdark:yes",
    // tick 539 (surface audit #27) — CSS Level-4 stepped/sign math round()/mod()/abs()/sign() (Baseline
    // 2024) PARSE + VALIDATE (a bogus function is rejected). calc/min/max/clamp were already WPT-covered;
    // these newer functions are what modern fluid-type math uses. Parse half only (whether round(down,
    // 23px, 10px) actually computes 20px is untested here) → cell `partial`.
    "cssmath4:yes",
];
