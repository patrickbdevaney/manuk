//! CORS — the cross-origin read barrier for page-issued `fetch()`/XHR subresource requests.
//!
//! A page may always read a **same-origin** response. A **cross-origin** response, however, is
//! only readable when the server opts in with an `Access-Control-Allow-Origin` (ACAO) header that
//! names the page's origin (or `*`). This is the load-bearing half of the same-origin policy: it
//! is why `fetch('https://api.other.example/data')` from `https://app.example/` returns the body
//! only when `api.other.example` said it may, and why a page cannot silently read a logged-in
//! user's bank statement from a script.
//!
//! **What this module is, and is not.** It implements the *response* CORS check — the decision
//! made once a cross-origin response has landed. A blocked read is surfaced to the page exactly as
//! Chromium surfaces it: as a **network failure** (the host settles the request with `status == 0`,
//! and the page's `fetch()` Promise rejects with a `TypeError`). It does **not** yet implement the
//! CORS *preflight* (the `OPTIONS` round-trip a non-simple request — custom headers, a non-simple
//! method — must pass before it is even sent). Preflight is a documented follow-on; the response
//! check is the part that gates the common case (a simple cross-origin GET to a JSON API), and
//! without it every cross-origin body leaked to the page.

use url::Url;

/// The ASCII-serialized tuple origin of `url` — `scheme://host[:port]`, with a default port
/// (`:80` for http, `:443` for https, …) omitted. `None` for a URL with an *opaque* origin
/// (`data:`, `blob:`, `file:` — no host to compare), which callers treat as cross-origin.
pub fn origin_of(url: &Url) -> Option<String> {
    let origin = url.origin();
    origin.is_tuple().then(|| origin.ascii_serialization())
}

/// Is `request_url` a **different** origin than the page running at `page_origin` (an
/// already-serialized origin string, e.g. from [`origin_of`])? A request URL with no tuple origin,
/// or a `page_origin` that does not match it byte-for-byte, is cross-origin. Fails **closed**: an
/// unrecognizable request origin counts as cross-origin, never same-origin.
pub fn is_cross_origin(page_origin: &str, request_url: &Url) -> bool {
    match origin_of(request_url) {
        Some(req) => req != page_origin,
        None => true,
    }
}

/// May a page at `page_origin` **read** the response to its `fetch()`/XHR of `request_url`?
///
/// - **Same-origin** → always `true`; CORS does not apply.
/// - **Cross-origin** → only when the response's `Access-Control-Allow-Origin` permits the read,
///   per the Fetch standard's CORS check:
///   - ACAO `*` allows an **uncredentialed** request; a credentialed one may **not** ride a
///     wildcard (the spec forbids `*` with credentials — the server must echo the exact origin).
///   - ACAO `<origin>` must equal `page_origin` (byte comparison of the serialized origin).
///   - A **credentialed** request additionally requires
///     `Access-Control-Allow-Credentials: true`.
///   - A missing or blank ACAO **blocks** — a server that says nothing has not opted in.
pub fn fetch_response_readable(
    page_origin: &str,
    request_url: &Url,
    response_headers: &[(String, String)],
    with_credentials: bool,
) -> bool {
    if !is_cross_origin(page_origin, request_url) {
        return true;
    }
    let header = |name: &str| {
        response_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
    };
    let acao = match header("access-control-allow-origin") {
        Some(v) if !v.is_empty() => v,
        _ => return false,
    };
    let credentials_allowed = || {
        header("access-control-allow-credentials").is_some_and(|v| v.eq_ignore_ascii_case("true"))
    };
    if acao == "*" {
        // The wildcard cannot carry credentials.
        return !with_credentials;
    }
    if acao != page_origin {
        return false;
    }
    // Exact-origin echo: readable uncredentialed; a credentialed read also needs ACAC: true.
    !with_credentials || credentials_allowed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }
    fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn origin_serialization_omits_default_ports() {
        assert_eq!(
            origin_of(&u("https://app.example/x")).unwrap(),
            "https://app.example"
        );
        assert_eq!(
            origin_of(&u("https://app.example:443/x")).unwrap(),
            "https://app.example"
        );
        assert_eq!(
            origin_of(&u("http://app.example/x")).unwrap(),
            "http://app.example"
        );
        // A non-default port IS part of the origin.
        assert_eq!(
            origin_of(&u("https://app.example:8443/x")).unwrap(),
            "https://app.example:8443"
        );
        // Opaque origins have no tuple to compare.
        assert_eq!(origin_of(&u("data:text/plain,hi")), None);
    }

    #[test]
    fn cross_origin_is_scheme_host_and_port() {
        let page = "https://app.example";
        assert!(!is_cross_origin(page, &u("https://app.example/a/b"))); // same origin, other path
        assert!(is_cross_origin(page, &u("http://app.example/"))); // scheme differs
        assert!(is_cross_origin(page, &u("https://api.app.example/"))); // host differs
        assert!(is_cross_origin(page, &u("https://app.example:8443/"))); // port differs
        assert!(is_cross_origin(page, &u("data:text/plain,x"))); // opaque → cross-origin
    }

    #[test]
    fn same_origin_read_ignores_cors_headers() {
        // No ACAO at all, yet a same-origin fetch is readable — CORS does not gate it.
        assert!(fetch_response_readable(
            "https://app.example",
            &u("https://app.example/data.json"),
            &h(&[]),
            false,
        ));
    }

    #[test]
    fn cross_origin_without_acao_is_blocked() {
        // The pre-CORS behavior — a cross-origin body with no opt-in — is now refused.
        assert!(!fetch_response_readable(
            "https://app.example",
            &u("https://api.other.example/v1/data"),
            &h(&[("content-type", "application/json")]),
            false,
        ));
    }

    #[test]
    fn wildcard_allows_uncredentialed_only() {
        let url = u("https://api.other.example/v1/data");
        assert!(fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[("access-control-allow-origin", "*")]),
            false,
        ));
        // `*` cannot carry credentials.
        assert!(!fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[("access-control-allow-origin", "*")]),
            true,
        ));
    }

    #[test]
    fn exact_origin_echo_allows_and_matches_precisely() {
        let url = u("https://api.other.example/v1/data");
        assert!(fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[("Access-Control-Allow-Origin", "https://app.example")]),
            false,
        ));
        // A different origin echoed → blocked (a server echoing the WRONG origin does not help).
        assert!(!fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[("access-control-allow-origin", "https://evil.example")]),
            false,
        ));
    }

    #[test]
    fn credentialed_cross_origin_needs_allow_credentials_and_exact_origin() {
        let url = u("https://api.other.example/v1/data");
        // Exact origin but no ACAC → a credentialed read is blocked.
        assert!(!fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[("access-control-allow-origin", "https://app.example")]),
            true,
        ));
        // Exact origin + ACAC: true → allowed.
        assert!(fetch_response_readable(
            "https://app.example",
            &url,
            &h(&[
                ("access-control-allow-origin", "https://app.example"),
                ("access-control-allow-credentials", "true"),
            ]),
            true,
        ));
    }
}
