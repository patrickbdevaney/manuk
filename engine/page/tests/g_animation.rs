//! **G_ANIMATION — an animated element renders its END state, not its first frame.**
//!
//! We cannot animate. The question is what a *static* renderer should show, and the answer is **not**
//! "the base rule, literally" — because the single most common animation on the web is a **fade-in**
//! whose base rule is `opacity: 0` and whose keyframes reveal the element. Render that literally and the
//! content **never appears at all**.
//!
//! Measured, on the oracle corpus: **52 of 237 sites (21%) pair `opacity: 0` with an animation.** That is
//! a fifth of the web with invisible content, and it is why this is a *correctness* fix rather than a
//! polish one. `prefers-reduced-motion: reduce` is the same idea, blessed by the spec: **show the
//! destination, skip the journey.**
//!
//! The second assertion is the one that keeps this honest. It would be trivial — and catastrophic — to
//! "fix" this by forcing every `opacity: 0` element to be visible. An author who hides something with no
//! animation **meant it**: a closed dropdown, an off-screen menu, a screen-reader-only label, a cookie
//! banner that has not fired. Revealing those is not a fix, it is a different and louder bug.
//!
//! So the rule is narrow on purpose: **`opacity: 0` + an animation → show it.** `opacity: 0` alone stays
//! hidden. And it is scoped to *opacity*, because opacity is the only one of these that makes content
//! disappear — a `transform` slide-in still renders (merely offset), and a colour transition still
//! renders a colour.

use manuk_text::FontContext;

#[test]
fn animated_content_is_visible_and_deliberately_hidden_content_is_not() {
    let fonts = FontContext::new();
    let html = r#"<html><body style="margin:0">
      <style>
        @keyframes fadeIn { from { opacity: 0 } to { opacity: 1 } }
        .reveal { opacity: 0; animation: fadeIn 1s forwards; background: #00aa00; height: 60px }
        .hidden { opacity: 0; background: #aa0000; height: 60px }
      </style>
      <div class="reveal">revealed by an animation</div>
      <div class="hidden">deliberately hidden — and must STAY hidden</div>
      </body></html>"#;

    let page = manuk_page::Page::load(html, "https://anim.test/", &fonts, 800.0);
    let canvas = page.paint(&fonts, 800, 200);
    let px = canvas.rgba_bytes();
    let at = |y: usize| {
        let i = (y * 800 + 400) * 4;
        (px[i], px[i + 1], px[i + 2])
    };

    // (1) The fade-in element is VISIBLE. Its base rule says `opacity: 0`; its animation reveals it.
    let (r, g, b) = at(30);
    assert!(
        g > 100 && r < 100,
        "G_ANIMATION: an element with `opacity:0` + an animation painted rgb({r},{g},{b}) — it is \
         INVISIBLE. Its keyframes reveal it, so a static renderer must show the end state.\n  \
         21% of the corpus (52 of 237 sites) has this exact pattern. Rendering the first frame \
         literally means a fifth of the web has content nobody can see."
    );

    // (2) **And an element the author deliberately hid STAYS hidden.** This is the assertion that stops
    //     the fix from becoming a worse bug: a closed dropdown, an off-screen menu, a cookie banner that
    //     has not fired. `opacity: 0` with NO animation means what it says.
    let (r2, g2, b2) = at(90);
    assert!(
        r2 > 200 && g2 > 200 && b2 > 200,
        "G_ANIMATION: an element with `opacity:0` and NO animation painted rgb({r2},{g2},{b2}) — we \
         REVEALED something the author hid.\n  \
         Forcing every transparent element visible is not a fix, it is a louder bug: closed dropdowns, \
         off-screen menus, and un-fired cookie banners would all appear on top of the page."
    );
}
