//! Upstream WPT **reftest** runner (P0.3).
//!
//! A reftest is a test file carrying `<link rel="match" href="ref.html">` (or
//! `mismatch`). We render both the test and its reference with the Manuk CPU
//! pipeline and compare pixels: `match` passes iff identical, `mismatch` passes iff
//! different. Deterministic CPU raster makes exact comparison meaningful.
//!
//! Pinned corpus: check out `web-platform-tests/wpt` at the commit named in
//! `IMPLEMENTATION.md` (currently `7f6164e…`, 2026-07-09) so pass counts are
//! meaningful. Tests needing JS/testharness or external resources we don't yet load
//! are **skipped** (honest — not counted as pass).

use std::path::{Path, PathBuf};

use manuk_page::Page;
use manuk_text::FontContext;

use crate::Report;

/// WPT default reference viewport.
const VW: u32 = 800;
const VH: u32 = 600;

/// Run every reftest under `wpt_dir/subdir`, returning a [`Report`].
pub fn run_reftests(wpt_dir: &Path, subdir: &str, fonts: &FontContext) -> Report {
    let mut report = Report::default();
    let root = wpt_dir.join(subdir);
    let mut files = Vec::new();
    collect_html(&root, &mut files);
    files.sort();

    for path in files {
        let name = path
            .strip_prefix(wpt_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        match run_one(&path, fonts) {
            RefOutcome::Pass => report.push(&name, Ok(())),
            RefOutcome::Fail(why) => report.push(&name, Err::<(), String>(why)),
            RefOutcome::Skip(why) => report.skip(&name, &why),
        }
    }
    report
}

enum RefOutcome {
    Pass,
    Fail(String),
    Skip(String),
}

fn run_one(test: &Path, fonts: &FontContext) -> RefOutcome {
    let Ok(content) = std::fs::read_to_string(test) else {
        return RefOutcome::Skip("unreadable".into());
    };
    // testharness.js / scripted tests need a JS runtime we don't wire by default.
    if content.contains("testharness") || content.contains("<script") {
        return RefOutcome::Skip("needs JS/testharness".into());
    }
    let Some((kind, href)) = find_ref_link(&content) else {
        return RefOutcome::Skip("not a reftest (no rel=match/mismatch)".into());
    };
    let Some(ref_path) = resolve_sibling(test, &href) else {
        return RefOutcome::Skip("reference path unresolved".into());
    };
    let Ok(ref_content) = std::fs::read_to_string(&ref_path) else {
        return RefOutcome::Skip("reference unreadable".into());
    };

    let test_px = render(&content, test, fonts);
    let ref_px = render(&ref_content, &ref_path, fonts);
    let equal = test_px == ref_px;
    let pass = if kind == RefKind::Mismatch {
        !equal
    } else {
        equal
    };
    if pass {
        RefOutcome::Pass
    } else {
        RefOutcome::Fail(format!(
            "{} render {}",
            if kind == RefKind::Mismatch {
                "mismatch"
            } else {
                "match"
            },
            if equal { "identical" } else { "differs" }
        ))
    }
}

fn render(html: &str, path: &Path, fonts: &FontContext) -> Vec<u8> {
    let url = format!("file://{}", path.display());
    let page = Page::load(html, &url, fonts, VW as f32);
    let canvas = page.paint(fonts, VW, VH);
    canvas.rgba_bytes().to_vec()
}

#[derive(PartialEq)]
enum RefKind {
    Match,
    Mismatch,
}

/// Find the first `<link rel="match|mismatch" href="…">` via a lightweight scan.
fn find_ref_link(html: &str) -> Option<(RefKind, String)> {
    let dom = manuk_html::parse(html);
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) != Some("link") {
            continue;
        }
        let el = dom.element(n)?;
        let rel = el.attr("rel").unwrap_or("").to_ascii_lowercase();
        let kind = if rel == "match" {
            RefKind::Match
        } else if rel == "mismatch" {
            RefKind::Mismatch
        } else {
            continue;
        };
        if let Some(href) = el.attr("href") {
            return Some((kind, href.to_string()));
        }
    }
    None
}

fn resolve_sibling(test: &Path, href: &str) -> Option<PathBuf> {
    if href.contains("://") {
        return None; // absolute/external reference — out of scope for now
    }
    let dir = test.parent()?;
    // Strip any query/fragment; join relative.
    let clean = href.split(['?', '#']).next().unwrap_or(href);
    Some(dir.join(clean))
}

fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_html(&p, out);
        } else if matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("html" | "xht" | "xhtml" | "htm")
        ) {
            // Skip obvious reference files (named *-ref.*) as *tests* — they are
            // targets, discovered via their test's match link.
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !stem.ends_with("-ref") && !stem.ends_with("-notref") {
                out.push(p);
            }
        }
    }
}
