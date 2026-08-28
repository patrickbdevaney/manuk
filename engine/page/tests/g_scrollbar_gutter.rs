//! **G_SCROLLBAR_GUTTER — the page reserved space for a scrollbar and the engine gave the space away.**
//!
//! `scrollbar-gutter: stable` (CSS Overflow 3 §3.2) is the modern spelling of the oldest
//! layout-shift-prevention idiom on the web. The classic recipe was `html { overflow-y: scroll }` —
//! force a scrollbar onto every page so the content does not jump 15px sideways the moment it grows
//! past one screen — and this engine has reserved that gutter since t469. `stable` asks for the same
//! strip **without** forcing a scrollbar to appear, and the engine did not know the property at all:
//! `scrollbar-gutter` is `engine = "gecko"` in stylo 0.19, so the servo build we borrow never parses
//! it and the declaration evaporated. Every box on such a page was one scrollbar too wide.
//!
//! ⭐ **A root that is 15px too wide is not a 15px bug.** It is the width-launders-into-dy shape the
//! render burndown ranks first: a container a few px too wide re-wraps its prose, the line count
//! changes, and a whole-line height error cascades down every following block. It was found from the
//! other end — `www.fragrantica.com`'s shape dump has Chrome's `<body>` at `1185` against ours at
//! `1200`, repeated through 2,988 elements — and the site's own sheet says only `html {
//! scrollbar-gutter: stable }`.
//!
//! **PRICED ON THE CORPUS BEFORE IT WAS BUILT** (`docs/bench/corpus-crux-trend.txt`, 97 of 200 sites
//! reachable and probed for the computed value on the root): **2 sites set it** —
//! `www.fragrantica.com` (shape 0.733, near the 0.75 bar) and `www.aftenbladet.no` (0.352). Small,
//! named, and not guessed at.
//!
//! ⚠⚠⚠ **THE ONE THING THAT COULD NOT BE REASONED OUT: A RESERVED GUTTER SURVIVES
//! `--hide-scrollbars` AND A SHOWN ONE DOES NOT.** This engine models the host's
//! scrollbars-take-no-space mode (`manuk_layout::set_scrollbars_hidden`) because the fidelity
//! oracle's Chrome reference has run with `--hide-scrollbars` for the whole project. The obvious
//! assumption — that a hidden scrollbar reserves nothing, whatever the page asked — is **wrong**,
//! and only a measurement says so. Headless Chrome, one 200×100 box with a `width: 100%` child:
//!
//! ```text
//!     box                                        --hide-scrollbars      default
//!     overflow: scroll                                     200            185
//!     overflow: hidden; scrollbar-gutter: stable           185            185
//!     overflow: clip;   scrollbar-gutter: stable           200            200
//! ```
//!
//! Row 1 goes through `scrollbar_gutter()` (which honours the mode); row 2 must not. Row 3 is the
//! spec's own line — `clip` clips without creating a scroll container, which is the entire reason it
//! exists beside `hidden` — so there is no gutter to reserve. All three are gated below.
//!
//! **EVERY EXPECTED NUMBER BELOW IS HEADLESS CHROME'S ANSWER FOR THAT EXACT FIXTURE** at an 800px
//! viewport (`google-chrome --headless=new --hide-scrollbars --window-size=800,600 --dump-dom`), not
//! a derivation:
//!
//! ```text
//!     ROOT (viewport 800)                              #a width   #a x   computed
//!     html{scrollbar-gutter:stable}                        785      0    stable
//!     html{...:stable both-edges}                          770     15    stable both-edges
//!     html{...:stable; scrollbar-width:thin}               790      0    stable
//!     html{...:stable; scrollbar-width:none}               800      0    stable
//!     (no declaration — CONTROL)                           800      0    auto
//!     html{scrollbar-gutter:both-edges}  (INVALID)         800      0    auto
//!
//!     BOX (200×100)                                    child w   child x   clientWidth
//!     overflow:auto    + stable                            185        0        185
//!     overflow:hidden  + stable                            185        0        185
//!     overflow:scroll  + stable                            185        0        185
//!     overflow:visible + stable        (CONTROL)           200        0        200
//!     overflow:clip    + stable        (CONTROL)           200        0        200
//!     overflow:auto    + stable both-edges                 170       15        170
//!     overflow:auto    (no gutter)     (CONTROL)           200        0        200
//! ```
//!
//! ⭐ The `overflow: scroll + stable` row is the one that rules out the obvious wrong shape: the two
//! reasons to reserve are **MAXed, not summed**. One scrollbar, one gutter — 185, never 170.
//!
//! ⚠ `scrollbar-width: none` + `stable` is a CONTROL with teeth: the reservation is *a scrollbar's
//! width*, and that width is zero, so the correct answer is to reserve nothing. An implementation
//! that hard-coded 15px for `stable` passes every other row here and fails this one.
//!
//! ⚠ `both-edges` ALONE is INVALID — it modifies `stable`, it is not a value — so the declaration is
//! dropped and the computed value stays `auto`. Accepting it would move the t1177 lie (an invalid
//! declaration applied as something) one property down.

use manuk_text::FontContext;

/// The four numbers the root fixtures report: `#a`'s width and x, plus the computed value, so a
/// regression in the CASCADE and one in the LAYOUT cannot be mistaken for each other.
const ROOT_TAIL: &str = r##"<div id=a style="height:10px"></div><div id=out></div><script>
var a=document.getElementById('a'), r=a.getBoundingClientRect();
document.getElementById('out').textContent =
  Math.round(r.width)+','+Math.round(r.left)+','+
  getComputedStyle(document.documentElement).getPropertyValue('scrollbar-gutter');
</script></body></html>"##;

fn root_case(css: &str) -> String {
    format!(
        "<!doctype html><html><head><style>body{{margin:0}}{css}</style></head><body>{ROOT_TAIL}"
    )
}

const BOXES: &str = r##"<!doctype html><html><head><style>
body{margin:0}
.b{width:200px;height:100px}
#b1{overflow:auto;scrollbar-gutter:stable}
#b2{overflow:hidden;scrollbar-gutter:stable}
#b3{overflow:scroll;scrollbar-gutter:stable}
#b4{overflow:visible;scrollbar-gutter:stable}
#b5{overflow:clip;scrollbar-gutter:stable}
#b6{overflow:auto;scrollbar-gutter:stable both-edges}
#b7{overflow:auto}
</style></head><body>
<div class=b id=b1><div id=c1 style="width:100%;height:10px"></div></div>
<div class=b id=b2><div id=c2 style="width:100%;height:10px"></div></div>
<div class=b id=b3><div id=c3 style="width:100%;height:10px"></div></div>
<div class=b id=b4><div id=c4 style="width:100%;height:10px"></div></div>
<div class=b id=b5><div id=c5 style="width:100%;height:10px"></div></div>
<div class=b id=b6><div id=c6 style="width:100%;height:10px"></div></div>
<div class=b id=b7><div id=c7 style="width:100%;height:10px"></div></div>
<div id=out></div><script>
var s='';
for(var i=1;i<=7;i++){
  var b=document.getElementById('b'+i), c=document.getElementById('c'+i);
  var rb=b.getBoundingClientRect(), rc=c.getBoundingClientRect();
  s+=Math.round(rc.width)+','+Math.round(rc.left-rb.left)+','+b.clientWidth+';';
}
document.getElementById('out').textContent=s;
</script></body></html>"##;

fn read_out(html: &str, fonts: &FontContext, vw: f32, what: &str) -> String {
    let p = manuk_page::Page::load(html, "http://x/", fonts, vw);
    let root = p.dom().root();
    let hits = manuk_css::query_selector_all(p.dom(), root, "#out");
    assert!(!hits.is_empty(), "{what}: fixture is missing #out");
    let got = p.dom().text_content(hits[0]);
    assert!(
        got.contains(','),
        "{what}: the fixture's script must run, or this gate measures nothing — got {got:?}"
    );
    got
}

#[test]
fn a_reserved_scrollbar_gutter_narrows_the_content_box_and_survives_hidden_scrollbars() {
    let fonts = FontContext::new();

    // ── THE ROOT. `html`'s own `overflow` is the initial `visible`, which is NOT a scroll
    // container — but CSS Overflow §3.3 propagates it to the VIEWPORT, which always is. That is the
    // single declaration both priced corpus sites actually write, so it is the first row here.
    //   (label, css, #a width, #a x, computed value)
    let root_expect: [(&str, &str, i32, i32, &str); 6] = [
        (
            "t1 root stable",
            "html{scrollbar-gutter:stable}",
            785,
            0,
            "stable",
        ),
        (
            "t2 root stable both-edges — content stays CENTRED",
            "html{scrollbar-gutter:stable both-edges}",
            770,
            15,
            "stable both-edges",
        ),
        (
            "t3 root stable + scrollbar-width:thin — the gutter is the SCROLLBAR's width",
            "html{scrollbar-gutter:stable;scrollbar-width:thin}",
            790,
            0,
            "stable",
        ),
        (
            "t4 root stable + scrollbar-width:none — a zero-width scrollbar reserves NOTHING",
            "html{scrollbar-gutter:stable;scrollbar-width:none}",
            800,
            0,
            "stable",
        ),
        ("n1 CONTROL no declaration", "", 800, 0, "auto"),
        (
            "n2 CONTROL `both-edges` alone is INVALID and is dropped",
            "html{scrollbar-gutter:both-edges}",
            800,
            0,
            "auto",
        ),
    ];
    for (label, css, w, x, computed) in root_expect {
        let got = read_out(&root_case(css), &fonts, 800.0, label);
        let cols: Vec<&str> = got.split(',').collect();
        let gw: i32 = cols[0].trim().parse().unwrap_or(-1);
        let gx: i32 = cols[1].trim().parse().unwrap_or(-1);
        let gc = cols.get(2).map(|s| s.trim()).unwrap_or("");
        assert_eq!(
            (gw, gx, gc),
            (w, x, computed),
            "{label}: expected (#a width, #a x, computed scrollbar-gutter) = ({w}, {x}, \
             {computed:?}) — headless Chrome's own numbers for this exact fixture at an 800px \
             viewport — but got {cols:?}. A width of 800 where 785 is expected means the \
             reservation never reached the initial containing block; a computed value of \"auto\" \
             alongside a correct width means the LAYOUT is right and the CASCADE is not reporting \
             it (the t1353 shape — right in one channel, wrong in the other)."
        );
    }

    // ── NON-ROOT SCROLL CONTAINERS, and the three controls that say which boxes are not one.
    //   (label, child width, child x within the box, clientWidth)
    let box_expect: [(&str, i32, i32, i32); 7] = [
        ("t5 overflow:auto + stable", 185, 0, 185),
        (
            "t6 overflow:hidden + stable — `hidden` IS a scroll container",
            185,
            0,
            185,
        ),
        (
            "t7 overflow:scroll + stable — the two reasons are MAXed, NOT summed",
            185,
            0,
            185,
        ),
        (
            "n3 CONTROL overflow:visible + stable — not a scroll container, inert",
            200,
            0,
            200,
        ),
        (
            "n4 CONTROL overflow:clip + stable — `clip` clips WITHOUT a scroll container",
            200,
            0,
            200,
        ),
        ("t8 overflow:auto + stable both-edges", 170, 15, 170),
        ("n5 CONTROL overflow:auto, no declaration", 200, 0, 200),
    ];
    let got = read_out(BOXES, &fonts, 800.0, "boxes");
    let rows: Vec<&str> = got.split(';').collect();
    for (i, (label, cw, cx, clientw)) in box_expect.iter().enumerate() {
        let cols: Vec<i32> = rows[i]
            .split(',')
            .map(|v| v.trim().parse().unwrap_or(-1))
            .collect();
        assert_eq!(
            (cols[0], cols[1], cols[2]),
            (*cw, *cx, *clientw),
            "{label}: expected (child width, child x in box, clientWidth) = ({cw}, {cx}, \
             {clientw}) — headless Chrome's own numbers for this exact fixture — but got {cols:?}. \
             A 170 on the `overflow: scroll + stable` row means the shown scrollbar and the \
             reserved gutter were SUMMED; a 185 on a CONTROL row means a box that is not a scroll \
             container reserved a gutter anyway; a correct width beside a full-padding-box \
             clientWidth means CSSOM-View's \"excluding the scrollbar\" is not being applied to the \
             reserved half."
        );
    }
}
