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
    let end = std::time::Instant::now() + std::time::Duration::from_secs_f64(a[1].parse()?);
    while std::time::Instant::now() < end {
        b.settle().await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    println!("{}", b.eval_value(&js).await?);
    Ok(())
}
