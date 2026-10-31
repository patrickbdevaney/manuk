//! W4.D2: value-flow taint. Strings the tool layer hands the agent (navigate/observe/extract/find)
//! are remembered with the origin they were read on. A later `type` or `navigate` whose payload
//! carries one of them, verbatim or urlencoded / base64 / hex, toward a different origin is refused
//! (the CometJacking trick). Only value-like tokens are tracked (>=6 chars with a digit or `@`), so
//! ordinary prose a page shows never blocks typing. `MANUK_TAINT_MUTATE=1` disables the check
//! (gate mutation); `MANUK_TAINT=off` turns the feature off for utility A/B runs.

use base64::Engine;

use crate::policy::origin;

const MIN_LEN: usize = 6;
const MAX_TOKENS: usize = 4000;

#[derive(Default)]
pub struct Taint {
    /// (lowercased token, origin it was read on)
    seen: Vec<(String, String)>,
}

fn enabled() -> bool {
    std::env::var("MANUK_TAINT").map_or(true, |v| v != "off")
        && std::env::var_os("MANUK_TAINT_MUTATE").is_none()
}

fn value_like(t: &str) -> bool {
    t.len() >= MIN_LEN && (t.contains('@') || t.bytes().any(|b| b.is_ascii_digit()))
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || "@._-+".contains(c)))
        .map(|t| t.trim_matches(|c| "._-+".contains(c)))
        .filter(|t| value_like(t))
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 3 <= b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_decode(s: &str) -> Option<String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() < 8 || s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(bytes).ok()
}

fn b64_decode(s: &str) -> Option<String> {
    if s.len() < 8 {
        return None;
    }
    let t = s.trim_end_matches('=');
    let engines = [
        base64::engine::general_purpose::STANDARD_NO_PAD,
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ];
    engines
        .iter()
        .find_map(|e| e.decode(t).ok())
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|d| d.chars().all(|c| !c.is_control() || c.is_whitespace()))
}

/// The payload as the receiver could read it: raw plus every decoding, up to depth 3, and the same
/// for each separator-delimited piece (a query string `d=<b64>` decodes piecewise).
fn views(payload: &str) -> Vec<String> {
    let mut out = vec![payload.to_ascii_lowercase()];
    let mut frontier = vec![payload.to_string()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for s in &frontier {
            let mut cands = vec![s.clone()];
            cands.extend(
                s.split(|c: char| "&=?/ ,;:#\n\r\t\"'".contains(c))
                    .filter(|p| p.len() >= 8)
                    .map(String::from),
            );
            for c in cands {
                let mut decs = vec![url_decode(&c)];
                decs.extend(hex_decode(&c));
                decs.extend(b64_decode(&c));
                for d in decs {
                    if d != c && !out.contains(&d.to_ascii_lowercase()) {
                        out.push(d.to_ascii_lowercase());
                        next.push(d);
                    }
                }
            }
        }
        frontier = next;
    }
    out
}

impl Taint {
    /// Remember the value-like tokens of text read on `page_url`.
    pub fn record(&mut self, page_url: &str, text: &str) {
        let Some(o) = origin(page_url) else { return };
        for t in tokens(text) {
            let t = t.to_ascii_lowercase();
            if self.seen.len() >= MAX_TOKENS {
                self.seen.remove(0);
            }
            if !self.seen.iter().any(|(s, so)| *s == t && *so == o) {
                self.seen.push((t, o.clone()));
            }
        }
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// May `payload` go toward `dest_url`? `Err` is the explained refusal.
    pub fn check(&self, payload: &str, dest_url: &str, what: &str) -> Result<(), String> {
        if !enabled() || self.seen.is_empty() {
            return Ok(());
        }
        let Some(dest) = origin(dest_url) else {
            return Ok(());
        };
        let foreign: Vec<&(String, String)> = self.seen.iter().filter(|(_, o)| *o != dest).collect();
        if foreign.is_empty() {
            return Ok(());
        }
        let vs = views(payload);
        for (tok, src) in foreign {
            // A token that is also shown on the destination origin is not a secret from it.
            if self.seen.iter().any(|(t, o)| t == tok && *o == dest) {
                continue;
            }
            if vs.iter().any(|v| v.contains(tok.as_str())) {
                return Err(format!(
                    "EGRESS-DENIED (value-flow): the {what} carries a value read on {src} ({}) toward {dest}; \
                     nothing was sent. If the user really wants this, they must grant {dest} (--write-origins) or do it themselves",
                    mask(tok)
                ));
            }
        }
        Ok(())
    }
}

fn mask(t: &str) -> String {
    let head: String = t.chars().take(2).collect();
    format!("{head}… {} chars", t.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "http://a.test:80/p";
    const B: &str = "http://b.test/post";

    fn t() -> Taint {
        let mut t = Taint::default();
        t.record(A, "Your API token: tok_9f8e7d6c5b4a ok");
        t
    }

    #[test]
    fn four_encodings_blocked_raw_to_a_allowed() {
        let t = t();
        let v = "tok_9f8e7d6c5b4a";
        let b64 = base64::engine::general_purpose::STANDARD.encode(v);
        let hex: String = v.bytes().map(|b| format!("{b:02x}")).collect();
        for p in [
            v.to_string(),
            "tok%5F9f8e7d6c5b4a".to_string(),
            format!("d={b64}"),
            format!("x={hex}"),
        ] {
            assert!(t.check(&p, B, "typed text").is_err(), "{p}");
            assert!(t.check(&p, "http://a.test/other", "typed text").is_ok(), "{p}");
        }
        assert!(t.check("hello world", B, "typed text").is_ok());
    }
}
