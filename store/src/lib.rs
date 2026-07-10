//! manuk-store — local encrypted password store + origin-scoped autofill (E2).
//!
//! **Hard rule (CLAUDE.md § E2): audited crates only, ZERO hand-rolled crypto.** This
//! crate *orchestrates* audited primitives and implements only the (crypto-free)
//! origin-matching policy:
//!
//! - at-rest sealing: **`chacha20poly1305`** (RustCrypto AEAD, NCC-audited);
//! - key derivation for the no-keyring fallback: **`argon2`** (Argon2id, PHC winner);
//! - OS secret store for the at-rest key: **`keyring`** (DPAPI / Keychain / Secret
//!   Service), behind the `os-keyring` feature (needs a running keyring service);
//! - related-domain scoping: **`psl`** (Public Suffix List, eTLD+1).
//!
//! ## Key management
//!
//! The 32-byte at-rest key comes from the OS secret store when available
//! ([`PasswordStore::from_os_keyring`], `os-keyring` feature); otherwise from a user
//! **primary password** via Argon2id ([`PasswordStore::from_primary_password`]).
//! There is **no hardcoded-password fallback** (Chromium's known weak spot).
//!
//! ## Autofill origin policy (a wrong rule leaks credentials)
//!
//! Credentials are keyed by **`signon_realm` = scheme + host + port**. Autofill is
//! called with the **field's own origin** (so a cross-origin iframe field only matches
//! its own origin, never the top document's):
//! - exact `signon_realm` match → [`Match::Exact`] (safe to auto-fill);
//! - same **eTLD+1** + same scheme, different host → [`Match::RelatedDomain`] (a
//!   *suggestion*, surfaced not silently filled);
//! - **no scheme downgrade**: an `https`-saved credential is never offered to an
//!   `http` field.

/// G-b — local-first searchable history (encrypted at rest, LEANN-style recompute).
pub mod history_index;

use anyhow::{anyhow, Context, Result};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};
use url::Url;

/// A saved credential. `signon_realm` is `scheme://host:port` (see [`signon_realm`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub signon_realm: String,
    pub username: String,
    pub password: String,
}

/// How a stored credential relates to the field origin autofill was asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// Exact `signon_realm` (scheme+host+port) — safe to auto-fill.
    Exact,
    /// Same registrable domain (eTLD+1) + scheme, different host — a *suggestion*.
    RelatedDomain,
}

/// An autofill suggestion: the credential + why it matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub credential: Credential,
    pub kind: Match,
}

/// The `signon_realm` (`scheme://host:port`) for an `http(s)` URL, or `None` for a
/// non-`http(s)` / unparseable URL.
pub fn signon_realm(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    Some(format!(
        "{}://{}:{}",
        u.scheme(),
        u.host_str()?,
        u.port_or_known_default()?
    ))
}

/// Registrable domain (eTLD+1) of a host, via the Public Suffix List.
fn etld1(host: &str) -> Option<String> {
    psl::domain_str(host).map(|d| d.to_owned())
}

/// A decrypted, in-memory credential store keyed by a 32-byte at-rest key.
pub struct PasswordStore {
    key: [u8; 32],
    creds: Vec<Credential>,
}

impl PasswordStore {
    /// An empty store sealed by an explicit 32-byte key (e.g. one fetched from the OS
    /// secret store).
    pub fn with_key(key: [u8; 32]) -> Self {
        PasswordStore {
            key,
            creds: Vec::new(),
        }
    }

    /// Derive the at-rest key from a user **primary password** with Argon2id (the
    /// portable, no-keyring path). `salt` must be stored alongside the DB (it is not
    /// secret) and be ≥ 8 bytes.
    pub fn from_primary_password(password: &str, salt: &[u8]) -> Result<Self> {
        let key = derive_key(password, salt)?;
        Ok(PasswordStore::with_key(key))
    }

    pub fn add(&mut self, credential: Credential) {
        self.creds.push(credential);
    }

    pub fn credentials(&self) -> &[Credential] {
        &self.creds
    }

    /// Seal the store: JSON-serialize the credentials and AEAD-encrypt them. Layout is
    /// `nonce (12 B) || ciphertext` — the nonce is random per seal.
    pub fn seal(&self) -> Result<Vec<u8>> {
        let plaintext = serde_json::to_vec(&self.creds).context("serialize credentials")?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, plaintext.as_ref())
            .map_err(|_| anyhow!("AEAD seal failed"))?;
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Open a sealed blob with `key`. Fails (authentication error) on a wrong key or a
    /// tampered blob — the AEAD's integrity guarantee.
    pub fn open(key: [u8; 32], blob: &[u8]) -> Result<Self> {
        if blob.len() < 12 {
            return Err(anyhow!("sealed blob too short"));
        }
        let (nonce_bytes, ct) = blob.split_at(12);
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ct)
            .map_err(|_| anyhow!("AEAD open failed (wrong key or tampered store)"))?;
        let creds: Vec<Credential> =
            serde_json::from_slice(&plaintext).context("deserialize credentials")?;
        Ok(PasswordStore { key, creds })
    }

    /// Autofill suggestions for a form field at `field_url` (**the field's own
    /// origin**). Exact-realm matches first (auto-fillable), then related-domain
    /// suggestions. Enforces no-scheme-downgrade and same-origin-iframe semantics.
    pub fn autofill(&self, field_url: &str) -> Vec<Suggestion> {
        let Some(realm) = signon_realm(field_url) else {
            return Vec::new();
        };
        let field = match Url::parse(field_url) {
            Ok(u) => u,
            Err(_) => return Vec::new(),
        };
        let field_scheme = field.scheme();
        let field_etld1 = field.host_str().and_then(etld1);

        let mut exact = Vec::new();
        let mut related = Vec::new();
        for c in &self.creds {
            if c.signon_realm == realm {
                exact.push(Suggestion {
                    credential: c.clone(),
                    kind: Match::Exact,
                });
                continue;
            }
            // Related-domain: parse the stored realm and compare eTLD+1 + scheme.
            let Ok(cred_url) = Url::parse(&c.signon_realm) else {
                continue;
            };
            let cred_scheme = cred_url.scheme();
            // No scheme downgrade: never offer an https credential to an http field.
            if cred_scheme == "https" && field_scheme == "http" {
                continue;
            }
            if cred_scheme != field_scheme {
                continue; // related-domain requires the same scheme
            }
            let cred_etld1 = cred_url.host_str().and_then(etld1);
            if cred_etld1.is_some() && cred_etld1 == field_etld1 {
                related.push(Suggestion {
                    credential: c.clone(),
                    kind: Match::RelatedDomain,
                });
            }
        }
        exact.extend(related);
        exact
    }
}

/// Derive a 32-byte key from a password + salt via Argon2id (default params).
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow!("Argon2id key derivation failed: {e}"))?;
    Ok(key)
}

/// OS secret-store key management (audited `keyring` wrapper). Feature-gated because it
/// needs a running keyring service (DBus Secret Service on Linux).
#[cfg(feature = "os-keyring")]
pub mod os_keyring {
    use super::*;
    use chacha20poly1305::aead::rand_core::RngCore;

    /// Fetch the store's 32-byte key from the OS secret store, generating + saving a
    /// fresh random key on first use. `service`/`account` name the keyring entry.
    pub fn key(service: &str, account: &str) -> Result<[u8; 32]> {
        let entry = keyring::Entry::new(service, account).context("open keyring entry")?;
        match entry.get_secret() {
            Ok(bytes) if bytes.len() == 32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                Ok(k)
            }
            _ => {
                let mut k = [0u8; 32];
                OsRng.fill_bytes(&mut k);
                entry.set_secret(&k).context("store key in keyring")?;
                Ok(k)
            }
        }
    }

    impl PasswordStore {
        /// Build a store whose at-rest key lives in the OS secret store.
        pub fn from_os_keyring(service: &str, account: &str) -> Result<Self> {
            Ok(PasswordStore::with_key(key(service, account)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cred(realm: &str, user: &str) -> Credential {
        Credential {
            signon_realm: realm.to_string(),
            username: user.to_string(),
            password: "hunter2".to_string(),
        }
    }

    #[test]
    fn signon_realm_is_scheme_host_port() {
        assert_eq!(
            signon_realm("https://example.com/login").as_deref(),
            Some("https://example.com:443")
        );
        assert_eq!(
            signon_realm("http://example.com:8080/x").as_deref(),
            Some("http://example.com:8080")
        );
        assert_eq!(signon_realm("ftp://example.com/"), None);
    }

    #[test]
    fn seal_open_round_trips_and_wrong_key_fails() {
        let key = derive_key("primary-password", b"salt-1234").unwrap();
        let mut store = PasswordStore::with_key(key);
        store.add(cred("https://example.com:443", "alice"));
        let blob = store.seal().unwrap();

        // Right key round-trips.
        let reopened = PasswordStore::open(key, &blob).unwrap();
        assert_eq!(reopened.credentials(), store.credentials());

        // Wrong key fails authentication (AEAD integrity).
        let wrong = derive_key("wrong-password", b"salt-1234").unwrap();
        assert!(PasswordStore::open(wrong, &blob).is_err());

        // Tampering the ciphertext fails authentication.
        let mut tampered = blob.clone();
        *tampered.last_mut().unwrap() ^= 0xff;
        assert!(PasswordStore::open(key, &tampered).is_err());
    }

    #[test]
    fn argon2id_key_is_deterministic_per_password_salt() {
        let a = derive_key("pw", b"the-salt-16bytes").unwrap();
        let b = derive_key("pw", b"the-salt-16bytes").unwrap();
        let c = derive_key("pw", b"different-salt!!").unwrap();
        assert_eq!(a, b, "same password+salt → same key");
        assert_ne!(a, c, "different salt → different key");
    }

    #[test]
    fn autofill_exact_match_only_by_default() {
        let mut s = PasswordStore::with_key([7u8; 32]);
        s.add(cred("https://example.com:443", "alice"));
        s.add(cred("https://other.test:443", "bob"));

        let hits = s.autofill("https://example.com/login");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, Match::Exact);
        assert_eq!(hits[0].credential.username, "alice");
    }

    #[test]
    fn autofill_related_domain_is_suggestion_not_silent() {
        let mut s = PasswordStore::with_key([7u8; 32]);
        s.add(cred("https://login.example.com:443", "alice"));
        // A form on www.example.com (same eTLD+1, different host) → related suggestion.
        let hits = s.autofill("https://www.example.com/login");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, Match::RelatedDomain);
    }

    #[test]
    fn autofill_never_cross_origin_or_scheme_downgrade() {
        let mut s = PasswordStore::with_key([7u8; 32]);
        s.add(cred("https://example.com:443", "alice"));

        // Different registrable domain → nothing (no cross-origin leak).
        assert!(s.autofill("https://evil.test/login").is_empty());

        // Scheme downgrade: an https-saved credential is never offered to http,
        // even same host.
        let downgrade = s.autofill("http://example.com/login");
        assert!(
            downgrade.iter().all(|h| h.kind != Match::Exact),
            "no exact fill across a scheme downgrade"
        );
        assert!(
            downgrade.is_empty(),
            "https cred not offered to http field at all"
        );
    }

    #[test]
    fn autofill_uses_the_fields_own_origin_for_iframes() {
        // A cross-origin iframe field: autofill is called with the FIELD's origin, so
        // the top document's saved credential must not appear.
        let mut s = PasswordStore::with_key([7u8; 32]);
        s.add(cred("https://top.example:443", "alice")); // saved for the top page
        let iframe_field = "https://ads.thirdparty.test/embed";
        assert!(
            s.autofill(iframe_field).is_empty(),
            "a third-party iframe field must not see the top page's credential"
        );
    }
}
