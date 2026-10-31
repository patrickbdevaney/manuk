//! The browser the MCP session drives. A trait so a Chrome (CDP) backend can slot in later without
//! touching the tool surface; the one implementation today is the in-process [`AgentBrowser`].

use anyhow::Result;
use async_trait::async_trait;
use manuk_a11y::{A11yNode, Rect};
use manuk_agent::{Activation, AgentBrowser, NodeFacts};
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
    /// Let the page's queued fetches/timers run (after a scroll, or while idle).
    async fn settle(&mut self) {}
    /// W3.12: called before an action is dispatched.
    fn begin_act(&mut self) {}
    /// W3.12: let the action's own consequences finish, then say what is still pending, read from the
    /// backend's own queues: `settled: 0 fetch, 0 timers<=2s, layout clean` or `pending: 2 fetch (/api/cart)`.
    async fn settle_line(&mut self) -> String {
        self.settle().await;
        "settled".into()
    }
    /// Activate `node`; returns a one-line description of what the engine reports happened.
    async fn click(&mut self, node: NodeId) -> Result<String>;
    fn type_text(&mut self, node: NodeId, text: &str) -> Result<()>;
    /// Move keyboard focus to `node`. `Ok(false)` when the page refuses (inert or disabled).
    async fn focus(&mut self, node: NodeId) -> Result<bool>;
    /// Move the pointer over `node`; `mouseover` and `:hover` follow.
    async fn hover(&mut self, node: NodeId) -> Result<()>;
    /// Submit the form owning `node`.
    async fn submit(&mut self, node: NodeId) -> Result<String>;
    fn page_text(&self) -> Result<String>;
    /// W3.8: press `key` (`Enter`, `Escape`, `ArrowDown`, `Control+a`, a single character) on the focused
    /// element, else the page.
    async fn press_key(&mut self, _key: &str) -> Result<String> {
        anyhow::bail!("this backend cannot press keys")
    }
    /// W3.8: choose the files at `paths` in the file input `node`.
    async fn upload(&mut self, _node: NodeId, _paths: &[std::path::PathBuf]) -> Result<String> {
        anyhow::bail!("this backend cannot upload files")
    }
    /// W3.8: drag `from` onto `to`.
    async fn drag(&mut self, _from: NodeId, _to: NodeId) -> Result<String> {
        anyhow::bail!("this backend cannot drag")
    }
    /// W3.8: answer the NEXT JS dialog (alert/confirm/prompt) this way. Unarmed, confirm and prompt are
    /// dismissed.
    fn arm_dialog(&mut self, _accept: bool, _text: Option<String>) -> Result<()> {
        anyhow::bail!("this backend has no JS dialogs to answer (the engine implements none)")
    }
    /// W3.8: `list` | `switch` | `close` the browser's tabs (`index` is 1-based). A popup the page opens
    /// is surfaced by the next action's result.
    async fn tabs(&mut self, _action: &str, _index: Option<usize>) -> Result<String> {
        anyhow::bail!("this backend has a single page and no tabs")
    }
    /// W3.8: things that happened on their own since the last call: dialogs answered, downloads finished.
    fn take_notes(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// W3.8: wait up to `secs` for a download to finish; its result lines.
    async fn wait_download(&mut self, _secs: u64) -> Vec<String> {
        Vec::new()
    }
    /// The injection shield's verdict (R1.4); `None` for a backend that cannot compute paint truth.
    fn shield(&self) -> Result<Option<manuk_agent::shield::Shield>> {
        Ok(None)
    }
    /// `(text, absolute href)` for every link on the page.
    fn links(&self) -> Result<Vec<(String, String)>>;
    fn screenshot_png(&mut self) -> Result<Vec<u8>>;
    /// Cookies + localStorage as Playwright storage-state JSON.
    fn export_storage_state(&mut self) -> Result<serde_json::Value> {
        anyhow::bail!("this backend has no storage state")
    }
    fn import_storage_state(&mut self, _state: &serde_json::Value) -> Result<(usize, usize)> {
        anyhow::bail!("this backend has no storage state")
    }
    /// Drop the web storage a previous page left behind, so a replay starts from the empty state a
    /// fresh process has (R3.2 restore). Cookies are not cleared.
    fn reset_storage(&mut self) {}
    /// W3.9: input type / autocomplete / owning form / link target of `node`, for the grant policy.
    /// `None` = this backend cannot say (the policy then treats the element as unknown).
    fn node_facts(&mut self, _node: NodeId) -> Option<NodeFacts> {
        None
    }
    /// Whether this process's request ledger sees the backend's network traffic. A backend whose
    /// requests bypass it (Chrome over CDP) cannot tell CLIENT from WORLD, so checkpoints say UNKNOWN.
    fn sees_network(&self) -> bool {
        true
    }
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
    async fn settle(&mut self) {
        AgentBrowser::settle(self).await
    }
    fn begin_act(&mut self) {
        AgentBrowser::begin_act(self)
    }
    async fn settle_line(&mut self) -> String {
        AgentBrowser::settle(self).await;
        self.pending().line()
    }
    async fn click(&mut self, node: NodeId) -> Result<String> {
        Ok(describe(self.activate(node).await?))
    }
    fn type_text(&mut self, node: NodeId, text: &str) -> Result<()> {
        self.type_into_handle(node, text)
    }
    async fn focus(&mut self, node: NodeId) -> Result<bool> {
        self.focus_handle(node)
    }
    async fn hover(&mut self, node: NodeId) -> Result<()> {
        self.hover_handle(node)
    }
    async fn submit(&mut self, node: NodeId) -> Result<String> {
        Ok(describe(self.submit_at(node).await?))
    }
    fn page_text(&self) -> Result<String> {
        Ok(self.observe()?.text)
    }
    async fn press_key(&mut self, key: &str) -> Result<String> {
        let go = AgentBrowser::press_key(self, key)?;
        Ok(format!(
            "pressed {key}{}",
            if go { "" } else { " (the page cancelled it)" }
        ))
    }
    async fn upload(&mut self, node: NodeId, paths: &[std::path::PathBuf]) -> Result<String> {
        let mut files = Vec::new();
        for p in paths {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text = String::from_utf8_lossy(&std::fs::read(p)?).into_owned();
            files.push((name, "application/octet-stream".to_string(), text));
        }
        self.upload_handle(node, &files)?;
        Ok(format!("chose {} file(s)", files.len()))
    }
    async fn drag(&mut self, from: NodeId, to: NodeId) -> Result<String> {
        let go = self.drag_handles(from, to)?;
        Ok(format!(
            "dragged and dropped{}",
            if go {
                ""
            } else {
                " (the page cancelled the drop)"
            }
        ))
    }
    fn shield(&self) -> Result<Option<manuk_agent::shield::Shield>> {
        Ok(Some(AgentBrowser::shield(self)?))
    }
    fn links(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .observe()?
            .links
            .into_iter()
            .map(|l| (l.text, l.href))
            .collect())
    }
    fn screenshot_png(&mut self) -> Result<Vec<u8>> {
        AgentBrowser::screenshot_png(self)
    }
    fn export_storage_state(&mut self) -> Result<serde_json::Value> {
        manuk_net::storage_state::export()
    }
    fn import_storage_state(&mut self, state: &serde_json::Value) -> Result<(usize, usize)> {
        manuk_net::storage_state::import(state)
    }
    fn node_facts(&mut self, node: NodeId) -> Option<NodeFacts> {
        AgentBrowser::node_facts(self, node)
    }
    fn reset_storage(&mut self) {
        use manuk_net::webstorage::{clear, origins, Area};
        for area in [Area::Local, Area::Session] {
            for o in origins(area) {
                clear(area, &o);
            }
        }
    }
}
