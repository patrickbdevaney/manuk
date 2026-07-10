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
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
