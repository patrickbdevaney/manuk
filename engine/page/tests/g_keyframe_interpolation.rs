//! **G_KEYFRAME_INTERPOLATION — the engine did not interpolate ANYTHING, and a `@keyframes`
//! animation rendered its base rule.**
//!
//! `@keyframes` parsed, `animation-*` cascaded, and the whole lot was then thrown away. The only
//! thing the engine ever did with an animation was one reveal-hack in `stylo_map.rs` — *"An animated
//! element renders its END state, not its first frame"*, which forces `opacity: 0` to `1` so that a
//! fifth of the corpus does not render blank — and that function's own comment states the position
//! plainly: **"We cannot animate."**
//!
//! So an element half-way through `@keyframes grow { from { width: 0 } to { width: 100px } }` had a
//! width of whatever its base rule said, and `getComputedStyle` agreed with the base rule. Not a
//! rounding error — the animation was **not evaluated at all**.
//!
//! ⭐ **The reason this is a tick and not a subsystem: the value is a STATIC function of the
//! cascade.** `animation-duration: 100s; animation-delay: -50s` places an animation at its half-way
//! point *at time zero* — no compositor, no frame loop, no rAF, nothing to schedule. That is also
//! exactly the idiom WPT's `css/support/interpolation-testcommon.js` is built on, which is why one
//! static evaluation reaches **194 `*-interpolation.html` files across twelve CSS areas**.
//!
//! **Every number is borrowed** (`STATUS.md`'s ladder, option 1 — no fork, no patch):
//! `Stylist::lookup_keyframes` for the ordered steps, `AnimationValue::from_computed_values` +
//! `Animate` for the interpolation itself, `ComputedTimingFunction::calculate_output` for the easing.
//! What is hand-rolled is only the part Stylo's servo build does not ship: *where in its timeline is
//! this animation right now*.
//!
//! ⚠ **The two bracketing keyframes are reached by CASCADING, not by reading the keyframe block.** A
//! keyframe declares SPECIFIED values and interpolation is defined on COMPUTED ones — and cascading
//! each side also gives the spec's fill-in for free: `w2`'s `from` keyframe declares no width, so the
//! from-side is the element's own `20px` base rule rather than an initial value.
//!
//! ⚠⚠ **The reveal-hack is DELIBERATELY still in force and this gate pins it** (`op0`). It exists
//! because 52 of 237 corpus sites pair `opacity: 0` with an animation, and at time zero a real
//! evaluation of a fade-in returns **0** — i.e. correctness here would make a fifth of the web
//! render blank until a clock exists. Per THE RATCHET a capability is never traded for another, so
//! the hack stays until the clock lands, and `op50` proves interpolation is nonetheless real: the
//! hack cannot produce `0.5`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 /* -50s of a 100s animation = the exact half-way point, at time zero. WPT's own idiom. */
 @keyframes grow   { from { width: 0px }   to { width: 100px } }
 @keyframes onlyto {                       to { width: 100px } }
 @keyframes fade   { from { opacity: 0 }   to { opacity: 1 } }
 @keyframes flip   { from { visibility: visible } to { visibility: hidden } }
 @keyframes noint  { from { width: auto }         to { width: 100px } }
 @keyframes tint   { from { color: rgb(0,0,0) }   to { color: rgb(100,100,100) } }
 @keyframes kfease { from { width: 0px; animation-timing-function: steps(2, end) }
                     to   { width: 100px } }
 div { height: 4px; }
 .half { animation-duration: 100s; animation-delay: -50s;
         animation-timing-function: linear; animation-fill-mode: both; }
</style></head><body>
 <!-- The core claim: half of 0->100 is 50, and it is not 0 and not 100. -->
 <div id="w1" class="half" style="animation-name: grow; width: 20px"></div>
 <!-- The FILL-IN: `onlyto` declares no `from` width, so the from-side must be the element's own
      20px base rule. Half of 20->100 is 60 — a number neither keyframe contains. -->
 <div id="w2" class="half" style="animation-name: onlyto; width: 20px"></div>
 <!-- A non-length type, proving `Animate` is doing the work and not a length special case. -->
 <div id="c1" class="half" style="animation-name: tint"></div>
 <!-- ⚠ MEASURED AGAINST THE PREDICTION AND THE PREDICTION WAS WRONG. This row was written
      expecting `hidden` — "visibility is discrete, so it flips at 0.5". It is not: `visibility` has
      its own animation rule (any interval with a `visible` endpoint is `visible` throughout), which
      Stylo implements and Chrome agrees with. Kept, because a row that corrected the author is
      worth more than one that confirmed him, and because it pins a REAL special case. -->
 <div id="v1" class="half" style="animation-name: flip"></div>
 <!-- NON-INTERPOLABLE, which is a different thing from discrete: `auto` and `100px` have no
      midpoint at all, so `Animate` REFUSES and the spec's answer is a 50% flip. This is the row
      that exercises the `Err(())` arm — the discrete keyword properties never reach it, because
      Stylo's `animate_discrete` already returns the flipped value as `Ok`. -->
 <div id="d1" class="half" style="animation-name: noint; width: 20px"></div>
 <!-- The KEYFRAME's own timing function beats the element's: `steps(2, end)` at raw 0.5 outputs
      0.5, so this happens to agree with linear — the discriminating row is `ks2` below. -->
 <div id="ks" class="half" style="animation-name: kfease; width: 20px"></div>
 <!-- …at raw 0.25 (delay -25s) `steps(2,end)` outputs 0, so width stays at the FROM keyframe's 0
      where linear would give 25. This is the row that proves the keyframe's function is read. -->
 <div id="ks2" style="animation-name: kfease; animation-duration: 100s; animation-delay: -25s;
                      animation-timing-function: linear; animation-fill-mode: both; width: 20px"></div>
 <!-- Opacity: `op50` proves interpolation is live (the reveal-hack cannot produce 0.5); `op0`
      pins the reveal-hack itself, which must NOT be regressed away by this tick. -->
 <div id="op50" class="half" style="animation-name: fade"></div>
 <div id="op0" style="animation-name: fade; animation-duration: 100s; animation-fill-mode: both;
                      animation-timing-function: linear; opacity: 0"></div>
 <!-- THE ENDPOINTS. `e1` is progress exactly 1 — a 100s animation delayed by -100s with
      `fill-mode: forwards`; `n5` below is its progress-exactly-0 twin. ⚠⚠⚠ **These two rows
      passed under a WRONG two-cascade fast path and that is why the source now carries a
      do-not-rebuild-this comment.** Short-circuiting progress 0 to the from-side cascade looks
      exact and is not — `Animate` at 0 is not the identity (`transform: none` ↔ `scale(2)` at 0 is
      `matrix(1,0,0,1,0,0)`, not `none`) — and it cost 36 subtests in `css/css-transforms` while
      EVERY row of this gate stayed byte-identical. The lesson is not "add a transform row": it is
      that an equivalence claimed over a fast path has to be checked against a population big
      enough to contain the counterexample, and 15 rows is not that population. -->
 <div id="e1" style="animation-name: grow; animation-duration: 100s; animation-delay: -100s;
                     animation-fill-mode: forwards; animation-timing-function: linear;
                     width: 20px"></div>
 <!-- CONTROLS, and they are what bound the claim. -->
 <!-- (1) NO animation at all: the base rule stands, untouched. -->
 <div id="n1" style="width: 20px"></div>
 <!-- (2) An animation NAME THAT HAS NO @keyframes RULE: also untouched — a missing rule must not
      be read as "animate to the initial value". -->
 <div id="n2" class="half" style="animation-name: nosuchrule; width: 20px"></div>
 <!-- (3) `animation-play-state: paused`: we never started it, so there is nothing to sample. -->
 <div id="n3" class="half" style="animation-name: grow; animation-play-state: paused; width: 20px"></div>
 <!-- (4) A POSITIVE delay that has not elapsed, with fill-mode:none — not animating yet. -->
 <div id="n4" style="animation-name: grow; animation-duration: 100s; animation-delay: 50s;
                     animation-fill-mode: none; width: 20px"></div>
 <!-- (5) …the SAME row with fill-mode:backwards, which DOES back-fill the first keyframe (0px).
      (4) and (5) differ in one keyword and must differ in their answer, or the fill-mode arm is
      not being consulted at all. -->
 <div id="n5" style="animation-name: grow; animation-duration: 100s; animation-delay: 50s;
                     animation-fill-mode: backwards; width: 20px"></div>
 <div id="out">-</div>
 <script>
 window.addEventListener('load', function(){
   var ids=['w1','w2','c1','v1','d1','ks','ks2','op50','op0','e1','n1','n2','n3','n4','n5'], r=[];
   for (var i=0;i<ids.length;i++){
     var e=document.getElementById(ids[i]), s=getComputedStyle(e), v;
     if (ids[i][0]==='c') { v=s.color; }
     else if (ids[i][0]==='v') { v=s.visibility; }
     else if (ids[i].indexOf('op')===0) { v=s.opacity; }
     else { v=e.getBoundingClientRect().width; }
     r.push(ids[i]+'='+v);
   }
   document.getElementById('out').textContent=r.join(' ');
 });
 </script></body></html>"##;

/// **One test, on purpose** — see `g_defer`.
#[test]
fn a_keyframes_animation_resolves_to_an_interpolated_value() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://anim.test/",
        &fonts,
        800.0,
    ));
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "#out must exist");
    let got = page.dom().text_content(hits[0]);
    println!("KEYFRAME-INTERPOLATION {got}");

    // RED, run: delete the `if cv.get_ui().specifies_animations()` block in
    // `stylo_engine::cascade_via_stylo_sized` — every animated row collapses onto its base rule
    // (`w1=20 w2=20 ks=20 ks2=20 n5=20`), `c1` reads the inherited black, `v1` reads `visible`,
    // and `op50` reads `1` from the reveal-hack. The CONTROLS do not move, which is what says the
    // gate is measuring the animation path and not the cascade in general.
    assert_eq!(
        got,
        "w1=50 w2=60 c1=rgb(50, 50, 50) v1=visible d1=100 ks=50 ks2=0 op50=0.5 op0=1 \
         e1=100 n1=20 n2=20 n3=50 n4=20 n5=0",
        "a `@keyframes` animation must resolve to the INTERPOLATED value at its current position. \
         `w1=50` is the core claim (half of 0→100). `w2=60` is the spec's fill-in — the `from` \
         keyframe declares no width, so the from-side is the element's own 20px base rule, and 60 \
         is a number NEITHER keyframe contains. `c1` proves a non-length type goes through \
         `Animate` too. `v1=visible` is the row that CORRECTED THE PREDICTION — \
         `visibility` is not plain-discrete, its interval is `visible` whenever either endpoint \
         is, and Chrome agrees. `d1=100` is the NON-INTERPOLABLE arm (`auto` ↔ `100px` has no \
         midpoint, so `Animate` refuses and the spec flips at 0.5) and is the only row that \
         reaches the `Err(())` handler at all. `ks2=0` is the discriminator for \
         the keyframe's OWN timing function — `steps(2, end)` at raw 0.25 outputs 0 where the \
         element's `linear` would give 25. `op0=1` pins the reveal-hack (`stylo_map.rs`), which is \
         a deliberate un-traded capability and NOT a bug this tick may delete. \
         ⚠⚠⚠ `n3=50` WAS `n3=20` AND THAT EXPECTATION WAS PINNING A BUG (corrected t1308/t1309): \
         `n3` is `animation-play-state: paused` on the `.half` row, i.e. `delay:-50s` of `100s`, so \
         its position is progress 0.5 and its width is 50. The sampler used to SKIP any paused \
         animation — filing it in the not-running space *\"we have no way to have started it\"* — \
         which conflates NOT ADVANCING with NOT EXISTING: pausing freezes the timeline, a frozen \
         animation still has the position its delay gives it, and the document clock is 0 for a \
         static render so nothing advances for a RUNNING animation either. The skip lost EVERY \
         animated property, not just width (the same declaration on `opacity` read the initial 1, \
         and on an auto-width box read 784px). So `paused` is NOT a control for the not-running \
         space and never was. CONTROLS, now three: `n1` no \
         animation, `n2` a name with no `@keyframes` rule, `n4` a delay that has not \
         elapsed under `fill-mode: none` — all three must read the base rule 20 — while `n5`, the \
         same row with `fill-mode: backwards`, must back-fill to 0. `n4` vs `n5` differ in one \
         keyword and must differ in their answer, or the fill-mode arm is not consulted at all"
    );
}
