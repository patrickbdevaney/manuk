//! N5 — **per-invocation permission scoping** (research item H4).
//!
//! ## Why a capability *value*, not a bearer token
//!
//! CaMeL (Debenedetti et al., arXiv:2503.18813) shows that what actually defeats prompt
//! injection is enforcing a policy **at the tool-call boundary** — a check before each
//! action, over the *existing* action schema. That is precisely where [`crate::assess_action`]
//! already sits, so scoping composes with H1–H3 instead of becoming a second schema.
//!
//! Macaroons (Birgisson et al., CCS 2014) give *attenuable bearer credentials* via chained
//! HMACs, and their reason for existing is **decentralized delegation without a round-trip
//! to an issuer**: a holder attenuates its own authority for another holder across a trust
//! boundary. In-process — where the shell constructs the agent loop directly and no
//! untrusted party ever holds the credential — an HMAC-chained token buys nothing over a
//! plain Rust value, while adding crypto surface for **zero threat-model gain**. So:
//! a plain [`Capabilities`] value now; attenuation only once authority actually crosses a
//! process boundary (out-of-process agent, or agent→sub-agent delegation).
//!
//! ## What it adds over the existing guard
//!
//! [`crate::assess_action`] is a *heuristic* about an action's intrinsic risk (does this URL
//! look like a checkout page?). `Capabilities` is an *authority* statement (was this agent
//! ever allowed to submit forms, or to leave `example.com`?). They are independent, and
//! both must pass — a target the heuristic happily calls `Safe` is still refused when it
//! lies outside the granted origins. The E6 panel simply constructs a narrower
//! `Capabilities` than the headless binary.
//!
//! **Documented gaps (not faked):** no per-action argument constraints (e.g. "may type into
//! `q` but not into `password`"); no revocation mid-run; the origin allowlist is checked on
//! `navigate`/`click`-style targets, not on subresource loads the page itself initiates
//! (that is the content-blocker's layer, E5).

use std::collections::HashSet;

use crate::{Action, ActionRisk, Observation};

/// The kinds of action an agent may be granted. One variant per [`Action`] discriminant,
/// so a grant is exhaustive and adding an action forces a decision here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Navigate,
    Click,
    Scroll,
    Finish,
    Type,
    ClickText,
    ClickAt,
    Submit,
    Back,
    Forward,
    ScrollTo,
}

impl ActionKind {
    pub fn of(action: &Action) -> ActionKind {
        match action {
            Action::Navigate { .. } => ActionKind::Navigate,
            Action::Click { .. } => ActionKind::Click,
            Action::Scroll { .. } => ActionKind::Scroll,
            Action::Finish { .. } => ActionKind::Finish,
            Action::Type { .. } => ActionKind::Type,
            Action::ClickText { .. } => ActionKind::ClickText,
            Action::ClickAt { .. } => ActionKind::ClickAt,
            Action::Submit { .. } => ActionKind::Submit,
            Action::Back => ActionKind::Back,
            Action::Forward => ActionKind::Forward,
            Action::ScrollTo { .. } => ActionKind::ScrollTo,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ActionKind::Navigate => "navigate",
            ActionKind::Click => "click",
            ActionKind::Scroll => "scroll",
            ActionKind::Finish => "finish",
            ActionKind::Type => "type",
            ActionKind::ClickText => "click_text",
            ActionKind::ClickAt => "click_at",
            ActionKind::Submit => "submit",
            ActionKind::Back => "back",
            ActionKind::Forward => "forward",
            ActionKind::ScrollTo => "scroll_to",
        }
    }
}

/// What an agent invocation is authorized to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    /// Permitted action kinds. An action not in this set is refused.
    allowed: HashSet<ActionKind>,
    /// Permitted origins (`scheme://host[:port]`). **Empty means "any origin"** — the
    /// headless default — because an empty allowlist that meant "nothing" would silently
    /// brick every grant that forgot to name one.
    origins: HashSet<String>,
    /// E6: may the agent auto-run [`ActionRisk::Sensitive`] actions without confirmation?
    pub allow_sensitive_actions: bool,
}

impl Default for Capabilities {
    /// The headless default: every action, any origin, but **sensitive actions still
    /// require confirmation** (E6's default stands).
    fn default() -> Self {
        Capabilities::all_actions()
    }
}

impl Capabilities {
    /// Every action kind, any origin, sensitive actions refused.
    pub fn all_actions() -> Self {
        use ActionKind::*;
        Capabilities {
            allowed: [
                Navigate, Click, Scroll, Finish, Type, ClickText, ClickAt, Submit, Back,
                Forward, ScrollTo,
            ]
            .into_iter()
            .collect(),
            origins: HashSet::new(),
            allow_sensitive_actions: false,
        }
    }

    /// A **read-only** grant: look, scroll, traverse history, answer. Cannot type, click a
    /// control, submit, or navigate anywhere new. This is the shape the E6 panel wants for
    /// "summarize this page".
    pub fn read_only() -> Self {
        use ActionKind::*;
        Capabilities {
            allowed: [Scroll, ScrollTo, Back, Forward, Finish].into_iter().collect(),
            origins: HashSet::new(),
            allow_sensitive_actions: false,
        }
    }

    /// Grant exactly these action kinds.
    pub fn with_actions(kinds: impl IntoIterator<Item = ActionKind>) -> Self {
        Capabilities {
            allowed: kinds.into_iter().collect(),
            origins: HashSet::new(),
            allow_sensitive_actions: false,
        }
    }

    /// Restrict navigation to these origins. Values are normalized to `scheme://host[:port]`.
    pub fn allow_origins<S: AsRef<str>>(mut self, origins: impl IntoIterator<Item = S>) -> Self {
        for o in origins {
            if let Some(n) = normalize_origin(o.as_ref()) {
                self.origins.insert(n);
            }
        }
        self
    }

    pub fn allow_sensitive(mut self, yes: bool) -> Self {
        self.allow_sensitive_actions = yes;
        self
    }

    pub fn permits_kind(&self, kind: ActionKind) -> bool {
        self.allowed.contains(&kind)
    }

    /// Whether `url` is within the granted origins. An empty allowlist permits any origin.
    pub fn permits_origin(&self, url: &str) -> bool {
        if self.origins.is_empty() {
            return true;
        }
        match normalize_origin(url) {
            Some(o) => self.origins.contains(&o),
            // A URL we cannot parse an origin from (e.g. `data:`) is not in any allowlist.
            None => false,
        }
    }

    /// The origins granted, for logging.
    pub fn origins(&self) -> Vec<&str> {
        self.origins.iter().map(String::as_str).collect()
    }
}

/// `scheme://host[:port]`, lowercased. `None` for opaque-origin URLs (`data:`, `about:`).
fn normalize_origin(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    let host = u.host_str()?;
    let scheme = u.scheme().to_ascii_lowercase();
    Some(match u.port() {
        Some(p) => format!("{scheme}://{}:{p}", host.to_ascii_lowercase()),
        None => format!("{scheme}://{}", host.to_ascii_lowercase()),
    })
}

/// Why an action was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Denial {
    /// The invocation was never granted this kind of action.
    KindNotGranted(ActionKind),
    /// The target lies outside the granted origins.
    OriginNotGranted(String),
    /// The action is [`ActionRisk::Sensitive`] and confirmation was not granted.
    NeedsConfirmation(&'static str),
}

impl std::fmt::Display for Denial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Denial::KindNotGranted(k) => {
                write!(f, "action {:?} was not granted to this invocation", k.as_str())
            }
            Denial::OriginNotGranted(u) => {
                write!(f, "target {u} is outside the granted origins")
            }
            Denial::NeedsConfirmation(reason) => {
                write!(f, "needs human confirmation: {reason}")
            }
        }
    }
}

/// The **single enforcement point**: authority first, then the risk heuristic.
///
/// Both must pass. They are independent questions — "were you ever allowed to do this?"
/// and "does this particular target look dangerous?" — and a target the heuristic calls
/// `Safe` is still refused when it lies outside the granted origins.
pub fn check(action: &Action, obs: &Observation, caps: &Capabilities) -> Result<(), Denial> {
    let kind = ActionKind::of(action);
    if !caps.permits_kind(kind) {
        return Err(Denial::KindNotGranted(kind));
    }

    // Origin authority, for the actions that choose a destination.
    if let Some(target) = action_target(action, obs) {
        if !caps.permits_origin(&target) {
            return Err(Denial::OriginNotGranted(target));
        }
    }

    // The existing E6 risk heuristic, unchanged.
    if !caps.allow_sensitive_actions {
        if let ActionRisk::Sensitive(reason) = crate::assess_action(action, obs) {
            return Err(Denial::NeedsConfirmation(reason));
        }
    }
    Ok(())
}

/// The absolute URL an action would navigate to, when it names one.
fn action_target(action: &Action, obs: &Observation) -> Option<String> {
    match action {
        Action::Navigate { url } => Some(url.clone()),
        Action::Click { index } => obs.links.get(*index).map(|l| l.href.clone()),
        Action::ClickText { role, name } => {
            // Only a link names a destination; a button's target is not knowable here.
            if crate::parse_role(role) != manuk_a11y::Role::Link {
                return None;
            }
            let needle = name.to_ascii_lowercase();
            obs.links
                .iter()
                .find(|l| l.text.to_ascii_lowercase().contains(&needle))
                .map(|l| l.href.clone())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Link;

    fn obs(links: Vec<(&str, &str)>) -> Observation {
        Observation {
            url: "https://example.com/".into(),
            title: "T".into(),
            text: String::new(),
            links: links
                .into_iter()
                .map(|(text, href)| Link {
                    text: text.into(),
                    href: href.into(),
                })
                .collect(),
            semantics: Vec::new(),
            scroll_y: 0.0,
            content_height: 100.0,
            viewport: (800, 600),
        }
    }

    /// N5's headline acceptance: an origin allowlist refuses a navigate that the risk
    /// heuristic alone would call **Safe** — proving the allowlist is a distinct,
    /// composing constraint, not a duplicate of the heuristic.
    #[test]
    fn the_origin_allowlist_refuses_a_target_the_heuristic_calls_safe() {
        let o = obs(vec![]);
        let evil = Action::Navigate {
            url: "https://evil.org/harmless-looking-page".into(),
        };

        // The heuristic on its own sees nothing wrong with it.
        assert_eq!(crate::assess_action(&evil, &o), ActionRisk::Safe);

        // The capability check refuses it anyway.
        let caps = Capabilities::all_actions().allow_origins(["https://example.com"]);
        assert_eq!(
            check(&evil, &o, &caps),
            Err(Denial::OriginNotGranted(
                "https://evil.org/harmless-looking-page".into()
            ))
        );

        // The same action to a granted origin passes.
        let ok = Action::Navigate {
            url: "https://example.com/page".into(),
        };
        assert_eq!(check(&ok, &o, &caps), Ok(()));
    }

    /// The other half of the acceptance: with only `{navigate, scroll, finish}` granted,
    /// a `submit` is refused **with a reason**, and the run can continue.
    #[test]
    fn an_ungranted_action_kind_is_refused_with_a_reason() {
        let o = obs(vec![]);
        let caps = Capabilities::with_actions([
            ActionKind::Navigate,
            ActionKind::Scroll,
            ActionKind::Finish,
        ]);

        let denial = check(&Action::Submit { field: None }, &o, &caps).unwrap_err();
        assert_eq!(denial, Denial::KindNotGranted(ActionKind::Submit));
        assert!(denial.to_string().contains("submit"), "{denial}");

        // The granted ones still pass.
        assert_eq!(check(&Action::Scroll { dy: 10.0 }, &o, &caps), Ok(()));
        assert_eq!(
            check(&Action::Finish { answer: "x".into() }, &o, &caps),
            Ok(())
        );
    }

    /// Authority is checked **before** the heuristic: an ungranted kind is refused as
    /// ungranted, not mislabelled "needs confirmation".
    #[test]
    fn authority_is_checked_before_the_risk_heuristic() {
        let o = obs(vec![]);
        // `submit` is Sensitive by the heuristic AND ungranted here.
        let caps = Capabilities::with_actions([ActionKind::Scroll]);
        assert_eq!(
            check(&Action::Submit { field: None }, &o, &caps).unwrap_err(),
            Denial::KindNotGranted(ActionKind::Submit)
        );
    }

    /// Granting the kind does not grant the risk: `submit` still needs confirmation.
    #[test]
    fn granting_a_kind_does_not_bypass_the_sensitivity_gate() {
        let o = obs(vec![]);
        let caps = Capabilities::all_actions();
        assert!(matches!(
            check(&Action::Submit { field: None }, &o, &caps),
            Err(Denial::NeedsConfirmation(_))
        ));
        // ...unless confirmation was granted.
        let caps = Capabilities::all_actions().allow_sensitive(true);
        assert_eq!(check(&Action::Submit { field: None }, &o, &caps), Ok(()));
    }

    /// An empty allowlist means "any origin", not "no origin" — a grant that forgot to
    /// name an origin must not silently brick.
    #[test]
    fn an_empty_origin_allowlist_permits_any_origin() {
        let caps = Capabilities::all_actions();
        assert!(caps.permits_origin("https://anywhere.test/x"));
        assert!(caps.permits_origin("data:text/html,x"));
    }

    /// Origins compare on scheme+host+port, and an opaque-origin URL is in no allowlist.
    #[test]
    fn origins_normalize_and_opaque_urls_are_never_granted() {
        let caps = Capabilities::all_actions().allow_origins(["HTTPS://Example.COM"]);
        assert!(caps.permits_origin("https://example.com/deep/path?q=1"));
        // Different scheme, host, or port are different origins.
        assert!(!caps.permits_origin("http://example.com/"));
        assert!(!caps.permits_origin("https://sub.example.com/"));
        assert!(!caps.permits_origin("https://example.com:8443/"));
        // Opaque origins cannot be in an allowlist.
        assert!(!caps.permits_origin("data:text/html,<b>x</b>"));
        assert!(!caps.permits_origin("about:blank"));
    }

    /// A link click resolves to its href for the origin check; a button click names no
    /// destination and so is not origin-checked (its risk is the heuristic's job).
    #[test]
    fn origin_is_checked_for_link_targets_including_click_by_index_and_name() {
        let o = obs(vec![("Docs", "https://evil.org/docs")]);
        let caps = Capabilities::all_actions().allow_origins(["https://example.com"]);

        assert_eq!(
            check(&Action::Click { index: 0 }, &o, &caps),
            Err(Denial::OriginNotGranted("https://evil.org/docs".into()))
        );
        assert_eq!(
            check(
                &Action::ClickText {
                    role: "link".into(),
                    name: "docs".into()
                },
                &o,
                &caps
            ),
            Err(Denial::OriginNotGranted("https://evil.org/docs".into()))
        );

        // A button names no destination: no origin check (it is Sensitive anyway).
        assert!(matches!(
            check(
                &Action::ClickText {
                    role: "button".into(),
                    name: "Go".into()
                },
                &o,
                &caps
            ),
            Err(Denial::NeedsConfirmation(_))
        ));
    }

    /// The read-only grant is genuinely read-only.
    #[test]
    fn the_read_only_grant_cannot_act_on_the_page() {
        let o = obs(vec![("Docs", "https://example.com/docs")]);
        let caps = Capabilities::read_only();

        assert_eq!(check(&Action::Scroll { dy: 1.0 }, &o, &caps), Ok(()));
        assert_eq!(check(&Action::Back, &o, &caps), Ok(()));
        assert_eq!(check(&Action::Finish { answer: "a".into() }, &o, &caps), Ok(()));

        for a in [
            Action::Navigate { url: "https://example.com/".into() },
            Action::Click { index: 0 },
            Action::Type { field: "q".into(), text: "x".into() },
            Action::Submit { field: None },
            Action::ClickAt { x: 1.0, y: 1.0 },
        ] {
            assert!(
                matches!(check(&a, &o, &caps), Err(Denial::KindNotGranted(_))),
                "read-only must refuse {a:?}"
            );
        }
    }
}
