//! **AIA intermediate fetching** — the chain-completion step Chrome does and bare webpki does not.
//!
//! Some servers (e.g. `ebanking.bdm-sa.com`) send only their leaf certificate and omit the
//! intermediate CA. Chrome recovers by following the leaf's *Authority Information Access*
//! `caIssuers` URL and downloading the missing intermediate; rustls/webpki does not, so the host
//! failed here with `invalid peer certificate: UnknownIssuer`. This verifier wraps the normal
//! webpki verifier and, ONLY on `UnknownIssuer`, fetches the intermediate over plain HTTP and
//! retries. Trust still anchors in the same root store: a self-signed or unknown-root chain still
//! fails, so no validation is relaxed.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::{CertificateError, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// DER of OID 1.3.6.1.5.5.7.48.2 (id-ad-caIssuers).
const CA_ISSUERS_OID: [u8; 10] = [0x06, 0x08, 0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x02];
const MAX_HOPS: usize = 3;
const MAX_CERT_BYTES: usize = 64 * 1024;

/// Extract the `http://` caIssuers URL from a certificate's AIA extension, if any.
/// (A byte scan for the accessMethod OID followed by a uniformResourceIdentifier `[6]` — enough
/// for AIA, whose structure is fixed, without a full X.509 parser.)
pub fn ca_issuers_url(cert_der: &[u8]) -> Option<String> {
    let pos = cert_der
        .windows(CA_ISSUERS_OID.len())
        .position(|w| w == CA_ISSUERS_OID)?;
    let rest = &cert_der[pos + CA_ISSUERS_OID.len()..];
    if rest.len() < 2 || rest[0] != 0x86 {
        return None;
    }
    let (len, off) = if rest[1] < 0x80 {
        (rest[1] as usize, 2)
    } else if rest[1] == 0x81 && rest.len() > 2 {
        (rest[2] as usize, 3)
    } else if rest[1] == 0x82 && rest.len() > 3 {
        (((rest[2] as usize) << 8) | rest[3] as usize, 4)
    } else {
        return None;
    };
    let url = std::str::from_utf8(rest.get(off..off + len)?).ok()?;
    url.starts_with("http://").then(|| url.to_string())
}

/// Minimal blocking `GET` of a plain-HTTP URL (AIA URLs are http by design: the payload is
/// authenticated by the signature chain, not the transport).
fn http_get(url: &str) -> Option<Vec<u8>> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let addr = if hostport.contains(':') {
        hostport.to_string()
    } else {
        format!("{hostport}:80")
    };
    let host = hostport.split(':').next()?;
    use std::net::ToSocketAddrs;
    let sa = addr.to_socket_addrs().ok()?.next()?;
    let mut s = std::net::TcpStream::connect_timeout(&sa, Duration::from_secs(5)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(5))).ok()?;
    s.write_all(
        format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut buf = Vec::new();
    // A reset after the body (peer closed with unread input) still leaves a usable buffer.
    let _ = s.take(MAX_CERT_BYTES as u64 + 4096).read_to_end(&mut buf);
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    if !buf.starts_with(b"HTTP/1.") || buf.get(9..12) != Some(b"200") {
        return None;
    }
    let body = buf[split + 4..].to_vec();
    (body.len() <= MAX_CERT_BYTES).then_some(body)
}

#[derive(Debug)]
pub struct AiaVerifier {
    inner: Arc<WebPkiServerVerifier>,
    cache: Mutex<HashMap<String, Vec<u8>>>,
}

impl AiaVerifier {
    pub fn new(roots: RootCertStore) -> Arc<Self> {
        let inner = WebPkiServerVerifier::builder(Arc::new(roots))
            .build()
            .expect("webpki verifier from a non-empty root store");
        Arc::new(Self {
            inner,
            cache: Mutex::new(HashMap::new()),
        })
    }

    fn fetch(&self, url: &str) -> Option<Vec<u8>> {
        if let Some(c) = self.cache.lock().ok()?.get(url) {
            return Some(c.clone());
        }
        let u = url.to_string();
        // Own thread: the verifier is sync but may be called on a tokio worker.
        let body = std::thread::spawn(move || http_get(&u)).join().ok()??;
        self.cache
            .lock()
            .ok()?
            .insert(url.to_string(), body.clone());
        Some(body)
    }
}

impl ServerCertVerifier for AiaVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let first =
            self.inner
                .verify_server_cert(end_entity, intermediates, server_name, ocsp, now);
        if !matches!(
            first,
            Err(Error::InvalidCertificate(CertificateError::UnknownIssuer))
        ) {
            return first;
        }
        let mut extra: Vec<CertificateDer<'static>> = intermediates
            .iter()
            .map(|c| c.clone().into_owned())
            .collect();
        let mut tip: Vec<u8> = intermediates
            .last()
            .map(|c| c.to_vec())
            .unwrap_or_else(|| end_entity.to_vec());
        for _ in 0..MAX_HOPS {
            let Some(url) = ca_issuers_url(&tip) else {
                break;
            };
            let Some(der) = self.fetch(&url) else { break };
            extra.push(CertificateDer::from(der.clone()));
            match self
                .inner
                .verify_server_cert(end_entity, &extra, server_name, ocsp, now)
            {
                Err(Error::InvalidCertificate(CertificateError::UnknownIssuer)) => tip = der,
                other => return other,
            }
        }
        first
    }

    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(m, c, d)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(m, c, d)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// A rustls client config using the webpki roots plus AIA chain completion.
pub fn client_config() -> rustls::ClientConfig {
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(AiaVerifier::new(roots))
        .with_no_client_auth()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aia_url_extracted() {
        let mut der = vec![0x30, 0x20];
        der.extend_from_slice(&CA_ISSUERS_OID);
        let url = b"http://example.test/ca.crt";
        der.push(0x86);
        der.push(url.len() as u8);
        der.extend_from_slice(url);
        assert_eq!(
            ca_issuers_url(&der).as_deref(),
            Some("http://example.test/ca.crt")
        );
        assert_eq!(ca_issuers_url(&[1, 2, 3]), None);
    }

    /// Hermetic end-to-end: a leaf-only TLS server whose intermediate is reachable only via the
    /// leaf's AIA URL. Needs the `openssl` CLI to mint the chain (skipped when absent).
    fn sh(dir: &std::path::Path, args: &[&str]) -> bool {
        std::process::Command::new("openssl")
            .args(args)
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn handshake(roots: RootCertStore, mutate_plain: bool, chain: &str) -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("manuk-aia-{}-{}", std::process::id(), chain));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let _ = roots;
        let http = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = http.local_addr().unwrap().port();
        let d = dir.as_path();
        std::fs::write(
            d.join("int.ext"),
            "basicConstraints=critical,CA:TRUE\nkeyUsage=keyCertSign\n",
        )
        .unwrap();
        std::fs::write(d.join("leaf.ext"), format!("subjectAltName=DNS:localhost\nauthorityInfoAccess=caIssuers;URI:http://127.0.0.1:{port}/int.der\n")).unwrap();
        let ok = sh(
            d,
            &[
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-nodes",
                "-keyout",
                "root.key",
                "-out",
                "root.pem",
                "-subj",
                "/CN=root",
                "-days",
                "2",
                "-addext",
                "basicConstraints=critical,CA:TRUE",
            ],
        ) && sh(
            d,
            &[
                "req",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-nodes",
                "-keyout",
                "int.key",
                "-out",
                "int.csr",
                "-subj",
                "/CN=int",
            ],
        ) && sh(
            d,
            &[
                "x509",
                "-req",
                "-in",
                "int.csr",
                "-CA",
                "root.pem",
                "-CAkey",
                "root.key",
                "-CAcreateserial",
                "-out",
                "int.pem",
                "-days",
                "2",
                "-extfile",
                "int.ext",
            ],
        ) && sh(
            d,
            &[
                "req",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-nodes",
                "-keyout",
                "leaf.key",
                "-out",
                "leaf.csr",
                "-subj",
                "/CN=localhost",
            ],
        ) && sh(
            d,
            &[
                "x509",
                "-req",
                "-in",
                "leaf.csr",
                "-CA",
                "int.pem",
                "-CAkey",
                "int.key",
                "-CAcreateserial",
                "-out",
                "leaf.pem",
                "-days",
                "2",
                "-extfile",
                "leaf.ext",
            ],
        ) && sh(
            d,
            &[
                "x509", "-in", "int.pem", "-outform", "DER", "-out", "int.der",
            ],
        ) && sh(
            d,
            &[
                "x509", "-in", "leaf.pem", "-outform", "DER", "-out", "leaf.der",
            ],
        ) && sh(
            d,
            &[
                "pkcs8",
                "-topk8",
                "-nocrypt",
                "-in",
                "leaf.key",
                "-outform",
                "DER",
                "-out",
                "leaf.key.der",
            ],
        ) && sh(
            d,
            &[
                "x509", "-in", "root.pem", "-outform", "DER", "-out", "root.der",
            ],
        );
        assert!(ok, "openssl chain minting failed");
        let int_der = std::fs::read(d.join("int.der")).unwrap();
        std::thread::spawn(move || {
            for mut c in http.incoming().flatten() {
                let mut b = [0u8; 1024];
                let _ = c.read(&mut b);
                let _ = write!(
                    c,
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n",
                    int_der.len()
                );
                let _ = c.write_all(&int_der);
            }
        });
        let leaf = CertificateDer::from(std::fs::read(d.join("leaf.der")).unwrap());
        let root = CertificateDer::from(std::fs::read(d.join("root.der")).unwrap());
        let mut store = RootCertStore::empty();
        if chain != "untrusted" {
            store.add(root).unwrap();
        } else {
            store
                .roots
                .extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        let verifier: Arc<dyn ServerCertVerifier> = if mutate_plain {
            WebPkiServerVerifier::builder(Arc::new(store))
                .build()
                .unwrap()
        } else {
            AiaVerifier::new(store)
        };
        // Server sends the LEAF ONLY.
        let key = rustls_pki_types::PrivateKeyDer::try_from(
            std::fs::read(d.join("leaf.key.der")).unwrap(),
        )
        .unwrap();
        let _ = rustls::crypto::ring::default_provider().install_default();
        let scfg = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![leaf], key)
                .unwrap(),
        );
        let ccfg = Arc::new(
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth(),
        );
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut conn = rustls::ServerConnection::new(scfg).unwrap();
                let _ = conn.complete_io(&mut s);
            }
        });
        let mut cs = std::net::TcpStream::connect(addr).unwrap();
        let mut cc =
            rustls::ClientConnection::new(ccfg, ServerName::try_from("localhost").unwrap())
                .unwrap();
        let r = cc
            .complete_io(&mut cs)
            .map(|_| ())
            .map_err(|e| e.to_string());
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    #[test]
    fn leaf_only_chain_completes_via_aia() {
        let r = handshake(RootCertStore::empty(), false, "ok");
        assert!(
            r.is_ok(),
            "AIA verifier must complete a leaf-only chain: {r:?}"
        );
    }

    #[test]
    fn plain_webpki_rejects_the_same_chain() {
        // The red half of the gate: without AIA fetching this chain is UnknownIssuer.
        let r = handshake(RootCertStore::empty(), true, "plain");
        assert!(r.unwrap_err().contains("UnknownIssuer"));
    }

    #[test]
    fn untrusted_root_still_rejected_even_with_aia() {
        // AIA must not relax trust: the fetched intermediate chains to a root we don't trust.
        let r = handshake(RootCertStore::empty(), false, "untrusted");
        assert!(r.is_err(), "unknown root must still fail");
    }
}
