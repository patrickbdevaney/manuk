//! §4a — the **accessibility / semantic tree** over the arena DOM.
//!
//! One investment, two payoffs (IMPLEMENTATION.md § Phase 4):
//!
//! 1. **Human a11y** — this tree is the source for a screen-reader bridge (`accesskit`
//!    is the intended platform adapter; the mapping below is the engine-side half).
//! 2. **Agent observation** — a `role + accessible name` tree is a far better, and
//!    much less injection-prone, observation channel than raw text + screenshot
//!    (see `manuk-agent`'s `Observation` and the E6 prompt-injection retrofit).
//!
//! The DOM→tree mapping is hand-rolled against **HTML-AAM** (implicit roles) and
//! **WAI-ARIA** (`role` / `aria-*` overrides) plus a pragmatic subset of **accname**
//! (accessible-name computation). It is deliberately a *subset*, and every gap is
//! stated rather than silently approximated — see [`Role`] and [`accessible_name`].
//!
//! **Geometry (§4a):** [`build_tree_with_rects`] attaches each element's absolute
//! border-box from the layout fragment tree, enabling [`A11yNode::to_viewport_lines`]
//! (viewport-clipped, with a click point per element), [`A11yNode::hit_test`], and
//! [`A11yNode::find`] — so an agent can act by role+name or by coordinate rather than
//! by link index. Nodes with no laid-out box keep `bbox == None` and are omitted from
//! the viewport rendering, because an agent cannot click what has no place to click.
//!
//! **Not yet modelled (documented, not faked):** `aria-owns` re-parenting, live
//! regions, and the full accname §2 recursion (we do one level of `aria-labelledby`
//! dereference, not arbitrary nesting). Occlusion is not modelled either: `hit_test`
//! picks the smallest containing box, which is not the same as the topmost painted
//! box under a `z-index` stack.

use std::collections::HashMap;

use manuk_dom::{Dom, NodeId};

/// A rectangle in absolute document CSS pixels.
///
/// Deliberately defined here rather than imported from `manuk-layout`: this crate
/// stays dependency-lean (DOM only) so the `accesskit` bridge and the agent can use
/// it without pulling the layout/text/css stack. Callers convert.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Center point — where an agent should click this element.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Whether the two rects overlap (touching edges do not count).
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// The subset of ARIA roles we compute. `Generic` is the honest fallback for
/// containers that carry no semantics (`div`, `span`, `a` without `href`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Document,
    Article,
    Banner,
    Complementary,
    ContentInfo,
    Form,
    Main,
    Navigation,
    Region,
    Search,

    Heading { level: u8 },
    Paragraph,
    Separator,

    Link,
    Button,
    TextBox,
    CheckBox,
    Radio,
    ComboBox,

    Image,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    RowHeader,

    Generic,
}

impl Role {
    /// Whether an agent (or user) can meaningfully *act* on a node of this role — click,
    /// type, or toggle it. Used for readiness/affordance counting.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Role::Link
                | Role::Button
                | Role::TextBox
                | Role::CheckBox
                | Role::Radio
                | Role::ComboBox
        )
    }

    /// Roles whose accessible name is computed **from their subtree text**
    /// (accname "name from content"). Others must get a name from an explicit
    /// attribute, or have none.
    pub fn name_from_content(&self) -> bool {
        matches!(
            self,
            Role::Link
                | Role::Button
                | Role::Heading { .. }
                | Role::ListItem
                | Role::Cell
                | Role::ColumnHeader
                | Role::RowHeader
                | Role::Row
        )
    }

    /// The lowercase ARIA role token, as a screen reader / `accesskit` would name it.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Document => "document",
            Role::Article => "article",
            Role::Banner => "banner",
            Role::Complementary => "complementary",
            Role::ContentInfo => "contentinfo",
            Role::Form => "form",
            Role::Main => "main",
            Role::Navigation => "navigation",
            Role::Region => "region",
            Role::Search => "search",
            Role::Heading { .. } => "heading",
            Role::Paragraph => "paragraph",
            Role::Separator => "separator",
            Role::Link => "link",
            Role::Button => "button",
            Role::TextBox => "textbox",
            Role::CheckBox => "checkbox",
            Role::Radio => "radio",
            Role::ComboBox => "combobox",
            Role::Image => "image",
            Role::List => "list",
            Role::ListItem => "listitem",
            Role::Table => "table",
            Role::Row => "row",
            Role::Cell => "cell",
            Role::ColumnHeader => "columnheader",
            Role::RowHeader => "rowheader",
            Role::Generic => "generic",
        }
    }

    /// Parse a role token (`"button"`, `"link"`, `"heading"`, …). Used both for
    /// explicit `role="…"` attributes and by callers naming a role (e.g. an agent
    /// action `{"action":"click_text","role":"button","name":"Sign in"}`).
    /// A bare `"heading"` has no level, so it parses as level 2 — see [`Role::matches`]
    /// for level-insensitive comparison.
    pub fn parse(tok: &str) -> Option<Role> {
        Role::from_aria_token(&tok.trim().to_ascii_lowercase())
    }

    /// Role equality that ignores a heading's level, so `parse("heading")` matches an
    /// `<h1>`. Exact `Role` equality (`==`) still compares levels.
    pub fn matches(&self, other: &Role) -> bool {
        match (self, other) {
            (Role::Heading { .. }, Role::Heading { .. }) => true,
            (a, b) => a == b,
        }
    }

    /// Parse an explicit `role="…"` token (first valid token wins, per ARIA).
    fn from_aria_token(tok: &str) -> Option<Role> {
        Some(match tok {
            "document" => Role::Document,
            "article" => Role::Article,
            "banner" => Role::Banner,
            "complementary" => Role::Complementary,
            "contentinfo" => Role::ContentInfo,
            "form" => Role::Form,
            "main" => Role::Main,
            "navigation" => Role::Navigation,
            "region" => Role::Region,
            "search" => Role::Search,
            "heading" => Role::Heading { level: 2 },
            "paragraph" => Role::Paragraph,
            "separator" => Role::Separator,
            "link" => Role::Link,
            "button" => Role::Button,
            "textbox" => Role::TextBox,
            "checkbox" => Role::CheckBox,
            "radio" => Role::Radio,
            "combobox" => Role::ComboBox,
            "img" | "image" => Role::Image,
            "list" => Role::List,
            "listitem" => Role::ListItem,
            "table" => Role::Table,
            "row" => Role::Row,
            "cell" | "gridcell" => Role::Cell,
            "columnheader" => Role::ColumnHeader,
            "rowheader" => Role::RowHeader,
            "generic" | "none" | "presentation" => Role::Generic,
            _ => return None,
        })
    }
}

/// One node of the accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNode {
    /// The arena node this was computed from.
    pub node: NodeId,
    pub role: Role,
    /// Accessible name (may be empty — an unnamed `generic` container is normal).
    pub name: String,
    /// Absolute border box, when the element produced one. `None` for elements the
    /// layout never boxed — an agent has nowhere to click those.
    pub bbox: Option<Rect>,
    /// Effective stacking layer (z-index) of this node, for occlusion-aware hit-testing —
    /// a higher-`z` box on top wins a click even if a lower-`z` box also contains the point.
    /// `0` for the common non-positioned case (then hit-testing falls back to deepest-wins).
    pub z: i32,
    /// Interaction state — checked, expanded, selected, disabled, value. **This is what lets an
    /// agent confirm its own action.** See [`A11yState`].
    pub state: A11yState,
    pub children: Vec<A11yNode>,
}

/// Tri-state checkedness. A checkbox is not a boolean: `mixed` is the real third value a
/// "select all" parent checkbox shows, and flattening it to `false` tells an agent the opposite of
/// what the page means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    False,
    True,
    Mixed,
}

/// The interaction state of an accessibility node.
///
/// **Why this exists, and it is the agentic moat rather than a nicety.** Without it the tree says
/// `checkbox "Remember me"` before a click and `checkbox "Remember me"` after it — identical. An
/// agent that cannot observe the result of its own action cannot verify it, so it either proceeds on
/// faith or re-clicks and toggles the setting back off. Every field here is one an agent needs to
/// answer "did that work?".
///
/// `Option` means **not applicable** rather than false: a link is not "unchecked", it simply has no
/// checkedness, and reporting `checked: false` on it would be a lie an agent could act on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct A11yState {
    /// Checkboxes, radios, and anything with `aria-checked` (including `mixed`).
    pub checked: Option<Checked>,
    /// Disclosure state — `aria-expanded`, and `<details open>`. How an agent knows whether the
    /// menu it just clicked actually opened.
    pub expanded: Option<bool>,
    /// `<option selected>` and `aria-selected` (tabs, listbox rows, grid cells).
    pub selected: Option<bool>,
    /// `disabled` or `aria-disabled`. An agent that clicks a disabled button waits forever for a
    /// result that is never coming; this is what tells it not to.
    pub disabled: bool,
    /// `required` / `aria-required` — which field a blocked form submission is complaining about.
    pub required: bool,
    /// `readonly` / `aria-readonly`.
    pub readonly: bool,
    /// The element has DOM focus. Host-owned (the shell tracks it), so it is only populated by
    /// [`build_tree_with_focus`]; the plain builders leave it `false`.
    pub focused: bool,
    /// Current value: a field's text, a select's chosen option, or `aria-valuenow` for a slider or
    /// progress bar. This is how an agent reads back what it just typed.
    pub value: Option<String>,
}

impl A11yState {
    /// Nothing to report — the common case for static content, and rendered as no suffix at all.
    pub fn is_empty(&self) -> bool {
        *self == A11yState::default()
    }

    /// A compact agent-readable suffix, e.g. ` [checked disabled value="ada"]`. Empty when there is
    /// no state, so a static document's observation lines are unchanged.
    pub fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        match self.checked {
            Some(Checked::True) => parts.push("checked".into()),
            Some(Checked::False) => parts.push("unchecked".into()),
            Some(Checked::Mixed) => parts.push("mixed".into()),
            None => {}
        }
        if let Some(e) = self.expanded {
            parts.push(if e {
                "expanded".into()
            } else {
                "collapsed".into()
            });
        }
        if let Some(true) = self.selected {
            parts.push("selected".into());
        }
        if self.disabled {
            parts.push("disabled".into());
        }
        if self.required {
            parts.push("required".into());
        }
        if self.readonly {
            parts.push("readonly".into());
        }
        if self.focused {
            parts.push("focused".into());
        }
        if let Some(v) = &self.value {
            if !v.is_empty() {
                parts.push(format!("value={v:?}"));
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(" [{}]", parts.join(" "))
        }
    }
}

/// Whether `node` is disabled by its own attribute or by an ancestor `<fieldset disabled>`.
/// Only a `<fieldset>` propagates disabledness; a disabled `<div>` means nothing.
fn inherits_disabled(dom: &Dom, node: NodeId) -> bool {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(e) = dom.element(n) {
            if e.attr("disabled").is_some() && (n == node || e.name == "fieldset") {
                return true;
            }
        }
        cur = dom.parent(n);
    }
    false
}

/// Read an element's interaction state out of the DOM.
///
/// ARIA wins over the native attribute where both are present, which is the cascade assistive tech
/// uses: an author who wrote `aria-checked="mixed"` on a checkbox means it, and the native attribute
/// cannot express `mixed` at all.
pub fn state_of(dom: &Dom, node: NodeId, role: &Role) -> A11yState {
    let Some(el) = dom.element(node) else {
        return A11yState::default();
    };
    let tag = el.name.as_str();
    let attr = |n: &str| el.attr(n);
    let aria_bool = |n: &str| match attr(n) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let ty = attr("type").unwrap_or("").to_ascii_lowercase();

    // Checked. `el.checked = true` from script writes the `checked` attribute (see the reflector),
    // so reading the attribute sees script-driven state as well as authored state.
    let checked = match attr("aria-checked") {
        Some("mixed") => Some(Checked::Mixed),
        Some("true") => Some(Checked::True),
        Some("false") => Some(Checked::False),
        _ if tag == "input" && (ty == "checkbox" || ty == "radio") => {
            Some(if attr("checked").is_some() {
                Checked::True
            } else {
                Checked::False
            })
        }
        _ => None,
    };

    let expanded = aria_bool("aria-expanded").or(if tag == "details" {
        Some(attr("open").is_some())
    } else {
        None
    });

    let selected = aria_bool("aria-selected").or(if tag == "option" {
        Some(attr("selected").is_some())
    } else {
        None
    });

    // Value. A text field's `value`, a slider's `aria-valuenow`, a progress/meter's `value`.
    let value = match tag {
        "input"
            if !matches!(
                ty.as_str(),
                "checkbox" | "radio" | "submit" | "button" | "reset"
            ) =>
        {
            attr("value").map(str::to_string)
        }
        "textarea" => Some(dom.text_content(node)),
        "progress" | "meter" => attr("value").map(str::to_string),
        _ => attr("aria-valuenow").map(str::to_string),
    }
    .filter(|v| !v.is_empty());

    let _ = role;
    A11yState {
        checked,
        expanded,
        selected,
        // Disabledness is INHERITED from an ancestor `<fieldset disabled>` — the idiomatic way to
        // disable a whole step of a multi-step form. Reporting only the control's own attribute
        // tells an agent that every control in that fieldset is actionable when none of them are.
        disabled: inherits_disabled(dom, node) || aria_bool("aria-disabled") == Some(true),
        required: attr("required").is_some() || aria_bool("aria-required") == Some(true),
        readonly: attr("readonly").is_some() || aria_bool("aria-readonly") == Some(true),
        focused: false, // host-owned; filled in by `build_tree_with_focus`
        value,
    }
}

/// The result of [`A11yNode::diff`]: semantic `(role, name)` nodes that appeared
/// (`added`) or disappeared (`removed`) between two accessibility snapshots.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct A11yDiff {
    pub added: Vec<(Role, String)>,
    pub removed: Vec<(Role, String)>,
}

impl A11yDiff {
    /// No semantic change between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// A compact agent-readable summary, e.g. `+button "Submit"  -link "Sign in"`.
    /// Empty string when nothing changed.
    pub fn summarize(&self) -> String {
        let mut parts = Vec::new();
        for (r, n) in &self.added {
            parts.push(format!("+{} {:?}", r.as_str(), n));
        }
        for (r, n) in &self.removed {
            parts.push(format!("-{} {:?}", r.as_str(), n));
        }
        parts.join("  ")
    }
}

impl A11yNode {
    /// Depth-first iteration over `self` and all descendants.
    pub fn iter(&self) -> impl Iterator<Item = &A11yNode> {
        let mut out = Vec::new();
        collect(self, &mut out);
        out.into_iter()
    }

    /// Nodes that carry semantics (unnamed `generic` containers are noise).
    fn interesting(&self) -> impl Iterator<Item = &A11yNode> {
        self.iter()
            .filter(|n| n.role != Role::Generic || !n.name.is_empty())
    }

    /// A concise **semantic diff** against a previous accessibility snapshot: which
    /// semantic (role + name) nodes appeared or disappeared. Computed in-process from two
    /// owned trees, so it is race-free — no serialization, no cross-process staleness that
    /// a CDP/WebDriver diff would suffer. An agent calls this after an action to see *what
    /// changed* (e.g. "a `dialog` opened", "the `Sign in` button is gone") instead of
    /// re-reading and re-reasoning over the whole tree. Nodes are keyed by
    /// `(role, lowercased-name)`; a renamed node reads as one removal + one addition.
    pub fn diff(&self, prev: &A11yNode) -> A11yDiff {
        use std::collections::HashSet;
        let key = |n: &A11yNode| (n.role.clone(), n.name.to_ascii_lowercase());
        let before: HashSet<(Role, String)> = prev.interesting().map(&key).collect();
        let after: HashSet<(Role, String)> = self.interesting().map(&key).collect();
        let sort = |mut v: Vec<(Role, String)>| {
            v.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()).then_with(|| a.1.cmp(&b.1)));
            v
        };
        A11yDiff {
            added: sort(after.difference(&before).cloned().collect()),
            removed: sort(before.difference(&after).cloned().collect()),
        }
    }

    fn render(n: &A11yNode) -> String {
        format!("{}{}", Self::render_role_name(n), n.state.render())
    }

    fn render_role_name(n: &A11yNode) -> String {
        match &n.role {
            Role::Heading { level } if !n.name.is_empty() => {
                format!("heading level {level} {:?}", n.name)
            }
            Role::Heading { level } => format!("heading level {level}"),
            r if n.name.is_empty() => r.as_str().to_string(),
            r => format!("{} {:?}", r.as_str(), n.name),
        }
    }

    /// A flat, human/agent-readable rendering: one `role "name"` line per node that
    /// carries semantics (unnamed `generic` containers are skipped as noise).
    pub fn to_observation_lines(&self) -> Vec<String> {
        self.interesting().map(Self::render).collect()
    }

    /// §4a — the same rendering **clipped to the viewport**, with each element's
    /// click point appended. An agent can act on these directly: "click at (x, y)".
    ///
    /// `viewport` is in absolute document coordinates (i.e. already offset by the
    /// current scroll), so a caller scrolled to `scroll_y` passes
    /// `Rect { y: scroll_y, height: viewport_height, .. }`. Nodes with no geometry
    /// (`bbox == None`) are **omitted**, because an agent cannot act on them and
    /// listing them would imply it could.
    pub fn to_viewport_lines(&self, viewport: Rect) -> Vec<String> {
        self.interesting()
            .filter_map(|n| {
                let b = n.bbox?;
                if !b.intersects(&viewport) {
                    return None;
                }
                let (cx, cy) = b.center();
                Some(format!("{} @({:.0},{:.0})", Self::render(n), cx, cy))
            })
            .collect()
    }

    /// The first node matching `role` whose accessible name equals `name`
    /// (case-insensitive). This is how an agent says "click the *Sign in* button"
    /// without needing a CSS selector. Heading levels are ignored (see [`Role::matches`]).
    pub fn find(&self, role: &Role, name: &str) -> Option<&A11yNode> {
        self.iter()
            .find(|n| n.role.matches(role) && n.name.eq_ignore_ascii_case(name))
    }

    /// As [`Self::find`], but matches any node whose name *contains* `name`
    /// (case-insensitive) — models are imprecise about exact label text.
    pub fn find_containing(&self, role: &Role, name: &str) -> Option<&A11yNode> {
        let needle = name.trim().to_ascii_lowercase();
        self.iter()
            .find(|n| n.role.matches(role) && n.name.to_ascii_lowercase().contains(needle.as_str()))
    }

    /// The deepest node whose `bbox` contains `(x, y)` — hit-testing for click-by-
    /// coordinate. Deepest wins, since a button inside a `main` should beat the `main`.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&A11yNode> {
        // Occlusion-aware: the box on the highest stacking layer (`z`) that contains the
        // point wins — so a `position:fixed`/high-`z` overlay beats content beneath it.
        // Within a layer, the smallest (deepest) box wins; on an exact area tie the deeper
        // node wins, and since `iter()` is pre-order "deeper" is "seen later" (`<=`).
        let mut best: Option<&A11yNode> = None;
        for n in self.iter() {
            let Some(b) = n.bbox else { continue };
            if !(x >= b.x && x < b.right() && y >= b.y && y < b.bottom()) {
                continue;
            }
            let area = b.width * b.height;
            match best {
                None => best = Some(n),
                Some(bn) => {
                    let ba = bn.bbox.map_or(f32::MAX, |r| r.width * r.height);
                    if n.z > bn.z || (n.z == bn.z && area <= ba) {
                        best = Some(n);
                    }
                }
            }
        }
        best
    }
}

fn collect<'a>(n: &'a A11yNode, out: &mut Vec<&'a A11yNode>) {
    out.push(n);
    for c in &n.children {
        collect(c, out);
    }
}

/// Tags that never surface in the accessibility tree (they carry no content).
fn is_non_rendered_tag(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "head" | "meta" | "link" | "title" | "noscript" | "template"
    )
}

/// Whether this element (and its subtree) is excluded from the a11y tree.
pub fn is_hidden(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if is_non_rendered_tag(&el.name) {
        return true;
    }
    if el.attr("hidden").is_some() {
        return true;
    }
    if el
        .attr("aria-hidden")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        return true;
    }
    // `<input type=hidden>` is not exposed.
    if el.name == "input"
        && el
            .attr("type")
            .is_some_and(|t| t.eq_ignore_ascii_case("hidden"))
    {
        return true;
    }
    false
}

/// The element's role: an explicit `role="…"` token if valid, else the HTML-AAM
/// implicit role for its tag. `None` means "expose no node" (e.g. `<img alt="">`,
/// which HTML-AAM maps to `presentation`).
pub fn role_of(dom: &Dom, node: NodeId) -> Option<Role> {
    let el = dom.element(node)?;

    if let Some(explicit) = el.attr("role") {
        // ARIA: the first *valid* token wins; invalid tokens fall through to implicit.
        if let Some(r) = explicit
            .split_ascii_whitespace()
            .find_map(Role::from_aria_token)
        {
            return Some(r);
        }
    }

    Some(match el.name.as_str() {
        "a" | "area" => {
            if el.attr("href").is_some() {
                Role::Link
            } else {
                Role::Generic // an anchor without href has no link semantics
            }
        }
        "button" => Role::Button,
        "input" => match el
            .attr("type")
            .unwrap_or("text")
            .to_ascii_lowercase()
            .as_str()
        {
            "checkbox" => Role::CheckBox,
            "radio" => Role::Radio,
            "button" | "submit" | "reset" | "image" => Role::Button,
            // `hidden` is filtered by `is_hidden` before we get here.
            _ => Role::TextBox,
        },
        "textarea" => Role::TextBox,
        "select" => Role::ComboBox,
        "h1" => Role::Heading { level: 1 },
        "h2" => Role::Heading { level: 2 },
        "h3" => Role::Heading { level: 3 },
        "h4" => Role::Heading { level: 4 },
        "h5" => Role::Heading { level: 5 },
        "h6" => Role::Heading { level: 6 },
        "img" => {
            // HTML-AAM: `alt=""` is an explicit "decorative" signal → no node at all.
            match el.attr("alt") {
                Some("") => return None,
                _ => Role::Image,
            }
        }
        // N4 — a `<slot>` is a rendering hole, not a semantic node: its assigned nodes
        // take its place in the flat tree, so it exposes no a11y node of its own.
        "slot" => return None,
        "ul" | "ol" => Role::List,
        "li" => Role::ListItem,
        "table" => Role::Table,
        "tr" => Role::Row,
        "td" => Role::Cell,
        "th" => {
            // HTML-AAM: scope decides column vs row header; default to column.
            if el
                .attr("scope")
                .is_some_and(|s| s.eq_ignore_ascii_case("row"))
            {
                Role::RowHeader
            } else {
                Role::ColumnHeader
            }
        }
        "nav" => Role::Navigation,
        "main" => Role::Main,
        "header" => Role::Banner,
        "footer" => Role::ContentInfo,
        "form" => Role::Form,
        "article" => Role::Article,
        // HTML-AAM: `<section>` is only a `region` when it has an accessible name.
        "section" => {
            if has_explicit_name(dom, node) {
                Role::Region
            } else {
                Role::Generic
            }
        }
        "aside" => Role::Complementary,
        "p" => Role::Paragraph,
        "hr" => Role::Separator,
        "html" => Role::Document,
        _ => Role::Generic,
    })
}

/// Whether an element carries a naming attribute (used by the `<section>` rule).
fn has_explicit_name(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    el.attr("aria-label").is_some_and(|v| !v.trim().is_empty())
        || el
            .attr("aria-labelledby")
            .is_some_and(|v| !v.trim().is_empty())
        || el.attr("title").is_some_and(|v| !v.trim().is_empty())
}

/// Build an `id` → node index once, so `aria-labelledby` / `<label for>` are O(1).
fn id_index(dom: &Dom) -> HashMap<String, NodeId> {
    let mut map = HashMap::new();
    let mut stack = vec![dom.root()];
    while let Some(n) = stack.pop() {
        if let Some(el) = dom.element(n) {
            if let Some(id) = el.id() {
                map.entry(id.to_string()).or_insert(n);
            }
        }
        stack.extend(dom.children(n));
    }
    map
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Accessible name for `node`, following a pragmatic subset of **accname**:
///
/// 1. `aria-labelledby` (dereferenced one level — no recursion into further
///    `aria-labelledby`, which is the documented gap)
/// 2. `aria-label`
/// 3. native host-language label: `<img alt>`, `<input value/placeholder>`,
///    an associated `<label for=…>`
/// 4. subtree text, but **only** for roles with "name from content"
/// 5. `title` attribute
pub fn accessible_name(dom: &Dom, node: NodeId, role: &Role) -> String {
    let index = id_index(dom);
    accessible_name_with(dom, node, role, &index)
}

fn accessible_name_with(
    dom: &Dom,
    node: NodeId,
    role: &Role,
    index: &HashMap<String, NodeId>,
) -> String {
    let Some(el) = dom.element(node) else {
        return String::new();
    };

    // 1. aria-labelledby → concat the referenced elements' text, in order.
    if let Some(refs) = el.attr("aria-labelledby") {
        let text = refs
            .split_ascii_whitespace()
            .filter_map(|id| index.get(id))
            .map(|&n| normalize(&dom.text_content(n)))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() {
            return text;
        }
    }

    // 2. aria-label
    if let Some(label) = el.attr("aria-label") {
        let label = normalize(label);
        if !label.is_empty() {
            return label;
        }
    }

    // 3. native host-language labelling
    match el.name.as_str() {
        "img" | "area" => {
            if let Some(alt) = el.attr("alt") {
                let alt = normalize(alt);
                if !alt.is_empty() {
                    return alt;
                }
            }
        }
        "input" | "textarea" | "select" => {
            // <label for="id">
            if let Some(id) = el.id() {
                if let Some(text) = find_label_for(dom, id) {
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
            // Button-ish inputs are named by their `value`.
            if el.name == "input" {
                if matches!(role, Role::Button) {
                    if let Some(v) = el.attr("value") {
                        let v = normalize(v);
                        if !v.is_empty() {
                            return v;
                        }
                    }
                }
                // accname allows `placeholder` as a last-resort native label.
                if let Some(p) = el.attr("placeholder") {
                    let p = normalize(p);
                    if !p.is_empty() {
                        return p;
                    }
                }
            }
        }
        _ => {}
    }

    // 4. name from content (only for roles that allow it)
    if role.name_from_content() {
        let text = normalize(&dom.text_content(node));
        if !text.is_empty() {
            return text;
        }
    }

    // 5. title fallback
    if let Some(t) = el.attr("title") {
        let t = normalize(t);
        if !t.is_empty() {
            return t;
        }
    }

    String::new()
}

/// Text of the first `<label for="{id}">` in the document.
fn find_label_for(dom: &Dom, id: &str) -> Option<String> {
    let mut stack = vec![dom.root()];
    while let Some(n) = stack.pop() {
        if let Some(el) = dom.element(n) {
            if el.name == "label" && el.attr("for") == Some(id) {
                return Some(normalize(&dom.text_content(n)));
            }
        }
        stack.extend(dom.children(n));
    }
    None
}

/// Build the accessibility tree for the document.
///
/// Hidden subtrees are pruned entirely. Elements whose role resolves to `None`
/// (e.g. `<img alt="">`, i.e. `presentation`) are dropped but their children are
/// **kept and reparented**, matching how ARIA `role=presentation` behaves.
pub fn build_tree(dom: &Dom) -> A11yNode {
    build_tree_with_rects(dom, &HashMap::new())
}

/// The effective stacking layer per node, for occlusion-aware hit-testing (see [`A11yNode::z`]).
pub type ZIndex = HashMap<NodeId, i32>;

/// Build the accessibility tree, attaching **element geometry** from `rects`.
///
/// `rects` maps a DOM node to its absolute border-box rect — produced by
/// `manuk_layout::LayoutBox::node_rects()` (converted to this crate's [`Rect`]).
/// Nodes with no entry keep `bbox == None`, which is honest: an anonymous or
/// unlaid-out node has no place to click.
pub fn build_tree_with_rects(dom: &Dom, rects: &HashMap<NodeId, Rect>) -> A11yNode {
    build_tree_with_geometry(dom, rects, &HashMap::new())
}

/// As [`build_tree_with_rects`], plus a per-node effective stacking layer (`z_index`, from
/// the page's z-index map) so [`A11yNode::hit_test`] is occlusion-aware.
pub fn build_tree_with_geometry(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
) -> A11yNode {
    let index = id_index(dom);
    let root = dom.root();
    let children = build_children(dom, root, &index, rects, z_index);
    A11yNode {
        node: root,
        role: Role::Document,
        name: String::new(),
        bbox: None,
        z: 0,
        state: A11yState::default(),
        children,
    }
}

/// As [`build_tree_with_geometry`], plus the **focused** node — which the host owns (the shell
/// tracks focus and publishes it into the JS world via `set_view_state`), so it cannot be read out
/// of the DOM here. A caller that knows the focused node passes it and gets `state.focused` filled
/// in; the plain builders leave it `false` rather than guessing.
pub fn build_tree_with_focus(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    focused: Option<NodeId>,
) -> A11yNode {
    let mut tree = build_tree_with_geometry(dom, rects, z_index);
    if let Some(f) = focused {
        mark_focused(&mut tree, f);
    }
    tree
}

fn mark_focused(node: &mut A11yNode, focused: NodeId) {
    if node.node == focused {
        node.state.focused = true;
    }
    for c in &mut node.children {
        mark_focused(c, focused);
    }
}

fn build_children(
    dom: &Dom,
    parent: NodeId,
    index: &HashMap<String, NodeId>,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
) -> Vec<A11yNode> {
    let mut out = Vec::new();
    // N3/N4 — the FLAT tree: a shadow host exposes its shadow content, and a `<slot>`
    // exposes the light-DOM nodes assigned to it. That is what a screen reader reads.
    for child in dom.flat_children(parent) {
        if !dom.is_element(child) {
            continue; // text nodes contribute to names, not to tree nodes
        }
        if is_hidden(dom, child) {
            continue;
        }
        // The tree root already *is* the document; `<html>` must not nest a second
        // `document` node inside it. Reparent its children instead.
        if dom.element(child).is_some_and(|e| e.name == "html") {
            out.extend(build_children(dom, child, index, rects, z_index));
            continue;
        }
        match role_of(dom, child) {
            Some(role) => {
                let name = accessible_name_with(dom, child, &role, index);
                let state = state_of(dom, child, &role);
                out.push(A11yNode {
                    node: child,
                    role,
                    name,
                    bbox: rects.get(&child).copied(),
                    z: z_index.get(&child).copied().unwrap_or(0),
                    state,
                    children: build_children(dom, child, index, rects, z_index),
                });
            }
            // presentational: drop the node, keep (reparent) its children
            None => out.extend(build_children(dom, child, index, rects, z_index)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(role: Role, name: &str) -> A11yNode {
        A11yNode {
            node: NodeId(0),
            role,
            name: name.to_string(),
            bbox: None,
            z: 0,
            state: A11yState::default(),
            children: vec![],
        }
    }

    #[test]
    fn hit_test_is_occlusion_aware() {
        // A higher-layer overlay (z=10) covering a button (z=0) wins the click even though
        // it is larger — you can't click through a modal.
        let button = A11yNode {
            node: NodeId(1),
            role: Role::Button,
            name: "Buy".into(),
            bbox: Some(Rect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 20.0,
            }),
            z: 0,
            state: A11yState::default(),
            children: vec![],
        };
        let overlay = A11yNode {
            node: NodeId(2),
            role: Role::Generic,
            name: "dialog".into(),
            bbox: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            }),
            z: 10,
            state: A11yState::default(),
            children: vec![],
        };
        let root = A11yNode {
            node: NodeId(0),
            role: Role::Document,
            name: String::new(),
            bbox: None,
            z: 0,
            state: A11yState::default(),
            children: vec![button, overlay],
        };
        assert_eq!(root.hit_test(20.0, 15.0).map(|n| n.node), Some(NodeId(2)));
    }

    #[test]
    fn a11y_diff_reports_added_and_removed() {
        let before = A11yNode {
            node: NodeId(0),
            role: Role::Generic,
            name: String::new(),
            bbox: None,
            z: 0,
            state: A11yState::default(),
            children: vec![leaf(Role::Link, "Sign in"), leaf(Role::Button, "Menu")],
        };
        let after = A11yNode {
            node: NodeId(0),
            role: Role::Generic,
            name: String::new(),
            bbox: None,
            z: 0,
            state: A11yState::default(),
            children: vec![leaf(Role::Button, "Menu"), leaf(Role::Button, "Sign out")],
        };
        let d = after.diff(&before);
        assert_eq!(d.added, vec![(Role::Button, "sign out".to_string())]);
        assert_eq!(d.removed, vec![(Role::Link, "sign in".to_string())]);
        assert!(!d.is_empty());
        // No change against itself.
        assert!(after.diff(&after).is_empty());
    }

    /// Build a small DOM: root -> html -> body -> ...
    fn dom_with(body_children: impl FnOnce(&mut Dom, NodeId)) -> Dom {
        let mut dom = Dom::new();
        let html = dom.create_element("html");
        let body = dom.create_element("body");
        dom.append_child(dom.root(), html);
        dom.append_child(html, body);
        body_children(&mut dom, body);
        dom
    }

    #[test]
    fn implicit_roles_follow_html_aam() {
        let dom = dom_with(|d, body| {
            for (tag, attrs) in [
                ("nav", vec![]),
                ("main", vec![]),
                ("h3", vec![]),
                ("p", vec![]),
                ("button", vec![]),
                ("a", vec![("href", "/x")]),
                ("a", vec![]), // no href -> generic, NOT a link
                ("ul", vec![]),
                ("input", vec![("type", "checkbox")]),
                ("input", vec![("type", "submit")]),
                ("input", vec![]), // defaults to text
                ("select", vec![]),
                ("th", vec![("scope", "row")]),
            ] {
                let e = d.create_element(tag);
                for (k, v) in attrs {
                    d.set_attr(e, k, v);
                }
                d.append_child(body, e);
            }
        });

        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let roles: Vec<Role> = dom
            .children(body)
            .map(|c| role_of(&dom, c).unwrap())
            .collect();

        assert_eq!(
            roles,
            vec![
                Role::Navigation,
                Role::Main,
                Role::Heading { level: 3 },
                Role::Paragraph,
                Role::Button,
                Role::Link,
                Role::Generic, // <a> without href
                Role::List,
                Role::CheckBox,
                Role::Button, // input[type=submit]
                Role::TextBox,
                Role::ComboBox,
                Role::RowHeader,
            ]
        );
    }

    #[test]
    fn explicit_aria_role_overrides_implicit_and_invalid_falls_through() {
        let dom = dom_with(|d, body| {
            let a = d.create_element("div");
            d.set_attr(a, "role", "button");
            d.append_child(body, a);

            // First *valid* token wins; the bogus one is skipped.
            let b = d.create_element("div");
            d.set_attr(b, "role", "totally-bogus navigation");
            d.append_child(body, b);

            // All-invalid → fall back to the implicit role.
            let c = d.create_element("h1");
            d.set_attr(c, "role", "not-a-role");
            d.append_child(body, c);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let roles: Vec<Role> = dom
            .children(body)
            .map(|c| role_of(&dom, c).unwrap())
            .collect();
        assert_eq!(
            roles,
            vec![Role::Button, Role::Navigation, Role::Heading { level: 1 }]
        );
    }

    #[test]
    fn hidden_subtrees_are_pruned_and_decorative_images_reparent_children() {
        let dom = dom_with(|d, body| {
            let hidden = d.create_element("div");
            d.set_attr(hidden, "aria-hidden", "true");
            let inner = d.create_element("button");
            d.append_child(hidden, inner);
            d.append_child(body, hidden);

            let script = d.create_element("script");
            d.append_child(body, script);

            let h = d.create_element("div");
            d.set_attr(h, "hidden", "");
            d.append_child(body, h);

            // <img alt=""> is presentational: no node, but children survive.
            let img = d.create_element("img");
            d.set_attr(img, "alt", "");
            let kid = d.create_element("button");
            d.append_child(img, kid);
            d.append_child(body, img);

            let visible = d.create_element("h1");
            let t = d.create_text("Title");
            d.append_child(visible, t);
            d.append_child(body, visible);
        });

        let tree = build_tree(&dom);
        let lines = tree.to_observation_lines();
        // aria-hidden button, script, hidden div all gone. The decorative <img> is
        // gone but its <button> child was reparented. The heading survives with a name.
        assert_eq!(
            lines,
            vec!["document", "button", "heading level 1 \"Title\"",]
        );
    }

    #[test]
    fn accessible_name_precedence_labelledby_then_label_then_content() {
        let dom = dom_with(|d, body| {
            // aria-labelledby beats aria-label beats content
            let lbl = d.create_element("span");
            d.set_attr(lbl, "id", "l1");
            let lt = d.create_text("From labelledby");
            d.append_child(lbl, lt);
            d.append_child(body, lbl);

            let btn = d.create_element("button");
            d.set_attr(btn, "aria-labelledby", "l1");
            d.set_attr(btn, "aria-label", "From aria-label");
            let bt = d.create_text("From content");
            d.append_child(btn, bt);
            d.append_child(body, btn);

            // aria-label beats content
            let btn2 = d.create_element("button");
            d.set_attr(btn2, "aria-label", "Close dialog");
            let bt2 = d.create_text("X");
            d.append_child(btn2, bt2);
            d.append_child(body, btn2);

            // content only
            let a = d.create_element("a");
            d.set_attr(a, "href", "/docs");
            let at = d.create_text("  Read   the docs ");
            d.append_child(a, at);
            d.append_child(body, a);
        });

        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let kids: Vec<NodeId> = dom.children(body).collect();
        let name = |n: NodeId| {
            let r = role_of(&dom, n).unwrap();
            accessible_name(&dom, n, &r)
        };
        assert_eq!(name(kids[1]), "From labelledby");
        assert_eq!(name(kids[2]), "Close dialog");
        // whitespace normalized
        assert_eq!(name(kids[3]), "Read the docs");
    }

    #[test]
    fn input_named_by_label_for_then_placeholder() {
        let dom = dom_with(|d, body| {
            let lab = d.create_element("label");
            d.set_attr(lab, "for", "email");
            let lt = d.create_text("Email address");
            d.append_child(lab, lt);
            d.append_child(body, lab);

            let inp = d.create_element("input");
            d.set_attr(inp, "id", "email");
            d.set_attr(inp, "type", "text");
            d.set_attr(inp, "placeholder", "you@example.com");
            d.append_child(body, inp);

            // no label → placeholder is the fallback
            let inp2 = d.create_element("input");
            d.set_attr(inp2, "type", "text");
            d.set_attr(inp2, "placeholder", "Search");
            d.append_child(body, inp2);

            // submit button named by `value`
            let sub = d.create_element("input");
            d.set_attr(sub, "type", "submit");
            d.set_attr(sub, "value", "Send");
            d.append_child(body, sub);
        });

        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let kids: Vec<NodeId> = dom.children(body).collect();
        let name = |n: NodeId| {
            let r = role_of(&dom, n).unwrap();
            accessible_name(&dom, n, &r)
        };
        assert_eq!(name(kids[1]), "Email address"); // label beats placeholder
        assert_eq!(name(kids[2]), "Search");
        assert_eq!(name(kids[3]), "Send");
    }

    #[test]
    fn section_is_a_region_only_when_named() {
        let dom = dom_with(|d, body| {
            let plain = d.create_element("section");
            d.append_child(body, plain);
            let named = d.create_element("section");
            d.set_attr(named, "aria-label", "Sidebar");
            d.append_child(body, named);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let kids: Vec<NodeId> = dom.children(body).collect();
        assert_eq!(role_of(&dom, kids[0]), Some(Role::Generic));
        assert_eq!(role_of(&dom, kids[1]), Some(Role::Region));
    }

    /// The synthetic DOMs above are hand-built. This one goes through the **real**
    /// HTML parser, which inserts implied `<html>`/`<head>`/`<body>` — exercising the
    /// `<html>` reparenting and the `<head>`-subtree pruning on a realistic tree.
    #[test]
    fn builds_a_sane_tree_from_real_parsed_html() {
        let dom = manuk_html::parse(
            r#"<!doctype html>
            <title>Shop</title>
            <style>.x{color:red}</style>
            <body>
              <header><h1>Acme</h1></header>
              <nav aria-label="Primary">
                <a href="/">Home</a>
                <a href="/cart">Cart</a>
              </nav>
              <main>
                <h2>Products</h2>
                <img src="deco.png" alt="">
                <img src="hat.png" alt="A blue hat">
                <form>
                  <label for="q">Search products</label>
                  <input id="q" type="text" placeholder="type here">
                  <input type="submit" value="Go">
                </form>
                <ul><li>One</li><li>Two</li></ul>
              </main>
              <div hidden><button>Secret</button></div>
            </body>"#,
        );

        let lines = build_tree(&dom).to_observation_lines();

        // <title>/<style> live in <head> and must not appear; the hidden button is gone;
        // the decorative <img alt=""> produced no node. Exactly one `document` root.
        assert_eq!(lines.iter().filter(|l| *l == "document").count(), 1);
        assert!(!lines.iter().any(|l| l.contains("Secret")));
        assert!(!lines.iter().any(|l| l.contains("color:red")));

        assert!(lines.contains(&"banner".to_string()));
        assert!(lines.contains(&"heading level 1 \"Acme\"".to_string()));
        assert!(lines.contains(&"navigation \"Primary\"".to_string()));
        assert!(lines.contains(&"link \"Home\"".to_string()));
        assert!(lines.contains(&"link \"Cart\"".to_string()));
        assert!(lines.contains(&"main".to_string()));
        assert!(lines.contains(&"heading level 2 \"Products\"".to_string()));
        assert!(lines.contains(&"image \"A blue hat\"".to_string()));
        // label[for] names the input, beating its placeholder
        assert!(lines.contains(&"textbox \"Search products\"".to_string()));
        assert!(lines.contains(&"button \"Go\"".to_string()));
        assert!(lines.contains(&"list".to_string()));
        assert!(lines.contains(&"listitem \"One\"".to_string()));

        // Exactly one image node (the decorative one was dropped).
        assert_eq!(lines.iter().filter(|l| l.starts_with("image")).count(), 1);
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// §4a geometry: bboxes attach, the viewport clips, and each surviving element
    /// carries the point an agent should click.
    #[test]
    fn geometry_attaches_and_viewport_clips_with_click_points() {
        let dom = dom_with(|d, body| {
            let b1 = d.create_element("button");
            let t = d.create_text("Near");
            d.append_child(b1, t);
            d.append_child(body, b1);
            let b2 = d.create_element("button");
            let t = d.create_text("Far");
            d.append_child(b2, t);
            d.append_child(body, b2);
            let b3 = d.create_element("button"); // no rect -> no geometry
            let t = d.create_text("Unlaid");
            d.append_child(b3, t);
            d.append_child(body, b3);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let kids: Vec<NodeId> = dom.children(body).collect();

        let mut rects = HashMap::new();
        rects.insert(kids[0], rect(10.0, 20.0, 100.0, 40.0)); // in viewport
        rects.insert(kids[1], rect(10.0, 5000.0, 100.0, 40.0)); // far below
                                                                // kids[2] intentionally absent

        let tree = build_tree_with_rects(&dom, &rects);
        assert_eq!(
            tree.find(&Role::Button, "Near").unwrap().bbox,
            Some(rects[&kids[0]])
        );
        assert_eq!(tree.find(&Role::Button, "Unlaid").unwrap().bbox, None);

        // Viewport = the first 800px of the document.
        let lines = tree.to_viewport_lines(rect(0.0, 0.0, 1024.0, 800.0));
        // "Near" survives with its center; "Far" is clipped; "Unlaid" has no geometry.
        assert_eq!(lines, vec!["button \"Near\" @(60,40)"]);

        // Scrolled down, "Far" comes into view and "Near" leaves.
        let scrolled = tree.to_viewport_lines(rect(0.0, 4800.0, 1024.0, 800.0));
        assert_eq!(scrolled, vec!["button \"Far\" @(60,5020)"]);
    }

    #[test]
    fn hit_test_picks_the_deepest_containing_element() {
        let dom = dom_with(|d, body| {
            let main = d.create_element("main");
            let btn = d.create_element("button");
            let t = d.create_text("Go");
            d.append_child(btn, t);
            d.append_child(main, btn);
            d.append_child(body, main);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let main = dom.children(body).next().unwrap();
        let btn = dom.children(main).next().unwrap();

        let mut rects = HashMap::new();
        rects.insert(main, rect(0.0, 0.0, 1000.0, 1000.0));
        rects.insert(btn, rect(100.0, 100.0, 80.0, 30.0));

        let tree = build_tree_with_rects(&dom, &rects);
        // Inside the button: the button wins over the enclosing main.
        assert_eq!(
            tree.hit_test(120.0, 110.0).map(|n| n.role.clone()),
            Some(Role::Button)
        );
        // Outside the button but inside main.
        assert_eq!(
            tree.hit_test(500.0, 500.0).map(|n| n.role.clone()),
            Some(Role::Main)
        );
        // Outside everything.
        assert!(tree.hit_test(5000.0, 5000.0).is_none());
    }

    /// A wrapper exactly as large as its only child (`<form>` around a lone `<button>`)
    /// produces an area tie. The deeper element must win, or an agent clicking a button
    /// would "hit" the form.
    #[test]
    fn hit_test_breaks_area_ties_in_favor_of_the_deeper_element() {
        let dom = dom_with(|d, body| {
            let form = d.create_element("form");
            let btn = d.create_element("button");
            let t = d.create_text("Go");
            d.append_child(btn, t);
            d.append_child(form, btn);
            d.append_child(body, form);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let form = dom.children(body).next().unwrap();
        let btn = dom.children(form).next().unwrap();

        let mut rects = HashMap::new();
        let same = rect(0.0, 0.0, 100.0, 20.0);
        rects.insert(form, same);
        rects.insert(btn, same);

        let tree = build_tree_with_rects(&dom, &rects);
        assert_eq!(tree.hit_test(50.0, 10.0).map(|n| n.node), Some(btn));
    }

    #[test]
    fn find_is_case_insensitive_and_role_scoped() {
        let dom = dom_with(|d, body| {
            let a = d.create_element("a");
            d.set_attr(a, "href", "/in");
            let t = d.create_text("Sign In");
            d.append_child(a, t);
            d.append_child(body, a);
        });
        let tree = build_tree(&dom);
        assert!(tree.find(&Role::Link, "sign in").is_some());
        // Right name, wrong role.
        assert!(tree.find(&Role::Button, "sign in").is_none());
    }

    #[test]
    fn observation_lines_drop_unnamed_generics_as_noise() {
        let dom = dom_with(|d, body| {
            let wrap = d.create_element("div"); // unnamed generic -> noise
            let nav = d.create_element("nav");
            let a = d.create_element("a");
            d.set_attr(a, "href", "/home");
            let at = d.create_text("Home");
            d.append_child(a, at);
            d.append_child(nav, a);
            d.append_child(wrap, nav);
            d.append_child(body, wrap);
        });
        let tree = build_tree(&dom);
        assert_eq!(
            tree.to_observation_lines(),
            vec!["document", "navigation", "link \"Home\""]
        );
        // The generic wrapper still exists in the real tree (we only filter the view).
        assert!(tree.iter().any(|n| n.role == Role::Generic));
    }
}
