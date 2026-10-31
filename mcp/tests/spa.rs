//! W4.A4 gate: an observation that disambiguates SPAs, on BOTH backends. Three identical 'Add to
//! cart' buttons name their card; a div with a click handler is marked; a `pushState` route change
//! and a re-render keep handles valid; an ARIA combobox can be selected; and over budget the content
//! text survives while chrome text is cut. Mutants: `MANUK_TREE_MUTATE=flat` (no container context),
//! `MANUK_REMOUNT_MUTATE=1` (no re-bind), `MANUK_FIT_MUTATE=chrome-first`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const PAGE: &str = r#"<!doctype html><html><head><title>Shop</title></head><body>
<nav><p>Chrome-One</p><p>Chrome-Two</p><p>Chrome-Three</p><p>Chrome-Four</p><p>Chrome-Five</p><p>Chrome-Six</p>
<p>Chrome-Seven</p><p>Chrome-Eight</p><p>Chrome-Nine</p><p>Chrome-Ten</p><p>Chrome-Eleven</p><p>Chrome-Twelve</p></nav>
<button id="route">Go to cart</button>
<main>
<p>PRICE-MARKER Teal Kettle costs 19.99</p>
<div id="gift" onclick="document.title='gift'">Gift wrap</div>
<div id="promo">Promo banner</div>
<div id="dd" role="combobox" aria-label="City" aria-haspopup="listbox" aria-expanded="false" tabindex="0">Choose city</div>
<div id="pop" role="listbox" style="display:none"><div role="option">Paris</div><div role="option">Rome</div></div>
<h2 id="o">cart:</h2>
<div id="app"></div>
</main>
<script>
var cart = [];
function render(view) {
  var names = ['Teal Kettle', 'Red Mug', 'Blue Lamp'];
  var html = '<h2>' + view + '</h2><ul aria-label="Products">';
  names.forEach(function (n) { html += '<li><h3>' + n + '</h3><button data-n="' + n + '">Add to cart</button></li>'; });
  document.getElementById('app').innerHTML = html + '</ul>';
}
document.getElementById('app').addEventListener('click', function (e) {
  var n = e.target.getAttribute && e.target.getAttribute('data-n');
  if (n) { cart.push(n); document.getElementById('o').textContent = 'cart:' + cart.join(','); }
});
render('Products');
document.getElementById('route').addEventListener('click', function () {
  history.pushState({}, '', '/cart'); render('Cart');
});
document.getElementById('promo').addEventListener('click', function () {});
var dd = document.getElementById('dd'), pop = document.getElementById('pop');
dd.addEventListener('click', function () { pop.style.display = 'block'; dd.setAttribute('aria-expanded', 'true'); });
Array.prototype.forEach.call(pop.children, function (o) {
  o.addEventListener('click', function () { dd.textContent = o.textContent; pop.style.display = 'none'; dd.setAttribute('aria-expanded', 'false'); });
});
</script></body></html>"#;

/// Chrome text first in page order, then the content: an order-based cut would keep the wrong half.
const BUDGET_PAGE: &str = r#"<!doctype html><html><head><title>Budget</title></head><body>
<nav><p>Chrome-One lorem ipsum dolor</p><p>Chrome-Two lorem ipsum dolor</p><p>Chrome-Three lorem ipsum dolor</p>
<p>Chrome-Four lorem ipsum dolor</p><p>Chrome-Five lorem ipsum dolor</p><p>Chrome-Six lorem ipsum dolor</p>
<p>Chrome-Seven lorem ipsum dolor</p><p>Chrome-Eight lorem ipsum dolor</p><p>Chrome-Nine lorem ipsum dolor</p>
<p>Chrome-Ten lorem ipsum dolor</p><p>Chrome-Eleven lorem ipsum dolor</p><p>Chrome-Twelve lorem ipsum dolor</p></nav>
<main><h1>Kettles</h1><p>PRICE-MARKER Teal Kettle costs 19.99</p><button>Buy</button></main></body></html>"#;

fn serve() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let body = if buf[..n].starts_with(b"GET /budget") {
                    BUDGET_PAGE.as_bytes()
                } else {
                    PAGE.as_bytes()
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body);
            });
        }
    });
    format!("http://{addr}/")
}

struct Client {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(chrome: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_manuk-mcp"));
        if chrome {
            cmd.args(["--backend", "chrome"]);
        }
        cmd.env(
            "MANUK_ACTLOG_DIR",
            std::env::temp_dir().join(format!("cb1-log-{}-{chrome}", std::process::id())),
        );
        cmd.arg("--allow-write").arg(std::env::temp_dir());
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manuk-mcp");
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            tx,
            rx,
            id: 0,
        };
        c.rpc("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
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
            assert!(
                self.rx.read_line(&mut line).unwrap() > 0,
                "server closed stdout"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }
    /// (is_error, text, base64 image data if any)
    fn call(&mut self, name: &str, args: Value) -> (bool, String, Option<String>) {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let res = &r["result"];
        let blocks = res["content"].as_array().cloned().unwrap_or_default();
        let text = blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let img = blocks
            .iter()
            .find(|b| b["type"] == "image")
            .and_then(|b| b["data"].as_str().map(String::from));
        (res["isError"].as_bool().unwrap_or(false), text, img)
    }
    fn tool(&mut self, name: &str, args: Value) -> String {
        let (err, t, _) = self.call(name, args);
        assert!(!err, "{name} failed: {t}");
        t
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn handle_of(text: &str, needle: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"));
    let s = line.find("[h").unwrap();
    let e = line[s..].find(']').unwrap() + s;
    line[s + 1..e].to_string()
}

fn line_of<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no {needle:?} in:\n{text}"))
}

fn run(chrome: bool) {
    let url = serve();
    let mut c = Client::start(chrome);
    let nav = c.tool("navigate", json!({"url": url}));
    // Three identical buttons, told apart by the card they sit in.
    let adds: Vec<&str> = nav.lines().filter(|l| l.contains("\"Add to cart\"")).collect();
    assert_eq!(adds.len(), 3, "three Add to cart lines (chrome={chrome}):\n{nav}");
    for card in ["Teal Kettle", "Red Mug", "Blue Lamp"] {
        assert!(
            adds.iter().any(|l| l.contains(&format!("in listitem \"{card}\""))),
            "no Add to cart line names card {card:?} (chrome={chrome}):\n{nav}"
        );
    }
    // A div with a click handler is marked.
    assert!(line_of(&nav, "Gift wrap").contains("[clickable]"), "onclick div unmarked (chrome={chrome}):\n{nav}");
    if chrome {
        assert!(line_of(&nav, "Promo banner").contains("[clickable]"), "listener div unmarked:\n{nav}");
    }
    // Act on the second card only.
    let mug = handle_of(&nav, "button \"Add to cart\" in listitem \"Red Mug\"");
    let teal = handle_of(&nav, "button \"Add to cart\" in listitem \"Teal Kettle\"");
    let r = c.tool("act", json!({"handle": mug, "action": "click"}));
    assert!(r.contains("cart:Red Mug"), "wrong card added (chrome={chrome}):\n{r}");
    // A route change keeps the document and the handles that did not re-render.
    let route = handle_of(&nav, "button \"Go to cart\"");
    let r = c.tool("act", json!({"handle": route, "action": "click"}));
    assert!(r.contains("routed (same document"), "pushState treated as a new page (chrome={chrome}):\n{r}");
    assert!(!r.contains("navigated:"), "{r}");
    // The list was re-rendered into new nodes; the old handle still acts, on the same card.
    let r = c.tool("act", json!({"handle": teal, "action": "click"}));
    assert!(r.contains("cart:Red Mug,Teal Kettle"), "re-mounted handle lost (chrome={chrome}):\n{r}");
    // ARIA combobox: open, pick, verify.
    let obs = c.tool("observe", json!({}));
    let dd = handle_of(&obs, "combobox \"City\"");
    let r = c.tool("act", json!({"handle": dd, "action": "select", "value": "Paris"}));
    assert!(r.contains("now shows \"Paris\""), "combobox select not confirmed (chrome={chrome}):\n{r}");
}

/// Over budget, the content text survives and chrome text is what is cut. Chrome only: the engine's
/// a11y tree does not carry paragraph text at all yet (W4.A4.b).
fn budget() {
    let url = serve();
    let mut c = Client::start(true);
    c.tool("navigate", json!({"url": format!("{url}budget")}));
    let o = c.tool("observe", json!({"budget_tokens": 150}));
    assert!(o.contains("cut "), "budget did not bite:\n{o}");
    assert!(o.contains("PRICE-MARKER"), "content text was cut before chrome:\n{o}");
    assert!(!o.contains("Chrome-Twelve"), "chrome text survived the cut:\n{o}");
}

#[test]
fn spa_observation_on_the_manuk_backend() {
    run(false);
}

#[test]
fn spa_observation_on_the_chrome_backend() {
    run(true);
}

#[test]
fn budget_keeps_content_before_chrome_on_the_chrome_backend() {
    budget();
}
