//! **G_EXPANDO_READONLY — six readonly IDL attributes sat on `Node.prototype`, so `this.index = 0`
//! in a custom-element constructor threw and the element never existed.**
//!
//! `index`, `options`, `selectedOptions`, `mode`, `origin` and `wholeText` are each readonly on
//! exactly ONE interface — `HTMLOptionElement`, `HTMLSelectElement`, `ShadowRoot`,
//! `HTMLAnchorElement`, `Text` — and every one of them was installed **getter-only on
//! `Node.prototype`**, which every element in the document inherits.
//!
//! On a `<my-widget>` none of those names is in the prototype chain at all in a real browser, so
//! `this.index = 0` is an ordinary expando that simply creates an own property. Here it found an
//! inherited accessor with no setter — and a `class` body is **always strict** — so it threw
//! `TypeError: setting getter-only property "index"` out of the constructor, *before the element
//! existed*. The t777 sweep logged that exact message **18 times on `meet.google.com`** (which scored shape 0.126),
//! 17 of them tagged `custom element ctor` or `attributeChangedCallback`.
//!
//! Two of this project's recurring shapes at once: a **wrong answer of the right type** (the name is
//! present, correctly-shaped, and wrong about who owns it), and **one rule, N implementations** —
//! six accessors, one mis-tiering. No probe of NAMES could see any of it, because every one of these
//! names is *supposed* to exist.
//!
//! ## The claim that keeps this from being a hole punched in `readonly`
//!
//! It would be easy to "fix" this by making all six writable, which trades a throw for a lie. The
//! gate asserts **both halves**: the expando lands on an element that does not own the attribute,
//! **and** the write is still ignored on the one that does — `option.index` still reports its
//! position, `a.origin` still reports the URL's origin, `shadowRoot.mode` is still `open`.
//!
//! ⚠ **The accepted divergence, asserted rather than discovered later.** A native accessor cannot
//! see whether its caller is strict, so "readonly" here means *the write is ignored*, not *the write
//! throws in strict mode*. Chrome ignores it sloppy and throws strict; we ignore it in both. That
//! costs code writing to a genuinely readonly attribute — already a bug in that code — and buys back
//! every element that is not an `<option>`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<select id="sel"><option id="o0">a</option><option id="o1">b</option></select>
<a id="lnk" href="https://example.test:8443/p?q=1">l</a>
<div id="host"></div>
<span id="txt">hello</span>
<my-widget id="w" data-k="v"></my-widget>
<script>
  'use strict';
  var R = [];
  function t(n, f) { try { R.push(n + ':' + f()); } catch (e) { R.push(n + ':THROW(' + e + ')'); } }

  var NAMES = ['index', 'options', 'selectedOptions', 'mode', 'origin', 'wholeText'];

  // ── 1. **THE UNGUARDED WRITE, exactly as a component writes it.** Strict mode, because a class
  // body is always strict — which is why this landed as a constructor abort rather than a silent
  // no-op. Every one of these names belongs to some OTHER interface, so on a plain <div> a real
  // browser just makes an own property.
  t('plain', function () {
    var d = document.createElement('div');
    var threw = NAMES.filter(function (n) {
      try { d[n] = 'x-' + n; return false; } catch (e) { return true; }
    });
    if (threw.length) return 'THREW(' + threw.join(',') + ')';
    var lost = NAMES.filter(function (n) { return d[n] !== 'x-' + n; });
    return lost.length ? 'LOST(' + lost.join(',') + ')' : 'ok';
  });

  // ── 2. **THE CUSTOM ELEMENT.** The live symptom: the constructor assigns, the assignment throws,
  // and the element is never upgraded — so the failure presents as "my component renders nothing",
  // with the real cause six frames up in a setter that does not exist.
  var ctorErr = 'never-ran';
  class Widget extends HTMLElement {
    constructor() {
      super();
      this.index = 7;
      this.options = { a: 1 };
      this.selectedOptions = [];
      this.mode = 'dark';
      this.origin = 'local';
      this.wholeText = 'w';
      ctorErr = 'ok';
    }
  }
  try { customElements.define('my-widget', Widget); } catch (e) { ctorErr = 'DEFINE-THREW(' + e + ')'; }
  t('ctor', function () { return ctorErr; });
  // Read off the element the constructor demonstrably RAN on — the one `customElements.define`
  // upgraded out of the markup above. A constructor that survives but drops its own state is the
  // same bug one layer quieter, so the values have to come back.
  t('upgraded', function () {
    var w = document.getElementById('w');
    return (w.index === 7) + ',' + (w.mode === 'dark') + ',' + (w.origin === 'local');
  });
  // ⚠ MEASURED, NOT ASSERTED, AND NOT THIS TICK'S BUG: `document.createElement('my-widget')` does
  // NOT run the constructor here, so this reads `false,false,false` for a reason that has nothing to
  // do with the six accessors. Reported so the number is a fact the successor inherits rather than a
  // blind spot — folding it into the claim above would make this gate fail for two unrelated causes.
  t('viaCreateElement', function () {
    var w = document.createElement('my-widget');
    return (w.index === 7) + ',' + (w.mode === 'dark') + ',' + (w.origin === 'local');
  });

  // ── 3. **READONLY STILL HOLDS WHERE IT IS REAL.** The other half of the fix, and the half a
  // careless version would have thrown away: on the interface that genuinely owns the attribute the
  // write is ignored and the platform's own value survives.
  t('optionIndex', function () {
    var o = document.getElementById('o1');
    var before = o.index;
    o.index = 99;
    return before + '/' + o.index;
  });
  t('selectOptions', function () {
    var s = document.getElementById('sel');
    s.options = 'clobbered';
    return (typeof s.options === 'string') ? 'CLOBBERED' : String(s.options.length);
  });
  t('anchorOrigin', function () {
    var a = document.getElementById('lnk');
    a.origin = 'clobbered';
    return a.origin;
  });
  t('wholeText', function () {
    var n = document.getElementById('txt').firstChild;
    n.wholeText = 'clobbered';
    return n.wholeText;
  });
  t('shadowMode', function () {
    var r = document.getElementById('host').attachShadow({ mode: 'open' });
    r.mode = 'clobbered';
    return r.mode;
  });

  document.getElementById('out').textContent = R.join('  ');
</script></body></html>"##;

#[test]
fn readonly_idl_attributes_do_not_abort_a_constructor() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://expando.test/", &fonts, 1200.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "plain:ok",
            "⚠ THE UNGUARDED WRITE. `index`/`options`/`selectedOptions`/`mode`/`origin`/`wholeText` \
             are readonly on ONE interface each and were installed getter-only on `Node.prototype`, \
             which every element inherits. On a <div> none of these names is in the chain in a real \
             browser, so each write is an ordinary expando — here each one was a strict-mode \
             TypeError",
        ),
        (
            "ctor:ok",
            "⚠ THE LIVE SYMPTOM. A `class` body is always strict, so the first assignment threw out \
             of the constructor and the element was never upgraded. The t777 sweep logged \
             `setting getter-only property \"index\"` 18× on meet.google.com, 17 of them tagged \
             `custom element` — every component on it failed to construct, which is what a page \
             scoring shape 0.126 looks like from the inside",
        ),
        (
            "upgraded:true,true,true",
            "the values must actually be readable back off the upgraded element — a constructor that \
             survives but drops its own state is the same bug one layer quieter",
        ),
        (
            "viaCreateElement:false,false,false",
            "⚠ MEASURED, NOT THIS TICK'S BUG, AND ASSERTED AT ITS MEASURED VALUE SO IT CANNOT CHANGE \
             SILENTLY. `document.createElement('my-widget')` does NOT run a defined custom element's \
             constructor here — the upgrade above came from the PARSER's pass over the markup. \
             Folding that into `upgraded` would make this gate fail for two unrelated causes; \
             banking the number makes it a fact the successor inherits instead of a blind spot. When \
             `createElement` starts upgrading, this claim goes red and is the reminder to promote it",
        ),
        (
            "optionIndex:1/1",
            "⚠ READONLY STILL HOLDS WHERE IT IS REAL. `index` IS a readonly IDL attribute of \
             `HTMLOptionElement`, so the write is ignored and the option still reports its position. \
             Making all six writable would have traded a throw for a lie",
        ),
        (
            "selectOptions:2",
            "`select.options` is the live options collection, not a slot to overwrite — it still \
             reports both `<option>`s after being written to",
        ),
        (
            "anchorOrigin:https://example.test:8443",
            "`a.origin` is readonly on `HTMLAnchorElement` and is derived from `href`; the write is \
             ignored and the URL's own origin survives",
        ),
        (
            "wholeText:hello",
            "`wholeText` is readonly on `Text` — it is the concatenation of the contiguous text run, \
             which is a computed answer and not storage",
        ),
        (
            "shadowMode:open",
            "`mode` is readonly on `ShadowRoot`, and it is the one of the six with a non-element \
             owner — the predicate is `shadow_root_mode(n).is_some()`, not a tag name",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_EXPANDO_READONLY: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
