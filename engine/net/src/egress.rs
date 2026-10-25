//! Private-network egress guard (W3.10) — the class of CVE-2025-49596.
//!
//! A page served from a **public** origin must not be able to make this browser talk to the
//! loopback interface, the LAN, link-local (cloud metadata at 169.254.169.254) or a unique-local
//! address. landlock cannot filter by IP, and a hostname check is defeated by DNS rebinding, so the
//! decision is made **after resolution**, on the addresses the socket would actually dial: the
//! guarded connector's resolver drops every private address while the top-level document is public,
//! and fails the connect when nothing is left. IP-literal hosts bypass the resolver inside hyper's
//! connector, so [`check_url_literal`] refuses those before the request is built.
//!
//! "Public" is a property of the top-level document ([`crate::set_top_level_document`]): a page the
//! user opened at `http://localhost:3000/` may talk to its own backend; a page at
//! `https://example.com/` may not. `--allow-private-network` / `MANUK_ALLOW_PRIVATE_NETWORK=1`
//! switches the guard off for the process.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use hyper_util::client::legacy::connect::dns::Name;
use url::{Host, Url};

static TOP_PUBLIC: AtomicBool = AtomicBool::new(false);
static ALLOW_PRIVATE: AtomicBool = AtomicBool::new(false);

/// Switch the guard off for this process (`--allow-private-network`).
pub fn set_allow_private_network(allow: bool) {
    ALLOW_PRIVATE.store(allow, Ordering::SeqCst);
}

fn allow_private() -> bool {
    static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    ALLOW_PRIVATE.load(Ordering::SeqCst)
        || *ENV.get_or_init(|| {
            std::env::var("MANUK_ALLOW_PRIVATE_NETWORK").is_ok_and(|v| !v.is_empty() && v != "0")
        })
}

/// Is the guard enforcing right now? (Top document public and no opt-out.)
pub fn guard_active() -> bool {
    TOP_PUBLIC.load(Ordering::SeqCst) && !allow_private()
}

/// Record whether the top-level document at `url` is public. Called from
/// [`crate::set_top_level_document`].
pub(crate) fn note_top_level(url: &str) {
    TOP_PUBLIC.store(is_public_document(url), Ordering::SeqCst);
}

/// A document is public when it is http(s) and its host is neither a private literal nor a
/// local-looking name (`localhost`, `*.localhost`, `*.local`, a dotless intranet name).
pub fn is_public_document(url: &str) -> bool {
    let Ok(u) = Url::parse(url) else { return false };
    if !matches!(u.scheme(), "http" | "https") {
        return false;
    }
    match u.host() {
        Some(Host::Ipv4(a)) => !is_private_ip(IpAddr::V4(a)),
        Some(Host::Ipv6(a)) => !is_private_ip(IpAddr::V6(a)),
        Some(Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d.contains('.')
                && d != "localhost"
                && !d.ends_with(".localhost")
                && !d.ends_with(".local")
        }
        None => false,
    }
}

/// Loopback, RFC1918, CGNAT, link-local, unspecified, broadcast, ULA (and v4-mapped forms of them).
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => is_private_v4(a),
        IpAddr::V6(a) => {
            if let Some(m) = a.to_ipv4_mapped() {
                return is_private_v4(m);
            }
            let s = a.segments();
            a == Ipv6Addr::LOCALHOST
                || a == Ipv6Addr::UNSPECIFIED
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}

fn is_private_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_loopback()
        || a.is_private()
        || a.is_link_local()
        || a.is_unspecified()
        || a.is_broadcast()
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10 CGNAT
}

/// Refuse an IP-literal URL host (hyper never asks the resolver about those).
pub fn check_url_literal(url: &Url) -> anyhow::Result<()> {
    if !guard_active() {
        return Ok(());
    }
    let ip = match url.host() {
        Some(Host::Ipv4(a)) => IpAddr::V4(a),
        Some(Host::Ipv6(a)) => IpAddr::V6(a),
        _ => return Ok(()),
    };
    if is_private_ip(ip) {
        anyhow::bail!("blocked: {url} is a private-network address and the page is public (--allow-private-network overrides)");
    }
    Ok(())
}

/// Resolve `host:port` for a raw socket dial (WebSocket, proxy target) with the same post-DNS rule
/// as [`GuardResolver`]: while the guard is active, private addresses are dropped and an all-private
/// answer fails. The caller dials the returned addresses, not the name, so a rebind cannot swap
/// the answer between check and connect.
pub async fn resolve_for_dial(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    // IP literals (including bracketed v6) skip DNS but not the check.
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = match bare.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host, port)).await?.collect(),
    };
    if !guard_active() {
        return Ok(addrs);
    }
    let total = addrs.len();
    let ok: Vec<SocketAddr> = addrs
        .into_iter()
        .filter(|a| !is_private_ip(a.ip()))
        .collect();
    if ok.is_empty() && total > 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("blocked: {host} is a private-network address and the page is public (--allow-private-network overrides)"),
        ));
    }
    Ok(ok)
}

/// The connector's DNS resolver. `guarded == false` is the embedder's own client (LLM endpoint,
/// MCP plumbing), which is allowed to reach loopback.
#[derive(Clone, Copy)]
pub struct GuardResolver {
    pub guarded: bool,
}

impl tower_service::Service<Name> for GuardResolver {
    type Response = std::vec::IntoIter<SocketAddr>;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        let guarded = self.guarded;
        Box::pin(async move {
            let t = std::time::Instant::now();
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((name.as_str(), 0)).await?.collect();
            if crate::nettrace::enabled() {
                crate::nettrace::emit(
                    "dns",
                    &format!("{}\t{}", name.as_str(), t.elapsed().as_micros()),
                );
            }
            if !guarded || !guard_active() {
                return Ok(addrs.into_iter());
            }
            let total = addrs.len();
            let ok: Vec<SocketAddr> = addrs
                .into_iter()
                .filter(|a| !is_private_ip(a.ip()))
                .collect();
            if ok.is_empty() && total > 0 {
                tracing::warn!(
                    host = name.as_str(),
                    "egress guard: host resolves only to private addresses"
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "blocked: {} resolves to a private-network address",
                        name.as_str()
                    ),
                ));
            }
            Ok(ok.into_iter())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_addresses() {
        for s in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(is_private_ip(s.parse().unwrap()), "{s} should be private");
        }
        for s in [
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "93.184.216.34",
            "2606:4700::1111",
        ] {
            assert!(!is_private_ip(s.parse().unwrap()), "{s} should be public");
        }
    }

    #[test]
    fn classifies_documents() {
        assert!(is_public_document("https://example.com/"));
        assert!(is_public_document("http://public.test:8080/"));
        assert!(is_public_document("http://8.8.8.8/"));
        assert!(!is_public_document("http://localhost:3000/"));
        assert!(!is_public_document("http://127.0.0.1/"));
        assert!(!is_public_document("http://192.168.0.5/"));
        assert!(!is_public_document("http://intranet/"));
        assert!(!is_public_document("file:///tmp/x.html"));
    }
}
