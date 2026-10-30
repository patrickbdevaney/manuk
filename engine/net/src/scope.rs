//! W3.9.a: origin sets enforced below the MCP hub. The hub judges what the *agent* asks for; this
//! judges what the *page* asks for (subresources, redirect hops, fetch/XHR, form POSTs, frames).
//! Every request passes `send_raw`, which asks [`check`] first, so a refused URL never reaches a
//! socket. Empty set = unrestricted. `MANUK_SCOPE_MUTATE=1` disables the check (gate mutation).

use std::sync::Mutex;

use url::Url;

#[derive(Default)]
struct Scope {
    read: Vec<String>,
    write: Vec<String>,
    refused: Vec<String>,
}

static SCOPE: Mutex<Scope> = Mutex::new(Scope {
    read: Vec::new(),
    write: Vec::new(),
    refused: Vec::new(),
});

fn origin(url: &str) -> Option<String> {
    let u = Url::parse(url.trim()).ok()?;
    let h = u.host_str()?;
    Some(match u.port() {
        Some(p) => format!("{}://{}:{p}", u.scheme(), h.to_ascii_lowercase()),
        None => format!("{}://{}", u.scheme(), h.to_ascii_lowercase()),
    })
}

fn in_set(set: &[String], url: &str) -> bool {
    set.is_empty()
        || origin(url).is_some_and(|o| set.iter().any(|s| origin(s).as_deref() == Some(&o)))
}

/// Install the read (GET/HEAD/OPTIONS) and write (POST/PUT/...) origin sets for this process.
pub fn set(read: &[String], write: &[String]) {
    let mut s = SCOPE.lock().unwrap();
    s.read = read.to_vec();
    s.write = write.to_vec();
}

/// Is any origin set installed?
pub fn active() -> bool {
    let s = SCOPE.lock().unwrap();
    !(s.read.is_empty() && s.write.is_empty())
}

/// Judge one outgoing request. `Err` carries the refusal text (also logged).
pub fn check(method: &str, url: &str) -> Result<(), String> {
    if !matches!(url.split(':').next(), Some("http" | "https")) {
        return Ok(()); // data:/blob:/about: never leave the process
    }
    let mut s = SCOPE.lock().unwrap();
    if s.read.is_empty() && s.write.is_empty() {
        return Ok(());
    }
    let safe = matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "OPTIONS"
    );
    let (set, kind) = if safe {
        (&s.read, "read")
    } else {
        (&s.write, "write")
    };
    if in_set(set, url) || std::env::var_os("MANUK_SCOPE_MUTATE").is_some() {
        return Ok(());
    }
    let msg = format!("origin not in {kind} set: {method} {url}");
    eprintln!("manuk-net: REFUSED {msg}");
    s.refused.push(msg.clone());
    Err(msg)
}

/// Refusals so far (for tests and the hub's observe note).
pub fn refusals() -> Vec<String> {
    SCOPE.lock().unwrap().refused.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_by_method_and_origin() {
        set(&["http://a.test".into()], &["http://a.test".into()]);
        assert!(check("GET", "http://a.test/x").is_ok());
        assert!(check("GET", "http://evil.test/x").is_err());
        assert!(check("POST", "http://evil.test/x").is_err());
        assert!(check("GET", "data:text/plain,hi").is_ok());
        set(&[], &[]);
        assert!(check("GET", "http://evil.test/x").is_ok());
    }
}
