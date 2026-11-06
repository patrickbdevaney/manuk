//! W3.17: consent that page and MCP code cannot construct.
//!
//! [`HandoffConsent`] (move the live session between the human and the agent) and
//! [`HumanActivation`] (permission for a page to touch the OS clipboard) have private fields and no
//! public constructor except the ones below, and those need a real winit [`KeyEvent`] (whose
//! constructor is private to winit) or a pointer-press [`WindowEvent`] that the shell's event loop
//! received. Page code, the MCP server and the agent loop never see a window event, so they cannot
//! mint either token. The doctests are compile-fail gates for exactly that claim.
//!
//! ```compile_fail
//! // A struct literal is rejected: the field is private and the type is non_exhaustive.
//! let _ = manuk_consent::HandoffConsent { _private: () };
//! ```
//!
//! ```compile_fail
//! // The old public seam is gone.
//! let _ = manuk_consent::HandoffConsent::user_approved();
//! ```
//!
//! ```compile_fail
//! let _ = manuk_consent::HumanActivation { _private: () };
//! ```
//!
//! ```compile_fail
//! // Default/Clone would let code duplicate a token, so neither exists.
//! let c: manuk_consent::HandoffConsent = Default::default();
//! ```

use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};

/// Explicit human consent to move a live session between the human UI and the agent.
#[derive(Debug)]
#[non_exhaustive]
pub struct HandoffConsent {
    _private: (),
}

/// A human gesture that is still "transient": the only thing that lets a page write the OS
/// clipboard. Valid for the dispatch of that one input event, not for timers the page set up.
#[derive(Debug)]
#[non_exhaustive]
pub struct HumanActivation {
    _private: (),
}

/// A key press that really came from the keyboard (not winit's synthetic focus-gain replay).
pub fn key_press_is_human(ev: &KeyEvent, is_synthetic: bool) -> bool {
    ev.state == ElementState::Pressed && !is_synthetic && !ev.repeat
}

impl HandoffConsent {
    /// Mint consent from a human key press (e.g. Enter on the Take over prompt).
    pub fn from_key(ev: &KeyEvent, is_synthetic: bool) -> Option<Self> {
        key_press_is_human(ev, is_synthetic).then_some(HandoffConsent { _private: () })
    }

    /// Mint consent from a left-button press the shell received (a click on Take over / Resume).
    pub fn from_pointer(ev: &WindowEvent) -> Option<Self> {
        pointer_press(ev).then_some(HandoffConsent { _private: () })
    }
}

impl HumanActivation {
    pub fn from_key(ev: &KeyEvent, is_synthetic: bool) -> Option<Self> {
        key_press_is_human(ev, is_synthetic).then_some(HumanActivation { _private: () })
    }

    pub fn from_pointer(ev: &WindowEvent) -> Option<Self> {
        pointer_press(ev).then_some(HumanActivation { _private: () })
    }
}

fn pointer_press(ev: &WindowEvent) -> bool {
    matches!(
        ev,
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        }
    )
}

/// The clipboard writes allowed through to the OS for this drain. `activation` is `Some` only while
/// the shell is inside the dispatch of a human input event; otherwise every write (agent clicks,
/// page timers, anything queued before the gesture) is dropped.
pub fn admit_clipboard(writes: Vec<String>, activation: Option<&HumanActivation>) -> Vec<String> {
    if activation.is_some() {
        writes
    } else {
        Vec::new()
    }
}

/// Test-only minting: gated behind a feature the shell never enables outside `dev-dependencies`.
#[cfg(feature = "test-mint")]
pub mod test_mint {
    use super::*;
    pub fn consent() -> HandoffConsent {
        HandoffConsent { _private: () }
    }
    pub fn activation() -> HumanActivation {
        HumanActivation { _private: () }
    }
}
