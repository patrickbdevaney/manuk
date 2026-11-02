//! W3.19 reachability experiment: GET each URL on stdin through the real manuk_net stack, with or without
//! a Web Bot Auth identity, and print `url<TAB>status<TAB>class<TAB>bytes`. class = ok (2xx, no
//! challenge marker) | wall (401/403/429/503 or a challenge page) | other | err.
//!   wba_reach [--sign <jwk> <signature-agent-url>] < urls.txt
use std::io::BufRead;

const MARKERS: &[&str] = &[
    "just a moment",
    "cf-chl",
    "challenge-platform",
    "captcha",
    "access denied",
    "px-captcha",
    "are you a robot",
    "verify you are human",
    "_incapsula_",
    "akamai",
];

fn classify(status: u16, body: &[u8]) -> &'static str {
    let head = String::from_utf8_lossy(&body[..body.len().min(60_000)]).to_lowercase();
    match status {
        401 | 403 | 429 | 503 => "wall",
        200..=299 if MARKERS.iter().any(|m| head.contains(m)) && head.len() < 30_000 => "wall",
        200..=299 => "ok",
        _ => "other",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--sign") {
        manuk_net::botauth::configure(&args[1], &args[2]).expect("identity");
    }
    manuk_net::set_ephemeral(true);
    let urls: Vec<String> = std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .collect();
    let rt = manuk_net::runtime();
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(12));
    let mut tasks = Vec::new();
    for u in urls {
        let sem = sem.clone();
        tasks.push(rt.spawn(async move {
            let _p = sem.acquire().await.unwrap();
            let r = tokio::time::timeout(
                std::time::Duration::from_secs(25),
                manuk_net::fetch_document(&u),
            )
            .await;
            match r {
                Ok(Ok(resp)) => format!(
                    "{u}\t{}\t{}\t{}",
                    resp.status,
                    classify(resp.status, &resp.body),
                    resp.body.len()
                ),
                Ok(Err(_)) => format!("{u}\t0\terr\t0"),
                Err(_) => format!("{u}\t0\terr\t0"),
            }
        }));
    }
    for t in tasks {
        println!("{}", rt.block_on(t).unwrap());
    }
}
