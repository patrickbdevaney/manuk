//! manuk — the human-operator GUI browser shell.
//!
//! Two entry points over the same engine pipeline (CLAUDE.md § phase: headful GUI):
//!
//!   manuk render <url> [-o out.png] [--width N] [--height N]
//!       Headless: run net→html→css→layout→paint and write a PNG. No GPU/display
//!       needed — this is the CPU raster tier.
//!
//!   manuk browse <url> [--width N]              (requires the `gui` feature)
//!       Open a winit/wgpu window and present the rendered page; scroll with the
//!       wheel, resize to reflow.
//!
//! `<url>` may be `http(s)://…`, `file://…`, or a local path.

mod tab;

#[cfg(feature = "gui")]
mod gui;

use anyhow::{bail, Result};
use manuk_text::FontContext;

const DEFAULT_WIDTH: u32 = 1024;
/// Cap the headless canvas height so a very long page can't allocate absurd memory.
const MAX_RENDER_HEIGHT: u32 = 20_000;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("render") => cmd_render(&args[1..]),
        Some("browse") => cmd_browse(&args[1..]),
        #[cfg(feature = "spidermonkey")]
        Some("eval") => cmd_eval(&args[1..]),
        Some("version") | Some("--version") | Some("-V") => {
            println!("manuk {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn print_usage() {
    eprintln!(
        "manuk {} — a browser engine (headful GUI phase)\n\n\
         USAGE:\n  \
         manuk render <url> [-o out.png] [--width N] [--height N]\n  \
         manuk browse <url> [--width N]{}\n  \
         manuk version\n\n\
         <url> may be http(s)://, file://, or a local path.",
        env!("CARGO_PKG_VERSION"),
        if cfg!(feature = "gui") {
            ""
        } else {
            "   (unavailable: built without the `gui` feature)"
        }
    );
}

/// Parse a flag with a value, e.g. `--width 800` or `-o page.png`.
fn flag_value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if names.contains(&a.as_str()) {
            return it.next().map(String::as_str);
        }
    }
    None
}

/// First positional argument (not starting with `-`).
fn positional(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|a| !a.starts_with('-'))
        .map(String::as_str)
}

fn cmd_render(args: &[String]) -> Result<()> {
    let Some(url) = positional(args) else {
        bail!("render: missing <url>");
    };
    let out = flag_value(args, &["-o", "--out"]).unwrap_or("manuk.png");
    let width: u32 = flag_value(args, &["--width", "-w"])
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WIDTH);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let fonts = FontContext::new();
    if fonts.face_count() == 0 {
        eprintln!("warning: no system fonts found; text will not render");
    }

    // Streaming load with a first-paint checkpoint (http(s)); buffered for data:/file.
    let (mut page, first_paint) =
        rt.block_on(manuk_page::fetch_streaming_page(url, &fonts, width as f32))?;
    if let Some(fp) = &first_paint {
        println!(
            "  first-paint: {:.0}px at the head-complete checkpoint (before full load)",
            fp.content_bottom()
        );
    }
    let final_url = page.final_url.clone();
    // Fetch + apply render-blocking external stylesheets (<link rel=stylesheet>).
    let sheets = rt.block_on(page.fetch_and_apply_stylesheets(&fonts, width as f32));
    if sheets > 0 {
        println!("  styles: {sheets} external stylesheet(s) applied");
    }

    let height: u32 = flag_value(args, &["--height", "-h"])
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| (page.content_height.ceil() as u32).clamp(1, MAX_RENDER_HEIGHT));

    let canvas = page.paint(&fonts, width, height);
    canvas.save_png(out)?;

    println!("Rendered: {}", page.title);
    println!("  url:    {final_url}");
    println!(
        "  size:   {width}x{height}px  (content height {:.0}px)",
        page.content_height
    );
    if let Some(rss) = manuk_compositor::mem::process_rss_bytes() {
        println!(
            "  rss:    {:.1} MB (process resident)",
            rss as f64 / 1_048_576.0
        );
    }
    println!("  wrote:  {out}");
    Ok(())
}

/// `manuk eval <expr>` — evaluate JavaScript via SpiderMonkey and print the result.
/// Present only under the `spidermonkey` feature; also the link anchor for the C2
/// binary-size measurement (it keeps the JS engine from being dead-stripped).
#[cfg(feature = "spidermonkey")]
fn cmd_eval(args: &[String]) -> Result<()> {
    let Some(expr) = positional(args) else {
        bail!("eval: missing <expr>");
    };
    let mut rt = manuk_js::new_runtime();
    match rt.eval(expr, "eval") {
        Ok(v) => {
            println!("{v:?}");
            Ok(())
        }
        Err(e) => bail!("{e}"),
    }
}

#[cfg(feature = "gui")]
fn cmd_browse(args: &[String]) -> Result<()> {
    let Some(url) = positional(args) else {
        bail!("browse: missing <url>");
    };
    let width: u32 = flag_value(args, &["--width", "-w"])
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WIDTH);
    gui::run(url.to_string(), width)
}

#[cfg(not(feature = "gui"))]
fn cmd_browse(_args: &[String]) -> Result<()> {
    bail!("this binary was built without the `gui` feature; use `manuk render` or rebuild with --features gui")
}
