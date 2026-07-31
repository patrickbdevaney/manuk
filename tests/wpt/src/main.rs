//! `manuk-wpt` — run the conformance suite and report results.
//!
//! - No args: runs the built-in layout reftests.
//! - `--wpt <dir> [subdir]` (or `$WPT_DIR`): runs the **upstream WPT reftests** under
//!   `<dir>/<subdir>` — check the tree out at the commit pinned in IMPLEMENTATION.md
//!   (`7f6164e…`, 2026-07-09) so pass counts are meaningful.

use std::path::PathBuf;

/// Which site the fidelity sweep is currently measuring, as a monotonic counter.
///
/// The per-site timeout watchdog is a detached thread, so it cannot be joined or cancelled — it is
/// disarmed by this counter instead: a timer captures the generation it was armed for and does nothing
/// unless that is still the current one. Without it, a `--urls a,b` run would let site A's timer fire
/// while site B is rendering and file B's row as A's timeout.
static SITE_GEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

    // `manuk-wpt certificate --rows FILE` — the Phase-0 exit certificate over an ACCUMULATED sweep.
    // The companion to `fidelity --rows-out`: a chunked sweep writes rows, this reads them all and
    // prints one certificate. No engine, no network, no fonts — it is arithmetic over a file.
    if args.first().map(String::as_str) == Some("certificate") {
        let Some(p) = flag(&args[1..], "--rows") else {
            eprintln!(
                "usage: manuk-wpt certificate --rows FILE   (written by `fidelity --rows-out`)"
            );
            std::process::exit(2);
        };
        match manuk_wpt::fidelity::rows_from_tsv(std::path::Path::new(&p)) {
            Ok(rows) => {
                eprintln!(
                    "certificate over {} accumulated site row(s) from {p}",
                    rows.len()
                );
                // The spread FIRST — `rows_from_tsv` keeps the last row per site, so by the time the
                // certificate prints, the evidence that a site was measured three times with a
                // 3.7-point range is already gone. See `fidelity::shape_spreads`.
                if let Ok(text) = std::fs::read_to_string(&p) {
                    manuk_wpt::fidelity::spread_report(&text);
                }
                manuk_wpt::fidelity::certificate_report(&rows);
            }
            Err(e) => {
                eprintln!("✗ {e}");
                std::process::exit(2);
            }
        }
        return;
    }

    // `manuk-wpt test262` — the ECMAScript conformance suite against the engine we ship. See
    // `test262.rs` for the probity rules and the runner's stated limits.
    if args.first().map(String::as_str) == Some("test262") {
        run_test262_cmd(&args[1..]);
        return;
    }

    // `manuk-wpt memtabs` — the standing 100-tab RSS benchmark. See `memtabs.rs` for what
    // the memory claim actually says and what would falsify it.
    if args.first().map(String::as_str) == Some("memtabs") {
        manuk_wpt::memtabs::run(&args[1..], &fonts);
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
        // **How many images LANDED, not just how long the phase took.** The elapsed time alone reads
        // the same whether every image arrived or every image timed out — and on a stampeded host it
        // is the second one. A phase that reports only its duration cannot tell a slow success from a
        // fast total loss, which is exactly the distinction this measurement exists to make.
        let n_ok = imgs.len();
        println!(
            "  FIRST PAINT {t_paint:7.1}ms  (fetch+parse {t_fetch:7.1}ms)   \
             then {n_def} deferred scripts in {t_def:7.1}ms, {n_ok} of {n_img} images in {t_img:7.1}ms",
        );
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
    // `--urls-file` takes the corpus from a file — one URL per line, `#` comments and blanks
    // ignored, and a leading `category<WHITESPACE>url` (the shape of `docs/bench/oracle-corpus.txt`)
    // has the category stripped. A 265-site sweep is not expressible as a comma list on a command
    // line without hitting ARG_MAX and, worse, without any record of WHICH list was swept.
    //
    // ⚠⚠ **THIS SPLIT WAS `\t`-ONLY AND `oracle-corpus.txt` IS SPACE-ALIGNED** (`news` then padding
    // then the URL), so `rsplit('\t')` returned each line WHOLE, `starts_with("http")` rejected all
    // 265 of them, and the sweep ran over an EMPTY corpus — and then printed a full Phase-0
    // certificate reading `sites 0 · scored 0 · shape ≥0.75 on 0 (0.0%)` with five shortfall lines.
    // Every column was `NaN` or `0.0%` and none of it said *the corpus was empty*.
    //
    // That is the project's oldest failure shape wearing a new hat: **a metric whose denominator
    // comes from the thing being measured is satisfied by measuring LESS**, and the tick-650 lesson
    // says the fix is never a stricter threshold — it is a SECOND POPULATION that the metric cannot
    // silently shrink. Here that population is the FILE: it knows how many candidate lines it had,
    // and a parse that turns N candidates into 0 URLs is an INPUT error, not a 0% score.
    //
    // Split on any whitespace so both shapes parse, and refuse loudly rather than sweep nothing.
    let urls = match flag(args, "--urls-file") {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(text) => {
                let parsed = manuk_wpt::fidelity::parse_corpus(&text);
                // A sweep of nothing is not a sweep, and a certificate computed over nothing is a
                // lie with a clean bill of health. Never let the run reach the scorers.
                if parsed.urls.is_empty() {
                    eprintln!(
                        "✗ --urls-file {p}: {} non-comment line(s), and NOT ONE parsed as a URL. \
                         The corpus was not swept; no certificate is computable from it. \
                         Expected `url` or `category<whitespace>url` per line.",
                        parsed.candidates
                    );
                    std::process::exit(2);
                }
                if parsed.urls.len() < parsed.candidates {
                    // A partial parse is the same bug, quieter. Say how much was dropped, because a
                    // sweep of 12 sites reported as "the corpus" is how an optimistic number is born.
                    eprintln!(
                        "⚠ --urls-file {p}: {} of {} non-comment line(s) parsed as URLs — {} DROPPED \
                         and NOT swept. The certificate below covers the {} that parsed, not the file.",
                        parsed.urls.len(),
                        parsed.candidates,
                        parsed.candidates - parsed.urls.len(),
                        parsed.urls.len()
                    );
                }
                parsed.urls.join(",")
            }
            Err(e) => {
                eprintln!("✗ cannot read --urls-file {p}: {e}");
                std::process::exit(2);
            }
        },
        None => match flag(args, "--urls") {
            Some(u) => u.to_string(),
            None => {
                eprintln!(
                    "usage: manuk-wpt fidelity (--urls URL[,URL...] | --urls-file PATH) [--out DIR] [--floor 0.9]"
                );
                std::process::exit(2);
            }
        },
    };
    let _ = std::fs::create_dir_all(&out);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
    let mut rows = Vec::new();
    // Brick 5 (§3b) accumulator — every page's SHAPE/display/missing divergences, pooled so the
    // gate can cluster them into DISTINCT ROOT CAUSES at the end instead of reciting per-site counts.
    let mut all_divs: Vec<manuk_wpt::oracle::Divergence> = Vec::new();

    // ── THE LEDGER IS WRITTEN AS IT IS EARNED, NOT AT THE END.
    //
    // `--rows-out` used to append once, after the loop, which made every completed site hostage to
    // the survival of the whole run. It is not a survivable run: the sweep drives our own engine
    // over twenty live sites in ONE process, and this session's HEAD-20 attempts died of an engine
    // SIGSEGV **twice in three runs** — at site 5 and at site 11 — each time discarding every row
    // already measured. Ten sites of work, and the certificate could not be computed at all.
    //
    // That is the identical harm `Unmeasurable::Timeout` documents for an unbounded CHILD, and t625
    // fixed it there by bounding the child. It could not be fixed the same way here, because the
    // process that dies is *us*. So the answer is the other half: make the ledger durable, so a
    // crash costs exactly the site that crashed. `append_rows_tsv` was already append-only and
    // `certificate --rows` already reads an accumulated file — the durability was one call site away
    // the whole time.
    //
    // Flushing at the TOP of each iteration (rather than the bottom) is deliberate: the body has
    // five `continue` paths, and a bottom-of-loop flush would silently skip every one of them.
    // Before site N begins, sites 1..N-1 are on disk. That is the invariant, and it is `continue`-proof.
    let rows_out = flag(args, "--rows-out").map(PathBuf::from);
    // Our OWN per-site budget, strictly under the sweep runner's external `timeout` so that WE file the
    // row rather than being killed holding a marker the next run can only read as `crashed`. See the
    // arming site below for why this exists at all.
    //
    // **150s is measured, not picked.** Across the 438 runs that SUCCEEDED in the two real corpus sweeps
    // (t750's 265 sites + t752's 200), the slowest took **132s** and the distribution is
    // `<30s: 275 · 30-60: 137 · 60-90: 20 · 90-120: 4 · 120-150: 2 · >=150: 0`. So this budget sits above
    // every success ever observed and below the 180s external watchdog: it converts kills into honest
    // timeouts without costing a single measurable site. `--site-budget` overrides for a slower box or a
    // different external watchdog — and if the two are ever mis-ordered, the failure is visible (rows go
    // back to reading `crashed`) rather than silent.
    let site_budget: u64 = flag(args, "--site-budget")
        .and_then(|v| v.parse().ok())
        .unwrap_or(150);
    let mut flushed = 0usize;
    if let Some(p) = &rows_out {
        // The previous run's marker, if it died holding one. Recovered FIRST, so the site that
        // killed it is counted in this run's file rather than dropped by both.
        if let Some(name) = manuk_wpt::fidelity::recover_inflight(p) {
            eprintln!(
                "  RECOVERED [crashed]: {name} — the previous run died while rendering it; counted, not dropped"
            );
        }
    }
    let mut flush = |rows: &[manuk_wpt::fidelity::Fidelity], flushed: &mut usize| {
        // Disarm this site's watchdog: any timer still sleeping now belongs to a generation that has
        // already produced its row, and must stay silent.
        SITE_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(p) = &rows_out {
            if *flushed < rows.len() {
                if let Err(e) = manuk_wpt::fidelity::append_rows_tsv(p, &rows[*flushed..]) {
                    eprintln!("✗ --rows-out {}: {e}", p.display());
                }
                *flushed = rows.len();
            }
            manuk_wpt::fidelity::clear_inflight(p);
        }
    };

    // ── A SITE THE INSTRUMENT HAS MEASURED AS UNSTABLE DOES NOT GET A SINGLE DRAW.
    //
    // The spread block has printed which sites those are since tick 657 and nothing consumed it.
    // Tick 672 is what that costs: `keirin.jp` read **0.048 against a ~0.40 population** and the
    // certificate published it as that site's score. Three controls on the same tree minutes later
    // read 0.400 / 0.351 / 0.402, so the sweep's number was a draw, not a measurement — and it was
    // one paragraph away from being reported as a 35-point regression against the previous tick.
    //
    // The repeats are CONSECUTIVE because `rows_from_tsv` takes the median of a consecutive run and
    // last-wins across separated ones; scattering them would pay for three renders and publish the
    // last. And this is deliberately keyed on `--rows-out`: without an accumulated file there is no
    // spread to read, so the two-site G1 wall gate is untouched and costs nothing.
    let mut sweep_urls: Vec<String> = urls
        .split(',')
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
        .collect();
    if let Some(p) = &rows_out {
        if let Ok(text) = std::fs::read_to_string(p) {
            let (expanded, plan) = manuk_wpt::fidelity::repeat_urls(&sweep_urls, &text);
            for (name, n, pts) in &plan {
                eprintln!(
                    "  ⟳ REPEAT-UNSTABLE: {name} rendered {n}× this sweep (recorded spread {pts:.1} pts > {:.1}); \
                     the certificate takes the MEDIAN of the run, not the last draw",
                    manuk_wpt::fidelity::SPREAD_UNSTABLE_PTS
                );
            }
            sweep_urls = expanded;
        }
    }

    // ── **`--jobs N` — THE SWEEP IS 40–60% OF THE TICK CYCLE AND IT RAN ONE SITE AT A TIME.**
    //
    // A 200-site trend sweep took ~2h serial (measured, t767: ~1min/site on 32 cores, with 31 of them
    // idle). Nothing lands while it runs, so it is pure cycle overhead — the board's throughput
    // directive (owner, 2026-07-30) names parallelising it as the single biggest lever.
    //
    // The fan-out is **process-level, not thread-level**, and that is not a convenience:
    //   * a site that SEGFAULTS (tick 768 found a live one) kills its own chunk and nothing else — the
    //     other chunks keep their rows, and the in-flight marker recovery already handles the victim;
    //   * each child gets its own SpiderMonkey runtime and its own Chromium, which is what the scorer
    //     needs anyway (one runtime per process is the model everywhere else in this binary);
    //   * `--rows-out` is append-with-flush per site, so N children writing N chunk files and one merge
    //     at the end preserves the "sites 1..N-1 are on disk before N starts" invariant per chunk.
    //
    // The parent does no rendering: it splits, spawns, waits, merges, and re-reads the merged file
    // through the SAME certificate path a serial run uses, so the number cannot depend on the job count.
    let jobs: usize = flag(args, "--jobs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    if jobs > 1 && sweep_urls.len() > 1 {
        let Some(rows_path) = rows_out.clone() else {
            eprintln!("✗ --jobs requires --rows-out (the chunks are merged into it)");
            std::process::exit(2);
        };
        let exe = std::env::current_exe().expect("current exe");
        let chunks = jobs.min(sweep_urls.len());
        // Round-robin rather than contiguous blocks: the corpus is stratified (HEAD sites first), so
        // contiguous chunks would give one child every slow site and leave the rest idle at the end.
        let mut buckets: Vec<Vec<String>> = vec![Vec::new(); chunks];
        for (i, u) in sweep_urls.iter().enumerate() {
            buckets[i % chunks].push(u.clone());
        }
        let stamp = std::process::id();
        // ⚠ **A CHUNK CAN EXIT EARLY, AND EVERY SITE AFTER IT WOULD VANISH.** The per-site watchdog
        // files its `timeout-150s` row and then `std::process::exit(0)`s — correct for a wedged main
        // thread, fatal for a chunk, because the sites QUEUED BEHIND it in that child never run.
        // Measured on a 9-site trial: 8 rows merged, `www.ikea.com` silently absent, which is precisely
        // the denominator trap ("a metric satisfied by measuring LESS") this instrument refuses.
        //
        // So a chunk is not one spawn, it is a spawn LOOP: re-spawn the remainder until every URL in the
        // bucket has a row or the retry cap is reached, and file an explicit `crashed` row for anything
        // still missing. The certificate's denominator stays whole either way.
        const CHUNK_ROUNDS: usize = 4;
        let run_chunk = |urls: &[String], part: &std::path::Path| -> std::io::Result<()> {
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("fidelity")
                .arg("--urls")
                .arg(urls.join(","))
                .arg("--rows-out")
                .arg(part)
                .arg("--width")
                .arg(vw.to_string())
                .arg("--height")
                .arg(vh.to_string())
                .arg("--site-budget")
                .arg(site_budget.to_string())
                .stdout(std::process::Stdio::null());
            cmd.spawn()?.wait().map(|_| ())
        };
        let have = |part: &std::path::Path| -> std::collections::HashSet<String> {
            std::fs::read_to_string(part)
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.split('\t').next().map(str::to_string))
                .collect()
        };
        eprintln!(
            "▶ FIDELITY SWEEP: {} sites across {} processes (--jobs {jobs})",
            sweep_urls.len(),
            buckets.len()
        );
        let parts: Vec<std::path::PathBuf> = (0..buckets.len())
            .map(|i| out.join(format!("sweep-part-{stamp}-{i}.tsv")))
            .collect();
        for p in &parts {
            let _ = std::fs::remove_file(p);
        }
        // The buckets run CONCURRENTLY (that is the whole point), each in its own thread that owns its
        // spawn-loop. The threads only spawn and wait; every render happens in a child process.
        std::thread::scope(|sc| {
            for (i, b) in buckets.iter().enumerate() {
                let part = parts[i].clone();
                let run_chunk = &run_chunk;
                let have = &have;
                sc.spawn(move || {
                    let mut todo: Vec<String> = b.clone();
                    for round in 0..CHUNK_ROUNDS {
                        if todo.is_empty() {
                            break;
                        }
                        if run_chunk(&todo, &part).is_err() {
                            eprintln!("  ✗ chunk {i}: failed to spawn (round {round})");
                            break;
                        }
                        if let Some(name) = manuk_wpt::fidelity::recover_inflight(&part) {
                            eprintln!(
                                "  RECOVERED [crashed]: {name} — its chunk died while rendering it; counted, not dropped"
                            );
                        }
                        let done = have(&part);
                        let left: Vec<String> = todo
                            .iter()
                            .filter(|u| !done.contains(manuk_wpt::fidelity::site_name(u).as_str()))
                            .cloned()
                            .collect();
                        if left.len() == todo.len() && round > 0 {
                            break; // no progress this round — stop rather than spin
                        }
                        if !left.is_empty() {
                            eprintln!(
                                "  ⟳ chunk {i} exited early with {} site(s) unrun — re-spawning (round {})",
                                left.len(),
                                round + 1
                            );
                        }
                        todo = left;
                    }
                    // Anything still missing after the cap is UNSCORED-and-counted, never absent.
                    if !todo.is_empty() {
                        let rows: Vec<manuk_wpt::fidelity::Fidelity> = todo
                            .iter()
                            .map(|u| {
                                manuk_wpt::fidelity::Fidelity::unmeasured(
                                    &manuk_wpt::fidelity::site_name(u),
                                    manuk_wpt::fidelity::Unmeasurable::Crashed,
                                )
                            })
                            .collect();
                        eprintln!(
                            "  ⚠ chunk {i}: {} site(s) never produced a row after {CHUNK_ROUNDS} rounds — filed as crashed",
                            rows.len()
                        );
                        let _ = manuk_wpt::fidelity::append_rows_tsv(&part, &rows);
                    }
                });
            }
        });
        let mut merged: Vec<String> = Vec::new();
        for part in &parts {
            let text = std::fs::read_to_string(part).unwrap_or_default();
            merged.extend(
                text.lines()
                    .filter(|l| !l.starts_with('#'))
                    .map(str::to_string),
            );
            let _ = std::fs::remove_file(part);
        }
        let header = "#name\tcoverage\tshape\th_overflow\toverlap\treading_order\tdead_target\tshape_n\treason\tinstrument";
        let body = merged.join("\n");
        let _ = std::fs::write(&rows_path, format!("{header}\n{body}\n"));
        // SAMPLED == ROWS is the reconciliation this instrument owes (METHODOLOGY: accounting
        // reconciliation is a first-class mechanism). Print BOTH numbers, always.
        eprintln!(
            "▶ merged {} row(s) into {} (sampled {})",
            merged.len(),
            rows_path.display(),
            sweep_urls.len()
        );
        if merged.len() != sweep_urls.len() {
            eprintln!(
                "  ⚠ RECONCILIATION: {} sampled vs {} rows — the difference is an INSTRUMENT bug, not a result",
                sweep_urls.len(),
                merged.len()
            );
        }
        return;
    }

    for url in &sweep_urls {
        let url = url.as_str();
        let name = manuk_wpt::fidelity::site_name(url);
        // Everything the previous site earned goes to disk before this one is allowed to touch the
        // engine, and only then do we claim this site as in-flight.
        flush(&rows, &mut flushed);
        if let Some(p) = &rows_out {
            if let Err(e) = manuk_wpt::fidelity::mark_inflight(p, &name) {
                eprintln!("✗ in-flight marker for {name}: {e}");
            }
        }
        eprintln!("fidelity: {name}");

        // ── **BOUND OUR OWN WORK, SO A SLOW SITE IS A `timeout`, NOT A MANUFACTURED `crashed`.**
        //
        // The sweep runs each site under an EXTERNAL watchdog (`timeout -k 10 180 …`). When that fires,
        // this process is killed holding its in-flight marker, and the next run recovers that marker as
        // `Unmeasurable::Crashed` — because the marker carries only a site name, and a killed process
        // and a faulting one leave the identical trace. So the t752 baseline reported **8 `crashed`
        // sites**, every one of them `rc=124` at exactly 180s: no panic, no SIGSEGV, no OOM.
        //
        // ⚠ That is the most expensive kind of instrument lie. `Crashed` is a **Bar 0** event, and Bar 0
        // outranks every visual divergence in the priority ledger (Part 24.3) — so a future session
        // reading `8 crashed` would correctly drop everything to hunt a crash that does not exist.
        // t706 already learned this once ("an external SIGKILL and a SIGSEGV leave the same marker") and
        // fixed it OUTSIDE, by having the runner log `rc` beside the row. The runner does. The row is
        // written by the recovery pass, which has never seen the runner's file, so the two records never
        // meet — the same shape as t751's oracle-refuses/fidelity-scores split.
        //
        // Fixing it by teaching the recovery pass to read the runner's `rc` would wire the instrument to
        // one particular runner. Instead, remove the ambiguity at the SOURCE: give ourselves a budget
        // strictly under the external one and file our own honest `Timeout` row before we can be killed.
        // The external watchdog stays as a true backstop for a genuine hang below this thread, and if it
        // ever fires again, `crashed` will mean what it says.
        //
        // The watchdog is armed per site and disarmed by a GENERATION counter rather than by being
        // joined: a `--urls a,b` run must not have site A's timer fire while site B is rendering.
        if let Some(p) = &rows_out {
            let gen = SITE_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let (p, name_c) = (p.clone(), name.clone());
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(site_budget));
                if SITE_GEN.load(std::sync::atomic::Ordering::SeqCst) != gen {
                    return; // this site finished; the timer belongs to a completed generation
                }
                let reason = manuk_wpt::fidelity::Unmeasurable::Timeout(site_budget);
                eprintln!(
                    "  UNMEASURABLE [{}]: {}",
                    reason.tag(),
                    "this engine did not finish the site inside its own budget — filed as a TIMEOUT by \
                     us, so it is never recovered as a phantom Bar-0 `crashed` by the next run"
                );
                let row = manuk_wpt::fidelity::Fidelity::unmeasured(&name_c, reason);
                let _ = manuk_wpt::fidelity::append_rows_tsv(&p, std::slice::from_ref(&row));
                manuk_wpt::fidelity::clear_inflight(&p);
                // Exit rather than unwind: the row and the cleared marker are already on disk, and the
                // main thread is wedged in whatever took too long.
                std::process::exit(0);
            });
        }

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
        // The SECOND silent drop on this path, found by the first honest run of the fix below: a
        // 20-site sweep reported `sites 17`. Three origins failed OUR fetch and "skipped" — leaving
        // no row, hence no denominator entry. `Unreachable` is the honest classification: our own
        // fetch is where the transport failure surfaced, and unlike `capture_url_screenshot` it
        // hands back no status to be more specific with.
        let Ok((html, final_url)) = rt.block_on(manuk_page::fetch_html(url)) else {
            let reason = manuk_wpt::fidelity::Unmeasurable::Unreachable;
            eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
            rows.push(manuk_wpt::fidelity::Fidelity::unmeasured(&name, reason));
            continue;
        };
        let page = rt.block_on(async {
            let mut p = Page::load_async(&html, &final_url, fonts, vw as f32).await;
            p.finish_loading(fonts, vw as f32).await;
            p
        });
        let mpath = out.join(format!("{name}.manuk.png"));
        // …and the THIRD. A page we fetched but could not paint is the most damning outcome of the
        // four, and it was the quietest: it left no row at all, so a render failure REMOVED the site
        // from the corpus rather than counting against it.
        if page.paint(fonts, vw, vh).save_png(&mpath).is_err() {
            let reason = manuk_wpt::fidelity::Unmeasurable::RenderFailed;
            eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
            rows.push(manuk_wpt::fidelity::Fidelity::unmeasured(&name, reason));
            continue;
        }
        let manuk_ms = t_manuk.elapsed().as_millis();

        // …and the FOURTH, which is the one the ORACLE had already been refusing all along.
        //
        // ⚠⚠⚠ **A PAGE WHOSE AUTHOR STYLESHEETS NEVER ARRIVED MUST NOT BE GIVEN A SHAPE SCORE.** We
        // painted faithfully; we painted the *wrong document* — the UA stylesheet's idea of it. Diffing
        // that against a fully-styled Chromium charges **network weather to the layout engine's
        // account**, which is precisely what `Page::failed_stylesheet_fetches`'s own doc comment says
        // it exists to prevent: *"The differential oracle discards such runs."*
        //
        // It does. `run_oracle_cmd` calls this very method and prints `DISCARDED`. **This path — the
        // one that produces the Phase-0 headline — never asked.** One question, two instruments, two
        // answers, and the permissive one was the published one.
        //
        // The signature is unmistakable once named: **coverage ≈ 1.000 with shape ≈ 0.** Every element
        // is present (there was no CSS to drop any) and almost none is placed (there was no CSS to
        // place it). `postgresql.org` was scored `cov 1.000 / shape 0.018` over 337 nodes in the t745
        // sweep while the oracle discarded that same site for three starved sheets — and it is
        // INTERMITTENT, so a site alternates between its real shape and ~0 across runs, which reads
        // exactly like a layout regression and is a standing variance source in the headline.
        //
        // Counted IN-SCOPE, not excluded: our own `load_deadline` cut those sheets, so this is our bug.
        //
        // ⚠⚠ **THE REFUSAL IS ABOUT ASYMMETRY, NOT ABOUT STARVATION — and the wall caught me getting
        // that wrong.** The first version of this check fired on any `starved > 0` and turned `G1`'s
        // `MEAN VISUAL` into **NaN**: G1 feeds the two engines `file://` snapshots from `.verify-cache`,
        // and a snapshot's `href="/static/…"` cannot resolve under `file://`, so `failed_css` is
        // non-zero there **by construction**. Every G1 site went unscored and the gate had an empty set
        // to average.
        //
        // That failure is the useful half: under `file://` **both engines see the same missing sheets**
        // — Chromium renders that snapshot unstyled too — so the diff is still apples-to-apples and the
        // number means something. What makes a starved sheet fatal on the CORPUS is that the reference
        // engine fetches its own subresources over the live network and **succeeds** where we did not:
        // the two sides are then looking at different documents, and that is the thing no diff survives.
        //
        // So the condition is the scheme, and it is not a special case bolted on — it is the actual
        // rule: refuse when the comparison is ASYMMETRIC.
        let starved = page.failed_stylesheet_fetches();
        let live = final_url.starts_with("http://") || final_url.starts_with("https://");
        if starved > 0 && live {
            let reason = manuk_wpt::fidelity::Unmeasurable::CssStarved(starved);
            eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
            rows.push(manuk_wpt::fidelity::Fidelity::unmeasured(&name, reason));
            continue;
        }

        // Chromium — the same live URL, so it fetches its own subresources.
        let t_chrome = std::time::Instant::now();
        let cpath = out.join(format!("{name}.chrome.png"));
        // **A SITE WE COULD NOT REACH IS A COUNTED ROW, NOT A `continue`.**
        //
        // This was `eprintln!; continue` — the site left no row, so it left the DENOMINATOR too, and
        // a sweep's "sites N" silently shrank by however many origins had refused us that day. That
        // is precisely the failure `DAILY-DRIVER-CERTIFICATION.md` §0 names as cause #1 of every past
        // optimistic reading: *dropping the hard sites is what made every past number flattering.*
        // The fixed-denominator rule was built at t583 and this path walked around it.
        if let Err(reason) = manuk_wpt::chrome::capture_url_screenshot(url, vw, vh, &cpath) {
            eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
            rows.push(manuk_wpt::fidelity::Fidelity::unmeasured(&name, reason));
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
                // element against Manuk's, keyed by SELECTOR-PATH (tick 532: replaced `[id]` keys,
                // which 39% of the corpus lacks and which carry no `/`-ancestry for SHAPE). A missing
                // sidebar is a MISSING BOX; the pixel score averages it away, this does not.
                // **THE REASON IS RECORDED, NOT DISCARDED.** This was `if let Ok(...)` — every way of
                // failing to reach a page fell into the same silent `else`, the row printed `—`, and
                // the certificate counted an UNSCORED site it could not explain. t602 asked "why can
                // 9 of 14 not be scored"; the instrument had thrown the answer away here.
                let probe = manuk_wpt::chrome::capture_seen_all_paths(url, vw, vh);
                if let Err(reason) = &probe {
                    eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
                    f.unmeasurable = Some(reason.clone());
                }
                if let Ok(mut cseen) = probe {
                    cseen.remove("__META__"); // health metadata, not an element (as run_oracle_cmd does)
                    let dom = page.dom();
                    let rects = page.root_box.node_rects(dom);
                    let styles = page.styles_map();
                    // Manuk side as `oracle::Seen` (tag + display + box), keyed by the SAME selector-
                    // path Chromium's probe computes (the shared free functions, so the exit gate keys
                    // pages exactly as the differential oracle does). Since tick 537 (brick 4b) the
                    // producer carries tag+display, so the G1 exit gate scores placement AND the four
                    // jarring invariants through the oracle's OWN functions — no Box4 mirror in between.
                    // Skip the structural tags Chrome's probe also skips, and skip non-rendered boxes so
                    // both sides only carry what actually painted (SHAPE scores the intersection anyway).
                    let mut mseen: std::collections::HashMap<String, manuk_wpt::oracle::Seen> =
                        std::collections::HashMap::new();
                    for n in dom.descendants(dom.root()) {
                        if !dom.is_element(n) {
                            continue;
                        }
                        let Some(tag) = dom.tag_name(n) else { continue };
                        if matches!(
                            tag,
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
                            continue;
                        }
                        let Some(r) = rects.get(&n) else { continue };
                        if r.width <= 0.0 && r.height <= 0.0 {
                            continue;
                        }
                        if let Some(path) = path_of(dom, n) {
                            let display = styles
                                .get(&n)
                                .map(|s| css_display_name(s.display))
                                .unwrap_or("none")
                                .to_string();
                            // **THE FIRST DECLARED FAMILY — because that is what the other side of
                            // the diff records, and a diff of two different quantities is not a diff.**
                            //
                            // This used to report the RESOLVED family, under a comment asserting "so
                            // the two are comparable". They were not. Chromium's probe emits
                            // `getComputedStyle(e).fontFamily.split(',')[0]`, which is the first
                            // **declared** entry of the stack — `getComputedStyle` cannot report the
                            // face actually used, and no DOM API can. So the field compared a
                            // DECLARATION against a RESOLUTION and therefore differed on essentially
                            // every element of every page that ships a font stack, which is every page.
                            //
                            // The visible result was `{-apple-system/17} vs {sans-serif/17}` in the
                            // root-cause clusters, which reads as "Chromium used the system font and we
                            // fell back" and means no such thing: Chromium on Linux has no
                            // `-apple-system` either and falls back exactly as we do. t563 added this
                            // field to ATTRIBUTE text-metric divergences and it could not — the one job
                            // it was built for.
                            //
                            // What it can honestly answer is "did the two engines' CASCADES arrive at
                            // the same font-family declaration for this element", which is a real
                            // question with a real failure mode, and is now what it asks. Attributing a
                            // metric difference to a FACE needs a channel Chromium does not expose;
                            // pretending otherwise is what made the field misleading rather than merely
                            // limited.
                            let font = styles
                                .get(&n)
                                .map(|st| {
                                    let fam = st
                                        .font_family
                                        .first()
                                        .map(|f| f.trim().trim_matches(['"', '\'']).to_string())
                                        .unwrap_or_else(|| "?".to_string());
                                    format!("{fam}/{}", st.font_size.round() as i64)
                                })
                                .unwrap_or_default();
                            mseen.insert(
                                path,
                                manuk_wpt::oracle::Seen {
                                    tag: tag.to_string(),
                                    display,
                                    font,
                                    rect: [
                                        r.x.round() as i64,
                                        r.y.round() as i64,
                                        r.width.round() as i64,
                                        r.height.round() as i64,
                                    ],
                                },
                            );
                        }
                    }
                    // ── THE CLASS SIGNATURE IS OFF THE KEY (tick 550, MEASURED).
                    //
                    // The t549 sweep found 13 of 54 sites under 5% coverage, and several have the
                    // signature of a KEYING failure rather than a rendering one: `gov.uk` 418 of 418
                    // missing in 6.8s with no timeout excuse, `nytimes.com` 2,406 of 2,407. The
                    // suspect is the `.SIG` component the path key carries at EVERY level: a path's
                    // identity is hostage to the class lists of all its ancestors, and class lists are
                    // the single most JS-mutated thing on the web. Chrome's gov.uk body key is
                    // `body.ba1d8e99:nth-child(2)` (its JS adds `js-enabled`); Chrome's nytimes body key
                    // is `body:nth-child(2)` (no class at all). ONE differing class on `<body>`
                    // invalidates every descendant key on the page, in either direction — the
                    // Wikipedia `client-nojs`→`client-js` lesson generalised.
                    //
                    // ABLATION MEASURED, on six sites, and it is decisive in both directions:
                    //
                    //   healthy sites, sigs ON vs OFF ......... BYTE-IDENTICAL
                    //     jvns.ca cov 100.0% / shape 94.3%  ·  blog.rust-lang.org 100.0% / 87.4%
                    //     lobste.rs 84.1% / 85.8%           ·  every figure unchanged to the decimal
                    //   sites reading ~0%, sigs ON -> OFF ..... RECOVERED
                    //     gov.uk    coverage   0.0% -> 82.8%   (418 paths: 418 missing -> 72 missing)
                    //     stripe.com coverage  0.1% -> 43.1%   (1441 paths: 1439 -> 820 missing)
                    //   and one that did NOT move — nytimes.com 0.0% -> 0.0% (2,381 of 2,382 still
                    //     missing), which is how we know this is a real second failure and not all one bug
                    //
                    // So the signature adds NO discriminating power where the two DOMs agree, and
                    // DESTROYS the measurement where a single ancestor's class list differs. `nth-child`
                    // already distinguishes siblings uniquely, so the sig never carried identity — only
                    // fragility. It is off by default from here.
                    //
                    // `MANUK_G1_CLASS_SIG=1` restores it, so the decision stays auditable and reversible
                    // rather than becoming folklore. ⚠ Two consequences to carry forward: the t549
                    // certificate's COVERAGE figures for the sub-5% class are INVALIDATED (pessimistic),
                    // and `run_oracle_cmd`'s crawl keys still carry sigs — the same correction is owed
                    // there, as its own tick.
                    let (cseen, mseen) = if std::env::var_os("MANUK_G1_CLASS_SIG").is_some() {
                        (cseen, mseen)
                    } else {
                        (strip_sigs(cseen), strip_sigs(mseen))
                    };
                    // Rect-only Box4 views for the placement scorers (SHAPE / coverage / first-
                    // divergence still take bare box maps); the jarring invariants below read the
                    // `Seen` maps directly.
                    let cmap: std::collections::HashMap<String, [i64; 4]> =
                        cseen.iter().map(|(k, v)| (k.clone(), v.rect)).collect();
                    let mboxes: std::collections::HashMap<String, [i64; 4]> =
                        mseen.iter().map(|(k, v)| (k.clone(), v.rect)).collect();
                    let (sc, missing, misplaced, probed, missing_ids) =
                        manuk_wpt::fidelity::compare_structure_detail(&cmap, &mboxes, 8);
                    // Which elements are missing? A coverage number is only actionable if it names
                    // the culprits — and 1,402 missing elements are a handful of CLASS bugs, not
                    // 1,402 bugs. The key IS a selector-path, so the missing element's tag is the
                    // tag of its LAST component (after the final '/', before its `.SIG`/`:nth-child`)
                    // — no DOM lookup needed, and it works even though the key is no longer an `id`.
                    if !missing_ids.is_empty() {
                        let mut by_tag: std::collections::BTreeMap<String, usize> =
                            std::collections::BTreeMap::new();
                        for id in &missing_ids {
                            let leaf = id.rsplit('/').next().unwrap_or(id.as_str());
                            let tag = leaf
                                .split(['.', ':'])
                                .next()
                                .filter(|s| !s.is_empty())
                                .unwrap_or("(unknown)")
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
                    // ⚠ **BOTH SIDES' COUNTS, ALWAYS** (t782, meta-instrument #3 — accounting
                    // reconciliation). This line used to print only the ORACLE's `probed` and how
                    // many of those we lacked, which reads as "we rendered 15.8% of the page" and
                    // can be true at the same time as *"we rendered three times as many boxes as the
                    // reference"* — which is what naukri.com actually does. A coverage ratio whose
                    // denominator is one engine and whose numerator is an intersection says nothing
                    // about the size of the other side; printing `ours` is what makes the two
                    // readings distinguishable at a glance instead of one tick's archaeology.
                    eprintln!(
                        "  structural: {:.1}% (oracle {probed} paths, ours {} paths, {missing} missing, {misplaced} misplaced)",
                        sc * 100.0,
                        mboxes.len()
                    );
                    // **A HANDFUL OF PROBED ELEMENTS IS A SHELL, AND SCORING IT IS A FALSE NUMBER.**
                    //
                    // The oracle serves one fetched copy from `file://` so both Chrome probes see the
                    // same document — deliberate, and it costs this: from `file://` the page's origin
                    // is `null`, so a JS-rendered site's own fetches are cross-origin and blocked and
                    // Chrome builds almost nothing. `comix.to` yields 28 elements here against ~2643
                    // live, and the certificate duly printed `coverage 66.7%` computed over THREE
                    // elements — in the same column, in the same units, as a 4,122-path score.
                    //
                    // The threshold is `CERT_MIN_SHAPE_SAMPLE`, reused rather than invented: the
                    // certificate ALREADY refuses to score a placement ratio over fewer than that many
                    // elements, so this changes no verdict. It supplies the REASON that refusal never
                    // had, which is the whole of t611's unexplained residue.
                    if let Some((last_ok, _, first_bad, dy)) =
                        manuk_wpt::fidelity::first_divergence(&cmap, &mboxes, 60)
                    {
                        eprintln!("  FIRST DIVERGENCE: after #{last_ok}, element #{first_bad} is off by dy={dy}");
                    }
                    // Layer-1 SHAPE (parent-relative) — the redesign's primary placement number,
                    // now that the producer emits `/`-keyed paths with real ancestry. It replaces
                    // the old absolute PLACEMENT line, which charged one constant offset N times.
                    // `placement_stats` (absolute) is kept as a Layer-3 diagnostic only.
                    let (shape, shape_n) = manuk_wpt::fidelity::shape_stats(&cmap, &mboxes, 8);
                    f.shape = Some(shape);
                    // The SAMPLE travels with the ratio. `shape_stats` returns 1.0 over an empty
                    // sample, and the first real sweep produced seven `SHAPE: 100.0% … (0 scored)`
                    // rows that the certificate counted as passes — one of them a page whose 418 probed
                    // elements were ALL missing. The certificate needs `shape_n` to refuse that.
                    f.shape_n = shape_n;
                    // **WHY it cannot be scored, decided in ONE place** — and only now, because the
                    // question needs BOTH numbers: what the oracle built (`probed`) and what the two
                    // engines share (`shape_n`). Deciding it above, on `probed` alone, could say
                    // "the oracle rendered a shell" and could not say "the oracle rendered the page
                    // and we did not" — so `ebay` (probed 25, common 4) fell through both and left
                    // the certificate's *"the instrument could not say why"* row.
                    // Never overwrites a reason already established (a bot-wall, a blank render):
                    // the earlier cause is the true one, and this is the residue.
                    if f.unmeasurable.is_none() {
                        if let Some(reason) =
                            manuk_wpt::fidelity::unscoreable_reason(probed, shape_n, mboxes.len())
                        {
                            eprintln!("  UNMEASURABLE [{}]: {}", reason.tag(), reason.explain());
                            f.unmeasurable = Some(reason);
                        }
                    }
                    let (dx, dy, dw, dh, within) =
                        manuk_wpt::fidelity::placement_stats(&cmap, &mboxes, 8);
                    eprintln!(
                        "  SHAPE: {:.1}% within 8px vs shared ancestor ({shape_n} scored)  |  \
                         [diag] absolute PLACEMENT {:.1}% (median dx={dx} dy={dy} dw={dw} dh={dh})",
                        shape * 100.0,
                        within * 100.0
                    );
                    // Layer-2 JARRING invariants (FIDELITY-SCORING-REDESIGN.md §2) — the actual Phase-0
                    // bar SHAPE cannot see. Since brick 4b the G1 producer carries `oracle::Seen`, so all
                    // four call the oracle's OWN functions directly (no Box4 mirror re-deriving the tag) —
                    // the exit gate and the differential crawl can never drift on what "jarring" means.
                    //
                    // #1 horizontal overflow: a box we alone push past `vw` that Chrome keeps inside.
                    let (hover, hover_ex) =
                        manuk_wpt::oracle::jarring_h_overflow(&cseen, &mseen, vw as i64, 8);
                    if hover > 0 {
                        eprintln!(
                            "  H-OVERFLOW: {hover} element(s) escape the {vw}px viewport (Chrome keeps them in)  e.g. {}",
                            hover_ex
                                .iter()
                                .take(2)
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                    }
                    // #2 sibling overlap — the #1 "broken page" perception (text on text, control under a
                    // banner): siblings Chrome renders disjoint but we render colliding. Now wired into the
                    // exit gate (needed `Seen`'s sibling grouping, which the Box4 producer could not carry).
                    let (overlap, ov_skip, ov_ex) =
                        manuk_wpt::oracle::jarring_overlap(&cseen, &mseen, 8);
                    if overlap > 0 {
                        eprintln!(
                            "  OVERLAP: {overlap} sibling pair(s) collide that Chrome keeps apart{}  e.g. {}",
                            if ov_skip > 0 {
                                format!(" ({ov_skip} large group(s) unscanned)")
                            } else {
                                String::new()
                            },
                            ov_ex
                                .iter()
                                .take(2)
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                    }
                    // #3 reading-order inversion — a float/abspos/grid item we alone pull out of sequence,
                    // which SHAPE and overlap both miss (well-shaped, disjoint, yet read swapped).
                    let (rinv, rinv_skip, rinv_ex) =
                        manuk_wpt::oracle::jarring_reading_order(&cseen, &mseen, 8);
                    if rinv > 0 {
                        eprintln!(
                            "  READING-ORDER: {rinv} sibling pair(s) read out of sequence vs Chrome{}  e.g. {}",
                            if rinv_skip > 0 {
                                format!(" ({rinv_skip} large group(s) unscanned)")
                            } else {
                                String::new()
                            },
                            rinv_ex
                                .iter()
                                .take(2)
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                    }
                    // #4 collapsed interactive target (the box-dump half of hittability): a control an
                    // interactive tag names, that Chrome renders with a real clickable box but Manuk
                    // collapses to <2px on an axis — a dead button. Reads `Seen.tag`, no key-parse mirror.
                    let (dead, dead_ex) =
                        manuk_wpt::oracle::jarring_collapsed_target(&cseen, &mseen, 2);
                    if dead > 0 {
                        eprintln!(
                            "  DEAD-TARGET: {dead} interactive control(s) collapsed below hit size (Chrome gives them area)  e.g. {}",
                            dead_ex
                                .iter()
                                .take(2)
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                    }
                    // ── KEEP the four counts on the row, don't just print them (tick 547).
                    //
                    // They were computed and printed per site since brick 4b and then dropped on the
                    // floor, which meant the exit CERTIFICATE — whose bar is literally "≥95% of sites
                    // clean on each invariant" — could not be computed from a sweep at all: it needed a
                    // human reading 265 stanzas of stderr, which is the kind of step that gets skipped
                    // and then estimated. A number printed and discarded is a log line, not a
                    // measurement.
                    f.jarring = [hover, overlap, rinv, dead];
                    // Brick 5 (§3b): pool THIS page's divergences (missing / display / SHAPE-geometry)
                    // for end-of-sweep root-cause clustering. `diff_page` is the differential oracle's
                    // OWN diff (parent-relative SHAPE + display-before-geometry), so the exit gate and
                    // the crawl can never drift on what counts as a divergence — one definition.
                    all_divs.extend(manuk_wpt::oracle::diff_page(&name, &cseen, &mseen, 8));
                }
                rows.push(f);
            }
            Err(e) => eprintln!("  compare: {e}"),
        }
    }

    // Brick 5 (§3b) ROOT-CAUSE CLUSTERING — group the pooled divergences by FIRST-DIVERGENCE
    // signature + offset magnitude band and report DISTINCT CAUSES, ranked by how many sites each
    // explains, NOT failing sites. If 40 sites drift from one `<header>` delta the board must say
    // ONE bug — that is what separates a saturated near-miss from an amplified one. Reuses
    // `oracle::cluster` verbatim (the same clustering the differential crawl uses), so the exit gate
    // and the crawl can never disagree on what a root cause is (FIDELITY-SCORING-REDESIGN.md §3b).
    if !all_divs.is_empty() {
        let clusters = manuk_wpt::oracle::cluster(&all_divs);
        eprintln!(
            "\n  === G1 ROOT CAUSES (§3b) — {} distinct cause(s) across {} divergence(s); ranked by sites explained ===",
            clusters.len(),
            all_divs.len()
        );
        for (rank, c) in clusters.iter().take(12).enumerate() {
            // ⚠ The `~Npx` inside the signature is a POWER-OF-TWO BAND (`oracle::mag_band` rounds
            // down), so every geometry cluster's headline number is a power of two by construction.
            // At t551 that printer made me record "the deltas are QUANTISED — 8/16/32/64/128 — one
            // systematic cause" in the roadmap anchor. They were quantised by the PRINTER. The real
            // median travels alongside now, so the band cannot be mistaken for the measurement again.
            eprintln!(
                "    {:>2} site(s) · {:>4} hit(s)   {}{}",
                c.sites,
                c.hits,
                c.signature,
                if c.median_mag > 0 {
                    format!(
                        "   [median {}px — the ~Npx above is a power-of-2 BAND]",
                        c.median_mag
                    )
                } else {
                    String::new()
                }
            );
            // ── AND ONE INSTANCE, because a cause you cannot open is not actionable (tick 553).
            //
            // `Cluster.examples` has carried `site#path: chrome vs manuk` since the clustering landed and
            // this printer never showed it, so the ledger could say *"`<a>` width is wrong on 18 sites"*
            // and give a reader no way to look at a single one. Chasing the t552 text-measurement lead
            // began by wanting exactly this line and not having it. One example opens the door; three
            // would bury the sites-explained ranking the ordering exists to give.
            // THREE instances for the leading causes, one for the tail. t554 left the ranking a real
            // cause list and immediately hit the next limit: ONE instance cannot tell you whether a
            // cluster is homogeneous. `mis-sized: width ~8px (<a>)` spans three sites and 138 hits, and
            // whether those are TEXT anchors (a shaping/metrics question) or ICON anchors (an
            // intrinsic-sizing question) decides which subsystem the next tick touches. One example is a
            // door; three are a sample.
            let show = if rank < 4 { 3 } else { 1 };
            for ex in c.examples.iter().take(show) {
                eprintln!("        e.g. {ex}");
            }
        }
    }

    // The last site's row, and the in-flight marker it no longer needs. Everything before it is
    // already durable — this is the tail of the same invariant, not a second mechanism.
    flush(&rows, &mut flushed);

    let ok = manuk_wpt::fidelity::report(&rows, floor);
    // The certificate, computed by the thing that measured it. Printed on every run — a two-site G1
    // gate run says "2 sites" and is obviously not a corpus read, which is better than a headline that
    // only appears when someone remembers to ask for it.
    //
    // **Collapsed first, and that is not a formality.** `report` above is per-site diagnostics and
    // wants every draw; the CERTIFICATE is a fixed denominator and a repeated site is still ONE
    // site. The first live run of the repeat plan printed `sites 4` for a two-site corpus — the
    // change meant to make the numerator honest broke the denominator in the same breath, and it
    // was the accounting that said so. Both certificate paths now go through one collapse.
    manuk_wpt::fidelity::certificate_report(&manuk_wpt::fidelity::collapse_repeats(rows.clone()));
    // (`--rows-out` is written incrementally by `flush` above, so a 265-site sweep split into
    // timeout-isolated chunks — or interrupted by a crash — still yields ONE certificate via
    // `manuk-wpt certificate --rows FILE` instead of 53 stanzas for a human to add up.)
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
                "usage: manuk-wpt boxes (--html FILE [--url URL] | --fetch URL) [--width W] \
                 [--tree ID] [--why ID]"
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
    // `--dump-html FILE` — **the bytes our own net stack received, before a parser or a script has
    // touched them.**
    //
    // It exists because t708 concluded "the origin serves our engine a stripped document" from a
    // comparison of `<head>` CHILD COUNTS — 9 live against 49 from `curl` — and that comparison is
    // not sound: the live number is read AFTER scripts have run and AFTER our CSS loader has
    // consumed the `<link>` elements (it strips them exactly as it strips a `<script>`'s `src`),
    // so it measures the post-JS DOM, not the response. Two different documents and one document
    // rewritten by its own scripts look identical through that lens.
    //
    // The only instrument that can separate them is the raw body, and there was no way to get it.
    // Same lesson the comparison itself violated: a stand-in for the subsystem under test IS the
    // instrument, and `curl` was never our net stack.
    if let Some(dest) = flag(args, "--dump-html") {
        match std::fs::write(dest, html.as_bytes()) {
            Ok(()) => println!("wrote {} bytes of RESPONSE BODY to {dest}", html.len()),
            Err(e) => {
                eprintln!("cannot write {dest}: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
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
    // `--why ID` — **the MISSING BOX question, answered.** The oracle books a divergence as
    // `MISSING BOX` whenever a selector-path Chrome produced is absent from our map, and the map drops
    // any element with no layout rect. That single label covers three completely different bugs:
    //
    //   * the element is **not in the DOM at all** (a parse failure, or script deleted it),
    //   * it is in the DOM with **no computed style** (the cascade never reached it),
    //   * it has a style and still **generated no box** (the layout bug people assume it always is).
    //
    // Those need three different fixes and the cluster registry cannot tell them apart, so every
    // MISSING_BOX investigation starts by re-deriving which one it is. This prints the element's
    // ancestor chain with `display`, box and element-child count at each level, so the answer is one
    // command. Chasing `C3833` it read `main_MF: NOT IN THE DOM AT ALL` — which moved the whole
    // cluster from "we lay it out wrong" to "script deleted it", in one run.
    if let Some(want) = flag(args, "--why") {
        let dom = page.dom();
        let styles = page.styles_map();
        let rects = page.root_box.node_rects(dom);
        // An id OR a full CSS selector. Ids are convenient when a page has them, and the tick-267
        // finding is that the modern ones do not: an id-keyed probe cannot address a single element
        // on a React/Astro/Tailwind page, which is exactly the class this instrument is for.
        let matches: Vec<manuk_dom::NodeId> = {
            let by_id: Vec<_> = dom
                .descendants(dom.root())
                .filter(|&n| dom.is_element(n))
                .filter(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(want))
                .collect();
            if by_id.is_empty() {
                manuk_css::query_selector_all(dom, dom.root(), want)
            } else {
                by_id
            }
        };
        if matches.is_empty() {
            println!("{want}: NOT IN THE DOM AT ALL");
        }
        // Every match, because "the first one is fine and the other four are at the viewport origin"
        // is itself the finding — a single hit would have hidden it. The cap is 32 rather than a
        // handful for the same reason: a probe that silently truncates its own answer is how a
        // partial list gets read as a complete one (it was 8, and a 23-element display sweep quietly
        // came back with 8 rows).
        const WHY_CAP: usize = 32;
        if matches.len() > WHY_CAP {
            println!(
                "(showing {WHY_CAP} of {} matches — TRUNCATED)",
                matches.len()
            );
        }
        for (mi, &n) in matches.iter().enumerate().take(WHY_CAP) {
            println!("── match {} of {}", mi + 1, matches.len());
            {
                let mut cur = Some(n);
                let mut chain = Vec::new();
                while let Some(c) = cur {
                    if !dom.is_element(c) {
                        cur = dom.parent(c);
                        continue;
                    }
                    let tag = dom.tag_name(c).unwrap_or("?");
                    let id = dom.element(c).and_then(|e| e.attr("id")).unwrap_or("");
                    // `position` travels with `display`, because the two questions this probe gets
                    // asked are "why is there no box" and "why is the box somewhere else", and the
                    // second one is almost always a containing-block answer.
                    let d = styles
                        .get(&c)
                        .map(|s| format!("{:?} position={:?}", s.display, s.position))
                        .unwrap_or_else(|| "(NO COMPUTED STYLE)".into());
                    let bx = rects
                        .get(&c)
                        .map(|r| format!("[{:.0} {:.0} {:.0}x{:.0}]", r.x, r.y, r.width, r.height))
                        .unwrap_or_else(|| "(NO BOX)".into());
                    let nkids = dom.children(c).filter(|&k| dom.is_element(k)).count();
                    chain.push(format!("{tag}#{id} display={d} box={bx} elem_kids={nkids}"));
                    cur = dom.parent(c);
                }
                for (i, l) in chain.iter().rev().enumerate() {
                    println!("{:width$}{l}", "", width = i * 2);
                }
            }
        }
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
            .map(|(id, (tag, display, rect))| {
                // The crawl-side producer does not carry the computed font yet (the exit gate got it
                // first at t563). Empty rather than fabricated: `Seen::font` documents that an empty
                // string prints as ABSENCE, so a crawl instance simply says nothing about the font
                // instead of implying one was measured.
                (
                    id,
                    Seen {
                        tag,
                        display,
                        rect,
                        font: String::new(),
                    },
                )
            })
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
        // Selector-path keying (tick 399 spec) — via the shared free functions `path_of`/`sig_of`
        // (defined next to `fnv`). The class signature is the identity change that matters: a
        // positional counterpart with a different class list now FAILS the key lookup and books
        // as missing+extra (tree drift, which it is) instead of minting a phantom style diff
        // between two unrelated elements (tick 395: okta's 316 "display diffs" were exactly this).
        // This mirrors the JS `sigOf`/`pathOf` in chrome.rs BYTE-IDENTICALLY, and — since tick 532 —
        // the G1 fidelity probe uses the SAME two functions, so the oracle and the exit gate can
        // never drift apart on what a key means. Two engines agreeing on a naming scheme is a
        // precondition for the diff meaning anything.
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
                let id = path_of(dom, n)?;
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
                Some((
                    id,
                    Seen {
                        tag,
                        display,
                        rect,
                        font: String::new(),
                    },
                ))
            })
            .collect();

        // ── **THE SAME CORRECTION THE EXIT GATE GOT AT t549, OWED HERE EVER SINCE.**
        //
        // t549 measured what the class signature costs and turned it OFF for the G1 fidelity probe:
        // `gov.uk` coverage **0.0% → 82.8%** (418 paths: 418 missing → 72), `stripe.com` **0.1% →
        // 43.1%**. Its comment ends by naming this exact site as the remaining half — *"`run_oracle_cmd`'s
        // crawl keys still carry sigs — the same correction is owed there, as its own tick."* This is
        // that tick.
        //
        // ⚠ **The cost is not a slightly noisier ledger — it is the ledger's #1 ROW.** `nth-child`
        // already distinguishes siblings uniquely, so the sig never carried identity, only fragility:
        // ONE ancestor whose class list differs between the engines (a script that adds `js`/`loaded`,
        // a framework hydration class, a viewport-dependent modifier) re-keys **every descendant**, and
        // the diff books the entire subtree as `missing box`. Measured here on `www.heart.org`: the
        // shallowest "missing" element was **`<body>` itself** — `block [0 0 1200×5993] vs (no box)` —
        // for a page we plainly render. From there the whole document is missing by construction.
        //
        // That is why `missing box: <div>` / `<a>` / `<span>` sat at the top of the mechanism ledger
        // that ranks this project's work, and it is a measurement artefact of the instrument's own key.
        //
        // `MANUK_ORACLE_CLASS_SIG=1` restores the old keys, matching `MANUK_G1_CLASS_SIG` on the gate
        // side, so the decision stays auditable and reversible rather than becoming folklore.
        let (chrome, manuk) = if std::env::var_os("MANUK_ORACLE_CLASS_SIG").is_some() {
            (chrome, manuk)
        } else {
            (strip_sigs(chrome), strip_sigs(manuk))
        };
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
            // ⚠⚠⚠ **This line used to drop `delta`**, and `delta` is the only field that separates a
            // wrong WIDTH from a wrong HEIGHT from a pure displacement. Every geometry divergence in
            // the corpus therefore reached `oracle-merge` unkeyable, and the ledger the loop ranks by
            // fell back to the tag. The emitter and its reader now live together in `oracle.rs` so the
            // pair cannot drift again — see `oracle::div_to_jsonl`.
            for d in &divs {
                out.push_str(&manuk_wpt::oracle::div_to_jsonl(
                    d,
                    flag(args, "--class").unwrap_or("?"),
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

// The JSON escaper that used to live here is gone: it was the WRITER's half of a pair whose reader
// lived in `oracle.rs`, and a pair split across two files is the drift this tick exists to close.
// Both halves are now `oracle::div_to_jsonl` / `oracle::div_from_jsonl`, beside each other, with a
// round-trip test between them.

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
        D::FlowRoot => "flow-root",
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

/// Selector-path keying — ONE Rust definition, shared by the differential oracle (`run_oracle_cmd`)
/// AND the G1 fidelity probe (`run_fidelity_cmd`). `sig_of` is the class SIGNATURE: fnv1a-32 over the
/// ASCII-lowercased SORTED deduped class list joined with '.'; a classless element emits the empty
/// string. The hash runs over **UTF-16 code units** (`encode_utf16`) because that is what the JS
/// `charCodeAt` yields — this mirrors `sigOf` in chrome.rs BYTE-IDENTICALLY, which is a precondition
/// for the diff meaning anything (a signature computed two ways is two different keys).
fn sig_of(dom: &manuk_dom::Dom, n: manuk_dom::NodeId) -> String {
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
}

/// Strip every `.SIG` component from a keyed map — the class-signature ABLATION (`MANUK_G1_NO_SIG=1`).
///
/// A sig is exactly `.` + 8 lowercase hex digits, and it always sits immediately before `:nth-child(`,
/// so the removal is unambiguous and needs no regex crate. Applied to BOTH sides identically, so the
/// comparison stays a comparison; applied to `Seen` maps rather than at the producers, so Chromium's
/// injected JS (a byte-identical contract with `sig_of`) is left untouched.
///
/// Collisions are possible in principle — two same-tag siblings differing only by class cannot happen,
/// because `nth-child` already distinguishes siblings. Where a collision WOULD occur the later entry
/// wins, which can only LOSE elements from each side, never invent matches; so a coverage number that
/// rises under ablation is a real signal, not an artifact of the ablation.
fn strip_sigs(
    m: std::collections::HashMap<String, manuk_wpt::oracle::Seen>,
) -> std::collections::HashMap<String, manuk_wpt::oracle::Seen> {
    m.into_iter()
        .map(|(k, v)| {
            let mut out = String::with_capacity(k.len());
            let mut rest = k.as_str();
            while let Some(i) = rest.find(":nth-child(") {
                let (head, tail) = rest.split_at(i);
                // A sig, if present, is the final 9 bytes of `head`: `.` + 8 hex digits.
                //
                // ⚠⚠ **`is_char_boundary` is the guard, not a micro-optimisation.** A sig is nine
                // ASCII bytes, so where one exists `len - 9` is always a char boundary — but a class
                // name is arbitrary author text, and a page served with a broken charset produces
                // keys full of multi-byte replacement characters. `swift.org` did exactly that, and
                // `&head[head.len() - 9..]` landed inside a two-byte `'\u{fffd}'` and **panicked the
                // whole sweep process**. The site was then recorded as `reason=crashed`, which reads
                // as a browser Bar-0 event: *the instrument charged its own panic to the engine.*
                let keep = match head.len().checked_sub(9) {
                    Some(cut) if head.is_char_boundary(cut) => {
                        let cand = &head[cut..];
                        if cand.starts_with('.')
                            && cand[1..]
                                .bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                        {
                            &head[..cut]
                        } else {
                            head
                        }
                    }
                    _ => head,
                };
                out.push_str(keep);
                // Copy `:nth-child(N)` verbatim, then continue after it.
                let close = tail.find(')').map(|c| c + 1).unwrap_or(tail.len());
                out.push_str(&tail[..close]);
                rest = &tail[close..];
            }
            out.push_str(rest);
            (out, v)
        })
        .collect()
}

/// `tag.SIG:nth-child(N)/…` from the root — N 1-based over ALL element siblings — mirroring the JS
/// `pathOf` in chrome.rs. An element whose parent is NOT an element (i.e. `<html>`, whose parent is
/// the document) contributes NO component, because the JS `e.parentElement` is null there and its
/// loop never runs for it. Emitting a root component on our side once shifted every key by one level
/// and reported `<html>` MISSING on every site, with total confidence — so this asymmetry is load-
/// bearing, not an oversight.
fn path_of(dom: &manuk_dom::Dom, n: manuk_dom::NodeId) -> Option<String> {
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
        parts.push(format!("{tag}{}:nth-child({i})", sig_of(dom, cur)));
        cur = parent;
    }
    parts.reverse();
    Some(parts.join("/"))
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

    // signature -> (sites, classes, hits, example, dominant-axis magnitudes)
    #[allow(clippy::type_complexity)]
    let mut acc: BTreeMap<
        String,
        (BTreeSet<String>, BTreeSet<String>, usize, String, Vec<i64>),
    > = BTreeMap::new();
    let (mut ok, mut hang, mut fail, mut discard) = (0usize, 0usize, 0usize, 0usize);
    // **Divergence records this merge REFUSED because they carried no `delta`.** A pre-t744 crawl
    // directory has none, and re-keying those to a fabricated `[0,0,0,0]` would fill the ledger with
    // rows that read `geometry/displaced: x (horizontal) ~0px` and look measured. Counted and
    // reported instead — an old crawl must announce itself, not quietly produce a plausible ledger.
    let mut unkeyable = 0usize;
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
            // ⚠⚠⚠ **THE LEDGER'S KEY IS NO LONGER COMPUTED HERE.** It used to be, and its geometry arm
            // was the single line `format!("geometry: <{tag}>")` — while `oracle::cluster()`, on the
            // in-process path, already built the full `{displaced|mis-sized}: {axis} ~{band}px`
            // mechanism key. Same rule, two implementations, and *this* one — the one that writes
            // `docs/loop/CLUSTERS.md`, the file the board ranks the whole loop's work by — was the
            // poorer of the two for 351 ticks. It now calls the shared definition.
            let Some((d, class)) = manuk_wpt::oracle::div_from_jsonl(line) else {
                unkeyable += 1;
                continue;
            };
            // `missing` keeps the merge's own longer wording (the registry has always spelled out
            // what it means, and the cluster IDs of 401 sites' worth of rows hang off that string).
            let sig = if d.kind == "missing" {
                format!(
                    "MISSING BOX: <{}>  (Chrome renders it, we render nothing)",
                    d.tag
                )
            } else {
                manuk_wpt::oracle::signature_of(&d)
            };
            let en = acc.entry(sig).or_insert_with(|| {
                (
                    BTreeSet::new(),
                    BTreeSet::new(),
                    0,
                    format!("{}#{}: {} vs {}", d.site, d.id, d.chrome, d.manuk),
                    Vec::new(),
                )
            });
            en.0.insert(d.site);
            en.1.insert(class);
            en.2 += 1;
            // The RAW dominant-axis magnitude, kept because `~Npx` in the signature is a power-of-two
            // BAND and a reader cannot tell that by looking. t551 read "the deltas are QUANTISED —
            // 8/16/32/64/128 — the signature of ONE systematic box-model delta" off exactly that, and
            // the quantisation was the PRINTER. Both numbers travel now, on this path too.
            if d.kind == "geometry" {
                en.4.push(manuk_wpt::oracle::dominant_axis(d.delta).1.abs());
            }
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
    // **An old-format crawl must say so out loud.** Pre-t744 JSONL carries no `delta`, so its geometry
    // records are unkeyable — and the only wrong answer available here is a QUIET one: default the
    // delta to zero, key everything as `~0px`, and print a ledger that looks measured. Refused and
    // counted instead, with the remedy named, because a ledger is only worth its provenance.
    if unkeyable > 0 {
        println!(
            "  \x1b[31m{unkeyable:>4} divergence records REFUSED\x1b[0m — no `delta` field, so no \
             mechanism key.\n       This crawl directory predates the mechanism oracle (t744). Its \
             geometry rows are NOT\n       in the ledger below and were not re-keyed to a fabricated \
             zero. Re-crawl to rank by primitive."
        );
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
        "# ORACLE CLUSTER REGISTRY — generated by `manuk-wpt oracle-merge`. Do not hand-edit.\n         #\n         # This IS the priority ledger (Part 4) and the website-class taxonomy (Part 24.1). Ranked by\n         # DISTINCT SITES then DISTINCT CLASSES, never by hit count. A pattern class that CRASHES or\n         # HANGS outranks every visual divergence here (Part 24.3) — those live in STATUS.md's Bar 0.\n         #\n         # A geometry row names a MECHANISM, not a tag (t744): {displaced|mis-sized} says whether the\n         # box is the wrong SIZE or in the wrong PLACE, the axis says which of the four numbers is\n         # wrong, `~Npx` is the power-of-two BAND, and `med=` is the REAL median of that axis — the\n         # band is a printer artifact and must never be read as the measurement (t552).\n         #\n         # id            sites  classes  hits  root cause\n",
    );
    println!("\n──── ROOT CAUSES, ranked by SITES EXPLAINED — this IS the ledger ────\n");
    println!(
        "{:>6} {:>8} {:>7}  {:<10} {}",
        "sites", "classes", "hits", "cluster", "root cause"
    );
    for (i, (sig, (sites, classes, hits, example, mags))) in ranked.iter().enumerate() {
        // Stable, human-quotable, and derived from the signature so the same root cause keeps the
        // same id across crawls even as its rank moves.
        let id = format!("C{:04x}", fnv(sig) & 0xffff);
        // The REAL median dominant-axis delta, beside the band the signature quotes.
        let med = if mags.is_empty() {
            String::new()
        } else {
            let mut m = mags.clone();
            m.sort_unstable();
            format!("   med={}px", m[m.len() / 2])
        };
        registry.push_str(&format!(
            "{id:<14} {:>5} {:>8} {:>5}  {sig}{med}\n",
            sites.len(),
            classes.len(),
            hits
        ));
        if i < 30 {
            println!(
                "{:>6} {:>8} {:>7}  {id:<10} {sig}{med}",
                sites.len(),
                classes.len(),
                hits
            );
            println!("{:>26}e.g. {example}", "");
        }
    }
    // **`--no-registry` exists because this write is a FOOTGUN, and it fired during this tick's own
    // development.** `oracle-merge <dir>` unconditionally overwrote `docs/loop/CLUSTERS.md` — the
    // 265-site priority ledger — from whatever directory it was pointed at, so verifying the merge on
    // a two-site test crawl would have replaced the corpus ledger with a two-site one. The default is
    // unchanged (the observer's `oracle-crawl.sh` calls it with no flags and must keep working); this
    // only gives a small ad-hoc run a way to say *"I am not the corpus."*
    //
    // It is a BARE flag, so it is matched by `any`, never by `flag()` — `flag()` returns the arg
    // AFTER the name, so asking it about a boolean either reads the next token as a value or, at the
    // end of the line, answers "absent" for a flag that is plainly present.
    if args.iter().any(|a| a == "--no-registry") {
        println!(
            "\n  \x1b[33m--no-registry\x1b[0m: docs/loop/CLUSTERS.md NOT written ({} clusters from \
             {ok} site(s) — too few to be the ledger)",
            ranked.len()
        );
    } else {
        let _ = std::fs::write("docs/loop/CLUSTERS.md", &registry);
        println!(
            "\n  cluster registry → docs/loop/CLUSTERS.md ({} clusters from {ok} diffed site(s))",
            ranked.len()
        );
    }
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

/// This process's resident set, human-readable — printed alongside the progress line because the
/// first full run died of memory, not of a test, and a runner that cannot see its own footprint
/// reports that as "it hung".
fn rss_human() -> String {
    let kb = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|v| {
                    v.split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<u64>().ok())
                })
        })
        .unwrap_or(0);
    format!("{:.1}GB", kb as f64 / 1024.0 / 1024.0)
}

/// `manuk-wpt test262 [--root DIR] [--limit N] [--dir PREFIX] [--out TSV]` — the ECMAScript
/// conformance suite against the engine we ship (tick 546).
///
/// The suite is not vendored: it is a large third-party checkout that would dominate the repo, so
/// the runner takes a path (`--root`, `$MANUK_TEST262`, or one of the usual local checkouts) and
/// says so plainly when it cannot find one. **It refuses to print a number without a JS engine** —
/// `NoScriptRuntime::eval` returns `Ok(undefined)` for every input, so a no-JS build would score a
/// flawless 100% on a suite it never executed, which is the most dangerous shape a metric takes.
fn run_test262_cmd(args: &[String]) {
    use manuk_wpt::test262::{self, Tally, Verdict};

    let mut root = String::new();
    let mut limit = 0usize;
    let mut prefix = String::new();
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                root = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--limit" => {
                limit = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 2;
            }
            "--dir" => {
                prefix = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--out" => {
                out = args.get(i + 1).cloned();
                i += 2;
            }
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(2);
            }
        }
    }
    if root.is_empty() {
        root = std::env::var("MANUK_TEST262").unwrap_or_default();
    }
    let root = if root.is_empty() {
        // The checkouts that actually exist on a dev box, in preference order. Named rather than
        // guessed at silently, so "no number" always comes with "and here is where I looked".
        let candidates = ["WebKit/JSTests/test262", "test262", "../test262"];
        match candidates
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.join("test").is_dir())
        {
            Some(p) => p,
            None => {
                eprintln!(
                    "✗ no test262 checkout found. Looked in: {}. \
                     Pass --root DIR or set MANUK_TEST262 (git clone https://github.com/tc39/test262).",
                    candidates.join(", ")
                );
                std::process::exit(2);
            }
        }
    } else {
        std::path::PathBuf::from(root)
    };

    let mut rt = manuk_js::new_runtime();
    if !rt.engine_name().starts_with("SpiderMonkey") {
        eprintln!(
            "✗ no JS engine in this build ({}) — REFUSING to report a conformance number. \
             A no-JS runtime returns undefined for every evaluation and would score 100% on a suite \
             it never ran. Rebuild with --features spidermonkey.",
            rt.engine_name()
        );
        std::process::exit(2);
    }

    let revision = std::fs::read_to_string(root.join("test262-Revision.txt"))
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("test262 revision:")
                    .map(|r| r.trim().to_string())
            })
        })
        .unwrap_or_else(|| "(unknown)".to_string());

    let (mut files, fixtures, staging) = test262::discover(&root);
    let discovered = files.len();
    if !prefix.is_empty() {
        files.retain(|p| p.to_string_lossy().contains(&prefix));
    }
    let selected_before_limit = files.len();
    let files = test262::stride_sample(files, limit);

    println!("TEST262 — {} @ {revision}", root.display());
    println!(
        "  engine: {}  ·  files discovered {discovered}  ·  excluded {fixtures} fixtures + {staging} staging",
        rt.engine_name()
    );
    if limit > 0 && files.len() < selected_before_limit {
        println!(
            "  ⚠ SAMPLE: {} of {selected_before_limit} files, strided evenly across the suite — this is a \
             SAMPLE and its number must be quoted as one.",
            files.len()
        );
    }

    let harness_dir = root.join("harness");
    let mut cache: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut tally = Tally::default();
    let mut rows = String::new();
    let started = std::time::Instant::now();

    for (n, path) in files.iter().enumerate() {
        let rel = path
            .strip_prefix(root.join("test"))
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        // Lossy on purpose: a handful of cases are deliberately not valid UTF-8, and dropping them
        // silently would be an unnamed skip. They run, and if the replacement character changes the
        // verdict that shows up as an ordinary failure rather than a hole in the denominator.
        let body = String::from_utf8_lossy(&bytes).to_string();
        tally.files += 1;

        // A crash takes the process with it and prints nothing, so the case that killed the run is
        // otherwise unknowable. `MANUK_T262_TRACE=1` names every case BEFORE it runs: the last line
        // on stderr is the culprit. (This is how the missing `JSAutoRealm` in the runtime's
        // exception path was found — it aborted, it did not return an error.)
        if std::env::var_os("MANUK_T262_TRACE").is_some() {
            eprintln!("  → {rel}");
        }
        let fm = test262::parse_frontmatter(&body);
        let ms = test262::modes(&fm);
        if let Some(reason) = test262::skip_reason(&fm, &body) {
            tally.skip(reason, ms.len());
            continue;
        }
        if test262::is_slow(&rel) {
            tally.skip(test262::SKIP_SLOW, ms.len());
            continue;
        }

        for mode in ms {
            let mut load = |name: &str| -> Option<String> {
                cache
                    .entry(name.to_string())
                    .or_insert_with(|| std::fs::read_to_string(harness_dir.join(name)).ok())
                    .clone()
            };
            let Some(src) = test262::assemble(&fm, &body, mode, &mut load) else {
                // A harness file we could not read is OUR gap, not the engine's.
                tally.skip("harness file missing from the checkout", 1);
                continue;
            };
            let outcome = match rt.eval(&src, &rel) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.message),
            };
            match test262::verdict(&fm, outcome) {
                Verdict::Pass => tally.passed += 1,
                Verdict::Fail(why) => {
                    tally.record_fail(&rel, &why);
                    if out.is_some() {
                        rows.push_str(&format!("{rel}\t{}\tFAIL\t{why}\n", mode.label()));
                    }
                }
            }
        }
        // ── COLLECT, or the run eats the machine. Each `eval` is a fresh global+realm (the isolation
        // that makes one test's damage invisible to the next), and SpiderMonkey's incremental GC never
        // gets a chance against a loop that only ever evaluates. Measured before this line existed:
        // **14.7 GB RSS at 14,000 of 51,922 files**, still climbing, on a 31 GB box — the run does not
        // finish, it gets OOM-killed, and a conformance number that cannot survive its own suite is not
        // a number. Every 250 files is far below the growth rate and costs a few seconds total; peak RSS at 500 was still
        // 5.3 GB, which is fine on this box and would not be on a smaller one.
        if n % 250 == 249 {
            rt.gc();
        }
        if n % 2000 == 1999 {
            eprintln!(
                "  … {n}/{} files · {} passed · {} failed · {:.0}s · rss {}",
                files.len(),
                tally.passed,
                tally.failed,
                started.elapsed().as_secs_f64(),
                rss_human()
            );
        }
    }

    if let Some(p) = &out {
        let _ = std::fs::write(p, &rows);
    }

    println!(
        "\n  subtests EXECUTED {}  ·  passed {}  ·  failed {}",
        tally.executed(),
        tally.passed,
        tally.failed
    );
    println!("  PASS (of executed):  {:.2}%", tally.pass_pct_executed());
    println!(
        "  PASS (of defined):   {:.2}%   ← the honest number: every skip counted as NOT passed",
        tally.pass_pct_defined()
    );
    println!("  skipped {} —", tally.skipped_total());
    for (reason, n) in &tally.skipped {
        println!("      {n:>7}  {reason}");
    }
    if !tally.fail_by_area.is_empty() {
        let mut v: Vec<_> = tally.fail_by_area.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        println!(
            "\n  FAILURES by area (a cause is area-shaped; 4,000 failures are never 4,000 bugs):"
        );
        for (area, n) in v.iter().take(15) {
            println!("      {n:>7}  {area}");
        }
        let mut t: Vec<_> = tally.fail_by_type.iter().collect();
        t.sort_by(|a, b| b.1.cmp(a.1));
        println!("  FAILURES by thrown type:");
        for (ty, n) in t.iter().take(8) {
            println!("      {n:>7}  {ty}");
        }
        println!("  samples:");
        for s in tally.samples.iter().take(12) {
            println!("      {s}");
        }
    }
    println!("\n  wall {:.0}s", started.elapsed().as_secs_f64());
    // ADR-009: the thread that ran SpiderMonkey ends SpiderMonkey, or the process segfaults on the
    // way out with every byte of output already correct.
    drop(rt);
    manuk_js::shutdown();
}

#[cfg(test)]
mod path_key_tests {
    //! Brick 2 of the fidelity-instrument rebuild (tick 532): the SELECTOR-PATH producer that the
    //! G1 SHAPE gate needs. These pin the two invariants a `/`-keyed producer must hold, each with a
    //! demonstrated way to go red — the same discipline the `shape_stats` gate (fidelity.rs) already
    //! honours for the SCORER. Without a slash-keyed key, `shape_stats::common_frame` finds no shared
    //! ancestor and silently degrades to absolute placement (the misleading 4.5% the redesign exists
    //! to kill), so "the key carries `/`-ancestry" is the load-bearing property, not a nicety.
    use super::{path_of, sig_of, strip_sigs};

    // Independent fnv1a-32 over UTF-16 code units — the byte-identical contract with chrome.rs's JS
    // `sigOf`. Written separately from `sig_of` so the assertion is a real cross-check, not a
    // tautology: two implementations of the hash must agree, or the oracle and the exit gate would
    // key the same page differently and compare strangers.
    fn ref_sig(classes: &[&str]) -> String {
        let mut toks: Vec<String> = classes.iter().map(|c| c.to_ascii_lowercase()).collect();
        toks.sort();
        toks.dedup();
        let joined = toks.join(".");
        let mut h: u32 = 0x811c9dc5;
        for u in joined.encode_utf16() {
            h ^= u32::from(u);
            h = h.wrapping_mul(0x0100_0193);
        }
        format!(".{h:08x}")
    }

    fn find_tag(d: &manuk_dom::Dom, tag: &str) -> Vec<manuk_dom::NodeId> {
        d.descendants(d.root())
            .filter(|&n| d.is_element(n) && d.tag_name(n) == Some(tag))
            .collect()
    }

    #[test]
    fn path_is_slash_keyed_and_excludes_html() {
        let d = manuk_html::parse(
            r#"<html><body><div class="b a"><span>x</span><span>y</span></div></body></html>"#,
        );
        let div = find_tag(&d, "div")[0];
        let spans = find_tag(&d, "span");
        let pdiv = path_of(&d, div).unwrap();
        // Brick-2 CORE: the key carries `/`-ancestry (SHAPE cannot cancel a page offset without it)
        // and never a root `html` term (the off-by-one-level bug the comment warns about).
        assert!(pdiv.contains('/'), "path must be slash-keyed, got {pdiv}");
        assert!(
            !pdiv.split('/').any(|c| c.starts_with("html")),
            "html must not appear as a component: {pdiv}"
        );
        // `body` is nth-child(2): the parser inserts an implicit `<head>` as html's first element
        // child, and nth-child counts ALL element siblings — exactly as Chrome's `pathOf` does via
        // `previousElementSibling` (head is counted positionally though it is never emitted as a key).
        assert_eq!(
            pdiv,
            format!("body:nth-child(2)/div{}:nth-child(1)", ref_sig(&["a", "b"]))
        );
        // nth-child counts element siblings 1-based; a classless element emits NO `.SIG` component.
        let p2 = path_of(&d, spans[1]).unwrap();
        assert!(p2.ends_with("/span:nth-child(2)"), "got {p2}");
    }

    #[test]
    fn sig_is_order_and_dup_independent_and_matches_the_js_hash() {
        let d = manuk_html::parse(
            r#"<html><body><i class="  z   a a "></i><i class="a z"></i><p>x</p></body></html>"#,
        );
        let is = find_tag(&d, "i");
        // Class order and duplicates must not change the signature (the sorted-deduped contract).
        assert_eq!(sig_of(&d, is[0]), sig_of(&d, is[1]));
        // And it must equal the independent reference hash — the JS/Rust byte-identical contract.
        assert_eq!(sig_of(&d, is[0]), ref_sig(&["a", "z"]));
        // A classless element has the empty signature (so its key is `p:nth-child(N)`, no dot).
        let p = find_tag(&d, "p")[0];
        assert_eq!(sig_of(&d, p), "");
    }

    /// **Chrome's key for every real page's body is `body:nth-child(2)` — because `<head>` is the
    /// first element child of `<html>`. If OUR producer disagrees, EVERY key on the page mismatches at
    /// its root component and the site reports ~100% MISSING.**
    ///
    /// This is not hypothetical: the t549 sweep found 13 of 54 sites under 5% coverage, several of them
    /// with the exact signature of a root-component mismatch — `gov.uk` 418 of 418 missing in 6.8s (no
    /// timeout excuse), `nytimes.com` 2,406 of 2,407 missing with the leading component of every missing
    /// path printed as `body:nth-child(2)`. A keying artifact and a rendering failure look identical in
    /// the coverage number, so the key has to be pinned against the shape a real document has.
    #[test]
    fn the_body_key_matches_chromes_because_head_is_the_first_element_child() {
        let d = manuk_html::parse(
            r#"<!doctype html><html><head><title>t</title></head><body class="js-enabled"><div id="a"></div></body></html>"#,
        );
        let body = find_tag(&d, "body");
        assert_eq!(body.len(), 1, "one body");
        let p = path_of(&d, body[0]).expect("body has a path");
        assert!(
            p.starts_with("body"),
            "the path is rooted at body (html's parent is the document, not an element): got {p}"
        );
        assert!(
            p.ends_with(":nth-child(2)"),
            "body must be the SECOND element child of <html> — <head> is the first, and Chrome keys it \
             that way. If this is nth-child(1) then every descendant key on every real page mismatches \
             at its root and the site reports ~100% MISSING: got {p}"
        );
        // And a descendant's full path must carry that same root component, or the mismatch is one
        // level down instead of at the root.
        let div = find_tag(&d, "div");
        let dp = path_of(&d, div[0]).expect("div has a path");
        assert!(
            dp.starts_with("body.") || dp.starts_with("body:"),
            "descendant paths are rooted at the body component: got {dp}"
        );
        assert!(
            dp.contains(":nth-child(2)/"),
            "…and it is body's nth-child(2) that leads: got {dp}"
        );
    }

    /// The class-signature ABLATION must strip sigs and NOTHING else — an off-by-one here would
    /// silently corrupt every key on both sides and produce a coverage number out of thin air.
    #[test]
    fn strip_sigs_removes_only_the_signature_component() {
        use manuk_wpt::oracle::Seen;
        let seen = |t: &str| Seen {
            tag: t.into(),
            display: "block".into(),
            rect: [0, 0, 1, 1],
            // `font` was added to `Seen` at t563 so a geometry divergence could name the FACE that
            // produced it; this constructor was missed, and `cargo test -p manuk-wpt` has not
            // compiled since. It is the `paint-enum-field-breaks-wpt-bench` shape exactly — a struct
            // field added, one test constructor missed, and the breakage invisible because the wall
            // does not build this crate's tests. Empty is the legitimate "no font worth reporting".
            font: String::new(),
        };
        let mut m = std::collections::HashMap::new();
        m.insert(
            "body.ba1d8e99:nth-child(2)/div.74146c68:nth-child(6)".to_string(),
            seen("div"),
        );
        m.insert("body:nth-child(2)/a:nth-child(3)".to_string(), seen("a"));
        // A tag that ends in 9 hex-ish chars must NOT be truncated: only a leading `.` marks a sig.
        m.insert("abcdef012:nth-child(1)".to_string(), seen("abcdef012"));
        let out = strip_sigs(m);
        assert!(
            out.contains_key("body:nth-child(2)/div:nth-child(6)"),
            "sigs gone: {:?}",
            out.keys()
        );
        assert!(
            out.contains_key("body:nth-child(2)/a:nth-child(3)"),
            "a sig-less key is unchanged"
        );
        assert!(
            out.contains_key("abcdef012:nth-child(1)"),
            "9 hex chars with NO leading dot is a tag name, not a signature: {:?}",
            out.keys()
        );
        assert_eq!(out.len(), 3, "no key collapsed into another");
        // The values travel with their keys.
        assert_eq!(out["body:nth-child(2)/div:nth-child(6)"].tag, "div");
    }

    /// **ONE ANCESTOR'S CLASS LIST RE-KEYS THE WHOLE SUBTREE, AND THE DIFF BOOKS IT ALL AS MISSING.**
    ///
    /// `nth-child` already identifies a sibling uniquely, so the 8-hex class signature in a path key
    /// never carried identity — only fragility. When a single ancestor's class list differs between the
    /// engines (a script adding `js`/`loaded`, a hydration class, a viewport modifier), **every
    /// descendant key changes**, nothing matches, and the entire subtree is reported `missing box`.
    ///
    /// t549 measured this for the G1 exit gate and turned the signature off there — `gov.uk` coverage
    /// **0.0% → 82.8%**, `stripe.com` **0.1% → 43.1%** — and its comment named the remaining half:
    /// *"`run_oracle_cmd`'s crawl keys still carry sigs — the same correction is owed there."* That
    /// matters more than it sounds, because the oracle writes `CLUSTERS.md`, **the ledger this project
    /// ranks its work by**. Measured on `www.heart.org` before the fix, the shallowest "missing"
    /// element was `<body>` itself (`block [0 0 1200×5993] vs (no box)`) for a page we plainly render;
    /// over three CrUX sites the reported divergences fell **2750 → 892** once the keys were stripped.
    ///
    /// This asserts the mechanism directly: two trees that are structurally IDENTICAL and differ only
    /// in one ancestor's class signature must produce **no** divergences once keyed for diffing, and
    /// must produce a full subtree of phantom `missing` ones when they are not.
    ///
    /// RED, run: delete the `strip_sigs` call guarded by `MANUK_ORACLE_CLASS_SIG` in `run_oracle_cmd`
    /// — this test still passes (it tests the composition, not the call site), but
    /// `oracle --urls https://www.heart.org/` reports `missing box: <div>` at 211 hits again and the
    /// shallowest missing element is `<body>`. The call site's guard is the env var, kept auditable.
    #[test]
    fn a_differing_ancestor_class_signature_must_not_book_the_whole_subtree_as_missing() {
        use manuk_wpt::oracle::{diff_page, Seen};
        let seen = |tag: &str| Seen {
            tag: tag.into(),
            display: "block".into(),
            rect: [0, 0, 100, 20],
            font: String::new(),
        };
        // Same document, same geometry. Chrome's <body> carries a class signature; ours does not —
        // which is exactly what a script that sets a class on <body> produces.
        let mk = |body: &str| {
            let mut m = std::collections::HashMap::new();
            m.insert(format!("{body}:nth-child(2)"), seen("body"));
            m.insert(format!("{body}:nth-child(2)/div:nth-child(1)"), seen("div"));
            m.insert(
                format!("{body}:nth-child(2)/div:nth-child(1)/a:nth-child(1)"),
                seen("a"),
            );
            m
        };
        let chrome = mk("body.d6441846");
        let manuk = mk("body");

        // Raw keys: nothing matches, and every element of the tree is a phantom divergence.
        let raw = diff_page("t", &chrome, &manuk, 8);
        assert_eq!(
            raw.len(),
            3,
            "unstripped, one differing ancestor sig must lose the WHOLE subtree — got {raw:?}"
        );
        assert!(
            raw.iter().all(|d| d.kind == "missing"),
            "and it must lose them as `missing`, which is what put `missing box` at the top of the              priority ledger — got {:?}",
            raw.iter().map(|d| d.kind.clone()).collect::<Vec<_>>()
        );

        // Keyed for diffing, the two trees are what they actually are: identical.
        let stripped = diff_page("t", &strip_sigs(chrome), &strip_sigs(manuk), 8);
        assert!(
            stripped.is_empty(),
            "structurally identical trees must produce NO divergences once the signature is \
             stripped — got {stripped:?}"
        );
    }

    /// **A NON-ASCII class name must not panic the sweep.** `strip_sigs` runs on every site by
    /// default (t549 turned the sig off; `MANUK_G1_CLASS_SIG=1` restores it), and it looked back
    /// exactly 9 BYTES from the end of each path component. A sig is nine ASCII bytes, so that index
    /// is a char boundary whenever a sig is present — but a class name is arbitrary author text, and
    /// a page served with a broken charset produces components full of multi-byte replacement
    /// characters.
    ///
    /// `swift.org` did exactly that in the t745 corpus sweep: `byte index 194 is not a char boundary;
    /// it is inside '\u{fffd}'`. The process died, and the site was banked as `reason=crashed` —
    /// which reads as a browser Bar-0 event. **The instrument charged its own panic to the engine**,
    /// in the one file whose whole job is to measure the engine honestly.
    ///
    /// RED, run: replace the `is_char_boundary` guard with the old `head.len() >= 9` test — this
    /// test panics instead of failing, which is the point.
    #[test]
    fn a_multibyte_class_name_does_not_panic_the_sig_stripper() {
        use manuk_wpt::oracle::Seen;
        let seen = || Seen {
            tag: "div".into(),
            display: "block".into(),
            rect: [0, 0, 1, 1],
            font: String::new(),
        };
        // Each component's last 9 bytes straddle a multi-byte char, which is what panicked.
        let mojibake = "body:nth-child(2)/div.\u{fffd}\u{fffd}\u{fffd}\u{fffd}x:nth-child(3)";
        // …and the mirror case: a REAL sig sitting immediately after multi-byte text must still be
        // stripped, or the guard would have traded a panic for a silently unstripped key.
        let after_utf8 = "body:nth-child(2)/div.caf\u{e9}.0a1b2c3d:nth-child(4)";
        let mut m = std::collections::HashMap::new();
        m.insert(mojibake.to_string(), seen());
        m.insert(after_utf8.to_string(), seen());

        let out = strip_sigs(m); // must not panic

        assert!(
            out.contains_key(mojibake),
            "a component whose last 9 bytes are not a sig must pass through UNCHANGED: {:?}",
            out.keys()
        );
        assert!(
            out.contains_key("body:nth-child(2)/div.caf\u{e9}:nth-child(4)"),
            "a real sig after multi-byte text must still be stripped: {:?}",
            out.keys()
        );
    }
}
