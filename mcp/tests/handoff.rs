//! R3.3 gate: `handoff(reason, timeout_ms)` round-trips state through a scripted human. The agent stops, the
//! human (a `HumanSurface` that acts on the SAME live page) logs in, and the agent resumes with an effect
//! diff of what the human changed. A human that does nothing yields "no change"; one that never resumes
//! yields TIMED OUT; a browser with no human surface errors rather than pretending.

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_agent::{AgentBrowser, HandoffRequest, HumanSurface, HumanTurn};
use manuk_mcp::{Output, Session};
use serde_json::{json, Map, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Gate</title></head><body>
<main id="m"><h1>Sign in required</h1><input id="pw" type="password" aria-label="Password">
<button id="go">Continue</button></main>
<script>window.signin=function(){document.getElementById('m').innerHTML='<h1>Welcome back, Pat</h1><a href="/acct">Account settings</a>';};</script>
</body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
            });
        }
    });
    format!("http://{addr}/")
}

/// The scripted human: runs `js` in the page, then resumes (or never does).
struct Human {
    js: &'static str,
    ends: HumanTurn,
    seen: std::rc::Rc<std::cell::RefCell<Vec<HandoffRequest>>>,
}

#[async_trait::async_trait(?Send)]
impl HumanSurface for Human {
    async fn turn(&mut self, b: &mut AgentBrowser, req: &HandoffRequest) -> HumanTurn {
        self.seen.borrow_mut().push(req.clone());
        if !self.js.is_empty() {
            b.eval_value(self.js).await.unwrap();
        }
        self.ends.clone()
    }
}

fn args(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

async fn text(s: &mut Session<AgentBrowser>, tool: &str, a: Value) -> anyhow::Result<String> {
    match s.call(tool, &args(a)).await? {
        Output::Text(t) => Ok(t),
        _ => anyhow::bail!("non-text output"),
    }
}

async fn session(url: &str, human: Option<Human>) -> Session<AgentBrowser> {
    let mut b = AgentBrowser::new(800, 600);
    if let Some(h) = human {
        b.set_human(Box::new(h));
    }
    let mut s = Session::new(b);
    text(&mut s, "navigate", json!({"url": url})).await.unwrap();
    s
}

// One test fn: the engine (SpiderMonkey, taint ledger) is per-process state, so scenarios run in sequence.
#[tokio::test(flavor = "current_thread")]
async fn handoff_round_trips() {
    let url = serve();
    human_logs_in_and_agent_resumes_with_the_diff(&url).await;
    idle_human_is_no_change_and_timeout_is_reported(&url).await;
    no_human_surface_errors(&url).await;
}

async fn human_logs_in_and_agent_resumes_with_the_diff(url: &str) {
    let seen = Default::default();
    let human = Human {
        js: "signin()",
        ends: HumanTurn::Resumed,
        seen: std::rc::Rc::clone(&seen),
    };
    let mut s = session(url, Some(human)).await;
    let r = text(
        &mut s,
        "handoff",
        json!({"reason": "log in to Gate", "timeout_ms": 5000}),
    )
    .await
    .unwrap();
    assert!(r.contains("handoff(\"log in to Gate\"): resumed"), "{r}");
    assert!(
        r.contains("Welcome back, Pat"),
        "diff must show what the human did: {r}"
    );
    assert!(
        r.contains("-") && r.contains("Sign in required"),
        "and what went away: {r}"
    );
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].reason.as_str(), seen[0].timeout_ms),
        ("log in to Gate", 5000)
    );
    // the agent's next observation is of the human's page state
    let o = text(&mut s, "observe", json!({})).await.unwrap();
    assert!(o.contains("Account settings"), "{o}");
}

async fn idle_human_is_no_change_and_timeout_is_reported(url: &str) {
    let idle = Human {
        js: "",
        ends: HumanTurn::Resumed,
        seen: Default::default(),
    };
    let mut s = session(url, Some(idle)).await;
    let r = text(&mut s, "handoff", json!({"reason": "x"}))
        .await
        .unwrap();
    assert!(r.contains("no change"), "{r}");
    assert!(!r.contains("Welcome"), "{r}");

    let slow = Human {
        js: "signin()",
        ends: HumanTurn::TimedOut,
        seen: Default::default(),
    };
    let mut s = session(url, Some(slow)).await;
    let r = text(&mut s, "handoff", json!({"reason": "x", "timeout_ms": 50}))
        .await
        .unwrap();
    assert!(r.contains("TIMED OUT after 50 ms"), "{r}");
}

async fn no_human_surface_errors(url: &str) {
    let mut s = session(url, None).await;
    let e = text(&mut s, "handoff", json!({"reason": "x"}))
        .await
        .unwrap_err();
    assert!(e.to_string().contains("no human surface"), "{e}");
    let e = text(&mut s, "handoff", json!({})).await.unwrap_err();
    assert!(e.to_string().contains("needs `reason`"), "{e}");
}
