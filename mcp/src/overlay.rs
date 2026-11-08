//! W5.L4: interruptions. A modal or consent overlay is found in the a11y tree (a dialog/alertdialog, or a
//! positioned box covering a large part of the viewport that holds controls), classified, and given a
//! dismiss control: a reject/necessary-only/close button. A CAPTCHA or bot wall is never dismissed: it is
//! classified `BotWall` and handed to the human. "Accept" is never chosen on the agent's behalf.

use manuk_a11y::{A11yNode, Rect, Role};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Consent,
    Modal,
    BotWall,
}

pub struct Overlay<'t> {
    pub node: &'t A11yNode,
    pub kind: Kind,
    /// The control `dismiss` would click (never an accept control; none for a bot wall).
    pub control: Option<&'t A11yNode>,
}

const BOT: [&str; 8] = [
    "captcha",
    "i'm not a robot",
    "i am not a robot",
    "verify you are human",
    "verify you are a human",
    "unusual traffic",
    "checking your browser",
    "are you a robot",
];
const CONSENT: [&str; 5] = ["cookie", "consent", "gdpr", "your privacy", "privacy choices"];
const REJECT: [&str; 9] = [
    "reject",
    "decline",
    "deny",
    "refuse",
    "necessary only",
    "only necessary",
    "essential only",
    "only essential",
    "without accepting",
];
const CLOSE: [&str; 8] = ["close", "dismiss", "no thanks", "no, thanks", "not now", "maybe later", "skip", "cancel"];
const ACCEPT: [&str; 8] = ["accept", "agree", "allow", "yes", "subscribe", "sign up", "got it", "ok"];

fn lower(s: &str) -> String {
    s.to_lowercase()
}

fn word_hit(name: &str, pats: &[&str]) -> bool {
    let n = lower(name);
    pats.iter().any(|p| {
        if p.len() <= 3 {
            n.split(|c: char| !c.is_alphanumeric()).any(|w| w == *p)
        } else {
            n.contains(p)
        }
    })
}

fn is_close_glyph(name: &str) -> bool {
    matches!(name.trim(), "×" | "✕" | "✖" | "x" | "X" | "╳")
}

/// Does the page ask to prove it is human, anywhere in the tree?
pub fn bot_wall(tree: &A11yNode) -> bool {
    tree.iter().any(|n| word_hit(&n.name, &BOT))
}

fn text_of(n: &A11yNode) -> String {
    n.iter().map(|k| k.name.as_str()).collect::<Vec<_>>().join(" ")
}

fn pick_control<'t>(n: &'t A11yNode, kind: Kind) -> Option<&'t A11yNode> {
    let ctl: Vec<&A11yNode> = n
        .iter()
        .filter(|k| matches!(k.role, Role::Button | Role::Link) && !k.state.disabled)
        .collect();
    let ok = |k: &&&A11yNode| !word_hit(&k.name, &ACCEPT) || word_hit(&k.name, &REJECT);
    if kind == Kind::Consent {
        if let Some(c) = ctl.iter().filter(ok).find(|k| word_hit(&k.name, &REJECT)) {
            return Some(*c);
        }
    }
    ctl.iter()
        .filter(ok)
        .find(|k| word_hit(&k.name, &CLOSE) || is_close_glyph(&k.name) || word_hit(&k.name, &REJECT))
        .copied()
}

fn covers(n: &A11yNode, vp: &Rect) -> bool {
    n.bbox
        .is_some_and(|b| n.z > 0 && b.width * b.height >= 0.3 * vp.width * vp.height)
}

/// Is some control OUTSIDE `n`, on a lower layer, under `n`'s box? That is what makes a covering box a
/// modal: it blocks something. An app's own positioned wrapper holds every control, so it blocks none.
fn blocks_something(n: &A11yNode, all: &[&A11yNode]) -> bool {
    let (Some(b), inside) = (n.bbox, n.iter().map(|k| k.node).collect::<Vec<_>>()) else {
        return false;
    };
    all.iter().any(|k| {
        k.role.is_interactive()
            && k.z < n.z
            && !inside.contains(&k.node)
            && k.bbox.is_some_and(|r| {
                let (cx, cy) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
                cx >= b.x && cx <= b.x + b.width && cy >= b.y && cy <= b.y + b.height
            })
    })
}

/// Overlays on the page, topmost-looking first. A bot wall anywhere reports once, on its container if
/// it has one, else on the document.
pub fn find<'t>(tree: &'t A11yNode, vp: &Rect) -> Vec<Overlay<'t>> {
    let mut out: Vec<Overlay> = Vec::new();
    let all: Vec<&A11yNode> = tree.iter().collect();
    for &n in &all {
        let dialog = matches!(n.role, Role::Dialog | Role::AlertDialog);
        let boxy = covers(n, vp)
            && n.iter().skip(1).any(|k| k.role.is_interactive())
            && blocks_something(n, &all);
        if !(dialog || boxy) || out.iter().any(|o| o.node.iter().any(|k| k.node == n.node)) {
            continue;
        }
        let text = text_of(n);
        let kind = if word_hit(&text, &BOT) {
            Kind::BotWall
        } else if word_hit(&text, &CONSENT) {
            Kind::Consent
        } else {
            Kind::Modal
        };
        let control = if kind == Kind::BotWall { None } else { pick_control(n, kind) };
        out.push(Overlay { node: n, kind, control });
    }
    if !out.iter().any(|o| o.kind == Kind::BotWall) && bot_wall(tree) {
        out.push(Overlay { node: tree, kind: Kind::BotWall, control: None });
    }
    out
}
