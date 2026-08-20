//! **A running CSS TRANSITION resolved to an INTERPOLATED computed value.**
//!
//! ⚠⚠⚠ **Before this module the engine sampled no transition, anywhere.** `transition-*` cascaded
//! and was then thrown away: an element part-way through `transition: width .3s` computed — and
//! reported through `getComputedStyle` — its **after-change** value, as though the transition had
//! already finished. Sibling module [`crate::animation`] closed the same hole for `@keyframes` at
//! tick 1301; this is the other four legs of the same WPT harness.
//!
//! **What a transition needs that an animation does not: A MEMORY.**
//!
//! An animation's two endpoints are both written down in the stylesheet, so sampling one is a pure
//! function of the cascade. A transition's `from` endpoint is *the value this element had before the
//! style change* — it appears in no rule, no keyframe and no declaration. The only place it can come
//! from is **what the last cascade published**, which is why this module owns a per-node
//! before-change table and [`crate::animation`] owns no state at all.
//!
//! **Everything else is borrowed, per the ladder in `STATUS.md` — option 1, no fork.**
//!
//! | need | borrowed from |
//! |---|---|
//! | `transition-property`, with `all` expanded and `none` dropped | `ComputedValues::transition_properties` |
//! | is this property transitionable at all / discretely? | `LonghandId::is_animatable` / `is_discrete_animatable` |
//! | a computed value in animatable form | `AnimationValue::from_computed_values` |
//! | the interpolation itself, per property type | `Animate::animate(Procedure::Interpolate)` |
//! | back to a declaration the cascade can take | `AnimationValue::uncompute` |
//! | `cubic-bezier` / `steps` / `linear()` | `ComputedTimingFunction::calculate_output` |
//!
//! ⚠ **The per-index lists are read with `_mod`, not `_at`.** `transition-property: width, color`
//! with a single `transition-duration: 2s` is the common authoring shape, and CSS Transitions §2
//! says the shorter lists REPEAT. Stylo already ships that as `transition_duration_mod(i)`; reading
//! `_at(i)` would panic on the second property.
//!
//! ⚠⚠⚠ **THE GUARD — SAMPLE ONLY WHEN THE ELAPSED TIME IS GENUINELY POSITIVE — AND IT IS THE WHOLE
//! REAL-PAGE SAFETY ARGUMENT.** The document clock ([`crate::animation::time_ms`]) is **0**: nothing
//! in this engine advances it yet. So an ordinary `transition: width .3s` has
//! `elapsed = 0 - 0 = 0` and sits at progress **0** — which is its *start* value. A sampler without
//! this guard would render every hover, accordion, drawer and menu on the real web in the state it
//! was **leaving**, and would do it on the majority of the corpus. With the guard, a transition is
//! sampled only when its `transition-delay` is NEGATIVE, i.e. when the author has deliberately
//! placed it in the past — which is exactly the idiom WPT's
//! `css/support/interpolation-testcommon.js` is built on (`duration: 100s; delay: -50s`, a fixed
//! half-way sample warped by the timing function) and is essentially absent from real pages.
//!
//! This is not a workaround for a missing clock, it is what a clock at 0 *means*: a page that has
//! not been running has not been transitioning either. When the clock lands, the same expression
//! starts answering for real pages with no change here.

use std::cell::RefCell;
use std::collections::HashMap;

use manuk_dom::NodeId;
use stylo::properties::animated_properties::AnimationValue;
use stylo::properties::{
    ComputedValues, OwnedPropertyDeclarationId, PropertyDeclaration, PropertyDeclarationId,
};
use stylo::servo_arc::Arc as ServoArc;
use stylo::values::animated::{Animate, Procedure};
use stylo::values::generics::easing::BeforeFlag;

thread_local! {
    /// **THE SAMPLE MEMO — `(before, after, clock) → declarations`, one entry per element.**
    ///
    /// **WITHOUT IT [`sample`] IS QUADRATIC ON THE PAGE THAT EXERCISES IT MOST.**
    /// `transition-property: all` expands to every longhand, so a single element costs ~200
    /// `AnimationValue::from_computed_values` pairs — and WPT's `interpolation-testcommon.js`
    /// forces one full-document style recalc per target while N targets accumulate, which is
    /// `O(N² · 200)`. On `css/css-grid/animation/grid-template-rows-interpolation.html` the page
    /// overran the document's own wall-clock budget and was cut off at a different point on every
    /// run, emitting **558, 571, 642 and 654 subtests across four runs of the SAME binary**.
    ///
    /// ⚠⚠⚠ **THIS IS A COST REDUCTION AND IT IS NOT A DETERMINISM FIX — THAT WAS MEASURED, AND THE
    /// FIRST DRAFT OF THIS COMMENT CLAIMED OTHERWISE.** With the memo installed,
    /// `css/css-grid/animation` still emitted **1841 then 1766 subtests on two runs of the one
    /// release binary** (t1312). So the sampler was *a* cost in that budget, not *the* cost, and
    /// whatever else overruns it is still unattributed. Do not cite this memo as the reason a
    /// `css/css-grid` reading is trustworthy: that area's denominator still moves, and the rule
    /// from t1311 stands — difference only against a baseline you took yourself, on the same
    /// binary, in the same hour, and prefer an area whose denominator does not move at all.
    ///
    /// [`sample`] is a pure function of `(before, after, clock)`, so it memoises exactly. `before`
    /// is compared by POINTER (it is the very `Arc` [`PREV`] handed out, kept unchanged while the
    /// transition runs) and `after` by VALUE across ALL TWENTY style structs — ~350 field compares
    /// with no allocation, against ~400 animated-value constructions with plenty. A settled
    /// transition therefore costs one comparison per recalc instead of one full sample.
    ///
    /// ⚠ The clock is part of the key and not an afterthought: the whole point of a transition is
    /// that its value changes with time, so a memo that ignored the clock would freeze every
    /// transition at the first frame the moment a clock exists.
    #[allow(clippy::type_complexity)]
    static MEMO: RefCell<HashMap<NodeId, (ServoArc<ComputedValues>, ServoArc<ComputedValues>, f64, Vec<PropertyDeclaration>)>> =
        RefCell::new(HashMap::new());

    /// **The BEFORE-CHANGE style of every element the last cascade published**, which is the one
    /// endpoint of a transition that exists nowhere in the document.
    ///
    /// **A thread-local rather than state on the caller, for the same reason the animation clock is
    /// one:** `cascade_via_stylo_sized` is a free function called from six places and none of them
    /// owns a style history. It is REPLACED wholesale at the end of each pass
    /// ([`publish_pass`]) rather than merged, so a node the document no longer contains stops
    /// being remembered in the same instant it stops being cascaded — a merging table would grow
    /// for the life of the process on any page that churns its DOM.
    ///
    /// ⚠ Keyed by `NodeId` alone, so two documents cascaded on the SAME thread share the key
    /// space. The only thing a stale entry can do is offer a wrong `from` endpoint, and that is
    /// reachable only through a NEGATIVE `transition-delay` (see the guard in the module docs), so
    /// it is recorded here rather than paid for with a document generation counter.
    static PREV: RefCell<HashMap<NodeId, ServoArc<ComputedValues>>> =
        RefCell::new(HashMap::new());
}

/// The style this element had when the last cascade finished, if it had one.
pub fn prev_style(node: NodeId) -> Option<ServoArc<ComputedValues>> {
    PREV.with(|p| p.borrow().get(&node).cloned())
}

/// Install the table the pass just built. See [`PREV`] — replacement, not merge.
///
/// The sample memo is pruned to the same key set here, for the same reason and in the same instant:
/// a node the document no longer contains must stop being remembered by BOTH tables, or the memo
/// becomes the leak the `PREV` note was written to avoid.
pub fn publish_pass(next: HashMap<NodeId, ServoArc<ComputedValues>>) {
    MEMO.with(|m| m.borrow_mut().retain(|k, _| next.contains_key(k)));
    PREV.with(|p| *p.borrow_mut() = next);
}

/// Do these two styles agree on every longhand? **Twenty struct comparisons — every style struct
/// `ComputedValues` has, and the exhaustiveness is the whole correctness argument**, not a tidiness
/// preference. See [`MEMO`] for why this is worth doing instead of the animated-value pass it
/// replaces.
///
/// ⚠⚠⚠ **A STRUCT LEFT OUT OF THIS LIST IS A STALE SAMPLE, NOT A MISSED OPTIMISATION.** The memo
/// returns the PREVIOUS declaration list whenever this says *"same"*, so a struct nobody compares is
/// a struct whose change the transition never sees: the target moves and the interpolation keeps
/// running to the old endpoint. This function's first draft omitted `get_svg()`, which is
/// fill/stroke/stroke-width — animatable, and the whole colour surface of an inline icon.
///
/// ⚠ **The list is derived from `properties.rs`'s generated `pub fn get_*(&self)` accessors, and
/// two of those are NOT style structs**: `get_system()` reports whether a system font set the value,
/// and `get_inset(PhysicalSide)` is a per-side longhand getter — `top`/`right`/`bottom`/`left` live
/// in `Position`, which is already here. Re-derive this list whenever Stylo is bumped; a new struct
/// is silent.
///
/// ⚠ **By VALUE, not by `Arc` pointer.** `before` and `after` come from two different cascade
/// passes, so even a completely unchanged element gets freshly allocated style structs and pointer
/// equality would report *"changed"* every single time — i.e. it would compile, run, and memoise
/// nothing.
fn same_style(a: &ComputedValues, b: &ComputedValues) -> bool {
    a.get_background() == b.get_background()
        && a.get_border() == b.get_border()
        && a.get_box() == b.get_box()
        && a.get_column() == b.get_column()
        && a.get_counters() == b.get_counters()
        && a.get_effects() == b.get_effects()
        && a.get_font() == b.get_font()
        && a.get_inherited_box() == b.get_inherited_box()
        && a.get_inherited_table() == b.get_inherited_table()
        && a.get_inherited_text() == b.get_inherited_text()
        && a.get_inherited_ui() == b.get_inherited_ui()
        && a.get_list() == b.get_list()
        && a.get_margin() == b.get_margin()
        && a.get_outline() == b.get_outline()
        && a.get_padding() == b.get_padding()
        && a.get_position() == b.get_position()
        && a.get_table() == b.get_table()
        && a.get_text() == b.get_text()
        && a.get_ui() == b.get_ui()
        // ⚠ `svg` — fill/stroke/stroke-width/stroke-dasharray, all animatable — was the one
        // struct missing from the first draft of this list.
        && a.get_svg() == b.get_svg()
}

/// **How many times [`sample`] has actually run its animated-value loop.**
///
/// ⚠ This exists to be READ, by `g_transition_sample_is_not_paid_by_every_element` — the cost this
/// module's early-out and memo remove is invisible in any output the engine produces, so the only
/// falsifiable statement about it is a COUNT. A wall-clock assertion would be the alternative and it
/// is not one: the same synthetic probe that measured this fix moved its own untouched control arm
/// by 54% between two runs of one binary. A count does not move.
///
/// One relaxed atomic increment on a path that constructs hundreds of allocating values.
pub static SAMPLES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Read [`SAMPLES`].
pub fn samples_taken() -> u64 {
    SAMPLES.load(std::sync::atomic::Ordering::Relaxed)
}

/// [`sample`], memoised on its own inputs. See [`MEMO`].
pub fn sample_memoized(
    node: NodeId,
    before: &ServoArc<ComputedValues>,
    after: &ServoArc<ComputedValues>,
) -> Vec<PropertyDeclaration> {
    // ⭐⭐⭐ **AN ELEMENT WHOSE STYLE DID NOT CHANGE CANNOT BE TRANSITIONING, AND THIS IS THE ONLY
    // CHECK THAT COSTS NOTHING TO MAKE.**
    //
    // [`sample`] already skips every property whose two animated values compare equal — but it
    // discovers that by CONSTRUCTING both of them, and under `transition-property: all` it does so
    // for every animatable longhand: ~400 `AnimationValue::from_computed_values` calls, each
    // allocating, to conclude that nothing moved. If all twenty style structs are equal then every
    // longhand's computed value is equal, so every one of those comparisons is `a == b` and the
    // result is provably the empty vector. [`same_style`] reaches the same answer in ~350 field
    // compares with no allocation at all.
    //
    // ⚠⚠⚠ **AND THE MEMO BELOW CANNOT COVER THIS CASE — IT IS THE CASE THE MEMO STRUCTURALLY
    // MISSES.** The memo keys `before` by POINTER, and that pointer is only stable while a
    // transition is actually running: `cascade_via_stylo_sized` re-publishes a FRESH `Arc` as the
    // next pass's `before` for every element whose sample came back EMPTY. So the elements that
    // most need memoising — the ones that answer "nothing is transitioning" over and over — are
    // exactly the ones whose key is invalidated on every pass, and they paid the full ~400
    // constructions on every cascade forever. Measured (debug build, 80 targets carrying
    // `transition-property: all` with a negative delay, one `getComputedStyle` per target):
    // **36.24 ms/target with `all` against 16.31 ms/target with no transition at all, and the gap
    // GREW with the target count** — the signature of a per-cascade cost paid by every element.
    if same_style(before, after) {
        return Vec::new();
    }
    let now = crate::animation::time_ms();
    let hit = MEMO.with(|m| {
        m.borrow().get(&node).and_then(|(b, a, t, d)| {
            (ServoArc::ptr_eq(b, before) && *t == now && same_style(a, after)).then(|| d.clone())
        })
    });
    if let Some(d) = hit {
        return d;
    }
    let out = sample(before, after);
    MEMO.with(|m| {
        m.borrow_mut()
            .insert(node, (before.clone(), after.clone(), now, out.clone()))
    });
    out
}

/// Is there any transition worth looking at on this style? The cheap gate the caller pays on every
/// element of every page, before any of the work below.
///
/// ⚠ **Deliberately NOT Stylo's own `specifies_transitions()`**, which asks whether
/// `duration.max(0) + delay > 0` — true for the ordinary `transition: width .3s` that this engine
/// can never sample while the clock is 0. This asks the question the sampler actually acts on: is
/// any delay negative, i.e. has any transition been placed in the past?
pub fn may_be_running(cv: &ComputedValues) -> bool {
    let ui = cv.get_ui();
    (0..ui.transition_property_count()).any(|i| {
        ui.transition_duration_mod(i).seconds() > 0. && ui.transition_delay_mod(i).seconds() < 0.
    })
}

/// **Sample every running transition on this element.**
///
/// `before` is the style the last cascade published; `after` is this cascade's answer *without*
/// transitions (but WITH animations — CSS Cascade 5 puts the transition origin above the animation
/// one, so a transition interpolates towards the animated value, not past it). The returned
/// declarations are mixed on top by the caller.
///
/// An empty result means **nothing is running**, and the caller uses that to decide whether the
/// before-change value may be replaced — see the note in `g_transition_interpolation`.
pub fn sample(before: &ComputedValues, after: &ComputedValues) -> Vec<PropertyDeclaration> {
    SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let now = crate::animation::time_ms();
    let ui = after.get_ui();
    let mut out = Vec::new();
    // `transition_properties()` is Stylo's own expansion: `all` becomes every longhand of the `all`
    // shorthand, a shorthand becomes its longhands, and `none` yields nothing. Writing that by hand
    // is where a hand-rolled transition engine drifts from the cascade it is supposed to shadow.
    for it in after.transition_properties() {
        // A custom property transitions as its `@property` syntax says, and this engine does not
        // consult `@property` yet (the same omission [`crate::animation`] names for keyframes). A
        // registered `--x` therefore keeps its after-change value instead of getting a wrong one.
        let OwnedPropertyDeclarationId::Longhand(id) = it.property else {
            continue;
        };
        // Not animatable ⇒ never transitions, whatever the behavior keyword says. This is also what
        // keeps `transition-property: all` cheap and correct: `all` expands to every longhand
        // including `transition-duration` itself, and the non-animatable ones fall out here.
        if !id.is_animatable() {
            continue;
        }
        let dur = ui.transition_duration_mod(it.index).seconds() as f64 * 1000.0;
        if !(dur > 0.0) {
            continue;
        }
        let delay = ui.transition_delay_mod(it.index).seconds() as f64 * 1000.0;
        let elapsed = now - delay;
        // ⚠⚠⚠ THE GUARD (module docs). `elapsed == 0` is NOT running — see `n1` in
        // `g_transition_interpolation`, which is the shape every real page's transition has.
        if !(elapsed > 0.0) {
            continue;
        }
        // Past the end: the transition is over and the after-change value stands. There is no
        // `fill-mode` on a transition to hold the last frame with.
        if elapsed >= dur {
            continue;
        }
        // **A discrete property does not transition at all unless the author asked for it.** CSS
        // Transitions 2 added `transition-behavior: allow-discrete` precisely because the default
        // is a jump — and WPT asserts the difference as a flip point of `-Infinity` (always the
        // after-change value) versus `0.5`.
        let behavior_normal = ui.transition_behavior_mod(it.index).is_normal();
        if id.is_discrete_animatable() && behavior_normal {
            continue;
        }
        let pid = PropertyDeclarationId::Longhand(id);
        let (Some(a), Some(b)) = (
            AnimationValue::from_computed_values(pid, before),
            AnimationValue::from_computed_values(pid, after),
        ) else {
            continue;
        };
        // Nothing changed ⇒ no transition for this property. Load-bearing under `all`, which would
        // otherwise re-declare ~200 unchanged longhands per element and make the caller re-cascade
        // the entire style for nothing.
        if a == b {
            continue;
        }
        let tf = ui.transition_timing_function_mod(it.index);
        let progress = tf.calculate_output(elapsed / dur, BeforeFlag::Unset, 1e-7);
        match a.animate(&b, Procedure::Interpolate { progress }) {
            Ok(v) if !crate::animation::is_unresolvable(&v) => out.push(v.uncompute()),
            // ⚠⚠⚠ **DISCRETENESS IS A PROPERTY OF THE VALUE PAIR, NOT ONLY OF THE PROPERTY ID.**
            // `transform` is a continuous property, and yet
            // `matrix3d(2,0,0,0, 0,2,0,0, 0,0,0,0, 0,0,0,1)` → `matrix(3,0,0,3,0,0)` has no
            // midpoint at all: the from-matrix is singular, so the spec makes THAT PAIR discrete.
            // The same is true of any two transform lists whose functions do not match
            // ([`crate::animation::is_unresolvable`]).
            //
            // So the behavior keyword has to be consulted a second time, here, on the pair — and
            // this is not a detail: without it the four transition legs of WPT's
            // `non-invertible-matrix-interpolation` answer an interpolated value where Chrome
            // jumps, and it is worth **22 subtests in `css/css-transforms` alone**, which is how
            // this arm was found (it was a measured REGRESSION against the tick's own control run,
            // not a reading of the spec).
            _ => {
                if !behavior_normal {
                    out.push(if progress < 0.5 { a } else { b }.uncompute());
                }
            }
        }
    }
    out
}
