//! `manuk-wpt memtabs` — **the standing 100-tab RSS benchmark**, which had been defined
//! for ~350 ticks and never run once (`docs/loop/PHASE0-BOUNDED-REMAINDER.md` row 25b:
//! *"defined, never run; the memory thesis rests on zero data"*).
//!
//! # What the claim actually is, and what would falsify it
//!
//! `docs/loop/PROCESS-MODEL.md` §4 states it precisely, and it is easy to misread:
//!
//! > **"Beat Chrome at 100 tabs" is about how many of those 100 tabs have a live process
//! > AT ALL — not about making a live tab individually smaller.**
//!
//! So the number to watch is **not** per-tab RSS in isolation. It is the shape of the
//! curve as tabs accumulate: a design whose background tabs are evicted should show a
//! **marginal** cost per tab that collapses once the warm-tab budget is exceeded, while
//! a design that merely *labels* tabs as hibernated shows a straight line. Those two are
//! indistinguishable in any single-N measurement, which is exactly why this harness holds
//! at several N rather than reporting one figure.
//!
//! `docs/ENGINEERING-SYNTHESIS.md` names the falsification condition out loud — *within
//! 20% of Chrome means drop the memory claim* — and defers the absolute 2–3 GB budget to
//! Phase 1. **This harness therefore reports and does not grade.** Inventing a pass
//! threshold before the first measurement exists would be fitting the bar to the result.
//!
//! # The two numbers, and why both are printed
//!
//! `manuk_page::Page::estimated_bytes` is a **proxy**: it walks the fragment tree, the DOM
//! and the computed styles. It cannot see the JS heap, decoded images, raster tiles, the
//! font atlas, or the network cache — and on a real page those are most of the cost. The
//! whole per-tab memory accounting in `TabManager` is built on that proxy. Printing it
//! beside the real footprint is the point of this harness: **the ratio between them is the
//! error bar on every per-tab memory number this browser has ever reported.**
//!
//! # Why per-tab RSS is a *marginal* figure and never an absolute one
//!
//! All tabs live in one process (one SpiderMonkey runtime, one realm per page — see
//! `engine/js/src/lib.rs`), so there is no per-tab RSS to read: the OS accounts memory to
//! the process. What is real is the **delta** as each page is added, against a baseline
//! captured after fonts and the JS runtime are already up. Reporting `total_rss / n`
//! instead would silently amortise the fixed process floor across the tabs and make the
//! per-tab figure fall as `n` rises purely by arithmetic — a way of "winning" by dividing.

use manuk_compositor::mem::{self, Footprint};
use manuk_text::FontContext;

/// One page, held alive, plus what it cost to add.
struct TabSample {
    /// Snapshot file it came from — so an outlier can be traced back to a real site.
    name: String,
    /// Bytes of HTML source.
    source_bytes: usize,
    /// Real RSS delta attributable to loading this page (marginal cost).
    rss_delta: i64,
    /// Real PSS delta.
    pss_delta: i64,
    /// What `Page::estimated_bytes` claims this page retains — the proxy under test.
    proxy_bytes: usize,
}

fn median(mut v: Vec<i64>) -> i64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2]
}

/// The p-th percentile, nearest-rank. Used for p90, which is the figure that catches the
/// heavy tail a median hides — and a browser is judged on its worst tabs, not its median.
fn percentile(mut v: Vec<i64>, p: f64) -> i64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    v[rank.min(v.len()) - 1]
}

fn mb(bytes: i64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Collect snapshot HTML files, sorted by name so a re-run measures the same corpus in the
/// same order — a benchmark whose input set shuffles between runs cannot show a trend.
fn snapshot_files(dir: &str) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "html").unwrap_or(false))
        .collect();
    v.sort();
    v
}

pub fn run(args: &[String], fonts: &FontContext) {
    let dir = flag(args, "--snapshots").unwrap_or("/tmp/manuk-oracle-snapshots");
    let width: f32 = flag(args, "--width")
        .and_then(|w| w.parse().ok())
        .unwrap_or(1280.0);
    let checkpoints: Vec<usize> = flag(args, "--n")
        .unwrap_or("10,50,100")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let max_n = checkpoints.iter().copied().max().unwrap_or(0);
    // Trim after each page settles, rather than only on eviction. This is the counterfactual
    // for the finding: it separates *memory the tabs are holding* from *memory the allocator
    // is holding on their behalf*, and the gap between a run with and without it is the size
    // of the load-time spike that would otherwise be baked into RSS for the session's life.
    let trim_each = args.iter().any(|a| a == "--trim-each");

    if mem::process_footprint().is_none() {
        println!(
            "memtabs: /proc/self/smaps_rollup unavailable — this benchmark is LINUX-ONLY.\n\
             macOS and Windows are UNMEASURED and must be reported as a known gap, not \
             covered by this number (docs/loop/PROCESS-MODEL.md §6)."
        );
        return;
    }

    let files = snapshot_files(dir);
    // **Fail loudly rather than fetching.** Silently falling back to the network would
    // turn a memory benchmark into a 100-site crawl and measure the fetch buffers too.
    if files.len() < max_n {
        println!(
            "memtabs: need {max_n} snapshots, found {} in {dir}.\n\
             Populate it with `scripts/oracle-crawl.sh` (it caches every page it visits), \
             or lower --n. NOT fetching: a benchmark that quietly hits the network is \
             measuring the network.",
            files.len()
        );
        if files.is_empty() {
            return;
        }
    }
    let files: Vec<_> = files.into_iter().take(max_n).collect();
    let n_total = files.len();

    println!(
        "== memtabs — the standing 100-tab RSS benchmark (LINUX-ONLY; macOS/Windows UNMEASURED)"
    );
    println!("   corpus: {n_total} real-site snapshots from {dir}, viewport {width}px");
    println!(
        "   mode:   {}",
        if trim_each {
            "--trim-each (allocator trimmed after every page settles)"
        } else {
            "as-loaded (no trim between pages — the allocator keeps the load spike)"
        }
    );

    // Baseline AFTER the font context is up, so the font atlas is charged to the floor and
    // not to tab #1. Everything measured from here is tabs.
    let base = mem::process_footprint().unwrap_or_default();
    println!(
        "   floor:  rss {:.1} MB · pss {:.1} MB · private {:.1} MB  (fonts up, zero tabs)",
        mb(base.rss_bytes as i64),
        mb(base.pss_bytes as i64),
        mb(base.private_bytes as i64),
    );
    println!();

    let mut pages: Vec<manuk_page::Page> = Vec::with_capacity(n_total);
    let mut samples: Vec<TabSample> = Vec::with_capacity(n_total);
    let mut prev = base;
    // Wall time is part of this measurement, not a footnote. A memory figure with no time
    // beside it cannot settle the question these numbers raise — whether to trim after every
    // load or only on eviction — because that trade is precisely bytes against seconds.
    let t0 = std::time::Instant::now();

    // `marg-med`/`marg-p90` are RSS; `mpss-med` is the same median over PSS. They diverge
    // only where growth is in SHARED pages (mmap'd fonts, the binary, page cache), so a
    // large gap says the growth is not really ours to reclaim — and a small one says it is.
    println!(
        "     n   rss MB   pss MB  marg-med  marg-p90  mpss-med   proxy MB  real/proxy    secs"
    );
    for (i, path) in files.iter().enumerate() {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let url = format!("https://snapshot.test/{name}");
        let page = manuk_page::Page::load(&src, &url, fonts, width);
        let proxy_bytes = page.estimated_bytes();
        pages.push(page);

        if trim_each {
            mem::release_free_memory_to_os();
        }
        let now = mem::process_footprint().unwrap_or_default();
        samples.push(TabSample {
            name,
            source_bytes: src.len(),
            rss_delta: now.rss_bytes as i64 - prev.rss_bytes as i64,
            pss_delta: now.pss_bytes as i64 - prev.pss_bytes as i64,
            proxy_bytes,
        });
        prev = now;

        let n = i + 1;
        if checkpoints.contains(&n) || n == n_total {
            report(&samples, base, now, n, t0.elapsed().as_secs_f64());
        }
    }

    println!();
    let held = mem::process_footprint().unwrap_or_default();

    // --- the reclaim half -------------------------------------------------------------
    //
    // Dropping every `Page` is the strongest form of the eviction the tier model calls
    // `Hibernated`: not a frozen renderer, an actually-freed one. If the process does not
    // give the memory back HERE, no softer policy can, and "hibernated tabs cost a few KB"
    // is not a claim this engine can make. This is the half of the benchmark that can
    // falsify us, so it is measured rather than asserted.
    let proxy_total: usize = samples.iter().map(|s| s.proxy_bytes).sum();
    drop(pages);
    let dropped = mem::process_footprint().unwrap_or_default();
    // ...and then the trim, measured SEPARATELY, because the gap between these two numbers
    // is the whole finding: `drop` frees to the allocator, only the trim frees to the OS.
    let trimmed_any = mem::release_free_memory_to_os();
    let after = mem::process_footprint().unwrap_or_default();

    let grew = held.rss_bytes as i64 - base.rss_bytes as i64;
    let by_drop = held.rss_bytes as i64 - dropped.rss_bytes as i64;
    let by_trim = dropped.rss_bytes as i64 - after.rss_bytes as i64;
    let given_back = by_drop + by_trim;
    let pc = |x: i64| {
        if grew > 0 {
            100.0 * x as f64 / grew as f64
        } else {
            0.0
        }
    };

    println!("== RECLAIM — every page dropped (the strongest form of hibernation)");
    println!(
        "   grew {:.1} MB over the floor\n   \
         drop()  returned {:>8.1} MB ({:>3.0}%)   — freed to the ALLOCATOR\n   \
         trim()  returned {:>8.1} MB ({:>3.0}%)   — freed to the KERNEL (malloc_trim, released={trimmed_any})\n   \
         retained{:>9.1} MB ({:>3.0}%)",
        mb(grew),
        mb(by_drop),
        pc(by_drop),
        mb(by_trim),
        pc(by_trim),
        mb(grew - given_back),
        pc(grew - given_back),
    );
    println!(
        "   residual rss {:.1} MB vs floor {:.1} MB",
        mb(after.rss_bytes as i64),
        mb(base.rss_bytes as i64)
    );
    println!();
    println!(
        "== PROXY ERROR — Page::estimated_bytes summed {:.1} MB against {:.1} MB of real growth",
        mb(proxy_total as i64),
        mb(grew)
    );
    if grew > 0 {
        println!(
            "   the per-tab accounting in TabManager is off by {:.1}x — it cannot see the JS\n\
             \x20  heap, decoded images, raster tiles or the font atlas.",
            grew as f64 / proxy_total.max(1) as f64
        );
    }

    // Heaviest tabs by real marginal cost, so an outlier is traceable to a site.
    let mut by_cost: Vec<_> = samples.iter().collect();
    by_cost.sort_by_key(|s| -s.rss_delta);
    println!();
    println!("== heaviest tabs by marginal RSS");
    for s in by_cost.iter().take(5) {
        println!(
            "   {:>8.1} MB  {:>7} KB src  proxy {:>6.1} MB  {}",
            mb(s.rss_delta),
            s.source_bytes / 1024,
            mb(s.proxy_bytes as i64),
            s.name
        );
    }
}

fn report(samples: &[TabSample], base: Footprint, now: Footprint, n: usize, secs: f64) {
    let rss = now.rss_bytes as i64 - base.rss_bytes as i64;
    let pss = now.pss_bytes as i64 - base.pss_bytes as i64;
    let deltas: Vec<i64> = samples.iter().map(|s| s.rss_delta).collect();
    let pss_deltas: Vec<i64> = samples.iter().map(|s| s.pss_delta).collect();
    let proxy: usize = samples.iter().map(|s| s.proxy_bytes).sum();
    println!(
        "  {n:>4}  {:>7.1}  {:>7.1}  {:>8.2}  {:>8.2}  {:>8.2}  {:>9.1}  {:>9.2}x  {secs:>6.1}",
        mb(rss),
        mb(pss),
        mb(median(deltas.clone())),
        mb(percentile(deltas, 90.0)),
        mb(median(pss_deltas)),
        mb(proxy as i64),
        rss as f64 / proxy.max(1) as f64,
    );
}
