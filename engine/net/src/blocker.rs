//! E5 — native content-blocking at the request layer (NOT an extension runtime).
//!
//! Reuses Brave's audited [`adblock`] engine (EasyList / EasyPrivacy / uBO network
//! syntax). A [`ContentBlocker`] is built from filter-list rules and consulted per
//! subresource request; blocked requests are simply never issued. Cosmetic filtering
//! (element hiding) is a DOM/paint hook, separate from this request-layer block.
//!
//! Extensions-as-a-runtime stays out of scope (a scope trap + attack surface); this
//! is a §2/§7 privacy feature.

use adblock::request::Request;
use adblock::Engine;

/// A compiled network content-blocker.
pub struct ContentBlocker {
    engine: Engine,
}

impl ContentBlocker {
    /// Build a blocker from filter-list rules (EasyList/uBO syntax lines).
    pub fn from_rules<S: AsRef<str>>(rules: &[S]) -> Self {
        use adblock::lists::{FilterSet, ParseOptions};
        let mut filter_set = FilterSet::new(false);
        let list_text = rules
            .iter()
            .map(|r| r.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        filter_set.add_filter_list(list_text, ParseOptions::default());
        ContentBlocker {
            engine: Engine::new_with_filter_set(filter_set),
        }
    }

    /// Should a subresource `url`, requested by the page at `source_url` as a
    /// `request_type` (e.g. `"script"`, `"image"`, `"stylesheet"`, `"xmlhttprequest"`),
    /// be blocked? Unparseable requests are not blocked (fail-open).
    pub fn should_block(&self, url: &str, source_url: &str, request_type: &str) -> bool {
        match Request::new(url, source_url, request_type, "GET") {
            Ok(req) => self.engine.check_network_request(&req).should_block(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_tracker_allows_first_party() {
        // A tiny filter list: block a tracker domain + a specific pixel path. (Real
        // TLDs so adblock's public-suffix parser resolves the registrable domain.)
        let blocker =
            ContentBlocker::from_rules(&["||evil-tracker.com^", "||ads-network.net/pixel.gif"]);

        // Third-party tracker script/image on a news page → blocked.
        assert!(blocker.should_block(
            "https://evil-tracker.com/analytics.js",
            "https://news-site.org/",
            "script"
        ));
        assert!(blocker.should_block(
            "https://ads-network.net/pixel.gif",
            "https://news-site.org/",
            "image"
        ));

        // The page's own first-party resource → not blocked.
        assert!(!blocker.should_block(
            "https://news-site.org/app.js",
            "https://news-site.org/",
            "script"
        ));
        // An unrelated third party not on the list → not blocked.
        assert!(!blocker.should_block(
            "https://cdn-provider.com/lib.js",
            "https://news-site.org/",
            "script"
        ));
    }
}
