//! R0.4.a.a.a.b.a probe: one document fetch through manuk's OWN network stack (`manuk_net::fetch_document`,
//! the navigation request the engine makes), printed as an HTTP status line plus the facts needed to
//! tell an empty origin answer from a real document. Usage: `m1_net_probe <url>`.
//! Not a gate by itself — `scripts/m1-amazon-net-check.sh` is the gate over the saved output.
fn main() {
    let url = std::env::args().nth(1).expect("usage: m1_net_probe <url>");
    let rt = manuk_net::runtime();
    match rt.block_on(manuk_net::fetch_document(&url)) {
        Ok(resp) => {
            let version = match resp.http_version {
                manuk_net::HttpVersion::Http2 => "2",
                manuk_net::HttpVersion::Http3 => "3",
                manuk_net::HttpVersion::Http10 => "1.0",
                manuk_net::HttpVersion::Http09 => "0.9",
                _ => "1.1",
            };
            println!("HTTP/{version} {} (manuk-net fetch_document)", resp.status);
            println!("request_url: {url}");
            println!("final_url: {}", resp.final_url);
            println!("body_bytes: {}", resp.body.len());
            println!("content_type: {}", resp.header("content-type").unwrap_or("-"));
            println!("server: {}", resp.header("server").unwrap_or("-"));
            println!("user_agent: {}", manuk_net::user_agent());
        }
        Err(e) => {
            println!("TRANSPORT_ERROR: {e:#}");
            std::process::exit(2);
        }
    }
}
