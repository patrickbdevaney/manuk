//! **Layout-parity harness** — how close is Manuk's layout to Chromium's, measured, tracked,
//! and regression-proof.
//!
//! For each page in a corpus we compare the border-box geometry of every `#p-*` probe
//! element between Manuk and headless Chrome (see [`crate::chrome`]). Comparing *boxes*, not
//! pixels, is deliberate: it measures whether the **layout** agrees (positions, sizes)
//! without being drowned out by font-rasterization differences no two engines share. A probe
//! passes when its worst per-axis delta is within tolerance; a page's score is its passing
//! probes over its total, and the suite score aggregates those.
//!
//! Side-by-side PNGs (Manuk render + Chrome screenshot) are written per page so a human can
//! eyeball what a numeric delta means. Chrome is optional: without it the harness reports the
//! Manuk boxes alone and marks the page "no reference".

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use manuk_page::Page;
use manuk_text::FontContext;

use crate::chrome::{self, Box4};

/// Default per-axis tolerance in px. Explicitly-sized boxes should match near-exactly; a few
/// px of slack absorbs sub-pixel rounding and minor font-metric effects on text boxes.
pub const DEFAULT_TOL: i32 = 3;

/// Manuk's border-box geometry for every `#p-*` probe on a page.
pub fn manuk_boxes(html: &str, url: &str, vw: u32, fonts: &FontContext) -> HashMap<String, Box4> {
    let page = Page::load(html, url, fonts, vw as f32);
    boxes_from_page(&page)
}

fn boxes_from_page(page: &Page) -> HashMap<String, Box4> {
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);
    let mut out = HashMap::new();
    for n in dom.descendants(dom.root()) {
        if let Some(el) = dom.element(n) {
            if let Some(id) = el.id() {
                if id.starts_with("p-") {
                    if let Some(r) = rects.get(&n) {
                        out.insert(
                            id.to_string(),
                            [
                                r.x.round() as i32,
                                r.y.round() as i32,
                                r.width.round() as i32,
                                r.height.round() as i32,
                            ],
                        );
                    }
                }
            }
        }
    }
    out
}

/// One probe's comparison.
#[derive(Clone, Debug)]
pub struct ProbeDelta {
    pub id: String,
    pub manuk: Option<Box4>,
    pub chrome: Option<Box4>,
    /// Worst per-axis absolute difference, or `i32::MAX` if a side is missing.
    pub max_delta: i32,
}

impl ProbeDelta {
    pub fn within(&self, tol: i32) -> bool {
        self.max_delta <= tol
    }
}

/// One page's parity result.
#[derive(Clone, Debug)]
pub struct PageParity {
    pub name: String,
    pub probes: Vec<ProbeDelta>,
    pub have_reference: bool,
    pub note: Option<String>,
}

impl PageParity {
    pub fn within(&self, tol: i32) -> usize {
        self.probes.iter().filter(|p| p.within(tol)).count()
    }
    pub fn total(&self) -> usize {
        self.probes.len()
    }
}

/// The whole suite's result.
#[derive(Clone, Debug, Default)]
pub struct ParityReport {
    pub pages: Vec<PageParity>,
    pub tol: i32,
    pub reference: String,
}

impl ParityReport {
    pub fn probes_within(&self) -> usize {
        self.pages.iter().map(|p| p.within(self.tol)).sum()
    }
    pub fn probes_total(&self) -> usize {
        self.pages.iter().map(PageParity::total).sum()
    }
    pub fn all_within(&self) -> bool {
        self.pages
            .iter()
            .all(|p| !p.have_reference || p.within(self.tol) == p.total())
    }

    /// A human-readable report: per-page pass counts and the worst offenders.
    pub fn summary(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "Layout parity vs {} (tolerance ±{}px, box geometry)\n",
            self.reference, self.tol
        );
        for page in &self.pages {
            if !page.have_reference {
                let _ = writeln!(
                    s,
                    "  {:<28} — no reference{}",
                    page.name,
                    page.note
                        .as_deref()
                        .map(|n| format!(" ({n})"))
                        .unwrap_or_default()
                );
                continue;
            }
            let (w, t) = (page.within(self.tol), page.total());
            let mark = if w == t { "ok " } else { "FAIL" };
            let _ = writeln!(
                s,
                "  [{mark}] {:<28} {w}/{t} probes within ±{}px",
                page.name, self.tol
            );
            // Show the worst few offenders on a failing page.
            if w < t {
                let mut bad: Vec<&ProbeDelta> =
                    page.probes.iter().filter(|p| !p.within(self.tol)).collect();
                bad.sort_by_key(|p| std::cmp::Reverse(p.max_delta));
                for p in bad.iter().take(4) {
                    let _ = writeln!(
                        s,
                        "         {:<16} manuk={:<20} chrome={:<20} Δ={}",
                        p.id,
                        fmt_box(p.manuk),
                        fmt_box(p.chrome),
                        if p.max_delta == i32::MAX {
                            "MISSING".to_string()
                        } else {
                            format!("{}px", p.max_delta)
                        }
                    );
                }
            }
        }
        let _ = writeln!(
            s,
            "\nTOTAL: {}/{} probes within tolerance across {} page(s)",
            self.probes_within(),
            self.probes_total(),
            self.pages.iter().filter(|p| p.have_reference).count()
        );
        s
    }
}

fn fmt_box(b: Option<Box4>) -> String {
    match b {
        Some([x, y, w, h]) => format!("[{x},{y} {w}×{h}]"),
        None => "(absent)".to_string(),
    }
}

/// Compare Manuk vs Chrome boxes for one page.
fn compare(manuk: &HashMap<String, Box4>, chrome: &HashMap<String, Box4>) -> Vec<ProbeDelta> {
    let mut ids: BTreeMap<&String, ()> = BTreeMap::new();
    for k in manuk.keys().chain(chrome.keys()) {
        ids.insert(k, ());
    }
    ids.into_keys()
        .map(|id| {
            let m = manuk.get(id).copied();
            let c = chrome.get(id).copied();
            let max_delta = match (m, c) {
                (Some(a), Some(b)) => (0..4).map(|i| (a[i] - b[i]).abs()).max().unwrap_or(0),
                _ => i32::MAX,
            };
            ProbeDelta {
                id: id.clone(),
                manuk: m,
                chrome: c,
                max_delta,
            }
        })
        .collect()
}

/// Run the parity suite over every `.html` in `corpus_dir`. Writes side-by-side PNG artifacts
/// to `out_dir` when it is `Some` and Chrome is available.
pub fn run_parity(
    corpus_dir: &Path,
    vw: u32,
    vh: u32,
    tol: i32,
    out_dir: Option<&Path>,
    fonts: &FontContext,
) -> ParityReport {
    let have_chrome = chrome::available();
    let reference = chrome::chrome_bin()
        .map(|p| {
            p.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "chrome".into())
        })
        .unwrap_or_else(|| "(no reference browser)".into());

    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(corpus_dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("html"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut pages = Vec::new();
    for path in files {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let Ok(html) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Absolutized — a relative path here yields `file://fixtures/x.html`, in which `fixtures` is
        // the HOSTNAME and every subresource fails to resolve. See `file_url` in `main.rs`.
        let abs = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let url = format!("file://{}", abs.display());
        let manuk = manuk_boxes(&html, &url, vw, fonts);

        // Artifacts: always write the Manuk render; the Chrome shot when available.
        if let Some(dir) = out_dir {
            let page = Page::load(&html, &url, fonts, vw as f32);
            let _ = page
                .paint(fonts, vw, vh)
                .save_png(dir.join(format!("{name}.manuk.png")));
            if have_chrome {
                let _ = chrome::capture_screenshot_png(
                    &html,
                    vw,
                    vh,
                    &dir.join(format!("{name}.chrome.png")),
                );
            }
        }

        if !have_chrome {
            pages.push(PageParity {
                name,
                probes: vec![],
                have_reference: false,
                note: Some("no Chrome/Chromium installed".into()),
            });
            continue;
        }
        match chrome::capture_boxes(&html, vw, vh) {
            Ok(chrome_boxes) => {
                let probes = compare(&manuk, &chrome_boxes);
                pages.push(PageParity {
                    name,
                    probes,
                    have_reference: true,
                    note: None,
                });
            }
            Err(e) => {
                pages.push(PageParity {
                    name,
                    probes: vec![],
                    have_reference: false,
                    note: Some(format!("chrome capture failed: {e}")),
                });
            }
        }
    }

    ParityReport {
        pages,
        tol,
        reference,
    }
}

/// Manuk's box for a single probe id on an already-loaded page (for unit tests).
pub fn probe_rect(page: &Page, id: &str) -> Option<Box4> {
    boxes_from_page(page).get(id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manuk_boxes_reads_probe_geometry() {
        let fonts = FontContext::new();
        // Two stacked, explicitly-sized blocks; the second sits below the first.
        let html = r#"<body style="margin:0">
            <div id="p-a" style="width:100px;height:40px"></div>
            <div id="p-b" style="width:60px;height:20px"></div></body>"#;
        let b = manuk_boxes(html, "x", 800, &fonts);
        assert_eq!(b["p-a"], [0, 0, 100, 40], "first block at origin, 100×40");
        assert_eq!(
            b["p-b"][1], 40,
            "second block stacks below the first (y=40)"
        );
        assert_eq!(b["p-b"][2], 60, "second block is 60 wide");
    }

    /// A live regression gate: when Chrome is available, the layout primitives Manuk already
    /// matches (block flow, positioning) must **stay** matched. This turns the harness into a
    /// guard — a regression in those areas fails the build. Skipped (not failed) where Chrome
    /// is absent, so it never blocks a Chrome-less machine.
    #[test]
    fn known_good_pages_stay_within_tolerance_when_chrome_is_present() {
        if !chrome::available() {
            eprintln!("skip: no Chrome/Chromium — parity regression gate not run");
            return;
        }
        let fonts = FontContext::new();
        let corpus = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus");
        let report = run_parity(&corpus, 800, 600, DEFAULT_TOL, None, &fonts);

        // The whole corpus now matches Chrome; the gate holds all of it to parity so any
        // regression in these primitives fails the build.
        for page in &report.pages {
            if page.have_reference {
                assert_eq!(
                    page.within(report.tol),
                    page.total(),
                    "regression: {} fell out of parity ({}/{} within ±{}px)\n{}",
                    page.name,
                    page.within(report.tol),
                    page.total(),
                    report.tol,
                    report.summary(),
                );
            }
        }
    }

    #[test]
    fn compare_flags_out_of_tolerance_and_missing() {
        let mut m = HashMap::new();
        m.insert("p-a".to_string(), [0, 0, 100, 40]);
        m.insert("p-only-manuk".to_string(), [1, 2, 3, 4]);
        let mut c = HashMap::new();
        c.insert("p-a".to_string(), [0, 0, 102, 40]); // 2px wide diff
        c.insert("p-only-chrome".to_string(), [9, 9, 9, 9]);

        let d = compare(&m, &c);
        let a = d.iter().find(|p| p.id == "p-a").unwrap();
        assert_eq!(a.max_delta, 2);
        assert!(a.within(3) && !a.within(1));
        // A probe present on only one side is a full miss.
        assert_eq!(
            d.iter().find(|p| p.id == "p-only-manuk").unwrap().max_delta,
            i32::MAX
        );
        assert_eq!(
            d.iter()
                .find(|p| p.id == "p-only-chrome")
                .unwrap()
                .max_delta,
            i32::MAX
        );
    }
}
