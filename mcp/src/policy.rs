//! W3.9: grants + origin sets + risk v2, enforced in the backend-agnostic session so one policy
//! covers `--backend manuk` and `--backend chrome`. A call is judged BEFORE it reaches the backend:
//! a refused navigation or form submit puts nothing on the wire.
//!
//! `--read-origins` bounds where the agent may GET (navigate, follow a link, submit a GET form);
//! `--write-origins` bounds where it may type or POST. Empty = unrestricted (the headless default).
//! `--confirm=submit,download,navigate-new-origin` makes those actions need an explicit
//! `confirm=true` on the call. Credential fields are never typed by the agent: that is a HANDOFF.
//!
//! Not covered here (named, not faked): subresource loads and redirects the page itself issues
//! (needs enforcement in manuk_net / a CDP interceptor), and frames outside the set in `observe`.

use std::collections::HashSet;

use manuk_agent::NodeFacts;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grants {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub confirm: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Needs `confirm=true` on the call.
    Confirm(String),
    /// A human must do this; no flag lets the agent.
    Handoff(String),
    Deny(String),
}

impl Verdict {
    pub fn tag(&self) -> &'static str {
        match self {
            Verdict::Allow => "allow",
            Verdict::Confirm(_) => "confirm",
            Verdict::Handoff(_) => "handoff",
            Verdict::Deny(_) => "deny",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op<'a> {
    Navigate(&'a str),
    /// click / check / select on an element.
    Click,
    Type {
        submit: bool,
    },
}

pub fn origin(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    let h = u.host_str()?;
    Some(match u.port() {
        Some(p) => format!("{}://{}:{p}", u.scheme(), h.to_ascii_lowercase()),
        None => format!("{}://{}", u.scheme(), h.to_ascii_lowercase()),
    })
}

fn in_set(set: &[String], url: &str) -> bool {
    set.is_empty()
        || origin(url).is_some_and(|o| set.iter().any(|s| origin(s).as_deref() == Some(&o)))
}

const RISKY_NAMES: &[&str] = &[
    "delete",
    "remove",
    "pay",
    "purchase",
    "buy now",
    "place order",
    "transfer",
    "send money",
    "confirm order",
    "unsubscribe",
    "close account",
];

impl Grants {
    pub fn is_open(&self) -> bool {
        self.read.is_empty() && self.write.is_empty() && self.confirm.is_empty()
    }

    fn needs(&self, kind: &str) -> bool {
        self.confirm.contains(kind)
    }

    fn read_ok(&self, url: &str) -> Result<(), Verdict> {
        if in_set(&self.read, url) {
            Ok(())
        } else {
            Err(Verdict::Deny(format!("origin not in read set: {url}")))
        }
    }

    fn write_ok(&self, url: &str) -> Result<(), Verdict> {
        if in_set(&self.write, url) {
            Ok(())
        } else {
            Err(Verdict::Deny(format!("origin not in write set: {url}")))
        }
    }

    fn judge(
        &self,
        op: Op,
        f: Option<&NodeFacts>,
        name: &str,
        page_url: &str,
    ) -> Result<Verdict, Verdict> {
        // A blank page has no origin to leave, so the first load is never a "new origin".
        let same = |u: &str| origin(page_url).is_none() || origin(u) == origin(page_url);
        if let Op::Navigate(url) = op {
            self.read_ok(url)?;
            if self.needs("navigate-new-origin") && !same(url) {
                return Ok(Verdict::Confirm("navigate-new-origin".into()));
            }
            return Ok(Verdict::Allow);
        }
        let Some(f) = f else {
            // The backend could not describe the element: a click is judged by name alone.
            return Ok(self.by_name(name));
        };
        let submitting = match op {
            Op::Type { submit } => submit,
            _ => {
                f.tag == "button" && f.input_type.as_deref().is_none_or(|t| t == "submit")
                    || (f.tag == "input"
                        && matches!(f.input_type.as_deref(), Some("submit" | "image")))
            }
        };
        if let Op::Type { .. } = op {
            self.write_ok(page_url)?;
            let ty = f.input_type.as_deref().unwrap_or("text");
            let ac = f.autocomplete.as_deref().unwrap_or("");
            let cred = ty == "password"
                || ac.split_whitespace().any(|t| {
                    matches!(t, "current-password" | "new-password" | "one-time-code")
                        || t.starts_with("cc-")
                });
            if cred {
                return Ok(Verdict::Handoff(format!(
                    "{ty} field (autocomplete={ac:?}): a human types credentials and payment data"
                )));
            }
        }
        if let Some(href) = &f.href {
            self.read_ok(href)?;
            if f.download && self.needs("download") {
                return Ok(Verdict::Confirm("download".into()));
            }
            if self.needs("navigate-new-origin") && !same(href) {
                return Ok(Verdict::Confirm("navigate-new-origin".into()));
            }
            return Ok(Verdict::Allow);
        }
        if submitting {
            if let (Some(m), Some(action)) = (&f.form_method, &f.form_action) {
                if m == "get" {
                    self.read_ok(action)?;
                    return Ok(Verdict::Allow);
                }
                if self.write.is_empty() && !same(action) {
                    return Err(Verdict::Deny(format!("cross-origin form {m} to {action}")));
                }
                self.write_ok(action)?;
                if self.needs("submit") {
                    return Ok(Verdict::Confirm("submit".into()));
                }
                return Ok(Verdict::Allow);
            }
        }
        if matches!(op, Op::Click) {
            return Ok(self.by_name(name));
        }
        Ok(Verdict::Allow)
    }

    fn by_name(&self, name: &str) -> Verdict {
        let n = name.to_ascii_lowercase();
        if self.needs("submit") && RISKY_NAMES.iter().any(|w| n.contains(w)) {
            Verdict::Confirm("submit".into())
        } else {
            Verdict::Allow
        }
    }

    /// Judge one call. `Deny` and `Handoff` are final; `Confirm` stands unless the call carried
    /// `confirm=true`.
    pub fn assess(&self, op: Op, f: Option<&NodeFacts>, name: &str, page_url: &str) -> Verdict {
        match self.judge(op, f, name, page_url) {
            Ok(v) | Err(v) => v,
        }
    }
}

/// Pull `--read-origins a,b`, `--write-origins a,b` and `--confirm=submit,download,...` out of `args`.
pub fn parse_grants(args: &mut Vec<String>) -> Result<Grants, String> {
    let mut g = Grants::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        let (key, inline) = match a.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        if !matches!(
            key.as_str(),
            "--read-origins" | "--write-origins" | "--confirm"
        ) {
            i += 1;
            continue;
        }
        args.remove(i);
        let v = match inline {
            Some(v) => v,
            None if i < args.len() => args.remove(i),
            None => return Err(format!("{key} needs a value")),
        };
        let items = v
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(String::from);
        match key.as_str() {
            "--read-origins" => g.read.extend(items),
            "--write-origins" => g.write.extend(items),
            _ => {
                for k in items {
                    if !matches!(k.as_str(), "submit" | "download" | "navigate-new-origin") {
                        return Err(format!(
                            "--confirm: unknown kind {k:?} (submit, download, navigate-new-origin)"
                        ));
                    }
                    g.confirm.insert(k);
                }
            }
        }
    }
    for o in g.read.iter().chain(&g.write) {
        if origin(o).is_none() {
            return Err(format!("not an origin: {o:?} (want scheme://host[:port])"));
        }
    }
    Ok(g)
}

/// MCP tool annotations (`readOnlyHint` / `destructiveHint`) for each tool, from the same policy
/// vocabulary: what a client may auto-approve versus what can leave the machine.
pub fn annotations(tool: &str) -> (bool, bool) {
    match tool {
        "observe" | "extract" | "find" | "screenshot" | "checkpoint" | "scroll" | "wait_for" => {
            (true, false)
        }
        "act" | "fill_form" | "type" | "commit" | "key" | "upload" | "drag" | "download" => {
            (false, true)
        }
        _ => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(tag: &str, ty: Option<&str>, ac: Option<&str>) -> NodeFacts {
        NodeFacts {
            tag: tag.into(),
            input_type: ty.map(Into::into),
            autocomplete: ac.map(Into::into),
            ..NodeFacts::default()
        }
    }
    fn form(mut n: NodeFacts, method: &str, action: &str) -> NodeFacts {
        n.form_method = Some(method.into());
        n.form_action = Some(action.into());
        n
    }
    const PAGE: &str = "https://shop.test/cart";

    /// The vendor-category table: 12 rows, expected verdicts fixed before the code.
    #[test]
    fn vendor_category_table() {
        let g = Grants {
            confirm: ["submit", "download", "navigate-new-origin"]
                .map(String::from)
                .into(),
            ..Grants::default()
        };
        let mut link = f("a", None, None);
        link.href = Some("https://shop.test/p".into());
        let mut ext = f("a", None, None);
        ext.href = Some("https://other.test/".into());
        let mut dl = f("a", None, None);
        dl.href = Some("https://shop.test/x.pdf".into());
        dl.download = true;
        let rows: Vec<(&str, Op, Option<NodeFacts>, &str, &str)> = vec![
            (
                "type password",
                Op::Type { submit: false },
                Some(f("input", Some("password"), None)),
                "",
                "handoff",
            ),
            (
                "type cc-number",
                Op::Type { submit: false },
                Some(f("input", Some("text"), Some("cc-number"))),
                "",
                "handoff",
            ),
            (
                "type otp",
                Op::Type { submit: false },
                Some(f("input", Some("text"), Some("one-time-code"))),
                "",
                "handoff",
            ),
            (
                "type search q",
                Op::Type { submit: false },
                Some(f("input", Some("search"), None)),
                "",
                "allow",
            ),
            (
                "type username",
                Op::Type { submit: false },
                Some(f("input", Some("email"), Some("username"))),
                "",
                "allow",
            ),
            (
                "plain search button",
                Op::Click,
                Some(form(
                    f("button", None, None),
                    "get",
                    "https://shop.test/search",
                )),
                "Search",
                "allow",
            ),
            (
                "post submit same origin",
                Op::Click,
                Some(form(
                    f("button", None, None),
                    "post",
                    "https://shop.test/order",
                )),
                "Continue",
                "confirm",
            ),
            (
                "post submit cross origin",
                Op::Click,
                Some(form(
                    f("button", Some("submit"), None),
                    "post",
                    "https://evil.test/c",
                )),
                "Continue",
                "deny",
            ),
            (
                "same-origin link",
                Op::Click,
                Some(link),
                "Product",
                "allow",
            ),
            (
                "new-origin link",
                Op::Click,
                Some(ext),
                "Partner",
                "confirm",
            ),
            ("download link", Op::Click, Some(dl), "Get PDF", "confirm"),
            (
                "plain button named Menu",
                Op::Click,
                Some(f("button", Some("button"), None)),
                "Menu",
                "allow",
            ),
            (
                "button named Delete account",
                Op::Click,
                Some(f("button", Some("button"), None)),
                "Delete account",
                "confirm",
            ),
        ];
        for (label, op, facts, name, want) in rows {
            assert_eq!(
                g.assess(op, facts.as_ref(), name, PAGE).tag(),
                want,
                "{label}"
            );
        }
    }

    #[test]
    fn origin_sets() {
        let g = Grants {
            read: vec!["https://a.test".into()],
            write: vec!["https://a.test".into()],
            ..Grants::default()
        };
        assert_eq!(
            g.assess(
                Op::Navigate("https://a.test/x"),
                None,
                "",
                "https://a.test/"
            )
            .tag(),
            "allow"
        );
        assert_eq!(
            g.assess(
                Op::Navigate("https://evil.test/?c=1"),
                None,
                "",
                "https://a.test/"
            )
            .tag(),
            "deny"
        );
        let t = f("input", Some("text"), None);
        assert_eq!(
            g.assess(
                Op::Type { submit: false },
                Some(&t),
                "",
                "https://attacker.test/"
            )
            .tag(),
            "deny"
        );
        assert_eq!(
            g.assess(Op::Type { submit: false }, Some(&t), "", "https://a.test/")
                .tag(),
            "allow"
        );
        let leak = form(f("button", None, None), "get", "https://evil.test/?c=1");
        assert_eq!(
            g.assess(Op::Click, Some(&leak), "Go", "https://a.test/")
                .tag(),
            "deny"
        );
    }
}
