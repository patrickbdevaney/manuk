//! Persistent visited-site history — the **frecency-ranked** source for omnibox autocomplete.
//!
//! The omnibox used to draw its suggestions from `SessionHistory`, the current session's
//! back/forward stack: a flat list of URLs, newest first, that evaporates on quit. So typing `git`
//! could not surface `github.com` because the user visits it *often* — only because it happened to be
//! in this session's stack — and a fresh launch offered no suggestions at all. Every real browser
//! ranks the address bar by **frecency** (frequency + recency) over a persistent history, so the site
//! you go to every day is the first completion.
//!
//! This is that store: one [`VisitEntry`] per URL with a visit count and a recency marker, persisted
//! across restart (via `SessionStore`), queried by prefix for the dropdown.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// One visited site: its URL, the last title we saw it under, how many times it has been visited, and
/// a monotonic recency marker (`last_visit`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisitEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    /// A monotonic sequence number (NOT a wall-clock timestamp) stamped on each visit. Using a
    /// sequence rather than the clock keeps ranking **deterministic** (so it is testable) while still
    /// capturing "which was visited more recently" — the only recency fact frecency needs.
    pub last_visit: u64,
}

/// The persistent visited-site table. Frecency ranks frequency first (the site you visit most is the
/// top completion) with recency as a bounded tie-breaker/boost, so a long-abandoned once-frequent
/// site eventually yields to what you actually use now.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VisitedHistory {
    entries: Vec<VisitEntry>,
    /// The last sequence number handed out — the next visit stamps `seq + 1`.
    #[serde(default)]
    seq: u64,
}

/// Strip the scheme and a leading `www.` so an address-bar prefix (`git…`) matches the way a user
/// reads a URL, not the way it is stored (`https://www.git…`).
fn display_host(url: &str) -> &str {
    let u = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    u.strip_prefix("www.").unwrap_or(u)
}

impl VisitedHistory {
    /// Record a completed navigation to `url` (titled `title`). A repeat visit increments the count
    /// and refreshes recency + title; a new URL is inserted with count 1. `about:blank` and empty
    /// URLs are ignored — they are not places the user chose to go.
    pub fn record(&mut self, url: &str, title: &str) {
        if url.is_empty() || url == "about:blank" {
            return;
        }
        self.seq += 1;
        let seq = self.seq;
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url) {
            e.visit_count = e.visit_count.saturating_add(1);
            e.last_visit = seq;
            if !title.is_empty() {
                e.title = title.to_string();
            }
        } else {
            self.entries.push(VisitEntry {
                url: url.to_string(),
                title: title.to_string(),
                visit_count: 1,
                last_visit: seq,
            });
        }
    }

    /// The frecency score of an entry: the visit count plus a recency term in `[0, 1]` (how close its
    /// last visit is to the newest). Frequency therefore dominates — one extra visit outranks any
    /// recency difference — and recency only orders entries of equal frequency (and nudges a
    /// just-visited site up).
    fn score(&self, e: &VisitEntry) -> f64 {
        let recency = if self.seq == 0 {
            0.0
        } else {
            e.last_visit as f64 / self.seq as f64
        };
        e.visit_count as f64 + recency
    }

    /// All entries, most-frecent first.
    pub fn ranked(&self) -> Vec<&VisitEntry> {
        let mut v: Vec<&VisitEntry> = self.entries.iter().collect();
        v.sort_by(|a, b| {
            self.score(b)
                .partial_cmp(&self.score(a))
                .unwrap_or(Ordering::Equal)
                // Stable, deterministic final tie-break so equal-score entries have a fixed order.
                .then_with(|| a.url.cmp(&b.url))
        });
        v
    }

    /// The omnibox completions for `input`, most-frecent first, capped at `limit`. An empty input
    /// returns the top sites (the dropdown-as-history-list). A non-empty input keeps entries whose
    /// display host **prefix-matches** ahead of ones that merely **contain** the query in the URL or
    /// title — the same prefix-before-substring ordering the address bar has always used — with
    /// frecency ordering *within* each tier.
    pub fn suggest(&self, input: &str, limit: usize) -> Vec<&VisitEntry> {
        let q = input.trim().to_ascii_lowercase();
        let ranked = self.ranked();
        if q.is_empty() {
            return ranked.into_iter().take(limit).collect();
        }
        let mut prefix: Vec<&VisitEntry> = Vec::new();
        let mut substr: Vec<&VisitEntry> = Vec::new();
        for e in ranked {
            let host = display_host(&e.url).to_ascii_lowercase();
            if host.starts_with(&q) {
                prefix.push(e);
            } else if e.url.to_ascii_lowercase().contains(&q)
                || e.title.to_ascii_lowercase().contains(&q)
            {
                substr.push(e);
            }
        }
        prefix.into_iter().chain(substr).take(limit).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_dominates_then_recency_orders() {
        let mut h = VisitedHistory::default();
        h.record("https://a.example/", "A");
        h.record("https://b.example/", "B");
        h.record("https://b.example/", "B"); // b visited twice
        h.record("https://c.example/", "C");
        h.record("https://c.example/", "C");
        h.record("https://c.example/", "C"); // c visited thrice
        let ranked: Vec<&str> = h.ranked().iter().map(|e| e.url.as_str()).collect();
        assert_eq!(
            ranked,
            vec![
                "https://c.example/", // 3 visits
                "https://b.example/", // 2 visits
                "https://a.example/", // 1 visit
            ],
            "the most-frequently-visited site ranks first"
        );
        // Two equally-frequent sites: the more recently visited ranks higher.
        let mut h2 = VisitedHistory::default();
        h2.record("https://old.example/", "old");
        h2.record("https://new.example/", "new");
        let ranked2: Vec<&str> = h2.ranked().iter().map(|e| e.url.as_str()).collect();
        assert_eq!(
            ranked2,
            vec!["https://new.example/", "https://old.example/"],
            "among equal frequency, the more recent visit wins"
        );
    }

    #[test]
    fn repeat_visit_increments_and_refreshes_title() {
        let mut h = VisitedHistory::default();
        h.record("https://x.example/", "First");
        h.record("https://x.example/", "Renamed");
        assert_eq!(h.len(), 1, "a repeat visit updates in place, not appends");
        let e = &h.ranked()[0];
        assert_eq!(e.visit_count, 2);
        assert_eq!(e.title, "Renamed", "the latest title is kept");
    }

    #[test]
    fn about_blank_and_empty_are_not_recorded() {
        let mut h = VisitedHistory::default();
        h.record("about:blank", "New Tab");
        h.record("", "");
        assert!(
            h.is_empty(),
            "about:blank / empty are not real destinations"
        );
    }

    #[test]
    fn suggest_prefix_beats_substring_and_honours_frecency() {
        let mut h = VisitedHistory::default();
        h.record("https://github.com/", "GitHub");
        h.record("https://github.com/", "GitHub"); // frequent
        h.record("https://mygit.io/", "MyGit"); // 'git' only as substring of host tail
        h.record("https://docs.rs/git2", "git2 docs"); // 'git' in path, not host prefix
        let s: Vec<&str> = h
            .suggest("git", 10)
            .iter()
            .map(|e| e.url.as_str())
            .collect();
        // github.com prefix-matches the display host and is most frequent → first.
        assert_eq!(s[0], "https://github.com/");
        // The others match only as substrings (in URL/title); they come after the prefix match.
        assert!(
            s.contains(&"https://docs.rs/git2") && s.contains(&"https://mygit.io/"),
            "substring matches on URL/title are still offered, after prefix matches (got {s:?})"
        );
        assert!(
            s.iter().position(|u| *u == "https://github.com/").unwrap()
                < s.iter().position(|u| *u == "https://docs.rs/git2").unwrap(),
            "the host-prefix match ranks ahead of a path substring match"
        );
        // An empty query returns the top sites (dropdown-as-history).
        assert_eq!(h.suggest("", 2).len(), 2);
    }
}
