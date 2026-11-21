//! `dom-eval <url> <wait_secs> <js-file>` — load `url` under the engine, wait, evaluate the JS expression in the
//! file in the page, print its string value. The manuk-side half of `bench/agent-tasks/rg3ba_close_dom.py`.

use anyhow::{bail, Result};
use manuk_agent::AgentBrowser;

#[tokio::main]
async fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 3 {
        bail!("usage: dom-eval <url> <wait_secs> <js-file>");
    }
    let js = std::fs::read_to_string(&a[2])?;
    let mut b = AgentBrowser::new(1024, 768);
    b.navigate(&a[0]).await?;
    // W6.R1.a.a.a: the wait is PAGE time, the same thing Chrome's harness gives a page by sleeping.
    // Each `settle()` opens the virtual clock a further 1.8 s (`Page::settle_async`, 3 x 600 ms), so
    // the old loop (settle every 100 ms for `wait` real seconds) advanced ~18x real time, by a count
    // that depended on host speed: at wait 1 a 10 s timer had already fired (now=9997). Settle a fixed
    // ceil(wait / 1.8 s) times instead, so `wait` maps to page time deterministically.
    let wait: f64 = a[1].parse()?;
    let settles = (wait / 1.8).ceil().max(0.0) as u32;
    for _ in 0..settles {
        b.settle().await;
    }
    println!("{}", b.eval_value(&js).await?);
    Ok(())
}
