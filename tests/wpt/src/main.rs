//! `manuk-wpt` — run the conformance suite and report results.
//!
//! - No args: runs the built-in layout reftests.
//! - `--wpt <dir> [subdir]` (or `$WPT_DIR`): runs the **upstream WPT reftests** under
//!   `<dir>/<subdir>` — check the tree out at the commit pinned in IMPLEMENTATION.md
//!   (`7f6164e…`, 2026-07-09) so pass counts are meaningful.

use std::path::PathBuf;

use manuk_text::FontContext;

fn main() {
    let fonts = FontContext::new();
    if fonts.face_count() == 0 {
        eprintln!("note: no system fonts; text-dependent tests will be skipped");
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    // `manuk-wpt parity` — layout-parity vs headless Chrome over a corpus.
    if args.first().map(String::as_str) == Some("parity") {
        run_parity_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt fidelity` — G1: real-site VISUAL parity vs Chromium (ADR-010).
    if args.first().map(String::as_str) == Some("fidelity") {
        run_fidelity_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt bench` — EPOCH probe: per-stage hot-path timings + scaling (§10.2).
    if args.first().map(String::as_str) == Some("bench") {
        run_bench_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt render` — headless screenshot of one page to PNG (autonomous visual check).
    if args.first().map(String::as_str) == Some("render") {
        run_render_cmd(&args[1..], &fonts);
        return;
    }

    let wpt_flag = flag(&args, "--wpt");
    let wpt_dir = wpt_flag
        .map(PathBuf::from)
        .or_else(manuk_wpt::find_wpt_checkout);
    // The subdir is the first positional arg (not `--wpt` and not its value).
    let subdir = args
        .iter()
        .filter(|a| a.as_str() != "--wpt" && Some(a.as_str()) != wpt_flag)
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_default();

    let report = match wpt_dir {
        Some(dir) => {
            eprintln!(
                "running upstream WPT reftests under {}/{}",
                dir.display(),
                subdir
            );
            manuk_wpt::reftest::run_reftests(&dir, &subdir, &fonts)
        }
        None => {
            eprintln!("(no --wpt <dir>/$WPT_DIR — running built-in layout reftests)");
            manuk_wpt::run_layout_suite(&fonts)
        }
    };

    print!("{}", report.summary());
    if !report.all_passed() {
        std::process::exit(1);
    }
}

/// `manuk-wpt parity [--corpus DIR] [--out DIR] [--tol PX] [--width W] [--height H]`
fn run_parity_cmd(args: &[String], fonts: &FontContext) {
    let corpus = flag(args, "--corpus")
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_dir);
    let out = flag(args, "--out").map(PathBuf::from);
    let tol = flag(args, "--tol").and_then(|s| s.parse().ok()).unwrap_or(manuk_wpt::parity::DEFAULT_TOL);
    let vw: u32 = flag(args, "--width").and_then(|s| s.parse().ok()).unwrap_or(800);
    let vh: u32 = flag(args, "--height").and_then(|s| s.parse().ok()).unwrap_or(600);

    if !manuk_wpt::chrome::available() {
        eprintln!(
            "note: no Chrome/Chromium found — writing Manuk renders only, no parity numbers.\n\
             Install google-chrome or chromium to get the vs-Chromium box comparison."
        );
    }
    eprintln!("parity corpus: {}", corpus.display());
    if let Some(o) = &out {
        eprintln!("artifacts:     {}", o.display());
    }
    let report = manuk_wpt::parity::run_parity(&corpus, vw, vh, tol, out.as_deref(), fonts);
    print!("{}", report.summary());
    if !report.all_within() {
        std::process::exit(1);
    }
}

/// `manuk-wpt render (--html FILE | --inline HTML) --out PNG [--width W] [--height H] [--chrome]`
///
/// Headlessly renders one page through Manuk's CPU painter (no window/GPU) and writes a PNG — the
/// autonomous **visual** check: render, then read the PNG back (it is a real screenshot of what
/// Manuk draws). With `--chrome`, also writes `<out>.chrome.png` from headless Chrome for an
/// eyeball comparison. Self-contained HTML only (no network); inline `<style>`/`<script>` run.
fn run_render_cmd(args: &[String], fonts: &FontContext) {
    use manuk_page::Page;

    let vw: u32 = flag(args, "--width").and_then(|s| s.parse().ok()).unwrap_or(1280);
    let vh: u32 = flag(args, "--height").and_then(|s| s.parse().ok()).unwrap_or(800);
    let out = flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("render.png"));
    let has_chrome = args.iter().any(|a| a == "--chrome");

    let (html, mut url) = if let Some(f) = flag(args, "--html") {
        let html = std::fs::read_to_string(f).unwrap_or_else(|e| {
            eprintln!("cannot read {f}: {e}");
            std::process::exit(1);
        });
        (html, format!("file://{f}"))
    } else if let Some(inline) = flag(args, "--inline") {
        (inline.to_string(), "about:inline".to_string())
    } else {
        eprintln!("usage: manuk-wpt render (--html FILE | --inline HTML) --out PNG [--width W] [--height H] [--chrome]");
        std::process::exit(2);
    };

    // `--url URL` overrides the document's own URL, so a page saved to disk still resolves its
    // relative `<link href>` / `<img src>` against the real origin (without it they'd resolve
    // against `file://…` and silently fail — rendering the page undressed).
    if let Some(u) = flag(args, "--url") {
        url = u.to_string();
    }

    // Load through the **async** path so external `<link rel=stylesheet>` and `<img>` actually
    // fetch — a real page's visual identity lives in its external CSS, and rendering it undressed
    // would make any Chrome comparison dishonest. `--offline` keeps the old sync (no-network) path
    // for self-contained fixtures.
    let offline = args.iter().any(|a| a == "--offline");
    let page = if offline {
        Page::load(&html, &url, fonts, vw as f32)
    } else {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            let mut p = Page::load_async(&html, &url, fonts, vw as f32).await;
            let sheets = p.fetch_and_apply_stylesheets(fonts, vw as f32).await;
            if sheets > 0 {
                eprintln!("applied {sheets} external stylesheet(s)");
            }
            p
        })
    };
    match page.paint(fonts, vw, vh).save_png(&out) {
        Ok(()) => eprintln!("wrote {} ({}x{})", out.display(), vw, vh),
        Err(e) => {
            eprintln!("failed to write {}: {e}", out.display());
            std::process::exit(1);
        }
    }
    if has_chrome && manuk_wpt::chrome::available() {
        let chrome_out = out.with_extension("chrome.png");
        match manuk_wpt::chrome::capture_screenshot_png(&html, vw, vh, &chrome_out) {
            Ok(()) => eprintln!("wrote {} (headless Chrome reference)", chrome_out.display()),
            Err(e) => eprintln!("chrome screenshot failed: {e}"),
        }
    }
}

/// `manuk-wpt fidelity --urls https://a,https://b [--out DIR] [--width W] [--height H] [--floor 0.9]`
///
/// **G1 (ADR-010).** Renders each real URL through Manuk's full pipeline (external CSS, images,
/// JS), screenshots Chromium rendering the same URL, and compares them **visually**. Box probes
/// measure geometry; this measures what the user actually sees.
fn run_fidelity_cmd(args: &[String], fonts: &FontContext) {
    use manuk_page::Page;

    let vw: u32 = flag(args, "--width").and_then(|s| s.parse().ok()).unwrap_or(1200);
    let vh: u32 = flag(args, "--height").and_then(|s| s.parse().ok()).unwrap_or(800);
    let floor: f64 = flag(args, "--floor").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let out = flag(args, "--out").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let Some(urls) = flag(args, "--urls") else {
        eprintln!("usage: manuk-wpt fidelity --urls URL[,URL...] [--out DIR] [--floor 0.9]");
        std::process::exit(2);
    };
    let _ = std::fs::create_dir_all(&out);

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("rt");
    let mut rows = Vec::new();

    for url in urls.split(',').map(str::trim).filter(|u| !u.is_empty()) {
        let name = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or(url)
            .to_string();
        eprintln!("fidelity: {name}");

        // Manuk — the real pipeline (fetch, external CSS, images, JS), same as a user gets.
        let Ok((html, final_url)) = rt.block_on(manuk_page::fetch_html(url)) else {
            eprintln!("  fetch failed, skipping");
            continue;
        };
        let page = rt.block_on(async {
            let mut p = Page::load_async(&html, &final_url, fonts, vw as f32).await;
            p.fetch_and_apply_stylesheets(fonts, vw as f32).await;
            p
        });
        let mpath = out.join(format!("{name}.manuk.png"));
        if page.paint(fonts, vw, vh).save_png(&mpath).is_err() {
            eprintln!("  manuk render failed");
            continue;
        }

        // Chromium — the same live URL, so it fetches its own subresources.
        let cpath = out.join(format!("{name}.chrome.png"));
        if let Err(e) = manuk_wpt::chrome::capture_url_screenshot(url, vw, vh, &cpath) {
            eprintln!("  chrome: {e}");
            continue;
        }

        match manuk_wpt::fidelity::compare(&mpath, &cpath, &name) {
            Ok(mut f) => {
                let side = out.join(format!("{name}.SIDE.png"));
                let _ = manuk_wpt::fidelity::write_side_by_side(&mpath, &cpath, &side);
                eprintln!("  side-by-side: {}", side.display());

                // STRUCTURAL half — the honest number. Compare Chrome's box for every rendered
                // `[id]` element against Manuk's. A missing sidebar is a MISSING BOX; the pixel
                // score averages it away, this does not.
                if let Ok(cboxes) = manuk_wpt::chrome::capture_boxes_all_ids(url, vw, vh) {
                    let rects = page.root_box.node_rects(page.dom());
                    let mut mboxes: std::collections::HashMap<String, [i64; 4]> =
                        std::collections::HashMap::new();
                    for n in page.dom().descendants(page.dom().root()) {
                        if let Some(id) = page.dom().element(n).and_then(|e| e.attr("id")) {
                            if let Some(r) = rects.get(&n) {
                                if r.width > 0.0 || r.height > 0.0 {
                                    mboxes.insert(
                                        id.to_string(),
                                        [
                                            r.x.round() as i64,
                                            r.y.round() as i64,
                                            r.width.round() as i64,
                                            r.height.round() as i64,
                                        ],
                                    );
                                }
                            }
                        }
                    }
                    let (sc, missing, misplaced, probed) =
                        manuk_wpt::fidelity::compare_structure(&cboxes.iter().map(|(k,v)| (k.clone(), [v[0] as i64, v[1] as i64, v[2] as i64, v[3] as i64])).collect(), &mboxes, 8);
                    f.structure = Some(sc);
                    f.missing = missing;
                    f.misplaced = misplaced;
                    f.probed = probed;
                    eprintln!("  structural: {:.1}% ({probed} ids, {missing} missing, {misplaced} misplaced)", sc * 100.0);
                }
                rows.push(f);
            }
            Err(e) => eprintln!("  compare: {e}"),
        }
    }

    let ok = manuk_wpt::fidelity::report(&rows, floor);
    if !ok && floor > 0.0 {
        std::process::exit(1);
    }
}

/// `manuk-wpt bench --pages a.html,b.html [--runs N] [--width W]`
///
/// EPOCH probe (CONSTITUTION §10.2): per-stage hot-path timings on pages of increasing size, with
/// per-KB / per-node costs so **superlinear scaling** is visible by inspection.
fn run_bench_cmd(args: &[String], fonts: &FontContext) {
    let runs: usize = flag(args, "--runs").and_then(|s| s.parse().ok()).unwrap_or(5);
    let vw: f32 = flag(args, "--width").and_then(|s| s.parse().ok()).unwrap_or(1280.0);
    let vh: u32 = flag(args, "--height").and_then(|s| s.parse().ok()).unwrap_or(900);
    let Some(list) = flag(args, "--pages") else {
        eprintln!("usage: manuk-wpt bench --pages f1.html,f2.html [--runs N] [--width W]");
        std::process::exit(2);
    };
    let mut rows = Vec::new();
    for path in list.split(',') {
        let path = path.trim();
        let Ok(html) = std::fs::read_to_string(path) else {
            eprintln!("skip (unreadable): {path}");
            continue;
        };
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        eprintln!("benching {name} ({} KB, {runs} runs)…", html.len() / 1024);
        rows.push(manuk_wpt::bench::bench_page(
            &name,
            &html,
            &format!("file://{path}"),
            vw,
            vh,
            fonts,
            runs,
        ));
    }
    manuk_wpt::bench::report(&rows);
}

/// The in-repo corpus next to this crate: `tests/wpt/corpus`.
fn default_corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().map(String::as_str);
        }
    }
    None
}
