//! Minimal `.env` loading and provider key/model helpers — no `dotenv` dependency.
//!
//! Real process environment always wins; `.env` only fills gaps. The `.env` file
//! `.env` itself is gitignored — see the project `.gitignore`.

use std::path::PathBuf;

/// Load `.env` from the current dir or a nearby ancestor, setting any keys not
/// already present in the environment. Safe to call multiple times.
pub fn load_dotenv() {
    let Some(path) = find_dotenv() else {
        return;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
}

fn find_dotenv() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..4 {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// The model id (`GROQ_MODEL`, else the crate default).
pub fn model() -> String {
    std::env::var("GROQ_MODEL").unwrap_or_else(|_| crate::DEFAULT_MODEL.to_string())
}

/// The provider credential for the committed runner.
pub fn single_key() -> Option<String> {
    std::env::var("GROQ_API_KEY").ok().filter(|k| !k.is_empty())
}

