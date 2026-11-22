//! W4.P0.c.a.a: navigate, then settle N rounds, printing the body text head and the page's JSON
//! data responses each round. Usage: `w4p0caa_probe <url> [rounds]`.
fn main() {
    let url = std::env::args()
        .nth(1)
        .expect("usage: w4p0caa_probe <url> [rounds]");
    let rounds: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let rt = manuk_net::runtime();
    rt.block_on(async {
        let mut b = manuk_agent::AgentBrowser::new(1280, 800);
        let r = b.navigate(&url).await;
        println!("navigate ok={}", r.is_ok());
        for i in 0..=rounds {
            let t = b
                .eval_value("document.body.innerText")
                .await
                .unwrap_or_default();
            let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
            let has_ans = t.contains("33038") || t.contains("9675");
            let und = t.contains("doesn't understand");
            let p = b.pending();
            println!(
                "round {i}: len={} answer={has_ans} dontunderstand={und} pending f={} t={} | {}",
                t.len(),
                p.fetches.len(),
                p.timers.len(),
                t.chars().skip(40).take(160).collect::<String>()
            );
            for d in b.data_responses() {
                println!(
                    "  data: {:?}",
                    format!("{:?}", d).chars().take(200).collect::<String>()
                );
            }
            b.settle().await;
        }
    });
}
