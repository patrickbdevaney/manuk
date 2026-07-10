//! Stylo-backed [`StyleEngine`], compiled only under `--features stylo`.
//!
//! CLAUDE.md's reuse target for CSS is Stylo (Servo/Firefox's production engine).
//! Fully driving Stylo's cascade — building its `Device`, `Stylist`, author
//! `CascadeData`, and mapping its `ComputedValues` back onto [`crate::ComputedStyle`]
//! — is a substantial integration and is the follow-on work behind this boundary.
//!
//! For now this adapter *links* Stylo (proving the dependency builds and the
//! feature/trait wiring is correct) and delegates to [`MinimalCascade`] so behavior
//! is well-defined. Replacing the delegation body with a real Stylist run is a
//! change contained entirely to this file — no caller sees the difference.
//!
//! D2 Step-0 (see [`crate::stylo_probe`]) has already proven the *non-DOM half* of
//! that run works here — building a `Device`, parsing with Stylo's own parser, and
//! compiling selectors through a `Stylist`. The `selectors::Element` wall (30 methods)
//! is landed and tested (see [`crate::stylo_dom`]). What still blocks stopping the
//! delegation, confirmed against the on-disk `stylo-0.19.0` source:
//!
//! 1. **The `TElement` type requirement.** Both cascade entry points
//!    (`Stylist::compute_for_declarations` and `properties::cascade`) are
//!    `where E: TElement`, even though the element is passed `None` and no `TElement`
//!    method is called at runtime. Rust still requires *naming* a concrete `E`, so a
//!    type implementing `TElement` must exist — a closed graph of `TDocument` (5) +
//!    `TNode` (20) + `TShadowRoot` (6) + `TElement` (76) methods over the arena.
//! 2. **The `ComputedValues` → [`crate::ComputedStyle`] mapping** (~30 properties over
//!    Stylo's packed computed-value types). Independently testable against
//!    `Device::default_computed_values()` without (1).
//!
//! The **exact, source-verified, step-by-step plan** (signatures, module paths, the
//! `match → merge → compute_for_declarations → read` flow, and the property-mapping
//! table) lives in `docs/parity/STYLO-CASCADE-PLAN.md`. This adapter delegates to
//! [`MinimalCascade`] until that lands, so behaviour is well-defined meanwhile.

// Link the Stylo crate so the feature genuinely pulls it in. The real adapter will
// replace this with concrete `stylo::…` usage.
use stylo as _;

use manuk_dom::Dom;

use crate::{MinimalCascade, StyleEngine, StyleMap, Stylesheet};

/// Stylo cascade adapter (currently delegating; see module docs).
#[derive(Debug, Default, Clone, Copy)]
pub struct StyloEngine;

impl StyleEngine for StyloEngine {
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        // TODO(stylo): build Stylist + Device, feed author sheets, run the cascade,
        // and translate Stylo ComputedValues -> crate::ComputedStyle.
        MinimalCascade.cascade(dom, sheets)
    }
}
