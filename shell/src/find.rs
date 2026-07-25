//! E1 — **find-in-page**, over the layout fragment tree.
//!
//! The spec's constraint is the interesting part: highlights are an **overlay**, so
//! finding text must not trigger a relayout. We therefore read the *existing*
//! `TextFragment`s — which already carry absolute positions and widths (§4a) — and
//! return highlight rects the compositor draws on top. Nothing in the page changes.
//!
//! Matching runs over the document-order concatenation of the runs, so a query can span
//! run boundaries ("hello world" matches across two words). Since inline layout emits
//! one run per word, a match maps back to the runs it overlaps, and each match's
//! highlight is that set of rects — precise at word granularity, which is exactly what
//! a highlight needs.
//!
//! **Documented gaps (not faked):** matching is plain substring (no regex, no word
//! boundaries, no diacritic folding); a match is highlighted at *run* granularity, so a
//! query matching the middle of a long word highlights the whole word; text hidden by
//! `overflow`/`clip` is still matchable, because the fragment tree does not model
//! clipping.

use manuk_layout::{BoxContent, LayoutBox, Rect};

/// One match: the rects to highlight, in document order.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub rects: Vec<Rect>,
}

impl Match {
    /// The union of this match's rects — what to scroll to.
    pub fn bounds(&self) -> Rect {
        let mut it = self.rects.iter();
        let first = *it.next().expect("a match always has at least one rect");
        it.fold(first, |acc, r| acc.union(r))
    }
}

/// A find-in-page session: the matches plus which one is active.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindSession {
    pub query: String,
    pub matches: Vec<Match>,
    active: usize,
}

impl FindSession {
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// 1-based index of the active match, for the "3 / 12" counter. Zero when empty.
    pub fn active_index(&self) -> usize {
        if self.matches.is_empty() {
            0
        } else {
            self.active + 1
        }
    }

    pub fn active_match(&self) -> Option<&Match> {
        self.matches.get(self.active)
    }

    /// Advance to the next match, wrapping around.
    pub fn next(&mut self) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }
        self.active = (self.active + 1) % self.matches.len();
        self.active_match()
    }

    /// Step to the previous match, wrapping around.
    pub fn prev(&mut self) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }
        self.active = (self.active + self.matches.len() - 1) % self.matches.len();
        self.active_match()
    }

    /// Every rect to highlight (all matches).
    pub fn all_rects(&self) -> Vec<Rect> {
        self.matches
            .iter()
            .flat_map(|m| m.rects.iter().copied())
            .collect()
    }
}

/// A run of text with its absolute rect, in document order.
struct Run {
    text: String,
    rect: Rect,
    /// Does this run continue the previous one — same word, split by the line breaker? See
    /// [`manuk_layout::TextFragment::continues`]. Decided while the fragment geometry is still in
    /// hand, because by the time a `Run` reaches the haystack builder the baselines are gone.
    continues_prev: bool,
}

/// Collect the fragment tree's text runs in document order.
fn runs(root: &LayoutBox) -> Vec<Run> {
    let mut out = Vec::new();
    let mut prev: Option<manuk_layout::TextFragment> = None;
    root.walk(&mut |b| {
        if let BoxContent::Inline(frags) = &b.content {
            for f in frags {
                if f.text.is_empty() {
                    continue;
                }
                out.push(Run {
                    text: f.text.clone(),
                    rect: f.rect(),
                    continues_prev: prev.as_ref().is_some_and(|p| f.continues(p)),
                });
                prev = Some(f.clone());
            }
        }
    });
    out
}

/// Find every occurrence of `query` in the laid-out page.
///
/// `case_sensitive == false` folds ASCII case (the default a find bar uses). Returns an
/// empty session for an empty or whitespace-only query, rather than matching everywhere.
pub fn find(root: &LayoutBox, query: &str, case_sensitive: bool) -> FindSession {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return FindSession {
            query: query.to_string(),
            ..Default::default()
        };
    }

    let runs = runs(root);
    if runs.is_empty() {
        return FindSession {
            query: query.to_string(),
            ..Default::default()
        };
    }

    // Build the searchable haystack, remembering each run's byte span so a hit maps back to rects.
    //
    // ⚠ This used to join EVERY run with a space, on the premise that "inline layout drops the
    // original whitespace". It does not: layout emits one run per line-break **opportunity**, and
    // CSS puts one after a hyphen, after `//`, and after `?`. So Ctrl+F for `non-mainstream` or for
    // a URL searched a haystack that read `non- mainstream` and `https:// example.com/? a=1`, and
    // found nothing on a page that plainly contained it. Same defect, found independently, in
    // `Page::visible_text` (tick 577) and in selection-copy — hence `TextFragment::continues`.
    let mut haystack = String::new();
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(runs.len());
    for (i, r) in runs.iter().enumerate() {
        if i > 0 && !r.continues_prev {
            haystack.push(' ');
        }
        let start = haystack.len();
        haystack.push_str(&r.text);
        spans.push((start, haystack.len()));
    }

    let (hay, needle) = if case_sensitive {
        (haystack.clone(), query_trimmed.to_string())
    } else {
        (haystack.to_lowercase(), query_trimmed.to_lowercase())
    };
    // Lowercasing can change byte lengths (e.g. 'İ'), which would invalidate `spans`.
    // Fall back to the case-sensitive haystack rather than report wrong rects.
    let (hay, needle) = if hay.len() == haystack.len() {
        (hay, needle)
    } else {
        (haystack.clone(), query_trimmed.to_string())
    };

    let mut matches = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = hay[from..].find(&needle) {
        let start = from + rel;
        let end = start + needle.len();

        // Every run whose span overlaps [start, end) is part of this match.
        let rects: Vec<Rect> = spans
            .iter()
            .enumerate()
            .filter(|(_, (s, e))| *s < end && start < *e)
            .map(|(i, _)| runs[i].rect)
            .collect();
        if !rects.is_empty() {
            matches.push(Match { rects });
        }

        // Advance past this match's start; overlapping matches are not reported twice.
        from = start + needle.len().max(1);
        if from >= hay.len() {
            break;
        }
    }

    FindSession {
        query: query.to_string(),
        matches,
        active: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};
    use manuk_layout::layout_document;
    use manuk_text::FontContext;

    fn layout(html: &str, width: f32) -> LayoutBox {
        let dom = manuk_html::parse(html);
        let sheets = vec![Stylesheet::parse("")];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let fonts = FontContext::new();
        layout_document(&dom, &styles, &fonts, width)
    }

    /// **A line-break OPPORTUNITY is not a space — Ctrl+F must find a hyphenated word.**
    ///
    /// Inline layout emits one fragment per break *opportunity*, not per line, and CSS puts one
    /// after a hyphen, after `//` and after `?`. The haystack builder joined every run with a
    /// space on the premise that "inline layout drops the original whitespace" — so searching a
    /// page that plainly reads `non-mainstream` searched `non- mainstream` and found nothing.
    ///
    /// The same defect was found independently in `Page::visible_text` (the agent's observation
    /// text and the history index) at tick 577 and in selection-copy; the rule now lives once, on
    /// `TextFragment::continues`.
    ///
    /// RED, run: restore `if i > 0 { haystack.push(' ') }` in `find` — the first two assertions
    /// fail. Drop the `continues` test the other way (never separate) and `beta gamma` matches
    /// nothing, which is what the third assertion guards.
    #[test]
    fn a_break_opportunity_is_not_a_space() {
        // Narrow enough that real wrapping happens too, so both paths are exercised.
        let root = layout(
            "<body><p>blocks non-mainstream browsers</p>\
             <p>https://walled.example/?a=1 here</p>\
             <p>alpha beta gamma</p></body>",
            320.0,
        );
        assert_eq!(
            find(&root, "non-mainstream", false).len(),
            1,
            "a hyphen is a break OPPORTUNITY, not a space: joining every run with ' ' wedged one \
             into the middle of the word, so Ctrl+F found nothing on a page that contains it"
        );
        assert_eq!(
            find(&root, "https://walled.example/?a=1", false).len(),
            1,
            "a URL breaks after `//` and after `?` too — the string a user is most likely to \
             search for is the one most likely to be broken"
        );
        assert_eq!(
            find(&root, "beta gamma", false).len(),
            1,
            "THE GUARD: words genuinely separated by a space must stay separated. Concatenating \
             every run would pass the first two assertions and glue the page into one token"
        );
    }

    /// E1's headline acceptance: find-in-page highlights **all** matches, over the
    /// fragment tree, with no relayout.
    #[test]
    fn highlights_every_match_over_the_fragment_tree() {
        let root = layout(
            "<body><p>alpha beta alpha</p><p>gamma alpha</p></body>",
            800.0,
        );
        let s = find(&root, "alpha", false);
        assert_eq!(s.len(), 3, "three occurrences of 'alpha'");

        // Every match has a real, positive-area rect.
        for m in &s.matches {
            let b = m.bounds();
            assert!(b.width > 0.0 && b.height > 0.0, "degenerate rect: {b:?}");
        }
        // The three matches are at three distinct positions.
        let mut ys: Vec<i32> = s.matches.iter().map(|m| m.bounds().y as i32).collect();
        let xs: Vec<i32> = s.matches.iter().map(|m| m.bounds().x as i32).collect();
        ys.dedup();
        assert!(ys.len() >= 2 || xs[0] != xs[1], "matches must be distinct");
        assert_eq!(s.all_rects().len(), 3);
    }

    #[test]
    fn matching_is_case_insensitive_by_default_and_exact_when_asked() {
        let root = layout("<body><p>Rust rust RUST</p></body>", 800.0);
        assert_eq!(find(&root, "rust", false).len(), 3);
        assert_eq!(find(&root, "rust", true).len(), 1);
        assert_eq!(find(&root, "RUST", true).len(), 1);
    }

    /// A query can span run boundaries, because inline layout emits one run per word.
    #[test]
    fn a_multi_word_query_matches_across_runs_and_highlights_both() {
        let root = layout("<body><p>hello world again</p></body>", 800.0);
        let s = find(&root, "hello world", false);
        assert_eq!(s.len(), 1);
        assert_eq!(s.matches[0].rects.len(), 2, "spans two word runs");
        // Its bounds cover both words.
        let b = s.matches[0].bounds();
        assert!(b.width > s.matches[0].rects[0].width);
    }

    #[test]
    fn an_empty_or_blank_query_matches_nothing() {
        let root = layout("<body><p>text</p></body>", 800.0);
        assert!(find(&root, "", false).is_empty());
        assert!(find(&root, "   ", false).is_empty());
    }

    #[test]
    fn a_missing_query_matches_nothing() {
        let root = layout("<body><p>text</p></body>", 800.0);
        assert!(find(&root, "absent", false).is_empty());
    }

    #[test]
    fn next_and_prev_cycle_through_matches_and_wrap() {
        let root = layout("<body><p>a x a x a</p></body>", 800.0);
        let mut s = find(&root, "a", false);
        assert_eq!(s.len(), 3);
        assert_eq!(s.active_index(), 1);

        s.next();
        assert_eq!(s.active_index(), 2);
        s.next();
        assert_eq!(s.active_index(), 3);
        s.next();
        assert_eq!(s.active_index(), 1, "wraps forward");

        s.prev();
        assert_eq!(s.active_index(), 3, "wraps backward");
    }

    #[test]
    fn cycling_an_empty_session_is_a_no_op_not_a_panic() {
        let mut s = FindSession::default();
        assert!(s.next().is_none());
        assert!(s.prev().is_none());
        assert_eq!(s.active_index(), 0);
    }

    /// Text in the `<head>` (title/style) never lays out, so it is never matched.
    #[test]
    fn head_content_is_not_searchable() {
        let root = layout(
            "<title>secret</title><style>.x{color:red}</style><body><p>visible</p></body>",
            800.0,
        );
        assert!(find(&root, "secret", false).is_empty());
        assert!(find(&root, "color", false).is_empty());
        assert_eq!(find(&root, "visible", false).len(), 1);
    }
}
