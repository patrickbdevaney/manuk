//! **G_TRANSITION_INTERPOLATION — a CSS TRANSITION was never sampled, so a page mid-transition
//! rendered its END state and `getComputedStyle` agreed with the end state.**
//!
//! t1301/t1309 taught the engine to interpolate a `@keyframes` animation. The *other* four legs of
//! WPT's `css/support/interpolation-testcommon.js` are transitions, and they were untouched:
//!
//! ```text
//!   CSS Transitions                                             ← this tick
//!   CSS Transitions with transition: all                        ← this tick
//!   CSS Transitions with transition-behavior:allow-discrete     ← this tick
//!   CSS Transitions with transition-property:all + allow-discrete ← this tick
//!   CSS Animations            (t1301)
//!   Web Animations            (t1309, through the CSS-Animations path)
//! ```
//!
//! **The mechanism the harness uses is the same trick the animation legs use, one property over.**
//! `transition-duration: 100s; transition-delay: -50s` places a transition at its half-way point *at
//! time zero*, and the timing function is warped so that the fixed 0.5 sample eases to the tested
//! progress. So the value is again a STATIC function of the cascade — no frame loop, no clock.
//!
//! ⚠⚠⚠ **WHAT A TRANSITION NEEDS THAT AN ANIMATION DOES NOT: A MEMORY.** An animation's endpoints are
//! both in the stylesheet. A transition's `from` is *the value this element had before the style
//! change*, which exists nowhere in the document — so the cascade has to remember what it published
//! last time. That per-node before-change table is the whole of the new state, and everything else
//! is borrowed (`ComputedValues::transition_properties` expands `all`, `LonghandId::is_animatable`
//! and `is_discrete_animatable` classify, `Animate` interpolates).
//!
//! ⚠⚠⚠ **THE GUARD, AND IT IS LOAD-BEARING FOR EVERY REAL PAGE: SAMPLE ONLY WHEN THE ELAPSED TIME IS
//! GENUINELY POSITIVE.** The document clock is 0 (`animation::time_ms`, never advanced by anything
//! yet), so an ordinary `transition: width .3s` has `elapsed = 0 - 0 = 0` and sits at progress 0 —
//! i.e. *its start value*. Sampling that would make every hover, every accordion, every menu on the
//! real web render the state it was leaving. `n1` is that control: the identical row with the delay
//! at its initial `0s` must read the after-change value, unchanged from today.
//!
//! ⚠ **The prev-value table is NOT overwritten while a transition is running on that element**, and
//! that is what makes the harness's shape work at all. It runs `interpolate()` on *every* target and
//! only then `measure()`s them — so target 1 is re-cascaded once per later target, and an
//! unconditional overwrite would make its before-change value become its after-change value on the
//! very next recalc and the answer would be the end state. `t1` and `t2` sit in front of six later
//! rows for exactly this reason: they are measured after ten further cascades.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 div { height: 4px; }
</style></head><body>
 <div id="t1" style="width:50px"></div>
 <div id="t2" style="width:50px"></div>
 <div id="t3" style="color:rgb(0,0,0)"></div>
 <div id="t4" style="width:50px"></div>
 <div id="t5" style="width:50px"></div>
 <div id="t6" style="display:block"></div>
 <div id="t7" style="display:block"></div>
 <div id="t8" style="transform:matrix3d(2,0,0,0, 0,2,0,0, 0,0,0,0, 0,0,0,1)"></div>
 <div id="t9" style="transform:matrix3d(2,0,0,0, 0,2,0,0, 0,0,0,0, 0,0,0,1)"></div>
 <div id="n1" style="width:50px"></div>
 <div id="n2" style="width:50px"></div>
 <div id="n3" style="width:50px"></div>
 <div id="n4" style="width:50px"></div>
 <div id="n5" style="width:50px"></div>
 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   function E(id){ return document.getElementById(id); }
   // Every row: [id, transition-property, transition-delay, transition-duration,
   //             transition-timing-function, transition-behavior, prop, newValue]
   var rows = [
     // ── THE CLAIM: half of 50px -> 100px is 75px, and it is neither endpoint.
     ['t1','width',        '-50s', '100s','linear',        '',              'width','100px'],
     // `transition-property: all` must reach the same value through the shorthand expansion.
     ['t2','all',          '-50s', '100s','linear',        '',              'width','100px'],
     // A non-length type, so this cannot be a length special case.
     ['t3','color',        '-50s', '100s','linear',        '',              'color','rgb(100,100,100)'],
     // The POSITION is read from delay/duration and is not a hardcoded 0.5: raw 0.25 of 50->100.
     ['t4','width',        '-25s', '100s','linear',        '',              'width','100px'],
     // The TIMING FUNCTION is consulted: steps(1,end) outputs 0 at raw 0.5, so this holds the
     // BEFORE-change value while `t1` — the same row with `linear` — gives 75.
     ['t5','width',        '-50s', '100s','steps(1, end)', '',              'width','100px'],
     // DISCRETE, behavior:normal -> a discrete property does not transition at all: jump to `inline`.
     ['t6','display',      '-30s', '100s','linear',        'normal',        'display','inline'],
     // …the SAME row with allow-discrete: a discrete transition flips at 0.5, and raw 0.3 < 0.5, so
     // it still reads `block`. t6/t7 differ in ONE keyword and must differ in their answer.
     ['t7','display',      '-30s', '100s','linear',        'allow-discrete','display','inline'],
     // ── A CONTINUOUS PROPERTY WHOSE VALUE PAIR IS NEVERTHELESS DISCRETE. The from-matrix is
     // SINGULAR (m33 = 0), so `matrix3d -> matrix` has no midpoint and the spec makes the pair
     // discrete — which the behavior keyword must then be consulted about a SECOND time, on the
     // pair rather than on the property id. `t8` jumps, `t9` flips at 0.5 (raw 0.3 < 0.5).
     ['t8','transform',    '-30s', '100s','linear',        'normal',        'transform','matrix(3, 0, 0, 3, 0, 0)'],
     ['t9','transform',    '-30s', '100s','linear',        'allow-discrete','transform','matrix(3, 0, 0, 3, 0, 0)'],
     // ── CONTROLS. All five must read the after-change value, exactly as they do today.
     // (1) THE GUARD: the initial `0s` delay -> elapsed 0 -> not running -> no sample.
     ['n1','width',        '0s',   '100s','linear',        '',              'width','100px'],
     // (2) no transition declared at all.
     ['n2','',             '',     '',    '',              '',              'width','100px'],
     // (3) a transition on a DIFFERENT property than the one that changed.
     ['n3','color',        '-50s', '100s','linear',        '',              'width','100px'],
     // (4) zero duration.
     ['n4','width',        '-50s', '0s',  'linear',        '',              'width','100px'],
     // (5) FINISHED: 200s elapsed of a 100s transition.
     ['n5','width',        '-200s','100s','linear',        '',              'width','100px']
   ];
   // PHASE 1 — exactly WPT's shape: interpolate EVERY target, then measure every target. Each
   // `interpolate` forces a recalc, so `t1` is re-cascaded once per later row.
   for (var i=0;i<rows.length;i++){
     var r=rows[i], e=E(r[0]);
     getComputedStyle(e).getPropertyValue(r[6]);   // force the recalc that records the FROM value
     if (r[1]) {
       e.style.transitionProperty = r[1];
       e.style.transitionDelay = r[2];
       e.style.transitionDuration = r[3];
       e.style.transitionTimingFunction = r[4];
       if (r[5]) { e.style.transitionBehavior = r[5]; }
     }
     e.style.setProperty(r[6], r[7]);
   }
   // PHASE 2 — measure.
   var out=[];
   for (var j=0;j<rows.length;j++){
     var q=rows[j];
     out.push(q[0]+'='+getComputedStyle(E(q[0])).getPropertyValue(q[6]));
   }
   E('out').textContent = out.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_css_transition_resolves_to_an_interpolated_value() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://trans.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("TRANSITION-INTERPOLATION {got}");

    // RED, run: delete the `crate::transition::sample_for` block in
    // `stylo_engine::cascade_via_stylo_sized` — every `t*` row collapses onto its AFTER-change value
    // (`t1=100px t3=rgb(100, 100, 100) t5=100px t7=inline`) while the five `n*` controls do not
    // move, which is what says this gate measures the transition path and not the cascade at large.
    assert_eq!(
        got,
        "t1=75px t2=75px t3=rgb(50, 50, 50) t4=62.5px t5=50px t6=inline t7=block \
         t8=matrix(3, 0, 0, 3, 0, 0) t9=matrix(2, 0, 0, 2, 0, 0) \
         n1=100px n2=100px n3=100px n4=100px n5=100px",
        "a running CSS transition must resolve to the INTERPOLATED value at its current position. \
         `t1=75px` is the core claim (half of 50px→100px) and is measured AFTER eleven further \
         cascades, which is what proves the before-change value is not overwritten while the \
         transition runs. `t2` reaches it through `transition-property: all`. `t3` proves a \
         non-length type goes through `Animate`. `t4=62.5px` proves the position is read from \
         delay/duration rather than assumed to be the half-way point. `t5=50px` proves the timing \
         function is consulted — `steps(1, end)` outputs 0 at raw 0.5 where `t1`'s `linear` gives \
         0.5. `t6`/`t7` differ in ONE keyword: a discrete property does not transition under \
         `transition-behavior: normal` (jump to `inline`) and flips at 0.5 under `allow-discrete` \
         (raw 0.3 < 0.5, so still `block`). `t8`/`t9` are that same pair for a CONTINUOUS \
         property whose VALUE PAIR is nevertheless discrete: the from-matrix is SINGULAR \
         (m33 = 0), so `matrix3d → matrix` has no midpoint, and the behavior keyword therefore has \
         to be consulted a second time on the PAIR rather than on the property id. Without that \
         second consultation the four transition legs of WPT's \
         `non-invertible-matrix-interpolation` cost 22 subtests in `css/css-transforms`, which is \
         how this arm was found — as a measured REGRESSION against this tick's own control run, \
         not as a reading of the spec. \
         ⚠⚠⚠ THE FIVE CONTROLS ARE THE REAL-PAGE SAFETY ARGUMENT. `n1` is THE GUARD: the identical \
         row with the initial `transition-delay: 0s` has elapsed time 0, so it is NOT running and \
         must read the after-change value. The document clock is 0 for every static render, so \
         `n1` is the shape EVERY transition on the real web has — a hover, an accordion, a menu — \
         and sampling it would render the state the page was LEAVING. `n2` no transition, `n3` a \
         transition on a different property, `n4` zero duration, `n5` a finished transition"
    );
}
