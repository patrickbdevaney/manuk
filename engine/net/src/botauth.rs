//! W3.19: Web Bot Auth (draft-meunier-web-bot-auth-architecture, RFC 9421 HTTP Message Signatures).
//!
//! The operator hands the browser an Ed25519 key (`--agent-key <jwk>`) and the URL of the key
//! directory they host (`--signature-agent <url>`). Every request the browser makes on a page's
//! behalf then carries `Signature-Agent`, `Signature-Input` and `Signature` (and `Content-Digest`
//! when it has a body), so a site that verifies Web Bot Auth can tell who is asking. The identity is
//! the operator's, not manuk's. Nothing here evades anything: an unsigned request is still sent
//! exactly as before when no key is configured.
//!
//! Covered components: `@authority`, `signature-agent;key="sig1"`, and `content-digest` for bodies.

use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair};
use sha2::{Digest, Sha256};
use url::Url;

struct Identity {
    key: Ed25519KeyPair,
    /// RFC 7638 thumbprint of the public JWK (the `keyid`).
    keyid: String,
    x: String,
    agent: String,
}

static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// Install the signing identity from a JWK (`{"kty":"OKP","crv":"Ed25519","d":..,"x":..}`; a file
/// path or the JSON itself) and the operator's `Signature-Agent` URL. Once per process.
pub fn configure(jwk: &str, signature_agent: &str) -> Result<()> {
    let text = if jwk.trim_start().starts_with('{') {
        jwk.to_string()
    } else {
        std::fs::read_to_string(jwk).with_context(|| format!("reading --agent-key {jwk}"))?
    };
    let v: serde_json::Value = serde_json::from_str(&text).context("--agent-key is not JSON")?;
    if v["kty"] != "OKP" || v["crv"] != "Ed25519" {
        bail!("--agent-key must be an Ed25519 JWK (kty OKP, crv Ed25519)");
    }
    let d = URL_SAFE_NO_PAD
        .decode(
            v["d"]
                .as_str()
                .context("--agent-key has no private part `d`")?,
        )
        .context("bad `d`")?;
    let key =
        Ed25519KeyPair::from_seed_unchecked(&d).map_err(|_| anyhow::anyhow!("bad Ed25519 seed"))?;
    let x = URL_SAFE_NO_PAD.encode(key.public_key().as_ref());
    if let Some(given) = v["x"].as_str() {
        if given != x {
            bail!("--agent-key: `x` does not match `d`");
        }
    }
    let u = Url::parse(signature_agent).context("--signature-agent is not a URL")?;
    if u.scheme() != "https" && u.scheme() != "http" {
        bail!("--signature-agent must be an http(s) URL");
    }
    let keyid = thumbprint(&x);
    IDENTITY
        .set(Identity {
            key,
            keyid,
            x,
            agent: signature_agent.to_string(),
        })
        .map_err(|_| anyhow::anyhow!("Web Bot Auth identity already configured"))
}

fn thumbprint(x: &str) -> String {
    let canon = format!(r#"{{"crv":"Ed25519","kty":"OKP","x":"{x}"}}"#);
    URL_SAFE_NO_PAD.encode(Sha256::digest(canon.as_bytes()))
}

pub fn active() -> bool {
    IDENTITY.get().is_some()
}

/// The JWKS the operator publishes at `<signature-agent>/.well-known/http-message-signatures-directory`.
pub fn key_directory() -> Option<String> {
    let id = IDENTITY.get()?;
    Some(serde_json::json!({"keys": [{"kty": "OKP", "crv": "Ed25519", "x": id.x}]}).to_string())
}

/// RFC 9421 `@authority`: lowercase host, port only when not the scheme default.
fn authority(u: &Url) -> String {
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    }
}

/// The headers to add to a request. Empty when no identity is configured. `body` non-empty adds a
/// `Content-Digest` (sha-256) that the signature covers.
pub fn sign(url: &Url, body: &[u8]) -> Vec<(String, String)> {
    match IDENTITY.get() {
        Some(id) => sign_with(id, &authority(url), body, now_secs()),
        None => Vec::new(),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sign_with(id: &Identity, authority: &str, body: &[u8], created: u64) -> Vec<(String, String)> {
    let agent_member = format!("sig1=\"{}\"", id.agent);
    let mut covered = String::from("\"@authority\" \"signature-agent\";key=\"sig1\"");
    let mut lines = vec![
        format!("\"@authority\": {authority}"),
        format!("\"signature-agent\";key=\"sig1\": \"{}\"", id.agent),
    ];
    let mut out = vec![("Signature-Agent".to_string(), agent_member)];
    if !body.is_empty() {
        let digest = format!("sha-256=:{}:", STANDARD.encode(Sha256::digest(body)));
        covered.push_str(" \"content-digest\"");
        lines.push(format!("\"content-digest\": {digest}"));
        out.push(("Content-Digest".to_string(), digest));
    }
    let mut nonce = [0u8; 64];
    let _ = SystemRandom::new().fill(&mut nonce);
    let params = format!(
        "({covered});created={created};keyid=\"{}\";alg=\"ed25519\";expires={};nonce=\"{}\";tag=\"web-bot-auth\"",
        id.keyid,
        created + 300,
        STANDARD.encode(nonce)
    );
    lines.push(format!("\"@signature-params\": {params}"));
    let sig = id.key.sign(lines.join("\n").as_bytes());
    out.push(("Signature-Input".to_string(), format!("sig1={params}")));
    out.push((
        "Signature".to_string(),
        format!("sig1=:{}:", STANDARD.encode(sig.as_ref())),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{UnparsedPublicKey, ED25519};

    // RFC 9421 Appendix B.1.4 test key.
    const D: &str = "n4Ni-HpISpVObnQMW0wOhCKROaIKqKtW_2ZYb2p9KcU";
    const X: &str = "JrQLj5P_89iXES9-vFgrIy29clF9CC_oPPsw3c5D0bs";

    fn id() -> Identity {
        let key = Ed25519KeyPair::from_seed_unchecked(&URL_SAFE_NO_PAD.decode(D).unwrap()).unwrap();
        assert_eq!(URL_SAFE_NO_PAD.encode(key.public_key().as_ref()), X);
        Identity {
            key,
            keyid: thumbprint(X),
            x: X.into(),
            agent: "https://agent.test".into(),
        }
    }

    #[test]
    fn signature_verifies_over_the_base_and_not_over_another_authority() {
        let id = id();
        let h = sign_with(&id, "example.com", b"a=1", 1_700_000_000);
        let get = |n: &str| h.iter().find(|(k, _)| k == n).unwrap().1.clone();
        assert!(get("Content-Digest").starts_with("sha-256=:"));
        let params = get("Signature-Input")
            .strip_prefix("sig1=")
            .unwrap()
            .to_string();
        let base = |auth: &str| {
            format!(
                "\"@authority\": {auth}\n\"signature-agent\";key=\"sig1\": \"https://agent.test\"\n\"content-digest\": {}\n\"@signature-params\": {params}",
                get("Content-Digest")
            )
        };
        let sig = STANDARD
            .decode(
                get("Signature")
                    .strip_prefix("sig1=:")
                    .unwrap()
                    .trim_end_matches(':'),
            )
            .unwrap();
        let pk = UnparsedPublicKey::new(&ED25519, URL_SAFE_NO_PAD.decode(X).unwrap());
        assert!(pk.verify(base("example.com").as_bytes(), &sig).is_ok());
        assert!(pk.verify(base("evil.test").as_bytes(), &sig).is_err());
    }

    #[test]
    fn authority_keeps_nondefault_port_only() {
        assert_eq!(
            authority(&Url::parse("https://Example.com/x").unwrap()),
            "example.com"
        );
        assert_eq!(
            authority(&Url::parse("https://example.com:443/x").unwrap()),
            "example.com"
        );
        assert_eq!(
            authority(&Url::parse("http://127.0.0.1:8080/x").unwrap()),
            "127.0.0.1:8080"
        );
    }

    #[test]
    fn bodyless_request_has_no_digest() {
        let h = sign_with(&id(), "a.test", b"", 1);
        assert!(h.iter().all(|(k, _)| k != "Content-Digest"));
    }
}
