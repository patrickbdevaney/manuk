//! R3.4.a: `manuk-mcp --zygote-after-load <sock> <url>` loads a page once, quiesces SpiderMonkey and
//! forks one session per connection. Each session must (1) already have the page, (2) act
//! independently of its siblings, and (3) roll its own dice, including through a `Math.random` the
//! page cached BEFORE the fork. Gate: `scripts/r3-4a-fork-gate.sh` runs the 64-way version on a big page.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};

struct Client {
    tx: UnixStream,
    rx: BufReader<UnixStream>,
    id: u64,
}

impl Client {
    fn connect(sock: &std::path::Path) -> Client {
        let tx = UnixStream::connect(sock).expect("connect zygote");
        let rx = BufReader::new(tx.try_clone().unwrap());
        let mut c = Client { tx, rx, id: 0 };
        let init = c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
        assert!(
            init["result"]["capabilities"]["tools"].is_object(),
            "{init}"
        );
        c.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        writeln!(self.tx, "{v}").unwrap();
        self.tx.flush().unwrap();
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            assert!(self.rx.read_line(&mut line).unwrap() > 0, "session closed");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let text = res["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (res["isError"].as_bool().unwrap_or(false), text)
    }
}

struct Zygote(Child);
impl Drop for Zygote {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} in:\n{text}"));
    let start = line.find("[h").unwrap();
    let end = line[start..].find(']').unwrap() + start;
    line[start + 1..end].to_string()
}

fn roll(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains("Roll: ") && l.contains(" at "))
        .unwrap_or_else(|| panic!("no roll in:\n{text}"));
    let after = &line[line.find("Roll: ").unwrap() + 6..];
    after.split_once(" at ").unwrap().0.trim().to_string()
}

#[test]
fn sessions_forked_from_a_loaded_page_start_on_it_and_roll_distinct_dice() {
    let dir = std::env::temp_dir().join(format!("manuk-zal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock = dir.join("z.sock");
    let page = dir.join("dice.html");
    // The page caches Math.random and draws from the parent's stream BEFORE the fork.
    std::fs::write(&page, std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dice.html")).unwrap()
        .replace("</body>", "<script>window.__mr=Math.random;__mr();__mr();</script>\n<button id=\"cache\" onclick=\"document.getElementById('out').textContent='Roll: '+window.__mr()+' at '+Date.now();\">Cached</button></body>")).unwrap();
    let url = format!("file://{}", page.display());
    let z = Zygote(
        Command::new(env!("CARGO_BIN_EXE_manuk-mcp"))
            .args(["--no-confine", "--zygote-after-load"])
            .arg(&sock)
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn zygote"),
    );
    for _ in 0..600 {
        if sock.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(sock.exists(), "zygote never became ready");
    let mut sessions: Vec<Client> = (0..4).map(|_| Client::connect(&sock)).collect();
    let (mut plain, mut cached) = (vec![], vec![]);
    for (i, c) in sessions.iter_mut().enumerate() {
        // No navigate: the page is already there.
        let (e, obs) = c.tool("observe", json!({"mode": "full"}));
        assert!(
            !e && obs.contains("Dice roller"),
            "session {i} does not have the preloaded page: {obs}"
        );
        let page_id = obs
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix("page: "))
            .unwrap()
            .trim()
            .to_string();
        let h = handle_of(&obs, "Roll");
        let (e, act) = c.tool(
            "act",
            json!({"handle": h, "action": "click", "page": page_id}),
        );
        assert!(!e, "session {i} click: {act}");
        plain.push(roll(&act));
        let page_id = act
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix("page: "))
            .unwrap()
            .trim()
            .to_string();
        let (_, obs) = c.tool("observe", json!({"mode": "full", "page": page_id}));
        let h = handle_of(&obs, "Cached");
        let page_id = obs
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix("page: "))
            .unwrap()
            .trim()
            .to_string();
        let (e, act) = c.tool(
            "act",
            json!({"handle": h, "action": "click", "page": page_id}),
        );
        assert!(!e, "session {i} cached click: {act}");
        cached.push(roll(&act));
    }
    drop(z);
    for (name, v) in [("plain", &plain), ("cached", &cached)] {
        let mut d = v.clone();
        d.sort();
        d.dedup();
        assert_eq!(
            d.len(),
            v.len(),
            "{name} Math.random replayed across sessions: {v:?}"
        );
    }
}
