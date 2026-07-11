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
