//! manuk-wpt — the Web Platform Tests harness + results tracking.
//!
//! CLAUDE.md makes **WPT the ground-truth conformance signal** (not Chromium
//! quirks, not a hand-rolled corpus). Fully running upstream WPT needs the
//! testharness.js + reftest runners driving a live browser build; that is the
//! larger integration this crate grows into. [`find_wpt_checkout`] locates a WPT
//! tree (via `$WPT_DIR`) for that future runner.
//!
//! What runs **today** is a small built-in **layout reftest suite** ([`run_layout_suite`])
//! expressed against the real engine — the same methodology (assert layout facts,
//! track pass/fail) WPT uses, so results reporting and the CI signal exist now and
//! the upstream tests slot into the same [`Report`].

use std::path::PathBuf;

pub mod chrome;
pub mod parity;
pub mod reftest;

use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};
use manuk_layout::{layout_document, BoxContent, LayoutBox};
use manuk_text::FontContext;

/// Outcome of a single test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
}

/// One test's result.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

/// A collection of outcomes with summary + JSON reporting.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
}

impl Report {
    pub fn push(&mut self, name: &str, result: Result<(), String>) {
        let (status, detail) = match result {
            Ok(()) => (Status::Pass, String::new()),
            Err(d) => (Status::Fail, d),
        };
        self.outcomes.push(Outcome {
            name: name.to_string(),
            status,
            detail,
        });
    }

    pub fn skip(&mut self, name: &str, why: &str) {
        self.outcomes.push(Outcome {
            name: name.to_string(),
            status: Status::Skip,
            detail: why.to_string(),
        });
    }

    pub fn count(&self, status: Status) -> usize {
        self.outcomes.iter().filter(|o| o.status == status).count()
    }

    pub fn all_passed(&self) -> bool {
        self.count(Status::Fail) == 0
    }

    /// One-line-per-test summary plus a totals line.
    pub fn summary(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        for o in &self.outcomes {
            let _ = writeln!(
                s,
                "  {:<5} {}{}",
                o.status.as_str(),
                o.name,
                if o.detail.is_empty() {
                    String::new()
                } else {
                    format!("  — {}", o.detail)
                }
            );
        }
        let _ = writeln!(
            s,
            "{} passed, {} failed, {} skipped ({} total)",
            self.count(Status::Pass),
            self.count(Status::Fail),
            self.count(Status::Skip),
            self.outcomes.len()
        );
        s
    }

    /// Minimal JSON, so CI can diff results over time without a serde dependency.
    pub fn to_json(&self) -> String {
        let mut items = Vec::new();
        for o in &self.outcomes {
            items.push(format!(
                r#"{{"name":{},"status":"{}","detail":{}}}"#,
                json_str(&o.name),
                o.status.as_str(),
                json_str(&o.detail)
            ));
        }
        format!("[{}]", items.join(","))
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Locate a WPT checkout for the (future) upstream runner. Set `WPT_DIR` to your
/// `web-platform-tests/wpt` clone.
pub fn find_wpt_checkout() -> Option<PathBuf> {
    std::env::var("WPT_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.join("css").is_dir() || p.join("dom").is_dir())
}

// --- Built-in layout reftests (runnable today) -----------------------------

fn lay(html: &str, css: &str, fonts: &FontContext, width: f32) -> LayoutBox {
    let dom = manuk_html::parse(html);
    let styles = MinimalCascade.cascade(&dom, &[Stylesheet::parse(css)]);
    layout_document(&dom, &styles, fonts, width)
}

/// Collect all block boxes in document order.
fn blocks(root: &LayoutBox) -> Vec<LayoutBox> {
    let mut v = Vec::new();
    root.walk(&mut |b| {
        if b.node.is_some() && matches!(b.content, BoxContent::Block(_) | BoxContent::Inline(_)) {
            v.push(b.clone());
        }
    });
    v
}

fn approx(a: f32, b: f32, tol: f32) -> Result<(), String> {
    if (a - b).abs() <= tol {
        Ok(())
    } else {
        Err(format!("expected ~{b}, got {a}"))
    }
}

/// Run the built-in layout suite. These stand in for WPT `css/CSS2/normal-flow`
/// reftests until the upstream runner lands.
pub fn run_layout_suite(fonts: &FontContext) -> Report {
    let mut r = Report::default();

    // block-in-inline / auto width fills the containing block minus body margin.
    r.push("normal-flow/auto-width-fills-containing-block", {
        let root = lay(
            "<body><div id='d'></div></body>",
            "body{margin:8px} #d{height:10px;background:red}",
            fonts,
            800.0,
        );
        let d = blocks(&root).into_iter().find(|b| b.background.is_some());
        match d {
            Some(b) => approx(b.rect.width, 800.0 - 16.0, 0.5),
            None => Err("no #d box".into()),
        }
    });

    // Blocks stack with no margin collapsing shortfall (heights add up).
    r.push("normal-flow/blocks-stack-vertically", {
        let root = lay(
            "<body style='margin:0'><div style='height:40px'></div><div style='height:60px'></div></body>",
            "",
            fonts,
            600.0,
        );
        if let BoxContent::Block(children) = &root.content {
            if children.len() != 2 {
                Err(format!("expected 2 children, got {}", children.len()))
            } else {
                approx(children[1].rect.y, 40.0, 0.5)
            }
        } else {
            Err("body not a block container".into())
        }
    });

    // display:none produces no box.
    r.push("display/none-generates-no-box", {
        let root = lay(
            "<body style='margin:0'><div style='display:none;height:99px'></div><p>x</p></body>",
            "p{margin:0}",
            fonts,
            400.0,
        );
        let has_99 = {
            let mut found = false;
            root.walk(&mut |b| {
                if (b.rect.height - 99.0).abs() < 0.5 {
                    found = true;
                }
            });
            found
        };
        if has_99 {
            Err("display:none box was laid out".into())
        } else {
            Ok(())
        }
    });

    // Inline text wraps onto multiple lines in a narrow container.
    if fonts.face_count() == 0 {
        r.skip("inline/text-wraps", "no system fonts");
    } else {
        r.push("inline/text-wraps", {
            let root = lay(
                "<body style='margin:0'><p style='margin:0'>the quick brown fox jumps over the lazy dog</p></body>",
                "",
                fonts,
                60.0,
            );
            let mut tops = std::collections::BTreeSet::new();
            root.walk(&mut |b| {
                if let BoxContent::Inline(frags) = &b.content {
                    for f in frags {
                        tops.insert(f.line_top as i32);
                    }
                }
            });
            if tops.len() > 1 {
                Ok(())
            } else {
                Err(format!("expected multiple lines, got {}", tops.len()))
            }
        });
    }

    // Fixed-width block with auto side margins is horizontally centered.
    r.push("normal-flow/auto-margins-center", {
        let root = lay(
            "<body style='margin:0'><div style='width:200px;margin-left:auto;margin-right:auto;height:10px;background:blue'></div></body>",
            "",
            fonts,
            800.0,
        );
        let d = blocks(&root).into_iter().find(|b| b.background.is_some());
        match d {
            Some(b) => approx(b.rect.x, (800.0 - 200.0) / 2.0, 0.5),
            None => Err("no centered box".into()),
        }
    });

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_suite_all_pass() {
        let fonts = FontContext::new();
        let report = run_layout_suite(&fonts);
        assert!(
            report.all_passed(),
            "layout reftests failed:\n{}",
            report.summary()
        );
        // JSON is well-formed enough to start/end as an array.
        let json = report.to_json();
        assert!(json.starts_with('[') && json.ends_with(']'));
    }

    #[test]
    fn wpt_checkout_absent_by_default() {
        // Without WPT_DIR set, the upstream runner has nothing to run.
        if std::env::var("WPT_DIR").is_err() {
            assert!(find_wpt_checkout().is_none());
        }
    }
}
