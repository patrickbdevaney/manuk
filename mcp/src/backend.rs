//! The browser the MCP session drives. A trait so a Chrome (CDP) backend can slot in later without
//! touching the tool surface; the one implementation today is the in-process [`AgentBrowser`].

use anyhow::Result;
use async_trait::async_trait;
use manuk_a11y::{A11yNode, Rect};
use manuk_agent::{Activation, AgentBrowser};
use manuk_dom::NodeId;

/// Not `Send`: the engine's page state is single-threaded, so the server pins a backend to one
/// thread (see `server`) instead of forcing `Send` onto every implementation.
#[async_trait(?Send)]
pub trait Backend {
    async fn navigate(&mut self, url: &str) -> Result<()>;
    fn tree(&self) -> Result<A11yNode>;
    fn url(&self) -> String;
    fn title(&self) -> String;
    /// The viewport in absolute document coordinates (already offset by the scroll).
    fn viewport(&self) -> Rect;
    fn scroll_y(&self) -> f32;
    fn content_height(&self) -> f32;
    fn scroll_by(&mut self, dy: f32);
    /// Activate `node`; returns a one-line description of what the engine reports happened.
    async fn click(&mut self, node: NodeId) -> Result<String>;
    fn type_text(&mut self, node: NodeId, text: &str) -> Result<()>;
    /// Submit the form owning `node`.
    async fn submit(&mut self, node: NodeId) -> Result<String>;
    fn page_text(&self) -> Result<String>;
    /// `(text, absolute href)` for every link on the page.
    fn links(&self) -> Result<Vec<(String, String)>>;
    fn screenshot_png(&self) -> Result<Vec<u8>>;
}

fn describe(a: Activation) -> String {
    match a {
        Activation::Navigated(u) => format!("followed link to {u}"),
        Activation::Submitted(u) => format!("submitted form to {u}"),
        Activation::Toggled(on) => format!("toggled {}", if on { "on" } else { "off" }),
        Activation::Disclosed(open) => {
            format!("disclosure {}", if open { "opened" } else { "closed" })
        }
        Activation::Inert => "dispatched click (no built-in activation behaviour)".into(),
    }
}

#[async_trait(?Send)]
impl Backend for AgentBrowser {
    async fn navigate(&mut self, url: &str) -> Result<()> {
        AgentBrowser::navigate(self, url).await
    }
    fn tree(&self) -> Result<A11yNode> {
        self.a11y_tree()
    }
    fn url(&self) -> String {
        self.current_url().unwrap_or("").to_string()
    }
    fn title(&self) -> String {
        self.current_title().unwrap_or("").to_string()
    }
    fn viewport(&self) -> Rect {
        let (w, h) = AgentBrowser::viewport(self);
        Rect {
            x: 0.0,
            y: self.scroll_offset(),
            width: w as f32,
            height: h as f32,
        }
    }
    fn scroll_y(&self) -> f32 {
        self.scroll_offset()
    }
    fn content_height(&self) -> f32 {
        self.observe().map(|o| o.content_height).unwrap_or(0.0)
    }
    fn scroll_by(&mut self, dy: f32) {
        AgentBrowser::scroll_by(self, dy)
    }
    async fn click(&mut self, node: NodeId) -> Result<String> {
        Ok(describe(self.activate(node).await?))
    }
    fn type_text(&mut self, node: NodeId, text: &str) -> Result<()> {
        self.type_into_handle(node, text)
    }
    async fn submit(&mut self, node: NodeId) -> Result<String> {
        Ok(describe(self.submit_at(node).await?))
    }
    fn page_text(&self) -> Result<String> {
        Ok(self.observe()?.text)
    }
    fn links(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .observe()?
            .links
            .into_iter()
            .map(|l| (l.text, l.href))
            .collect())
    }
    fn screenshot_png(&self) -> Result<Vec<u8>> {
        AgentBrowser::screenshot_png(self)
    }
}
