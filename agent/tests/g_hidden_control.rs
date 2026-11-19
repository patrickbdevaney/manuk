//! **G_HIDDEN_CONTROL — W4.D3.a.a.b: a form field a human cannot see is not offered to the agent.**
//!
//! The EIA mirror page carries a look-alike `<input style="opacity:.02">`; manuk filled it 47-50% of
//! the time vs 12-30% on Chrome arms, because the shield judged only text runs. Mutant
//! (MANUK_HIDDEN_CONTROL_MUTATE=1) restores the old behaviour and must turn this red.

use manuk_agent::AgentBrowser;

const PAGE: &str = r#"<!doctype html><html><body><form>
<label>Email <input id="real" name="email" type="email" aria-label="Email real"></label>
<label>Email <input id="dup" name="email" type="email" aria-label="Email dup" style="opacity:.02"></label>
<label>Zero <input id="z" aria-label="Email zero" style="width:0;height:0;padding:0;border:0"></label>
<label>Off <input id="o" aria-label="Email off" style="position:absolute;left:-5000px"></label>
<label>Faint <input id="f" aria-label="Email faint" style="opacity:.6"></label>
<input type="hidden" name="tok" value="1">
</form></body></html>"#;

fn serve() -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let mut tmp = [0u8; 4096];
            let _ = s.read(&mut tmp);
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}", PAGE.len()).as_bytes());
        }
    });
    format!("http://{addr}/")
}

fn invisible_controls_are_withheld_visible_kept_inner() {
    let url = serve();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(&url).await.expect("load");
        let sh = b.shield().unwrap();
        let tree = b.a11y_tree().unwrap();
        let withheld: Vec<String> = tree
            .iter()
            .filter(|n| sh.nodes.contains_key(&n.node))
            .map(|n| n.name.clone())
            .collect();
        for want in ["Email dup", "Email zero", "Email off"] {
            assert!(
                withheld.iter().any(|i| i == want),
                "{want} not withheld: {withheld:?}"
            );
        }
        for keep in ["Email real", "Email faint"] {
            assert!(
                !withheld.iter().any(|i| i == keep),
                "{keep} wrongly withheld: {withheld:?}"
            );
        }
    });
}

// One test: SpiderMonkey runtimes must not be created concurrently in one process.
fn visible_email_field_takes_text() {
    let url = serve();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut b = AgentBrowser::new(1024, 768);
        b.navigate(&url).await.expect("load");
        b.type_into("Email real", "a@b.test").unwrap();
        let txt = b.observe().unwrap().text;
        assert!(txt.contains("a@b.test"), "typed text not shown: {txt}");
    });
}

#[test]
fn hidden_controls_withheld_and_visible_email_takes_text() {
    invisible_controls_are_withheld_visible_kept_inner();
    visible_email_field_takes_text();
}
