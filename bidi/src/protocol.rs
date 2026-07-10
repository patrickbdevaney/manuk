//! E4 — the **WebDriver BiDi** wire protocol: message shapes + the command dispatcher.
//!
//! Deliberately **not CDP**: CDP is Chromium-only and not a standard. BiDi is the
//! W3C standards-track bidirectional protocol, and it is what modern Puppeteer and
//! Selenium speak. Implementing it means a standards-conformant remote end rather
//! than chasing a competitor's private protocol.
//!
//! This module is transport-free on purpose — [`Session::dispatch`] maps a decoded
//! [`Command`] to an [`Outgoing`], so the whole protocol is testable without a socket.
//! [`crate::server`] is the thin `tokio-tungstenite` shell around it.
//!
//! **Implemented modules:** `session`, `browsingContext`, `input` (pointer click),
//! plus `log` / `network` events. **Deferred (documented, not stubbed):**
//! `script.evaluate`/`callFunction` need the JS engine (D3, feature-gated); the full
//! `input` grammar (keys, wheel, multi-pointer, durations) is not modelled.

use std::collections::HashMap;

use base64::Engine as _;
use manuk_agent::AgentBrowser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// A decoded BiDi command: `{"id":1,"method":"session.new","params":{…}}`.
#[derive(Clone, Debug, Deserialize)]
pub struct Command {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// The BiDi error codes we can produce (spec §"Error Codes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidArgument,
    NoSuchFrame,
    UnknownCommand,
    UnknownError,
    UnsupportedOperation,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidArgument => "invalid argument",
            ErrorCode::NoSuchFrame => "no such frame",
            ErrorCode::UnknownCommand => "unknown command",
            ErrorCode::UnknownError => "unknown error",
            ErrorCode::UnsupportedOperation => "unsupported operation",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BidiError {
    pub code: ErrorCode,
    pub message: String,
}

impl BidiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        BidiError {
            code,
            message: message.into(),
        }
    }
    fn invalid(msg: impl Into<String>) -> Self {
        BidiError::new(ErrorCode::InvalidArgument, msg)
    }
}

/// One message from the remote end to the client.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Outgoing {
    Success { r#type: String, id: u64, result: Value },
    Error { r#type: String, id: u64, error: String, message: String },
    Event { r#type: String, method: String, params: Value },
}

impl Outgoing {
    pub fn success(id: u64, result: Value) -> Self {
        Outgoing::Success {
            r#type: "success".into(),
            id,
            result,
        }
    }
    pub fn error(id: u64, e: &BidiError) -> Self {
        Outgoing::Error {
            r#type: "error".into(),
            id,
            error: e.code.as_str().into(),
            message: e.message.clone(),
        }
    }
    pub fn event(method: impl Into<String>, params: Value) -> Self {
        Outgoing::Event {
            r#type: "event".into(),
            method: method.into(),
            params,
        }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Outgoing is always serializable")
    }
}

/// The result of dispatching one command.
///
/// Ordering matters and is not cosmetic. A real remote end emits
/// `browsingContext.contextCreated` **before** the `browsingContext.create` reply,
/// because clients (Puppeteer among them) build their context map from the *event* and
/// then look the new context up the moment the command resolves. Emitting the event
/// after the reply loses that race and the client reports "failing to create a browsing
/// context correctly" — observed against puppeteer-core 23.11. Hence `before`/`after`.
#[derive(Debug, Default)]
pub struct Dispatched {
    /// Events that must precede the reply on the wire.
    pub before: Vec<Outgoing>,
    pub reply: Outgoing,
    /// Events that follow the reply (e.g. `browsingContext.load` after `navigate`).
    pub events: Vec<Outgoing>,
}

impl Default for Outgoing {
    fn default() -> Self {
        Outgoing::success(0, Value::Null)
    }
}

/// A command's outcome before it is wrapped in a wire envelope.
struct Reply {
    result: Value,
    /// Events the spec requires *before* the reply (see [`Dispatched`]).
    before: Vec<Outgoing>,
    after: Vec<Outgoing>,
}

impl Reply {
    fn plain(result: Value) -> Self {
        Reply { result, before: Vec::new(), after: Vec::new() }
    }
    fn with_before(result: Value, before: Vec<Outgoing>) -> Self {
        Reply { result, before, after: Vec::new() }
    }
}

/// One browsing context (a "tab"): an id plus the headless browser driving it.
struct Context {
    browser: AgentBrowser,
}

/// A BiDi session: a set of browsing contexts plus the client's event subscriptions.
pub struct Session {
    contexts: HashMap<String, Context>,
    /// Insertion order, so `browsingContext.getTree` is stable.
    order: Vec<String>,
    subscriptions: Vec<String>,
    next_context: u64,
    next_navigation: u64,
    session_id: Option<String>,
    viewport: (u32, u32),
}

impl Session {
    pub fn new(width: u32, height: u32) -> Self {
        Session {
            contexts: HashMap::new(),
            order: Vec::new(),
            subscriptions: Vec::new(),
            next_context: 0,
            next_navigation: 0,
            session_id: None,
            viewport: (width, height),
        }
    }

    pub fn context_ids(&self) -> &[String] {
        &self.order
    }

    /// Whether the client subscribed to `method` (exactly, or to its whole module —
    /// `session.subscribe {events:["log"]}` covers `log.entryAdded`).
    fn subscribed(&self, method: &str) -> bool {
        let module = method.split('.').next().unwrap_or(method);
        self.subscriptions
            .iter()
            .any(|s| s == method || s == module)
    }

    fn new_context(&mut self) -> String {
        let id = format!("ctx-{}", self.next_context);
        self.next_context += 1;
        let (w, h) = self.viewport;
        self.contexts.insert(
            id.clone(),
            Context {
                browser: AgentBrowser::new(w, h),
            },
        );
        self.order.push(id.clone());
        id
    }

    fn browser_mut(&mut self, ctx: &str) -> Result<&mut AgentBrowser, BidiError> {
        self.contexts
            .get_mut(ctx)
            .map(|c| &mut c.browser)
            .ok_or_else(|| BidiError::new(ErrorCode::NoSuchFrame, format!("no such context: {ctx}")))
    }

    fn browser(&self, ctx: &str) -> Result<&AgentBrowser, BidiError> {
        self.contexts
            .get(ctx)
            .map(|c| &c.browser)
            .ok_or_else(|| BidiError::new(ErrorCode::NoSuchFrame, format!("no such context: {ctx}")))
    }

    /// Dispatch one command. Never panics; protocol errors become `Outgoing::Error`.
    pub async fn dispatch(&mut self, cmd: Command) -> Dispatched {
        let id = cmd.id;
        match self.run(&cmd).await {
            Ok(Reply { result, before, after }) => Dispatched {
                before,
                reply: Outgoing::success(id, result),
                events: after,
            },
            Err(e) => Dispatched {
                before: Vec::new(),
                reply: Outgoing::error(id, &e),
                events: Vec::new(),
            },
        }
    }

    async fn run(&mut self, cmd: &Command) -> Result<Reply, BidiError> {
        let p = &cmd.params;
        match cmd.method.as_str() {
            // ---- session ----
            "session.status" => Ok(Reply::plain(json!({"ready": true, "message": "manuk BiDi remote end ready"}))),
            "session.new" => {
                let sid = "manuk-session-1".to_string();
                self.session_id = Some(sid.clone());
                // A session always starts with one context, as browsers do.
                if self.order.is_empty() {
                    self.new_context();
                }
                Ok(Reply::plain(json!({
                    "sessionId": sid,
                    "capabilities": {
                        "browserName": "manuk",
                        "browserVersion": env!("CARGO_PKG_VERSION"),
                        "acceptInsecureCerts": false,
                        "setWindowRect": false,
                        "userAgent": manuk_agent::user_agent(),
                    }
                })))
            }
            "session.end" => {
                self.session_id = None;
                self.contexts.clear();
                self.order.clear();
                self.subscriptions.clear();
                Ok(Reply::plain(json!({})))
            }
            "session.subscribe" => {
                for e in str_array(p, "events")? {
                    if !self.subscriptions.contains(&e) {
                        self.subscriptions.push(e);
                    }
                }
                Ok(Reply::plain(json!({})))
            }
            "session.unsubscribe" => {
                let drop: Vec<String> = str_array(p, "events")?;
                self.subscriptions.retain(|s| !drop.contains(s));
                Ok(Reply::plain(json!({})))
            }

            // ---- browser ----
            // Manuk has no user-context (profile) partitioning wired into BiDi yet, so
            // it reports exactly the one "default" user context that always exists.
            "browser.getUserContexts" => Ok(Reply::plain(json!({"userContexts": [{"userContext": "default"}]}))),
            "browser.getClientWindows" => Ok(Reply::plain(json!({"clientWindows": []}))),
            "browser.close" => {
                self.contexts.clear();
                self.order.clear();
                Ok(Reply::plain(json!({})))
            }

            // ---- browsingContext ----
            "browsingContext.getTree" => {
                let contexts: Vec<Value> = self
                    .order
                    .iter()
                    .map(|id| {
                        let b = &self.contexts[id].browser;
                        context_info(id, b.current_url().unwrap_or("about:blank"))
                    })
                    .collect();
                Ok(Reply::plain(json!({ "contexts": contexts })))
            }
            "browsingContext.create" => {
                let id = self.new_context();
                // The event MUST precede the reply (see `Dispatched`), and must carry the
                // full `browsingContext.Info`: clients filter on `parent` (top-level
                // contexts only) and on `userContext`, silently dropping events missing
                // them — which then look like "the context was never created".
                let ev = if self.subscribed("browsingContext.contextCreated") {
                    vec![Outgoing::event(
                        "browsingContext.contextCreated",
                        context_info(&id, "about:blank"),
                    )]
                } else {
                    vec![]
                };
                Ok(Reply::with_before(json!({ "context": id }), ev))
            }
            "browsingContext.close" => {
                let ctx = str_param(p, "context")?;
                if self.contexts.remove(&ctx).is_none() {
                    return Err(BidiError::new(
                        ErrorCode::NoSuchFrame,
                        format!("no such context: {ctx}"),
                    ));
                }
                self.order.retain(|c| c != &ctx);
                let ev = if self.subscribed("browsingContext.contextDestroyed") {
                    vec![Outgoing::event(
                        "browsingContext.contextDestroyed",
                        json!({"context": ctx}),
                    )]
                } else {
                    vec![]
                };
                Ok(Reply::with_before(json!({}), ev))
            }
            "browsingContext.navigate" => {
                let ctx = str_param(p, "context")?;
                let url = str_param(p, "url")?;
                let nav = format!("nav-{}", self.next_navigation);
                self.next_navigation += 1;

                // `navigationStarted` must precede the reply: a client creates its
                // Navigation object from that event and then matches the subsequent
                // `domContentLoaded`/`load` against its `navigation` id. Without it the
                // client's `goto` never resolves.
                let mut before = Vec::new();
                if self.subscribed("browsingContext.navigationStarted") {
                    before.push(Outgoing::event(
                        "browsingContext.navigationStarted",
                        nav_info(&ctx, &nav, &url),
                    ));
                }

                let browser = self.browser_mut(&ctx)?;
                browser
                    .navigate(&url)
                    .await
                    .map_err(|e| BidiError::new(ErrorCode::UnknownError, format!("{e:#}")))?;
                let landed = browser.current_url().unwrap_or(&url).to_string();

                let mut events = Vec::new();
                // We have no incremental parser hook here, so the document is complete
                // by the time `navigate` returns: both readiness states fire together.
                // Each must carry the `navigation` id, or a client ignores it.
                if self.subscribed("browsingContext.domContentLoaded") {
                    events.push(Outgoing::event(
                        "browsingContext.domContentLoaded",
                        nav_info(&ctx, &nav, &landed),
                    ));
                }
                if self.subscribed("browsingContext.load") {
                    events.push(Outgoing::event(
                        "browsingContext.load",
                        nav_info(&ctx, &nav, &landed),
                    ));
                }
                // No per-request network hooks are wired into the remote end yet, so a
                // navigation reports one synthetic responseCompleted for the document
                // itself. It is honest about what it is: the document load.
                if self.subscribed("network.responseCompleted") {
                    events.push(Outgoing::event(
                        "network.responseCompleted",
                        json!({
                            "context": ctx,
                            "navigation": nav,
                            "timestamp": now_ms(),
                            "isBlocked": false,
                            "redirectCount": 0,
                            "request": {"request": nav, "url": landed, "method": "GET",
                                        "headers": [], "cookies": [], "timings": {}},
                        }),
                    ));
                }
                Ok(Reply {
                    result: json!({"navigation": nav, "url": landed}),
                    before,
                    after: events,
                })
            }
            "browsingContext.captureScreenshot" => {
                let ctx = str_param(p, "context")?;
                let png = self
                    .browser(&ctx)?
                    .screenshot_png()
                    .map_err(|e| BidiError::new(ErrorCode::UnknownError, format!("{e:#}")))?;
                let data = base64::engine::general_purpose::STANDARD.encode(png);
                Ok(Reply::plain(json!({ "data": data })))
            }
            "browsingContext.setViewport" => {
                let ctx = str_param(p, "context")?;
                // A null viewport means "reset to the default"; we keep the current one.
                if let Some(v) = p.get("viewport").and_then(Value::as_object) {
                    let w = v.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
                    let h = v.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
                    if w == 0 || h == 0 {
                        return Err(BidiError::invalid("viewport width/height must be > 0"));
                    }
                    self.browser_mut(&ctx)?.set_viewport(w, h);
                }
                Ok(Reply::plain(json!({})))
            }
            "browsingContext.traverseHistory" => {
                let ctx = str_param(p, "context")?;
                let delta = p.get("delta").and_then(Value::as_i64).unwrap_or(0);
                let browser = self.browser_mut(&ctx)?;
                let res = if delta < 0 {
                    browser.back().await
                } else if delta > 0 {
                    browser.forward().await
                } else {
                    Ok(())
                };
                res.map_err(|e| BidiError::new(ErrorCode::UnknownError, format!("{e:#}")))?;
                Ok(Reply::plain(json!({})))
            }

            // ---- input (pointer click only) ----
            "input.performActions" => {
                let ctx = str_param(p, "context")?;
                let (x, y) = pointer_click_target(p)?;
                let browser = self.browser_mut(&ctx)?;
                browser
                    .click_at(x, y)
                    .await
                    .map_err(|e| BidiError::new(ErrorCode::UnknownError, format!("{e:#}")))?;
                Ok(Reply::plain(json!({})))
            }

            // ---- deferred, and said so ----
            "script.evaluate" | "script.callFunction" => Err(BidiError::new(
                ErrorCode::UnsupportedOperation,
                "script.* requires the JS engine (build with --features spidermonkey); \
                 not wired into the BiDi remote end yet",
            )),

            other => Err(BidiError::new(
                ErrorCode::UnknownCommand,
                format!("unknown command: {other}"),
            )),
        }
    }
}

/// Milliseconds since the Unix epoch — BiDi timestamps are numbers, and clients feed
/// them straight to `new Date(...)`.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The spec's `browsingContext.NavigationInfo`, shared by `navigationStarted`,
/// `domContentLoaded`, and `load`. The `navigation` id is what a client matches on.
fn nav_info(context: &str, navigation: &str, url: &str) -> Value {
    json!({
        "context": context,
        "navigation": navigation,
        "url": url,
        "timestamp": now_ms(),
    })
}

/// The spec's `browsingContext.Info` object. Clients parse every field, so a partial
/// object (missing `parent`/`userContext`) makes them reject the context.
fn context_info(id: &str, url: &str) -> Value {
    json!({
        "context": id,
        "url": url,
        "children": [],
        "parent": Value::Null,
        "userContext": "default",
    })
}

fn obj(v: &Value) -> Result<&Map<String, Value>, BidiError> {
    v.as_object()
        .ok_or_else(|| BidiError::invalid("params must be an object"))
}

fn str_param(p: &Value, key: &str) -> Result<String, BidiError> {
    obj(p)?
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| BidiError::invalid(format!("missing string param {key:?}")))
}

fn str_array(p: &Value, key: &str) -> Result<Vec<String>, BidiError> {
    obj(p)?
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| BidiError::invalid(format!("missing array param {key:?}")))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| BidiError::invalid(format!("{key} must contain strings")))
        })
        .collect()
}

/// Extract the `(x, y)` of a pointer click from a BiDi `input.performActions` payload.
///
/// We support the common shape a client emits for "click here": a pointer source whose
/// action list contains a `pointerMove` (giving the coordinate) followed by
/// `pointerDown`/`pointerUp`. Anything else is refused rather than guessed at.
fn pointer_click_target(p: &Value) -> Result<(f32, f32), BidiError> {
    let sources = obj(p)?
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| BidiError::invalid("missing `actions` array"))?;

    for src in sources {
        if src.get("type").and_then(Value::as_str) != Some("pointer") {
            continue;
        }
        let list = src
            .get("actions")
            .and_then(Value::as_array)
            .ok_or_else(|| BidiError::invalid("pointer source has no `actions`"))?;

        let mut at: Option<(f32, f32)> = None;
        let mut pressed = false;
        let mut released = false;
        for a in list {
            match a.get("type").and_then(Value::as_str) {
                Some("pointerMove") => {
                    let x = a.get("x").and_then(Value::as_f64);
                    let y = a.get("y").and_then(Value::as_f64);
                    if let (Some(x), Some(y)) = (x, y) {
                        at = Some((x as f32, y as f32));
                    }
                }
                Some("pointerDown") => pressed = true,
                Some("pointerUp") => released = true,
                _ => {}
            }
        }
        if pressed && released {
            return at.ok_or_else(|| {
                BidiError::invalid("pointer click has no pointerMove supplying x/y")
            });
        }
    }
    Err(BidiError::new(
        ErrorCode::UnsupportedOperation,
        "only a pointerMove + pointerDown + pointerUp click is supported",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(id: u64, method: &str, params: Value) -> Command {
        Command {
            id,
            method: method.to_string(),
            params,
        }
    }

    fn result_of(d: &Dispatched) -> &Value {
        match &d.reply {
            Outgoing::Success { result, .. } => result,
            other => panic!("expected success, got {other:?}"),
        }
    }

    fn error_of(d: &Dispatched) -> (&str, &str) {
        match &d.reply {
            Outgoing::Error { error, message, .. } => (error, message),
            other => panic!("expected error, got {other:?}"),
        }
    }

    fn data_url(html: &str) -> String {
        format!("data:text/html,{html}")
    }

    #[tokio::test]
    async fn session_new_creates_a_context_and_reports_capabilities() {
        let mut s = Session::new(800, 600);
        let d = s.dispatch(cmd(1, "session.new", json!({}))).await;
        let r = result_of(&d);
        assert_eq!(r["capabilities"]["browserName"], "manuk");
        assert!(r["sessionId"].is_string());
        assert_eq!(s.context_ids().len(), 1);

        // The reply is a BiDi `success` envelope carrying the same id.
        let wire: Value = serde_json::from_str(&d.reply.to_json()).unwrap();
        assert_eq!(wire["type"], "success");
        assert_eq!(wire["id"], 1);
    }

    #[tokio::test]
    async fn status_works_before_a_session_exists() {
        let mut s = Session::new(800, 600);
        let d = s.dispatch(cmd(1, "session.status", json!({}))).await;
        assert_eq!(result_of(&d)["ready"], true);
    }

    #[tokio::test]
    async fn navigate_then_get_tree_reports_the_landed_url() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        let url = data_url("<title>T</title><body><h1>Hi</h1></body>");

        let d = s
            .dispatch(cmd(2, "browsingContext.navigate", json!({"context": ctx, "url": url})))
            .await;
        assert_eq!(result_of(&d)["url"], url);
        assert!(result_of(&d)["navigation"].is_string());

        let d = s.dispatch(cmd(3, "browsingContext.getTree", json!({}))).await;
        let contexts = result_of(&d)["contexts"].as_array().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0]["context"], ctx);
        assert_eq!(contexts[0]["url"], url);
    }

    /// Events only flow after `session.subscribe`, and a module subscription
    /// (`"browsingContext"`) covers its events.
    #[tokio::test]
    async fn load_event_is_emitted_only_when_subscribed() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        let url = data_url("<body>x</body>");

        let d = s
            .dispatch(cmd(2, "browsingContext.navigate", json!({"context": ctx, "url": url})))
            .await;
        assert!(d.events.is_empty(), "no subscription => no events");

        s.dispatch(cmd(3, "session.subscribe", json!({"events": ["browsingContext.load"]})))
            .await;
        let d = s
            .dispatch(cmd(4, "browsingContext.navigate", json!({"context": ctx, "url": url})))
            .await;
        assert_eq!(d.events.len(), 1);
        match &d.events[0] {
            Outgoing::Event { method, params, .. } => {
                assert_eq!(method, "browsingContext.load");
                assert_eq!(params["context"], ctx);
            }
            other => panic!("expected event, got {other:?}"),
        }

        // Unsubscribing stops them again.
        s.dispatch(cmd(5, "session.unsubscribe", json!({"events": ["browsingContext.load"]})))
            .await;
        let d = s
            .dispatch(cmd(6, "browsingContext.navigate", json!({"context": ctx, "url": url})))
            .await;
        assert!(d.events.is_empty());
    }

    #[tokio::test]
    async fn module_level_subscription_covers_its_events() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        s.dispatch(cmd(2, "session.subscribe", json!({"events": ["browsingContext"]})))
            .await;
        let d = s
            .dispatch(cmd(
                3,
                "browsingContext.navigate",
                json!({"context": ctx, "url": data_url("<body>y</body>")}),
            ))
            .await;
        // A module subscription covers BOTH readiness events.
        assert_eq!(d.events.len(), 2);
        let methods: Vec<&str> = d
            .events
            .iter()
            .map(|e| match e {
                Outgoing::Event { method, .. } => method.as_str(),
                _ => panic!("expected events"),
            })
            .collect();
        assert_eq!(
            methods,
            vec!["browsingContext.domContentLoaded", "browsingContext.load"]
        );
        // `navigationStarted` precedes the reply, so a client can match the id.
        assert!(matches!(
            d.before.first(),
            Some(Outgoing::Event { method, .. }) if method == "browsingContext.navigationStarted"
        ));
    }

    #[tokio::test]
    async fn create_and_close_contexts() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let d = s.dispatch(cmd(2, "browsingContext.create", json!({"type":"tab"}))).await;
        let new_ctx = result_of(&d)["context"].as_str().unwrap().to_string();
        assert_eq!(s.context_ids().len(), 2);

        s.dispatch(cmd(3, "browsingContext.close", json!({"context": new_ctx})))
            .await;
        assert_eq!(s.context_ids().len(), 1);

        // Closing it twice is `no such frame`, not a panic.
        let d = s
            .dispatch(cmd(4, "browsingContext.close", json!({"context": new_ctx})))
            .await;
        assert_eq!(error_of(&d).0, "no such frame");
    }

    #[tokio::test]
    async fn capture_screenshot_returns_base64_png() {
        let mut s = Session::new(200, 100);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        s.dispatch(cmd(
            2,
            "browsingContext.navigate",
            json!({"context": ctx, "url": data_url("<body><h1>Hi</h1></body>")}),
        ))
        .await;

        let d = s
            .dispatch(cmd(3, "browsingContext.captureScreenshot", json!({"context": ctx})))
            .await;
        let b64 = result_of(&d)["data"].as_str().unwrap();
        let png = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        // Real PNG magic, not a placeholder.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[tokio::test]
    async fn input_perform_actions_clicks_at_a_coordinate() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        let dest = data_url("<body><h1>Landed</h1></body>");
        let html = format!(r#"<body><a href="{dest}">Go</a></body>"#);
        s.dispatch(cmd(
            2,
            "browsingContext.navigate",
            json!({"context": ctx, "url": data_url(&html)}),
        ))
        .await;

        // The link's click point, from the a11y tree the agent shares.
        let (x, y) = {
            let b = s.browser(&ctx).unwrap();
            let tree = b.a11y_tree().unwrap();
            tree.find(&manuk_agent::A11yRole::Link, "Go")
                .unwrap()
                .bbox
                .unwrap()
                .center()
        };

        let d = s
            .dispatch(cmd(
                3,
                "input.performActions",
                json!({"context": ctx, "actions": [{
                    "type": "pointer", "id": "mouse",
                    "actions": [
                        {"type":"pointerMove","x": x, "y": y},
                        {"type":"pointerDown","button":0},
                        {"type":"pointerUp","button":0}
                    ]
                }]}),
            ))
            .await;
        assert!(matches!(d.reply, Outgoing::Success { .. }), "{:?}", d.reply);

        // The click really navigated the context.
        let d = s.dispatch(cmd(4, "browsingContext.getTree", json!({}))).await;
        assert_eq!(result_of(&d)["contexts"][0]["url"], dest);
    }

    #[tokio::test]
    async fn unsupported_and_unknown_commands_report_proper_error_codes() {
        let mut s = Session::new(800, 600);
        let d = s.dispatch(cmd(1, "script.evaluate", json!({}))).await;
        assert_eq!(error_of(&d).0, "unsupported operation");

        let d = s.dispatch(cmd(2, "nonsense.command", json!({}))).await;
        assert_eq!(error_of(&d).0, "unknown command");

        let d = s.dispatch(cmd(3, "browsingContext.navigate", json!({}))).await;
        assert_eq!(error_of(&d).0, "invalid argument");

        // The error envelope carries the id, as BiDi requires.
        let wire: Value = serde_json::from_str(&d.reply.to_json()).unwrap();
        assert_eq!(wire["type"], "error");
        assert_eq!(wire["id"], 3);
    }

    #[tokio::test]
    async fn navigating_a_missing_context_is_no_such_frame() {
        let mut s = Session::new(800, 600);
        let d = s
            .dispatch(cmd(
                1,
                "browsingContext.navigate",
                json!({"context": "ctx-999", "url": "data:text/html,x"}),
            ))
            .await;
        assert_eq!(error_of(&d).0, "no such frame");
    }

    #[tokio::test]
    async fn traverse_history_goes_back() {
        let mut s = Session::new(800, 600);
        s.dispatch(cmd(1, "session.new", json!({}))).await;
        let ctx = s.context_ids()[0].clone();
        let a = data_url("<body>AAA</body>");
        let b = data_url("<body>BBB</body>");
        s.dispatch(cmd(2, "browsingContext.navigate", json!({"context": ctx, "url": a})))
            .await;
        s.dispatch(cmd(3, "browsingContext.navigate", json!({"context": ctx, "url": b})))
            .await;

        let d = s
            .dispatch(cmd(4, "browsingContext.traverseHistory", json!({"context": ctx, "delta": -1})))
            .await;
        assert!(matches!(d.reply, Outgoing::Success { .. }));
        let d = s.dispatch(cmd(5, "browsingContext.getTree", json!({}))).await;
        assert_eq!(result_of(&d)["contexts"][0]["url"], a);
    }
}
