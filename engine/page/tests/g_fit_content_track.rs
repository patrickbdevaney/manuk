//! **G_FIT_CONTENT_TRACK — `fit-content(<length>)` on a grid track collapsed to `auto`, so it did
//! the OPPOSITE of what it says.**
//!
//! Grid §7.2.2 defines the track size as `min(max-content, max(min-content, <length>))` — *"as wide
//! as the content wants, but never wider than this"*. The cascade mapped it to `TrackSize::Auto`,
//! and an `auto` track **absorbs free space**: `fit-content(50px)` on a 400px grid produced a
//! **400px** track where every browser gives 50.
//!
//! ⚠ **A clamp that stretches is not a partial implementation, it is the inverse of the feature.**
//! The idiom is "hold this column to its content, capped" — a sidebar, a label column, a truncating
//! table cell, the `fit-content(20ch)` gutter in every documentation layout — and every one of them
//! took the whole container instead.
//!
//! Everything needed was already present: taffy implements the §7.2.2 clamp itself and was simply
//! never asked, because `stylo_map` handed it `Auto` one layer up. **Borrowed, not built.**
//!
//! **To watch it go RED:** restore `TS::FitContent(_) => crate::TrackSize::Auto` in `stylo_map` —
//! `fit=`, `big=` and `small=` all jump to 400 (the container), while the two CONTROLS keep passing, which is
//! what says the controls are not merely restating the fix.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 body { font: 10px/1 monospace; margin: 0 }
 .g { display: grid; width: 400px; }
 /* "AAAA BBBB" at 10px monospace: max-content 54, min-content 24 (the longest word). */
 #fit   { grid-template-columns: fit-content(50px); }
 #big   { grid-template-columns: fit-content(300px); }
 #small { grid-template-columns: fit-content(5px); }
 #auctl { grid-template-columns: auto; }
 #mcctl { grid-template-columns: max-content; }
</style></head><body>
<div class="g" id="fit"><div id="a">AAAA BBBB</div></div>
<div class="g" id="big"><div id="b">AAAA BBBB</div></div>
<div class="g" id="small"><div id="c">AAAA BBBB</div></div>
<div class="g" id="auctl"><div id="d">AAAA BBBB</div></div>
<div class="g" id="mcctl"><div id="e">AAAA BBBB</div></div>
<div id="out">-</div>
<script>
  var R = [];
  [['fit','a'], ['big','b'], ['small','c'], ['auctl','d'], ['mcctl','e']].forEach(function (p) {
    R.push(p[0] + '=' + Math.round(document.getElementById(p[1]).getBoundingClientRect().width));
  });
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn fit_content_clamps_a_track_instead_of_stretching_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://fitcontent.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "mcctl=54",
            "CONTROL FIRST — max-content of \"AAAA BBBB\" is 54px, which is the number every clamp \
             below is measured against. Asserting it first means a wrong font or a wrong measure \
             fails HERE rather than being absorbed into a clamp result",
        ),
        (
            "auctl=400",
            "CONTROL — an `auto` track absorbs the free space and takes the whole 400px container. \
             This is what `fit-content` was doing, and it must keep doing it",
        ),
        (
            "fit=50",
            "THE GATE. min(max-content 54, max(min-content 24, 50)) = 50. It read 400 — the clamp \
             stretching to the container, which is the inverse of the feature",
        ),
        (
            "big=54",
            "the argument is a CEILING, not a width: `fit-content(300px)` on 54px of content is 54. \
             A fix that simply used the argument as the track size passes `fit=` and fails here",
        ),
        (
            "small=24",
            "...and it is floored at MIN-content: `fit-content(5px)` cannot go below the longest \
             unbreakable word, so 24 rather than 5. The other half of the same clamp, and the half a \
             naive `min(arg, max-content)` gets wrong",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FIT_CONTENT_TRACK: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
