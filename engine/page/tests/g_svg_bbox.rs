//! **G_SVG_BBOX — `getBBox()` answers in USER SPACE, and it used to throw.**
//!
//! Measured at t602/t603 while pinning the SVG rows: `getBBox` was `undefined`, so
//! `node.getBBox().width` was a **TypeError that kills the caller's frame** — the same throw-class
//! shape as the `getComputedStyle` defect (t596/t597), and the reason it matters is the same. This
//! is *the* geometry call charting code makes: D3, Chart.js's SVG paths, and every hand-rolled label
//! placer measures shapes and text this way.
//!
//! **The alternative a page reaches for is worse than missing.** `getBoundingClientRect` on an SVG
//! child answers in **CSS-box** coordinates — a wrong number rather than an absent one, because SVG
//! children are not CSS boxes here. `getBBox` is defined in the element's **own** coordinate system:
//! unaffected by where the `<svg>` sits on the page, by the viewport, or by scroll. That is why it is
//! computed from the element's geometry ATTRIBUTES and not from the layout snapshot — and it is why
//! the gate asserts a `<rect x="10" y="20">` reports **10,20** rather than its on-screen position.
//!
//! ## What is asserted, and what is honestly zero
//!
//! ```text
//! rect      x/y/width/height          → 10,20,50,30      exact
//! circle    cx,cy,r                   → 80,40,40,40      (centre − r, diameter)
//! ellipse   cx,cy,rx,ry               → 150,30,40,20
//! line      x1,y1,x2,y2               → min/extent, and a HORIZONTAL line has zero height
//! polygon   points="…"                → the min/max of the point list
//! <g>       —                         → the UNION of the children, which is all a group has
//! <text>    x/y                       → origin, size 0   honestly: it needs shaping
//! ```
//!
//! **`<path>` is EXACT since t630; `<text>` and elliptical arcs report zero size on purpose.** A plausible-looking guess for a text bbox
//! silently mis-places every label that trusts it; a zero is visible. That is the same choice
//! `clip-path` made when it left `shape()` unclipped rather than approximating it, and it is the one
//! this project keeps arriving at: **a wrong answer costs more than an obviously missing one.**
//!
//! Stroke width, `transform` and markers are not included. Excluding stroke is *correct* (`getBBox`
//! is specified on fill geometry); the missing `transform` is a real gap, named here rather than
//! hidden.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<svg id="s" width="300" height="120" style="margin-left:40px">
  <rect id="r" x="10" y="20" width="50" height="30"/>
  <circle id="c" cx="100" cy="60" r="20"/>
  <ellipse id="e" cx="170" cy="40" rx="20" ry="10"/>
  <line id="l" x1="200" y1="10" x2="240" y2="10"/>
  <polygon id="p" points="10,90 40,90 40,110"/>
  <g id="g"><rect id="gr" x="5" y="5" width="10" height="10"/><rect x="20" y="30" width="10" height="10"/></g>
  <text id="t" x="7" y="115">label</text>
  <!-- `<path>`, the element every icon set is made of. The two CURVES are the load-bearing cases:
       their control points sit at 20 and their real extrema at 15 and 10, so a control-point hull
       (the easy wrong implementation) gives 20 and is caught. -->
  <path id="pl" d="M4 4 L20 4 L20 12 L4 12 Z"/>
  <path id="pc" d="M0 0 C 0 20 20 20 20 0"/>
  <path id="pq" d="M0 0 Q 10 20 20 0"/>
  <path id="pr" d="m5 5 l10 0 l0 6 z"/>
  <path id="pa" d="M0 0 A 5 5 0 0 1 10 10"/>
</svg>
<div id="out">-</div>
<script>
  var R = [], bb = function(id){
    var e = document.getElementById(id);
    if (!e || typeof e.getBBox !== 'function') return 'NOFN';
    var b = e.getBBox();
    return b.x + ',' + b.y + ',' + b.width + ',' + b.height;
  };
  ['r','c','e','l','p','g','t'].forEach(function(id){ R.push(id + '=' + bb(id)); });
  ['pl','pc','pq','pr','pa'].forEach(function(id){ R.push(id + '=' + bb(id)); });
  // The idiom that used to throw, run for real.
  var threw = false;
  try { document.getElementById('r').getBBox().width.toFixed(1); } catch (err) { threw = true; }
  R.push('threw=' + threw);
  // getBBox is USER SPACE: the <svg> is pushed 40px right, and the rect's bbox must not move.
  R.push('cssX=' + Math.round(document.getElementById('r').getBoundingClientRect().x));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn getbbox_reports_user_space_geometry() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://bbox.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SVG BBOX: {got}");

    for (claim, why) in [
        (
            "threw=false",
            "**THE POINT**: `getBBox()` existed nowhere, so `node.getBBox().width` was a TypeError \
             that killed the caller's frame. This is the call every charting library makes",
        ),
        (
            "r=10,20,50,30",
            "a `<rect>`'s bbox is its own x/y/width/height — and note it is 10,20 and NOT the \
             element's on-screen position: `getBBox` is USER SPACE",
        ),
        (
            "c=80,40,40,40",
            "a `<circle>` is centre−r with diameter extent (cx=100, cy=60, r=20)",
        ),
        ("e=150,30,40,20", "an `<ellipse>` uses rx/ry independently"),
        (
            "l=200,10,40,0",
            "a HORIZONTAL `<line>` has zero HEIGHT — a bbox routine that took a max instead of an \
             extent, or clamped degenerate axes to something non-zero, fails exactly here",
        ),
        (
            "p=10,90,30,20",
            "a `<polygon>` is the min/max of its `points` list, with commas and whitespace both \
             acting as separators",
        ),
        (
            "g=5,5,25,35",
            "a `<g>` has no geometry of its own, so its bbox is the UNION of its children — here two \
             rects at (5,5,10,10) and (20,30,10,10) union to (5,5,25,35)",
        ),
        (
            "t=7,115,0,0",
            "`<text>` reports its ORIGIN with a ZERO size, honestly: a text bbox needs shaping, and a \
             plausible-looking guess silently mis-places every label that trusts it while a zero is \
             visible. Same choice `clip-path` made leaving `shape()` unclipped",
        ),
        (
            "pl=4,4,16,8",
            "a `<path>` of straight segments is the min/max of its points — and until t630 `<path>` \
             had NO arm in `svg_bbox` at all, so `getBBox()` answered 0x0 for the single most common \
             SVG element. Every icon set (Lucide, Feather, Material), every chart shape generator and \
             every logo is made of these",
        ),
        (
            "pc=0,0,20,15",
            "**THE CASE THAT PROVES THE BOUNDS ARE EXACT.** This cubic's control points are at y=20 \
             and the curve itself only reaches y=15. A control-point hull — the easy wrong \
             implementation — reports 20 and is STRICTLY LARGER than the curve. A too-large bbox is a \
             wrong answer that looks plausible: it mis-positions every tooltip anchored to an icon and \
             mis-sizes every chart hit-area, while reading as 'close enough'",
        ),
        (
            "pq=0,0,20,10",
            "the same for a quadratic: control point at y=20, real extremum at y=10. Solved from the \
             derivative's root, not guessed from the hull",
        ),
        (
            "pr=5,5,10,6",
            "relative commands (`m`/`l`) accumulate from the current point, and `z` returns to the \
             SUBPATH start — not to the origin",
        ),
        (
            "pa=0,0,0,0",
            "**an elliptical arc returns NO bbox, deliberately.** Bounding one exactly needs the \
             endpoint->centre parameterisation and the extrema of a rotated ellipse over the swept \
             angle range; that work is not done, and the honest answer to 'what is this path's box' \
             when part of it cannot be bounded is NO ANSWER rather than a guess in either direction. \
             The same choice `<text>` makes above",
        ),
        (
            "cssX=48",
            "…and the contrast that makes the user-space claim meaningful: the SAME rect's \
             `getBoundingClientRect().x` is 48 (the page's 8px body margin + the svg's 40px \
             margin-left), a CSS-box number in a different coordinate system entirely. A page that \
             reaches for it instead of `getBBox` gets a WRONG value, not a missing one",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_SVG_BBOX: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
