//! EPOCH probe — RESPONSIVENESS + EFFICIENCY (CONSTITUTION §10.2).
//!
//! Measures the real hot path per stage (parse → cascade → layout → display-list → paint) on
//! pages of **increasing size**, and reports both the absolute cost and the **per-KB cost**. The
//! per-KB column is the point: a stage whose per-KB cost *grows* with page size is superlinear,
//! and that is where the real wins are (fix the complexity, not the constant).
//!
//! Numbers only — this module optimizes nothing. Publish first, then remediate worst-first.

use std::time::{Duration, Instant};

use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};
use manuk_layout::layout_document;
use manuk_paint::{CpuPainter, DisplayList, Painter};
use manuk_text::FontContext;

/// One page's per-stage timings.
pub struct StageTimes {
    pub name: String,
    pub bytes: usize,
    pub nodes: usize,
    pub parse: Duration,
    pub cascade: Duration,
    pub layout: Duration,
    pub display_list: Duration,
    pub paint: Duration,
}

impl StageTimes {
    pub fn total(&self) -> Duration {
        self.parse + self.cascade + self.layout + self.display_list + self.paint
    }
}

/// Median of `runs` timings for `f` (median, not mean — one GC/page-fault must not skew a number
/// we are about to turn into an invariant floor).
fn time_median(runs: usize, mut f: impl FnMut()) -> Duration {
    let mut ts: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    ts.sort();
    ts[ts.len() / 2]
}

/// Measure the full pipeline for one page.
pub fn bench_page(name: &str, html: &str, url: &str, vw: f32, vh: u32, fonts: &FontContext, runs: usize) -> StageTimes {
    // Parse.
    let parse = time_median(runs, || {
        let _ = manuk_html::parse(html);
    });
    let dom = manuk_html::parse(html);
    let nodes = dom.descendants(dom.root()).count();

    // Cascade (the engine the render path actually uses here).
    let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
    let cascade = time_median(runs, || {
        let _ = MinimalCascade.cascade(&dom, &sheets);
    });
    let styles = MinimalCascade.cascade(&dom, &sheets);

    // Layout.
    let layout = time_median(runs, || {
        let _ = layout_document(&dom, &styles, fonts, vw);
    });
    let root = layout_document(&dom, &styles, fonts, vw);

    // Display list.
    let display_list = time_median(runs, || {
        let _ = DisplayList::build(&root);
    });
    let dl = DisplayList::build(&root);

    // Paint (rasterize) — the CPU tier, exactly what the render/screenshot path uses.
    let _ = &dl;
    let painter = CpuPainter::new(fonts);
    let paint = time_median(runs, || {
        let _ = painter.render(&root, vw as u32, vh, manuk_css::Rgba::WHITE);
    });

    StageTimes {
        name: name.to_string(),
        bytes: html.len(),
        nodes,
        parse,
        cascade,
        layout,
        display_list,
        paint,
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Print the report: absolute per-stage cost, then **per-KB** cost so superlinear scaling is
/// visible by inspection (a per-KB number that climbs with page size = superlinear stage).
pub fn report(rows: &[StageTimes]) {
    println!("\n=== EPOCH-1 · RESPONSIVENESS + EFFICIENCY (median of runs, ms) ===\n");
    println!(
        "{:<14} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "page", "KB", "nodes", "parse", "cascade", "layout", "dlist", "paint", "TOTAL"
    );
    for r in rows {
        println!(
            "{:<14} {:>7.1} {:>7} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>9.2}",
            r.name,
            r.bytes as f64 / 1024.0,
            r.nodes,
            ms(r.parse),
            ms(r.cascade),
            ms(r.layout),
            ms(r.display_list),
            ms(r.paint),
            ms(r.total())
        );
    }

    println!("\n--- per-KB cost (µs/KB) — a column that CLIMBS with page size is superlinear ---\n");
    println!(
        "{:<14} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "page", "KB", "parse", "cascade", "layout", "dlist", "paint", "TOTAL"
    );
    for r in rows {
        let kb = (r.bytes as f64 / 1024.0).max(0.001);
        let per = |d: Duration| ms(d) * 1000.0 / kb; // µs per KB
        println!(
            "{:<14} {:>7.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>9.1}",
            r.name,
            kb,
            per(r.parse),
            per(r.cascade),
            per(r.layout),
            per(r.display_list),
            per(r.paint),
            per(r.total())
        );
    }

    println!("\n--- per-NODE cost (µs/node) ---\n");
    println!(
        "{:<14} {:>7} {:>8} {:>8} {:>8} {:>9}",
        "page", "nodes", "cascade", "layout", "paint", "TOTAL"
    );
    for r in rows {
        let n = (r.nodes as f64).max(1.0);
        let per = |d: Duration| ms(d) * 1000.0 / n;
        println!(
            "{:<14} {:>7} {:>8.2} {:>8.2} {:>8.2} {:>9.2}",
            r.name,
            r.nodes,
            per(r.cascade),
            per(r.layout),
            per(r.paint),
            per(r.total())
        );
    }
    println!();
}


/// **F4 — INTERACTIVE LATENCY.** The cost of one scroll notification and one click dispatch on a
/// real page.
///
/// The static pipeline bench measures *loading*. It says nothing about what happens once the page is
/// on screen — and that is where a browser is actually judged. A page that loads in 80ms and then
/// takes 200ms to acknowledge a wheel event is not fast; it is broken, and the load number will
/// happily report that everything is fine.
///
/// This is the number that caught it: publishing the layout and style snapshots into the JS world
/// used to CLONE them — a 19,000-entry rect map and 19,000 `ComputedStyle` structs — on every entry.
/// Per wheel event. The load bench never saw it.
///
/// Floor: **one frame (16ms)**. Anything slower is felt.
pub fn bench_interactive(name: &str, html: &str, url: &str, vw: f32, vh: u32, fonts: &FontContext, runs: usize) -> (f64, f64) {
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let mut page = manuk_page::Page::load(html, url, fonts, vw);
    // Scroll: what the shell does once per frame while the wheel is turning.
    let scroll = time_median(runs, || {
        page.publish_view_state(0.0, 500.0, None);
        page.view_changed(500.0, vw, vh as f32, true);
    });
    // Click: what the shell does when the user hits a link.
    let root = page.dom().root();
    let target = manuk_css::query_selector_all(page.dom(), root, "a")
        .first()
        .copied();
    let click = match target {
        Some(n) => time_median(runs, || {
            page.dispatch_click(n, fonts, vw);
        }),
        None => Duration::ZERO,
    };
    // What the old path did on EVERY entry, measured so the regression is a number and not a story:
    // deep-clone the style map and the rect map.
    let styles = page.styles_map();
    let clone_cost = time_median(runs, || {
        let _ = styles.clone();
    });
    let rects = page.root_box.node_rects(page.dom());
    let rect_clone = time_median(runs, || {
        let _ = rects.clone();
    });
    eprintln!(
        "    (the removed per-entry work: styles.clone() {:.2}ms + rects.clone() {:.2}ms \
         on {} nodes — this ran on every wheel event)",
        ms(clone_cost),
        ms(rect_clone),
        page.dom().descendants(page.dom().root()).count()
    );
    let _ = name;
    (ms(scroll), ms(click))
}
