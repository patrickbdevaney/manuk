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
    grants: crate::policy::Grants,
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
                let mut s = Hub::new(make, log_dir).with_grants(grants);
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
            if matches!(n, "navigate" | "act" | "type" | "fill_form") {
                p.insert("confirm".into(), json!({"type": "boolean", "description": "set true only when the user approved an action the server answered with CONFIRM REQUIRED"}));
            }
            p.insert("page".into(), json!({"type": "string", "description": "page handle `<id>:<seq>` returned by the previous call (a log position: pass it back to continue, even from a new client); omit for the default page, \"new\" for a fresh one"}));
        }
        let (ro, destructive) = crate::policy::annotations(n);
        let mut ann = rmcp::model::ToolAnnotations::new().read_only(ro);
        ann.destructive_hint = Some(destructive);
        ann.open_world_hint = Some(matches!(
            n,
            "navigate" | "act" | "type" | "commit" | "fill_form"
        ));
        let tool = Tool::new(n, d, Arc::new(s)).annotate(ann);
        // R-B1: the core tools load upfront (Claude Code otherwise defers every MCP tool behind a ToolSearch turn).
        if matches!(n, "navigate" | "act" | "type" | "extract") {
            let Value::Object(m) = json!({"anthropic/alwaysLoad": true}) else {
                unreachable!()
            };
            tool.with_meta(rmcp::model::MetaObject(m))
        } else {
            tool
        }
    };
    vec![
        t("navigate", "Load a URL. Returns the page header and a compact observation: one line per node, `[hN] role \"name\" [state]`. Handles (hN) are stable until the next navigation. With `query`, returns the header plus only the passages answering it (with handles for links in them) instead of the full observation.", schema(json!({"url": {"type": "string"}, "query": {"type": "string"}}), &["url"])),
        t("observe", "Read the page. mode=full (default), diff (only what changed since the last observation/action), or viewport (only what is on screen). Output is capped at budget_tokens (default 8000) with a summary of anything cut.", schema(json!({"mode": {"type": "string", "enum": ["full", "diff", "viewport"]}, "budget_tokens": {"type": "integer"}, "include_hidden": {"type": "boolean", "description": "opt out of the injection shield and also return text a human cannot see (opacity 0, offscreen, tiny, camouflaged); default false"}}), &[])),
        t("act", "Act on a handle: click, check (value=false to uncheck), select (value=option name). Fails loudly, with the reason, if the target is covered, disabled, stale or unreachable. Returns the effect as a diff (+added ~changed -removed) plus verification.", schema(json!({"handle": {"type": "string"}, "action": {"type": "string", "enum": ["click", "check", "select", "focus", "hover"]}, "value": {"type": "string"}, "steps": {"type": "array", "description": "INSTEAD of handle: run these in order in ONE call, e.g. [{\"tool\":\"type\",\"handle\":\"h3\",\"text\":\"x\"},{\"tool\":\"act\",\"handle\":\"h9\",\"action\":\"click\"}]. Tools: act, type, key, upload, drag, scroll, wait_for, navigate, back, dialog. Stops after the first step that fails, opens a dialog/popup, or navigates, and reports where.", "items": {"type": "object"}}}), &[])),
        t("fill_form", "Fill several fields and optionally submit in ONE call: fields=[{handle, value}] (text -> typed, checkbox/radio -> checked (value false unchecks), select/combobox -> option chosen; `action` overrides), `submit`=handle to click last. Stops at the first failed step, dialog, popup or navigation and says which step it stopped at.", schema(json!({"fields": {"type": "array", "items": {"type": "object", "properties": {"handle": {"type": "string"}, "value": {}, "action": {"type": "string", "enum": ["type", "check", "select"]}}, "required": ["handle", "value"]}}, "submit": {"type": "string"}}), &["fields"])),
        t("back", "Go back one history entry. Returns the page you land on like a navigation.", schema(json!({}), &[])),
        t("wait_for", "Wait until `text` appears on the page or `handle` is present, up to `timeout` seconds (default 10, max 60). Returns when found, or says TIMED OUT (not an error).", schema(json!({"text": {"type": "string"}, "handle": {"type": "string"}, "timeout": {"type": "number"}}), &[])),
        t("type", "Type text into a text field handle (replaces its value); submit=true then submits its form. Returns the effect diff.", schema(json!({"handle": {"type": "string"}, "text": {"type": "string"}, "submit": {"type": "boolean"}}), &["handle", "text"])),
        t("key", "Press a key on the focused element (or on `handle`, focused first): Enter, Escape, Tab, ArrowDown/Up/Left/Right, Backspace, Delete, Home, End, PageUp/PageDown, Space, F1-F12, one character, or a chord like Control+a. Returns the effect diff.", schema(json!({"key": {"type": "string"}, "handle": {"type": "string"}}), &["key"])),
        t("upload", "Attach local file(s) to a file-input handle (`path`, or `paths`). Only files under the temp dir, the working dir or the download dir can be attached. Returns the effect diff.", schema(json!({"handle": {"type": "string"}, "path": {"type": "string"}, "paths": {"type": "array", "items": {"type": "string"}}}), &["handle"])),
        t("drag", "Drag the `from` handle onto the `to` handle (HTML5 drag-and-drop, or a pointer drag). Returns the effect diff.", schema(json!({"from": {"type": "string"}, "to": {"type": "string"}}), &["from", "to"])),
        t("dialog", "Arm the answer for the NEXT native alert/confirm/prompt: action=accept|dismiss, text for a prompt. Unarmed, alerts are acknowledged and confirm/prompt are dismissed. The act that opens the dialog reports `dialog: ... -> accepted|dismissed`.", schema(json!({"action": {"type": "string", "enum": ["accept", "dismiss"]}, "text": {"type": "string"}}), &["action"])),
        t("tabs", "List (default), switch to or close browser tabs (`index` is 1-based). A popup a page opens (OAuth, target=_blank via window.open) is surfaced in the result of the action that opened it, and you are moved onto it; when it closes you are returned to tab 1. Handles are per document: observe after switching.", schema(json!({"action": {"type": "string", "enum": ["list", "switch", "close"]}, "index": {"type": "integer"}}), &[])),
        t("download", "Click `handle` (optional) and wait up to 15 s for the file it downloads. Needs the server's Download capability (--download-dir). Result: {name, bytes, sha256, mime}; the file is stored 0600, never executable, 100 MB cap.", schema(json!({"handle": {"type": "string"}}), &[])),
        t("scroll", "Scroll to a handle (`to`) or by `dy` pixels. Returns the nodes newly in view.", schema(json!({"to": {"type": "string"}, "dy": {"type": "number"}}), &[])),
        t("extract", "Page content. format=text (default), markdown (headings + text + links) or table (rows of a table in scope). Optional scope handle. Results over 20000 chars are paged: pass offset=N from the cursor line to read on.", schema(json!({"format": {"type": "string", "enum": ["markdown", "text", "table"]}, "scope": {"type": "string"}, "offset": {"type": "integer"}, "query": {"type": "string", "description": "return only the top passages matching this (BM25, with link handles) and say 'k of N blocks shown'"}, "include_hidden": {"type": "boolean"}}), &[])),
        t("find", "Rank nodes by role/name/text match for a query (no model call). Returns up to 8 handles.", schema(json!({"query": {"type": "string"}}), &["query"])),
        t("storage_state", "Export (default) or import this session's cookies + localStorage as Playwright storage-state JSON, to/from `path` (export without a path returns the JSON).", schema(json!({"action": {"type": "string", "enum": ["export", "import"]}, "path": {"type": "string"}}), &[])),
        t("checkpoint", "Remember this page's position (its action log) and return a checkpoint id. v1 is replay-based: exact, but restore/fork cost one page load, not a memory snapshot.", schema(json!({}), &[])),
        t("restore", "Rewind to a checkpoint: replay its log into a fresh page (every step hash-verified) and make that the default page. Cookies are not cleared; web storage is. Returns the new page handle.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("fork", "Like restore, but leaves the current default page alone: a new independent page at the checkpoint.", schema(json!({"checkpoint": {"type": "string"}}), &["checkpoint"])),
        t("commit", "Lift this forked branch's write-fence and send the non-safe requests (POST/PUT/DELETE...) it held, once each. Until then a fork sends none: acts report held=[...].", schema(json!({}), &[])),
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

pub fn run_stdio(width: u32, height: u32, grants: crate::policy::Grants) -> Result<()> {
    let log_dir = std::env::var_os("MANUK_ACTLOG_DIR").map(std::path::PathBuf::from);
    let tx = spawn_session(move || AgentBrowser::new(width, height), log_dir, grants);
    serve(tx)
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
    serve(tx)
}

fn serve(tx: mpsc::UnboundedSender<Job>) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::tools;

    /// R-B1 gate: the four core tools carry `_meta["anthropic/alwaysLoad"]`, nothing else does.
    #[test]
    fn core_tools_are_always_loaded() {
        for t in tools() {
            let always = t
                .meta
                .as_ref()
                .and_then(|m| m.0.get("anthropic/alwaysLoad"))
                .and_then(|v| v.as_bool());
            let core = matches!(t.name.as_ref(), "navigate" | "act" | "type" | "extract");
            assert_eq!(
                always == Some(true),
                core,
                "tool {} alwaysLoad mismatch",
                t.name
            );
        }
    }
}
