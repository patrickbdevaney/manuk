//! W3.16: secret handles. The agent names a credential (`type(handle, secret='bank')`); the bytes come from
//! the operator's store and never enter the transcript, the action log or any observation.
//!
//! * Providers: `op://…` references via the 1Password CLI (`op read`), libsecret via `secret-tool lookup
//!   service manuk account <name>`, and `MANUK_SECRET_<NAME>` (tests, CI). `<name>.totp` computes the RFC 6238
//!   code host-side from the seed stored under that name; the model never sees the seed.
//! * Origin binding: a secret is bound to the origin of the page it is first typed into. Any page-initiated
//!   request (URL, headers, body, WebSocket frame, beacon) carrying the bytes, raw or encoded, toward
//!   another origin is refused, and the refusal is logged (`blocked()`).
//! * `redact` replaces the bytes with `•••` in everything the tool layer returns.

use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use ring::hmac;

struct Secret {
    name: String,
    value: String,
    origin: Option<String>,
}

static REG: Mutex<Vec<Secret>> = Mutex::new(Vec::new());
static BLOCKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub const MASK: &str = "•••";

pub fn active() -> bool {
    REG.lock().map_or(false, |r| !r.is_empty())
}

/// Every refusal so far (`EGRESS-DENIED (secret) …`), oldest first.
pub fn blocked() -> Vec<String> {
    BLOCKED.lock().map(|b| b.clone()).unwrap_or_default()
}

fn env_key(name: &str) -> String {
    let n: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("MANUK_SECRET_{n}")
}

fn lookup(name: &str) -> Result<String> {
    if let Ok(v) = std::env::var(env_key(name)) {
        return Ok(v);
    }
    let out = if name.starts_with("op://") {
        std::process::Command::new("op")
            .args(["read", name])
            .output()
    } else {
        std::process::Command::new("secret-tool")
            .args(["lookup", "service", "manuk", "account", name])
            .output()
    };
    match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            Ok(String::from_utf8_lossy(&o.stdout)
                .trim_end_matches('\n')
                .to_string())
        }
        _ => bail!("no secret named {name:?}: set {} or store it in the OS keyring (secret-tool store service manuk account {name}) or give an op:// reference", env_key(name)),
    }
}

fn base32(s: &str) -> Option<Vec<u8>> {
    let (mut buf, mut bits, mut out) = (0u32, 0, Vec::new());
    for c in s.bytes().filter(|c| !matches!(c, b' ' | b'=' | b'-')) {
        let v = match c.to_ascii_uppercase() {
            c @ b'A'..=b'Z' => c - b'A',
            c @ b'2'..=b'7' => c - b'2' + 26,
            _ => return None,
        } as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// RFC 6238 (SHA-1, 30 s, 6 digits) at unix time `t`.
pub fn totp(seed_b32: &str, t: u64) -> Result<String> {
    let key = base32(seed_b32).context("TOTP seed is not base32")?;
    let k = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key);
    let tag = hmac::sign(&k, &(t / 30).to_be_bytes());
    let h = tag.as_ref();
    let o = (h[19] & 15) as usize;
    let n = u32::from_be_bytes([h[o] & 0x7f, h[o + 1], h[o + 2], h[o + 3]]);
    Ok(format!("{:06}", n % 1_000_000))
}

/// Fetch the bytes for `spec` (`name`, or `name.totp` for the current one-time code) and remember them so
/// they are redacted and origin-checked from now on. The value is returned for the one type action.
pub fn resolve(spec: &str) -> Result<String> {
    let value = match spec.strip_suffix(".totp") {
        Some(base) => {
            let seed = lookup(spec).or_else(|_| lookup(base))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            totp(&seed, now)?
        }
        None => lookup(spec)?,
    };
    if value.is_empty() {
        bail!("secret {spec:?} is empty");
    }
    let mut r = REG.lock().map_err(|_| anyhow::anyhow!("secret registry"))?;
    match r.iter_mut().find(|s| s.name == spec) {
        Some(s) => s.value = value.clone(),
        None => r.push(Secret {
            name: spec.to_string(),
            value: value.clone(),
            origin: None,
        }),
    }
    Ok(value)
}

/// Bind `spec` to the origin of `url` on first use; later uses elsewhere are refused.
pub fn bind(spec: &str, url: &str) -> Result<()> {
    let o = crate::taint::origin(url).context("cannot bind a secret to a page with no origin")?;
    let mut r = REG.lock().map_err(|_| anyhow::anyhow!("secret registry"))?;
    let s = r
        .iter_mut()
        .find(|s| s.name == spec)
        .context("secret not resolved")?;
    match &s.origin {
        None => s.origin = Some(o),
        Some(b) if *b != o => bail!(
            "EGRESS-DENIED (secret): {spec:?} belongs to {b}; this page is {o}. Nothing was typed"
        ),
        _ => {}
    }
    Ok(())
}

/// May a page-initiated send of `payload` go to `dest_url`? `Err` is the logged refusal.
pub fn check(payload: &str, dest_url: &str, what: &str) -> Result<(), String> {
    let Ok(r) = REG.lock() else { return Ok(()) };
    // Gate mutation only: the guard off, to prove the fixture can leak.
    if r.is_empty() || std::env::var_os("MANUK_SECRET_MUTATE").is_some() {
        return Ok(());
    }
    let dest = crate::taint::origin(dest_url);
    let vs = crate::taint::views(payload);
    for s in r.iter() {
        if s.origin.is_some() && s.origin == dest {
            continue;
        }
        let needle = s.value.to_ascii_lowercase();
        if vs.iter().any(|v| v.contains(&needle)) {
            let msg = format!(
                "EGRESS-DENIED (secret): the {what} carries secret {:?} toward {}; it is bound to {}. Nothing was sent",
                s.name,
                dest.as_deref().unwrap_or(dest_url),
                s.origin.as_deref().unwrap_or("(unbound)")
            );
            drop(r);
            eprintln!("manuk: {msg}");
            if let Ok(mut b) = BLOCKED.lock() {
                b.push(msg.clone());
            }
            return Err(msg);
        }
    }
    Ok(())
}

/// `text` with every registered secret (raw, and percent-encoded) replaced by `•••`.
pub fn redact(text: &str) -> String {
    let Ok(r) = REG.lock() else {
        return text.to_string();
    };
    let mut out = text.to_string();
    for s in r.iter() {
        let enc: String = s
            .value
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        out = out.replace(&s.value, MASK);
        if enc != s.value {
            out = out.replace(&enc, MASK);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_rfc6238_vector() {
        // RFC 6238 appendix B, SHA-1, T=59 -> 94287082 (8 digits); the 6-digit tail is 287082.
        assert_eq!(
            totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap(),
            "287082"
        );
    }

    #[test]
    fn bound_secret_blocks_other_origin_and_redacts() {
        std::env::set_var("MANUK_SECRET_UNITPW", "s3cr3t/pw+1");
        assert_eq!(resolve("unitpw").unwrap(), "s3cr3t/pw+1");
        bind("unitpw", "http://a.test/login").unwrap();
        assert!(bind("unitpw", "http://evil.test/").is_err());
        assert!(check("p=s3cr3t%2Fpw%2B1", "http://evil.test/x", "page request").is_err());
        assert!(check("p=s3cr3t/pw+1", "http://a.test/post", "page request").is_ok());
        assert_eq!(redact("v=s3cr3t/pw+1 and s3cr3t%2Fpw%2B1"), "v=••• and •••");
    }
}
