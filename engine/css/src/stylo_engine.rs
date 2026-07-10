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
//! compiling selectors through a `Stylist`. What this adapter still needs before it
//! can stop delegating is the `TElement`/`TNode`/`selectors::Element` trait wall
//! (~126 methods) over `(&Dom, NodeId)` so Stylo can *match* against our arena DOM,
//! plus a `NodeId`-indexed `AtomicRefCell<ElementData>` store — a dedicated
//! multi-session effort tracked in CLAUDE.md § D2.

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
