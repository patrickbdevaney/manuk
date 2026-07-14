//! E7 storage layer, part 2 — **partitioned** storage.
//!
//! Everything a site can persist is keyed by a three-level partition:
//!
//! ```text
//!   profile  ⊃  container  ⊃  top-level site   (Total Cookie Protection)
//! ```
//!
//! * **Profile** — a whole separate browsing identity (separate disk store).
//! * **Container** — an isolated cookie-jar within a profile (the "work / personal /
//!   shopping" tabs model). Two containers visiting the same site never share cookies.
//! * **Top-level site** — the eTLD+1 of the *page the user is on*. Keying on this too
//!   means a tracker embedded on `a.com` and on `b.com` gets **two different jars**,
//!   so it cannot join the two visits. This is Total Cookie Protection, and here it is
//!   **unconditional** — not opt-in per-cookie the way CHIPS `Partitioned` is.
//!
//! `SameSite` is enforced *here*, not in [`crate::cookies`], because it needs request
//! context (is this a same-site request? is it a top-level navigation?).
//!
//! **Documented gaps (not faked):** this is an in-memory layer — no disk persistence
//! or at-rest encryption yet (that reuses E2's AEAD and is the tracked follow-up); the
//! HTTP cache is not implemented (only cookies / localStorage / history live here).

use std::collections::HashMap;
use std::time::SystemTime;

use url::Url;

use crate::cookies::{Cookie, CookieJar, SameSite};

/// The registrable domain (eTLD+1) of `url` — the "site" for partitioning purposes.
/// Falls back to the full host when there is no registrable domain (IP literals,
/// `localhost`), which is the conservative choice: it isolates rather than merges.
pub fn site_of(url: &Url) -> String {
    let Some(host) = url.host_str() else {
        return String::new();
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    // `psl` happily "parses" an IP literal (`127.0.0.1` → `0.1`), which would merge
    // unrelated hosts into one partition. An IP is its own site.
    if host.parse::<std::net::IpAddr>().is_ok() || matches!(url.host(), Some(url::Host::Ipv6(_))) {
        return host;
    }
    match psl::domain_str(host.as_str()) {
        Some(d) => d.to_ascii_lowercase(),
        None => host,
    }
}

/// Two URLs are same-site when their registrable domains agree.
pub fn is_same_site(a: &Url, b: &Url) -> bool {
    let (sa, sb) = (site_of(a), site_of(b));
    !sa.is_empty() && sa == sb
}

/// Identifies one cookie jar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PartitionKey {
    pub profile: String,
    pub container: String,
    /// eTLD+1 of the top-level page. Empty for a top-level navigation to `url` itself
    /// is not a thing — callers always pass the page's own site.
    pub top_level_site: String,
}

/// Context for a single outgoing request, needed to apply `SameSite`.
///
/// Note the asymmetry, which is easy to get wrong: for a **subresource** the request
/// is cross-site when it disagrees with `top_level`; for a **top-level navigation**
/// the destination *becomes* the top level, so cross-site-ness must instead be judged
/// against the **initiator** (the page that contained the link). Comparing a
/// navigation against `top_level` would wrongly ship `SameSite=Strict` cookies on a
/// cross-site link click — exactly the CSRF case `Strict` exists to prevent.
#[derive(Clone, Debug)]
pub struct RequestContext {
    /// The top-level document's URL. For a navigation, this is the *destination*.
    pub top_level: Url,
    /// The page that initiated this request, if any. Only consulted for navigations.
    pub initiator: Option<Url>,
    /// True when this request *is* the top-level navigation (address bar, link click).
    pub is_top_level_navigation: bool,
}

impl RequestContext {
    /// A subresource load on `top_level`.
    pub fn subresource(top_level: Url) -> Self {
        RequestContext {
            top_level,
            initiator: None,
            is_top_level_navigation: false,
        }
    }

    /// A top-level navigation to `destination`, initiated by `from` (None = typed in
    /// the address bar, which is same-site by definition — it has no initiator).
    pub fn navigation(destination: Url, from: Option<Url>) -> Self {
        RequestContext {
            top_level: destination,
            initiator: from,
            is_top_level_navigation: true,
        }
    }

    /// Whether `request_url` is cross-site for `SameSite` purposes.
    fn is_cross_site(&self, request_url: &Url) -> bool {
        if self.is_top_level_navigation {
            match &self.initiator {
                // A cross-site link click: judged against who linked here.
                Some(from) => !is_same_site(from, request_url),
                // Address-bar / bookmark navigation has no initiator: not cross-site.
                None => false,
            }
        } else {
            !is_same_site(request_url, &self.top_level)
        }
    }
}

/// One entry in a profile's browsing history.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visited_at: SystemTime,
}

/// In-memory, partitioned storage for cookies, `localStorage`, and history.
#[derive(Default, Debug)]
pub struct StorageLayer {
    jars: HashMap<PartitionKey, CookieJar>,
    /// `localStorage`, keyed by partition **and** the script's own origin.
    local: HashMap<(PartitionKey, String), HashMap<String, String>>,
    /// History is per-**profile** (containers share it; that is the Firefox model).
    history: HashMap<String, Vec<HistoryEntry>>,
}

impl StorageLayer {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(profile: &str, container: &str, top_level: &Url) -> PartitionKey {
        PartitionKey {
            profile: profile.to_string(),
            container: container.to_string(),
            top_level_site: site_of(top_level),
        }
    }

    /// The jar for this (profile, container, top-level-site) partition.
    pub fn jar_mut(&mut self, profile: &str, container: &str, top_level: &Url) -> &mut CookieJar {
        self.jars
            .entry(Self::key(profile, container, top_level))
            .or_default()
    }

    /// Read-only view of a partition's jar, if it exists.
    pub fn jar(&self, profile: &str, container: &str, top_level: &Url) -> Option<&CookieJar> {
        self.jars.get(&Self::key(profile, container, top_level))
    }

    /// Store a `Set-Cookie` received from `request_url` while on `ctx.top_level`.
    pub fn store_set_cookie(
        &mut self,
        profile: &str,
        container: &str,
        ctx: &RequestContext,
        request_url: &Url,
        set_cookie: &str,
    ) -> bool {
        self.jar_mut(profile, container, &ctx.top_level)
            .store(request_url, set_cookie)
    }

    /// The `Cookie:` header for `request_url` in this partition, with `SameSite`
    /// applied against `ctx`.
    pub fn cookie_header(
        &self,
        profile: &str,
        container: &str,
        ctx: &RequestContext,
        request_url: &Url,
    ) -> Option<String> {
        let jar = self
            .jars
            .get(&Self::key(profile, container, &ctx.top_level))?;
        let cross_site = ctx.is_cross_site(request_url);
        let top_nav = ctx.is_top_level_navigation;
        jar.cookie_header_where(request_url, SystemTime::now(), |c: &Cookie| {
            if !cross_site {
                return true;
            }
            match c.same_site {
                // Never sent on any cross-site request — including a link click.
                SameSite::Strict => false,
                // Sent cross-site only on a top-level navigation (a link click).
                SameSite::Lax => top_nav,
                // Explicitly cross-site (and, by construction, Secure).
                SameSite::None => true,
            }
        })
    }

    /// `localStorage` read for `origin` within a partition.
    pub fn local_get(
        &self,
        profile: &str,
        container: &str,
        top_level: &Url,
        origin: &str,
        key: &str,
    ) -> Option<&str> {
        self.local
            .get(&(Self::key(profile, container, top_level), origin.to_string()))
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
    }

    /// `localStorage` write for `origin` within a partition.
    pub fn local_set(
        &mut self,
        profile: &str,
        container: &str,
        top_level: &Url,
        origin: &str,
        key: &str,
        value: &str,
    ) {
        self.local
            .entry((Self::key(profile, container, top_level), origin.to_string()))
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    /// Record a visit in the profile's history.
    pub fn record_visit(&mut self, profile: &str, url: &str, title: &str) {
        self.history
            .entry(profile.to_string())
            .or_default()
            .push(HistoryEntry {
                url: url.to_string(),
                title: title.to_string(),
                visited_at: SystemTime::now(),
            });
    }

    pub fn history(&self, profile: &str) -> &[HistoryEntry] {
        self.history
            .get(profile)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Forget everything a container ever stored (the "close container" action).
    pub fn clear_container(&mut self, profile: &str, container: &str) {
        self.jars
            .retain(|k, _| !(k.profile == profile && k.container == container));
        self.local
            .retain(|(k, _), _| !(k.profile == profile && k.container == container));
    }

    /// Forget an entire profile.
    pub fn clear_profile(&mut self, profile: &str) {
        self.jars.retain(|k, _| k.profile != profile);
        self.local.retain(|(k, _), _| k.profile != profile);
        self.history.remove(profile);
    }

    /// Number of live partitions (jars). Useful for the task manager / tests.
    pub fn partition_count(&self) -> usize {
        self.jars.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }
    fn top(s: &str) -> RequestContext {
        RequestContext::subresource(u(s))
    }

    #[test]
    fn site_is_the_registrable_domain() {
        assert_eq!(site_of(&u("https://a.b.example.com/x")), "example.com");
        assert_eq!(site_of(&u("https://example.co.uk/")), "example.co.uk");
        assert_eq!(site_of(&u("https://sub.example.co.uk/")), "example.co.uk");
        // No registrable domain -> isolate on the full host (conservative).
        assert_eq!(site_of(&u("http://localhost:8080/")), "localhost");
        assert_eq!(site_of(&u("http://127.0.0.1/")), "127.0.0.1");
    }

    /// E7's headline acceptance: two containers keep separate cookie jars for the
    /// same site.
    #[test]
    fn two_containers_keep_separate_jars_for_the_same_site() {
        let mut s = StorageLayer::new();
        let ctx = top("https://example.com/");
        let url = u("https://example.com/");

        s.store_set_cookie("default", "work", &ctx, &url, "sid=work-session; Path=/");
        s.store_set_cookie(
            "default",
            "personal",
            &ctx,
            &url,
            "sid=personal-session; Path=/",
        );

        assert_eq!(
            s.cookie_header("default", "work", &ctx, &url),
            Some("sid=work-session".to_string())
        );
        assert_eq!(
            s.cookie_header("default", "personal", &ctx, &url),
            Some("sid=personal-session".to_string())
        );
        // A third container has seen nothing.
        assert_eq!(s.cookie_header("default", "shopping", &ctx, &url), None);
    }

    #[test]
    fn profiles_are_isolated_from_each_other() {
        let mut s = StorageLayer::new();
        let ctx = top("https://example.com/");
        let url = u("https://example.com/");
        s.store_set_cookie("alice", "default", &ctx, &url, "sid=alice; Path=/");
        assert_eq!(s.cookie_header("bob", "default", &ctx, &url), None);
    }

    /// Total Cookie Protection: the same third-party tracker gets a different jar
    /// under each top-level site, so it cannot join the two visits.
    #[test]
    fn third_party_cookies_are_partitioned_by_top_level_site() {
        let mut s = StorageLayer::new();
        let tracker = u("https://tracker.example/pixel");

        let on_a = top("https://news-site.org/");
        let on_b = top("https://shop-site.com/");

        // The tracker sets an id while embedded on site A.
        s.store_set_cookie(
            "default",
            "default",
            &on_a,
            &tracker,
            "id=AAA; Path=/; SameSite=None; Secure",
        );

        // Embedded on site B it must NOT see that id.
        assert_eq!(s.cookie_header("default", "default", &on_b, &tracker), None);
        // Back on site A it does.
        assert_eq!(
            s.cookie_header("default", "default", &on_a, &tracker),
            Some("id=AAA".to_string())
        );
        assert_eq!(s.partition_count(), 1);
    }

    #[test]
    fn same_site_request_sends_every_cookie() {
        let mut s = StorageLayer::new();
        let site = u("https://example.com/");
        let own = top("https://example.com/");

        s.store_set_cookie("p", "c", &own, &site, "strict=1; Path=/; SameSite=Strict");
        s.store_set_cookie("p", "c", &own, &site, "lax=1; Path=/; SameSite=Lax");
        s.store_set_cookie(
            "p",
            "c",
            &own,
            &site,
            "none=1; Path=/; SameSite=None; Secure",
        );

        let hdr = s.cookie_header("p", "c", &own, &site).unwrap();
        assert!(hdr.contains("strict=1") && hdr.contains("lax=1") && hdr.contains("none=1"));
    }

    /// A cross-site **subresource** (e.g. a tracker pixel for `tracker.example`
    /// embedded on `news-site.org`) may only carry `SameSite=None` cookies.
    #[test]
    fn cross_site_subresource_sends_only_same_site_none() {
        let mut s = StorageLayer::new();
        let tracker = u("https://tracker.example/pixel");
        // Partition: browsing news-site.org. The tracker sets three cookies from there.
        let on_news = RequestContext::subresource(u("https://news-site.org/"));
        s.store_set_cookie(
            "p",
            "c",
            &on_news,
            &tracker,
            "strict=1; Path=/; SameSite=Strict",
        );
        s.store_set_cookie("p", "c", &on_news, &tracker, "lax=1; Path=/; SameSite=Lax");
        s.store_set_cookie(
            "p",
            "c",
            &on_news,
            &tracker,
            "none=1; Path=/; SameSite=None; Secure",
        );

        // A later subresource request to the tracker, still on news-site.org.
        let hdr = s.cookie_header("p", "c", &on_news, &tracker).unwrap();
        assert_eq!(
            hdr, "none=1",
            "Strict/Lax must not ride a cross-site subresource"
        );
    }

    /// The rule that the naive `top_level` comparison gets wrong: on a **cross-site
    /// link click**, `Lax` rides along but `Strict` must not — even though the
    /// destination has just become the top-level document.
    #[test]
    fn cross_site_navigation_sends_lax_but_never_strict() {
        let mut s = StorageLayer::new();
        let dest = u("https://example.com/account");
        // Cookies established while genuinely on example.com.
        let own = RequestContext::subresource(u("https://example.com/"));
        s.store_set_cookie("p", "c", &own, &dest, "strict=1; Path=/; SameSite=Strict");
        s.store_set_cookie("p", "c", &own, &dest, "lax=1; Path=/; SameSite=Lax");
        s.store_set_cookie(
            "p",
            "c",
            &own,
            &dest,
            "none=1; Path=/; SameSite=None; Secure",
        );

        // Clicking a link on evil.org that points at example.com/account (the CSRF shape).
        let clicked = RequestContext::navigation(dest.clone(), Some(u("https://evil.org/")));
        let hdr = s.cookie_header("p", "c", &clicked, &dest).unwrap();
        assert!(hdr.contains("lax=1"), "Lax rides a top-level navigation");
        assert!(hdr.contains("none=1"));
        assert!(
            !hdr.contains("strict=1"),
            "Strict must NOT ride a cross-site link click — that is the CSRF case"
        );
    }

    /// Typing the URL in the address bar has no initiator, so it is not cross-site
    /// and even `Strict` cookies are sent.
    #[test]
    fn address_bar_navigation_sends_strict_cookies() {
        let mut s = StorageLayer::new();
        let dest = u("https://example.com/account");
        let own = RequestContext::subresource(u("https://example.com/"));
        s.store_set_cookie("p", "c", &own, &dest, "strict=1; Path=/; SameSite=Strict");

        let typed = RequestContext::navigation(dest.clone(), None);
        assert_eq!(
            s.cookie_header("p", "c", &typed, &dest),
            Some("strict=1".to_string())
        );
    }

    /// A same-site link click (example.com -> example.com) keeps Strict cookies.
    #[test]
    fn same_site_navigation_keeps_strict() {
        let mut s = StorageLayer::new();
        let dest = u("https://example.com/account");
        let own = RequestContext::subresource(u("https://example.com/"));
        s.store_set_cookie("p", "c", &own, &dest, "strict=1; Path=/; SameSite=Strict");

        let clicked = RequestContext::navigation(dest.clone(), Some(u("https://www.example.com/")));
        assert_eq!(
            s.cookie_header("p", "c", &clicked, &dest),
            Some("strict=1".to_string())
        );
    }

    #[test]
    fn local_storage_is_partitioned_by_container_and_origin() {
        let mut s = StorageLayer::new();
        let tl = u("https://example.com/");
        s.local_set("p", "work", &tl, "https://example.com", "k", "work-val");
        s.local_set("p", "home", &tl, "https://example.com", "k", "home-val");

        assert_eq!(
            s.local_get("p", "work", &tl, "https://example.com", "k"),
            Some("work-val")
        );
        assert_eq!(
            s.local_get("p", "home", &tl, "https://example.com", "k"),
            Some("home-val")
        );
        // A different origin in the same container sees nothing.
        assert_eq!(
            s.local_get("p", "work", &tl, "https://other.org", "k"),
            None
        );
    }

    #[test]
    fn clearing_a_container_leaves_its_siblings_and_history_intact() {
        let mut s = StorageLayer::new();
        let tl = u("https://example.com/");
        let ctx = top("https://example.com/");
        s.store_set_cookie("p", "work", &ctx, &tl, "a=1; Path=/");
        s.store_set_cookie("p", "home", &ctx, &tl, "b=2; Path=/");
        s.local_set("p", "work", &tl, "https://example.com", "k", "v");
        s.record_visit("p", "https://example.com/", "Example");

        s.clear_container("p", "work");

        assert_eq!(s.cookie_header("p", "work", &ctx, &tl), None);
        assert_eq!(
            s.local_get("p", "work", &tl, "https://example.com", "k"),
            None
        );
        assert_eq!(
            s.cookie_header("p", "home", &ctx, &tl),
            Some("b=2".to_string())
        );
        // History is per-profile and survives a container reset.
        assert_eq!(s.history("p").len(), 1);

        s.clear_profile("p");
        assert_eq!(s.history("p").len(), 0);
        assert_eq!(s.partition_count(), 0);
    }
}
