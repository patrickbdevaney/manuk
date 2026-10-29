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

/// Spawn the session thread around any backend built *on that thread* (it need not be `Send`).
pub fn spawn_session<B: Backend + 'static>(
    make: impl Fn() -> B + Send + 'static,
    log_dir: Option<std::path::PathBuf>,
) -> mpsc::UnboundedSender<Job> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
    std::thread::Builder::new()
        .name("manuk-session".into())
        // The engine recurses deeply on real pages.
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("session runtime");
            rt.block_on(async move {
                let mut s = Hub::new(make, log_dir);
                while let Some(job) = rx.recv().await {
                    let r = s
                        .call(&job.tool, &job.args)
                        .await
                        .map_err(|e| format!("{e:#}"));
                    let _ = job.reply.send(r);
                }
            });
        })
        .expect("spawn session thread");
    tx
}

#[derive(Clone)]
pub struct ManukMcp {
    tx: mpsc::UnboundedSender<Job>,
    tools: Arc<Vec<Tool>>,
}

fn schema(props: Value, required: &[&str]) -> Map<String, Value> {
    let Value::Object(m) = json!({"type": "object", "properties": props, "required": required})
    else {
        unreachable!()
    };
    m
}

fn tools() -> Vec<Tool> {
    let t = |n: &'static str, d: &'static str, mut s: Map<String, Value>| {
        if let Some(Value::Object(p)) = s.get_mut("properties") {
            p.insert("page".into(), json!({"type": "string", "description": "page handle `<id>:<seq>` returned by the previous call (a log position: pass it back to continue, even from a new client); omit for the default page, \"new\" for a fresh one"}));
        }
        Tool::new(n, d, Arc::new(s))
    };
    vec![
        t("navigate", "Load a URL. Returns the page header and a compact observation: one line per node, `[hN] role \"name\" [state]`. Handles (hN) are stable until the next navigation.", schema(json!({"url": {"type": "string"}}), &["url"])),
        t("observe", "Read the page. mode=full (default), diff (only what changed since the last observation/action), or viewport (only what is on screen). Output is capped at budget_tokens (default 8000) with a summary of anything cut.", schema(json!({"mode": {"type": "string", "enum": ["full", "diff", "viewport"]}, "budget_tokens": {"type": "integer"}, "include_hidden": {"type": "boolean", "description": "opt out of the injection shield and also return text a human cannot see (opacity 0, offscreen, tiny, camouflaged); default false"}}), &[])),
        t("act", "Act on a handle: click, check (value=false to uncheck), select (value=option name). Fails loudly, with the reason, if the target is covered, disabled, stale or unreachable. Returns the effect as a diff (+added ~changed -removed) plus verification.", schema(json!({"handle": {"type": "string"}, "action": {"type": "string", "enum": ["click", "check", "select", "focus", "hover"]}, "value": {"type": "string"}}), &["handle"])),
        t("type", "Type text into a text field handle (replaces its value); submit=true then submits its form. Returns the effect diff.", schema(json!({"handle": {"type": "string"}, "text": {"type": "string"}, "submit": {"type": "boolean"}}), &["handle", "text"])),
        t("scroll", "Scroll to a handle (`to`) or by `dy` pixels. Returns the nodes newly in view.", schema(json!({"to": {"type": "string"}, "dy": {"type": "number"}}), &[])),
        t("extract", "Page content. format=text (default), markdown (headings + text + links) or table (rows of a table in scope). Optional scope handle.", schema(json!({"format": {"type": "string", "enum": ["markdown", "text", "table"]}, "scope": {"type": "string"}, "include_hidden": {"type": "boolean"}}), &[])),
        t("find", "Rank nodes by role/name/text match for a query (no model call). Returns up to 8 handles.", schema(json!({"query": {"type": "string"}}), &["query"])),
        t("storage_state", "Export (default) or import this session's cookies + localStorage as Playwright storage-state JSON, to/from `path` (export without a path returns the JSON).", schema(json!({"action": {"type": "string", "enum": ["export", "import"]}, "path": {"type": "string"}}), &[])),
        t("checkpoint", "Remember this page's position (its action log) and return a checkpoint id. v1 is replay-based: exact, but restore/fork cost one page load, not a memory snapshot.", schema(json!({}), &[])),
        t("restore", "Rewind to a checkpoint: replay its log into a fresh page (every step hash-verified) and make that the default page. Cookies are not cleared; web storage is. Returns the new page handle.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("fork", "Like restore, but leaves the current default page alone: a new independent page at the checkpoint.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("screenshot", "PNG of the current viewport (opt-in; the expensive channel).", schema(json!({"handle": {"type": "string"}}), &[])),
    ]
}

impl ManukMcp {
    pub fn new(tx: mpsc::UnboundedSender<Job>) -> Self {
        ManukMcp {
            tx,
            tools: Arc::new(tools()),
        }
    }
}

impl ServerHandler for ManukMcp {
    #[allow(deprecated)]
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _r: Option<PaginatedRequestParams>,
        _c: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut r = ListToolsResult::default();
        r.tools = (*self.tools).clone();
        Ok(r)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.iter().find(|t| t.name == name).cloned()
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _c: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let (reply, rx) = oneshot::channel();
        let job = Job {
            tool: req.name.to_string(),
            args: req.arguments.unwrap_or_default(),
            reply,
        };
        self.tx
            .send(job)
            .map_err(|_| McpError::internal_error("session thread is gone", None))?;
        let res = rx
            .await
            .map_err(|_| McpError::internal_error("session thread dropped the call", None))?;
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

pub fn run_stdio(width: u32, height: u32) -> Result<()> {
    let log_dir = std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from);
    let tx = spawn_session(move || AgentBrowser::new(width, height), log_dir);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let svc = ManukMcp::new(tx).serve(rmcp::transport::stdio()).await?;
        svc.waiting().await?;
        Ok(())
    })
}
