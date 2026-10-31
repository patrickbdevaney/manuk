//! W3.8: agent-mode downloads. A file lands only in the operator-granted directory (the Download
//! capability: `--download-dir <dir>`), under a sanitised name, mode 0600 and never executable, capped
//! at [`CAP_BYTES`]. The result the agent sees is `{name, bytes, sha256, mime}` (MIME sniffed from the
//! bytes, not trusted from the server).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

pub const CAP_BYTES: u64 = 100 * 1024 * 1024;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Grant the Download capability, once, for this process.
pub fn grant(dir: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&dir)?;
    let _ = DIR.set(std::fs::canonicalize(&dir)?);
    Ok(())
}

pub fn granted() -> Option<&'static Path> {
    DIR.get().map(|p| p.as_path())
}

/// A file name that cannot leave the directory or start a dotfile.
pub fn safe_name(suggested: &str) -> String {
    let base = suggested.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    let cleaned: String = cleaned.chars().take(120).collect();
    if cleaned.is_empty() {
        "download".into()
    } else {
        cleaned
    }
}

/// MIME from the leading bytes; `application/octet-stream` when nothing matches.
pub fn sniff(b: &[u8]) -> &'static str {
    let t = |p: &[u8]| b.starts_with(p);
    if t(b"%PDF-") {
        "application/pdf"
    } else if t(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if t(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if t(b"GIF87a") || t(b"GIF89a") {
        "image/gif"
    } else if t(b"PK\x03\x04") {
        "application/zip"
    } else if t(b"\x1f\x8b") {
        "application/gzip"
    } else if t(b"\x7fELF") {
        "application/x-executable"
    } else if std::str::from_utf8(&b[..b.len().min(4096)]).is_ok_and(|s| {
        !s.is_empty()
            && !s
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    }) {
        let head = String::from_utf8_lossy(&b[..b.len().min(512)]).to_ascii_lowercase();
        if head.trim_start().starts_with("<!doctype html") || head.contains("<html") {
            "text/html"
        } else if head.trim_start().starts_with('{') || head.trim_start().starts_with('[') {
            "application/json"
        } else if head.contains(',') && head.contains('\n') {
            "text/csv"
        } else {
            "text/plain"
        }
    } else {
        "application/octet-stream"
    }
}

/// Move the finished file at `tmp` to `<dir>/<safe name>` (de-duplicated), strip every execute bit,
/// refuse (and delete) anything over the cap. Returns the one-line result.
pub fn finalize(tmp: &Path, suggested: &str) -> Result<String> {
    let Some(dir) = granted() else {
        let _ = std::fs::remove_file(tmp);
        bail!(
            "download refused: no Download capability (start the server with --download-dir <dir>)"
        );
    };
    let len = std::fs::metadata(tmp)?.len();
    if len > CAP_BYTES {
        let _ = std::fs::remove_file(tmp);
        bail!("download aborted: {len} bytes exceeds the {CAP_BYTES}-byte cap; nothing was kept");
    }
    let bytes = std::fs::read(tmp)?;
    let mut name = safe_name(suggested);
    let mut dest = dir.join(&name);
    let mut n = 1;
    while dest.exists() {
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) => (s.to_string(), format!(".{e}")),
            None => (name.clone(), String::new()),
        };
        dest = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    if let Some(f) = dest.file_name() {
        name = f.to_string_lossy().into_owned();
    }
    std::fs::write(&dest, &bytes)?;
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
    let _ = std::fs::remove_file(tmp);
    let sha: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!(
        "downloaded {{name: {name:?}, bytes: {}, sha256: {sha}, mime: {}}} -> {}",
        bytes.len(),
        sniff(&bytes),
        dest.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cannot_escape_or_hide() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name(".bashrc"), "bashrc");
        assert_eq!(safe_name("a:b*c.txt"), "a_b_c.txt");
        assert_eq!(safe_name(""), "download");
        assert_eq!(safe_name("..\\x\\y.exe"), "y.exe");
    }

    #[test]
    fn sniffs_by_content_not_extension() {
        assert_eq!(sniff(b"%PDF-1.7 x"), "application/pdf");
        assert_eq!(sniff(b"PK\x03\x04zz"), "application/zip");
        assert_eq!(sniff(b"a,b\n1,2\n"), "text/csv");
        assert_eq!(sniff(b"\x00\x01\x02"), "application/octet-stream");
        assert_eq!(sniff(b"<!DOCTYPE html><p>"), "text/html");
    }
}
