//! **G_AGENT_CSSOM_STYLE_WRITE — R2.1.2: `el.style.x = v` takes effect in the agent's browser.**
//!
//! `manuk-agent` built `manuk-page` without `stylo` (or `cssom`), so `install_supports_hook` wired the no-engine
//! stubs: the serialize/expand hooks answered "nothing" and CSSOM writes (`style.display='block'`,
//! `style.setProperty`) never reached the element. Every disclosure widget (dialog, tabs) stayed shut.
//!
//! Mutation that must turn this red: drop `"cssom"` from the `manuk-page` features in agent/Cargo.toml.

use manuk_agent::AgentBrowser;
use std::io::{Read, Write};
use std::net::TcpListener;

const PAGE: &str = r#"<!doctype html><html><body>
<button id="b" onclick="var p=document.getElementById('panel');p.style.display='block';p.style.setProperty('color','red');document.getElementById('out').textContent='computed:'+getComputedStyle(p).display">Open</button>
<div id="panel" style="display:none"><a href="/x">Hidden link</a></div>
<h2 id="out">idle</h2></body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut tmp = [0u8; 4096];
            let _ = s.read(&mut tmp);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

#[test]
fn style_display_write_opens_the_panel() {
    let base = serve();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(&base).await.expect("navigate");
        let names = |b: &AgentBrowser| {
            let t = b.a11y_tree().unwrap();
            manuk_agent::a11y_score::manuk_bag(&t)
                .0
                .into_iter()
                .map(|(_, n)| n)
                .collect::<Vec<_>>()
        };
        assert!(
            !names(&b).iter().any(|n| n.contains("Hidden link")),
            "panel open before click"
        );
        b.click_by_name(&manuk_a11y::Role::Button, "Open")
            .await
            .expect("click");
        let after = names(&b);
        assert!(
            after.iter().any(|n| n.contains("Hidden link")),
            "panel still shut: {after:?}"
        );
        assert!(
            after.iter().any(|n| n.contains("computed:block")),
            "computed display not block: {after:?}"
        );
    });
}
