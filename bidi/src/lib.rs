//! manuk-bidi — **E4: a WebDriver BiDi remote end**.
//!
//! *Why BiDi and not CDP:* CDP is Chromium-only and is **not a standard**. WebDriver
//! BiDi is the W3C standards-track bidirectional protocol that modern Puppeteer and
//! Selenium speak. Implementing it makes Manuk drivable by existing tooling without
//! reimplementing a competitor's private protocol — the strategic call recorded in
//! CLAUDE.md § E4.
//!
//! Layering: [`protocol`] holds the message shapes and the command dispatcher and is
//! **transport-free** (hence directly testable); [`server`] is a thin
//! `tokio-tungstenite` shell that frames JSON over a WebSocket.
//!
//! ```no_run
//! # async fn run() -> anyhow::Result<()> {
//! let (listener, addr) = manuk_bidi::server::bind("127.0.0.1:0").await?;
//! println!("BiDi remote end on ws://{addr}");
//! manuk_bidi::server::serve(listener, 1024, 768).await
//! # }
//! ```

pub mod protocol;
pub mod server;

pub use protocol::{BidiError, Command, Dispatched, ErrorCode, Outgoing, Session};
