//! N1 — the **one** session-history model, shared by both front-ends.
//!
//! The headless agent and the headful shell each grew their own navigation stack, and
//! they implement the same contract: a new navigation **truncates** the forward entries;
//! re-navigating to the URL you are already on is a **no-op** (otherwise `back` would
//! appear to do nothing); running off either end is a **failure to report**, not a silent
//! stall. This module is that contract, once, so `shell::chrome`, `manuk-agent`, and
//! `manuk-bidi`'s `browsingContext.traverseHistory` cannot drift apart.
//!
//! It also carries the shape the HTML spec's `History` interface needs — `length`,
//! `index`, and `go(delta)` — so N2's `history.*` bindings bind to *this* rather than
//! inventing a third stack.
//!
//! **Not modelled here (deliberately):** per-entry state objects (`history.state`) and
//! same-origin checks live with the N2 bindings, because they are document concerns; this
//! type is the pure traversal model, usable with no JS engine present at all.

/// A navigation stack. `index` addresses the current entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionHistory {
    entries: Vec<String>,
    index: usize,
}

impl SessionHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// The URL of the current entry, or `None` before the first navigation.
    pub fn current(&self) -> Option<&str> {
        self.entries.get(self.index).map(String::as_str)
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Index of the current entry (the spec's `history.index`).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Alias for [`Self::index`], kept for the shell's chrome-state naming.
    pub fn position(&self) -> usize {
        self.index
    }

    /// Number of entries (the spec's `history.length`).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record a navigation. Returns `false` when it was a no-op because `url` is already
    /// the current entry — pushing a duplicate would make `back` look broken.
    pub fn push(&mut self, url: impl Into<String>) -> bool {
        let url = url.into();
        if self.current() == Some(url.as_str()) {
            return false;
        }
        if !self.entries.is_empty() {
            // A new navigation from the middle discards everything ahead of it.
            self.entries.truncate(self.index + 1);
        }
        self.entries.push(url);
        self.index = self.entries.len() - 1;
        true
    }

    /// Replace the current entry's URL in place, creating one if the stack is empty.
    /// This is `history.replaceState`'s traversal-model half: no new entry, no truncation.
    pub fn replace_current(&mut self, url: impl Into<String>) {
        let url = url.into();
        match self.entries.get_mut(self.index) {
            Some(e) => *e = url,
            None => {
                self.entries.push(url);
                self.index = 0;
            }
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.entries.is_empty() && self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        !self.entries.is_empty() && self.index + 1 < self.entries.len()
    }

    /// Step back one entry, returning the URL to load. `None` when there is nowhere to go.
    pub fn back(&mut self) -> Option<&str> {
        self.traverse(-1)
    }

    pub fn forward(&mut self) -> Option<&str> {
        self.traverse(1)
    }

    /// The spec's `history.go(delta)` and BiDi's `browsingContext.traverseHistory`.
    ///
    /// A `delta` that would land outside the stack is **rejected entirely** (`None`, no
    /// movement) rather than clamped — a clamped traversal silently lands somewhere the
    /// caller did not ask for.
    pub fn traverse(&mut self, delta: i64) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        if delta == 0 {
            return self.current();
        }
        let target = self.index as i64 + delta;
        if target < 0 || target as usize >= self.entries.len() {
            return None;
        }
        self.index = target as usize;
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traverses_and_truncates_forward_entries() {
        let mut h = SessionHistory::new();
        assert!(!h.can_go_back() && !h.can_go_forward());
        assert_eq!(h.back(), None, "running off the start is None, not a panic");

        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.current(), Some("c"));
        assert_eq!(h.len(), 3);
        assert!(h.can_go_back() && !h.can_go_forward());

        assert_eq!(h.back(), Some("b"));
        assert_eq!(h.back(), Some("a"));
        assert!(!h.can_go_back());
        assert_eq!(h.back(), None);

        assert_eq!(h.forward(), Some("b"));
        h.push("d");
        assert!(!h.can_go_forward());
        assert_eq!(h.entries(), &["a", "b", "d"]);
        assert_eq!(h.index(), 2);
    }

    #[test]
    fn pushing_the_current_url_is_a_no_op() {
        let mut h = SessionHistory::new();
        h.push("a");
        assert!(!h.push("a"));
        assert_eq!(h.len(), 1);
        h.push("b");
        assert!(h.can_go_back());
    }

    /// `go(delta)` past either end must move nowhere at all, not clamp to the end — a
    /// clamped traversal silently lands somewhere the caller did not ask for.
    #[test]
    fn traverse_rejects_out_of_range_deltas_without_moving() {
        let mut h = SessionHistory::new();
        h.push("a");
        h.push("b");
        h.push("c"); // index 2

        assert_eq!(h.traverse(-2), Some("a"));
        assert_eq!(h.index(), 0);

        assert_eq!(h.traverse(-1), None, "out of range");
        assert_eq!(h.index(), 0, "a rejected traversal must not move");

        assert_eq!(h.traverse(2), Some("c"));
        assert_eq!(h.traverse(1), None);
        assert_eq!(h.index(), 2);

        assert_eq!(h.traverse(0), Some("c"), "go(0) stays put");
    }

    #[test]
    fn replace_current_makes_no_entry_and_no_truncation() {
        let mut h = SessionHistory::new();
        h.push("a");
        h.push("b");
        h.back();

        h.replace_current("a2");
        assert_eq!(h.entries(), &["a2", "b"], "b survives: no truncation");
        assert_eq!(h.len(), 2);
        assert_eq!(h.index(), 0);
        assert!(h.can_go_forward());

        // On an empty stack it creates the first entry.
        let mut e = SessionHistory::new();
        e.replace_current("x");
        assert_eq!(e.current(), Some("x"));
        assert_eq!(e.len(), 1);
    }
}
