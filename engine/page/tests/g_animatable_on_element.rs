//! **G_ANIMATABLE_ON_ELEMENT — `element.animate()` worked, and `'animate' in Element.prototype` was
//! `false`.**
//!
//! The Web Animations shim is installed in the prelude on
//! `Object.getPrototypeOf(document.createElement('div'))`, which is **`HTMLElement.prototype`** — one
//! link *below* `Element` on the chain `HTMLElement → Element → Node → EventTarget`. Every element
//! inherits it, so every test that *calls* the method passed, and the gate that exercises
//! `el.animate(...).finished` was green for its whole life. But `in` walks **up**, never down, so asking
//! `Element.prototype` about a property defined on its subclass answers `false`.
//!
//! ⚠⚠⚠ **A FALSE ABSENCE is the mirror of the false presence the reliability doctrine already names,
//! and it is the more expensive of the two here**, because the caller who asks is precisely the caller
//! who has a fallback path: the detect answers "no", the library disables its animations, and nothing is
//! thrown, logged or rendered differently enough to notice. WPT's own
//! `css/support/interpolation-testcommon.js` writes exactly this detect —
//! `isSupported: function() {return 'animate' in Element.prototype;}` — and it runs in every one of the
//! **194 `*-interpolation.html` files across twelve CSS areas**. In `css/css-transforms` alone, **909
//! failing subtests said nothing but "Web Animations should be supported".**
//!
//! ⚠ **What this gate deliberately does NOT assert, because it was measured and is not true here.** The
//! natural second claim — *"and SVG was broken, since `SVGElement` reaches `Element` without passing
//! through `HTMLElement`"* — is true of the spec and **false of this engine**: every node shares one
//! chain, so `<rect>` and even a Text node already inherited `animate`. Asserting a benefit the
//! measurement does not support is how a gate starts flattering the thing it guards.
//!
//! ⚠⚠ **EXISTENCE IS NOT SUFFICIENCY, and this gate is careful not to imply otherwise.** Making the
//! detect honest moved `css/css-transforms` **1566 → 1711 (+145)** on an unchanged denominator — *not*
//! +909. The other 764 stopped saying "unsupported" and started saying the truth: the shim
//! fast-forwards to the animation's END STATE and cannot report an intermediate frame, so
//! `animation.currentTime = 50s` on a 100s animation still reads the final value. That is the next
//! tick's subject, and the number above is its size.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
 <svg id="s" width="20" height="20"><rect id="r" width="10" height="10"/></svg>
 <div id="out">-</div>
 <script>
   window.addEventListener('load', function () {
     var parts = [];
     var d = document.createElement('div');

     // (1) THE DETECT — the whole point. `in`, on `Element.prototype`, exactly as the web writes it.
     parts.push('detect=' + ('animate' in Element.prototype) +
                '/' + ('getAnimations' in Element.prototype));

     // (2) …and it is OWNED there, not merely reachable. `in` would also answer true if the property
     // sat on `Node.prototype` — which is where a careless "just use Element.prototype in the prelude"
     // fix actually lands it, because the chain is not built yet at that point.
     parts.push('owner=' + !!Object.getOwnPropertyDescriptor(Element.prototype, 'animate'));

     // (3) ONE home, not two — counted along a real element's whole chain, NOT by naming
     // `HTMLElement.prototype`. Measured: the link the prelude installs on is not the object that name
     // resolves to at page time, so an assertion phrased against it reads `false` whether the fix ran or
     // not — an assertion that cannot go red, which is exactly what `falsify.sh` exists to find.
     var homes = 0;
     for (var p = Object.getPrototypeOf(d); p; p = Object.getPrototypeOf(p)) {
       if (Object.getOwnPropertyDescriptor(p, 'animate')) { homes++; }
     }
     parts.push('homes=' + homes);

     // (4) AND IT STILL WORKS — on an HTML element and on an SVG element. Moving a method up a
     // prototype chain is exactly the kind of edit that satisfies a presence check while breaking the
     // call, because the descriptor, its closure and its WeakMap all have to survive the move.
     var ok = 'no';
     try {
       var a = d.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 100, fill: 'forwards' });
       var r = document.getElementById('r').animate([{ opacity: 0 }, { opacity: 1 }], 100);
       ok = (typeof a.pause === 'function' && typeof a.finished === 'object' &&
             typeof r.pause === 'function') ? 'yes' : 'partial';
     } catch (e) { ok = 'THREW:' + e; }
     parts.push('call=' + ok);

     // (5) `getAnimations()` still tracks — the WeakMap is closed over by the moved function, so a move
     // that re-created the function instead of carrying the descriptor would silently lose every
     // running animation.
     var t = document.createElement('div');
     document.body.appendChild(t);
     t.animate([{ opacity: 0 }, { opacity: 1 }], 100);
     parts.push('tracked=' + (t.getAnimations().length >= 1));

     document.getElementById('out').textContent = parts.join(' ');
   });
 </script>
</body></html>"##;

fn out(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn the_animatable_mixin_is_owned_by_element_prototype_where_every_feature_detect_looks() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://animatable.test/",
        &fonts,
        800.0,
    ));
    let got = out(&page);
    println!("ANIMATABLE-ON-ELEMENT {got}");
    let has = |s: &str| got.contains(s);

    // (1) RED: remove the `animatable.js` eval → `detect=false/false`, while every other assertion here
    // and the whole of `g_web_animations` stay green. That combination IS the bug: the API works and
    // the only question the web asks about it is answered wrongly.
    assert!(
        has("detect=true/true"),
        "`'animate' in Element.prototype` is the detect every library writes — WPT's interpolation \
         harness gates its entire Web Animations leg on it. Got {got:?}"
    );

    // (2) RED: install on `g.Element.prototype` in the prelude instead → the property lands on the
    // shared tier that later becomes `Node.prototype`, so `in` is still false. Measured, not imagined.
    assert!(
        has("owner=true"),
        "`Animatable` must be OWNED by `Element.prototype`, not merely reachable through it — got {got:?}"
    );

    // (3) RED: drop the `delete p[n]` and the relocation becomes a duplication → `homes=2`.
    assert!(
        has("homes=1"),
        "one home, not two: a relocation that copies instead of moving leaves a second definition on \
         the subclass, and a later monkey-patch lands on whichever one the patcher found — got {got:?}"
    );

    // (4)+(5) RED: re-create the function at the new home instead of carrying the descriptor → `call`
    // still says `yes` but `tracked=false`, because the new function closes over nothing.
    assert!(
        has("call=yes") && has("tracked=true"),
        "the move must carry the descriptor verbatim: the implementation, its closed-over WeakMap of \
         running animations and `Animation` identity all have to survive it — got {got:?}"
    );
}
