//! rmcp adapter. The engine's page state is not `Send`, so the session lives on one dedicated
//! thread with its own runtime; the (Send + Sync) rmcp handler forwards each call over a channel.

use std::sync::Arc;

use anyhow::Result;
use manuk_agent::AgentBrowser;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde_json::{json, Map, Value};
use tokio::sync::{mpsc, oneshot};

use crate::hub::Hub;
use crate::session::Output;
use crate::Backend;

type Reply = Result<Output, String>;
struct Job {
    tool: String,
    args: Map<String, Value>,
    reply: oneshot::Sender<Reply>,
}

/// The session loop: one `Hub` serving jobs until every sender is dropped. Runs on the calling thread.
fn session_loop<B: Backend + 'static>(
    make: impl Fn() -> B + 'static,
    log_dir: Option<std::path::PathBuf>,
    grants: crate::policy::Grants,
    mut rx: mpsc::UnboundedReceiver<Job>,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("session runtime");
    rt.block_on(async move {
        let mut s = Hub::new(make, log_dir).with_grants(grants);
        while let Some(job) = rx.recv().await {
            let r = s
                .call(&job.tool, &job.args)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = job.reply.send(r);
        }
    });
}

/// Spawn the session thread around any backend built *on that thread* (it need not be `Send`).
pub fn spawn_session<B: Backend + 'static>(
    make: impl Fn() -> B + Send + 'static,
    log_dir: Option<std::path::PathBuf>,
    grants: crate::policy::Grants,
) -> mpsc::UnboundedSender<Job> {
    let (tx, rx) = mpsc::unbounded_channel::<Job>();
    std::thread::Builder::new()
        .name("manuk-session".into())
        // The engine recurses deeply on real pages.
        .stack_size(256 * 1024 * 1024)
        .spawn(move || session_loop(make, log_dir, grants, rx))
        .expect("spawn session thread");
    tx
}

#[derive(Clone)]
pub struct ManukMcp {
    tx: mpsc::UnboundedSender<Job>,
    tools: Arc<Vec<Tool>>,
    /// W3.20: hash of the page's WebMCP tool list as last announced, for `list_changed`.
    webmcp_sig: Arc<std::sync::atomic::AtomicU64>,
}

/// One WebMCP tool the page offers, as an MCP tool `webmcp_<name>`. A withheld one stays listed, marked, and
/// refuses to run.
pub fn webmcp_tool(t: &Value) -> Option<Tool> {
    let name = t["name"].as_str()?;
    let mut desc = t["description"].as_str().unwrap_or("").to_string();
    if let Some(why) = t["withheld"].as_str() {
        desc = format!("⚠ WITHHELD (not runnable): {why}");
    }
    let schema = match &t["inputSchema"] {
        Value::Object(m) => m.clone(),
        _ => schema(json!({}), &[]),
    };
    let ann = rmcp::model::ToolAnnotations::new()
        .read_only(t["annotations"]["readOnlyHint"] == json!(true));
    Some(Tool::new(format!("webmcp_{name}"), desc, Arc::new(schema)).annotate(ann))
}

fn schema(props: Value, required: &[&str]) -> Map<String, Value> {
    let Value::Object(m) = json!({"type": "object", "properties": props, "required": required})
    else {
        unreachable!()
    };
    m
}

pub fn tools() -> Vec<Tool> {
    let t = |n: &'static str, d: &'static str, mut s: Map<String, Value>| {
        if let Some(Value::Object(p)) = s.get_mut("properties") {
            if matches!(n, "act" | "type" | "fill_form") {
                p.insert(
                    "detail".into(),
                    json!({"type": "boolean", "description": "full effect diff"}),
                );
            }
            if matches!(n, "navigate" | "act" | "type" | "fill_form") {
                p.insert(
                    "confirm".into(),
                    json!({"type": "boolean", "description": "user approved CONFIRM REQUIRED"}),
                );
            }
            p.insert(
                "page".into(),
                json!({"type": "string", "description": "page handle (or \"new\")"}),
            );
        }
        let (ro, destructive) = crate::policy::annotations(n);
        let mut ann = rmcp::model::ToolAnnotations::new().read_only(ro);
        ann.destructive_hint = Some(destructive);
        ann.open_world_hint = Some(matches!(
            n,
            "navigate" | "act" | "type" | "commit" | "fill_form"
        ));
        let tool = Tool::new(n, d, Arc::new(s)).annotate(ann);
        // R-B1/W5.T3: the six core tools load upfront (Claude Code otherwise defers every MCP tool behind a ToolSearch turn).
        if matches!(
            n,
            "navigate" | "observe" | "act" | "fill_form" | "extract" | "type"
        ) {
            let Value::Object(m) = json!({"anthropic/alwaysLoad": true}) else {
                unreachable!()
            };
            tool.with_meta(rmcp::model::MetaObject(m))
        } else {
            tool
        }
    };
    vec![
        t("navigate", "Load a URL; returns one line per node: `[hN] role \"name\" [state]`. `query` keeps matching passages. E.g. navigate(url=\"https://a.com\", query=\"shipping cost\")", schema(json!({"url": {"type": "string"}, "query": {"type": "string"}}), &["url"])),
        t("observe", "Re-read the page; mode: full (default), diff (changes only), viewport, interactive, content (text). E.g. observe(mode=\"diff\")", schema(json!({"mode": {"type": "string", "enum": ["full", "diff", "viewport", "interactive", "content"]}, "budget_tokens": {"type": "integer"}, "include_hidden": {"type": "boolean", "description": "include invisible text"}}), &[])),
        t("act", "Click, check, select, focus or hover a handle, or run `steps` in one call (stops at first failure). E.g. act(handle=\"h5\", action=\"click\")", schema(json!({"handle": {"type": "string"}, "action": {"type": "string", "enum": ["click", "check", "select", "focus", "hover", "dismiss"]}, "value": {"type": "string", "description": "option; false unchecks"}, "steps": {"type": "array", "description": "instead of handle: [{\"tool\":\"type\",\"handle\":\"h3\",\"text\":\"x\"}]", "items": {"type": "object"}}}), &[])),
        t("fill_form", "Fill several fields, optionally submit, in one call. E.g. fill_form(fields=[{\"handle\":\"h3\",\"value\":\"A\"}], submit=\"h9\")", schema(json!({"fields": {"type": "array", "items": {"type": "object", "properties": {"handle": {"type": "string"}, "value": {}, "action": {"type": "string", "enum": ["type", "check", "select"]}}, "required": ["handle", "value"]}}, "submit": {"type": "string", "description": "handle clicked last"}}), &["fields"])),
        t("back", "Go back one history entry.", schema(json!({}), &[])),
        t("wait_for", "Wait for `text` or `handle` (`timeout` s, default 10).", schema(json!({"text": {"type": "string"}, "handle": {"type": "string"}, "timeout": {"type": "number"}}), &[])),
        t("type", "Type text into a field handle, replacing its value; submit=true submits the form. `secret` = operator-held credential name, instead of `text`. E.g. type(handle=\"h3\", text=\"hi\", submit=true)", schema(json!({"handle": {"type": "string"}, "text": {"type": "string"}, "secret": {"type": "string"}, "submit": {"type": "boolean"}}), &["handle"])),
        t("key", "Press a key or chord (Enter, Tab, Control+a) on the focused element or `handle`.", schema(json!({"key": {"type": "string"}, "handle": {"type": "string"}}), &["key"])),
        t("upload", "Attach local file(s) (`path` or `paths`) to a file-input handle.", schema(json!({"handle": {"type": "string"}, "path": {"type": "string"}, "paths": {"type": "array", "items": {"type": "string"}}}), &["handle"])),
        t("drag", "Drag handle `from` onto handle `to`.", schema(json!({"from": {"type": "string"}, "to": {"type": "string"}}), &["from", "to"])),
        t("dialog", "Answer the next alert/confirm/prompt; `text` for a prompt.", schema(json!({"action": {"type": "string", "enum": ["accept", "dismiss"]}, "text": {"type": "string"}}), &["action"])),
        t("tabs", "List (default), switch or close tabs (`index` 1-based).", schema(json!({"action": {"type": "string", "enum": ["list", "switch", "close"]}, "index": {"type": "integer"}}), &[])),
        t("download", "Click `handle`, wait for its download (needs --download-dir).", schema(json!({"handle": {"type": "string"}}), &[])),
        t("scroll", "Scroll to handle `to` or by `dy` px (at most one screen per call); returns new nodes.", schema(json!({"to": {"type": "string"}, "dy": {"type": "number"}}), &[])),
        t("extract", "Read page content: format text (default), markdown or table; `query` keeps top passages; long output pages by offset. E.g. extract(format=\"table\", scope=\"h12\")", schema(json!({"format": {"type": "string", "enum": ["markdown", "text", "table"]}, "scope": {"type": "string"}, "offset": {"type": "integer"}, "query": {"type": "string"}, "include_hidden": {"type": "boolean"}}), &[])),
        t("page_data", "JSON the page itself fetched (same-origin, <=64 KB) with URLs.", schema(json!({}), &[])),
        t("find", "Rank nodes for a query; up to 8 handles.", schema(json!({"query": {"type": "string"}}), &["query"])),
        t("storage_state", "Export (default) or import cookies + localStorage JSON.", schema(json!({"action": {"type": "string", "enum": ["export", "import"]}, "path": {"type": "string"}}), &[])),
        t("handoff", "Ask the human to act for you (log in, CAPTCHA, card); returns the effect diff after Resume. Needs the GUI shell.", schema(json!({"reason": {"type": "string", "description": "shown to the human"}, "timeout_ms": {"type": "integer"}}), &["reason"])),
        t("recall", "Last stored tree for a site (needs MANUK_PAGE_STORE).", schema(json!({"site": {"type": "string", "description": "host, default current page's"}}), &[])),
        t("checkpoint", "Save this page's position; returns an id.", schema(json!({}), &[])),
        t("restore", "Rewind to a checkpoint in a fresh default page.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("fork", "Like restore, but a new page; the current one stays.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("note", "Task note kept across navigate/restore/fork. No key lists; empty value deletes.", schema(json!({"key": {"type": "string"}, "value": {"type": "string"}}), &[])),
        t("commit", "Send this fork's held POST/PUT/DELETE requests.", schema(json!({}), &[])),
        t("screenshot", "PNG of the viewport (opt-in; expensive).", schema(json!({"handle": {"type": "string"}}), &[])),
    ]
}

impl ManukMcp {
    pub fn new(tx: mpsc::UnboundedSender<Job>) -> Self {
        ManukMcp {
            tx,
            tools: Arc::new(tools()),
            webmcp_sig: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    async fn ask(&self, tool: &str, args: Map<String, Value>) -> Result<Reply, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Job {
                tool: tool.to_string(),
                args,
                reply,
            })
            .map_err(|_| "session thread is gone".to_string())?;
        rx.await
            .map_err(|_| "session thread dropped the call".to_string())
    }

    /// The page's current WebMCP tools; empty when there is no page or the backend cannot say.
    async fn webmcp(&self) -> Vec<Value> {
        match self.ask("webmcp_list", Map::new()).await {
            Ok(Ok(Output::Text(t))) => {
                // The hub appends a `page:` footer line.
                let body = t.lines().next().unwrap_or("[]");
                serde_json::from_str(body).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
}

impl ServerHandler for ManukMcp {
    #[allow(deprecated)]
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
    }

    async fn list_tools(
        &self,
        _r: Option<PaginatedRequestParams>,
        _c: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut r = ListToolsResult::default();
        // W5.L2: manual checkpoint is not in the default set (the hub checkpoints before state changes); still callable.
        r.tools = self
            .tools
            .iter()
            .filter(|t| t.name != "checkpoint")
            .cloned()
            .collect();
        let dynamic = self.webmcp().await;
        self.webmcp_sig.store(
            crate::hub::fnv(Value::Array(dynamic.clone()).to_string().as_bytes()),
            std::sync::atomic::Ordering::SeqCst,
        );
        r.tools.extend(dynamic.iter().filter_map(webmcp_tool));
        Ok(r)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.iter().find(|t| t.name == name).cloned()
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        c: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let name = req.name.to_string();
        let args = req.arguments.unwrap_or_default();
        let (tool, args) = match name.strip_prefix("webmcp_") {
            Some(n) if !self.tools.iter().any(|t| t.name == name) => {
                let mut a = Map::new();
                a.insert("name".into(), json!(n));
                a.insert("arguments".into(), Value::Object(args));
                ("webmcp_call".to_string(), a)
            }
            _ => (name.clone(), args),
        };
        let res = self
            .ask(&tool, args)
            .await
            .map_err(|e| McpError::internal_error(e, None))?;
        // W3.20: a page that registers or drops a tool announces it, so the client re-lists.
        let mutating = !matches!(
            tool.as_str(),
            "observe"
                | "extract"
                | "page_data"
                | "find"
                | "screenshot"
                | "storage_state"
                | "checkpoint"
                | "note"
                | "recall"
        );
        if mutating {
            let now = crate::hub::fnv(Value::Array(self.webmcp().await).to_string().as_bytes());
            if self
                .webmcp_sig
                .swap(now, std::sync::atomic::Ordering::SeqCst)
                != now
            {
                let _ = c.peer.notify_tool_list_changed().await;
            }
        }
        Ok(match res {
            Ok(Output::Text(t)) => CallToolResult::success(vec![ContentBlock::text(t)]),
            Ok(Output::Image { png, note }) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(png);
                CallToolResult::success(vec![
                    ContentBlock::text(note),
                    ContentBlock::image(b64, "image/png"),
                ])
            }
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
        .into())
    }
}

/// R3.3.a: the scripted human (test-only), read once at startup, before OS confinement closes the
/// filesystem. Its text is JS run in the live page when the agent calls `handoff`; a first line of
/// `//timeout` makes the human never press Resume. Without the env var there is no surface and
/// `handoff` errors.
static HANDOFF_SCRIPT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn load_handoff_script() -> Result<()> {
    if let Some(p) = std::env::var_os("MANUK_HANDOFF_SCRIPT") {
        let s = std::fs::read_to_string(&p)
            .map_err(|e| anyhow::anyhow!("MANUK_HANDOFF_SCRIPT {p:?}: {e}"))?;
        let _ = HANDOFF_SCRIPT.set(s);
    }
    Ok(())
}

struct ScriptedHuman(String);

#[async_trait::async_trait(?Send)]
impl manuk_agent::HumanSurface for ScriptedHuman {
    async fn turn(
        &mut self,
        b: &mut AgentBrowser,
        _req: &manuk_agent::HandoffRequest,
    ) -> manuk_agent::HumanTurn {
        let (timeout, js) = match self.0.strip_prefix("//timeout") {
            Some(rest) => (true, rest),
            None => (false, self.0.as_str()),
        };
        if !js.trim().is_empty() {
            let _ = b.eval_value(js).await;
        }
        if timeout {
            manuk_agent::HumanTurn::TimedOut
        } else {
            manuk_agent::HumanTurn::Resumed
        }
    }
}

/// The browser a stdio session drives: bridge + (when configured) the scripted human surface.
pub fn new_agent_browser(width: u32, height: u32) -> AgentBrowser {
    let mut b = AgentBrowser::new(width, height);
    // W3.20.a: the WebMCP bridge runs before page script, as on the chrome backend.
    b.set_init_script(crate::cdp::webmcp_bridge());
    if let Some(js) = HANDOFF_SCRIPT.get() {
        b.set_human(Box::new(ScriptedHuman(js.clone())));
    }
    b
}

pub fn run_stdio(width: u32, height: u32, grants: crate::policy::Grants) -> Result<()> {
    let log_dir = std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from);
    let tx = spawn_session(move || new_agent_browser(width, height), log_dir, grants);
    serve(tx)
}

/// R3.4.a: serve stdio from a browser that was loaded BEFORE this (forked) process existed. SpiderMonkey
/// is thread-affine, so the session runs on the calling thread - the one that loaded the page - and
/// the stdio transport runs on a fresh thread. The first page the hub opens is `browser`; later ones
/// are built fresh.
pub fn run_stdio_preloaded(
    browser: AgentBrowser,
    width: u32,
    height: u32,
    grants: crate::policy::Grants,
) -> Result<()> {
    let log_dir = std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from);
    let (tx, rx) = mpsc::unbounded_channel::<Job>();
    let transport = std::thread::Builder::new()
        .name("manuk-stdio".into())
        .spawn(move || serve(tx))?;
    let slot = std::cell::RefCell::new(Some(browser));
    session_loop(
        move || {
            slot.borrow_mut()
                .take()
                .unwrap_or_else(|| new_agent_browser(width, height))
        },
        log_dir,
        grants,
        rx,
    );
    transport
        .join()
        .map_err(|_| anyhow::anyhow!("stdio thread panicked"))?
}

/// R4.1: the same tool surface over Chrome (CDP) instead of the in-process engine.
pub fn run_stdio_chrome(width: u32, height: u32, grants: crate::policy::Grants) -> Result<()> {
    let log_dir = std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from);
    let tx = spawn_session(
        move || {
            crate::cdp::CdpBackend::launch(width, height).expect("launch Chrome (set MANUK_CHROME)")
        },
        log_dir,
        grants,
    );
    // CB.TMP: the session thread owns the backend and is not joined at exit, so its Drop never runs on
    // a normal end (stdin EOF); reap Chrome and its profile dir here.
    let r = serve(tx);
    crate::cdp::reap_all();
    r
}

fn serve(tx: mpsc::UnboundedSender<Job>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let r = rt.block_on(async move {
        let svc = ManukMcp::new(tx).serve(rmcp::transport::stdio()).await?;
        svc.waiting().await?;
        Ok(())
    });
    // The stdin reader is a blocking task; do not wait for it.
    rt.shutdown_background();
    r
}

#[cfg(test)]
mod tests {
    use super::tools;

    /// R-B1 gate: the six core tools carry `_meta["anthropic/alwaysLoad"]`, nothing else does.
    #[test]
    fn core_tools_are_always_loaded() {
        for t in tools() {
            let always = t
                .meta
                .as_ref()
                .and_then(|m| m.0.get("anthropic/alwaysLoad"))
                .and_then(|v| v.as_bool());
            let core = matches!(
                t.name.as_ref(),
                "navigate" | "observe" | "act" | "fill_form" | "extract" | "type"
            );
            assert_eq!(
                always == Some(true),
                core,
                "tool {} alwaysLoad mismatch",
                t.name
            );
        }
    }
}
