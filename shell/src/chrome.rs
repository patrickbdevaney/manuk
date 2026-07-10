//! E1 — **chrome UI state**: per-tab history, bookmarks, settings, and the omnibox
//! (URL-vs-search disambiguation + suggestions).
//!
//! All of this is shell state — no new engine surface, per the item's design. It is
//! kept here as plain data + pure functions so it is unit-testable without a window.

use std::collections::HashMap;

/// A per-tab navigation stack.
///
/// N1: this is now the **one** [`manuk_page::history::SessionHistory`] model, shared with
/// the agent and with BiDi's `browsingContext.traverseHistory`. The alias keeps the
/// shell's local name; `position` is the spec's `index`.
pub use manuk_page::history::SessionHistory as History;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmarks {
    items: Vec<Bookmark>,
}

impl Bookmarks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[Bookmark] {
        &self.items
    }

    pub fn contains(&self, url: &str) -> bool {
        self.items.iter().any(|b| b.url == url)
    }

    /// Add (or re-title) a bookmark. Returns false if it already existed.
    pub fn add(&mut self, url: impl Into<String>, title: impl Into<String>) -> bool {
        let url = url.into();
        let title = title.into();
        if let Some(b) = self.items.iter_mut().find(|b| b.url == url) {
            b.title = title;
            return false;
        }
        self.items.push(Bookmark { url, title });
        true
    }

    pub fn remove(&mut self, url: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|b| b.url != url);
        self.items.len() != before
    }

    /// Toggle: returns the new bookmarked state.
    pub fn toggle(&mut self, url: &str, title: &str) -> bool {
        if self.contains(url) {
            self.remove(url);
            false
        } else {
            self.add(url, title);
            true
        }
    }
}

/// Persistent shell settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub home_url: String,
    /// `%s` is replaced by the percent-encoded query.
    pub search_template: String,
    pub default_zoom: f32,
    pub block_trackers: bool,
    /// Per-origin zoom overrides, as browsers remember them.
    pub zoom_by_origin: HashMap<String, f32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            home_url: "about:blank".to_string(),
            search_template: "https://duckduckgo.com/?q=%s".to_string(),
            default_zoom: 1.0,
            block_trackers: true,
            zoom_by_origin: HashMap::new(),
        }
    }
}

/// What the omnibox decided a typed string means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OmniboxIntent {
    /// Navigate to this absolute URL.
    Navigate(String),
    /// Search for this text (already turned into a search URL).
    Search(String),
}

impl OmniboxIntent {
    pub fn url(&self) -> &str {
        match self {
            OmniboxIntent::Navigate(u) | OmniboxIntent::Search(u) => u,
        }
    }
}

/// Percent-encode a search query for a URL query component.
fn encode_query(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for b in q.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decide whether typed text is a URL or a search.
///
/// The rules are the pragmatic ones every browser converges on, and the ordering
/// matters: an explicit scheme always wins; text with a space is always a search (so
/// "rust lang" never becomes a hostname); a bare token is a URL only if it has a dot and
/// a plausible TLD, or is `localhost`, so "rust" searches but "rust-lang.org" navigates.
pub fn omnibox_intent(input: &str, settings: &Settings) -> OmniboxIntent {
    let t = input.trim();
    if t.is_empty() {
        return OmniboxIntent::Navigate(settings.home_url.clone());
    }

    // 1. An explicit scheme is decisive.
    if let Some((scheme, _)) = t.split_once("://") {
        if !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+') {
            return OmniboxIntent::Navigate(t.to_string());
        }
    }
    for s in ["about:", "data:", "file:", "javascript:"] {
        if t.starts_with(s) {
            return OmniboxIntent::Navigate(t.to_string());
        }
    }

    // 2. Whitespace means it is prose, not a host.
    if t.split_whitespace().count() > 1 {
        return OmniboxIntent::Search(search_url(t, settings));
    }

    // 3. A bare token: host-like?
    let host = t.split(['/', '?', '#']).next().unwrap_or(t);
    let host = host.split(':').next().unwrap_or(host); // strip :port
    let looks_like_host = host == "localhost"
        || host.parse::<std::net::IpAddr>().is_ok()
        || (host.contains('.')
            && !host.starts_with('.')
            && !host.ends_with('.')
            && host
                .rsplit('.')
                .next()
                .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())));

    if looks_like_host {
        OmniboxIntent::Navigate(format!("https://{t}"))
    } else {
        OmniboxIntent::Search(search_url(t, settings))
    }
}

/// Turn a query into a search URL via the configured `search_template`. Public so §5's
/// agent-driven "open a tab with a search query" reuses the same configurable engine setting
/// rather than hardcoding a provider.
pub fn search_url(q: &str, settings: &Settings) -> String {
    settings.search_template.replace("%s", &encode_query(q))
}

/// The directive's default search engine for the agent's open-with-search action. The
/// template stays configurable (via [`Settings::search_template`]); this is only the default.
pub const GOOGLE_SEARCH_TEMPLATE: &str = "https://www.google.com/search?q=%s";

/// One omnibox dropdown row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub url: String,
    pub title: String,
    pub source: SuggestionSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionSource {
    Bookmark,
    History,
}

/// Rank suggestions for `input`: bookmarks before history, and within each, a prefix
/// match on the URL before a match anywhere. Deduplicated by URL, capped at `limit`.
pub fn suggestions(
    input: &str,
    history: &[String],
    bookmarks: &Bookmarks,
    limit: usize,
) -> Vec<Suggestion> {
    let q = input.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    // (rank, suggestion). Lower rank sorts first.
    let mut scored: Vec<(u8, Suggestion)> = Vec::new();

    let rank_of = |haystack: &str, base: u8| -> Option<u8> {
        let h = haystack.to_lowercase();
        // A prefix match is what the user is most likely completing.
        let stripped = h
            .strip_prefix("https://")
            .or_else(|| h.strip_prefix("http://"))
            .unwrap_or(&h);
        if stripped.starts_with(&q) || h.starts_with(&q) {
            Some(base)
        } else if h.contains(&q) {
            Some(base + 1)
        } else {
            None
        }
    };

    for b in bookmarks.items() {
        let r = rank_of(&b.url, 0).or_else(|| {
            b.title
                .to_lowercase()
                .contains(&q)
                .then_some(1)
        });
        if let Some(r) = r {
            scored.push((
                r,
                Suggestion {
                    url: b.url.clone(),
                    title: b.title.clone(),
                    source: SuggestionSource::Bookmark,
                },
            ));
        }
    }

    // History: most recent first, so iterate in reverse.
    for url in history.iter().rev() {
        if let Some(r) = rank_of(url, 4) {
            scored.push((
                r,
                Suggestion {
                    url: url.clone(),
                    title: url.clone(),
                    source: SuggestionSource::History,
                },
            ));
        }
    }

    scored.sort_by_key(|(r, _)| *r);

    let mut seen = std::collections::HashSet::new();
    scored
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| seen.insert(s.url.clone()))
        .take(limit)
        .collect()
}

/// E1 zoom stepping — the discrete ladder browsers actually use, so Ctrl+/− lands on
/// familiar values rather than accumulating float error.
pub const ZOOM_STEPS: &[f32] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0,
];

fn nearest_step(zoom: f32) -> usize {
    ZOOM_STEPS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (*a - zoom)
                .abs()
                .partial_cmp(&(*b - zoom).abs())
                .expect("zoom steps are finite")
        })
        .map(|(i, _)| i)
        .unwrap_or(7)
}

pub fn zoom_in(zoom: f32) -> f32 {
    let i = nearest_step(zoom);
    ZOOM_STEPS[(i + 1).min(ZOOM_STEPS.len() - 1)]
}

pub fn zoom_out(zoom: f32) -> f32 {
    let i = nearest_step(zoom);
    ZOOM_STEPS[i.saturating_sub(1)]
}

pub fn zoom_reset() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_traverses_and_truncates_forward_entries() {
        let mut h = History::new();
        assert!(!h.can_go_back() && !h.can_go_forward());

        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.current(), Some("c"));
        assert!(h.can_go_back() && !h.can_go_forward());

        assert_eq!(h.back(), Some("b"));
        assert_eq!(h.back(), Some("a"));
        assert!(!h.can_go_back());
        assert_eq!(h.back(), None, "running off the start is None, not a panic");

        assert_eq!(h.forward(), Some("b"));
        // Navigating from the middle drops "c".
        h.push("d");
        assert!(!h.can_go_forward());
        assert_eq!(h.entries(), &["a", "b", "d"]);
    }

    /// Re-navigating to the current URL must not push a duplicate, or `back` would
    /// appear to do nothing.
    #[test]
    fn pushing_the_current_url_is_a_no_op() {
        let mut h = History::new();
        h.push("a");
        assert!(!h.push("a"));
        assert_eq!(h.entries().len(), 1);
        h.push("b");
        assert!(h.can_go_back());
    }

    #[test]
    fn bookmarks_add_toggle_and_retitle() {
        let mut b = Bookmarks::new();
        assert!(b.add("https://a.test/", "A"));
        assert!(!b.add("https://a.test/", "A renamed"), "existing => false");
        assert_eq!(b.items()[0].title, "A renamed");
        assert!(b.contains("https://a.test/"));

        assert!(!b.toggle("https://a.test/", "A"), "toggle off");
        assert!(!b.contains("https://a.test/"));
        assert!(b.toggle("https://a.test/", "A"), "toggle on");
        assert_eq!(b.items().len(), 1);
        assert!(b.remove("https://a.test/"));
        assert!(!b.remove("https://a.test/"), "removing twice => false");
    }

    #[test]
    fn omnibox_distinguishes_urls_from_searches() {
        let s = Settings::default();
        let nav = |i: &str| omnibox_intent(i, &s);

        // Explicit scheme wins, always.
        assert_eq!(
            nav("https://rust-lang.org"),
            OmniboxIntent::Navigate("https://rust-lang.org".into())
        );
        assert_eq!(
            nav("about:blank"),
            OmniboxIntent::Navigate("about:blank".into())
        );

        // Host-like bare tokens navigate (scheme added).
        assert_eq!(
            nav("rust-lang.org"),
            OmniboxIntent::Navigate("https://rust-lang.org".into())
        );
        assert_eq!(
            nav("localhost:8080/x"),
            OmniboxIntent::Navigate("https://localhost:8080/x".into())
        );
        assert_eq!(
            nav("127.0.0.1"),
            OmniboxIntent::Navigate("https://127.0.0.1".into())
        );

        // Prose searches — crucially, anything with a space.
        assert!(matches!(nav("rust lang"), OmniboxIntent::Search(_)));
        assert!(matches!(nav("rust"), OmniboxIntent::Search(_)));
        // A dotted token whose "TLD" is numeric is not a host.
        assert!(matches!(nav("version.2"), OmniboxIntent::Search(_)));

        // Empty goes home.
        assert_eq!(nav("  "), OmniboxIntent::Navigate(s.home_url.clone()));
    }

    #[test]
    fn search_urls_are_percent_encoded() {
        let s = Settings::default();
        match omnibox_intent("rust & c++", &s) {
            OmniboxIntent::Search(u) => {
                assert_eq!(u, "https://duckduckgo.com/?q=rust+%26+c%2B%2B");
            }
            other => panic!("expected a search, got {other:?}"),
        }
    }

    #[test]
    fn suggestions_rank_bookmarks_first_then_prefix_then_substring() {
        let mut bm = Bookmarks::new();
        bm.add("https://docs.rs/", "Docs");
        let history = vec![
            "https://example.com/rust".to_string(),
            "https://doc.rust-lang.org/".to_string(),
        ];

        let s = suggestions("doc", &history, &bm, 10);
        assert_eq!(s[0].source, SuggestionSource::Bookmark);
        assert_eq!(s[0].url, "https://docs.rs/");
        // Then the history prefix match (scheme-stripped).
        assert_eq!(s[1].url, "https://doc.rust-lang.org/");

        // Substring-only matches rank after prefix matches.
        let s = suggestions("rust", &history, &bm, 10);
        assert!(s.iter().any(|x| x.url.contains("example.com/rust")));

        // Dedup + limit.
        assert!(suggestions("http", &history, &bm, 1).len() <= 1);
        assert!(suggestions("", &history, &bm, 10).is_empty());
    }

    #[test]
    fn zoom_steps_walk_the_ladder_and_clamp_at_both_ends() {
        assert_eq!(zoom_reset(), 1.0);
        assert_eq!(zoom_in(1.0), 1.1);
        assert_eq!(zoom_out(1.0), 0.9);

        // From an off-ladder value we snap to the nearest step first.
        assert_eq!(zoom_in(1.05), 1.1);

        // Clamping: repeated zoom-out cannot go below the first step.
        let mut z = 1.0;
        for _ in 0..20 {
            z = zoom_out(z);
        }
        assert_eq!(z, ZOOM_STEPS[0]);

        let mut z = 1.0;
        for _ in 0..20 {
            z = zoom_in(z);
        }
        assert_eq!(z, ZOOM_STEPS[ZOOM_STEPS.len() - 1]);
    }
}
