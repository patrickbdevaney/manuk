//! W4.E2.b.a probe: load one URL and print the number of <img> elements in the DOM after navigation
//! and JS (`img_post_js=N`). The profile comes from `MANUK_AGENT_PROFILE` (unset = full, `lean`).
//! Usage: `img_count <url>`. Used by scripts/w4e2b-memory.py for the img column of summary.tsv.
fn main() {
    let url = std::env::args().nth(1).expect("usage: img_count <url>");
    let rt = manuk_net::runtime();
    rt.block_on(async {
        let mut b = manuk_agent::AgentBrowser::new(1280, 800);
        if let Err(e) = b.navigate(&url).await {
            println!(
                "ok=false err={}",
                e.to_string().replace(char::is_whitespace, "_")
            );
            return;
        }
        match b
            .eval_value("document.querySelectorAll('img').length")
            .await
        {
            Ok(v) => println!("ok=true img_post_js={}", v.trim()),
            Err(e) => println!(
                "ok=false err={}",
                e.to_string().replace(char::is_whitespace, "_")
            ),
        }
    });
}
