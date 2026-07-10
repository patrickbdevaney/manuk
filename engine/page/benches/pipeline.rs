//! Render-latency benchmark (P0.2 measurement harness).
//!
//! Deterministic proxy for "click-to-navigate latency": times the CPU pipeline
//! parse → style → layout → paint on a fixed in-memory page (no network variance).
//! Run: `cargo bench -p manuk-page`. Baseline numbers live in `bench/BASELINE.md`.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use manuk_page::Page;
use manuk_text::FontContext;

const SAMPLE: &str = r#"<!DOCTYPE html><html><head><title>Bench Page</title>
<style>
  body { background: white; color: #222; }
  h1 { color: rebeccapurple; }
  .box { background: #dde7ff; padding: 10px; }
  .note { color: #b00; font-style: italic; }
</style></head><body>
  <h1>Manuk render benchmark</h1>
  <p class="box">This paragraph exercises block + inline layout, CSS cascade, and
     font rasterization across several lines of wrapped text so the numbers reflect a
     representative content page rather than a trivial one. <a href="/next">a link</a>.</p>
  <h2>Second section</h2>
  <p>More text with <b>bold</b> and <i>italic</i> runs, plus another
     <a href="https://example.com/">absolute link</a> to resolve.</p>
  <p class="note">Floats, tables, and Vello GPU paint are future milestones.</p>
  <ul><li>alpha</li><li>beta</li><li>gamma</li><li>delta</li></ul>
</body></html>"#;

fn bench_pipeline(c: &mut Criterion) {
    let fonts = FontContext::new();

    c.bench_function("load_800 (parse+style+layout)", |b| {
        b.iter(|| {
            let p = Page::load(black_box(SAMPLE), "http://bench.local/", &fonts, 800.0);
            black_box(p.content_height)
        })
    });

    c.bench_function("load+paint_800x600 (full CPU pipeline)", |b| {
        b.iter(|| {
            let p = Page::load(black_box(SAMPLE), "http://bench.local/", &fonts, 800.0);
            let canvas = p.paint(&fonts, 800, 600);
            black_box(canvas.width())
        })
    });

    // B-latency: a large page where the head + above-the-fold arrives first and a long
    // tail streams after. The first-paint checkpoint lays out only the prefix, so it
    // completes well before the full document — that time gap is the click-to-first-
    // paint win. These two benches measure both ends of the gap.
    let head_top = "<html><head><title>Big</title></head><body>\
                    <h1>Above the fold</h1><p>intro paragraph of the article</p>";
    let tail: String = (0..400)
        .map(|i| format!("<p>streamed paragraph number {i} of the long article body</p>"))
        .collect();
    let full_html = format!("{head_top}{tail}</body></html>");

    c.bench_function("streaming_first_paint (head+fold only)", |b| {
        b.iter(|| {
            // Only the head+above-the-fold prefix has arrived: this is the work done
            // before the first paint is on screen.
            let load =
                Page::load_streaming([black_box(head_top)], "http://bench.local/", &fonts, 800.0);
            black_box(load.first_paint.map(|f| f.content_bottom()))
        })
    });

    c.bench_function("streaming_full_load (whole document)", |b| {
        b.iter(|| {
            let p = Page::load(black_box(&full_html), "http://bench.local/", &fonts, 800.0);
            black_box(p.content_height)
        })
    });

    // §8 metric #4 (frame time): the per-frame CPU raster cost in isolation (page laid
    // out once, then re-painted each iteration — a repaint, the compositor's frame).
    let page = Page::load(SAMPLE, "http://bench.local/", &fonts, 800.0);
    c.bench_function("paint_frame_800x600 (CPU raster only)", |b| {
        b.iter(|| {
            let canvas = page.paint(&fonts, 800, 600);
            black_box(canvas.width())
        })
    });
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
