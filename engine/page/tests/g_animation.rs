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
        /* PLACED MID-FLIGHT: a negative delay puts this animation at progress 0 deliberately, so
           its `opacity: 0` is the answer the author asked for and must NOT be overwritten. */
        .placed { animation: fadeIn 100s -50s steps(1, end) forwards;
                  background: #0000aa; height: 60px }
        /* PAUSED is not ABSENT: frozen at the position its delay gives it, i.e. opacity 0.5. */
        .paused { animation: fadeIn 100s -50s linear paused forwards;
                  background: #0000aa; height: 60px }
      </style>
      <div class="reveal">revealed by an animation</div>
      <div class="hidden">deliberately hidden — and must STAY hidden</div>
      <div class="placed">placed mid-flight AT opacity 0 — must stay transparent</div>
      <div class="paused">PAUSED mid-fade — must hold opacity 0.5, not vanish</div>
      </body></html>"#;

    let page = manuk_page::Page::load(html, "https://anim.test/", &fonts, 800.0);
    let canvas = page.paint(&fonts, 800, 300);
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

    // (3) **AND AN ANIMATION THE AUTHOR PLACED MID-FLIGHT KEEPS ITS OWN VALUE (t1307).**
    //     Since `crate::animation` landed, the opacity reaching paint is the value Stylo's `Animate`
    //     produced for the element's current position — so the reveal above is no longer rescuing a
    //     base rule, it is overwriting a COMPUTED one whenever that lands on exactly 0.
    //
    //     A negative `animation-delay` means the author explicitly placed the animation partway
    //     through: it is the device WPT's whole interpolation harness uses, and it is how a scrubbed
    //     animation is expressed. `opacity: 0` there is the answer that was asked for.
    //
    //     ⚠ This was found from the far end. t1306 concluded *"`steps()` is wrong in the
    //     CSS-animation path"* from `steps(1, end)` reading opacity 1 instead of 0. `steps()` is
    //     FINE — the same declaration on a LENGTH gives the correct `0px`, and `steps(1, start)` /
    //     `steps(2, end)` / `steps(4, end)` / `linear` / `ease` are all exact. **A value wrong in
    //     ONE property and right in every other names the special case, not the shared path.**
    //
    //     RED: drop the `!placed_mid_flight` term in `stylo_map.rs` -> this paints white, because the
    //     reveal fires and turns a correct 0 into 1.
    //     ⚠ The fixture's arithmetic is load-bearing and the first draft got it wrong twice, which is
    //     why it is spelled out: `-50s` of `100s` is progress 0.5, and `steps(1, end)` maps every
    //     progress below 1 to **0** — so the eased position is 0 and the correct paint is the page's
    //     WHITE, not the element's blue. A `-100s` delay would have been progress 1 (opacity 1), and
    //     asserting "blue" would then have passed with the narrowing REMOVED. Both mistakes were
    //     caught by the RED proof refusing to go red.
    let (r3, g3, b3) = at(150);
    assert!(
        r3 > 200 && g3 > 200 && b3 > 200,
        "G_ANIMATION: an animation PLACED mid-flight by a negative delay painted rgb({r3},{g3},{b3}) \
         instead of staying transparent — the reveal-hack overwrote a correctly computed \
         `opacity: 0` with 1.\n  \
         The hack exists for a fade-in that has NOT STARTED, where 0 is the journey's first frame. An \
         author who writes a negative delay has positioned the animation deliberately, so its value \
         at that point is the answer, not a page that failed to appear."
    );

    // (4) **A PAUSED ANIMATION HOLDS ITS VALUE — it is not an ABSENT one (t1308).**
    //     `samples_for` used to `continue` past any `animation-play-state: paused`, on the reasoning
    //     that we have no way to have STARTED it. That conflates *not advancing* with *not existing*:
    //     pausing freezes the timeline, and a frozen animation still has the position its
    //     `animation-delay` gives it. The document clock is 0 for a static render, so nothing advances
    //     for a RUNNING animation either — `paused` therefore changes nothing about the value.
    //
    //     ⚠ **EVERY property was wrong, which is what distinguishes this from case (3).** The skip made
    //     the element cascade as if it had no animation at all: `opacity` read the initial `1`, and the
    //     same rule on `width` read **784px** — `width: auto` filling the container — instead of 70px.
    //     *A value wrong in ONE property names a special case; wrong in ALL of them names the shared
    //     path.* Case (3) was the former, this is the latter.
    //
    //     `#0000aa` at opacity 0.5 over white is ~rgb(127, 127, 212). Skipping the animation instead
    //     leaves opacity at 1 and paints the flat ~rgb(0, 0, 170) — so the RED and GREEN readings
    //     differ in the RED channel by ~127, which no antialiasing can blur.
    //
    //     RED: restore the `continue` on `AnimationPlayState::Paused` in `animation.rs` -> r ~= 0.
    let (r4, g4, b4) = at(210);
    assert!(
        r4 > 90 && r4 < 170 && b4 > r4 + 30,
        "G_ANIMATION: a PAUSED animation painted rgb({r4},{g4},{b4}) — expected ~rgb(127,127,212), \
         i.e. `#0000aa` held at opacity 0.5.\n  \
         A flat rgb(0,0,170) means the animation was SKIPPED and the element cascaded as though it \
         had none: pause-on-hover marquees, paused-by-default spinners and CSS-driven scrubbers all \
         write this declaration, and skipping it loses every animated property, not just opacity."
    );
}
