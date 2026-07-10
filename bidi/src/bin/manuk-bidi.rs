//! `manuk-bidi` — run Manuk's WebDriver BiDi remote end.
//!
//!   manuk-bidi [--port 9222] [--width 1024] [--height 768]
//!
//! Prints the `ws://` URL a BiDi client (Puppeteer 23+, Selenium) connects to.

use anyhow::Result;

fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = arg("--port", 9222);
    let width: u32 = arg("--width", 1024);
    let height: u32 = arg("--height", 768);

    let (listener, addr) = manuk_bidi::server::bind(&format!("127.0.0.1:{port}")).await?;
    println!("manuk BiDi remote end listening on ws://{addr}");
    println!("viewport {width}x{height}; connect with a WebDriver BiDi client");
    manuk_bidi::server::serve(listener, width, height).await
}
