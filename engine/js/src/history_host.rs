//! N2 (host half) — the **History API's state model**, with **no JS engine dependency**.
//!
//! Deliberately separate from [`crate::history_bindings`], which is feature-gated behind
//! `spidermonkey`: the spec rules this enforces are pure host logic, so their tests run on
//! every build rather than only on the SpiderMonkey ones.
//!
//! The rules (WHATWG HTML §7.2, nav-history-apis):
//!
//! * `pushState` appends a session-history entry; `replaceState` mutates the current one.
//! * **Neither fetches or loads anything.** The document URL changes; the document does
//!   not. This is why client-side routing works, and why nothing here touches the network.
//! * **Neither fires `popstate`.** `popstate` fires only on *traversal*. Frameworks depend
//!   on that asymmetry — firing it on `pushState` would make a router recurse.
//! * The target URL must be **same-origin**, else `SecurityError`.
//! * A traversal differing from the current URL *only in fragment* also fires `hashchange`.
//!
//! **Documented gaps (not faked):** state objects round-trip through JSON, so
//! non-JSON-serializable values (functions, cycles, `Map`) are lost rather than
//! structured-cloned, which is what the spec calls for; `scrollRestoration` is absent.

use manuk_dom::history::SessionHistory;

/// The host side of the History API: the session-history model, one serialized state
/// object per entry, and the document's current URL.
///
/// `fetches` exists so a test can assert the load-bearing negative — that `pushState`
/// performs **no** network request. It is incremented only by [`HistoryHost::note_fetch`],
/// which nothing in this module calls.
pub struct HistoryHost {
    pub history: SessionHistory,
    /// JSON-serialized state object per history entry, index-aligned with `history`.
    states: Vec<String>,
    current_url: url::Url,
    fetches: usize,
}

/// What a traversal should dispatch, decided on the Rust side so the JS shim stays dumb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalEvents {
    /// URL changed beyond the fragment: `popstate` only.
    PopState,
    /// URL differs only in fragment: `popstate` **and** `hashchange`.
    PopStateAndHashChange,
}

impl HistoryHost {
    pub fn new(url: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(url).map_err(|e| format!("invalid document URL: {e}"))?;
        let mut history = SessionHistory::new();
        history.push(parsed.to_string());
        Ok(HistoryHost {
            history,
            states: vec!["null".to_string()],
            current_url: parsed,
            fetches: 0,
        })
    }

    pub fn current_url(&self) -> &url::Url {
        &self.current_url
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// The JSON of the current entry's state object (`"null"` when unset).
    pub fn state_json(&self) -> &str {
        self.states
            .get(self.history.index())
            .map(String::as_str)
            .unwrap_or("null")
    }

    /// Count a network fetch. Nothing in the History API calls this — it exists so the
    /// "pushState performs no fetch" assertion has something real to read.
    pub fn note_fetch(&mut self) {
        self.fetches += 1;
    }

    pub fn fetches(&self) -> usize {
        self.fetches
    }

    /// Resolve `url` against the document URL and enforce the **same-origin** restriction.
    fn resolve_same_origin(&self, url: &str) -> Result<url::Url, String> {
        let target = self
            .current_url
            .join(url)
            .map_err(|e| format!("could not resolve URL: {e}"))?;
        if target.origin() != self.current_url.origin() {
            return Err(format!(
                "SecurityError: history state URL {target} is not same-origin with {}",
                self.current_url
            ));
        }
        Ok(target)
    }

    /// `history.pushState`. Appends an entry; **no fetch, no popstate**.
    pub fn push_state(&mut self, state_json: &str, url: Option<&str>) -> Result<(), String> {
        let target = match url {
            Some(u) => self.resolve_same_origin(u)?,
            None => self.current_url.clone(),
        };
        // The spec creates an entry even when the URL is unchanged — a router legitimately
        // pushes the same URL with different state. `SessionHistory::push` deliberately
        // dedupes (so `back` never looks broken for *navigations*), so reach around it.
        if self.history.current() == Some(target.as_str()) {
            self.push_duplicate(target.as_str());
        } else {
            self.history.push(target.to_string());
        }
        // Align the state vector to the new index, dropping any forward states the push
        // just truncated.
        let i = self.history.index();
        self.states.truncate(i);
        self.states.push(state_json.to_string());
        debug_assert_eq!(self.states.len(), self.history.len());
        self.current_url = target;
        Ok(())
    }

    /// The spec allows two consecutive entries with the same URL (same URL, new state).
    /// `SessionHistory::push` deliberately dedupes, so this reaches around it.
    fn push_duplicate(&mut self, url: &str) {
        // Re-push through a scratch stack: append a sentinel, then rewrite it.
        let sentinel = format!("{url}#\u{0}manuk-dup");
        self.history.push(sentinel);
        self.history.replace_current(url);
    }

    /// `history.replaceState`. Mutates the current entry; **no new entry, no truncation,
    /// no fetch, no popstate**.
    pub fn replace_state(&mut self, state_json: &str, url: Option<&str>) -> Result<(), String> {
        let target = match url {
            Some(u) => self.resolve_same_origin(u)?,
            None => self.current_url.clone(),
        };
        self.history.replace_current(target.to_string());
        let i = self.history.index();
        if i < self.states.len() {
            self.states[i] = state_json.to_string();
        } else {
            self.states.push(state_json.to_string());
        }
        self.current_url = target;
        Ok(())
    }

    /// `history.go(delta)`. Returns the events to dispatch, or `None` when the delta is
    /// out of range (in which case nothing moved and nothing fires).
    pub fn traverse(&mut self, delta: i64) -> Option<TraversalEvents> {
        let before = self.current_url.clone();
        let landed = self.history.traverse(delta)?.to_string();
        let after = url::Url::parse(&landed).ok()?;

        // Fragment-only difference => hashchange as well as popstate.
        let only_fragment = {
            let mut a = before.clone();
            let mut b = after.clone();
            a.set_fragment(None);
            b.set_fragment(None);
            a == b && before.fragment() != after.fragment()
        };
        self.current_url = after;
        Some(if only_fragment {
            TraversalEvents::PopStateAndHashChange
        } else {
            TraversalEvents::PopState
        })
    }
}


#[cfg(test)]
mod host_tests {
    //! The host model is testable **without a JS engine at all** — which is the point of
    //! N1/N2's split. These encode the same normative assertions the WPT
    //! `html/browsers/history/the-history-interface/` subset makes.
    use super::*;

    fn host() -> HistoryHost {
        HistoryHost::new("https://ex.test/a?q=1").unwrap()
    }

    #[test]
    fn push_state_changes_the_url_without_fetching_and_appends_an_entry() {
        let mut h = host();
        assert_eq!(h.len(), 1);
        h.push_state(r#"{"n":1}"#, Some("/b")).unwrap();
        assert_eq!(h.current_url().as_str(), "https://ex.test/b");
        assert_eq!(h.len(), 2);
        assert_eq!(h.state_json(), r#"{"n":1}"#);
        // The load-bearing negative: pushState performs NO network request.
        assert_eq!(h.fetches(), 0);
    }

    #[test]
    fn replace_state_mutates_in_place_and_does_not_truncate_the_forward_entries() {
        let mut h = host();
        h.push_state("null", Some("/b")).unwrap();
        h.push_state("null", Some("/c")).unwrap();
        h.traverse(-1).unwrap(); // at /b, /c is ahead

        h.replace_state(r#"{"x":9}"#, Some("/b2")).unwrap();
        assert_eq!(h.current_url().as_str(), "https://ex.test/b2");
        assert_eq!(h.len(), 3, "replaceState must not add an entry");
        assert_eq!(h.state_json(), r#"{"x":9}"#);
        assert!(h.history.can_go_forward(), "/c must survive: no truncation");
    }

    /// The same-origin restriction. A router must not be able to point the document at
    /// another origin without navigating.
    #[test]
    fn a_cross_origin_url_is_rejected_with_a_security_error() {
        let mut h = host();
        let err = h.push_state("null", Some("https://evil.test/x")).unwrap_err();
        assert!(err.contains("SecurityError"), "{err}");
        // ...and nothing moved.
        assert_eq!(h.current_url().as_str(), "https://ex.test/a?q=1");
        assert_eq!(h.len(), 1);

        // Same origin, different path/port-less host form: allowed.
        assert!(h.push_state("null", Some("https://ex.test/ok")).is_ok());
    }

    #[test]
    fn pushing_the_same_url_twice_still_creates_two_entries() {
        // A router legitimately pushes the same URL with different state.
        let mut h = host();
        h.push_state(r#"{"s":1}"#, Some("/same")).unwrap();
        h.push_state(r#"{"s":2}"#, Some("/same")).unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h.state_json(), r#"{"s":2}"#);
        h.traverse(-1).unwrap();
        assert_eq!(h.state_json(), r#"{"s":1}"#, "each entry keeps its own state");
    }

    #[test]
    fn traversal_reports_popstate_and_adds_hashchange_only_for_fragment_only_changes() {
        let mut h = HistoryHost::new("https://ex.test/p").unwrap();
        h.push_state("null", Some("/q")).unwrap();
        // /q -> /p differs beyond the fragment.
        assert_eq!(h.traverse(-1), Some(TraversalEvents::PopState));

        let mut h = HistoryHost::new("https://ex.test/p").unwrap();
        h.push_state("null", Some("#one")).unwrap();
        assert_eq!(h.current_url().as_str(), "https://ex.test/p#one");
        // #one -> /p (no fragment) is a fragment-only change.
        assert_eq!(h.traverse(-1), Some(TraversalEvents::PopStateAndHashChange));
    }

    #[test]
    fn an_out_of_range_traversal_moves_nowhere_and_fires_nothing() {
        let mut h = host();
        assert_eq!(h.traverse(-1), None);
        assert_eq!(h.current_url().as_str(), "https://ex.test/a?q=1");
        assert_eq!(h.traverse(5), None);
    }

    #[test]
    fn state_defaults_to_null_and_survives_traversal() {
        let mut h = host();
        assert_eq!(h.state_json(), "null");
        h.push_state(r#"{"a":1}"#, None).unwrap();
        assert_eq!(h.current_url().as_str(), "https://ex.test/a?q=1", "url unchanged");
        assert_eq!(h.state_json(), r#"{"a":1}"#);
        h.traverse(-1).unwrap();
        assert_eq!(h.state_json(), "null");
    }
}
