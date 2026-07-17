//! E7 storage layer, part 1 — an **RFC 6265 cookie jar**.
//!
//! This is the substrate the profile/container partitioning ([`crate::storage`])
//! keys on. It implements the parts of RFC 6265 that carry security weight and
//! states the rest as gaps rather than pretending:
//!
//! * `Domain` must **domain-match** the request host and may **not be a public
//!   suffix** (`Domain=.com` / `Domain=.co.uk` are rejected via the `psl` crate) —
//!   the classic supercookie hole.
//! * `Secure` cookies are only ever returned over `https`.
//! * `Path` defaults to the request's directory, and path-matching follows §5.1.4.
//! * `Max-Age` takes precedence over `Expires` (§5.2.2); expired cookies are evicted.
//! * Cookies are ordered longest-path-first, then oldest-first (§5.4.2).
//!
//! **Documented gaps (not faked):** `SameSite` is parsed and stored but enforcement
//! needs a request's top-level site, so it is applied by [`crate::storage`], not here.
//! No cookie-count/size quotas, no `__Host-`/`__Secure-` prefix enforcement, no
//! `Partitioned`/CHIPS attribute (our partitioning is unconditional, see `storage`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use url::Url;

/// `SameSite` attribute (parsed and stored; enforced by `storage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// The domain the cookie is scoped to (never leading-dot; normalized lowercase).
    pub domain: String,
    /// True when no `Domain` attribute was sent: only the exact host may receive it.
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    /// Absolute expiry. `None` = session cookie.
    pub expires: Option<SystemTime>,
    /// Insertion order, for the §5.4.2 tie-break.
    creation: u64,
}

impl Cookie {
    fn is_expired_at(&self, now: SystemTime) -> bool {
        self.expires.is_some_and(|e| e <= now)
    }
}

/// Lowercased host of `url`, or `None` for non-host URLs.
fn host_of(url: &Url) -> Option<String> {
    url.host_str()
        .map(|h| h.trim_end_matches('.').to_ascii_lowercase())
}

/// RFC 6265 §5.1.3 domain matching.
fn domain_matches(host: &str, domain: &str) -> bool {
    if host == domain {
        return true;
    }
    // `host` is a subdomain of `domain`, and `host` is not an IP literal.
    host.ends_with(domain)
        && host.len() > domain.len()
        && host.as_bytes()[host.len() - domain.len() - 1] == b'.'
        && host.parse::<std::net::IpAddr>().is_err()
}

/// RFC 6265 §5.1.4 default path: the request path up to (not including) the last `/`.
fn default_path(url: &Url) -> String {
    let p = url.path();
    if !p.starts_with('/') {
        return "/".to_string();
    }
    match p.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => p[..i].to_string(),
    }
}

/// RFC 6265 §5.1.4 path matching.
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }
    cookie_path.ends_with('/') || request_path.as_bytes()[cookie_path.len()] == b'/'
}

/// Whether `domain` is a public suffix (`com`, `co.uk`, …). Such a `Domain` attribute
/// would let one site set a cookie for every site under the suffix.
fn is_public_suffix(domain: &str) -> bool {
    match psl::suffix_str(domain) {
        // `psl` returns the suffix for the input; if the whole input *is* the suffix,
        // there is no registrable label in front of it.
        Some(suffix) => suffix.eq_ignore_ascii_case(domain),
        None => true, // unknown/unlistable → refuse, fail closed
    }
}

/// Parse a single `Set-Cookie` header value in the context of `url`.
/// Returns `None` if the cookie must be ignored (per the rules in the module docs).
pub fn parse_set_cookie(url: &Url, header: &str) -> Option<Cookie> {
    let mut parts = header.split(';');
    let nv = parts.next()?.trim();
    let (name, value) = nv.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let value = value.trim().trim_matches('"').to_string();

    let host = host_of(url)?;
    let secure_scheme = url.scheme() == "https";

    let mut domain_attr: Option<String> = None;
    let mut path_attr: Option<String> = None;
    let mut secure = false;
    let mut http_only = false;
    let mut same_site = SameSite::Lax; // modern default
    let mut max_age: Option<i64> = None;
    let mut expires_attr: Option<SystemTime> = None;

    for attr in parts {
        let attr = attr.trim();
        let (k, v) = match attr.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (attr, ""),
        };
        match k.to_ascii_lowercase().as_str() {
            "domain" if !v.is_empty() => {
                domain_attr = Some(v.trim_start_matches('.').to_ascii_lowercase());
            }
            "path" if v.starts_with('/') => path_attr = Some(v.to_string()),
            "secure" => secure = true,
            "httponly" => http_only = true,
            "samesite" => {
                same_site = match v.to_ascii_lowercase().as_str() {
                    "strict" => SameSite::Strict,
                    "none" => SameSite::None,
                    _ => SameSite::Lax,
                }
            }
            "max-age" => max_age = v.parse::<i64>().ok(),
            "expires" => expires_attr = parse_http_date(v),
            _ => {}
        }
    }

    // `SameSite=None` is only meaningful on a Secure cookie; reject otherwise.
    if same_site == SameSite::None && !secure {
        return None;
    }
    // "Leave Secure Cookies Alone": a non-secure origin may not *set* a Secure cookie,
    // otherwise plaintext http could overwrite the https cookie of the same host.
    if secure && !secure_scheme {
        return None;
    }

    let (domain, host_only) = match domain_attr {
        Some(d) => {
            if is_public_suffix(&d) || !domain_matches(&host, &d) {
                return None; // supercookie / cross-site attempt
            }
            (d, false)
        }
        None => (host.clone(), true),
    };

    let path = path_attr.unwrap_or_else(|| default_path(url));

    // §5.5 cookie name prefixes (RFC 6265bis): `__Secure-` and `__Host-` are opt-in
    // integrity markers. A server that names its session cookie `__Host-sid` is telling
    // the client "only accept this if it could not have been planted by a network attacker
    // on a sibling subdomain or over plaintext http" — so a client that stores one whose
    // preconditions do not hold has silently defeated the very defence the name requests.
    // The prefix match is case-insensitive per spec. Enforced here, the one chokepoint both
    // the network `Set-Cookie` path and `document.cookie` writes funnel through.
    if has_ci_prefix(name, "__Secure-") && !secure {
        return None;
    }
    if has_ci_prefix(name, "__Host-") && !(secure && host_only && path == "/") {
        // Requires Secure, no Domain attribute (host-only), and Path exactly `/`.
        return None;
    }

    // §5.2.2: Max-Age wins over Expires. Max-Age <= 0 expires immediately.
    let expires = match max_age {
        Some(secs) if secs <= 0 => Some(UNIX_EPOCH),
        Some(secs) => Some(SystemTime::now() + Duration::from_secs(secs as u64)),
        None => expires_attr,
    };

    Some(Cookie {
        name: name.to_string(),
        value,
        domain,
        host_only,
        path,
        secure,
        http_only,
        same_site,
        expires,
        creation: 0,
    })
}

/// Case-insensitive ASCII prefix test that never panics on a multi-byte cookie name — RFC
/// 6265bis §5.5 matches the `__Secure-`/`__Host-` prefixes case-insensitively.
fn has_ci_prefix(name: &str, prefix: &str) -> bool {
    let (nb, pb) = (name.as_bytes(), prefix.as_bytes());
    nb.len() >= pb.len() && nb[..pb.len()].eq_ignore_ascii_case(pb)
}

/// Parse an IMF-fixdate (`Sun, 06 Nov 1994 08:49:37 GMT`), the only `Expires` form
/// servers realistically send. Returns `None` on anything else (the cookie then
/// falls back to being a session cookie, which is the safe direction).
fn parse_http_date(s: &str) -> Option<SystemTime> {
    let s = s.trim();
    let rest = s.split_once(',').map(|(_, r)| r.trim()).unwrap_or(s);
    let mut it = rest.split_whitespace();
    let day: u32 = it.next()?.parse().ok()?;
    let mon = match it.next()?.to_ascii_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };
    let year: i64 = it.next()?.parse().ok()?;
    let hms = it.next()?;
    let mut t = hms.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let m: i64 = t.next()?.parse().ok()?;
    let sec: i64 = t.next()?.parse().ok()?;

    let days = days_from_civil(year, mon, day as i64);
    let total = days * 86400 + h * 3600 + m * 60 + sec;
    if total < 0 {
        return Some(UNIX_EPOCH);
    }
    Some(UNIX_EPOCH + Duration::from_secs(total as u64))
}

/// Howard Hinnant's `days_from_civil` — days since 1970-01-01 (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// A cookie store for one storage partition.
#[derive(Default, Debug, Clone)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
    next_creation: u64,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Persist cookies that survive a restart — those with an explicit, unexpired `expires`.
    /// Session cookies (no expiry) are intentionally dropped, matching Chromium/Gecko. Writes
    /// JSON to `path`, creating parent directories. Best-effort (returns the IO error).
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        self.save_to_at(path, SystemTime::now())
    }

    pub fn save_to_at(&self, path: &std::path::Path, now: SystemTime) -> std::io::Result<()> {
        let keep: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| c.expires.is_some() && !c.is_expired_at(now))
            .collect();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(&keep).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load persistent cookies from `path`, dropping any that have since expired. A missing
    /// or corrupt file yields an empty jar (first run / partial write are non-fatal).
    pub fn load_from(path: &std::path::Path) -> Self {
        Self::load_from_at(path, SystemTime::now())
    }

    pub fn load_from_at(path: &std::path::Path, now: SystemTime) -> Self {
        let mut jar = Self::new();
        let Ok(bytes) = std::fs::read(path) else {
            return jar;
        };
        let Ok(cookies) = serde_json::from_slice::<Vec<Cookie>>(&bytes) else {
            return jar;
        };
        for mut c in cookies {
            if c.expires.is_some() && !c.is_expired_at(now) {
                c.creation = jar.next_creation;
                jar.next_creation += 1;
                jar.cookies.push(c);
            }
        }
        jar
    }

    /// Store a `Set-Cookie` header received from `url`. Returns whether it was accepted.
    pub fn store(&mut self, url: &Url, set_cookie: &str) -> bool {
        let Some(mut c) = parse_set_cookie(url, set_cookie) else {
            return false;
        };

        // Replace an existing cookie with the same (name, domain, path) identity,
        // preserving its original creation time (§5.3 step 11).
        let existing = self
            .cookies
            .iter()
            .position(|e| e.name == c.name && e.domain == c.domain && e.path == c.path);
        match existing {
            Some(i) => {
                c.creation = self.cookies[i].creation;
                self.cookies[i] = c;
            }
            None => {
                c.creation = self.next_creation;
                self.next_creation += 1;
                self.cookies.push(c);
            }
        }
        // A cookie set with Max-Age<=0 / a past Expires is a deletion.
        self.evict_expired(SystemTime::now());
        true
    }

    fn evict_expired(&mut self, now: SystemTime) {
        self.cookies.retain(|c| !c.is_expired_at(now));
    }

    /// The `Cookie:` header value to send to `url`, or `None` if no cookie applies.
    pub fn cookie_header(&self, url: &Url) -> Option<String> {
        self.cookie_header_at(url, SystemTime::now())
    }

    /// Deterministic variant taking `now` explicitly (used by tests).
    pub fn cookie_header_at(&self, url: &Url, now: SystemTime) -> Option<String> {
        self.cookie_header_where(url, now, |_| true)
    }

    /// As [`Self::cookie_header_at`], but only cookies for which `allow` returns true
    /// are considered. `storage` uses this to apply `SameSite` with request context,
    /// which this jar cannot know on its own.
    pub fn cookie_header_where(
        &self,
        url: &Url,
        now: SystemTime,
        allow: impl Fn(&Cookie) -> bool,
    ) -> Option<String> {
        let host = host_of(url)?;
        let secure_scheme = url.scheme() == "https";
        let path = url.path();

        let mut matched: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| !c.is_expired_at(now))
            .filter(|c| allow(c))
            .filter(|c| if c.secure { secure_scheme } else { true })
            .filter(|c| {
                if c.host_only {
                    host == c.domain
                } else {
                    domain_matches(&host, &c.domain)
                }
            })
            .filter(|c| path_matches(path, &c.path))
            .collect();

        if matched.is_empty() {
            return None;
        }
        // §5.4.2: longer paths first, then earlier creation time.
        matched.sort_by(|a, b| {
            b.path
                .len()
                .cmp(&a.path.len())
                .then(a.creation.cmp(&b.creation))
        });

        Some(
            matched
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    /// All live cookies (for inspection / the task manager).
    pub fn cookies(&self) -> impl Iterator<Item = &Cookie> {
        self.cookies.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn persists_expiring_cookies_but_not_session_cookies() {
        let dir = std::env::temp_dir().join(format!("manuk-cookie-test-{}", std::process::id()));
        let path = dir.join("cookies.json");
        let _ = std::fs::remove_dir_all(&dir);

        let mut jar = CookieJar::new();
        // A persistent (expiring) login cookie, and a session cookie (no expiry).
        jar.store(&u("https://example.com/"), "login=xyz; Max-Age=1209600"); // 2 weeks
        jar.store(&u("https://example.com/"), "tmp=session");
        jar.save_to(&path).unwrap();

        // Reload: the login cookie survives; the session cookie is dropped.
        let reloaded = CookieJar::load_from(&path);
        assert_eq!(
            reloaded.cookie_header(&u("https://example.com/")),
            Some("login=xyz".to_string())
        );

        // An already-expired persistent cookie is not resurrected on load.
        let past = SystemTime::now() - std::time::Duration::from_secs(3600);
        let mut jar2 = CookieJar::new();
        jar2.store(&u("https://example.com/"), "login=xyz; Max-Age=1209600");
        jar2.save_to(&path).unwrap();
        let expired_load =
            CookieJar::load_from_at(&path, past + std::time::Duration::from_secs(3 * 1209600));
        assert_eq!(expired_load.cookie_header(&u("https://example.com/")), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stores_and_returns_a_basic_cookie() {
        let mut jar = CookieJar::new();
        assert!(jar.store(&u("https://example.com/app/page"), "sid=abc"));
        assert_eq!(
            jar.cookie_header(&u("https://example.com/app/page")),
            Some("sid=abc".to_string())
        );
        // default path is the request directory -> not sent at the root
        assert_eq!(jar.cookie_header(&u("https://example.com/")), None);
    }

    #[test]
    fn rejects_public_suffix_domain_supercookie() {
        let mut jar = CookieJar::new();
        assert!(!jar.store(&u("https://evil.com/"), "track=1; Domain=com"));
        assert!(!jar.store(&u("https://evil.co.uk/"), "track=1; Domain=co.uk"));
        assert!(jar.is_empty());
        // but a legitimate registrable domain is fine
        assert!(jar.store(&u("https://a.example.com/"), "ok=1; Domain=example.com"));
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn rejects_domain_that_does_not_match_the_request_host() {
        let mut jar = CookieJar::new();
        assert!(!jar.store(&u("https://evil.com/"), "x=1; Domain=example.com"));
        assert!(jar.is_empty());
    }

    #[test]
    fn host_only_cookie_is_not_sent_to_subdomains_but_domain_cookie_is() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "hostonly=1");
        jar.store(&u("https://example.com/"), "wide=1; Domain=example.com");

        let sub = u("https://sub.example.com/");
        let hdr = jar.cookie_header(&sub).unwrap();
        assert!(hdr.contains("wide=1"));
        assert!(!hdr.contains("hostonly=1"));
    }

    #[test]
    fn secure_cookies_never_leak_over_http() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "s=1; Secure; Path=/");
        jar.store(&u("https://example.com/"), "p=2; Path=/");
        assert_eq!(
            jar.cookie_header(&u("http://example.com/")),
            Some("p=2".to_string())
        );
        let https = jar.cookie_header(&u("https://example.com/")).unwrap();
        assert!(https.contains("s=1") && https.contains("p=2"));
    }

    /// "Leave Secure Cookies Alone": plaintext http must not be able to set (and thus
    /// overwrite) the https cookie of the same host.
    #[test]
    fn insecure_origin_cannot_set_a_secure_cookie() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "sid=good; Secure; Path=/");
        assert!(!jar.store(&u("http://example.com/"), "sid=evil; Secure; Path=/"));
        assert_eq!(
            jar.cookie_header(&u("https://example.com/")),
            Some("sid=good".to_string())
        );
    }

    #[test]
    fn same_site_none_requires_secure() {
        let mut jar = CookieJar::new();
        assert!(!jar.store(&u("http://example.com/"), "x=1; SameSite=None"));
        assert!(jar.store(&u("https://example.com/"), "y=1; SameSite=None; Secure"));
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn max_age_zero_deletes_and_beats_expires() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "k=v; Path=/");
        assert_eq!(jar.len(), 1);
        // Max-Age=0 expires immediately, even with a far-future Expires.
        jar.store(
            &u("https://example.com/"),
            "k=v; Path=/; Max-Age=0; Expires=Sun, 06 Nov 2099 08:49:37 GMT",
        );
        assert!(jar.is_empty());
    }

    #[test]
    fn expires_in_the_past_is_evicted_and_future_is_kept() {
        let mut jar = CookieJar::new();
        jar.store(
            &u("https://example.com/"),
            "old=1; Path=/; Expires=Sun, 06 Nov 1994 08:49:37 GMT",
        );
        assert!(jar.is_empty(), "past Expires must not be stored");

        jar.store(
            &u("https://example.com/"),
            "new=1; Path=/; Expires=Sun, 06 Nov 2099 08:49:37 GMT",
        );
        assert_eq!(
            jar.cookie_header(&u("https://example.com/")),
            Some("new=1".to_string())
        );
    }

    #[test]
    fn http_date_parses_to_the_right_instant() {
        // 784111777 is the canonical RFC value for this date.
        let t = parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
        assert_eq!(t.duration_since(UNIX_EPOCH).unwrap().as_secs(), 784111777);
    }

    #[test]
    fn ordering_is_longest_path_first_then_oldest() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "root=1; Path=/");
        jar.store(&u("https://example.com/"), "deep=2; Path=/a/b");
        jar.store(&u("https://example.com/"), "mid=3; Path=/a");
        let hdr = jar.cookie_header(&u("https://example.com/a/b/c")).unwrap();
        assert_eq!(hdr, "deep=2; mid=3; root=1");
    }

    #[test]
    fn path_matching_respects_segment_boundaries() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "x=1; Path=/app");
        // /application must NOT match Path=/app
        assert_eq!(
            jar.cookie_header(&u("https://example.com/application")),
            None
        );
        assert!(jar.cookie_header(&u("https://example.com/app/x")).is_some());
        assert!(jar.cookie_header(&u("https://example.com/app")).is_some());
    }

    #[test]
    fn resetting_a_cookie_preserves_creation_order() {
        let mut jar = CookieJar::new();
        jar.store(&u("https://example.com/"), "a=1; Path=/");
        jar.store(&u("https://example.com/"), "b=2; Path=/");
        jar.store(&u("https://example.com/"), "a=9; Path=/"); // update, keeps position
        assert_eq!(
            jar.cookie_header(&u("https://example.com/")),
            Some("a=9; b=2".to_string())
        );
    }

    #[test]
    fn host_and_secure_name_prefixes_are_enforced() {
        // RFC 6265bis §5.5. A `__Secure-` cookie must carry `Secure`.
        let mut jar = CookieJar::new();
        assert!(
            !jar.store(&u("https://example.com/"), "__Secure-sid=1; Path=/"),
            "__Secure- without Secure must be dropped"
        );
        assert!(jar.store(&u("https://example.com/"), "__Secure-sid=1; Secure; Path=/"));

        // A `__Host-` cookie must be Secure, host-only (no Domain), and Path=/.
        let mut jar = CookieJar::new();
        // Set from a deep URL with no explicit Path: the default-path is `/app`, not `/`,
        // so the resolved path fails the __Host- precondition and the cookie is dropped.
        assert!(
            !jar.store(&u("https://example.com/app/page"), "__Host-sid=1; Secure"),
            "__Host- with a non-root resolved path must be dropped"
        );
        assert!(
            !jar.store(
                &u("https://example.com/"),
                "__Host-sid=1; Secure; Path=/; Domain=example.com"
            ),
            "__Host- with a Domain attribute must be dropped"
        );
        assert!(
            !jar.store(&u("https://example.com/"), "__Host-sid=1; Path=/"),
            "__Host- without Secure must be dropped"
        );
        assert!(
            !jar.store(
                &u("https://example.com/"),
                "__Host-sid=1; Secure; Path=/app"
            ),
            "__Host- with a non-root Path must be dropped"
        );
        // The one well-formed shape is accepted.
        assert!(jar.store(&u("https://example.com/"), "__Host-sid=1; Secure; Path=/"));
        assert_eq!(jar.len(), 1);

        // The prefix match is case-insensitive: `__hOsT-` is still a __Host- cookie.
        let mut jar = CookieJar::new();
        assert!(
            !jar.store(&u("https://example.com/"), "__hOsT-x=1; Path=/"),
            "prefix match is case-insensitive"
        );
        assert!(jar.is_empty());

        // A plain-named cookie is unaffected by the prefix rules.
        assert!(jar.store(&u("https://example.com/"), "ordinary=1; Path=/app"));
    }
}
