//! Content-Security-Policy — the author-declared restriction on what a document may execute.
//!
//! CSP is the mechanism by which a site tells the browser *"even if an injection lands in my HTML,
//! do not run it"*. Without enforcement, that declaration is decoration: a page that ships
//! `script-src 'self'` and is then XSS'd gets exactly the same outcome as a page with no policy at
//! all, because the attacker's `<script>` runs either way. Every real site that serves a CSP —
//! GitHub, Google, every bank — is relying on the browser to be the enforcing party. We were not
//! one, so this module is the difference between honouring the header and merely receiving it.
//!
//! **What this module is, and is not.** It is the *evaluator*: a pure function from a parsed policy
//! plus a request (an inline script with an optional nonce, or a URL to load) to allow/deny. The
//! call sites — `manuk-page` for document scripts, the JS host hook for inline execution — make the
//! decision; nothing here touches the network or the DOM. It implements **`script-src`** (with
//! `default-src` fallback), which is the directive that carries CSP's whole security argument.
//! `style-src`, `img-src`, `connect-src`, `frame-ancestors` and reporting are **not** implemented,
//! and are deliberately absent rather than stubbed: a directive that parses but never blocks is
//! indistinguishable, from the page's side, from one that is enforced, and that is the exact class
//! of lie this project keeps catching. [`Csp::restricts_scripts`] reports honestly which of the two
//! situations a caller is in.
//!
//! **Failing open vs failing closed.** A policy we cannot parse must not silently permit
//! everything, and must not break a page that would have worked. The rule here is: an *absent*
//! `script-src`/`default-src` allows (no policy was expressed), a *present* one allows only what it
//! names, and an unrecognized source expression matches nothing (so it neither grants nor revokes).

use url::Url;

/// One parsed `Content-Security-Policy` header value: directive name → source-expression list.
///
/// Kept as the raw expressions rather than a pre-resolved allow/deny set, because whether a source
/// matches depends on the *request* (`'self'` means nothing without a document origin, and
/// `'nonce-…'` means nothing without the element's attribute).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Policy {
    directives: Vec<(String, Vec<String>)>,
}

impl Policy {
    /// Parse one header value: `directive src src; directive src`. Directive names are
    /// ASCII-lowercased (they are case-insensitive); source expressions keep their case, because
    /// a nonce is compared byte-for-byte and a path is case-sensitive.
    ///
    /// A repeated directive keeps the **first** occurrence and ignores the rest, per spec — which
    /// matters, because the alternative (last wins) would let an injected trailing directive
    /// loosen the policy it was supposed to be constrained by.
    pub fn parse(header: &str) -> Policy {
        let mut directives: Vec<(String, Vec<String>)> = Vec::new();
        for chunk in header.split(';') {
            let mut parts = chunk.split_whitespace();
            let Some(name) = parts.next() else { continue };
            let name = name.to_ascii_lowercase();
            if directives.iter().any(|(n, _)| *n == name) {
                continue;
            }
            directives.push((name, parts.map(|s| s.to_string()).collect()));
        }
        Policy { directives }
    }

    fn get(&self, name: &str) -> Option<&[String]> {
        self.directives
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_slice())
    }

    /// The source list that governs scripts: `script-src`, else `default-src`, else none at all
    /// (which means *unrestricted* — not "deny").
    fn script_sources(&self) -> Option<&[String]> {
        self.get("script-src").or_else(|| self.get("default-src"))
    }
}

/// Every policy applying to one document — response headers and `<meta>` combined.
///
/// Multiple policies are **conjunctive**: a load must be allowed by *all* of them. That is the one
/// composition rule CSP has, and it is why a `<meta>` policy can only ever tighten a header policy,
/// never loosen it — which in turn is why it is safe to honour a policy that arrived in the markup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Csp {
    policies: Vec<Policy>,
    /// The document's own URL, for resolving `'self'`. Absent for a document with an opaque origin
    /// (`data:`, `about:`), where `'self'` matches nothing.
    document_url: Option<Url>,
}

impl Csp {
    /// An empty policy set — imposes nothing. This is what a document with no CSP gets, and it is
    /// the value every path must default to, so that adding CSP support cannot break a page that
    /// was working.
    pub fn none() -> Csp {
        Csp::default()
    }

    /// Collect every `Content-Security-Policy` response header. `Content-Security-Policy-Report-Only`
    /// is deliberately **skipped**: it is defined to observe without enforcing, so honouring it
    /// would block loads the site explicitly asked us not to block. (We do not report, either —
    /// [`Csp::restricts_scripts`] is how a caller learns that.)
    pub fn from_headers(headers: &[(String, String)], document_url: Option<&Url>) -> Csp {
        let policies = headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-security-policy"))
            .map(|(_, v)| Policy::parse(v))
            .collect();
        Csp {
            policies,
            document_url: document_url.cloned(),
        }
    }

    /// Add a policy delivered by `<meta http-equiv="Content-Security-Policy" content="…">`.
    pub fn add_meta(&mut self, content: &str) {
        self.policies.push(Policy::parse(content));
    }

    pub fn set_document_url(&mut self, url: Option<&Url>) {
        self.document_url = url.cloned();
    }

    /// Does any policy actually constrain script loading? `false` means every script question below
    /// answers "allow" — the honest way for a caller to distinguish *"CSP permitted this"* from
    /// *"there was no CSP"*, which are the two things a stubbed implementation would conflate.
    pub fn restricts_scripts(&self) -> bool {
        self.policies.iter().any(|p| p.script_sources().is_some())
    }

    /// May an **inline** `<script>` with this `nonce` attribute execute?
    ///
    /// The nonce interaction is the subtle part and the reason CSP works at all on real sites: when
    /// a source list carries any `'nonce-…'` (or hash) source, `'unsafe-inline'` in that same list
    /// is **ignored**. Sites ship both on purpose — the `'unsafe-inline'` is a fallback for old
    /// browsers, and a browser that honoured both would silently downgrade every nonce-based policy
    /// on the web to no policy at all.
    pub fn allows_inline_script(&self, nonce: Option<&str>) -> bool {
        self.policies.iter().all(|p| policy_allows_inline(p, nonce))
    }

    /// May a `<script src>` pointing at `url` be fetched and run?
    ///
    /// The check happens **before the request is issued**, not after the response lands: CSP is not
    /// only about execution, it is about not making the request at all. A blocked script that is
    /// still fetched leaks the visit to the attacker's server, which is half of what the policy was
    /// written to prevent.
    pub fn allows_script_url(&self, url: &Url) -> bool {
        self.policies
            .iter()
            .all(|p| policy_allows_url(p, url, self.document_url.as_ref()))
    }
}

fn policy_allows_inline(policy: &Policy, nonce: Option<&str>) -> bool {
    let Some(sources) = policy.script_sources() else {
        return true; // no script-src and no default-src — this policy says nothing about scripts
    };
    let has_nonce_or_hash = sources.iter().any(|s| {
        let s = s.trim_matches('\'');
        s.starts_with("nonce-")
            || s.starts_with("sha256-")
            || s.starts_with("sha384-")
            || s.starts_with("sha512-")
    });
    if let Some(n) = nonce.filter(|n| !n.is_empty()) {
        if sources
            .iter()
            .any(|s| s.trim_matches('\'').strip_prefix("nonce-") == Some(n))
        {
            return true;
        }
    }
    if has_nonce_or_hash {
        return false; // 'unsafe-inline' is ignored in the presence of a nonce/hash source
    }
    sources
        .iter()
        .any(|s| s.eq_ignore_ascii_case("'unsafe-inline'"))
}

fn policy_allows_url(policy: &Policy, url: &Url, document_url: Option<&Url>) -> bool {
    let Some(sources) = policy.script_sources() else {
        return true;
    };
    // A present-but-empty source list, and `'none'`, both mean: nothing matches.
    if sources.is_empty() || sources.iter().any(|s| s.eq_ignore_ascii_case("'none'")) {
        return false;
    }
    sources.iter().any(|s| source_matches(s, url, document_url))
}

/// Does one source expression match `url`? Unrecognized expressions match nothing — they cannot
/// grant, and (because matching is a disjunction over the list) they cannot revoke either.
fn source_matches(source: &str, url: &Url, document_url: Option<&Url>) -> bool {
    // Keyword sources are the quoted ones; none of them are host patterns.
    if source.starts_with('\'') {
        if source.eq_ignore_ascii_case("'self'") {
            return match document_url {
                Some(doc) => same_origin_allowing_upgrade(doc, url),
                None => false, // opaque document origin — 'self' names nothing
            };
        }
        // 'unsafe-inline' / 'unsafe-eval' / 'nonce-…' / 'sha…-…' do not authorize a URL load.
        return false;
    }
    if source == "*" {
        // The wildcard covers network schemes only — it never authorizes `data:` or `blob:`,
        // which is what stops `*` from re-opening the `data:` script injection it looks like it
        // would close.
        return matches!(url.scheme(), "http" | "https" | "ws" | "wss" | "ftp");
    }
    // Scheme-source: `https:` (a trailing colon, no host).
    if let Some(scheme) = source.strip_suffix(':') {
        if !scheme.contains('/') {
            return scheme_matches(scheme, url.scheme());
        }
    }

    // Host-source: [scheme "://"] host [":" port] [path]
    let (src_scheme, rest) = match source.split_once("://") {
        Some((s, r)) => (Some(s), r),
        None => (None, source),
    };
    if let Some(s) = src_scheme {
        if !scheme_matches(s, url.scheme()) {
            return false;
        }
    } else if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        // A bare host-source only ever matches a network-scheme URL.
        return false;
    }
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], Some(&rest[i..])),
        None => (rest, None),
    };
    let (src_host, src_port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (hostport, None),
    };
    let Some(url_host) = url.host_str() else {
        return false;
    };
    if !host_matches(src_host, url_host) {
        return false;
    }
    if let Some(p) = src_port {
        if p != "*" {
            let want: u16 = match p.parse() {
                Ok(w) => w,
                Err(_) => return false,
            };
            if url.port_or_known_default() != Some(want) {
                return false;
            }
        }
    } else if src_scheme.is_none() {
        // No port given and no scheme given: the source's default port is the URL's scheme's
        // default, so an explicit non-default port does NOT match. This is what keeps
        // `example.com` from authorizing `example.com:8080`.
        if url.port().is_some() {
            return false;
        }
    }
    match path {
        // A source path is a prefix match, except that a path not ending in `/` must match the
        // URL path exactly (otherwise `/a` would authorize `/attacker.js`).
        Some(p) if p.ends_with('/') => url.path().starts_with(p),
        Some(p) => url.path() == p,
        None => true,
    }
}

/// A source scheme matches the URL's scheme, plus the spec's one-way **upgrade**: an `http` source
/// also authorizes `https` (and `ws` authorizes `wss`). The reverse never holds — a policy naming
/// `https` must not be satisfied by a plaintext load.
fn scheme_matches(source_scheme: &str, url_scheme: &str) -> bool {
    let s = source_scheme.to_ascii_lowercase();
    if s == url_scheme {
        return true;
    }
    matches!(
        (s.as_str(), url_scheme),
        ("http", "https") | ("ws", "wss") | ("http", "ws") | ("http", "wss")
    )
}

/// `*.example.com` matches any subdomain but **not** `example.com` itself, and never matches by
/// mere suffix (`notexample.com` must not match `*.example.com`).
fn host_matches(source_host: &str, url_host: &str) -> bool {
    if let Some(suffix) = source_host.strip_prefix("*.") {
        return url_host.len() > suffix.len() + 1
            && url_host
                .to_ascii_lowercase()
                .ends_with(&format!(".{}", suffix.to_ascii_lowercase()));
    }
    source_host.eq_ignore_ascii_case(url_host)
}

/// `'self'` — same scheme, host and port as the document, with the same http→https upgrade
/// allowance the spec grants (a document on `http:` may load `https:` from its own host).
fn same_origin_allowing_upgrade(doc: &Url, url: &Url) -> bool {
    let (Some(dh), Some(uh)) = (doc.host_str(), url.host_str()) else {
        return false;
    };
    if !dh.eq_ignore_ascii_case(uh) {
        return false;
    }
    if !scheme_matches(doc.scheme(), url.scheme()) {
        return false;
    }
    doc.port_or_known_default() == url.port_or_known_default() || doc.scheme() != url.scheme()
    // an upgraded scheme carries its own default port
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }
    fn csp(header: &str, doc: &str) -> Csp {
        Csp::from_headers(
            &[("content-security-policy".to_string(), header.to_string())],
            Some(&u(doc)),
        )
    }

    #[test]
    fn no_policy_restricts_nothing() {
        let c = Csp::none();
        assert!(!c.restricts_scripts());
        assert!(c.allows_inline_script(None));
        assert!(c.allows_script_url(&u("https://evil.example/x.js")));
    }

    #[test]
    fn a_policy_without_script_or_default_src_says_nothing_about_scripts() {
        let c = csp("img-src 'none'", "https://a.example/");
        assert!(!c.restricts_scripts());
        assert!(c.allows_inline_script(None));
        assert!(c.allows_script_url(&u("https://evil.example/x.js")));
    }

    #[test]
    fn script_src_self_allows_own_origin_and_blocks_others() {
        let c = csp("script-src 'self'", "https://a.example/page");
        assert!(c.restricts_scripts());
        assert!(c.allows_script_url(&u("https://a.example/app.js")));
        assert!(!c.allows_script_url(&u("https://evil.example/x.js")));
        // 'self' does not authorize inline
        assert!(!c.allows_inline_script(None));
    }

    #[test]
    fn default_src_is_the_fallback_for_script_src() {
        let c = csp("default-src 'self'", "https://a.example/");
        assert!(c.restricts_scripts());
        assert!(c.allows_script_url(&u("https://a.example/app.js")));
        assert!(!c.allows_script_url(&u("https://evil.example/x.js")));
    }

    #[test]
    fn script_src_overrides_default_src() {
        let c = csp(
            "default-src 'none'; script-src 'self'",
            "https://a.example/",
        );
        assert!(c.allows_script_url(&u("https://a.example/app.js")));
    }

    #[test]
    fn none_blocks_everything() {
        let c = csp("script-src 'none'", "https://a.example/");
        assert!(!c.allows_script_url(&u("https://a.example/app.js")));
        assert!(!c.allows_inline_script(None));
    }

    #[test]
    fn unsafe_inline_allows_inline() {
        let c = csp("script-src 'unsafe-inline'", "https://a.example/");
        assert!(c.allows_inline_script(None));
        // …but still authorizes no URL
        assert!(!c.allows_script_url(&u("https://a.example/app.js")));
    }

    #[test]
    fn nonce_matches_only_the_exact_value() {
        let c = csp("script-src 'nonce-abc123'", "https://a.example/");
        assert!(c.allows_inline_script(Some("abc123")));
        assert!(!c.allows_inline_script(Some("abc124")));
        assert!(!c.allows_inline_script(None));
        assert!(!c.allows_inline_script(Some("")));
    }

    #[test]
    fn a_nonce_source_makes_unsafe_inline_ignored() {
        // The whole reason nonce-based CSP is worth anything: sites ship 'unsafe-inline' as a
        // legacy fallback, and honouring both would downgrade every such policy to no policy.
        let c = csp(
            "script-src 'unsafe-inline' 'nonce-abc123'",
            "https://a.example/",
        );
        assert!(c.allows_inline_script(Some("abc123")));
        assert!(!c.allows_inline_script(None));
    }

    #[test]
    fn a_hash_source_also_makes_unsafe_inline_ignored() {
        let c = csp(
            "script-src 'unsafe-inline' 'sha256-AAAA'",
            "https://a.example/",
        );
        assert!(!c.allows_inline_script(None));
    }

    #[test]
    fn host_source_matches_host_and_subdomain_wildcard() {
        let c = csp("script-src cdn.example.com", "https://a.example/");
        assert!(c.allows_script_url(&u("https://cdn.example.com/j.js")));
        assert!(!c.allows_script_url(&u("https://other.example.com/j.js")));

        let w = csp("script-src *.example.com", "https://a.example/");
        assert!(w.allows_script_url(&u("https://cdn.example.com/j.js")));
        // the bare apex is NOT covered by *.example.com
        assert!(!w.allows_script_url(&u("https://example.com/j.js")));
        // and a suffix collision must not match
        assert!(!w.allows_script_url(&u("https://notexample.com/j.js")));
    }

    #[test]
    fn scheme_source_and_the_one_way_upgrade() {
        let c = csp("script-src https:", "https://a.example/");
        assert!(c.allows_script_url(&u("https://anywhere.example/j.js")));
        assert!(!c.allows_script_url(&u("http://anywhere.example/j.js")));

        // an http source authorizes https, never the reverse
        let h = csp("script-src http://a.example", "http://a.example/");
        assert!(h.allows_script_url(&u("https://a.example/j.js")));
    }

    #[test]
    fn wildcard_does_not_authorize_data_urls() {
        let c = csp("script-src *", "https://a.example/");
        assert!(c.allows_script_url(&u("https://anywhere.example/j.js")));
        assert!(!c.allows_script_url(&u("data:text/javascript,alert(1)")));
    }

    #[test]
    fn an_explicit_port_must_match() {
        let c = csp("script-src a.example", "https://a.example/");
        assert!(c.allows_script_url(&u("https://a.example/j.js")));
        assert!(!c.allows_script_url(&u("https://a.example:8080/j.js")));

        let p = csp("script-src a.example:8080", "https://a.example/");
        assert!(p.allows_script_url(&u("https://a.example:8080/j.js")));
        assert!(!p.allows_script_url(&u("https://a.example/j.js")));

        let star = csp("script-src a.example:*", "https://a.example/");
        assert!(star.allows_script_url(&u("https://a.example:8080/j.js")));
        assert!(star.allows_script_url(&u("https://a.example/j.js")));
    }

    #[test]
    fn a_source_path_is_a_prefix_only_when_it_ends_in_a_slash() {
        let dir = csp("script-src cdn.example/lib/", "https://a.example/");
        assert!(dir.allows_script_url(&u("https://cdn.example/lib/j.js")));
        assert!(!dir.allows_script_url(&u("https://cdn.example/other/j.js")));
        // the classic bypass: /lib must not authorize /library-of-evil.js
        let exact = csp("script-src cdn.example/lib", "https://a.example/");
        assert!(exact.allows_script_url(&u("https://cdn.example/lib")));
        assert!(!exact.allows_script_url(&u("https://cdn.example/library-of-evil.js")));
    }

    #[test]
    fn multiple_policies_are_conjunctive() {
        let mut c = Csp::from_headers(
            &[
                (
                    "content-security-policy".to_string(),
                    "script-src 'self' cdn.example".to_string(),
                ),
                (
                    "content-security-policy".to_string(),
                    "script-src 'self'".to_string(),
                ),
            ],
            Some(&u("https://a.example/")),
        );
        assert!(c.allows_script_url(&u("https://a.example/j.js")));
        // allowed by the first policy, denied by the second → denied
        assert!(!c.allows_script_url(&u("https://cdn.example/j.js")));

        // a <meta> policy can only tighten
        c.add_meta("script-src 'none'");
        assert!(!c.allows_script_url(&u("https://a.example/j.js")));
    }

    #[test]
    fn report_only_is_not_enforced() {
        let c = Csp::from_headers(
            &[(
                "content-security-policy-report-only".to_string(),
                "script-src 'none'".to_string(),
            )],
            Some(&u("https://a.example/")),
        );
        assert!(!c.restricts_scripts());
        assert!(c.allows_script_url(&u("https://evil.example/x.js")));
    }

    #[test]
    fn a_repeated_directive_keeps_the_first() {
        // Last-wins would let an appended directive loosen the policy it was meant to constrain.
        let c = csp("script-src 'self'; script-src *", "https://a.example/");
        assert!(!c.allows_script_url(&u("https://evil.example/x.js")));
    }

    #[test]
    fn directive_names_are_case_insensitive() {
        let c = csp("Script-Src 'self'", "https://a.example/");
        assert!(c.restricts_scripts());
        assert!(!c.allows_script_url(&u("https://evil.example/x.js")));
    }
}
