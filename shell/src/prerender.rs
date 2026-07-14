//! Predictive prerender (L32): decide the single most likely next navigation so the shell can
//! prewarm it (fetch into the HTTP cache + pre-build the page into the bfcache) for an instant
//! click. Pure + unit-tested here; the shell owns the I/O and the bfcache insertion.
//!
//! Guardrails live in [`is_prewarmable`]: only same-origin `http(s)` GET targets are ever
//! predicted, so prerender can never issue a cross-origin request, a non-idempotent method, or a
//! non-network scheme on speculation.

use url::Url;

/// Whether two URLs share an origin (scheme + host + port) — the safety gate for prerender.
pub fn same_origin(a: &str, b: &str) -> bool {
    match (Url::parse(a), Url::parse(b)) {
        (Ok(x), Ok(y)) => x.origin() == y.origin(),
        _ => false,
    }
}

/// Whether `target` may be speculatively prewarmed from the page at `current`: an `http(s)` URL
/// (a GET navigation) with the **same origin** as the current page. Everything else — other
/// origins, `mailto:`/`javascript:`/`data:`, unparseable URLs — is refused.
pub fn is_prewarmable(current: &str, target: &str) -> bool {
    (target.starts_with("http://") || target.starts_with("https://"))
        && same_origin(current, target)
}

/// Predict the single most likely next navigation to prewarm from the page at `current_url`.
/// `hovered` (the link currently under the pointer, if any) is the strongest signal; absent
/// that, fall back to the first same-origin in-content link — a cheap top-of-document heuristic
/// for idle prerender. Returns `None` when nothing safe qualifies.
pub fn predict_next(
    current_url: &str,
    hovered: Option<&str>,
    in_content_links: &[String],
) -> Option<String> {
    if let Some(h) = hovered {
        if is_prewarmable(current_url, h) {
            return Some(h.to_string());
        }
    }
    in_content_links
        .iter()
        .find(|l| is_prewarmable(current_url, l))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_origin_basics() {
        assert!(same_origin("https://a.test/x", "https://a.test/y?z=1"));
        assert!(!same_origin("https://a.test/", "https://b.test/"));
        assert!(!same_origin("https://a.test/", "http://a.test/")); // scheme differs
        assert!(!same_origin("https://a.test/", "not a url"));
    }

    #[test]
    fn hovered_same_origin_is_the_prediction() {
        let links = vec!["https://a.test/other".to_string()];
        assert_eq!(
            predict_next("https://a.test/home", Some("https://a.test/next"), &links),
            Some("https://a.test/next".to_string()),
            "the hovered same-origin link wins outright"
        );
    }

    #[test]
    fn cross_origin_hover_falls_back_to_a_same_origin_content_link() {
        let links = vec![
            "https://cdn.other/asset".to_string(), // cross-origin — skipped
            "https://a.test/article".to_string(),  // same-origin — chosen
        ];
        assert_eq!(
            predict_next("https://a.test/home", Some("https://evil.test/x"), &links),
            Some("https://a.test/article".to_string())
        );
    }

    #[test]
    fn non_http_hover_is_never_prewarmed() {
        assert_eq!(
            predict_next("https://a.test/home", Some("mailto:x@a.test"), &[]),
            None
        );
        assert_eq!(
            predict_next("https://a.test/home", Some("javascript:void(0)"), &[]),
            None
        );
    }

    #[test]
    fn nothing_same_origin_predicts_nothing() {
        let links = vec![
            "https://b.test/1".to_string(),
            "https://c.test/2".to_string(),
        ];
        assert_eq!(predict_next("https://a.test/home", None, &links), None);
    }
}
