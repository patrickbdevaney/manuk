//! **G_ASPECT_RATIO_DEFINITE_HEIGHT — the box knew its own height and did not tell its children.**
//!
//! CSS Sizing 4: a **definite width plus an `aspect-ratio` makes the block size definite**, so a
//! `height: 100%` child resolves against it. `layout_block` computed exactly that height — the ratio
//! transfer below `inner_definite_h` is literally `width / r` — but only **after** the children had
//! been laid out. The height offered to them was `None`, so every percentage inside a ratio box
//! resolved against nothing and **collapsed to zero**.
//!
//! ⭐ This is not a near-miss; it is a box that vanishes. And it is the dominant modern card idiom:
//! Tailwind's `aspect-video` / `aspect-square` wrapper with an `h-full object-cover` image inside it
//! is how essentially every card grid on the web puts a picture above a title. On
//! `www.fragrantica.com` the wrapper is 480px tall in Chrome and the deficit repeats down 37 sibling
//! cards.
//!
//! ⚠ **A REPLACED child fails DIFFERENTLY, and the difference is the tell.** A `<div height:100%>`
//! collapsed to **0**; an `<img width:100%;height:100%>` came out **300** — its own intrinsic ratio —
//! because a replaced element with no usable percentage falls back to its natural size instead of to
//! zero. Same cause, two signatures, and only the pair identifies it.
//!
//! **PRICED ON THE CORPUS BEFORE IT WAS BUILT** (`docs/bench/corpus-crux-trend.txt`; a Chrome probe
//! asked each page for boxes with a computed `aspect-ratio` other than `auto` whose child's height
//! exactly fills them): **14 of 117 measured sites — 12%** — carry the construct, and 60 of 117 have
//! ratio boxes at all. Named hits include `www.paypal.com` (15), `ru.restaurantguru.com` (17),
//! `www.aftenbladet.no` (15), `pasarbokep.com` (16), `www.alphanews.live` (122), `bhramarah.in` (69).
//!
//! **EVERY EXPECTED NUMBER IS HEADLESS CHROME'S ANSWER FOR THIS EXACT FIXTURE**
//! (`--headless=new --hide-scrollbars --window-size=1200,800 --dump-dom`), not a derivation:
//!
//! ```text
//!     parent                                   child                    parent   child     was
//!     width:400px; aspect-ratio:16/9           width:100%; height:100%  400x225  400x225  400x0
//!     width:400px; aspect-ratio:16/9           height:100%              400x225  400x225  400x0
//!     width:400px; aspect-ratio:16/9           height:50%               400x225  400x113  400x0
//!     width:400px; aspect-ratio:16/9           height:100% > height:50% 400x225  400x113  400x0
//!     width:400px; height:225px        CTRL    height:100%              400x225  400x225   same
//!     width:400px; aspect-ratio:16/9;          height:100%              400x100  400x100   same
//!       height:100px                   CTRL
//!     height:225px; aspect-ratio:16/9  CTRL    height:100%              400x225  400x225   same
//! ```
//!
//! The three CONTROLS are what keep the fix from being "make every height definite":
//!
//! - **an explicit `height` parent already worked** — if that row moves, the change was not scoped to
//!   the ratio-derived case;
//! - **`aspect-ratio` BESIDE an explicit `height`** must keep the explicit height (400x**100**, not
//!   400x225) — the ratio must not overwrite a value the page asked for;
//! - **the ratio running the OTHER way** (`height: 225px` + a ratio deriving the WIDTH) already had a
//!   definite height and must be untouched.
//!
//! ⚠ RESIDUE, NAMED AND NOT FIXED HERE: the `<img>` case's PARENT is **229** in Chrome and 225 here —
//! an inline replaced child sits on a line box that adds the font's descender space below it. That is
//! inline-layout geometry, a different mechanism, and this gate asserts 225 so that a future fix to
//! it is a visible, deliberate change rather than a silent drift.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font:16px/1.2 monospace}
.r{width:400px;aspect-ratio:16/9}
#p1 .k{width:100%;height:100%}
#p2 .k{height:100%}
#p3 .k{height:50%}
#p4{width:400px;height:225px}
#p4 .k{height:100%}
#p5{width:400px;aspect-ratio:16/9;height:100px}
#p5 .k{height:100%}
#p6 .mid{height:100%}
#p6 .k{height:50%}
#p7{height:225px;aspect-ratio:16/9}
#p7 .k{height:100%}
</style></head><body>
<div class=r id=p1><div class=k id=k1></div></div>
<div class=r id=p2><div class=k id=k2></div></div>
<div class=r id=p3><div class=k id=k3></div></div>
<div id=p4><div class=k id=k4></div></div>
<div id=p5><div class=k id=k5></div></div>
<div class=r id=p6><div class=mid id=m6><div class=k id=k6></div></div></div>
<div id=p7><div class=k id=k7></div></div>
<div class=r id=p8><img id=k8 style="width:100%;height:100%"
   src="data:image/gif;base64,R0lGODlhkAEsAfAAAP8AAAAAACH5BAAAAAAALAAAAACQASwBAAIRhI+py+0Po5y02ouz3rz7rxUAOw=="></div>
<div id=out></div><script>
var s='';
for (var id of ['p1','k1','p2','k2','p3','k3','p4','k4','p5','k5','p6','m6','k6','p7','k7','p8','k8']){
  var r=document.getElementById(id).getBoundingClientRect();
  s+=id+'='+Math.round(r.width)+'x'+Math.round(r.height)+';';
}
document.getElementById('out').textContent=s;
</script></body></html>"##;

#[test]
fn a_ratio_derived_height_is_definite_for_the_percentage_children_inside_it() {
    let fonts = FontContext::new();
    let p = manuk_page::Page::load(HTML, "http://x/", &fonts, 1200.0);
    let root = p.dom().root();
    let hits = manuk_css::query_selector_all(p.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    let got = p.dom().text_content(hits[0]);
    assert!(
        got.contains('='),
        "the fixture's script must run, or this gate measures nothing — got {got:?}"
    );
    let mut seen = std::collections::HashMap::new();
    for row in got.split(';').filter(|r| r.contains('=')) {
        let (k, v) = row.split_once('=').unwrap();
        seen.insert(k.to_string(), v.to_string());
    }

    // (id, expected "WxH", what the row is for)
    let expect: [(&str, &str, &str); 17] = [
        (
            "p1",
            "400x225",
            "t1 the ratio box itself — already right, and must stay right",
        ),
        (
            "k1",
            "400x225",
            "t1 width:100% + height:100% inside it — was 400x0",
        ),
        ("p2", "400x225", "t2 the ratio box"),
        ("k2", "400x225", "t2 a bare height:100% — was 400x0"),
        ("p3", "400x225", "t3 the ratio box"),
        ("k3", "400x113", "t3 height:50% — half of 225, was 400x0"),
        ("p4", "400x225", "n1 CONTROL an EXPLICIT height parent"),
        (
            "k4",
            "400x225",
            "n1 CONTROL height:100% under it — already worked, must not move",
        ),
        (
            "p5",
            "400x100",
            "n2 CONTROL ratio BESIDE an explicit height — the height WINS",
        ),
        (
            "k5",
            "400x100",
            "n2 CONTROL height:100% resolves to the explicit 100, never 225",
        ),
        ("p6", "400x225", "t4 the ratio box"),
        (
            "m6",
            "400x225",
            "t4 NESTED — a height:100% mid-box is itself definite, was 400x0",
        ),
        ("k6", "400x113", "t4 NESTED — height:50% of that, was 400x0"),
        (
            "p7",
            "400x225",
            "n3 CONTROL the ratio runs the OTHER way (height given, width derived)",
        ),
        (
            "k7",
            "400x225",
            "n3 CONTROL height:100% under it — already worked, must not move",
        ),
        (
            "p8",
            "400x225",
            "⚠ RESIDUE the <img>'s parent is 229 in Chrome (inline descender space); asserted at \
             225 so a future inline fix is deliberate, not drift",
        ),
        (
            "k8",
            "400x225",
            "t5 a REPLACED child — was 400x300, its OWN intrinsic ratio, not 0",
        ),
    ];
    for (id, want, why) in expect {
        let got_v = seen.get(id).map(String::as_str).unwrap_or("<missing>");
        assert_eq!(
            got_v, want,
            "{why}: #{id} expected {want} — headless Chrome's own number for this exact fixture — \
             but got {got_v}. A height of 0 means the ratio-derived block size was still offered to \
             the children as indefinite; a 400x300 on the replaced row means the <img> fell back to \
             its own intrinsic ratio instead of resolving 100% of its parent; a CONTROL row that \
             moved means the change was not scoped to the ratio-derived case."
        );
    }
}
