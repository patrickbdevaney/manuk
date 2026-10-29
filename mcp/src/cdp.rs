//! R4.1 (K1 arm 3): a manuk-style agent layer over Chrome's DevTools protocol, as a second [`Backend`].
//!
//! The tool surface (`session.rs`) is unchanged; this file only supplies what the engine supplies
//! in-process, rebuilt from CDP so the control arm gets the same three disciplines:
//! * **taint** — `DOMSnapshot.captureSnapshot` (computed style + layout + paint order) feeds the same
//!   hidden-text rules as `manuk_agent::shield` (opacity, visibility, zero-size clip, tiny font,
//!   offscreen, transparent, low contrast), so `observe`/`extract` withhold what a human cannot see;
//! * **dispatch re-validation** — a click re-snapshots, re-finds the element, hit-tests its click point
//!   with `DOM.getNodeForLocation` and refuses (`OCCLUDED at dispatch`) unless the hit is the target or
//!   a descendant;
//! * **re-mount identity** — a handle survives the page replacing its element (same tag, role, name and
//!   ordinal) by re-issuing the old `NodeId` to the new element.
//!
//! What it cannot do (and says so): no write-fence, no `SERVER-DIVERGED` ledger, no storage-state tools;
//! `checkpoint`/`restore` are the Hub's replay over a fresh Chrome per page.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use manuk_a11y::{A11yNode, A11yState, Checked, Rect, Role};
use manuk_agent::shield::{contrast, Shield, MIN_CONTRAST, MIN_FONT_PX};
use manuk_dom::NodeId;
use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::backend::Backend;

// ───────────────────────────── CDP transport ─────────────────────────────

struct Cdp {
    ws: WebSocket<TcpStream>,
    next: u64,
    session: Option<String>,
    events: VecDeque<Value>,
}

impl Cdp {
    fn send(&mut self, method: &str, params: Value) -> Result<u64> {
        self.next += 1;
        let mut m = json!({"id": self.next, "method": method, "params": params});
        if let Some(s) = &self.session {
            m["sessionId"] = json!(s);
        }
        self.ws.send(Message::text(m.to_string()))?;
        Ok(self.next)
    }

    /// One message, or `None` on timeout.
    fn read(&mut self, wait: Duration) -> Result<Option<Value>> {
        self.ws.get_ref().set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
        match self.ws.read() {
            Ok(Message::Text(t)) => Ok(Some(serde_json::from_str(t.as_str())?)),
            Ok(Message::Close(_)) => bail!("Chrome closed the DevTools socket"),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// A page dialog blocks every other command until answered, so it is answered the moment it is seen.
    fn auto_dialog(&mut self, m: &Value) {
        if m["method"] == "Page.javascriptDialogOpening" {
            let _ = self.send("Page.handleJavaScriptDialog", json!({"accept": true}));
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.send(method, params)?;
        let t0 = Instant::now();
        loop {
            let left = Duration::from_secs(45).saturating_sub(t0.elapsed());
            if left.is_zero() {
                bail!("CDP {method} timed out");
            }
            let Some(m) = self.read(left.min(Duration::from_secs(1)))? else { continue };
            if m["id"].as_u64() == Some(id) {
                if let Some(e) = m.get("error") {
                    bail!("CDP {method}: {}", e["message"].as_str().unwrap_or("error"));
                }
                return Ok(m["result"].clone());
            }
            if m.get("method").is_some() {
                self.auto_dialog(&m);
                self.events.push_back(m);
            }
        }
    }

    /// Collect events for `dur`, or until `stop` says one of them is the one waited for.
    fn pump_until(&mut self, dur: Duration, stop: impl Fn(&Value) -> bool) -> Result<bool> {
        let t0 = Instant::now();
        if self.events.iter().any(&stop) {
            return Ok(true);
        }
        while t0.elapsed() < dur {
            let Some(m) = self.read((dur - t0.elapsed()).min(Duration::from_millis(250)))? else { continue };
            if m.get("method").is_some() {
                self.auto_dialog(&m);
                let hit = stop(&m);
                self.events.push_back(m);
                if hit {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// ───────────────────────────── snapshot → tree ─────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: f32,
}

fn parse_color(s: &str) -> Option<Color> {
    let inner = s.split('(').nth(1).and_then(|x| x.split(')').next()).unwrap_or("");
    let v: Vec<f32> = inner.split([',', ' ', '/']).filter(|x| !x.is_empty()).filter_map(|x| x.trim_end_matches('%').parse().ok()).collect();
    if v.len() < 3 {
        return None;
    }
    Some(Color { r: v[0] as u8, g: v[1] as u8, b: v[2] as u8, a: v.get(3).copied().unwrap_or(1.0) })
}

fn rgba(c: Color) -> manuk_css::Rgba {
    manuk_css::Rgba::new(c.r, c.g, c.b, (c.a * 255.0) as u8)
}

const STYLES: [&str; 10] = [
    "display", "visibility", "opacity", "pointer-events", "font-size", "color", "background-color", "overflow-x", "position", "overflow-y",
];

#[derive(Default, Clone)]
struct Style {
    visibility_hidden: bool,
    opacity: f32,
    pointer_none: bool,
    font_size: f32,
    color: Option<Color>,
    bg: Option<Color>,
    clips: bool,
}

struct Raw {
    parent: Vec<i64>,
    kind: Vec<i64>,
    name: Vec<String>,
    value: Vec<String>,
    backend: Vec<u64>,
    attrs: Vec<HashMap<String, String>>,
    kids: Vec<Vec<usize>>,
    input_value: HashMap<usize, String>,
    checked: HashSet<usize>,
    opt_selected: HashSet<usize>,
    clickable: HashSet<usize>,
    /// node index → (bounds, paint order, style)
    layout: HashMap<usize, ([f32; 4], i64, Style)>,
}

fn parse_snapshot(snap: &Value) -> Result<(Raw, f32, f32)> {
    let strings: Vec<String> = snap["strings"].as_array().ok_or_else(|| anyhow!("no strings"))?.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect();
    let s = |i: &Value| -> String { i.as_i64().filter(|&i| i >= 0).and_then(|i| strings.get(i as usize)).cloned().unwrap_or_default() };
    let d = &snap["documents"][0];
    let nd = &d["nodes"];
    let arr = |v: &Value| -> Vec<Value> { v.as_array().cloned().unwrap_or_default() };
    let parent: Vec<i64> = arr(&nd["parentIndex"]).iter().map(|v| v.as_i64().unwrap_or(-1)).collect();
    let n = parent.len();
    let kind: Vec<i64> = arr(&nd["nodeType"]).iter().map(|v| v.as_i64().unwrap_or(0)).collect();
    let name: Vec<String> = arr(&nd["nodeName"]).iter().map(&s).collect();
    let value: Vec<String> = arr(&nd["nodeValue"]).iter().map(&s).collect();
    let backend: Vec<u64> = arr(&nd["backendNodeId"]).iter().map(|v| v.as_u64().unwrap_or(0)).collect();
    let attrs: Vec<HashMap<String, String>> = arr(&nd["attributes"])
        .iter()
        .map(|a| {
            let a = arr(a);
            a.chunks(2).filter(|c| c.len() == 2).map(|c| (s(&c[0]), s(&c[1]))).collect()
        })
        .collect();
    let mut kids = vec![vec![]; n];
    for (i, &p) in parent.iter().enumerate() {
        if p >= 0 && (p as usize) < n {
            kids[p as usize].push(i);
        }
    }
    let idx_set = |v: &Value| -> HashSet<usize> { arr(&v["index"]).iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect() };
    let mut input_value = HashMap::new();
    let (iv, vv) = (arr(&nd["inputValue"]["index"]), arr(&nd["inputValue"]["value"]));
    for (a, b) in iv.iter().zip(vv.iter()) {
        if let Some(i) = a.as_u64() {
            input_value.insert(i as usize, s(b));
        }
    }
    let lay = &d["layout"];
    let (ni, bounds, styles, po) = (arr(&lay["nodeIndex"]), arr(&lay["bounds"]), arr(&lay["styles"]), arr(&lay["paintOrders"]));
    let mut layout = HashMap::new();
    for (k, i) in ni.iter().enumerate() {
        let Some(i) = i.as_u64().map(|i| i as usize) else { continue };
        let b = arr(&bounds[k]);
        let b = [0, 1, 2, 3].map(|j| b.get(j).and_then(Value::as_f64).unwrap_or(0.0) as f32);
        let sv: Vec<String> = arr(&styles[k]).iter().map(&s).collect();
        let g = |name: &str| STYLES.iter().position(|x| *x == name).and_then(|p| sv.get(p)).cloned().unwrap_or_default();
        let ox = g("overflow-x");
        let oy = g("overflow-y");
        let st = Style {
            visibility_hidden: matches!(g("visibility").as_str(), "hidden" | "collapse"),
            opacity: g("opacity").parse().unwrap_or(1.0),
            pointer_none: g("pointer-events") == "none",
            font_size: g("font-size").trim_end_matches("px").parse().unwrap_or(16.0),
            color: parse_color(&g("color")),
            bg: parse_color(&g("background-color")),
            clips: (!ox.is_empty() && ox != "visible") || (!oy.is_empty() && oy != "visible"),
        };
        layout.entry(i).or_insert((b, po.get(k).and_then(Value::as_i64).unwrap_or(0), st));
    }
    let raw = Raw {
        parent,
        kind,
        name,
        value,
        backend,
        attrs,
        kids,
        input_value,
        checked: idx_set(&nd["inputChecked"]),
        opt_selected: idx_set(&nd["optionSelected"]),
        clickable: idx_set(&nd["isClickable"]),
        layout,
    };
    Ok((raw, d["scrollOffsetX"].as_f64().unwrap_or(0.0) as f32, d["scrollOffsetY"].as_f64().unwrap_or(0.0) as f32))
}

fn is_skipped_tag(t: &str) -> bool {
    matches!(t, "head" | "script" | "style" | "noscript" | "template" | "title" | "meta" | "link" | "base" | "#comment")
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Raw {
    fn tag(&self, i: usize) -> String {
        self.name[i].to_ascii_lowercase()
    }
    fn attr(&self, i: usize, k: &str) -> Option<&str> {
        self.attrs[i].get(k).map(String::as_str)
    }
    fn rendered(&self, i: usize) -> bool {
        self.layout.contains_key(&i)
    }
    /// Rendered text under `i` (all text nodes with a layout box), whitespace-collapsed.
    fn text(&self, i: usize, out: &mut Vec<String>, depth: usize) {
        if depth > 60 {
            return;
        }
        for &c in &self.kids[i] {
            match self.kind[c] {
                3 if self.rendered(c) => out.push(self.value[c].clone()),
                1 if !is_skipped_tag(&self.tag(c)) => self.text(c, out, depth + 1),
                _ => {}
            }
        }
    }
    fn text_of(&self, i: usize) -> String {
        let mut v = vec![];
        self.text(i, &mut v, 0);
        norm(&v.join(" "))
    }
    fn direct_text(&self, i: usize) -> String {
        norm(&self.kids[i].iter().filter(|&&c| self.kind[c] == 3 && self.rendered(c)).map(|&c| self.value[c].as_str()).collect::<Vec<_>>().join(" "))
    }
    fn by_id(&self, id: &str) -> Option<usize> {
        (0..self.kind.len()).find(|&i| self.kind[i] == 1 && self.attr(i, "id") == Some(id))
    }
    fn input_type(&self, i: usize) -> String {
        self.attr(i, "type").unwrap_or("text").to_ascii_lowercase()
    }
}

fn role_of(r: &Raw, i: usize, body_child: bool) -> Role {
    let tag = r.tag(i);
    if let Some(role) = r.attr(i, "role").and_then(|v| v.split_whitespace().next()) {
        let mapped = match role {
            "button" => Some(Role::Button),
            "link" => Some(Role::Link),
            "textbox" => Some(Role::TextBox),
            "searchbox" => Some(Role::SearchBox),
            "checkbox" => Some(Role::CheckBox),
            "radio" => Some(Role::Radio),
            "combobox" => Some(Role::ComboBox),
            "switch" => Some(Role::Switch),
            "slider" => Some(Role::Slider),
            "tab" => Some(Role::Tab),
            "tablist" => Some(Role::TabList),
            "tabpanel" => Some(Role::TabPanel),
            "menuitem" => Some(Role::MenuItem),
            "menu" => Some(Role::Menu),
            "option" => Some(Role::Option),
            "listbox" => Some(Role::ListBox),
            "dialog" => Some(Role::Dialog),
            "alert" => Some(Role::Alert),
            "navigation" => Some(Role::Navigation),
            "main" => Some(Role::Main),
            "banner" => Some(Role::Banner),
            "search" => Some(Role::Search),
            "heading" => Some(Role::Heading { level: r.attr(i, "aria-level").and_then(|l| l.parse().ok()).unwrap_or(2) }),
            "img" => Some(Role::Image),
            "list" => Some(Role::List),
            "listitem" => Some(Role::ListItem),
            "table" | "grid" => Some(Role::Table),
            "row" => Some(Role::Row),
            "cell" | "gridcell" => Some(Role::Cell),
            "presentation" | "none" => Some(Role::Generic),
            _ => None,
        };
        if let Some(m) = mapped {
            return m;
        }
    }
    match tag.as_str() {
        "a" if r.attr(i, "href").is_some() => Role::Link,
        "button" => Role::Button,
        "input" => match r.input_type(i).as_str() {
            "checkbox" => Role::CheckBox,
            "radio" => Role::Radio,
            "button" | "submit" | "reset" | "image" => Role::Button,
            "range" => Role::Slider,
            "number" => Role::SpinButton,
            "search" => Role::SearchBox,
            _ => Role::TextBox,
        },
        "textarea" => Role::TextBox,
        "select" => {
            if r.attr(i, "multiple").is_some() {
                Role::ListBox
            } else {
                Role::ComboBox
            }
        }
        "option" => Role::Option,
        "h1" => Role::Heading { level: 1 },
        "h2" => Role::Heading { level: 2 },
        "h3" => Role::Heading { level: 3 },
        "h4" => Role::Heading { level: 4 },
        "h5" => Role::Heading { level: 5 },
        "h6" => Role::Heading { level: 6 },
        "p" => Role::Paragraph,
        "hr" => Role::Separator,
        "img" => Role::Image,
        "ul" | "ol" => Role::List,
        "li" => Role::ListItem,
        "table" => Role::Table,
        "tr" => Role::Row,
        "td" => Role::Cell,
        "th" => if r.attr(i, "scope") == Some("row") { Role::RowHeader } else { Role::ColumnHeader },
        "nav" => Role::Navigation,
        "main" => Role::Main,
        "aside" => Role::Complementary,
        "header" => if body_child { Role::Banner } else { Role::SectionHeader },
        "footer" => if body_child { Role::ContentInfo } else { Role::SectionFooter },
        "article" => Role::Article,
        "form" => Role::Form,
        "dialog" => Role::Dialog,
        "blockquote" => Role::Blockquote,
        "code" => Role::Code,
        "em" | "i" => Role::Emphasis,
        "strong" | "b" => Role::Strong,
        "figure" => Role::Figure,
        "section" if r.attr(i, "aria-label").is_some() => Role::Region,
        _ => Role::Generic,
    }
}

fn name_of(r: &Raw, i: usize, role: &Role) -> String {
    let tag = r.tag(i);
    if let Some(l) = r.attr(i, "aria-label").filter(|l| !l.trim().is_empty()) {
        return norm(l);
    }
    if let Some(ids) = r.attr(i, "aria-labelledby") {
        let t: Vec<String> = ids.split_whitespace().filter_map(|id| r.by_id(id)).map(|j| r.text_of(j)).collect();
        if !t.join("").is_empty() {
            return norm(&t.join(" "));
        }
    }
    match tag.as_str() {
        "img" => return norm(r.attr(i, "alt").or(r.attr(i, "title")).unwrap_or("")),
        "input" | "textarea" | "select" => {
            if matches!(r.input_type(i).as_str(), "button" | "submit" | "reset") && tag == "input" {
                return norm(r.attr(i, "value").unwrap_or(match r.input_type(i).as_str() { "submit" => "Submit", "reset" => "Reset", _ => "" }));
            }
            // <label for=id> or a wrapping <label>
            if let Some(id) = r.attr(i, "id") {
                if let Some(l) = (0..r.kind.len()).find(|&j| r.kind[j] == 1 && r.tag(j) == "label" && r.attr(j, "for") == Some(id)) {
                    let t = r.text_of(l);
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
            let mut p = r.parent[i];
            while p >= 0 {
                if r.tag(p as usize) == "label" {
                    let t = r.text_of(p as usize);
                    if !t.is_empty() {
                        return t;
                    }
                }
                p = r.parent[p as usize];
            }
            return norm(r.attr(i, "placeholder").or(r.attr(i, "title")).or(r.attr(i, "name")).unwrap_or(""));
        }
        _ => {}
    }
    match role {
        Role::Link | Role::Button | Role::Heading { .. } | Role::Option | Role::Tab | Role::MenuItem | Role::Paragraph | Role::ListItem | Role::Cell | Role::ColumnHeader | Role::RowHeader | Role::CheckBox | Role::Radio | Role::Switch | Role::Code | Role::Strong | Role::Emphasis | Role::Blockquote => {
            let t = r.text_of(i);
            if t.is_empty() {
                norm(r.attr(i, "title").unwrap_or(""))
            } else {
                t
            }
        }
        Role::Generic => r.direct_text(i),
        Role::Dialog | Role::Region | Role::Form | Role::Navigation => norm(r.attr(i, "title").unwrap_or("")),
        _ => String::new(),
    }
}

fn truthy(r: &Raw, i: usize, k: &str) -> bool {
    matches!(r.attr(i, k), Some(v) if v != "false")
}

fn tri(v: Option<&str>) -> Option<Checked> {
    match v? {
        "true" => Some(Checked::True),
        "mixed" => Some(Checked::Mixed),
        "false" => Some(Checked::False),
        _ => None,
    }
}

fn state_of(r: &Raw, i: usize, role: &Role) -> A11yState {
    let tag = r.tag(i);
    let mut st = A11yState::default();
    st.checked = tri(r.attr(i, "aria-checked"));
    if tag == "input" && matches!(r.input_type(i).as_str(), "checkbox" | "radio") {
        st.checked = Some(if r.checked.contains(&i) { Checked::True } else { Checked::False });
    }
    st.expanded = r.attr(i, "aria-expanded").map(|v| v == "true");
    if tag == "details" {
        st.expanded = Some(r.attr(i, "open").is_some());
    }
    st.selected = r.attr(i, "aria-selected").map(|v| v == "true");
    if tag == "option" {
        st.selected = Some(r.opt_selected.contains(&i));
    }
    st.disabled = r.attr(i, "disabled").is_some() || truthy(r, i, "aria-disabled");
    st.required = r.attr(i, "required").is_some() || truthy(r, i, "aria-required");
    st.readonly = r.attr(i, "readonly").is_some() || truthy(r, i, "aria-readonly");
    st.pressed = tri(r.attr(i, "aria-pressed"));
    st.invalid = truthy(r, i, "aria-invalid");
    st.value = match tag.as_str() {
        "input" if !matches!(r.input_type(i).as_str(), "checkbox" | "radio" | "button" | "submit" | "reset") => r.input_value.get(&i).cloned().or_else(|| r.attr(i, "value").map(String::from)),
        "textarea" => r.input_value.get(&i).cloned(),
        "select" => {
            let mut opts = vec![];
            collect_options(r, i, &mut opts);
            opts.iter().find(|&&o| r.opt_selected.contains(&o)).or(opts.first()).map(|&o| norm(&r.text_of_any(o)))
        }
        _ => r.attr(i, "aria-valuenow").map(String::from),
    };
    let _ = role;
    st
}

impl Raw {
    /// Text of a node that may not have a layout box (an `<option>`).
    fn text_of_any(&self, i: usize) -> String {
        norm(&self.kids[i].iter().filter(|&&c| self.kind[c] == 3).map(|&c| self.value[c].as_str()).collect::<Vec<_>>().join(" "))
    }
}

fn collect_options(r: &Raw, i: usize, out: &mut Vec<usize>) {
    for &c in &r.kids[i] {
        if r.kind[c] == 1 {
            match r.tag(c).as_str() {
                "option" => out.push(c),
                "optgroup" => collect_options(r, c, out),
                _ => {}
            }
        }
    }
}

/// What a built tree remembers about each element, for actions and re-mount matching.
#[derive(Default, Clone)]
struct Idents {
    /// stable NodeId → backendNodeId of the live element
    backend: HashMap<u64, u64>,
    /// stable NodeId → snapshot node index
    index: HashMap<u64, usize>,
    /// stable NodeId → re-mount key
    key: HashMap<u64, String>,
    /// stable NodeId → parent stable NodeId
    parent: HashMap<u64, u64>,
}

struct Built {
    tree: A11yNode,
    shield: Shield,
    idents: Idents,
    links: Vec<(String, String)>,
}

struct Builder<'a> {
    r: &'a Raw,
    alias: &'a mut HashMap<u64, u64>,
    /// orphaned stable ids from the previous tree, by re-mount key
    orphans: HashMap<String, VecDeque<u64>>,
    ord: HashMap<String, usize>,
    idents: Idents,
    claimed: HashSet<u64>,
}

impl Builder<'_> {
    fn stable(&mut self, i: usize, key: &str) -> u64 {
        let be = self.r.backend[i];
        let id = if let Some(&s) = self.alias.get(&be) {
            s
        } else if let Some(s) = self.orphans.get_mut(key).filter(|_| !mutated("no-remount")).and_then(|q| q.pop_front()) {
            self.alias.insert(be, s);
            s
        } else {
            be
        };
        // A stable id is issued once per tree.
        let id = if self.claimed.insert(id) { id } else { be + (1 << 40) };
        self.claimed.insert(id);
        id
    }

    fn node(&mut self, i: usize, body_child: bool, parent_id: Option<u64>, depth: usize) -> Option<A11yNode> {
        let r = self.r;
        let tag = r.tag(i);
        if depth > 400 || is_skipped_tag(&tag) || (tag == "input" && r.input_type(i) == "hidden") {
            return None;
        }
        let in_select = tag == "option";
        if !r.rendered(i) && !in_select {
            // display:none (or inside a collapsed container): not part of what a human sees.
            return None;
        }
        let role = role_of(r, i, body_child);
        let mut name = name_of(r, i, &role);
        let mut role = role;
        if matches!(role, Role::Generic) && r.clickable.contains(&i) && !name.is_empty() {
            role = Role::Button;
        }
        if matches!(role, Role::Generic) && name.is_empty() {
            name = String::new();
        }
        let key = format!("{tag}|{:?}|{}", role, name);
        let n = self.ord.entry(key.clone()).or_default();
        *n += 1;
        let full_key = format!("{key}#{n}");
        let id = self.stable(i, &full_key);
        self.idents.backend.insert(id, r.backend[i]);
        self.idents.index.insert(id, i);
        self.idents.key.insert(id, full_key);
        if let Some(p) = parent_id {
            self.idents.parent.insert(id, p);
        }
        let (bbox, z, hittable) = match r.layout.get(&i) {
            Some((b, po, st)) => (
                Some(Rect { x: b[0], y: b[1], width: b[2], height: b[3] }).filter(|b| b.width > 0.0 && b.height > 0.0),
                (*po).max(1) as i32,
                !st.pointer_none && !st.visibility_hidden,
            ),
            None => (None, 0, true),
        };
        let state = state_of(r, i, &role);
        let mut children = vec![];
        if tag == "select" || tag == "datalist" {
            let mut opts = vec![];
            collect_options(r, i, &mut opts);
            for o in opts {
                let oname = norm(&r.text_of_any(o));
                let key = format!("option|{oname}");
                let n = self.ord.entry(key.clone()).or_default();
                *n += 1;
                let k = format!("{key}#{n}");
                let oid = self.stable(o, &k);
                self.idents.backend.insert(oid, r.backend[o]);
                self.idents.index.insert(oid, o);
                self.idents.key.insert(oid, k);
                self.idents.parent.insert(oid, id);
                children.push(A11yNode { node: NodeId(oid), role: Role::Option, name: oname, bbox: None, z: 0, hittable: true, state: state_of(r, o, &Role::Option), children: vec![] });
            }
        } else {
            let bc = tag == "body";
            for &c in &r.kids[i] {
                if r.kind[c] == 1 {
                    if let Some(k) = self.node(c, bc, Some(id), depth + 1) {
                        children.push(k);
                    }
                }
            }
        }
        Some(A11yNode { node: NodeId(id), role, name, bbox, z, hittable, state, children })
    }
}

/// The shield: the same rules as `manuk_agent::shield`, over CDP computed style + layout.
fn shield_of(r: &Raw, ids: &Idents, viewport_w: f32) -> Shield {
    let n = r.kind.len();
    let mut sh = Shield::default();
    let mut seen_runs: Vec<(f32, String)> = vec![];
    let mut text_state: HashMap<usize, bool> = HashMap::new(); // text node → withheld?
    for i in 0..n {
        if r.kind[i] != 3 || r.value[i].trim().is_empty() {
            continue;
        }
        let Some((b, _, st)) = r.layout.get(&i) else { continue };
        // Walk ancestors for the box-wide reasons and the nearest opaque background.
        let mut reason: Option<&'static str> = None;
        let mut bg = manuk_css::Rgba::WHITE;
        let mut bg_found = false;
        let mut opacity = 1.0f32;
        let mut p = r.parent[i];
        while p >= 0 {
            let pu = p as usize;
            if let Some((pb, _, ps)) = r.layout.get(&pu) {
                opacity *= ps.opacity;
                if reason.is_none() && opacity <= 0.01 {
                    reason = Some("opacity");
                }
                if reason.is_none() && ps.visibility_hidden {
                    reason = Some("visibility");
                }
                if reason.is_none() && (pb[2] <= 1.0 || pb[3] <= 1.0) && ps.clips {
                    reason = Some("zero-size");
                }
                if !bg_found {
                    if let Some(c) = ps.bg {
                        if c.a * 255.0 >= 200.0 {
                            bg = rgba(c);
                            bg_found = true;
                        }
                    }
                }
            }
            p = r.parent[pu];
        }
        let color = st.color.unwrap_or(Color { r: 0, g: 0, b: 0, a: 1.0 });
        let reason = reason.or_else(|| {
            if st.font_size <= MIN_FONT_PX {
                Some("tiny-font")
            } else if b[0] + b[2] <= 0.0 || b[1] + b[3] <= 0.0 || b[0] >= viewport_w {
                Some("offscreen")
            } else if color.a <= 0.0 {
                Some("transparent")
            } else if contrast(rgba(color), bg) < MIN_CONTRAST {
                Some("low-contrast")
            } else {
                None
            }
        });
        let text = norm(&r.value[i]);
        match reason {
            Some(why) => {
                sh.withheld.push((why, text));
                text_state.insert(i, true);
            }
            None => {
                seen_runs.push((b[1], text));
                text_state.insert(i, false);
            }
        }
    }
    // Text: runs on a new line start a new line.
    let mut out = String::new();
    let mut last_y = f32::MIN;
    for (y, t) in &seen_runs {
        if !out.is_empty() {
            out.push(if (y - last_y).abs() > 6.0 { '\n' } else { ' ' });
        }
        out.push_str(t);
        last_y = *y;
    }
    sh.text = out;
    // Elements all of whose text is withheld (and invisible boxes) are dropped from the tree.
    let mut hidden_below = vec![0u32; n];
    let mut seen_below = vec![0u32; n];
    for (&t, &w) in &text_state {
        let mut p = r.parent[t];
        while p >= 0 {
            if w {
                hidden_below[p as usize] += 1;
            } else {
                seen_below[p as usize] += 1;
            }
            p = r.parent[p as usize];
        }
    }
    let by_index: HashMap<usize, u64> = ids.index.iter().map(|(&id, &i)| (i, id)).collect();
    for (&i, &id) in &by_index {
        let own_hidden = r.layout.get(&i).is_some_and(|(_, _, st)| st.opacity <= 0.01 || st.visibility_hidden);
        if own_hidden {
            let why = if r.layout[&i].2.visibility_hidden { "visibility" } else { "opacity" };
            sh.nodes.insert(NodeId(id), why);
        } else if hidden_below[i] > 0 && seen_below[i] == 0 {
            let why = r.kids[i].iter().find_map(|&c| text_state.get(&c).filter(|&&w| w).map(|_| "hidden")).unwrap_or("hidden");
            sh.nodes.insert(NodeId(id), why);
        }
    }
    sh
}

fn build(snap: &Value, alias: &mut HashMap<u64, u64>, prev: &Idents, viewport_w: f32) -> Result<(Built, f32, f32)> {
    let (raw, sx, sy) = parse_snapshot(snap)?;
    let live: HashSet<u64> = raw.backend.iter().copied().collect();
    let mut orphans: HashMap<String, VecDeque<u64>> = HashMap::new();
    let mut stale: Vec<(&u64, &u64)> = prev.backend.iter().filter(|(_, be)| !live.contains(be)).collect();
    stale.sort();
    for (sid, _) in stale {
        orphans.entry(prev.key[sid].clone()).or_default().push_back(*sid);
    }
    let html = (0..raw.kind.len()).find(|&i| raw.kind[i] == 1 && raw.tag(i) == "html").ok_or_else(|| anyhow!("snapshot has no <html>"))?;
    let mut b = Builder { r: &raw, alias, orphans, ord: HashMap::new(), idents: Idents::default(), claimed: HashSet::new() };
    let mut root = b.node(html, false, None, 0).ok_or_else(|| anyhow!("page has no rendered root"))?;
    root.role = Role::Document;
    let idents = b.idents;
    let shield = shield_of(&raw, &idents, viewport_w);
    let mut links = vec![];
    for (&id, &i) in &idents.index {
        if raw.tag(i) == "a" {
            if let Some(h) = raw.attr(i, "href") {
                let _ = id;
                links.push((i, raw.text_of(i), h.to_string()));
            }
        }
    }
    links.sort();
    let links = links.into_iter().map(|(_, t, h)| (t, h)).collect();
    Ok((Built { tree: root, shield, idents, links }, sx, sy))
}

// ───────────────────────────── the backend ─────────────────────────────

pub struct CdpBackend {
    cdp: Cdp,
    child: Child,
    dir: PathBuf,
    width: f32,
    height: f32,
    alias: HashMap<u64, u64>,
    built: Option<Built>,
    url: String,
    title: String,
    scroll: (f32, f32),
    content_h: f32,
    text: String,
    /// Dispatch-time facts for the last click, for the tool result.
    pub remounted: u32,
}

/// Test-only: `MANUK_CDP_MUTATE=no-taint|no-revalidate|no-remount` switches one discipline off, so the
/// gate can be shown to go red without it.
fn mutated(what: &str) -> bool {
    std::env::var("MANUK_CDP_MUTATE").is_ok_and(|v| v == what)
}

fn chrome_exe() -> String {
    std::env::var("MANUK_CHROME").ok().or_else(|| std::env::var("CHROME").ok()).unwrap_or_else(|| "google-chrome".into())
}

impl CdpBackend {
    pub fn launch(width: u32, height: u32) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("manuk-cdp-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
        std::fs::create_dir_all(&dir)?;
        let mut cmd = Command::new(chrome_exe());
        // If this server is killed (the harness ends a session by SIGKILL), Chrome must not outlive it.
        #[cfg(target_os = "linux")]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
        let child = cmd
            .args(["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check", "--mute-audio", "--disable-background-networking", "--remote-debugging-port=0"])
            .arg(format!("--user-data-dir={}", dir.display()))
            .arg(format!("--window-size={width},{height}"))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("launching {}", chrome_exe()))?;
        let port_file = dir.join("DevToolsActivePort");
        let t0 = Instant::now();
        let (port, path) = loop {
            if let Ok(s) = std::fs::read_to_string(&port_file) {
                let mut l = s.lines();
                if let (Some(p), Some(path)) = (l.next(), l.next()) {
                    break (p.trim().parse::<u16>()?, path.trim().to_string());
                }
            }
            if t0.elapsed() > Duration::from_secs(30) {
                bail!("Chrome did not publish DevToolsActivePort");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let stream = TcpStream::connect(("127.0.0.1", port))?;
        let (ws, _) = tungstenite::client(format!("ws://127.0.0.1:{port}{path}"), stream).map_err(|e| anyhow!("DevTools handshake: {e}"))?;
        let mut cdp = Cdp { ws, next: 0, session: None, events: VecDeque::new() };
        let targets = cdp.call("Target.getTargets", json!({}))?;
        let tid = targets["targetInfos"]
            .as_array()
            .and_then(|a| a.iter().find(|t| t["type"] == "page"))
            .and_then(|t| t["targetId"].as_str())
            .ok_or_else(|| anyhow!("no page target"))?
            .to_string();
        let att = cdp.call("Target.attachToTarget", json!({"targetId": tid, "flatten": true}))?;
        cdp.session = Some(att["sessionId"].as_str().ok_or_else(|| anyhow!("no sessionId"))?.to_string());
        for m in ["Page.enable", "DOM.enable", "Runtime.enable"] {
            cdp.call(m, json!({}))?;
        }
        cdp.call("Page.setLifecycleEventsEnabled", json!({"enabled": true}))?;
        cdp.call("Emulation.setDeviceMetricsOverride", json!({"width": width, "height": height, "deviceScaleFactor": 1, "mobile": false}))?;
        Ok(CdpBackend { cdp, child, dir, width: width as f32, height: height as f32, alias: HashMap::new(), built: None, url: "about:blank".into(), title: String::new(), scroll: (0.0, 0.0), content_h: 0.0, text: String::new(), remounted: 0 })
    }

    fn eval(&mut self, expr: &str) -> Result<Value> {
        let r = self.cdp.call("Runtime.evaluate", json!({"expression": expr, "returnByValue": true, "awaitPromise": false}))?;
        if let Some(e) = r.get("exceptionDetails") {
            bail!("page script error: {}", e["text"].as_str().unwrap_or("exception"));
        }
        Ok(r["result"]["value"].clone())
    }

    /// Re-snapshot the page and rebuild the tree, shield and identity tables.
    fn refresh(&mut self) -> Result<()> {
        let info = self.eval("JSON.stringify({u:location.href,t:document.title,x:scrollX,y:scrollY,w:innerWidth,h:innerHeight,ch:Math.max(document.documentElement.scrollHeight,document.body?document.body.scrollHeight:0),tx:document.body?document.body.innerText:''})")?;
        let info: Value = serde_json::from_str(info.as_str().unwrap_or("{}"))?;
        self.url = info["u"].as_str().unwrap_or("").to_string();
        self.title = info["t"].as_str().unwrap_or("").to_string();
        self.scroll = (info["x"].as_f64().unwrap_or(0.0) as f32, info["y"].as_f64().unwrap_or(0.0) as f32);
        self.width = info["w"].as_f64().unwrap_or(self.width as f64) as f32;
        self.height = info["h"].as_f64().unwrap_or(self.height as f64) as f32;
        self.content_h = info["ch"].as_f64().unwrap_or(0.0) as f32;
        self.text = info["tx"].as_str().unwrap_or("").to_string();
        let snap = self.cdp.call("DOMSnapshot.captureSnapshot", json!({"computedStyles": STYLES, "includePaintOrder": true, "includeDOMRects": true}))?;
        let prev = self.built.as_ref().map(|b| b.idents.clone()).unwrap_or_default();
        let before = self.alias.len();
        let (built, _, _) = build(&snap, &mut self.alias, &prev, self.width)?;
        self.remounted = (self.alias.len() - before.min(self.alias.len())) as u32;
        self.built = Some(built);
        Ok(())
    }

    fn built(&self) -> Result<&Built> {
        self.built.as_ref().ok_or_else(|| anyhow!("no page loaded yet: navigate first"))
    }

    /// After something that may have started a navigation: wait for it to finish, then re-read.
    fn after_action(&mut self) -> Result<()> {
        let nav = |m: &Value| matches!(m["method"].as_str(), Some("Page.frameStartedNavigating" | "Page.frameStartedLoading" | "Page.frameNavigated" | "Page.navigatedWithinDocument"));
        self.cdp.pump_until(Duration::from_millis(450), nav)?;
        let navigated = self.cdp.events.iter().any(nav);
        if navigated {
            self.cdp.pump_until(Duration::from_secs(20), |m| m["method"] == "Page.loadEventFired" || m["method"] == "Page.navigatedWithinDocument")?;
            self.cdp.pump_until(Duration::from_secs(3), |m| m["method"] == "Page.lifecycleEvent" && m["params"]["name"] == "networkIdle")?;
        }
        self.cdp.events.clear();
        self.refresh()
    }

    fn backend_of(&self, node: NodeId) -> Result<u64> {
        self.built()?.idents.backend.get(&node.0).copied().ok_or_else(|| anyhow!("STALE: that element is gone from the page and no re-mount match exists; observe again"))
    }

    fn object_for(&mut self, be: u64) -> Result<String> {
        let r = self.cdp.call("DOM.resolveNode", json!({"backendNodeId": be}))?;
        r["object"]["objectId"].as_str().map(String::from).ok_or_else(|| anyhow!("element has no JS object"))
    }

    fn call_on(&mut self, obj: &str, f: &str, args: Value) -> Result<Value> {
        let r = self.cdp.call("Runtime.callFunctionOn", json!({"objectId": obj, "functionDeclaration": f, "arguments": args, "returnByValue": true}))?;
        if let Some(e) = r.get("exceptionDetails") {
            bail!("page script error: {}", e["text"].as_str().unwrap_or("exception"));
        }
        Ok(r["result"]["value"].clone())
    }

    /// Is `hit` the target or inside it?
    fn within(&self, hit: u64, target_be: u64) -> bool {
        let Ok(b) = self.built() else { return false };
        let Some(&hid) = b.idents.backend.iter().find(|(_, &be)| be == hit).map(|(id, _)| id) else {
            return false;
        };
        let tid = b.idents.backend.iter().find(|(_, &be)| be == target_be).map(|(id, _)| *id);
        let mut cur = Some(hid);
        while let Some(c) = cur {
            if Some(c) == tid {
                return true;
            }
            cur = b.idents.parent.get(&c).copied();
        }
        false
    }

    /// Dispatch-time re-validation: pick a point of the (freshly re-found) element's box that
    /// `DOM.getNodeForLocation` says belongs to the element, else report what covers it.
    fn aim(&mut self, node: NodeId) -> Result<(f32, f32, u64)> {
        let be = self.backend_of(node)?;
        let b = self.built()?;
        let n = find(&b.tree, node).ok_or_else(|| anyhow!("STALE: element no longer in the tree"))?;
        let bbox = n.bbox.ok_or_else(|| anyhow!("UNREACHABLE: element has no box at dispatch"))?;
        let vp = Rect { x: self.scroll.0, y: self.scroll.1, width: self.width, height: self.height };
        let aim = bbox.intersection(&vp).ok_or_else(|| anyhow!("OFFSCREEN at dispatch: element is outside the viewport"))?;
        let pts = [
            aim.center(),
            (aim.x + aim.width * 0.25, aim.y + aim.height * 0.25),
            (aim.x + aim.width * 0.75, aim.y + aim.height * 0.25),
            (aim.x + aim.width * 0.25, aim.y + aim.height * 0.75),
            (aim.x + aim.width * 0.75, aim.y + aim.height * 0.75),
        ];
        let mut cover = None;
        for (px, py) in pts {
            let (cx, cy) = (px - self.scroll.0, py - self.scroll.1);
            let r = self.cdp.call("DOM.getNodeForLocation", json!({"x": cx.floor() as i64, "y": cy.floor() as i64, "includeUserAgentShadowDOM": false}))?;
            let hit = r["backendNodeId"].as_u64().unwrap_or(0);
            if hit == be || self.within(hit, be) || mutated("no-revalidate") {
                return Ok((cx, cy, be));
            }
            cover.get_or_insert(hit);
        }
        let by = cover
            .and_then(|h| self.built().ok().and_then(|b| b.idents.backend.iter().find(|(_, &x)| x == h).map(|(id, _)| *id)))
            .and_then(|id| find(&self.built().ok()?.tree, NodeId(id)).map(|n| format!("{:?} {:?}", n.role, n.name)))
            .unwrap_or_else(|| "another element".into());
        bail!("OCCLUDED at dispatch: {by} covers the target; nothing was clicked")
    }
}

fn find(n: &A11yNode, id: NodeId) -> Option<&A11yNode> {
    if n.node == id {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, id))
}

impl Drop for CdpBackend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[async_trait(?Send)]
impl Backend for CdpBackend {
    async fn navigate(&mut self, url: &str) -> Result<()> {
        self.cdp.events.clear();
        let r = self.cdp.call("Page.navigate", json!({"url": url}))?;
        if let Some(e) = r["errorText"].as_str() {
            bail!("navigation failed: {e}");
        }
        self.cdp.pump_until(Duration::from_secs(25), |m| m["method"] == "Page.loadEventFired")?;
        self.cdp.pump_until(Duration::from_secs(3), |m| m["method"] == "Page.lifecycleEvent" && m["params"]["name"] == "networkIdle")?;
        self.cdp.events.clear();
        self.refresh()
    }
    fn tree(&self) -> Result<A11yNode> {
        Ok(self.built()?.tree.clone())
    }
    fn url(&self) -> String {
        self.url.clone()
    }
    fn title(&self) -> String {
        self.title.clone()
    }
    fn viewport(&self) -> Rect {
        Rect { x: self.scroll.0, y: self.scroll.1, width: self.width, height: self.height }
    }
    fn scroll_y(&self) -> f32 {
        self.scroll.1
    }
    fn content_height(&self) -> f32 {
        self.content_h
    }
    fn scroll_by(&mut self, dy: f32) {
        if self.eval(&format!("window.scrollBy(0,{dy});0")).is_ok() {
            let _ = self.refresh();
        }
    }
    async fn settle(&mut self) {
        let _ = self.cdp.pump_until(Duration::from_millis(150), |_| false);
        self.cdp.events.clear();
        let _ = self.refresh();
    }
    async fn click(&mut self, node: NodeId) -> Result<String> {
        // Dispatch-time re-validation: fresh snapshot, re-mount, then a real hit-test.
        self.refresh()?;
        let remounted = self.remounted;
        let be = self.backend_of(node)?;
        let n = find(&self.built()?.tree, node).ok_or_else(|| anyhow!("STALE: element gone"))?;
        if matches!(n.role, Role::Option) {
            let obj = self.object_for(be)?;
            self.call_on(&obj, "function(){this.selected=true;const s=this.closest('select');if(s){s.dispatchEvent(new Event('input',{bubbles:true}));s.dispatchEvent(new Event('change',{bubbles:true}));}}", json!([]))?;
            self.after_action()?;
            return Ok("selected option".into());
        }
        let (x, y, be) = self.aim(node)?;
        // A new-tab link would leave this page; keep the agent in the one tab it observes.
        if let Ok(obj) = self.object_for(be) {
            let _ = self.call_on(&obj, "function(){const a=this.closest('a');if(a&&a.target==='_blank')a.removeAttribute('target');}", json!([]));
        }
        self.cdp.events.clear();
        for (t, cc) in [("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)] {
            self.cdp.call("Input.dispatchMouseEvent", json!({"type": t, "x": x, "y": y, "button": "left", "buttons": if t == "mousePressed" { 1 } else { 0 }, "clickCount": cc}))?;
        }
        self.after_action()?;
        Ok(format!("dispatched click at ({x:.0},{y:.0}); hit-test re-validated at dispatch{}", if remounted > 0 { format!("; {remounted} element(s) re-mounted by identity") } else { String::new() }))
    }
    fn type_text(&mut self, node: NodeId, text: &str) -> Result<()> {
        let be = self.backend_of(node)?;
        let obj = self.object_for(be)?;
        self.call_on(&obj, "function(){this.focus();if(this.select){this.select();}else{document.execCommand('selectAll');}}", json!([]))?;
        if text.is_empty() {
            self.cdp.call("Input.dispatchKeyEvent", json!({"type": "rawKeyDown", "key": "Backspace", "code": "Backspace", "windowsVirtualKeyCode": 8}))?;
            self.cdp.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Backspace", "code": "Backspace", "windowsVirtualKeyCode": 8}))?;
        } else {
            self.cdp.call("Input.insertText", json!({"text": text}))?;
        }
        self.refresh()
    }
    async fn submit(&mut self, node: NodeId) -> Result<String> {
        let be = self.backend_of(node)?;
        let obj = self.object_for(be)?;
        self.call_on(&obj, "function(){this.focus();}", json!([]))?;
        self.cdp.events.clear();
        for (t, extra) in [("rawKeyDown", json!({})), ("char", json!({"text": "\r"})), ("keyUp", json!({}))] {
            let mut p = json!({"type": t, "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13});
            if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
                o.extend(e.clone());
            }
            self.cdp.call("Input.dispatchKeyEvent", p)?;
        }
        self.after_action()?;
        Ok("pressed Enter in the field".into())
    }
    fn page_text(&self) -> Result<String> {
        Ok(self.text.clone())
    }
    fn shield(&self) -> Result<Option<Shield>> {
        if mutated("no-taint") {
            return Ok(None);
        }
        Ok(Some(self.built()?.shield.clone()))
    }
    fn links(&self) -> Result<Vec<(String, String)>> {
        Ok(self.built()?.links.clone())
    }
    fn screenshot_png(&self) -> Result<Vec<u8>> {
        // `&self` cannot drive the socket; a screenshot is rare enough to open a one-shot path.
        bail!("screenshot is not implemented on the CDP arm (the study's tasks are text-only)")
    }
}
