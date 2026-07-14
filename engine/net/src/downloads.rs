//! Downloads (L04): decide whether a response is a file to save rather than a page to render,
//! pick a filename, choose a target directory, and write the bytes to disk with de-duplication.
//!
//! Pure + testable — the network round-trip is the caller's; this module is only the policy +
//! filesystem tail. The shell branches on [`is_attachment`] at nav-completion, then uses
//! [`download_dir`] + [`suggested_filename`] + [`write_download`].

use std::path::{Path, PathBuf};

/// Directory downloads are written to: `$MANUK_DOWNLOAD_DIR` (tests + override) →
/// `$XDG_DOWNLOAD_DIR` → `$HOME/Downloads` → the current directory. The directory is created on
/// first write (see [`write_download`]).
pub fn download_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("MANUK_DOWNLOAD_DIR") {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        return PathBuf::from(d);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Downloads");
    }
    PathBuf::from(".")
}

/// Whether a response should be **downloaded** rather than rendered. True when
/// `Content-Disposition` is `attachment`, or the `Content-Type` is a clearly non-renderable
/// binary payload (octet-stream / zip / gzip / tar / generic `application/*` we don't handle).
/// Conservative on purpose: HTML/CSS/JS/JSON/text/images/PDF are NOT treated as downloads.
pub fn is_attachment(content_disposition: Option<&str>, content_type: Option<&str>) -> bool {
    if let Some(cd) = content_disposition {
        // "attachment" as the disposition type (before any ';').
        let disp = cd.split(';').next().unwrap_or("").trim();
        if disp.eq_ignore_ascii_case("attachment") {
            return true;
        }
    }
    if let Some(ct) = content_type {
        let mime = ct
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        return matches!(
            mime.as_str(),
            "application/octet-stream"
                | "application/zip"
                | "application/gzip"
                | "application/x-gzip"
                | "application/x-tar"
                | "application/x-bzip2"
                | "application/x-7z-compressed"
                | "application/x-rar-compressed"
                | "application/vnd.ms-excel"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                | "application/msword"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
    }
    false
}

/// A suggested filename for a download: the `Content-Disposition` `filename*=`/`filename=`
/// parameter if present (RFC 6266), else the URL's last path segment, else `"download"`. The
/// result is sanitized so it can never contain a path separator or escape the target directory.
pub fn suggested_filename(content_disposition: Option<&str>, url: &str) -> String {
    if let Some(cd) = content_disposition {
        if let Some(name) = filename_from_disposition(cd) {
            let s = sanitize_filename(&name);
            if !s.is_empty() {
                return s;
            }
        }
    }
    // URL basename (drop query/fragment).
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let base = path.rsplit('/').next().unwrap_or("");
    let s = sanitize_filename(base);
    if s.is_empty() {
        "download".to_string()
    } else {
        s
    }
}

/// Parse the `filename`/`filename*` parameter out of a `Content-Disposition` value.
/// `filename*=UTF-8''na%20me.pdf` (RFC 5987) takes precedence over a plain `filename="na me"`.
fn filename_from_disposition(cd: &str) -> Option<String> {
    let mut plain: Option<String> = None;
    for part in cd.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("filename*=") {
            // ext-value: charset'lang'pct-encoded
            if let Some(enc) = v.splitn(3, '\'').nth(2) {
                return Some(percent_decode(enc));
            }
        } else if let Some(v) = part.strip_prefix("filename=") {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                plain = Some(v.to_string());
            }
        }
    }
    plain
}

/// Minimal percent-decode (for `filename*` ext-values); invalid escapes are kept verbatim.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip anything that could traverse or break out of the target directory: path separators,
/// `..`, control chars, leading dots. Empty if nothing usable remains.
fn sanitize_filename(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\' && *c != '\0')
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    if cleaned == ".." || cleaned.is_empty() {
        String::new()
    } else {
        cleaned.to_string()
    }
}

/// Write `bytes` to `dir/filename`, creating `dir` if needed and **de-duplicating**: if the file
/// exists, insert ` (1)`, ` (2)`, … before the extension until a free name is found. Returns the
/// path actually written.
pub fn write_download(dir: &Path, filename: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dedupe_path(dir, filename);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// The first non-colliding path for `filename` in `dir` (`report.pdf` → `report (1).pdf` → …).
pub fn dedupe_path(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match filename.rsplit_once('.') {
        // Keep dotfiles-with-no-real-ext intact (".tar.gz" edge left simple on purpose).
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (filename.to_string(), String::new()),
    };
    for n in 1..10_000 {
        let cand = dir.join(format!("{stem} ({n}){ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    // Absurdly unlikely fallback.
    dir.join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_detection() {
        assert!(is_attachment(Some("attachment; filename=\"a.pdf\""), None));
        assert!(is_attachment(Some("ATTACHMENT"), None));
        assert!(is_attachment(None, Some("application/octet-stream")));
        assert!(is_attachment(None, Some("application/zip; boundary=x")));
        // Renderable types are NOT downloads.
        assert!(!is_attachment(Some("inline"), Some("text/html")));
        assert!(!is_attachment(None, Some("text/html; charset=utf-8")));
        assert!(!is_attachment(None, Some("image/png")));
        assert!(!is_attachment(None, Some("application/pdf")));
        assert!(!is_attachment(None, None));
    }

    #[test]
    fn filename_from_cd_and_url() {
        assert_eq!(
            suggested_filename(Some("attachment; filename=\"report.pdf\""), "https://x/y"),
            "report.pdf"
        );
        // RFC 5987 filename* wins and is percent-decoded.
        assert_eq!(
            suggested_filename(
                Some("attachment; filename=\"fallback.bin\"; filename*=UTF-8''re%20port.pdf"),
                "https://x/y"
            ),
            "re port.pdf"
        );
        // No disposition → URL basename, query stripped.
        assert_eq!(
            suggested_filename(None, "https://x/files/data.csv?token=abc"),
            "data.csv"
        );
        // No usable name anywhere → "download".
        assert_eq!(suggested_filename(None, "https://x/"), "download");
    }

    #[test]
    fn filename_cannot_traverse() {
        assert_eq!(
            suggested_filename(Some("attachment; filename=\"../../etc/passwd\""), "u"),
            "passwd"
        );
        assert_eq!(
            suggested_filename(Some("attachment; filename=\"..\""), "https://x/safe.txt"),
            "safe.txt"
        );
    }

    #[test]
    fn writes_and_dedupes() {
        let dir = std::env::temp_dir().join(format!("manuk-dl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p1 = write_download(&dir, "report.pdf", b"AAA").expect("write 1");
        let p2 = write_download(&dir, "report.pdf", b"BBB").expect("write 2");
        assert_eq!(p1.file_name().unwrap(), "report.pdf");
        assert_eq!(p2.file_name().unwrap(), "report (1).pdf");
        assert_eq!(std::fs::read(&p1).unwrap(), b"AAA");
        assert_eq!(std::fs::read(&p2).unwrap(), b"BBB");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
