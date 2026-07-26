//! **G_INNER_TEXT_SET — a getter-only `innerText` is not a missing feature, it is a THROW, and one
//! real site turns that throw into a WHITE SCREEN.**
//!
//! `innerText` was registered with no setter. Assigning to an accessor that has only a getter raises
//! `TypeError: setting getter-only property "innerText"` under strict mode — so the assignment does not
//! quietly no-op, it **kills the frame that made it** and everything that frame was going to do.
//!
//! Found on `www.welt.de` (top-1000, in the certification corpus) by the t611 sweep, and the failure
//! there is worse than a lost assignment. Its own anti-adblock check writes through `innerText`,
//! catches the resulting throw, concludes an ad blocker is present, and **deliberately blanks the
//! page**:
//!
//! ```text
//!   ERROR page.console: Failed to load website due to adblock:
//!                       TypeError: setting getter-only property "innerText"
//!   structural: 0.0% (3182 paths, 3181 missing)
//!   MISSING by tag: div×940  a×447  li×390  span×346  h4×103  ul×100  article×98  svg×89
//! ```
//!
//! One absent setter, and Chromium renders the front page of a national newspaper from bytes that
//! render for us as a blank white document. **The sibling property `outerText` had a setter the whole
//! time** — one rule, two implementations, only one of them written.
//!
//! This gate asserts the SHAPE of that failure and not merely the feature: an assignment inside a
//! `try` must leave the `catch` branch untaken, because *that* is the branch welt.de blanks itself in.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <div id="host"><span>old</span><b>content</b></div>
  <div id="lines"></div>
  <div id="probe"><i>x</i></div>
  <div id="out">-</div>
  <script>
    var R = [];

    // ── THE WELT.DE SHAPE, AND IT ONLY REPRODUCES IN STRICT MODE.
    //
    // The site never reads the value back — it only cares whether the assignment RAISED. But a
    // getter-only assignment throws only under strict mode; in sloppy mode it silently no-ops. The
    // first draft of this gate wrote the assignment at top level, where the script is sloppy, and
    // `threw:false` therefore passed WITH THE BUG STILL PRESENT — the headline claim of the whole
    // gate, vacuous, while every other assertion around it went correctly red.
    //
    // That is the exact trap t610 booked: a gate can be green because a SECOND mechanism produces the
    // same observable. Real bundles are strict (a module body always is, and every minifier emits
    // `'use strict'`), so the `'use strict'` below is not decoration — it is the only thing that makes
    // this assertion able to fail.
    var adblockDetected = false;
    (function () {
      'use strict';
      try {
        document.getElementById('probe').innerText = 'bait';
      } catch (e) {
        adblockDetected = true;
      }
    })();
    R.push('threw:' + adblockDetected);

    // ── It is a real ACCESSOR with a setter, found by walking the element's actual prototype chain
    // rather than by assuming which interface object holds it. Asserting `Element.prototype`
    // specifically is a guess about this engine's binding layout, and it failed here while every
    // behavioural claim passed — a gate must test the property, not my belief about where it lives.
    // What matters is that the setter is on a PROTOTYPE and not an own data property per element:
    // a per-instance value would not be patchable, would not be shared, and would not survive the
    // getter (see G_PROTOTYPE, and the localStorage.setItem discard of t587).
    var probeEl = document.getElementById('probe');
    var d = null, ownData = Object.getOwnPropertyDescriptor(probeEl, 'innerText');
    for (var o = Object.getPrototypeOf(probeEl); o; o = Object.getPrototypeOf(o)) {
      var c = Object.getOwnPropertyDescriptor(o, 'innerText');
      if (c) { d = c; break; }
    }
    R.push('onProto:' + (!!d));
    R.push('ownData:' + (!!(ownData && 'value' in ownData)));
    R.push('hasSetter:' + (!!(d && typeof d.set === 'function')));

    // ── It REPLACES the children, and the text reads back.
    var host = document.getElementById('host');
    host.innerText = 'replaced';
    R.push('kids:' + host.childNodes.length);          // exactly one text node
    R.push('text:' + host.textContent);
    R.push('read:' + host.innerText);

    // ── Newlines become <br>, per the spec's rendered-text-fragment steps. `\r\n` and `\r` normalise
    // to `\n` first, so all three line endings produce the SAME tree — a page that pastes CRLF text
    // must not get a different DOM from one that pastes LF.
    var L = document.getElementById('lines');
    L.innerText = 'a\nb';
    R.push('brCount:' + L.getElementsByTagName('br').length);
    R.push('brKids:' + L.childNodes.length);           // text, br, text
    L.innerText = 'a\r\nb';
    R.push('crlfBr:' + L.getElementsByTagName('br').length);
    L.innerText = 'a\rb';
    R.push('crBr:' + L.getElementsByTagName('br').length);

    // ── Plain text must NOT be parsed as markup. innerText is the SAFE sibling of innerHTML, and a
    // page that assigns user input through it is relying on exactly that.
    L.innerText = '<img src=x onerror=1>';
    R.push('noParse:' + L.getElementsByTagName('img').length);
    R.push('escaped:' + (L.textContent === '<img src=x onerror=1>'));

    // ── Setting it EMPTY clears the element (the common "reset this label" idiom).
    host.innerText = '';
    R.push('cleared:' + (host.textContent === ''));

    // ── And the sibling that already worked must keep working: outerText replaces the ELEMENT, not
    // its children. Both setters now share one line-splitting helper, so this is the assertion that
    // notices if the shared helper is changed to suit only one of them.
    var o = document.createElement('div');
    o.id = 'o';
    document.body.appendChild(o);
    var wrap = document.createElement('p');
    wrap.appendChild(o);
    document.body.appendChild(wrap);
    o.outerText = 'gone';
    R.push('outerGone:' + (document.getElementById('o') === null));
    R.push('outerText:' + wrap.textContent);

    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn inner_text_is_writable_and_does_not_throw() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://it.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // THE headline: the assignment must not raise. welt.de blanks itself on `threw:true`.
        "threw:false",
        // a real accessor, with a real setter
        "onProto:true",
        "ownData:false",
        "hasSetter:true",
        // replace-all semantics
        "kids:1",
        "text:replaced",
        "read:replaced",
        // rendered text fragment: newlines are <br>, and all three line endings agree
        "brCount:1",
        "brKids:3",
        "crlfBr:1",
        "crBr:1",
        // it is TEXT, not markup — the security property that makes innerText the safe sibling
        "noParse:0",
        "escaped:true",
        // empty clears
        "cleared:true",
        // the sibling setter still replaces the ELEMENT
        "outerGone:true",
        "outerText:gone",
    ] {
        assert!(
            got.contains(claim),
            "G_INNER_TEXT_SET: expected `{claim}`\n  got: {got}\n\n  \
             `el.innerText = x` on a getter-only accessor is a TypeError, not a no-op — it kills the \
             assigning frame. `threw:true` is the exact state in which www.welt.de decides an ad \
             blocker is present and blanks its own document: we rendered 1 of its 3,182 elements, a \
             white screen on a top-1000 site, from this one missing setter. `noParse:0` is a security \
             property, not a nicety: innerText is where pages put UNTRUSTED text precisely because it \
             does not parse markup, so if that ever reports an <img>, this became innerHTML."
        );
    }
}
