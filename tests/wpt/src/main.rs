//! `manuk-wpt` — run the conformance suite and report results.
//!
//! - No args: runs the built-in layout reftests.
//! - `--wpt <dir> [subdir]` (or `$WPT_DIR`): runs the **upstream WPT reftests** under
//!   `<dir>/<subdir>` — check the tree out at the commit pinned in IMPLEMENTATION.md
//!   (`7f6164e…`, 2026-07-09) so pass counts are meaningful.

use std::path::PathBuf;

use manuk_text::FontContext;

/// A `file://` URL for a local path — **absolutized**.
///
/// `format!("file://{path}")` on a RELATIVE path silently produces `file://tests/spa/index.html`,
/// in which `tests` is parsed as the **hostname** and the path is gone. Every subresource then
/// resolves against a host that does not exist, and the fetch fails.
///
/// This had been quietly breaking every local-file test that loads a subresource. It is why the React
/// app "mounted, threw nothing and rendered nothing": its bundle was never fetched, so **not one line
/// of React ever ran.** The framework was never the defect — the harness could not load it, and the
/// harness's failure looked exactly like the framework's.
///
/// Lesson, again, and it keeps being the same one: *when the instruments say the bug is impossible,
/// they are all sampling the same layer.* Test your own primitives before blaming the framework.
fn file_url(path: &str) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path));
    format!("file://{}", abs.display())
}

fn main() {
    run();
    // SpiderMonkey's atexit handler segfaults if the process exits with a live JSContext —
    // *after* main returns, so the output looks fine and the exit code is 139. Tear the runtime
    // down explicitly, once every Page has been dropped.
    manuk_net::webstorage::save();
    manuk_js::shutdown();
}

fn run() {
    // Engine diagnostics (a script that threw, a subresource that failed) go through `tracing`.
    // `RUST_LOG=debug cargo run -p manuk-wpt -- ...` to see them.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let fonts = FontContext::new();
    if fonts.face_count() == 0 {
        eprintln!("note: no system fonts; text-dependent tests will be skipped");
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    // `manuk-wpt wpt <subset>` — the UPSTREAM testharness.js suite. See `harness.rs`.
    if args.first().map(String::as_str) == Some("wpt") {
        run_wpt_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt diag <file>` — **run ONE test file and say what actually happened inside it.**
    //
    // "TH_TIMEOUT — the async test never completed" is a *status*, not a finding. It told me nothing
    // three separate times while I guessed at causes. A test that creates 8,000 subtests and completes
    // none of them threw an exception somewhere in the middle, and the browser knows exactly where —
    // it just had nobody to tell. This is the instrument that asks.
    if args.first().map(String::as_str) == Some("diag") {
        let Some(rel) = args.get(1) else {
            eprintln!("usage: manuk-wpt diag <path/relative/to/wpt>");
            std::process::exit(2);
        };
        // `diag` reads what a page's SCRIPTS experienced — it is meaningless without the JS engine, and
        // its `eval_for_test` seam only exists under the `spidermonkey` feature. Gating the whole path
        // keeps the **headless** `cargo build --workspace --no-default-features` (CI's gating lane)
        // compiling — the lane my tick-84 addition of this subcommand silently broke.
        #[cfg(feature = "spidermonkey")]
        diag(rel.clone(), &fonts);
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = rel;
            eprintln!("diag needs the `spidermonkey` feature (it reads what a page's scripts did)");
            std::process::exit(2);
        }
        return;
    }

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

    // `manuk-wpt interact` — G5: INTERACTION parity. A page that renders like Chromium but does not
    // respond like Chromium is a screenshot, not a browser.
    if args.first().map(String::as_str) == Some("interact") {
        run_interact_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt oracle` — THE DIFFERENTIAL ORACLE (METHODOLOGY Part 2). Chromium as an infinite
    // test generator: same document, both engines, diff, cluster by root cause, rank by sites.
    if args.first().map(String::as_str) == Some("oracle-merge") {
        run_oracle_merge(&args[1..]);
        return;
    }
    if args.first().map(String::as_str) == Some("oracle") {
        run_oracle_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt hittest` — the LINK-CLICK flow, reproduced headlessly. Take every `<a href>` on a
    // real page, hit-test its own centre, and see whether the browser finds it again.
    if args.first().map(String::as_str) == Some("hittest") {
        run_hittest_cmd(&args[1..], &fonts);
        return;
    }

    // `manuk-wpt boxes` — dump Manuk's rect for every `[id]` element. The counterpart of Chrome's
    // `getBoundingClientRect` probe: annotate any element with an id and the two engines' geometry
    // becomes directly comparable. A screenshot shows THAT the layout is wrong; this shows BY HOW
    // MUCH, and for which box.
    // **`firstpaint` — how long until there is something on the screen.**
    //
    // `boxes --fetch` measures `load_async` + `finish_loading`, which is NOT the path the shell takes.
    // The shell uses `prefetch_document` + `from_prefetched`, and that is what a user's first pixel
    // actually waits for. Measuring the wrong path is how "the browser feels slow" survives a green
    // benchmark — the number was real, it was just a number about something else.
    if args.first().map(String::as_str) == Some("firstpaint") {
        let url: String = flag(&args, "--url").expect("--url required").to_string();
        let fonts = manuk_text::FontContext::new();
        let rt = manuk_net::runtime();
        let t0 = std::time::Instant::now();
        let loaded = rt
            .block_on(manuk_page::prefetch_document(&url))
            .expect("prefetch");
        let t_fetch = t0.elapsed().as_secs_f64() * 1000.0;
        let page = match loaded {
            // The SHELL's path: blocking scripts only. The deferred ones run after paint.
            manuk_page::Loaded::Prefetched(pre) => {
                manuk_page::Page::from_prefetched_blocking_only(*pre, &fonts, 1200.0)
            }
            // WPT runs from local files, which carry no response headers and so no CSP.
            manuk_page::Loaded::Document {
                html, final_url, ..
            } => manuk_page::Page::load(&html, &final_url, &fonts, 1200.0),
            _ => panic!("not a document"),
        };
        let t_paint = t0.elapsed().as_secs_f64() * 1000.0;
        // Everything below happens with the document already on the screen.
        let mut page = page;
        let t2 = std::time::Instant::now();
        let n_def = page.run_deferred_scripts(&fonts, 1200.0);
        let t_def = t2.elapsed().as_secs_f64() * 1000.0;
        // Now the images, which in a real browser arrive AFTER the page is on screen.
        let urls = page.pending_image_urls();
        let n_img = urls.len();
        let t1 = std::time::Instant::now();
        let imgs = rt.block_on(manuk_page::fetch_image_urls(urls));
        let t_img = t1.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  FIRST PAINT {t_paint:7.1}ms  (fetch+parse {t_fetch:7.1}ms)   \
             then {n_def} deferred scripts in {t_def:7.1}ms, {n_img} images in {t_img:7.1}ms",
        );
        let _ = imgs.len();
        return;
    }

    if args.first().map(String::as_str) == Some("boxes") {
        run_boxes_cmd(&args[1..], &fonts);
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
    let tol = flag(args, "--tol")
        .and_then(|s| s.parse().ok())
        .unwrap_or(manuk_wpt::parity::DEFAULT_TOL);
    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

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

    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1280);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    let out = flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("render.png"));
    let has_chrome = args.iter().any(|a| a == "--chrome");

    let (html, mut url) = if let Some(f) = flag(args, "--html") {
        let html = std::fs::read_to_string(f).unwrap_or_else(|e| {
            eprintln!("cannot read {f}: {e}");
            std::process::exit(1);
        });
        (html, file_url(f))
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
            p.finish_loading(fonts, vw as f32).await;
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

    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    let floor: f64 = flag(args, "--floor")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let out = flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let Some(urls) = flag(args, "--urls") else {
        eprintln!("usage: manuk-wpt fidelity --urls URL[,URL...] [--out DIR] [--floor 0.9]");
        std::process::exit(2);
    };
    let _ = std::fs::create_dir_all(&out);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
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

        // **Time each engine separately, and attribute the cost to whoever actually spent it.**
        //
        // A sweep with one wall-clock budget per site cannot tell "our engine hung" from "Chromium
        // hung", and it will report both as *our* failure. That is not hypothetical: w3schools and
        // go.dev both came back HANG/FAIL at a 75s budget, which would have made "fix the page-load
        // hang" the single highest-priority item in the project — Pass 1, above everything. Timing
        // the halves says our engine renders w3schools in **2.7s** and *Chromium* takes **21s** on
        // the same bytes. The hang was the oracle's. The bug did not exist.
        //
        // This is the same hazard as `oracle::oracle_is_healthy` and it gets the same treatment:
        // make the mis-attribution impossible in code, not a thing to remember. An oracle you are
        // measuring yourself against must never be able to charge its own slowness to your account.
        let t_manuk = std::time::Instant::now();
        let Ok((html, final_url)) = rt.block_on(manuk_page::fetch_html(url)) else {
            eprintln!("  fetch failed, skipping");
            continue;
        };
        let page = rt.block_on(async {
            let mut p = Page::load_async(&html, &final_url, fonts, vw as f32).await;
            p.finish_loading(fonts, vw as f32).await;
            p
        });
        let mpath = out.join(format!("{name}.manuk.png"));
        if page.paint(fonts, vw, vh).save_png(&mpath).is_err() {
            eprintln!("  manuk render failed");
            continue;
        }
        let manuk_ms = t_manuk.elapsed().as_millis();

        // Chromium — the same live URL, so it fetches its own subresources.
        let t_chrome = std::time::Instant::now();
        let cpath = out.join(format!("{name}.chrome.png"));
        if let Err(e) = manuk_wpt::chrome::capture_url_screenshot(url, vw, vh, &cpath) {
            eprintln!("  chrome: {e}");
            continue;
        }
        let chrome_ms = t_chrome.elapsed().as_millis();
        eprintln!("  load: manuk {manuk_ms}ms · chromium {chrome_ms}ms");
        if manuk_ms > 10_000 {
            eprintln!(
                "  ** OURS IS SLOW: {manuk_ms}ms to load and paint. That is a bug in this engine and \
                 it belongs in the ledger. **"
            );
        } else if chrome_ms > 3 * manuk_ms.max(1) && chrome_ms > 10_000 {
            eprintln!(
                "  (chromium took {chrome_ms}ms against our {manuk_ms}ms — the ORACLE is the slow \
                 one here; do not book this as our latency)"
            );
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
                    let cmap: std::collections::HashMap<String, [i64; 4]> = cboxes
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                [v[0] as i64, v[1] as i64, v[2] as i64, v[3] as i64],
                            )
                        })
                        .collect();
                    let (sc, missing, misplaced, probed, missing_ids) =
                        manuk_wpt::fidelity::compare_structure_detail(&cmap, &mboxes, 8);
                    // Which elements are missing? A coverage number is only actionable if it names
                    // the culprits — and 1,402 missing elements are a handful of CLASS bugs, not
                    // 1,402 bugs. Print the tag of each missing id so the class is visible.
                    if !missing_ids.is_empty() {
                        let mut by_tag: std::collections::BTreeMap<String, usize> =
                            std::collections::BTreeMap::new();
                        for id in &missing_ids {
                            let tag = page
                                .dom()
                                .descendants(page.dom().root())
                                .find(|&n| {
                                    page.dom().element(n).and_then(|e| e.attr("id"))
                                        == Some(id.as_str())
                                })
                                .and_then(|n| page.dom().tag_name(n))
                                .unwrap_or("(not-in-dom)")
                                .to_string();
                            *by_tag.entry(tag).or_default() += 1;
                        }
                        let mut v: Vec<_> = by_tag.into_iter().collect();
                        v.sort_by(|a, b| b.1.cmp(&a.1));
                        eprintln!(
                            "  MISSING by tag: {}",
                            v.iter()
                                .take(8)
                                .map(|(t, c)| format!("{t}×{c}"))
                                .collect::<Vec<_>>()
                                .join("  ")
                        );
                        // A count says *how much* is missing; only the ids say *what*. 1,402 missing
                        // elements are never 1,402 bugs — they are a few CLASS bugs, and a sample of
                        // the actual ids is what identifies the class.
                        eprintln!(
                            "  MISSING sample: {}",
                            missing_ids
                                .iter()
                                .take(12)
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    f.structure = Some(sc);
                    f.missing = missing;
                    f.misplaced = misplaced;
                    f.probed = probed;
                    eprintln!("  structural: {:.1}% ({probed} ids, {missing} missing, {misplaced} misplaced)", sc * 100.0);
                    if let Some((last_ok, _, first_bad, dy)) =
                        manuk_wpt::fidelity::first_divergence(&cmap, &mboxes, 60)
                    {
                        eprintln!("  FIRST DIVERGENCE: after #{last_ok}, element #{first_bad} is off by dy={dy}");
                    }
                    let (dx, dy, dw, dh, within) =
                        manuk_wpt::fidelity::placement_stats(&cmap, &mboxes, 8);
                    eprintln!(
                        "  PLACEMENT: {:.1}% within 8px | median offset dx={dx} dy={dy} dw={dw} dh={dh}",
                        within * 100.0
                    );
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
    // F4 — INTERACTIVE LATENCY. Reported apart from the load stages, because a browser that loads
    // fast and then stutters on every wheel event is not fast — and the load bench cannot see it.
    if args.iter().any(|a| a == "--interactive") {
        let pages = flag(args, "--pages").unwrap_or_default();
        let runs: usize = flag(args, "--runs")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);
        println!(
            "\n=== F4 · INTERACTIVE LATENCY (median of {runs}, ms) — floor: ONE FRAME (16ms) ===\n"
        );
        println!("{:<20}{:>12}{:>12}", "page", "scroll", "click");
        for p in pages.split(',').filter(|p| !p.trim().is_empty()) {
            let Ok(html) = std::fs::read_to_string(p.trim()) else {
                eprintln!("cannot read {p}");
                continue;
            };
            let name = std::path::Path::new(p.trim())
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let (scroll, click) = manuk_wpt::bench::bench_interactive(
                &name,
                &html,
                "https://bench.test/",
                1200.0,
                800,
                fonts,
                runs,
            );
            let over = if scroll.max(click) > 16.0 {
                "   <-- OVER ONE FRAME"
            } else {
                ""
            };
            println!("{:<20}{:>12.2}{:>12.2}{over}", name, scroll, click);
        }
        println!();
        return;
    }
    let runs: usize = flag(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let vw: f32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1280.0);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(900);
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
            &file_url(path),
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

/// `manuk-wpt boxes --html FILE [--url URL] [--width W] [--height H]` — print `id x y w h` for every
/// element carrying an `id`, in document-y order.
fn run_boxes_cmd(args: &[String], fonts: &manuk_text::FontContext) {
    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    // `--fetch URL` probes a LIVE page — the boxes of the document as a user would actually get it,
    // subresources and scripts included. A local snapshot cannot stand in for it: relative `<link>`s
    // do not resolve from `file://`, so the CSS silently does not load and every box you measure is
    // the box of an unstyled page. That mistake cost real time here; a probe that only works on
    // local files invites it.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let (html, url) = if let Some(u) = flag(args, "--fetch") {
        match rt.block_on(manuk_page::fetch_html(u)) {
            Ok((h, final_url)) => (h, final_url),
            Err(e) => {
                eprintln!("fetch {u} failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let Some(f) = flag(args, "--html") else {
            eprintln!(
                "usage: manuk-wpt boxes (--html FILE [--url URL] | --fetch URL) [--width W] [--tree ID]"
            );
            std::process::exit(2);
        };
        let h = std::fs::read_to_string(f).unwrap_or_else(|e| {
            eprintln!("cannot read {f}: {e}");
            std::process::exit(1);
        });
        let u = flag(args, "--url")
            .map(String::from)
            .unwrap_or_else(|| file_url(f));
        (h, u)
    };
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(&html, &url, fonts, vw as f32).await;
        p.finish_loading(fonts, vw as f32).await;
        p
    });
    let _ = vh;
    // `--backgrounds` lists every element that carries a `background-image`, with its box. A page
    // that looks like one enormous image is either one enormous element or one badly-placed layer,
    // and this is the two-second way to tell which.
    if args.iter().any(|a| a == "--backgrounds") {
        let rects = page.root_box.node_rects(page.dom());
        let dom = page.dom();
        println!(
            "{:<26} {:<14} {:<10} {:<10} {:<32} {}",
            "element", "natural", "size", "repeat", "url", "box"
        );
        for (&n, st) in page.styles_map().iter() {
            let Some(u) = st.background_images.iter().find_map(|i| match i {
                manuk_css::BackgroundImage::Url(u) => Some(u),
                _ => None,
            }) else {
                continue;
            };
            let tag = dom.tag_name(n).unwrap_or("?");
            let id = dom.element(n).and_then(|e| e.attr("id")).unwrap_or("");
            let cls = dom.element(n).and_then(|e| e.attr("class")).unwrap_or("");
            let name = format!("{tag}#{id}.{}", cls.split_whitespace().next().unwrap_or(""));
            let bx = rects
                .get(&n)
                .map(|r| format!("[{:.0} {:.0} {:.0}x{:.0}]", r.x, r.y, r.width, r.height))
                .unwrap_or_else(|| "(no box)".into());
            let u = if u.len() > 30 {
                &u[u.len() - 30..]
            } else {
                u.as_str()
            };
            let nat = page
                .decoded_images()
                .get(&n)
                .map(|i| format!("{}x{}", i.width, i.height))
                .unwrap_or_else(|| "(not loaded)".into());
            println!(
                "{name:<26} {:<14} {:<10} {:<10} {u:<32} {bx}",
                nat,
                format!("{:?}", st.background_size),
                format!("{:?}", st.background_repeat)
            );
        }
        return;
    }
    // The <html> class is the page's own switch board (`client-nojs` → `client-js`, dark mode,
    // feature flags). If a script failed to flip it, everything downstream is styled for the wrong
    // world — so print what it ended up as.
    {
        let dom = page.dom();
        if let Some(h) = dom.find_first("html") {
            eprintln!(
                "<html class=\"{}\">",
                dom.element(h).and_then(|e| e.attr("class")).unwrap_or("")
            );
        }
    }

    // `--build` — the UI-THREAD cost of a navigation: everything between "the bytes are here" and
    // "the page is on screen". This is the number a person feels when they click a link, and it is
    // the one number no existing bench reported, because `bench` measures ONE parse+cascade+layout
    // and a real load runs several.
    if args.iter().any(|a| a == "--build") {
        let runs = 3;
        let mut best = f64::MAX;
        for _ in 0..runs {
            let t = std::time::Instant::now();
            let (p, t_load) = rt.block_on(async {
                let mut p = manuk_page::Page::load_async(&html, &url, fonts, vw as f32).await;
                let a = t.elapsed().as_secs_f64() * 1000.0;
                p.finish_loading(fonts, vw as f32).await;
                (p, a)
            });
            std::hint::black_box(&p);
            let total = t.elapsed().as_secs_f64() * 1000.0;
            let (n, dup) = manuk_net::fetch_stats();
            // **The wire, not the call.** A repeat `fetch()` served from the HTTP cache or the
            // per-navigation negative cache costs no bandwidth. The same URL going to the NETWORK
            // twice does — that is the number a browser has to keep at zero, and it is what G_DEDUP
            // asserts on. Reporting only `DUP` conflated a free repeat with an expensive one.
            let net = manuk_net::NET_REQUESTS.load(std::sync::atomic::Ordering::Relaxed);
            let netdup = manuk_net::NET_DUPES.load(std::sync::atomic::Ordering::Relaxed);
            let layouts = manuk_layout::LAYOUTS.swap(0, std::sync::atomic::Ordering::Relaxed);
            #[cfg(feature = "stylo")]
            let cascades =
                manuk_css::stylo_engine::CASCADES.swap(0, std::sync::atomic::Ordering::Relaxed);
            #[cfg(not(feature = "stylo"))]
            let cascades = 0u64;
            if total < best {
                best = total;
                println!(
                    "  load_async {t_load:7.1}ms   finish_loading {:7.1}ms   TOTAL {total:7.1}ms   \
                     calls {n} (repeat {dup})  NET {net} (DUP {netdup})  LAYOUTS {layouts}  \
                     CASCADES {cascades}",
                    total - t_load
                );
            }
            manuk_net::reset_fetch_stats();
        }
        println!("build (best of {runs}): {best:.1}ms");
        return;
    }
    // `--images` — every decoded bitmap the page actually got, with its box and whether its pixels
    // are a single flat colour (a decode that "succeeded" into a grey rectangle looks exactly like a
    // successful decode from every angle except the screen).
    if args.iter().any(|a| a == "--images") {
        let rects = page.root_box.node_rects(page.dom());
        let dom = page.dom();
        println!(
            "{:<28} {:<12} {:<22} {}",
            "element", "natural", "pixels", "box"
        );
        for (&n, img) in page.decoded_images() {
            let tag = dom.tag_name(n).unwrap_or("?");
            let cls = dom
                .element(n)
                .and_then(|e| e.attr("class"))
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            let px = &img.rgba;
            let flat = px.len() >= 4 && px.chunks_exact(4).all(|c| c[..3] == px[..3]);
            let desc = if flat && px.len() >= 4 {
                format!("FLAT #{:02x}{:02x}{:02x}", px[0], px[1], px[2])
            } else {
                "(real content)".to_string()
            };
            let bx = rects
                .get(&n)
                .map(|r| format!("[{:.0} {:.0} {:.0}x{:.0}]", r.x, r.y, r.width, r.height))
                .unwrap_or_else(|| "(no box)".into());
            println!(
                "{:<28} {:<12} {desc:<22} {bx}",
                format!("{tag}.{cls}"),
                format!("{}x{}", img.width, img.height)
            );
        }
        return;
    }
    // `--paint SUBSTR` — what the RASTERIZER is actually asked to draw. The gap between "the box is
    // laid out correctly" and "the user can see it" is where invisible-content bugs live, and no
    // geometry probe can see into it.
    if let Some(want) = flag(args, "--paint") {
        let dl = manuk_paint::DisplayList::build_with_images(&page.root_box, page.decoded_images());
        let mut n = 0;
        if want == "BIGTEXT" {
            // A glyph rasterized at an absurd size paints a flat blob the size of a paragraph. It is
            // a TEXT item, so every rect-oriented probe steps straight over it.
            for it in &dl.items {
                if let manuk_paint::DisplayItem::Text {
                    x,
                    baseline,
                    text,
                    style,
                } = it
                {
                    if style.font_size > 40.0 {
                        println!(
                            "size={:6.1} #{:02x}{:02x}{:02x} at [{x:.0} {baseline:.0}] {:?}",
                            style.font_size,
                            style.color.r,
                            style.color.g,
                            style.color.b,
                            text.chars().take(24).collect::<String>()
                        );
                    }
                }
            }
            return;
        }
        if want == "BAND" {
            for it in &dl.items {
                let (r, kind) = match it {
                    manuk_paint::DisplayItem::Rect { rect, color } => (
                        *rect,
                        format!(
                            "Rect #{:02x}{:02x}{:02x}a{}",
                            color.r, color.g, color.b, color.a
                        ),
                    ),
                    manuk_paint::DisplayItem::RoundRect { rect, color, .. } => (
                        *rect,
                        format!("RoundRect #{:02x}{:02x}{:02x}", color.r, color.g, color.b),
                    ),
                    manuk_paint::DisplayItem::MaskedRect { rect, color, .. } => (
                        *rect,
                        format!("MaskedRect #{:02x}{:02x}{:02x}", color.r, color.g, color.b),
                    ),
                    manuk_paint::DisplayItem::Gradient { rect, stops, .. } => {
                        (*rect, format!("Gradient {} stops", stops.len()))
                    }
                    manuk_paint::DisplayItem::Image { rect, image, .. } => {
                        (*rect, format!("Image {}x{}", image.width, image.height))
                    }
                    manuk_paint::DisplayItem::BackgroundImage {
                        rect,
                        image,
                        size,
                        repeat,
                        ..
                    } => (
                        *rect,
                        format!(
                            "BgImage {}x{} {size:?} {repeat:?}",
                            image.width, image.height
                        ),
                    ),
                    manuk_paint::DisplayItem::Shadow {
                        rect, color, blur, ..
                    } => (
                        *rect,
                        format!(
                            "Shadow #{:02x}{:02x}{:02x} blur={blur:.0}",
                            color.r, color.g, color.b
                        ),
                    ),
                    manuk_paint::DisplayItem::TextLine {
                        x,
                        y,
                        width,
                        thickness,
                        color,
                    } => (
                        manuk_layout::Rect {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *thickness,
                        },
                        format!("TextLine #{:02x}{:02x}{:02x}", color.r, color.g, color.b),
                    ),
                    manuk_paint::DisplayItem::Text {
                        x,
                        baseline,
                        text,
                        style,
                    } => {
                        if *baseline > 240.0 && *baseline < 360.0 {
                            println!(
                                "TEXT size={:5.1} #{:02x}{:02x}{:02x} at [{x:.0} {baseline:.0}] {:?}",
                                style.font_size, style.color.r, style.color.g, style.color.b,
                                text.chars().take(18).collect::<String>()
                            );
                        }
                        continue;
                    }
                };
                if r.y < 350.0 && r.y + r.height > 250.0 && r.x < 500.0 && r.x + r.width > 230.0 {
                    println!(
                        "{kind:<34} [{:.0} {:.0} {:.0}x{:.0}]",
                        r.x, r.y, r.width, r.height
                    );
                }
            }
            return;
        }
        if want == "NOCLIP" {
            // The one difference left between the probe's display list (which is CORRECT) and the
            // real render (which is not): the real one goes through `with_layers` with the page's
            // z-index and clip maps. Render once with the clip map emptied and diff.
            let z = std::collections::HashMap::new();
            let clip = std::collections::HashMap::new();
            use manuk_paint::Painter;
            let c = manuk_paint::CpuPainter::with_layers(fonts, page.decoded_images(), &z, &clip)
                .render(&page.root_box, vw, 800, manuk_css::Rgba::WHITE);
            let _ = c.save_png(std::path::Path::new("/tmp/manuk-noclip.png"));
            println!("wrote /tmp/manuk-noclip.png (empty z + clip maps)");
            return;
        }
        if want == "IMAGES" {
            for it in &dl.items {
                match it {
                    manuk_paint::DisplayItem::Image {
                        rect,
                        image,
                        content_clip,
                    } => println!(
                        "IMAGE      {}x{} -> [{:.0} {:.0} {:.0}x{:.0}]  ({})",
                        image.width,
                        image.height,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        if content_clip.is_some() {
                            "object-fit: cropped to box"
                        } else {
                            "fitted/stretched to the box"
                        }
                    ),
                    manuk_paint::DisplayItem::BackgroundImage {
                        rect,
                        image,
                        size,
                        repeat,
                        ..
                    } => {
                        println!(
                            "BACKGROUND {}x{} -> [{:.0} {:.0} {:.0}x{:.0}]  size={size:?} repeat={repeat:?}",
                            image.width, image.height, rect.x, rect.y, rect.width, rect.height
                        )
                    }
                    _ => {}
                }
            }
            return;
        }
        for it in &dl.items {
            if let manuk_paint::DisplayItem::Text {
                x,
                baseline,
                text,
                style,
            } = it
            {
                if text.contains(want) {
                    let c = style.color;
                    println!(
                        "TEXT {text:?} at x={x:.0} baseline={baseline:.0} size={:.1} \
                         colour=#{:02x}{:02x}{:02x} alpha={}",
                        style.font_size, c.r, c.g, c.b, c.a
                    );
                    n += 1;
                }
            }
        }
        println!("({n} text item(s) containing {want:?} in the display list)");
        return;
    }
    // `--tree ID` — print the LAYOUT BOX SUBTREE under one element (tag.class + rect), which is the
    // only view that shows *which* box is the wrong size. An id-keyed dump tells you the container
    // is 442px wide; this tells you which child made it so.
    if let Some(want) = flag(args, "--tree") {
        let dom = page.dom();
        let styles = page.styles_map();
        #[allow(clippy::too_many_arguments)]
        fn walk(
            b: &manuk_layout::LayoutBox,
            dom: &manuk_dom::Dom,
            styles: &std::collections::HashMap<manuk_dom::NodeId, manuk_css::ComputedStyle>,
            depth: usize,
            hit: bool,
            want_node: manuk_dom::NodeId,
            out: &mut Vec<String>,
        ) {
            let hit = hit || b.node == Some(want_node);
            if hit {
                let desc = b
                    .node
                    .map(|n| {
                        let tag = dom.tag_name(n).unwrap_or("#text");
                        let cls = dom
                            .element(n)
                            .and_then(|e| e.attr("class"))
                            .map(|c| {
                                format!(
                                    ".{}",
                                    c.split_whitespace().take(2).collect::<Vec<_>>().join(".")
                                )
                            })
                            .unwrap_or_default();
                        // The COMPUTED display is what decides whether a box fills or hugs — the
                        // difference between an icon button and a full-width bar. Show it.
                        // Display decides whether a box fills or hugs. COLOUR and VISIBILITY decide
                        // whether the user can see it at all — and "laid out correctly, painted
                        // invisibly" is a failure mode that a geometry-only probe reports as
                        // perfect.
                        let (d, extra) = styles
                            .get(&n)
                            .map(|s| {
                                let c = s.color;
                                let vis = if s.visibility != manuk_css::Visibility::Visible {
                                    " HIDDEN"
                                } else {
                                    ""
                                };
                                let bg = s
                                    .background_color
                                    .filter(|b| b.a > 0)
                                    .map(|b| format!(" bg=#{:02x}{:02x}{:02x}", b.r, b.g, b.b))
                                    .unwrap_or_default();
                                (
                                    format!("{:?}", s.display),
                                    format!(
                                        " #{:02x}{:02x}{:02x}{}{bg}{}",
                                        c.r,
                                        c.g,
                                        c.b,
                                        if c.a < 255 {
                                            format!("a{}", c.a)
                                        } else {
                                            String::new()
                                        },
                                        vis
                                    ),
                                )
                            })
                            .unwrap_or_else(|| ("?".into(), String::new()));
                        format!("{tag}{cls} <{d}>{extra}")
                    })
                    .unwrap_or_else(|| "(anon)".into());
                out.push(format!(
                    "{:indent$}{desc}  [{:.0} {:.0} {:.0}×{:.0}]",
                    "",
                    b.rect.x,
                    b.rect.y,
                    b.rect.width,
                    b.rect.height,
                    indent = depth * 2
                ));
            }
            if let manuk_layout::BoxContent::Block(kids) = &b.content {
                for k in kids {
                    walk(
                        k,
                        dom,
                        styles,
                        if hit { depth + 1 } else { depth },
                        hit,
                        want_node,
                        out,
                    );
                }
            }
            if let manuk_layout::BoxContent::Inline(frags) = &b.content {
                if hit {
                    for f in frags {
                        if !f.text.trim().is_empty() {
                            // The fragment's OWN colour — a text run takes its style from the inline
                            // element it came from, not from the block box above it, so a blue <p>
                            // full of invisible <a> text looks perfect in a block-level probe.
                            out.push(format!(
                                "{:indent$}\"{}\" #{:02x}{:02x}{:02x}{} [{:.0} {:.0} w={:.0}]",
                                "",
                                f.text.trim(),
                                f.style.color.r,
                                f.style.color.g,
                                f.style.color.b,
                                if f.style.color.a < 255 {
                                    format!("a{}", f.style.color.a)
                                } else {
                                    String::new()
                                },
                                f.x,
                                f.line_top,
                                f.width,
                                indent = (depth + 1) * 2
                            ));
                        }
                    }
                }
            }
        }
        let target = dom
            .descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(want));
        match target {
            Some(t) => {
                let mut out = Vec::new();
                walk(&page.root_box, dom, styles, 0, false, t, &mut out);
                for l in out {
                    println!("{l}");
                }
            }
            None => eprintln!("no element with id={want}"),
        }
        return;
    }

    let rects = page.root_box.node_rects(page.dom());
    let dom = page.dom();
    let mut rows: Vec<(String, manuk_layout::Rect)> = Vec::new();
    for n in dom.descendants(dom.root()) {
        if let Some(id) = dom.element(n).and_then(|e| e.attr("id")) {
            if let Some(r) = rects.get(&n) {
                rows.push((id.to_string(), *r));
            }
        }
    }
    rows.sort_by(|a, b| {
        a.1.y
            .partial_cmp(&b.1.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (id, r) in rows {
        println!(
            "{id}\t{}\t{}\t{}\t{}",
            r.x.round(),
            r.y.round(),
            r.width.round(),
            r.height.round()
        );
    }
}

/// `manuk-wpt interact --url U --steps "click:#a;type:#q=hi;scroll:400" [--name N] [--width W]`
///
/// Runs the SAME scripted interaction in Manuk and in Chromium, then compares the two resulting
/// documents. Not the two implementations — the two OUTCOMES.
fn run_interact_cmd(args: &[String], fonts: &manuk_text::FontContext) {
    use manuk_wpt::interact::{changed_boxes, InteractionResult, Step};

    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    let floor: f64 = flag(args, "--floor")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.75);
    let Some(spec) = flag(args, "--scenarios") else {
        eprintln!("usage: manuk-wpt interact --scenarios FILE [--width W] [--floor F]");
        eprintln!("  each line:  <name> <url> <step>;<step>;...");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(spec).unwrap_or_else(|e| {
        eprintln!("cannot read {spec}: {e}");
        std::process::exit(1);
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut rows: Vec<InteractionResult> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(name), Some(url)) = (parts.next(), parts.next()) else {
            continue;
        };
        let steps_src: String = parts.collect::<Vec<_>>().join(" ");
        let steps: Vec<Step> = match steps_src
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(Step::parse)
            .collect::<anyhow::Result<Vec<_>>>()
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{name}: bad steps: {e}");
                continue;
            }
        };
        eprintln!("G5: {name}  ({} step(s))", steps.len());

        // --- Chromium: the same steps, in the same document, before/after. ---
        let js: String = steps
            .iter()
            .map(|s| s.to_js())
            .collect::<Vec<_>>()
            .join("\n");
        let (c_before, c_after) =
            match manuk_wpt::chrome::capture_boxes_interaction(url, vw, vh, &js) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("  chrome: {e}");
                    continue;
                }
            };

        // --- Manuk: the same steps, through the real engine. ---
        let Ok((html, final_url)) = rt.block_on(manuk_page::fetch_html(url)) else {
            eprintln!("  manuk: fetch failed");
            continue;
        };
        let mut page = rt.block_on(async {
            let mut p = manuk_page::Page::load_async(&html, &final_url, fonts, vw as f32).await;
            p.finish_loading(fonts, vw as f32).await;
            p
        });
        let snap = |p: &manuk_page::Page| -> std::collections::HashMap<String, [i32; 4]> {
            let rects = p.root_box.node_rects(p.dom());
            let dom = p.dom();
            dom.descendants(dom.root())
                .filter_map(|n| {
                    let id = dom.element(n).and_then(|e| e.attr("id"))?;
                    let r = rects.get(&n)?;
                    // Zero-size boxes are excluded on the Chromium side too — do not demand Manuk
                    // render something Chromium does not.
                    (r.width > 0.0 || r.height > 0.0).then(|| {
                        (
                            id.to_string(),
                            [
                                r.x.round() as i32,
                                r.y.round() as i32,
                                r.width.round() as i32,
                                r.height.round() as i32,
                            ],
                        )
                    })
                })
                .collect()
        };
        let m_before = snap(&page);
        let mut scroll_y = 0.0f32;
        for step in &steps {
            match step {
                Step::Click(sel) => {
                    let root = page.dom().root();
                    if let Some(&n) = manuk_css::query_selector_all(page.dom(), root, sel).first() {
                        page.dispatch_click(n, fonts, vw as f32);
                    } else {
                        eprintln!("  manuk: no match for {sel:?}");
                    }
                }
                Step::Type(sel, t) => {
                    let root = page.dom().root();
                    if let Some(&n) = manuk_css::query_selector_all(page.dom(), root, sel).first() {
                        page.dispatch_type(n, t, fonts, vw as f32);
                    } else {
                        eprintln!("  manuk: no match for {sel:?}");
                    }
                }
                Step::Scroll(y) => {
                    scroll_y = *y;
                    page.publish_view_state(0.0, scroll_y, None);
                    page.view_changed(scroll_y, vw as f32, vh as f32, true);
                }
            }
        }
        let m_after = snap(&page);

        // --- Compare the OUTCOMES. ---
        let (cov, missing, moved, probed, _) =
            manuk_wpt::fidelity::compare_structure_detail(&to64(&c_after), &to64(&m_after), 8);
        let _ = cov;
        rows.push(InteractionResult {
            name: name.to_string(),
            missing_after: missing,
            moved_after: moved,
            probed_after: probed,
            chrome_changed: changed_boxes(&c_before, &c_after),
            manuk_changed: changed_boxes(&m_before, &m_after),
        });
    }

    let ok = manuk_wpt::interact::report(&rows, floor);
    // `std::process::exit` skips every destructor — including the SpiderMonkey teardown `main`
    // performs. Returning normally is what keeps the exit clean.
    if !ok {
        eprintln!("G5 FAILED — a dead interaction is an ADR-007 CRITICAL.");
    }
}

/// The structural comparator speaks `i64`; the probes speak `i32`.
fn to64(
    m: &std::collections::HashMap<String, [i32; 4]>,
) -> std::collections::HashMap<String, [i64; 4]> {
    m.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                [v[0] as i64, v[1] as i64, v[2] as i64, v[3] as i64],
            )
        })
        .collect()
}

/// `manuk-wpt hittest --html F [--url U]` — reproduce the LINK-CLICK flow without a window.
///
/// A click becomes a navigation only if `a11y_tree().hit_test(x, y)` finds the link under the
/// cursor and the walk up from it reaches an `<a href>`. That is the entire path, and it is testable
/// without a GUI: for every link on the page, hit-test its own centre and ask whether the browser
/// finds it again. A link the browser cannot find is a link the user cannot click.
fn run_hittest_cmd(args: &[String], fonts: &FontContext) {
    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    let Some(f) = flag(args, "--html") else {
        eprintln!("usage: manuk-wpt hittest --html FILE [--url URL]");
        std::process::exit(2);
    };
    let html = std::fs::read_to_string(f).unwrap_or_else(|e| {
        eprintln!("cannot read {f}: {e}");
        std::process::exit(1);
    });
    let url = flag(args, "--url")
        .map(String::from)
        .unwrap_or_else(|| file_url(f));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(&html, &url, fonts, vw as f32).await;
        p.finish_loading(fonts, vw as f32).await;
        p
    });

    let rects = page.root_box.node_rects(page.dom());
    let dom = page.dom();
    let links: Vec<manuk_dom::NodeId> = dom
        .descendants(dom.root())
        .filter(|&n| {
            dom.tag_name(n) == Some("a") && dom.element(n).and_then(|e| e.attr("href")).is_some()
        })
        .collect();

    let (mut hit, mut no_box, mut miss, mut invisible) = (0usize, 0usize, 0usize, 0usize);
    let mut misses: Vec<String> = Vec::new();
    for &a in &links {
        let Some(r) = rects.get(&a) else {
            no_box += 1;
            continue;
        };
        if r.width <= 0.0 || r.height <= 0.0 {
            no_box += 1;
            continue;
        }
        // **A link inside a `visibility:hidden` subtree is not a link the user can click** — in
        // this browser or any other. Chrome lays these out at full size and neither paints nor
        // hit-tests them; measured on the G6 page itself via CDP, `Main page` / `Contents` /
        // `Learn to edit` / `Community portal` / `Recent changes` all compute
        // `visibility:hidden` under `.vector-dropdown-content`, are laid out at 185x28, and
        // `document.elementFromPoint` at their own centre does not return them. 25 links on that
        // page are in that state, because that is how the entire web hides a closed dropdown.
        //
        // Counting them as misses inverts the gate: it scores a browser HIGHER for wrongly
        // hit-testing an invisible overlay. The exclusion is deliberately narrow — only
        // `visibility`, only from the cascade, and the count is REPORTED, never silently dropped,
        // so a sudden jump in it is as visible as a jump in the miss count. A visible link the
        // browser cannot find is still a miss, which is the property the gate exists for.
        let hidden_by_style = std::iter::successors(Some(a), |&n| dom.parent(n)).any(|n| {
            page.styles_of(n)
                .is_some_and(|s| s.visibility != manuk_css::Visibility::Visible)
        });
        if hidden_by_style {
            invisible += 1;
            continue;
        }
        let (cx, cy) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
        // Exactly what the shell does: hit-test, then walk up looking for an <a href>.
        let found = page.a11y_tree().hit_test(cx, cy).map(|n| n.node);
        let mut resolved = None;
        let mut cur = found;
        while let Some(n) = cur {
            if dom.tag_name(n) == Some("a") && dom.element(n).and_then(|e| e.attr("href")).is_some()
            {
                resolved = Some(n);
                break;
            }
            cur = dom.parent(n);
        }
        match resolved {
            Some(_) => hit += 1,
            None => {
                miss += 1;
                if misses.len() < 6 {
                    let text = dom.text_content(a);
                    misses.push(format!(
                        "  MISS  <a> at [{:.0} {:.0} {:.0}x{:.0}] {:?}",
                        r.x,
                        r.y,
                        r.width,
                        r.height,
                        text.trim().chars().take(40).collect::<String>()
                    ));
                }
            }
        }
    }

    let total = links.len();
    println!("\n=== LINK-CLICK FLOW (hit-test every <a href> at its own centre) ===\n");
    println!("  links on page:      {total}");
    println!("  clickable (found):  {hit}");
    println!("  MISSED (unclickable): {miss}");
    println!("  no box at all:      {no_box}");
    println!("  hidden (visibility — correctly unclickable, as in Chrome): {invisible}");
    for m in &misses {
        println!("{m}");
    }
    let denom = (hit + miss).max(1);
    println!("\n  CLICKABILITY: {:.1}%   (a link the browser cannot find is a link the user cannot click)\n",
             hit as f64 / denom as f64 * 100.0);
    if miss > 0 {
        std::process::exit(1);
    }
}

/// `manuk-wpt oracle --corpus FILE [--snapshots DIR] [--tol 8] [--width W]`
///
/// The discovery engine. For each site: fetch ONCE, feed the identical snapshot to both engines,
/// diff every `[id]` element's tag/`display`/box, cluster by root cause, rank by sites explained.
fn run_oracle_cmd(args: &[String], fonts: &FontContext) {
    use manuk_wpt::oracle::{cluster, diff_page, oracle_is_healthy, report, Seen};
    use std::collections::HashMap;

    let vw: u32 = flag(args, "--width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1200);
    let vh: u32 = flag(args, "--height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);
    let tol: i64 = flag(args, "--tol")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let snap_dir = flag(args, "--snapshots").unwrap_or("/tmp/manuk-oracle-snapshots");
    let _ = std::fs::create_dir_all(snap_dir);

    // The crawl frame. A corpus file, or explicit --urls.
    let urls: Vec<String> = if let Some(u) = flag(args, "--urls") {
        u.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if let Some(c) = flag(args, "--corpus") {
        let text = std::fs::read_to_string(c).unwrap_or_else(|e| {
            eprintln!("cannot read {c}: {e}");
            std::process::exit(1);
        });
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
            .collect()
    } else {
        eprintln!("usage: manuk-wpt oracle (--corpus FILE | --urls A,B) [--tol 8]");
        std::process::exit(2);
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
    let mut all_divs = Vec::new();
    let (mut diffed, mut skipped) = (0usize, 0usize);

    for url in &urls {
        let short = url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let short = short.split('/').next().unwrap_or(short).to_string();

        // --- ONE snapshot. Cached on disk, so a re-run diffs the SAME document and a fix is
        // --- attributable to the engine rather than to the site having changed under us.
        let key = format!("{:016x}", fnv(url));
        let snap_path = std::path::Path::new(snap_dir).join(format!("{key}.html"));
        let html = match std::fs::read_to_string(&snap_path) {
            Ok(h) => h,
            Err(_) => match rt.block_on(manuk_page::fetch_html(url)) {
                Ok((h, _)) => {
                    let _ = std::fs::write(&snap_path, &h);
                    h
                }
                Err(e) => {
                    eprintln!("  {short}: fetch failed ({e}) — skipping");
                    skipped += 1;
                    continue;
                }
            },
        };

        // --- Chromium, on that snapshot. Timed: Part 22.2 — a page that "passes" but takes 40x
        // --- Chromium's time is a stability signal that diff-based clustering cannot see on its own.
        let t_chrome = std::time::Instant::now();
        let chrome_raw = match manuk_wpt::chrome::oracle_probe(&html, url, vw, vh) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  {short}: chromium probe failed ({e}) — skipping");
                skipped += 1;
                continue;
            }
        };
        let mut chrome: HashMap<String, Seen> = chrome_raw
            .into_iter()
            .map(|(id, (tag, display, rect))| (id, Seen { tag, display, rect }))
            .collect();

        // --- Never diff against a degraded oracle.
        if let Err(why) = oracle_is_healthy(&chrome) {
            eprintln!("  {short}: DISCARDED — {why}");
            skipped += 1;
            continue;
        }
        chrome.remove("__META__"); // health metadata, not an element
        let chrome_ms = t_chrome.elapsed().as_millis();

        // --- Manuk, on the SAME snapshot, with the same base URL.
        let t_manuk = std::time::Instant::now();
        let page = rt.block_on(async {
            let mut p = manuk_page::Page::load_async(&html, url, fonts, vw as f32).await;
            p.finish_loading(fonts, vw as f32).await;
            p
        });
        let manuk_ms = t_manuk.elapsed().as_millis();
        // **Never diff a page we never styled** — the mirror of `oracle_is_healthy`. Under crawl
        // load our per-resource fetch timeouts starve author sheets, the page renders UA-default,
        // and every submenu Chrome hides becomes a phantom engine divergence (tick 383: apnews
        // booked 291 `none→block` this way; ZERO on a quiet re-run). A discarded site is counted,
        // labelled, and never scored — same honesty rule the Chrome side has always had.
        let starved = page.failed_stylesheet_fetches();
        if starved > 0 {
            eprintln!(
                "  {short}: DISCARDED — {starved} author stylesheet(s) never arrived \
                 (starved engine fetch; the layout is UA fallback, not ours)"
            );
            skipped += 1;
            continue;
        }
        let rects = page.root_box.node_rects(page.dom());
        let styles = page.styles_map();
        let dom = page.dom();
        // The SAME path Chromium's probe computes — selector-path keying (tick 399 spec):
        // `tag.SIG:nth-child(N)`, N 1-based over ALL element siblings, SIG = fnv1a-32 over the
        // ASCII-lowercased SORTED deduped class list joined with '.'; classless elements emit
        // `tag:nth-child(N)`. The class signature is the identity change that matters: a
        // positional counterpart with a different class list now FAILS the key lookup and books
        // as missing+extra (tree drift, which it is) instead of minting a phantom style diff
        // between two unrelated elements (tick 395: okta's 316 "display diffs" were exactly this).
        // This mirrors the JS `sigOf`/`pathOf` in chrome.rs BYTE-IDENTICALLY — the hash runs over
        // UTF-16 code units (encode_utf16) because that is what charCodeAt yields. And it keeps
        // the part that is easy to get wrong: an element whose parent is not an element (i.e.
        // `<html>`) contributes NO component, because `e.parentElement` is null there and the JS
        // loop never runs for it. Emitting a root component on our side once shifted every key by
        // one level and reported `<html>` MISSING on every site, with total confidence. Two
        // engines agreeing on a naming scheme is a precondition for the diff meaning anything.
        let sig_of = |n: manuk_dom::NodeId| -> String {
            let Some(cls) = dom.element(n).and_then(|e| e.attr("class")) else {
                return String::new();
            };
            let mut toks: Vec<String> = cls
                .split_ascii_whitespace()
                .map(|t| t.to_ascii_lowercase())
                .collect();
            if toks.is_empty() {
                return String::new();
            }
            toks.sort();
            toks.dedup();
            let joined = toks.join(".");
            let mut h: u32 = 0x811c9dc5;
            for u in joined.encode_utf16() {
                h ^= u32::from(u);
                h = h.wrapping_mul(0x0100_0193);
            }
            format!(".{h:08x}")
        };
        let path_of = |n: manuk_dom::NodeId| -> Option<String> {
            let mut parts: Vec<String> = Vec::new();
            let mut cur = n;
            loop {
                let Some(parent) = dom.parent(cur) else { break };
                if !dom.is_element(parent) {
                    break;
                }
                let tag = dom.tag_name(cur)?;
                let mut i = 1usize;
                for sib in dom.children(parent) {
                    if sib == cur {
                        break;
                    }
                    if dom.is_element(sib) {
                        i += 1;
                    }
                }
                parts.push(format!("{tag}{}:nth-child({i})", sig_of(cur)));
                cur = parent;
            }
            parts.reverse();
            Some(parts.join("/"))
        };
        let manuk: HashMap<String, Seen> = dom
            .descendants(dom.root())
            .filter(|&n| dom.is_element(n))
            .filter_map(|n| {
                let tag = dom.tag_name(n)?.to_string();
                if matches!(
                    tag.as_str(),
                    "script"
                        | "style"
                        | "head"
                        | "meta"
                        | "link"
                        | "base"
                        | "title"
                        | "noscript"
                        | "template"
                        | "html"
                ) {
                    return None;
                }
                let id = path_of(n)?;
                let display = styles
                    .get(&n)
                    .map(|s| css_display_name(s.display))
                    .unwrap_or("none")
                    .to_string();
                let r = rects.get(&n);
                // A `display:none` element legitimately has no box; report it as such rather than
                // as missing, so the oracle can tell "we hid it" from "we lost it".
                let rect = match r {
                    Some(r) => [r.x as i64, r.y as i64, r.width as i64, r.height as i64],
                    None if display == "none" => [0, 0, 0, 0],
                    None => return None,
                };
                Some((id, Seen { tag, display, rect }))
            })
            .collect();

        let divs = diff_page(&short, &chrome, &manuk, tol);
        // Layer-2 jarring invariant (FIDELITY-SCORING-REDESIGN.md): elements we alone push past the
        // viewport. SHAPE scoring forgives a constant offset; this catches the different, highly-
        // perceived failure of content spilling off-screen. Reported alongside the divergence count.
        let (hover, hover_ex) =
            manuk_wpt::oracle::jarring_h_overflow(&chrome, &manuk, vw as i64, tol);
        // And sibling overlap — the #1 "broken page" perception (text on text, control under banner),
        // which SHAPE also cannot see. `ov_skip` reports sibling groups too large to scan pairwise.
        let (overlap, ov_skip, ov_ex) = manuk_wpt::oracle::jarring_overlap(&chrome, &manuk, tol);
        // And reading-order inversion — a float/abspos/grid item we alone pull out of sequence, which
        // SHAPE and overlap both miss (the boxes can be well-shaped and disjoint yet read swapped).
        let (rinv, rinv_skip, rinv_ex) =
            manuk_wpt::oracle::jarring_reading_order(&chrome, &manuk, tol);
        // And collapsed interactive targets — a control we alone strip of its clickable area (a dead
        // button), the geometry half of hittability. `min_hit` is the smallest axis a real target has.
        let (dead, dead_ex) = manuk_wpt::oracle::jarring_collapsed_target(&chrome, &manuk, 2);
        let mut jflag = String::new();
        if hover > 0 {
            jflag.push_str(&format!(
                "  ⚠ {hover} h-overflow (e.g. {})",
                hover_ex.first().map(String::as_str).unwrap_or("")
            ));
        }
        if overlap > 0 {
            jflag.push_str(&format!(
                "  ⚠ {overlap} overlap (e.g. {})",
                ov_ex.first().map(String::as_str).unwrap_or("")
            ));
        }
        if rinv > 0 {
            jflag.push_str(&format!(
                "  ⚠ {rinv} reorder (e.g. {})",
                rinv_ex.first().map(String::as_str).unwrap_or("")
            ));
        }
        if dead > 0 {
            jflag.push_str(&format!(
                "  ⚠ {dead} dead-target (e.g. {})",
                dead_ex.first().map(String::as_str).unwrap_or("")
            ));
        }
        if ov_skip + rinv_skip > 0 {
            jflag.push_str(&format!(
                "  [{} group(s) too large to scan]",
                ov_skip.max(rinv_skip)
            ));
        }
        eprintln!(
            "  {short}: {} divergence(s) over {} probed{jflag}",
            divs.len(),
            chrome.len(),
        );

        // **Emit, don't accumulate.** At 265 sites, holding the whole crawl in one process means one
        // site's hang or crash loses all of it. Each site writes its own result file and the crawl is
        // resumable; the driver runs them under a watchdog in separate processes (G_HANG).
        if let Some(dir) = flag(args, "--emit") {
            let _ = std::fs::create_dir_all(dir);
            let mut out = String::new();
            out.push_str(&format!(
                "{{\"kind\":\"meta\",\"site\":\"{short}\",\"class\":\"{}\",\"status\":\"ok\",\"probed\":{},\"manuk_ms\":{},\"chrome_ms\":{},\"h_overflow\":{},\"overlap\":{},\"reorder\":{},\"dead_target\":{}}}\n",
                flag(args, "--class").unwrap_or("?"),
                chrome.len(),
                manuk_ms,
                chrome_ms,
                hover,
                overlap,
                rinv,
                dead
            ));
            for d in &divs {
                out.push_str(&format!(
                    "{{\"kind\":\"div\",\"site\":\"{}\",\"class\":\"{}\",\"tag\":\"{}\",\"dkind\":\"{}\",\"chrome\":{},\"manuk\":{},\"id\":{}}}\n",
                    d.site,
                    flag(args, "--class").unwrap_or("?"),
                    d.tag,
                    d.kind,
                    json_str(&d.chrome),
                    json_str(&d.manuk),
                    json_str(&d.id),
                ));
            }
            let _ = std::fs::write(
                std::path::Path::new(dir).join(format!("{short}.jsonl")),
                out,
            );
        }
        all_divs.extend(divs);
        diffed += 1;
    }

    let clusters = cluster(&all_divs);
    report(&clusters, diffed, skipped);
}

/// Minimal JSON string escaping — the crawl's own output must never be the thing that breaks it.
fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Our `Display` as CSS names it — the vocabulary the oracle diffs in.
fn css_display_name(d: manuk_css::Display) -> &'static str {
    use manuk_css::Display as D;
    match d {
        D::Block => "block",
        D::Inline => "inline",
        D::InlineBlock => "inline-block",
        D::Flex => "flex",
        D::Grid => "grid",
        D::InlineFlex => "inline-flex",
        D::InlineGrid => "inline-grid",
        D::Table => "table",
        D::TableRow => "table-row",
        D::TableRowGroup => "table-row-group",
        D::TableCell => "table-cell",
        D::TableCaption => "table-caption",
        D::TableColumn => "table-column",
        D::TableColumnGroup => "table-column-group",
        D::Contents => "contents",
        D::None => "none",
    }
}

/// FNV-1a — a stable snapshot key with no clock and no RNG, so a re-run finds the same file.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// **The merge — where the crawl becomes the ledger.**
///
/// The ranking key is DISTINCT SITES and DISTINCT CLASSES, never hit count. A cause that breaks forty
/// sites outranks one that breaks forty elements of a single site, and a cause that appears across
/// `docs` and `saas` and `news` is a *design-pattern* bug while one confined to a single class is
/// probably that class's house style. Ranking by hits would put whichever site has the most `<span>`s
/// at the top of the plan forever.
///
/// Health of the crawl is reported first and honestly: hangs, failures and discards are COUNTED, not
/// quietly excluded. A crawl that silently drops a third of its sites and reports the rest as "the
/// corpus" is worse than no crawl, because it is confidently wrong.
fn run_oracle_merge(args: &[String]) {
    use std::collections::{BTreeMap, BTreeSet};
    let dir = args
        .first()
        .map(String::as_str)
        .unwrap_or("/tmp/manuk-oracle-run");
    let field = |line: &str, k: &str| -> String {
        let pat = format!("\"{k}\":\"");
        line.find(&pat)
            .map(|i| {
                let rest = &line[i + pat.len()..];
                rest.find('"')
                    .map(|e| rest[..e].to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    };
    let num = |line: &str, k: &str| -> i64 {
        let pat = format!("\"{k}\":");
        line.find(&pat)
            .and_then(|i| {
                let rest = &line[i + pat.len()..];
                let end = rest
                    .find(|c: char| !c.is_ascii_digit() && c != '-')
                    .unwrap_or(rest.len());
                rest[..end].parse().ok()
            })
            .unwrap_or(0)
    };

    // signature -> (sites, classes, hits, example)
    let mut acc: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>, usize, String)> =
        BTreeMap::new();
    let (mut ok, mut hang, mut fail, mut discard) = (0usize, 0usize, 0usize, 0usize);
    let mut slow: Vec<(i64, i64, String)> = Vec::new();
    // Per-site jarring-invariant counts [overlap, h_overflow, reorder, dead_target], rolled up into
    // the Phase-0 exit-bar tally (FIDELITY-SCORING-REDESIGN.md §2 Layer 2) after the crawl.
    let mut jarring_rows: Vec<[i64; 4]> = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("no crawl results in {dir}");
        std::process::exit(1);
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            if line.contains("\"kind\":\"meta\"") {
                match field(line, "status").as_str() {
                    "ok" => {
                        ok += 1;
                        let (m, c) = (num(line, "manuk_ms"), num(line, "chrome_ms"));
                        // Part 22.2: a page that "passes" but takes many times Chromium's time is a
                        // stability signal the diff clustering cannot see on its own.
                        if c > 0 && m > c * 3 && m > 3000 {
                            slow.push((m, c, field(line, "site")));
                        }
                        // The Layer-2 jarring invariants — the actual Phase-0 exit bar. Emitted per
                        // site; rolled up across the corpus below (older result files without these
                        // fields read 0, which is correct — they predate the invariant).
                        jarring_rows.push([
                            num(line, "overlap"),
                            num(line, "h_overflow"),
                            num(line, "reorder"),
                            num(line, "dead_target"),
                        ]);
                    }
                    "HANG" => hang += 1,
                    "DISCARDED" => discard += 1,
                    _ => fail += 1,
                }
                continue;
            }
            if !line.contains("\"kind\":\"div\"") {
                continue;
            }
            let (site, class, tag, kind) = (
                field(line, "site"),
                field(line, "class"),
                field(line, "tag"),
                field(line, "dkind"),
            );
            let (chrome, manuk) = (field(line, "chrome"), field(line, "manuk"));
            let sig = match kind.as_str() {
                "display" => format!("display: {chrome} → {manuk}   (<{tag}>)"),
                "missing" => {
                    format!("MISSING BOX: <{tag}>  (Chrome renders it, we render nothing)")
                }
                _ => format!("geometry: <{tag}>"),
            };
            let en = acc.entry(sig).or_insert_with(|| {
                (
                    BTreeSet::new(),
                    BTreeSet::new(),
                    0,
                    format!("{site}: {chrome} vs {manuk}"),
                )
            });
            en.0.insert(site);
            en.1.insert(class);
            en.2 += 1;
        }
    }

    let total = ok + hang + fail + discard;
    println!("\n════════ ORACLE CRAWL — {total} sites ════════\n");
    println!("  {ok:>4} diffed");
    println!(
        "  {discard:>4} discarded (Chromium's OWN render was degraded — bot wall / error page /"
    );
    println!(
        "       no-script fallback. Never scored as our bug: a degraded oracle scores its own"
    );
    println!("       emptiness as our failure, and that has happened here before.)");
    if hang > 0 {
        println!("  \x1b[31m{hang:>4} HUNG\x1b[0m  ← a hard failure, counted and attributed (G_HANG). Not a skipped test.");
    }
    if fail > 0 {
        println!("  \x1b[33m{fail:>4} failed\x1b[0m");
    }

    if !slow.is_empty() {
        slow.sort_by_key(|(m, _, _)| -m);
        println!("\n──── SLOW (Part 22.2: passing, but many times Chromium's time — a stability signal) ────\n");
        for (m, c, site) in slow.iter().take(10) {
            println!(
                "  {site:<32} manuk {m:>6}ms   chromium {c:>6}ms   ({}×)",
                m / c.max(&1)
            );
        }
    }

    // **The Phase-0 exit bar.** The jarring invariants certify Phase 0 (FIDELITY-SCORING-REDESIGN.md
    // §2 Layer 2): a page can be 30px offset everywhere and still pass all of them. Reported as
    // sites-affected out of the diffed corpus — the honest "what fraction of the web looks broken"
    // number — with the raw instance count alongside. Computed per site, discarded until now.
    if ok > 0 {
        let agg = manuk_wpt::oracle::tally_jarring(&jarring_rows);
        println!(
            "\n──── JARRING INVARIANTS (Phase-0 exit bar — sites affected of {ok} diffed) ────\n"
        );
        let notes = [
            "text on text / control under a banner",
            "content spilled off-screen",
            "content out of reading sequence",
            "a control collapsed to no clickable area",
        ];
        for (i, (sites, total)) in agg.iter().enumerate() {
            let label = manuk_wpt::oracle::JARRING_LABELS[i];
            let flag = if *sites == 0 {
                "\x1b[32m✓\x1b[0m"
            } else {
                "\x1b[33m⚠\x1b[0m"
            };
            let pct = 100.0 * *sites as f64 / ok as f64;
            println!(
                "  {flag} {label:<12} {sites:>4} sites ({pct:>4.1}%)   {total:>5} total   — {}",
                notes[i]
            );
        }
        println!(
            "\n  A jarring-free page need not be pixel-identical; these are the failures a user actually\n  \
             perceives as broken. Phase-0 exit wants these near zero across the corpus."
        );
    }

    let mut ranked: Vec<_> = acc.into_iter().collect();
    // DISTINCT SITES, then DISTINCT CLASSES. Never hits.
    ranked.sort_by(|a, b| {
        (b.1 .0.len(), b.1 .1.len(), b.1 .2).cmp(&(a.1 .0.len(), a.1 .1.len(), a.1 .2))
    });

    // **The cluster REGISTRY.** Part 24.1: a cluster with N site-hits and one root cause *is* a
    // website class, discovered rather than hand-enumerated — so this file is the website-class
    // taxonomy, and there is no second artifact to build under another name.
    //
    // Part 28.2: every cluster gets a stable ID, and the pre-commit hook cross-checks the ID a tick
    // CLAIMS to be closing against this file. That converts "TICK SHAPE: pattern-class" from a
    // self-report — which a tick spent micro-tuning one site could write with a straight face and
    // nothing would catch — into a verified fact. A journal entry that cannot name a real, current
    // cluster ID is a single-site tick BY DEFINITION.
    let mut registry = String::from(
        "# ORACLE CLUSTER REGISTRY — generated by `manuk-wpt oracle-merge`. Do not hand-edit.\n         #\n         # This IS the priority ledger (Part 4) and the website-class taxonomy (Part 24.1). Ranked by\n         # DISTINCT SITES then DISTINCT CLASSES, never by hit count. A pattern class that CRASHES or\n         # HANGS outranks every visual divergence here (Part 24.3) — those live in STATUS.md's Bar 0.\n         #\n         # id            sites  classes  hits  root cause\n",
    );
    println!("\n──── ROOT CAUSES, ranked by SITES EXPLAINED — this IS the ledger ────\n");
    println!(
        "{:>6} {:>8} {:>7}  {:<10} {}",
        "sites", "classes", "hits", "cluster", "root cause"
    );
    for (i, (sig, (sites, classes, hits, example))) in ranked.iter().enumerate() {
        // Stable, human-quotable, and derived from the signature so the same root cause keeps the
        // same id across crawls even as its rank moves.
        let id = format!("C{:04x}", fnv(sig) & 0xffff);
        registry.push_str(&format!(
            "{id:<14} {:>5} {:>8} {:>5}  {sig}\n",
            sites.len(),
            classes.len(),
            hits
        ));
        if i < 30 {
            println!(
                "{:>6} {:>8} {:>7}  {id:<10} {sig}",
                sites.len(),
                classes.len(),
                hits
            );
            println!("{:>26}e.g. {example}", "");
        }
    }
    let _ = std::fs::write("docs/loop/CLUSTERS.md", &registry);
    println!(
        "\n  cluster registry → docs/loop/CLUSTERS.md ({} clusters)",
        ranked.len()
    );
    println!(
        "\nRanked by DISTINCT SITES and DISTINCT CLASSES, never by hit count — otherwise whichever\n\
         site has the most <span>s tops the plan forever. A cause spanning several classes is a\n\
         DESIGN-PATTERN bug; one confined to a single class is probably that class's house style.\n\
         This ordering is the priority ledger (Part 4). No judgement is applied to it.\n"
    );
}

/// `manuk-wpt wpt <subset> [--wpt DIR] [--timeout S] [--limit N] [--start N] [--json OUT]`
///
/// Runs the **upstream** WPT `testharness.js` suite. See `harness.rs` for why this is the highest-
/// leverage instrument available: it is the only one that does not need Chromium to tell it what
/// "right" is, and the only one that can see an API no site in the crawl corpus happens to call.
///
/// ## Why this forks a child process per batch
///
/// **`tokio::time::timeout` cannot interrupt synchronous JavaScript.** A test that spins inside
/// SpiderMonkey never yields, so the timeout future never gets to run, and the whole run wedges —
/// which is exactly what happened on the very first pass (`ChildNode-after` hit an infinite loop in
/// our own `insert_before`).
///
/// A hang is a **result**, not an accident: it is Bar 0 signal, and it is the single most valuable
/// thing this suite can tell us. So it has to be *survivable* and *attributable*. The only thing
/// that can reliably contain a spinning C++ JIT frame is an **OS process boundary** — the same
/// conclusion the process model reached for tabs (`docs/loop/PROCESS-MODEL.md`), arrived at here
/// independently and for the same reason.
///
/// So: the driver forks a child per batch; the child appends one JSON line per finished test and
/// flushes. If the child stops making progress, the driver kills it, and **the test after the last
/// flushed line is the one that hung** — named, recorded, and stepped over so the run continues.
fn run_wpt_cmd(args: &[String], fonts: &FontContext) {
    let Some(dir) = flag(args, "--wpt")
        .map(PathBuf::from)
        .or_else(manuk_wpt::find_wpt_checkout)
    else {
        eprintln!("no WPT checkout.  export WPT_DIR=/path/to/wpt   (or ./scripts/wpt-setup.sh)");
        std::process::exit(2);
    };
    let known: Vec<Option<&str>> = [
        "--wpt",
        "--timeout",
        "--limit",
        "--start",
        "--json",
        "--batch",
    ]
    .iter()
    .map(|f| flag(args, f))
    .collect();
    let subset = args
        .iter()
        .find(|a| !a.starts_with("--") && !known.iter().any(|k| *k == Some(a.as_str())))
        .cloned()
        .unwrap_or_else(|| "dom".into());
    let secs: u64 = flag(args, "--timeout")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let timeout = std::time::Duration::from_secs(secs);
    let limit: usize = flag(args, "--limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);
    let start: usize = flag(args, "--start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let batch: usize = flag(args, "--batch")
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);

    let disc = manuk_wpt::harness::discover(&dir, &subset);
    let all: Vec<String> = disc.tests.iter().skip(start).take(limit).cloned().collect();

    // ── CHILD: run a slice in-process, append one flushed JSON line per finished test.
    let show_fail = args.iter().any(|a| a == "--show-failures");
    if args.iter().any(|a| a == "--child") {
        let out = PathBuf::from(flag(args, "--out").expect("--child needs --out"));
        let rt = tokio::runtime::Runtime::new().expect("tokio");
        rt.block_on(async move {
            let (addr, _srv) = manuk_wpt::harness::serve(dir.clone())
                .await
                .expect("server");
            let base = format!("http://{addr}");
            for rel in &all {
                let r = manuk_wpt::harness::run_one(&base, rel, fonts, timeout).await;
                let (p, t) = r.counts();
                // **The failure MESSAGE is the whole product.** "0/3" tells you nothing you can act
                // on; `assert_equals: expected "flex" but got "block"` is a work item. Without this
                // the suite is a scoreboard, and a scoreboard does not fix anything.
                if show_fail {
                    if let Some(ts) = &r.subtests {
                        for (name, st) in ts {
                            match st {
                                manuk_wpt::harness::Sub::Pass => {}
                                manuk_wpt::harness::Sub::Fail(m) => {
                                    eprintln!("    FAIL {name}\n         {m}")
                                }
                                other => eprintln!("    {other:?} {name}"),
                            }
                        }
                    }
                }
                // Append + flush per test. If the NEXT test hangs and we are killed, this file is
                // the record of exactly how far we got — so the hang is attributable to one file.
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&out)
                    .unwrap();
                let _ = writeln!(
                    f,
                    "{{\"path\":{:?},\"pass\":{p},\"total\":{t},\"harness\":{:?},\"ms\":{}}}",
                    rel, r.harness_status, r.ms
                );
                let _ = f.flush();
            }
        });
        return;
    }

    // ── DRIVER
    eprintln!("WPT {subset}: {} runnable testharness files", all.len());
    for (why, n) in &disc.skipped {
        eprintln!("   skip {n:>5}  {why}");
    }

    let exe = std::env::current_exe().expect("exe");
    let tmp = std::env::temp_dir().join(format!("manuk-wpt-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&tmp);

    let mut results: Vec<(String, usize, usize, String, u128)> = Vec::new();
    // **Count the FILE's lines separately from `results`.**
    //
    // `results` also holds SYNTHETIC rows (HANG/CRASH) that have no corresponding line in the child's
    // JSONL. Using `results.len()` as the read offset therefore over-skips the moment one is pushed —
    // every later batch then reads short, is diagnosed as "the child died", and pushes ANOTHER synthetic
    // row, which over-skips further. **One real event manufactures a cascade of fake ones.**
    //
    // This produced "5 CRASHES" in the first baseline. Running any of those files — or a 120-file batch —
    // in one child exits 0 and completes every one. **The crashes were an accounting artifact of the
    // instrument, not a fault in the engine.**
    let mut lines_read = 0usize;
    let mut i = 0usize;
    // Files that SIGSEGV'd the shared batch runtime but PASS in a fresh single-file runtime — a
    // cross-file heap-accumulation artifact of reusing one SpiderMonkey runtime per batch, NOT a
    // per-page Bar 0. Tracked and printed so the artifact is never invisible.
    let mut accum_recovered = 0usize;
    while i < all.len() {
        let take = batch.min(all.len() - i);
        let mut child = std::process::Command::new(&exe)
            .arg("wpt")
            .arg(&subset)
            .arg("--wpt")
            .arg(&dir)
            .arg("--child")
            .arg("--out")
            .arg(&tmp)
            .arg("--start")
            .arg((start + i).to_string())
            .arg("--limit")
            .arg(take.to_string())
            .arg("--timeout")
            .arg(secs.to_string())
            .args(if show_fail {
                vec!["--show-failures"]
            } else {
                vec![]
            })
            .stdout(std::process::Stdio::null())
            .stderr(if show_fail {
                std::process::Stdio::inherit()
            } else {
                std::process::Stdio::null()
            })
            .spawn()
            .expect("spawn child");

        // Watchdog: kill the child when it stops making PROGRESS, not on a fixed total budget — a
        // batch of slow-but-fine tests must not be mistaken for a hang.
        let mut lines_seen = read_jsonl(&tmp).len();
        let mut last_progress = std::time::Instant::now();
        let stall = timeout + std::time::Duration::from_secs(15);
        // **Capture HOW the child ended.** "The child produced fewer rows than asked" is not a diagnosis —
        // it could be a segfault (Bar 0) or the instrument miscounting (nothing). Only the exit status can
        // say, and reporting CRASH without checking it is exactly the "one word for several findings" error.
        let mut exit_code: Option<i32> = None;
        let hung = loop {
            match child.try_wait() {
                Ok(Some(st)) => {
                    exit_code = st.code();
                    break false;
                }
                Ok(None) => {}
                Err(_) => break false,
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            let n = read_jsonl(&tmp).len();
            if n > lines_seen {
                lines_seen = n;
                last_progress = std::time::Instant::now();
            }
            if last_progress.elapsed() > stall {
                let _ = child.kill();
                let _ = child.wait();
                break true;
            }
        };

        let done = read_jsonl(&tmp);
        for r in done.iter().skip(lines_read) {
            results.push(r.clone());
        }
        let completed = done.len() - lines_read; // FILE lines this batch produced — not results.len()
        lines_read = done.len();

        if hung {
            // The test after the last flushed line is the culprit. Name it, record it, step over it.
            let idx = i + completed;
            if idx < all.len() {
                eprintln!("  \x1b[31mHANG\x1b[0m  {}", all[idx]);
                results.push((all[idx].clone(), 0, 0, "HANG".into(), 0));
            }
            i = idx + 1;
        } else if completed < take {
            // **The child DIED — it did not hang.** It exited on its own without finishing the batch:
            // a SpiderMonkey crash, an abort, an OOM. That is Bar 0, and it is a *finding*.
            //
            // Advancing by `take` here (as this did on its first run) silently DROPS every remaining
            // test in the batch — 33 files vanished from a 457-file suite and the pass rate was
            // computed over what was left, with nothing to say so. **A runner that quietly skips what
            // it cannot run reports a pass rate for a suite it did not run.**
            let idx = i + completed;
            // `None` = killed by a SIGNAL (segfault/abort) — a real Bar 0 crash.
            // `Some(0)` = the child exited CLEANLY and simply wrote fewer rows than we asked for, which is
            //             an INSTRUMENT fault, not an engine fault. Say which.
            let (label, note) = match exit_code {
                None => ("CRASH", "killed by a signal — Bar 0"),
                Some(0) => (
                    "SHORT",
                    "child exited 0 but wrote fewer rows than asked — INSTRUMENT fault",
                ),
                Some(c) => ("EXIT", if c == 101 { "panic" } else { "nonzero exit" }),
            };
            // ── ISOLATION-RETRY on a SIGNAL death. A page that SIGSEGVs a runtime only AFTER other
            //    pages accumulated state in it, yet runs CLEAN in a FRESH runtime, is a cross-file
            //    heap-accumulation artifact of the harness reusing ONE SpiderMonkey runtime per batch
            //    (a speed hack — real browsing never crams dozens of documents into one runtime). So
            //    re-run the culprit ALONE: if it passes, its per-page result is the truth and this is
            //    recorded as ACCUM (visible, tracked), NOT counted as a per-page Bar 0. A file that
            //    crashes ALONE too stays CRASH — a real per-page Bar 0 is NEVER reclassified away.
            let mut recovered = false;
            if exit_code.is_none() {
                let tmp2 = std::env::temp_dir()
                    .join(format!("manuk-wpt-iso-{}-{idx}.jsonl", std::process::id()));
                let _ = std::fs::remove_file(&tmp2);
                if let Ok(mut iso) = std::process::Command::new(&exe)
                    .arg("wpt")
                    .arg(&subset)
                    .arg("--wpt")
                    .arg(&dir)
                    .arg("--child")
                    .arg("--out")
                    .arg(&tmp2)
                    .arg("--start")
                    .arg((start + idx).to_string())
                    .arg("--limit")
                    .arg("1")
                    .arg("--timeout")
                    .arg(secs.to_string())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    let iso_deadline = std::time::Instant::now() + stall;
                    let iso_signal = loop {
                        match iso.try_wait() {
                            Ok(Some(st)) => break st.code().is_none(), // None = signal-killed ALONE too
                            Ok(None) => {}
                            Err(_) => break true,
                        }
                        if std::time::Instant::now() > iso_deadline {
                            let _ = iso.kill();
                            let _ = iso.wait();
                            break true; // hung alone → not a clean isolation pass
                        }
                        std::thread::sleep(std::time::Duration::from_millis(150));
                    };
                    let rows = read_jsonl(&tmp2);
                    let _ = std::fs::remove_file(&tmp2);
                    if !iso_signal && rows.len() == 1 {
                        eprintln!(
                            "  \x1b[33mACCUM\x1b[0m {} (SIGSEGV in-batch, PASSES alone → cross-file runtime-reuse artifact, not a per-page Bar 0)",
                            all[idx]
                        );
                        results.push(rows[0].clone());
                        accum_recovered += 1;
                        recovered = true;
                    }
                }
            }
            if !recovered {
                eprintln!("  \x1b[31m{label}\x1b[0m {} ({note})", all[idx]);
                results.push((all[idx].clone(), 0, 0, label.into(), 0));
            }
            i = idx + 1;
        } else {
            i += take;
        }
    }
    let _ = std::fs::remove_file(&tmp);

    // ── Report
    let mut by_dir: std::collections::BTreeMap<String, (usize, usize, usize)> = Default::default();
    let (mut pass, mut total, mut no_report, mut hangs) = (0usize, 0usize, 0usize, 0usize);
    let (mut slow, mut th_timeout, mut short) = (0usize, 0usize, 0usize);
    let mut jsonl = String::new();
    for (rel, p, t, h, ms) in &results {
        pass += p;
        total += t;
        match h.as_str() {
            "HANG" | "CRASH" | "EXIT" => hangs += 1, // Bar 0: the child stopped, or died
            "SHORT" => short += 1,                   // the INSTRUMENT lost a row — not the engine
            "SLOW" => slow += 1,                     // our budget expired — a PERF finding
            "TIMEOUT" => th_timeout += 1, // testharness's own verdict: an async test never completed
            "NO_REPORT" | "HARNESS_NOT_LOADED" | "BAD_REPORT" | "FETCH_FAILED" => no_report += 1,
            _ => {}
        }
        let d = rel
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default();
        let e = by_dir.entry(d).or_default();
        e.0 += p;
        e.1 += t;
        e.2 += 1;
        jsonl.push_str(&format!(
            "{{\"path\":{rel:?},\"pass\":{p},\"total\":{t},\"harness\":{h:?},\"ms\":{ms}}}\n"
        ));
        if h != "OK" && h != "ERROR" {
            continue;
        }
    }
    if let Some(path) = flag(args, "--json") {
        let _ = std::fs::write(&path, &jsonl);
        eprintln!("wrote {path}");
    }

    println!("\n── WPT {subset} ──────────────────────────────────────────");
    for (d, (p, t, n)) in &by_dir {
        if *t == 0 {
            continue;
        }
        println!(
            "  {:>5.1}%  {p:>5}/{t:<5}  ({n} files)  {d}",
            100.0 * *p as f64 / *t as f64
        );
    }
    let rate = if total == 0 {
        0.0
    } else {
        100.0 * pass as f64 / total as f64
    };
    println!(
        "\n  FILES  {}   subtests {pass}/{total}  =  {rate:.1}%",
        results.len()
    );
    // Three DIFFERENT findings. Never one word.
    println!("  NO_REPORT {no_report}   \x1b[1mHANG/CRASH {hangs}\x1b[0m (Bar 0)   SHORT {short} (instrument)   SLOW {slow} (our budget)   TH_TIMEOUT {th_timeout} (async test never completed)");
    if accum_recovered > 0 {
        println!("  ACCUM {accum_recovered} (SIGSEGV'd the shared batch runtime but PASS in a fresh one — a cross-file runtime-reuse artifact, recovered in isolation; the underlying UAF is a tracked Bar-0 to FIX, see docs/wiki/js-engine.md)");
    }
    if no_report * 4 > results.len() && results.len() > 20 {
        println!("\n  ⚠ {no_report}/{} files reported NOTHING. Above ~25% this is not measuring the engine's\n    \
                  conformance — it is measuring whether testharness.js can RUN here at all.", results.len());
    }
}

/// Read the child's incremental JSONL. Tolerates a torn final line (we may have killed it mid-write).
fn read_jsonl(p: &std::path::Path) -> Vec<(String, usize, usize, String, u128)> {
    let Ok(s) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in s.lines() {
        let g = |k: &str| -> Option<String> {
            let at = line.find(&format!("\"{k}\":"))? + k.len() + 3;
            let rest = &line[at..];
            if rest.starts_with('"') {
                let end = rest[1..].find('"')? + 1;
                Some(rest[1..end].to_string())
            } else {
                Some(rest.chars().take_while(|c| c.is_ascii_digit()).collect())
            }
        };
        let (Some(path), Some(p1), Some(t1), Some(h)) =
            (g("path"), g("pass"), g("total"), g("harness"))
        else {
            continue;
        };
        out.push((
            path,
            p1.parse().unwrap_or(0),
            t1.parse().unwrap_or(0),
            h,
            g("ms").and_then(|m| m.parse().ok()).unwrap_or(0),
        ));
    }
    out
}

/// Run one WPT file and report **what the page itself experienced** — uncaught errors first.
///
/// A file that creates 8,000 subtests and finishes none of them did not "time out": it threw, halfway,
/// and every test already created was left un-`done()`. `TH_TIMEOUT` is what that *looks* like from
/// outside, and it is why the status alone is useless. An instrument must be able to distinguish its own
/// condition from the thing it measures — and "the page threw" is the condition, not the measurement.
#[cfg(feature = "spidermonkey")]
fn diag(rel: String, fonts: &manuk_text::FontContext) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let root = std::path::PathBuf::from(
            std::env::var("WPT_DIR").unwrap_or_else(|_| format!("{}/wpt", std::env::var("HOME").unwrap())),
        );
        let (addr, _h) = manuk_wpt::harness::serve(root).await.expect("serve");
        let url = format!("http://{addr}/{rel}");
        println!("── {url}");

        let Some((html, final_url)) = manuk_page::fetch_html(&url).await.ok() else {
            println!("FETCH FAILED");
            return;
        };
        let mut page = manuk_page::Page::load_async(&html, &final_url, fonts, 800.0).await;
        println!("  after load_async : pending iframes = {}", page.pending_iframes().len());
        page.finish_loading(fonts, 800.0).await;
        println!("  after finish_load: pending iframes = {}", page.pending_iframes().len());

        // Ask the page what it experienced. Uncaught errors first — "the async test never completed"
        // is a status, not a finding, and only the exception says why.
        page.eval_for_test(
            r#"(function(){
              function rep(o){ try {
                var s = document.createElement('script'); s.id='__diag__'; s.type='application/json';
                s.textContent = JSON.stringify(o); document.documentElement.appendChild(s);
              } catch(e){} }
              var f = document.querySelector('iframe');
              var d = null, spans = -1, err = null;
              try {
                if (f) { d = f.contentWindow ? f.contentWindow.document : f.contentDocument; }
                if (d) { spans = d.querySelectorAll('*').length; }
              } catch (e) { err = String(e); }
              rep({
                errors: (globalThis.__errors || []).map(String).slice(0, 6),
                loadFired: !!globalThis.__loadFired,
                hasIframe: !!f,
                frameDoc: d ? 'OK' : String(d),
                frameNodes: spans,
                frameErr: err,
                testsCreated: (globalThis.tests && globalThis.tests.length) || 0,
                onloadCalls: globalThis.__onCalls || 0,
                harness: (typeof add_completion_callback === 'function')
              });
            })()"#,
        );
        let dom = page.dom();
        let hits = manuk_css::query_selector_all(dom, dom.root(), "#__diag__");
        match hits.first() {
            Some(&n) => println!("\n{}\n", dom.text_content(n)),
            None => println!("\nNO DIAG — the eval itself did not run (no JS context?)\n"),
        }
    });
}
