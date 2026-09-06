//! **G_AN_AUDIO_PLAYER_RESERVES_ITS_SPACE — `<audio controls>` had the right `display` and a `0x17`
//! box, so a player reserved no space and everything below it moved up.**
//!
//! t1464 corrected the UA sheet: `audio` unqualified was `display: none`, which hid the one form of
//! the element anybody ever sees, and Chrome's rule is `audio:not([controls])`. That fixed the
//! computed value and explicitly did **not** fix the box — the residue it recorded was *"ours is
//! `0x17` against Chrome's `300x54`, because there is no audio control-bar widget with an intrinsic
//! size."*
//!
//! ⭐⭐ **THE DIAGNOSIS IN THAT RESIDUE WAS THE WRONG LEVEL.** A widget was never the missing piece —
//! `<video>` has no widget either and is `300x150`, because it is an **atomic inline replaced** box
//! that takes the CSS 2.1 §10.3.2 *default object size*. `<audio controls>` was an ordinary inline,
//! so no default object size could ever apply to it and the `17` was just a line box.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`), as
//! `getComputedStyle(el).display / getBoundingClientRect()`:
//!
//! ```text
//!                            Chrome         before          after
//!   <audio>                  none 0x0       none 0x0        none 0x0      ✓ (t1464)
//!   <audio controls>         inline 300x54  inline 0x17     inline 300x54
//!   <video>       CONTROL    inline 300x150 inline 300x150  inline 300x150 ✓
//! ```
//!
//! ⚠ **`<video>` IS THE ROW THAT KEEPS THE SIZE HONEST.** An audio control bar is **not** the shared
//! `300x150` default object size — Chrome draws it at `300x54`, and taking the shared default would
//! reserve nearly three times too much height. Both numbers are asserted so a later tick cannot
//! collapse them into one constant.
//!
//! ⚠ And the bare `<audio>` row is what keeps the change scoped: `is_atomic_inline_replaced` tests
//! `display: Inline`, and the UA sheet makes a bare `<audio>` `display: none`, so only the
//! `controls` form is ever reached. **`audio` is deliberately absent from `is_replaced_element`** —
//! like `iframe`/`object`/`embed` it is atomic in a line without taking §10.4's ratio adjustment,
//! because a control bar has no aspect ratio.
//!
//! Mutations that must turn this red:
//!   1. remove `audio` from `is_atomic_inline_replaced` → the controls row returns to `0x17`
//!   2. drop the `audio` height special-case          → it reads `300x150`, the video default
//!   3. remove `audio` from `default_object_tag`      → the width falls back and it reads `0x…`

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0 }
</style></head><body>
<audio id="a1"></audio><audio id="a2" controls></audio><video id="v1"></video>
<div id="out">-</div>
<script>
function d(k){var e=document.getElementById(k);var r=e.getBoundingClientRect();
 return k+'='+getComputedStyle(e).display+'/'+Math.round(r.width)+'x'+Math.round(r.height);}
document.getElementById('out').textContent=['a1','a2','v1'].map(d).join(' ');
</script></body></html>"##;

#[test]
fn an_audio_control_bar_is_a_replaced_box_at_chromes_size() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://audio.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("AUDIO: {got}");

    // ── VACUITY. `<video>` must already take the default object size, or the audio rows below are
    //    measuring whether replaced sizing works at all rather than whether AUDIO reaches it.
    assert!(
        got.contains("v1=inline/300x150"),
        "VACUOUS: <video> does not take the default object size, so the audio rows prove nothing — \
         got {got:?}"
    );

    // Chrome headless, all three rows.
    let want = "a1=none/0x0 a2=inline/300x54 v1=inline/300x150";
    assert_eq!(
        got, want,
        "\n  an <audio controls> is an atomic inline replaced box at 300x54\n  want: {want}\n  got:  {got}"
    );
}
