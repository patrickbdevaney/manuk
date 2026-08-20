//! **A `@keyframes` animation resolved to an INTERPOLATED computed value, at the element's own
//! animation time.**
//!
//! ⚠⚠⚠ **Before this module the engine did not interpolate anything, anywhere.** `@keyframes` were
//! parsed (Stylo builds a `KeyframesAnimation` per name and hands it to `Stylist::lookup_keyframes`),
//! `animation-*` cascaded, and then **every one of those values was thrown away**: the only thing the
//! engine did with an animation was the `has_animation && opacity == 0 -> 1` reveal-hack in
//! `stylo_map.rs`, whose own comment says *"We cannot animate."* An element half-way through
//! `@keyframes fade { from { opacity: 0 } to { opacity: 1 } }` rendered its **base rule**, and
//! `getComputedStyle` reported the base rule too.
//!
//! **What that cost, measured.** WPT's `css/support/interpolation-testcommon.js` backs **194
//! `*-interpolation.html` files across twelve CSS areas**, and it runs *five* legs over every
//! expectation — CSS Animations, three CSS Transitions variants, and Web Animations. In
//! `css/css-transforms` alone the CSS-Animations leg is **764 failing subtests**, and the same leg is
//! the largest single mechanism under `css/css-grid/animation` and `css/css-values`.
//!
//! ⭐ **The harness never lets time pass, and that is what makes this tick-sized rather than a
//! subsystem.** Every leg sets `animation-duration: 100s; animation-delay: -50s` and warps the timing
//! function so that the fixed sample at 50% eases to the tested progress. So the value under test is a
//! **static** function of the cascade — no compositor, no frame loop, no rAF. This module is that
//! function, and the animation *clock* it reads ([`set_time_ms`]) is a separate, later concern.
//!
//! **Everything numeric here is borrowed, per the ladder in `STATUS.md` — option 1, no fork.**
//!
//! | need | borrowed from |
//! |---|---|
//! | `@keyframes` → ordered steps + the set of properties that change | `Stylist::lookup_keyframes` → `KeyframesAnimation` |
//! | a computed value in animatable form | `AnimationValue::from_computed_values` |
//! | the interpolation itself, per property type | `Animate::animate(Procedure::Interpolate)` |
//! | back to a declaration the cascade can take | `AnimationValue::uncompute` |
//! | `cubic-bezier` / `steps` / `linear()` | `ComputedTimingFunction::calculate_output` |
//!
//! ⚠ **The two bracketing keyframes are reached by CASCADING, not by reading the keyframe block.** A
//! keyframe declares *specified* values (`width: 50%`, `color: currentcolor`), and interpolation is
//! defined on *computed* ones. So each side is produced by re-running the element's own cascade with
//! that keyframe's declaration block appended at the top — which also gives the spec's fill-in for
//! free: a property the `0%` keyframe does not mention keeps the element's underlying value, because
//! that cascade still contains every rule that produced it.

use stylo::properties::animated_properties::AnimationValue;
use stylo::properties::{ComputedValues, LonghandId, PropertyDeclaration, PropertyDeclarationId};
use stylo::stylesheets::keyframes_rule::{KeyframesAnimation, KeyframesStepValue};
use stylo::values::animated::{Animate, Procedure};
use stylo::values::computed::easing::ComputedTimingFunction;
use stylo::values::generics::easing::BeforeFlag;

// The document's animation clock, in milliseconds, for the cascade about to run.
//
// **A thread-local rather than a parameter, deliberately.** `cascade_via_stylo_sized` is called from
// six places and none of them owns a clock; threading `f64` through all of them would be a wide,
// meaningless diff whose only real content is this one value. A static render leaves it at **0**,
// which is the honest answer for a page that has not been running.
//
// ⚠ It is read ONCE per cascade, not once per element — an animation must not see time move while a
// single style pass walks the tree, or a parent and its child sample different frames.
thread_local! {
    static TIME_MS: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// Set the animation clock the next cascade will sample. Milliseconds since the document began.
pub fn set_time_ms(ms: f64) {
    TIME_MS.with(|t| t.set(ms));
}

/// The animation clock the current cascade is sampling.
pub fn time_ms() -> f64 {
    TIME_MS.with(|t| t.get())
}

/// One animation's sample: the two keyframe blocks that bracket the current time, and how far
/// between them the element sits after its timing function has been applied.
pub struct Sample {
    /// Declarations of the keyframe at or below the current position.
    pub from: Vec<PropertyDeclaration>,
    /// Declarations of the keyframe at or above it.
    pub to: Vec<PropertyDeclaration>,
    /// RAW progress in `[0, 1]` between the two keyframes, before any timing function.
    ///
    /// ⚠ **Un-eased on purpose.** The timing function that governs this segment may be declared by
    /// the FROM keyframe itself, and a keyframe declares a *specified* value — computing it needs
    /// the cascade the caller has not run yet. So easing is applied in [`interpolate`], which holds
    /// the cascaded `from` style and can simply read the computed answer out of it.
    pub progress: f64,
    /// The element's own `animation-timing-function`, used unless the from-keyframe declared one.
    pub easing: ComputedTimingFunction,
    /// Did the from-keyframe declare `animation-timing-function`? CSS Animations §3 makes a
    /// keyframe's own function govern the segment that *starts* at it.
    pub from_declares_easing: bool,
    /// Every property this whole animation touches, so the caller knows which longhands to
    /// interpolate rather than diffing two full `ComputedValues`.
    ///
    /// ⚠ **Longhands only, and the omission is named rather than silent.** The set Stylo hands back
    /// also carries custom-property names; animating a registered `--x` needs its `@property`
    /// syntax to know what type it interpolates as, which this engine does not yet consult. A
    /// custom property therefore keeps its underlying value instead of getting a wrong one.
    pub properties: Vec<LonghandId>,
    /// ⚠⚠⚠ **HOW EACH ENDPOINT COMBINES WITH THE UNDERLYING VALUE — `animation-composition`.**
    ///
    /// `(mode, declared-longhands)` per side. Until tick 1287 every keyframe **replaced**, because
    /// each endpoint is produced by re-running the element's cascade with the keyframe's block
    /// appended, and appending IS replacement. `composite: add` says the keyframe's value is added
    /// to the underlying one instead — so `underlying [50px] from add [100px]` is `150px`, and we
    /// answered `100px`: the interpolation exactly right and every value short by the underlying.
    ///
    /// ⚠ **The declared set is carried, not just the mode, and it is load-bearing.**
    /// [`Self::properties`] is the union over ALL keyframes, and an endpoint that does not mention a
    /// property already carries the underlying value there — compositing it would add underlying to
    /// underlying and **silently double it**. Each side may only composite what it actually declared.
    pub from_composite: (Composite, Vec<LonghandId>),
    pub to_composite: (Composite, Vec<LonghandId>),
}

/// `animation-composition`, reduced to the three operations Stylo's `Procedure` already implements.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Composite {
    #[default]
    Replace,
    Add,
    Accumulate,
}

/// **Where in its own timeline is this animation, right now?**
///
/// Returns `None` — meaning *this element is not animating, cascade it normally* — for the whole of
/// the not-running space: no duration, a delay that has not elapsed under a `fill-mode` that does not
/// back-fill, and a finished animation whose `fill-mode` does not forward-fill.
///
/// ⚠ **`animation-play-state: paused` is NOT in that space, and used to be (t1308).** The old note read
/// *"we have no way to have started it"*, which confuses *not advancing* with *not existing*. A paused
/// animation is frozen at the position its delay gives it, and since the document clock is 0 for a
/// static render nothing advances for a RUNNING animation either — so `paused` changes nothing about
/// the value, and skipping it cascaded the element as though it had no animation at all.
///
/// ⚠ **`direction` and `iteration-count` are applied here and not by the caller**, because the
/// alternate directions are a transform on the *iteration* progress and the caller only ever sees the
/// result. `alternate` on an odd iteration is `1 - p`; that is the whole of it.
fn iteration_progress(
    now_ms: f64,
    duration_s: f32,
    delay_s: f32,
    iteration_count: f32,
    alternate: bool,
    alternate_reverse: bool,
    reverse: bool,
    fill_backwards: bool,
    fill_forwards: bool,
) -> Option<f64> {
    let dur = duration_s as f64 * 1000.0;
    if !(dur > 0.0) {
        return None;
    }
    let elapsed = now_ms - delay_s as f64 * 1000.0;
    let total = if iteration_count.is_infinite() {
        f64::INFINITY
    } else {
        dur * iteration_count as f64
    };
    if total <= 0.0 {
        return None;
    }
    // BEFORE the delay has elapsed. `fill-mode: backwards`/`both` says to hold the first frame; any
    // other fill mode means the element is simply not animating yet.
    if elapsed < 0.0 {
        if !fill_backwards {
            return None;
        }
        return Some(direction_of(0.0, 0, alternate, alternate_reverse, reverse));
    }
    // AFTER the last iteration. `forwards`/`both` holds the last frame — and the iteration index is
    // the LAST one, not the one past the end, or an `alternate` animation would hold the wrong end.
    if elapsed >= total {
        if !fill_forwards {
            return None;
        }
        let last = if iteration_count.is_infinite() {
            0
        } else {
            (iteration_count.ceil() as u64).saturating_sub(1)
        };
        let tail = if iteration_count.fract() == 0.0 {
            1.0
        } else {
            iteration_count.fract() as f64
        };
        return Some(direction_of(
            tail,
            last,
            alternate,
            alternate_reverse,
            reverse,
        ));
    }
    let iteration = (elapsed / dur).floor();
    let p = (elapsed / dur) - iteration;
    Some(direction_of(
        p,
        iteration as u64,
        alternate,
        alternate_reverse,
        reverse,
    ))
}

/// `animation-direction` as a transform on one iteration's progress (CSS Animations §4.2).
fn direction_of(p: f64, iteration: u64, alternate: bool, alt_reverse: bool, reverse: bool) -> f64 {
    let flip = if alternate {
        iteration % 2 == 1
    } else if alt_reverse {
        iteration % 2 == 0
    } else {
        reverse
    };
    if flip {
        1.0 - p
    } else {
        p
    }
}

/// Locate the pair of keyframes bracketing `p`, and ease between them.
///
/// ⚠ **The timing function applied here is the one on the FROM keyframe when it declares one**, not
/// the element's — CSS Animations §3 makes a keyframe's own `animation-timing-function` govern the
/// segment that *starts* at it. The harness leans on this: `createEasing` puts the warp on the
/// element, and a keyframe that overrides it must win.
fn bracket(
    anim: &KeyframesAnimation,
    guard: &stylo::shared_lock::SharedRwLockReadGuard<'_>,
    p: f64,
    element_easing: &ComputedTimingFunction,
) -> Option<Sample> {
    // Percentage-keyed steps only. `steps_with_range_name` belongs to scroll-driven animations, which
    // have no clock here at all, and silently treating a range-named step as a percentage one would
    // place it at a position its author never wrote.
    let steps = &anim.steps;
    if steps.len() < 2 {
        return None;
    }
    let pct = |i: usize| steps[i].start_offset.percentage.0 as f64;
    let mut lo = 0usize;
    for i in 0..steps.len() {
        if pct(i) <= p {
            lo = i;
        }
    }
    let hi = (lo + 1).min(steps.len() - 1);
    let (a, b) = (pct(lo), pct(hi));
    let local = if hi == lo || (b - a).abs() < f64::EPSILON {
        0.0
    } else {
        ((p - a) / (b - a)).clamp(0.0, 1.0)
    };

    let decls = |i: usize| -> Vec<PropertyDeclaration> {
        match steps[i].value {
            KeyframesStepValue::Declarations { ref block } => block
                .read_with(guard)
                .declarations()
                .iter()
                .cloned()
                .collect(),
            // A synthesised step IS the element's underlying style, and the underlying style is
            // exactly what the re-cascade produces when no keyframe declarations are appended.
            KeyframesStepValue::ComputedValues => Vec::new(),
        }
    };

    // `animation-composition` is declared INSIDE the keyframe block (that is where WPT's harness
    // puts it, and where CSS Animations 2 allows it), so it comes out of the same declaration list
    // the endpoint is built from — no second lookup, and no way for the two to disagree.
    let composite = |d: &[PropertyDeclaration]| -> (Composite, Vec<LonghandId>) {
        let mut mode = Composite::Replace;
        let mut declared = Vec::new();
        for decl in d {
            if let PropertyDeclaration::AnimationComposition(ref v) = *decl {
                if let Some(first) = v.0.first() {
                    mode = match *first {
                        stylo::values::specified::AnimationComposition::Add => Composite::Add,
                        stylo::values::specified::AnimationComposition::Accumulate => {
                            Composite::Accumulate
                        }
                        _ => Composite::Replace,
                    };
                }
                continue;
            }
            if let PropertyDeclarationId::Longhand(l) = decl.id() {
                declared.push(l);
            }
        }
        (mode, declared)
    };
    let (from_decls, to_decls) = (decls(lo), decls(hi));
    let from_composite = composite(&from_decls);
    let to_composite = composite(&to_decls);

    Some(Sample {
        from: from_decls,
        to: to_decls,
        progress: local,
        easing: element_easing.clone(),
        from_declares_easing: steps[lo].declared_timing_function,
        properties: anim
            .properties_changed
            .iter()
            .filter_map(|id| match id {
                PropertyDeclarationId::Longhand(l) => Some(l),
                PropertyDeclarationId::Custom(_) => None,
            })
            .collect(),
        from_composite,
        to_composite,
    })
}

/// **Sample every `animation-name` on this element, in cascade order.**
///
/// The returned samples are applied by the caller **in order**, so a later `animation-name` in the
/// list overrides an earlier one on the properties they share — which is what CSS Animations §3 says
/// (*"the last animation in the list wins"*) and is why this returns a `Vec` rather than one sample.
pub fn samples_for<'a, E>(
    stylist: &'a stylo::stylist::Stylist,
    guard: &stylo::shared_lock::SharedRwLockReadGuard<'_>,
    element: E,
    cv: &ComputedValues,
) -> Vec<Sample>
where
    E: stylo::dom::TElement + 'a,
{
    use stylo::values::computed::{AnimationDirection, AnimationFillMode};

    let ui = cv.get_ui();
    let n = ui.animation_name_count();
    let mut out = Vec::new();
    let now = time_ms();
    for i in 0..n {
        let name = ui.animation_name_at(i);
        let Some(atom) = name.as_atom() else { continue };
        // ⚠⚠⚠ **A PAUSED ANIMATION IS NOT AN ABSENT ONE (t1308).** This used to `continue`, on the
        // reasoning quoted in `iteration_progress`'s doc — *"we have no way to have started it"*. That
        // conflates *not advancing* with *not existing*: `animation-play-state: paused` freezes the
        // timeline, it does not delete the animation, and a frozen animation still HAS a position —
        // the one its `animation-delay` gives it. Our clock sits at 0 for a static render, so nothing
        // advances for a running animation either, and `paused` therefore changes **nothing** about
        // the value to compute.
        //
        // What skipping cost was not subtle: the element cascaded as if it had no animation at all,
        // so `animation: k 100s -50s linear paused forwards` read `opacity: 1` (the initial value)
        // instead of 0.6, and the same rule on `width` read **784px** — `width: auto` filling the
        // container — instead of 70px. **Every property was wrong, which is what distinguishes this
        // from t1307**: a value wrong in ONE property names a special case, and a value wrong in ALL
        // of them names the shared path. Here it is the shared path.
        //
        // `pause-on-hover` marquees, paused-by-default spinners, and CSS-driven scrubbers are all
        // this declaration.
        let Some(anim) = stylist.lookup_keyframes(atom, element) else {
            continue;
        };
        let dir = ui.animation_direction_at(i);
        let fill = ui.animation_fill_mode_at(i);
        let Some(p) = iteration_progress(
            now,
            ui.animation_duration_at(i).seconds(),
            ui.animation_delay_at(i).seconds(),
            ui.animation_iteration_count_at(i).0,
            dir == AnimationDirection::Alternate,
            dir == AnimationDirection::AlternateReverse,
            dir == AnimationDirection::Reverse,
            matches!(fill, AnimationFillMode::Backwards | AnimationFillMode::Both),
            matches!(fill, AnimationFillMode::Forwards | AnimationFillMode::Both),
        ) else {
            continue;
        };
        let easing = ui.animation_timing_function_at(i);
        if let Some(s) = bracket(anim, guard, p, &easing) {
            out.push(s);
        }
    }
    out
}

/// **Did `Animate` hand back an answer this engine cannot actually resolve?**
///
/// ⚠⚠⚠ **`Ok` from `animate` is not the same thing as an interpolated value.** When two
/// `transform` lists do not match function-for-function, CSS Transforms 1 says to interpolate them
/// as matrices — and Stylo's answer is to return `Ok` carrying a **deferred**
/// `InterpolateMatrix { from_list, to_list, progress }` for the consumer to resolve against the
/// reference box. Its servo build never resolves it: `generics/transform.rs` answers
/// `Transform3D::identity()` with a `TODO`, and our own `stylo_map.rs` drops the operation on its
/// `_ => {}` arm. So the value that reaches the page is neither endpoint and neither a midpoint —
/// it is **no transform at all**, which is the largest error the property can produce.
///
/// Until that resolution is built, a pair that lands here is treated as **discrete**: one of the
/// two real endpoints instead of the identity. Strictly better at every progress, and at 0 and 1
/// exactly right.
pub fn is_unresolvable(v: &AnimationValue) -> bool {
    use stylo::values::generics::transform::TransformOperation as TOp;
    match v {
        AnimationValue::Transform(t) => t.0.iter().any(|op| {
            matches!(
                op,
                TOp::InterpolateMatrix { .. } | TOp::AccumulateMatrix { .. }
            )
        }),
        _ => false,
    }
}

/// **Interpolate one sample's properties between two fully-cascaded styles.**
///
/// `from`/`to` are the element's own cascade re-run with each bracketing keyframe's declarations
/// appended, so a property the keyframe never mentions already carries the underlying value on BOTH
/// sides and interpolating it is a no-op that costs one `animate` call. That is the point: the
/// spec's fill-in is the cascade, not a special case here.
///
/// ⚠ **A property that refuses to interpolate is not an error and must not be skipped.** `Animate`
/// returns `Err` for discretely-animated values (`display`, `visibility`, a keyword), and the spec's
/// answer there is a **flip at 50%**, not "leave the base value" — so the failure is turned into the
/// flip rather than dropped, which is exactly what the harness's `expectFlip` asserts.
pub fn interpolate(
    sample: &Sample,
    from: &ComputedValues,
    to: &ComputedValues,
    underlying: &ComputedValues,
) -> Vec<PropertyDeclaration> {
    // The segment's timing function, resolved the only way a keyframe's specified value can be:
    // read it back COMPUTED out of the from-side cascade, which already contains that keyframe's
    // declarations. When the keyframe declared none, the element's own function stands.
    let tf = if sample.from_declares_easing {
        from.get_ui().animation_timing_function_at(0)
    } else {
        sample.easing.clone()
    };
    let progress = tf.calculate_output(sample.progress, BeforeFlag::Unset, 1e-7);

    let mut out = Vec::new();
    for id in &sample.properties {
        let pid = PropertyDeclarationId::Longhand(*id);
        let (Some(a), Some(b)) = (
            AnimationValue::from_computed_values(pid, from),
            AnimationValue::from_computed_values(pid, to),
        ) else {
            continue;
        };
        // ── `animation-composition`: combine each endpoint with the UNDERLYING value first.
        //
        // ⚠⚠⚠ **ONLY WHERE THAT ENDPOINT ACTUALLY DECLARED THE PROPERTY.** `sample.properties` is
        // the union over every keyframe, and an endpoint that does not mention a property already
        // carries the underlying value on that side (the fill-in IS the re-cascade) — compositing it
        // would add underlying to underlying and silently DOUBLE it. That is the one way this can be
        // worse than the bug it fixes, so the declared set is checked, not the mode alone.
        //
        // `Err` from `animate` means the pair does not compose (a discrete or mismatched type); the
        // spec's answer there is that the composite is a no-op — the endpoint stands as declared —
        // which is what `unwrap_or` gives, and it is why this cannot turn a working property into a
        // broken one.
        let composed = |side: &(Composite, Vec<LonghandId>), v: AnimationValue| -> AnimationValue {
            if side.0 == Composite::Replace || !side.1.contains(id) {
                return v;
            }
            let Some(u) = AnimationValue::from_computed_values(pid, underlying) else {
                return v;
            };
            let proc = match side.0 {
                Composite::Add => Procedure::Add,
                // One iteration's worth: this engine has no iteration counter on the sample, and
                // `accumulate` over a single pass is `add` for every type Stylo implements it for.
                Composite::Accumulate => Procedure::Accumulate { count: 1 },
                Composite::Replace => return v,
            };
            u.animate(&v, proc).unwrap_or(v)
        };
        let a = composed(&sample.from_composite, a);
        let b = composed(&sample.to_composite, b);
        let v = match a.animate(&b, Procedure::Interpolate { progress }) {
            // ⚠ `Ok` IS NOT ALWAYS AN ANSWER — see [`is_unresolvable`]. A mismatched pair of
            // transform lists comes back as a *deferred* `InterpolateMatrix`, which this engine
            // renders as the IDENTITY matrix: an element that should have been half-way between
            // two transforms sits completely untransformed. Routed into the discrete arm, it is at
            // least one of the two real endpoints.
            Ok(v) if !is_unresolvable(&v) => v,
            // Discrete: the value flips at the half-way point of the segment.
            _ => {
                if progress < 0.5 {
                    a
                } else {
                    b
                }
            }
        };
        out.push(v.uncompute());
    }
    out
}
