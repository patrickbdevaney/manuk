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

use std::collections::{HashMap, HashSet};

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

    /// The overlapping region, or `None` when they do not overlap. The *visible* part of an
    /// element is `bbox ∩ viewport`, and that — not the whole border box — is the only part of it
    /// an agent can aim at: the centre of a box half-scrolled off the screen is off the screen.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Some(Rect {
            x,
            y,
            width: self.right().min(other.right()) - x,
            height: self.bottom().min(other.bottom()) - y,
        })
    }
}

/// **Where a click aimed at a node will actually land** — the answer an agent needs *before* it
/// acts, and the one nothing in this codebase used to ask.
///
/// ⭐⭐⭐ **A CLICK POINT IS A CLAIM ABOUT THE HIT-TEST, AND IT WAS NEVER CHECKED AGAINST IT.**
/// Three separate entrances published `bbox.center()` as "where an agent should click this
/// element" — [`A11yNode::to_viewport_lines`] (the coordinates the *model* is shown, whose own doc
/// comment says "an agent can act on these directly"), `manuk_agent::targeting::resolve_target`,
/// and `manuk_agent::grounding::ground_action` — and not one of them ran the point back through
/// [`A11yNode::hit_test`]. On a page with a consent overlay (which is most of the web) the centre
/// of the *Sign in* button belongs to the cookie banner: the agent resolves the right node with
/// confidence `1.0`, clicks a coordinate that reaches the banner, the button's handler never runs,
/// and **every observable channel reports success**. Perception and actuation were both built and
/// nothing joined them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    /// A point that hit-tests back to the target **or to a descendant of it** — safe to click.
    /// A descendant counts because events bubble: clicking the `<span>` inside a `<button>`
    /// activates the button. An *ancestor* does not count — clicking the wrapping `<div>` does not.
    Clear { point: (f32, f32) },
    /// Something is on top of the target: no candidate point inside its visible box reaches it.
    /// `by` is what intercepted the centre — the node an agent should deal with first (dismiss the
    /// banner, close the modal) — and `point` is the centre, kept for reporting.
    Obstructed { by: NodeId, point: (f32, f32) },
    /// ⭐⭐⭐ **THE TARGET IS OFF THE SCREEN — SCROLL BY `dy` FIRST, THEN ASK AGAIN.** Not a
    /// refusal and not a point: a coordinate for a box the viewport does not contain is a
    /// coordinate no pointer can be at, and publishing one is the same silent misfire
    /// [`Landing::Obstructed`] exists to prevent, one branch over.
    ///
    /// **The obstruction map at scroll 0 is not the obstruction map at the scroll where the click
    /// happens**, which is why this cannot be answered by aiming at the box centre and hoping.
    /// A `position:sticky` header's *document* rect moves with the scroll: a checkbox at y=1000 is
    /// unobstructed while the header sits at y=0, and is underneath it the moment the agent
    /// scrolls far enough to see it. A below-the-fold target is **by definition** clicked after a
    /// scroll, so the verification has to be re-run in the viewport the click will actually happen
    /// in — which is what returning `dy` instead of a point forces the caller to do.
    ///
    /// `dy` is signed (positive = scroll down) and aligns the target's top edge with the top of
    /// the viewport — the alignment `Element.scrollIntoView()` defaults to (`block: "start"`).
    /// It is a *proposal*, not a promise: a caller that cannot scroll that far (the document ends
    /// first) still gets a truthful answer, because it re-grounds against wherever it landed
    /// rather than against this number.
    OffScreen { dy: f32 },
    /// The target has no box, or its box is on screen and **nothing inside it hit-tests back to
    /// it at all** — `pointer-events: none` on the target itself. There is nothing to aim at, and
    /// a coordinate would be a guess. A merely off-screen target is [`Landing::OffScreen`]; the
    /// two used to share this variant, which is how a scroll-away target came to be published as
    /// a click point with confidence `1.0`.
    Unreachable,
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

    // Interactive widget roles (ARIA). These are the custom controls modern web apps build out
    // of `<div>`s and `role="…"`, and the ones an agent most needs to *ground and act on*: a
    // switch to toggle, a tab to select, a slider to set, a menu item to choose. Before this they
    // all collapsed to `Generic`, so the agent saw an anonymous box where a togglable switch was —
    // it could click it but never name it, and never read the switch/tab state `state_of` already
    // computes from `aria-checked`/`aria-selected`. Their interaction state is unchanged: `state_of`
    // reads the `aria-*` attributes irrespective of role, so a `role="switch" aria-checked` already
    // reports `checked` — it was only missing a role to hang it on.
    Switch,
    Slider,
    SpinButton,
    Tab,
    MenuItem,
    Option,
    TreeItem,

    Image,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    RowHeader,

    // Container / grouping widget roles — the structures those interactive widgets live in. An
    // agent that finds a `tab` needs the enclosing `tablist` to know the set; a `dialog` tells it
    // a modal is up and the page behind it is inert.
    // ── ⭐ THE TEXT-LEVEL AND DOCUMENT-STRUCTURE ROLES ARIA 1.2/1.3 ADDED, AND THEY ARE NOT
    // DECORATION. A screen reader announces emphasis, a deletion, a term's definition; an agent
    // reading the tree needs `code` to know a run is a literal and `time` to know it is a date.
    // Every one of these collapsed to `Generic`, which is the tree saying "a box" about a word the
    // author marked up on purpose. `<del>`, `<ins>`, `<sub>`, `<sup>`, `<em>`, `<strong>`, `<code>`,
    // `<dfn>`, `<time>`, `<mark>`, `<blockquote>`, `<figure>` and `<caption>` are ordinary HTML.
    Blockquote,
    Caption,
    Code,
    Definition,
    Deletion,
    Insertion,
    Emphasis,
    Strong,
    Subscript,
    Superscript,
    Mark,
    Suggestion,
    Term,
    Time,
    Figure,
    Note,
    Application,
    Math,

    // Live regions past `alert`/`status` — a `log` and a `timer` announce differently.
    Log,
    Marquee,
    Timer,

    // ⚠ Grid structure the flat table roles COLLAPSED. `gridcell` is not `cell`: a grid is the
    // interactive widget (a spreadsheet, a data grid), a table is static content, and an agent that
    // cannot tell them apart cannot tell a document from an application.
    Grid,
    GridCell,
    RowGroup,

    Meter,
    ScrollBar,
    SearchBox,
    // Likewise `menuitemcheckbox`/`menuitemradio`: a checkable menu item was announced as an
    // ordinary one, so its whole point — that it has a state — was invisible in the role.
    MenuItemCheckBox,
    MenuItemRadio,

    // ARIA 1.3 SCOPED LANDMARKS: a `<header>`/`<footer>` inside sectioning content is NOT the
    // page's banner/contentinfo. Announcing every article's footer as THE page footer is worse
    // than announcing none.
    SectionHeader,
    SectionFooter,

    Menu,
    MenuBar,
    TabList,
    TabPanel,
    ListBox,
    Toolbar,
    Tree,
    Group,
    RadioGroup,
    Dialog,
    AlertDialog,
    Tooltip,
    Alert,
    Status,
    ProgressBar,

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
                | Role::Switch
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
                | Role::MenuItem
                // Split out of `MenuItem`/`TextBox` — they were interactive before and must stay so.
                | Role::MenuItemCheckBox
                | Role::MenuItemRadio
                | Role::SearchBox
                | Role::Option
                | Role::TreeItem
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
                // Split out of `Cell`; ARIA lists `gridcell` as name-from-content too.
                | Role::GridCell
                | Role::ColumnHeader
                | Role::RowHeader
                | Role::Row
                | Role::Tab
                | Role::MenuItem
                // Split out of `MenuItem`; ARIA gives both name-from-content, as menuitem has.
                | Role::MenuItemCheckBox
                | Role::MenuItemRadio
                | Role::Option
                | Role::Switch
                | Role::TreeItem
                | Role::Tooltip
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
            Role::Switch => "switch",
            Role::Slider => "slider",
            Role::SpinButton => "spinbutton",
            Role::ProgressBar => "progressbar",
            Role::Tab => "tab",
            Role::MenuItem => "menuitem",
            Role::Option => "option",
            Role::TreeItem => "treeitem",
            Role::Menu => "menu",
            Role::MenuBar => "menubar",
            Role::TabList => "tablist",
            Role::TabPanel => "tabpanel",
            Role::ListBox => "listbox",
            Role::Toolbar => "toolbar",
            Role::Tree => "tree",
            Role::Group => "group",
            Role::RadioGroup => "radiogroup",
            Role::Dialog => "dialog",
            Role::AlertDialog => "alertdialog",
            Role::Tooltip => "tooltip",
            Role::Alert => "alert",
            Role::Status => "status",
            Role::Image => "image",
            Role::List => "list",
            Role::ListItem => "listitem",
            Role::Table => "table",
            Role::Row => "row",
            Role::Cell => "cell",
            Role::ColumnHeader => "columnheader",
            Role::RowHeader => "rowheader",
            Role::Blockquote => "blockquote",
            Role::Caption => "caption",
            Role::Code => "code",
            Role::Definition => "definition",
            Role::Deletion => "deletion",
            Role::Insertion => "insertion",
            Role::Emphasis => "emphasis",
            Role::Strong => "strong",
            Role::Subscript => "subscript",
            Role::Superscript => "superscript",
            Role::Mark => "mark",
            Role::Suggestion => "suggestion",
            Role::Term => "term",
            Role::Time => "time",
            Role::Figure => "figure",
            Role::Note => "note",
            Role::Application => "application",
            Role::Math => "math",
            Role::Log => "log",
            Role::Marquee => "marquee",
            Role::Timer => "timer",
            Role::Grid => "grid",
            Role::GridCell => "gridcell",
            Role::RowGroup => "rowgroup",
            Role::Meter => "meter",
            Role::ScrollBar => "scrollbar",
            Role::SearchBox => "searchbox",
            Role::MenuItemCheckBox => "menuitemcheckbox",
            Role::MenuItemRadio => "menuitemradio",
            Role::SectionHeader => "sectionheader",
            Role::SectionFooter => "sectionfooter",
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
        // ⚠ **UN-COLLAPSING A ROLE MUST NOT BREAK THE CALLER THAT ASKED THE COARSE QUESTION.**
        // `gridcell` and `menuitemcheckbox`/`menuitemradio` used to BE `cell` and `menuitem`, and
        // `manuk-agent` targets by role name. Making them distinct is more correct in the tree and
        // would silently stop matching here; the coarse token still matches the specific role.
        match (self, other) {
            (Role::Heading { .. }, Role::Heading { .. }) => true,
            (Role::MenuItem, Role::MenuItemCheckBox | Role::MenuItemRadio)
            | (Role::MenuItemCheckBox | Role::MenuItemRadio, Role::MenuItem) => true,
            (Role::Cell, Role::GridCell) | (Role::GridCell, Role::Cell) => true,
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
            "switch" => Role::Switch,
            "slider" => Role::Slider,
            "spinbutton" => Role::SpinButton,
            "progressbar" => Role::ProgressBar,
            "tab" => Role::Tab,
            "menuitem" => Role::MenuItem,
            // ⚠ These two used to ground onto `menuitem`. A collapse is invisible in a tree dump
            // and wrong in the one place the role is read: `menuitemcheckbox` IS the announcement
            // that the item carries a state. `Role::matches` keeps `parse("menuitem")` matching
            // them, so nothing that asked the old question stops working.
            "menuitemcheckbox" => Role::MenuItemCheckBox,
            "menuitemradio" => Role::MenuItemRadio,
            "blockquote" => Role::Blockquote,
            "caption" => Role::Caption,
            "code" => Role::Code,
            "definition" => Role::Definition,
            "deletion" => Role::Deletion,
            "insertion" => Role::Insertion,
            "emphasis" => Role::Emphasis,
            "strong" => Role::Strong,
            "subscript" => Role::Subscript,
            "superscript" => Role::Superscript,
            "mark" => Role::Mark,
            "suggestion" => Role::Suggestion,
            "term" => Role::Term,
            "time" => Role::Time,
            "figure" => Role::Figure,
            "note" => Role::Note,
            "application" => Role::Application,
            "math" => Role::Math,
            "log" => Role::Log,
            "marquee" => Role::Marquee,
            "timer" => Role::Timer,
            "grid" => Role::Grid,
            "gridcell" => Role::GridCell,
            "rowgroup" => Role::RowGroup,
            "meter" => Role::Meter,
            "scrollbar" => Role::ScrollBar,
            "searchbox" => Role::SearchBox,
            "sectionheader" => Role::SectionHeader,
            "sectionfooter" => Role::SectionFooter,
            // `image` is the ARIA spelling and `img` the HTML one; both name the same role.
            "image" | "img" => Role::Image,
            "option" => Role::Option,
            "treeitem" => Role::TreeItem,
            "menu" => Role::Menu,
            "menubar" => Role::MenuBar,
            "tablist" => Role::TabList,
            "tabpanel" => Role::TabPanel,
            "listbox" => Role::ListBox,
            "toolbar" => Role::Toolbar,
            "tree" => Role::Tree,
            "group" => Role::Group,
            "radiogroup" => Role::RadioGroup,
            "dialog" => Role::Dialog,
            "alertdialog" => Role::AlertDialog,
            "tooltip" => Role::Tooltip,
            "alert" => Role::Alert,
            "status" => Role::Status,
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
    /// Whether this node is a hit-test target. `false` for a `pointer-events: none` element — it
    /// stays in the tree (a screen reader still announces it) but coordinate hit-testing passes
    /// THROUGH it to whatever is behind, so an agent grounding a click by coordinate does not land on
    /// a decorative overlay. Default `true`; only the live builder that holds the computed styles can
    /// set it, since `pointer-events` is a style, not a DOM attribute.
    pub hittable: bool,
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
    /// ⭐⭐⭐ **`aria-pressed` — A TOGGLE BUTTON'S ONLY OBSERVABLE STATE, and the field whose
    /// absence this struct's own doc comment describes.** *"Without it the tree says
    /// `checkbox "Remember me"` before a click and `checkbox "Remember me"` after it — identical."*
    /// That sentence was true of every toggle button on the web: `Follow`, `Bold`, `Mute`, a filter
    /// chip, a "show password" eye. They are `<button aria-pressed>`, not checkboxes, so `checked`
    /// never applied and the tree read `button "Follow"` in both states.
    ///
    /// Tri-state for the same reason `checked` is: `aria-pressed="mixed"` is a real authored value
    /// (a "bold" button over a mixed selection), and flattening it to `false` tells an agent the
    /// opposite of what the page means.
    pub pressed: Option<Checked>,
    /// `aria-invalid` — **the field a blocked form submission is complaining about**, which is the
    /// exact phrase [`A11yState::required`] already uses for its twin. An agent that submits a form,
    /// is refused, and re-reads the tree needs one bit to know WHICH field to fix; without it the
    /// only signal is that the page did not navigate.
    ///
    /// `aria-invalid` is an enumeration (`true` / `false` / `grammar` / `spelling`), and
    /// Chrome-measured, `grammar` and `spelling` both report **`invalid: 'true'`** — they say what
    /// KIND of wrong, not whether. So this is a `bool` and the three truthy spellings collapse.
    pub invalid: bool,
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
        // ⚠ `pressed` renders as its own word rather than reusing `checked`'s, because an agent
        // reading `[checked]` on a `button` would be reading about a checkbox that is not there.
        match self.pressed {
            Some(Checked::True) => parts.push("pressed".into()),
            Some(Checked::False) => parts.push("unpressed".into()),
            Some(Checked::Mixed) => parts.push("partially-pressed".into()),
            None => {}
        }
        if self.invalid {
            parts.push("invalid".into());
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
    // ── ⚠⚠⚠ **A `<fieldset disabled>` DISABLES ITS CONTROLS AND IS NOT ITSELF DISABLED.** The
    //    native `disabled` attribute belongs to the *listed form elements*; `<fieldset>` carries it
    //    as a PROPAGATOR. Chrome-measured (CDP `Accessibility.getFullAXTree`):
    //
    //    ```text
    //      <fieldset disabled>            role=group     NO `disabled` property
    //        <input type=checkbox>        role=checkbox  disabled: True
    //    ```
    //
    //    ⚠⚠ **THIS WAS INVISIBLE UNTIL t1384 GAVE `<fieldset>` A ROLE.** As a `generic` with no
    //    name the node was not printed in the observation lines at all, so a `disabled` it should
    //    never have had could not be seen. Promoting it to `group` — which is correct — published
    //    the wrong state, and `g_disabled_inert` (which counts the `disabled` lines) went red.
    //    *A latent wrong answer surfaces when the node it lives on becomes visible.*
    //
    //    ⚠ `aria-disabled` is NOT scoped this way and must not be: `<div role=button
    //    aria-disabled=true>` reports `disabled` in Chrome on any element, because the author said
    //    so explicitly. Only the NATIVE attribute belongs to controls. See `state_of`.
    if !matches!(
        dom.element(node).map(|e| e.name.as_str()),
        Some("input" | "select" | "textarea" | "button" | "option" | "optgroup")
    ) {
        return false;
    }
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

    // ⭐ The same tri-state shape as `checked`, and deliberately NOT the same field: a toggle button
    // is not a checkbox, and an agent told `[checked]` on a `button` is being told about a control
    // that is not there. `aria-pressed` has no native HTML twin, so there is no attribute fallback.
    let pressed = match attr("aria-pressed") {
        Some("mixed") => Some(Checked::Mixed),
        Some("true") => Some(Checked::True),
        Some("false") => Some(Checked::False),
        _ => None,
    };

    // ⚠ `aria-invalid` is an ENUMERATION, not a boolean: `grammar` and `spelling` are truthy and
    // say what KIND of wrong. Chrome-measured — both report `invalid: 'true'` — so they collapse.
    // Any other token (including a typo) is `false` per ARIA's enumerated-value rule, which is why
    // this is a match on the truthy set rather than `!= "false"`.
    let invalid = matches!(attr("aria-invalid"), Some("true" | "grammar" | "spelling"));

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
        pressed,
        invalid,
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
    ///
    /// ⭐ **The point is [`Landing`]-verified, and that is the whole difference between an agent
    /// that acts and one that thinks it acted.** This used to print `bbox.center()` unconditionally
    /// — a coordinate the model was told to click, that on any page with a consent overlay reached
    /// the overlay instead. Now the printed point is one that hit-tests back to the element, and an
    /// element nothing can reach is printed `obstructed` rather than with a lie for a coordinate.
    /// The obstructed node is still LISTED: an agent that can see the *Sign in* button is covered
    /// can dismiss what covers it, and one that cannot see the button at all can only give up.
    pub fn to_viewport_lines(&self, viewport: Rect) -> Vec<String> {
        let nodes: Vec<&A11yNode> = self
            .interesting()
            .filter(|n| n.bbox.is_some_and(|b| b.intersects(&viewport)))
            .collect();
        nodes
            .into_iter()
            .map(|n| {
                let line = Self::render(n);
                match self.landing(n.node, Some(viewport)) {
                    Landing::Clear { point: (x, y) } => format!("{line} @({x:.0},{y:.0})"),
                    Landing::Obstructed { point: (x, y), .. } => {
                        format!("{line} @({x:.0},{y:.0}) obstructed")
                    }
                    // The listing is filtered to boxes that intersect the viewport, so `OffScreen`
                    // cannot arise here — and now that it is its own variant, that sentence is a
                    // CHECKED claim rather than a comment: this arm is reached only by the
                    // `pointer-events: none` target, on screen and not clickable anywhere. Say so
                    // rather than drop the row.
                    Landing::OffScreen { .. } | Landing::Unreachable => {
                        let (x, y) = n.bbox.map(|b| b.center()).unwrap_or((0.0, 0.0));
                        format!("{line} @({x:.0},{y:.0}) obstructed")
                    }
                }
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
    ///
    /// Occlusion-aware: the box on the highest stacking layer (`z`) that contains the point wins,
    /// so a `position:fixed`/high-`z` overlay beats content beneath it. **Between two nodes on the
    /// same layer that are not related, the smaller box wins** — a button laid over a card is the
    /// more specific target.
    ///
    /// ⚠⚠⚠ **A DESCENDANT ALWAYS BEATS ITS ANCESTOR ON THE SAME LAYER, WHATEVER THE AREAS SAY.**
    /// This used to be a flat pre-order scan resolving *every* pair by area, with `<=` letting the
    /// deeper node win an **exact** tie because pre-order sees it later. That is only correct while
    /// an ancestor's box is never *smaller* than the descendant's — which held by accident, because
    /// a boxless inline's rect was lifted verbatim from its children and so was byte-identical to
    /// them.
    ///
    /// t853 gave every inline its own content area, and Wikipedia's `.hlist li { display: inline }`
    /// promptly produced a `<li>` a third of a pixel wider and a quarter-pixel taller than the `<a>`
    /// inside it. Float dust, and it inverted the answer: **16 links on the G6 page became
    /// unclickable**, because the shell walks *up* from whatever was hit looking for an `<a href>`
    /// and an ancestor `<li>` has no link above it. The geometry was right; the tie-break was
    /// resolving an ancestor/descendant question with a rule that only ever meant to order
    /// *unrelated* overlapping boxes.
    ///
    /// Chrome's `elementFromPoint` has no such ambiguity — the topmost, deepest element wins, full
    /// stop. So the walk is now a recursion that resolves the relationship structurally rather than
    /// numerically: a subtree reports its own best, and a hitting node loses to any hitting
    /// descendant on the same layer. Area only ever compares *siblings' subtrees*, which is the
    /// only place it was ever the right question.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&A11yNode> {
        fn area(n: &A11yNode) -> f32 {
            n.bbox.map_or(f32::MAX, |r| r.width * r.height)
        }
        // Two candidates from DIFFERENT subtrees: higher layer wins, then the smaller box. `<=`
        // keeps the later (document-order) node on an exact tie, which is what the flat scan did.
        fn across<'a>(a: &'a A11yNode, b: &'a A11yNode) -> &'a A11yNode {
            if b.z > a.z || (b.z == a.z && area(b) <= area(a)) {
                b
            } else {
                a
            }
        }
        fn go<'a>(n: &'a A11yNode, x: f32, y: f32) -> Option<&'a A11yNode> {
            let mut best: Option<&A11yNode> = None;
            for c in &n.children {
                if let Some(cand) = go(c, x, y) {
                    best = Some(match best {
                        None => cand,
                        Some(b) => across(b, cand),
                    });
                }
            }
            // `pointer-events: none` — transparent to hit-testing; the point passes through to the
            // element behind, so a decorative overlay never steals an agent's coordinate click.
            let hits = n.hittable
                && n.bbox
                    .is_some_and(|b| x >= b.x && x < b.right() && y >= b.y && y < b.bottom());
            match (best, hits) {
                // The descendant wins unless this node is on a HIGHER layer — a positioned ancestor
                // painted above its own overflowing content is the one case where it should not.
                (Some(b), true) => Some(if n.z > b.z { n } else { b }),
                (Some(b), false) => Some(b),
                (None, true) => Some(n),
                (None, false) => None,
            }
        }
        go(self, x, y)
    }

    /// **Verify a click point against the hit-test before an agent acts on it.** Called on the
    /// tree ROOT (hit-testing is a whole-page question — an overlay is somewhere else in the tree
    /// entirely), it returns where a click aimed at `target` actually lands. See [`Landing`].
    ///
    /// `viewport` clips the target to what is on screen, so the point is one a real pointer could
    /// reach; pass `None` to aim at the whole border box (document-coordinate callers that do not
    /// scroll).
    ///
    /// **The candidate ladder is deliberately short and ordered centre-first**, so a page with
    /// nothing covering anything gets exactly the point it got before — this is a strictly added
    /// check, not a new aiming policy. Only when the centre is intercepted does it try the four
    /// quadrant centres and then four near-corner insets, which is what rescues the very common
    /// half-covered case: a sticky header over the top of a link, a chat widget over one corner.
    /// The centre costs ONE hit-test on the overwhelmingly common clear path.
    pub fn landing(&self, target: NodeId, viewport: Option<Rect>) -> Landing {
        let Some(node) = find_node(self, target) else {
            return Landing::Unreachable;
        };
        let Some(bbox) = node.bbox else {
            return Landing::Unreachable;
        };
        let aim = match viewport {
            Some(v) => match bbox.intersection(&v) {
                Some(r) => r,
                // Off screen. A vertical scroll fixes this **only if the target is already within
                // the horizontal band** — a box parked off to the side comes no closer for any
                // `dy`, and saying otherwise would send an agent scrolling forever.
                None if bbox.x < v.right() && v.x < bbox.right() => {
                    return Landing::OffScreen { dy: bbox.y - v.y }
                }
                None => return Landing::Unreachable,
            },
            None => bbox,
        };
        if aim.width <= 0.0 || aim.height <= 0.0 {
            return Landing::Unreachable;
        }

        // A hit on the target itself or anywhere in its subtree activates the target. Collected
        // ONCE — this runs per listed node in `to_viewport_lines`, so a per-candidate walk of the
        // subtree would make an observation quadratic in the page.
        let mut subtree = Vec::new();
        collect_ids(node, &mut subtree);
        let try_point = |p: (f32, f32)| {
            self.hit_test(p.0, p.1)
                .is_some_and(|h| subtree.contains(&h.node))
        };

        let centre = aim.center();
        if try_point(centre) {
            return Landing::Clear { point: centre };
        }
        // `hit_test` is half-open on the right/bottom edges, so a candidate must stay strictly
        // inside; the insets are clamped for boxes only a pixel or two big.
        let (ix, iy) = ((aim.width / 4.0).min(4.0), (aim.height / 4.0).min(4.0));
        let (r, b) = (aim.right() - 0.5, aim.bottom() - 0.5);
        let quad = |fx: f32, fy: f32| {
            (
                (aim.x + aim.width * fx).min(r),
                (aim.y + aim.height * fy).min(b),
            )
        };
        for p in [
            quad(0.25, 0.25),
            quad(0.75, 0.25),
            quad(0.25, 0.75),
            quad(0.75, 0.75),
            (aim.x + ix, aim.y + iy),
            ((r - ix).max(aim.x), aim.y + iy),
            (aim.x + ix, (b - iy).max(aim.y)),
            ((r - ix).max(aim.x), (b - iy).max(aim.y)),
        ] {
            if try_point(p) {
                return Landing::Clear { point: p };
            }
        }
        match self.hit_test(centre.0, centre.1) {
            Some(h) => Landing::Obstructed {
                by: h.node,
                point: centre,
            },
            // Nothing at all is at the centre: the target's own box does not hit-test, which is
            // `pointer-events: none` on the target itself. Not obstructed — unaimable.
            None => Landing::Unreachable,
        }
    }
}

/// The node with this id, without allocating a flat view of the whole tree first.
fn find_node(n: &A11yNode, id: NodeId) -> Option<&A11yNode> {
    if n.node == id {
        return Some(n);
    }
    n.children.iter().find_map(|c| find_node(c, id))
}

/// Every arena id in `n`'s subtree, `n` included.
fn collect_ids(n: &A11yNode, out: &mut Vec<NodeId>) {
    out.push(n.node);
    for c in &n.children {
        collect_ids(c, out);
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

/// Whether `node` has a **sectioning-content** ancestor — the test that decides whether a
/// `<header>`/`<footer>` is the PAGE's banner/contentinfo landmark or a section's own header.
///
/// The scoping ancestors are HTML's sectioning content plus `<main>`: anything else (a `<div>`
/// wrapper, the body) leaves the landmark scoped to the document, which is the whole point of a
/// landmark.
fn in_sectioning_content(dom: &Dom, node: NodeId) -> bool {
    scoped_by(dom, node, &["article", "aside", "main", "nav", "section"])
}

/// Whether any ancestor of `node` is one of `tags`.
///
/// ⚠ **THE TWO CALLERS PASS DIFFERENT LISTS AND THAT IS NOT AN OVERSIGHT.** `<header>`/`<footer>`
/// are scoped by `<main>` as well as by sectioning content; an `<aside>` inside `<main>` is still
/// the page's complementary content and stays a landmark. HTML-AAM says so element by element, and
/// a single shared list would have silently broken one of them.
fn scoped_by(dom: &Dom, node: NodeId, tags: &[&str]) -> bool {
    let mut cur = dom.parent(node);
    while let Some(n) = cur {
        if let Some(el) = dom.element(n) {
            if tags.contains(&el.name.as_str()) {
                return true;
            }
        }
        cur = dom.parent(n);
    }
    false
}

/// Whether the element can take keyboard focus — one of the two triggers that make an authored
/// `role="none"` be IGNORED.
fn is_focusable(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if el.attr("tabindex").is_some() {
        return true;
    }
    match el.name.as_str() {
        "a" | "area" => el.attr("href").is_some(),
        "button" | "select" | "textarea" | "summary" | "iframe" => true,
        "input" => !el
            .attr("type")
            .is_some_and(|t| t.eq_ignore_ascii_case("hidden")),
        _ => el.attr("contenteditable").is_some(),
    }
}

/// The ARIA **global** states and properties — the ones valid on every role, and therefore the ones
/// whose presence proves the author meant the element to be in the tree. `aria-hidden` is
/// deliberately absent: it REMOVES the node, so it cannot also be evidence for keeping it.
const GLOBAL_ARIA: [&str; 21] = [
    "aria-atomic",
    "aria-busy",
    "aria-controls",
    "aria-current",
    "aria-describedby",
    "aria-description",
    "aria-details",
    "aria-disabled",
    "aria-dropeffect",
    "aria-errormessage",
    "aria-flowto",
    "aria-grabbed",
    "aria-haspopup",
    "aria-invalid",
    "aria-keyshortcuts",
    "aria-label",
    "aria-labelledby",
    "aria-live",
    "aria-owns",
    "aria-relevant",
    "aria-roledescription",
];

fn has_global_aria_attribute(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    GLOBAL_ARIA
        .iter()
        .any(|a| el.attr(a).is_some_and(|v| !v.trim().is_empty()))
}

/// ⚠⚠⚠ **ARIA PRESENTATIONAL-ROLE CONFLICT RESOLUTION.** `role="none"` is the author saying *"this
/// element is scaffolding, do not announce it"* — and the spec makes that request **inoperative**
/// when the element is focusable or carries a global ARIA attribute. The reason is not pedantry: a
/// user can still TAB to a focusable element, and a node the user reaches but that announces
/// nothing is worse than one with a wrong name. The commonest real instance is a layout `<table
/// role="none">` that somebody made focusable.
fn presentational_role_is_ignored(dom: &Dom, node: NodeId) -> bool {
    is_focusable(dom, node) || has_global_aria_attribute(dom, node)
}

/// Whether this element's own `role=` resolves to `none`/`presentation` **and is honoured**.
fn explicit_presentational(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    let Some(role) = el.attr("role") else {
        return false;
    };
    for tok in role.split_ascii_whitespace() {
        let t = tok.trim().to_ascii_lowercase();
        if t == "none" || t == "presentation" {
            return !presentational_role_is_ignored(dom, node);
        }
        if Role::from_aria_token(&t).is_some() {
            return false; // an earlier VALID token wins
        }
    }
    false
}

/// Whether the element is named by an ATTRIBUTE rather than by its content — **and the reference
/// actually resolves.**
///
/// ⚠ `aria-labelledby="typo"` is not a name. The old check asked only whether the attribute was
/// present and non-blank, so a dangling reference made a `<section>` a `region` landmark with no
/// name at all — an entry in the screen reader's landmark list that announces nothing.
fn has_attribute_name(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if el
        .attr("aria-label")
        .is_some_and(|v| !normalize(v).is_empty())
    {
        return true;
    }
    if let Some(refs) = el.attr("aria-labelledby") {
        let text = refs
            .split_ascii_whitespace()
            .filter_map(|id| dom.get_element_by_id(dom.root(), id))
            .map(|n| normalize(&dom.text_content(n)))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() {
            return true;
        }
    }
    el.attr("title").is_some_and(|v| !normalize(v).is_empty())
}

/// ⭐ **AN `<img>`'S ROLE IS A THREE-WAY CONDITION, NOT A TAG LOOKUP.**
///
/// ```text
///   alt="A cat"                          -> image      (named by its alt)
///   alt="" (or whitespace)               -> NO NODE    the author said "decorative"
///   alt="" + aria-label / title          -> image      an ARIA name OVERRIDES the empty alt
///   no alt, has src/srcset               -> image      a broken image is still an image
///   no alt, no src, no srcset, no name   -> NO NODE    there is nothing here to announce
/// ```
///
/// The last row is the one that is easy to get wrong and it is the largest single block of
/// `html-aam` failures: an `<img>` with no source and no name is not "an image that failed to
/// load", it is nothing at all, and announcing it puts a phantom in the tree on every page that
/// ships an empty placeholder.
fn img_role(dom: &Dom, node: NodeId) -> Option<Role> {
    let el = dom.element(node)?;
    let alt = el.attr("alt");
    if alt.is_some_and(|a| !normalize(a).is_empty()) {
        return Some(Role::Image);
    }
    if has_attribute_name(dom, node) {
        return Some(Role::Image);
    }
    if alt.is_some() {
        return None; // an explicit (possibly whitespace-only) empty alt IS "decorative"
    }
    let sourced = el.attr("src").is_some_and(|v| !v.trim().is_empty())
        || el.attr("srcset").is_some_and(|v| !v.trim().is_empty());
    sourced.then_some(Role::Image)
}

/// Whether the nearest enclosing `<table>` is presentational, which makes this row/cell one too.
fn scoped_by_presentational_table(dom: &Dom, node: NodeId) -> bool {
    let mut cur = dom.parent(node);
    while let Some(n) = cur {
        if let Some(el) = dom.element(n) {
            if el.name == "table" {
                return explicit_presentational(dom, n);
            }
        }
        cur = dom.parent(n);
    }
    false
}

/// The `display`/`visibility` an element declares in its own **inline `style` attribute**.
///
/// ⚠⚠ **A BOUNDED, NAMED APPROXIMATION.** The real answer is the computed style, and
/// `accessible_name` is not given one — the WPT entry point (`__axRoleName`) holds a DOM and
/// nothing else, and the tree builder's `invisible` set carries only `visibility`. What IS
/// reachable from the DOM is the inline declaration, which is how hidden name-fragments are
/// authored in practice (a `<span style="display:none">` holding text meant for a `labelledby`)
/// and how every WPT fixture writes them. **A `display:none` applied by a CLASS is still missed** —
/// stated here rather than discovered later, and the fix is to thread the computed set in the way
/// t1097 threaded `GeneratedText`.
fn inline_visibility(dom: &Dom, node: NodeId) -> (bool, Option<bool>) {
    let Some(el) = dom.element(node) else {
        return (false, None);
    };
    let Some(style) = el.attr("style") else {
        return (false, None);
    };
    let s = style.to_ascii_lowercase();
    let mut displayed_none = false;
    let mut vis = None;
    for decl in s.split(';') {
        let Some((k, v)) = decl.split_once(':') else {
            continue;
        };
        match (k.trim(), v.trim()) {
            ("display", "none") => displayed_none = true,
            ("visibility", "hidden" | "collapse") => vis = Some(false),
            ("visibility", "visible") => vis = Some(true),
            _ => {}
        }
    }
    (displayed_none, vis)
}

/// **The `display` / `visibility` the name walk should obey — the COMPUTED pair when the caller has
/// a style map, and [`inline_visibility`]'s DOM-only approximation when it does not.**
///
/// ⚠⚠⚠ **THE MAP WAS ALREADY IN THE CONTEXT AND THIS WALK NEVER ASKED IT.** t1365 threaded
/// [`NameStyles`] in so that a non-inline child could contribute a separator and `text-transform`
/// could reach the name. `display: none` was in that same map the whole time, and the prune below it
/// went on reading the element's inline `style=` attribute — so a fragment hidden by a STYLESHEET
/// RULE was announced. Chrome-measured (CDP `Accessibility.getFullAXTree`, `<button>Save <span
/// class=h>SECRET</span></button>`):
///
/// ```text
///                                                  chrome        before        after
///   .h { display: none }        (stylesheet)       "Save"        "Save SECRET" "Save"
///   style="display:none"        (inline)  CONTROL  "Save"        "Save"        "Save"
///   .h { visibility: hidden }   (stylesheet)       "Save"        "Save SECRET" "Save"
///   style="visibility:hidden"   (inline)  CONTROL  "Save"        "Save"        "Save"
/// ```
///
/// ⚠ **AND NO CONFORMANCE TEST IN THE TREE COULD HAVE FOUND IT**: every hidden-node fixture in
/// WPT's `accname/name/comp_labelledby_hidden_nodes.html` writes `style="display: none"` inline, and
/// so did this engine's own gate's control row. A rule with two sources, where the weaker source is
/// the one every test uses, is invisible to the whole suite.
///
/// ⭐ Returning `Some(_)` for the visibility of EVERY node in the map is not a detail: `visibility`
/// is inherited and *undoable*, so the computed value already carries the ancestor's state and a
/// `visibility: visible` child under a hidden parent resolves to visible on its own. The inline
/// reader has to return `None` for "not declared here" and let the flag flow down.
fn node_visibility(dom: &Dom, n: NodeId, styles: &NameStyles) -> (bool, Option<bool>) {
    match styles.get(&n) {
        Some(s) => (
            s.display == manuk_css::Display::None,
            Some(s.visibility == manuk_css::Visibility::Visible),
        ),
        None => inline_visibility(dom, n),
    }
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
/// **Is a `[popover]` element actually being RENDERED?** — the question HTML-AAM's popover mapping
/// turns on, answered from the DOM alone.
///
/// Two sources, and the first is the authoritative one for the ordinary path: `showPopover()` writes
/// `data-manuk-popover-open`, and the UA sheet keys the popover's `display` off exactly that
/// attribute — so the engine's own open-state marker IS the rendered-state answer. The second is an
/// author overriding the UA rule with a `display` of their own, which wins over it.
///
/// ⚠⚠ **AND THE LIMIT IS THE t1365 CLASS, STATED RATHER THAN DISCOVERED LATER: the inline arm reads
/// the `style=` ATTRIBUTE, so a popover forced visible by a STYLESHEET RULE is missed.** `role_of`
/// takes a `&Dom` and no style map — a signature with a dozen callers — while the name walk one
/// screen up takes `NameStyles` and reads the COMPUTED pair. That is the same rule with two sources
/// where the weaker source is the one the conformance suite happens to use: WPT's own
/// `popover-minimum-role.html` writes `popover.style = 'display:block'`, which is inline, so the
/// suite cannot see this gap either. Recorded here so the next reader does not have to rediscover it.
fn popover_is_rendered(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    let (inline_none, inline_vis) = inline_visibility(dom, node);
    // `visibility: hidden` removes it from the tree whatever the display says — Chrome-measured:
    // `style="display:block; visibility:hidden"` on an open popover is IGNORED, role `none`.
    if inline_vis == Some(false) {
        return false;
    }
    if el.attr("data-manuk-popover-open").is_some() {
        return true;
    }
    // An inline `display` that is not `none` beats the UA sheet's `[popover] { display: none }`.
    !inline_none
        && el
            .attr("style")
            .is_some_and(|st| st.to_ascii_lowercase().contains("display"))
}

pub fn role_of(dom: &Dom, node: NodeId) -> Option<Role> {
    let el = dom.element(node)?;

    if let Some(explicit) = el.attr("role") {
        // ARIA: the first *valid* token wins; invalid tokens fall through to implicit.
        // ⚠⚠⚠ **`Role::parse`, NOT `Role::from_aria_token` — THE CASE FOLD ALREADY EXISTED AND
        // THIS ENTRANCE DID NOT USE IT** (t1350). `parse` is
        // `from_aria_token(&tok.trim().to_ascii_lowercase())` and is what the AGENT calls; the
        // `role="…"` attribute — the entrance the actual WEB uses — called the raw matcher, so
        // `role="BUTTON"` matched nothing. The fallback-token form `role="foo Link"` — an unknown
        // token then a real one — is the documented way authors ship forward-compatible roles, and
        // it is why this is a LOOP over the tokens rather than a single lookup.
        for tok in explicit.split_ascii_whitespace() {
            let t = tok.trim().to_ascii_lowercase();
            if t == "none" || t == "presentation" {
                // Conflict resolution: an inoperative `role="none"` exposes the IMPLICIT role.
                if presentational_role_is_ignored(dom, node) {
                    break;
                }
                return Some(Role::Generic);
            }
            let Some(r) = Role::from_aria_token(&t) else {
                continue; // an unknown token is skipped, not fatal — that is the fallback form
            };
            // ⚠⚠⚠ **`region` AND `form` ARE LANDMARKS ONLY WHEN NAMED, AND AN UNNAMED ONE FALLS
            // THROUGH TO THE NEXT TOKEN.** A landmark's entire purpose is to be an entry in a jump
            // list; an unnamed one is a row that says nothing, so ARIA makes the role inoperative
            // rather than let it dilute the list. `role="region group"` on an unnamed element is a
            // `group` — which is exactly why authors write the pair.
            if matches!(r, Role::Region | Role::Form) && !has_attribute_name(dom, node) {
                continue;
            }
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
            // HTML-AAM: `<input type=range>` is a `slider`, `type=number` a `spinbutton`,
            // `type=search` a `searchbox` (which is what tells an agent it is THE search field).
            "range" => Role::Slider,
            "number" => Role::SpinButton,
            "search" => Role::SearchBox,
            // ── ⚠⚠⚠ **`<input type=file>` IS A BUTTON, AND CALLING IT A TEXT FIELD IS THE WORST
            //    AVAILABLE WRONG ANSWER.** HTML-AAM maps it to `button`, Chrome agrees
            //    (`role='button'`, name `"Choose File"`), and the difference is not cosmetic: the
            //    role is how the agent ADDRESSES a control. As a `textbox` an upload control is
            //    invisible to *"click Choose File"* and — much worse — `type_into` ACCEPTS it and
            //    silently does nothing, because a file input has no text to type into. **A wrong
            //    role that an actuator will act on is a lie the actuator cannot detect.**
            "file" => Role::Button,
            // ⚠ `<input type=color>` has **no corresponding ARIA role** in HTML-AAM; Chrome exposes
            //    an internal `ColorWell`. `generic` is the honest ARIA answer and it keeps the node
            //    in the tree and addressable by name — where `textbox` invited exactly the
            //    `type_into` the file row above describes. Naming it `ColorWell` would adopt a
            //    Chrome internal into a vocabulary that is otherwise ARIA's.
            "color" => Role::Generic,
            // ⚠ `hidden` is filtered by `is_hidden` before we get here.
            //
            // ⚠⚠ **`date` / `time` / `datetime-local` / `month` / `week` DELIBERATELY KEEP
            //    `textbox`.** Chrome gives each an internal role (`Date`, `InputTime`, `DateTime`)
            //    with no ARIA equivalent, and unlike a colour well these controls really do accept
            //    typed text — so `textbox` is both the useful answer and a non-harmful one. Measured
            //    and left, rather than folded into the `color` arm because the enum has a slot.
            _ => Role::TextBox,
        },
        "textarea" => Role::TextBox,
        // ── ⚠⚠⚠ **A MULTI-SELECT IS A `listbox`, NOT A `combobox`.** HTML-AAM: a `<select>` with
        //    `multiple`, or with `size` greater than 1, is a `listbox`; only the collapsed
        //    single-line form is a `combobox`. Chrome-measured, all three spellings. The two are
        //    different widgets to an agent — a combobox is opened and one option chosen, a listbox
        //    is a visible list with a selection that may be plural — so *"select all three regions"*
        //    against a `combobox` is a plan that cannot be executed.
        "select" => {
            let multiple = el.attr("multiple").is_some();
            let sized = el
                .attr("size")
                .and_then(|v| v.trim().parse::<i64>().ok())
                .is_some_and(|n| n > 1);
            if multiple || sized {
                Role::ListBox
            } else {
                Role::ComboBox
            }
        }
        // HTML-AAM implicit roles for the native widgets.
        "dialog" => Role::Dialog,
        "progress" => Role::ProgressBar,
        "option" => Role::Option,
        // ── ⚠⚠ **FOUR ELEMENTS HTML-AAM MAPS TO `group`, ALL OF THEM READING `generic`.** A
        //    `generic` node is a box with no meaning; a `group` is a named section of a form or a
        //    document, and it is what an agent walks to find *"the Billing address fields"*.
        //    `<fieldset>` is the one with corpus weight — every multi-section form on the web is
        //    built out of it, and its name already came from `<legend>` correctly, so only the role
        //    was wrong. Chrome-measured: all four are `group`.
        "fieldset" | "details" | "address" | "hgroup" => Role::Group,
        // A `<menu>` is a list per HTML-AAM (the `type=context` menu role never shipped).
        "menu" => Role::List,
        "h1" => Role::Heading { level: 1 },
        "h2" => Role::Heading { level: 2 },
        "h3" => Role::Heading { level: 3 },
        "h4" => Role::Heading { level: 4 },
        "h5" => Role::Heading { level: 5 },
        "h6" => Role::Heading { level: 6 },
        "img" => return img_role(dom, node),
        // N4 — a `<slot>` is a rendering hole, not a semantic node: its assigned nodes
        // take its place in the flat tree, so it exposes no a11y node of its own.
        "slot" => return None,
        // ── THE TEXT-LEVEL ELEMENTS, which are ordinary HTML and were all `generic`.
        "blockquote" => Role::Blockquote,
        "caption" => Role::Caption,
        "code" => Role::Code,
        "em" => Role::Emphasis,
        "strong" => Role::Strong,
        "sub" => Role::Subscript,
        "sup" => Role::Superscript,
        "mark" => Role::Mark,
        "time" => Role::Time,
        "figure" => Role::Figure,
        "meter" => Role::Meter,
        // HTML-AAM: `<ins>` is an insertion; BOTH `<del>` and `<s>` are deletions — `<s>` is
        // "no longer accurate", which is the same announcement.
        "ins" => Role::Insertion,
        "del" | "s" => Role::Deletion,
        // A definition list's parts: the term and what it means.
        "dfn" | "dt" => Role::Term,
        "dd" => Role::Definition,
        // `<output>` is a live region — the whole point of the element is that it is announced
        // when it changes.
        "output" => Role::Status,
        "optgroup" => Role::Group,
        // `<dir>` is obsolete and still renders as a list, so it still IS one.
        "dir" => Role::List,
        "thead" | "tbody" | "tfoot" => Role::RowGroup,
        "ul" | "ol" => Role::List,
        // ⚠ **AN ORPHANED `<li>` IS NOT A `listitem`.** `listitem` is defined by its owning list;
        // an `<li>` with no list parent is a stray element, and announcing "list item, 1 of 1"
        // about it is a fact the page does not contain. And a list whose own role is
        // `presentation` makes its REQUIRED OWNED elements presentational too — that inheritance
        // is the whole reason `role="none"` on a layout `<ul>` works at all.
        "li" => match dom
            .parent(node)
            .and_then(|p| dom.element(p).map(|e| (p, e)))
        {
            Some((p, e)) if matches!(e.name.as_str(), "ul" | "ol" | "menu") => {
                if explicit_presentational(dom, p) {
                    return None;
                }
                Role::ListItem
            }
            _ => Role::Generic,
        },
        "table" => Role::Table,
        // Same required-owned inheritance as `<li>`: a layout `<table role="none">` must not leave
        // its rows and cells behind as a skeleton of `row`/`cell` nodes.
        "tr" | "td" if scoped_by_presentational_table(dom, node) => return None,
        "th" if scoped_by_presentational_table(dom, node) => return None,
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
        // ── `<search>` IS A LANDMARK (Baseline Apr 2026, HTML-AAM `role=search`).
        //
        // `Role::Search` already existed for the explicit `role="search"` attribute; the ELEMENT was
        // missing from this map and fell through to `Role::Generic`, so a page using the modern wrapper
        // lost the landmark entirely. That is not only a screen-reader gap: per CONSTITUTION VI.1 this
        // tree already feeds `manuk-agent`'s observation channel, so an unmapped landmark is an
        // **agentic** gap — the agent cannot find "the search box" by role on any site that adopted the
        // element. Found by surface audit #29 (t558), which read what actually SHIPPED rather than what
        // the vendors prioritised.
        "search" => Role::Search,
        "main" => Role::Main,
        // ⚠⚠⚠ **A `<footer>` INSIDE AN `<article>` IS NOT THE PAGE'S FOOTER.** `banner` and
        // `contentinfo` are LANDMARKS — a screen reader offers a jump list of them and an agent
        // reads them as "the page's chrome". A blog index with thirty articles was publishing
        // thirty `contentinfo` landmarks, which is worse than publishing none: the one real page
        // footer is no longer findable. ARIA 1.3 (w3c/aria#1931) names the scoped ones
        // `sectionheader`/`sectionfooter`; the landmark role survives only at the top level.
        "header" | "footer" => {
            let landmark = !in_sectioning_content(dom, node);
            match (el.name.as_str(), landmark) {
                ("header", true) => Role::Banner,
                ("header", false) => Role::SectionHeader,
                (_, true) => Role::ContentInfo,
                (_, false) => Role::SectionFooter,
            }
        }
        // ── ⚠⚠⚠ **`<form>` IS A LANDMARK ONLY WHEN NAMED, AND THE RULE WAS WRITTEN DOWN NEXT
        //    DOOR.** The `<section>` arm below carries the identical clause, and `role_of`'s
        //    EXPLICIT-role path six hundred lines up carries it for exactly these two roles:
        //    `matches!(r, Role::Region | Role::Form) && !has_attribute_name(…)`. So `role="form"`
        //    was guarded and `<form>` was not — **the same rule, guarded at one entrance of one
        //    function and unguarded at the other.**
        //
        //    Chrome-measured: `<form>` plain is `generic`; with `aria-label`, `title` or a
        //    resolving `aria-labelledby` it is `form`. ⚠ A `name="…"` attribute does NOT count
        //    (Chrome: `generic`) — it is the form's SUBMISSION name, not an accessible one, and it
        //    is the row that stops "has any nameish attribute" from being the rule.
        //
        //    ⭐ Why it matters to an agent rather than to a spec: a landmark list is a JUMP LIST.
        //    Every `<form>` on a page — the newsletter box, the search field, the login — would
        //    appear in it, and *"go to the form"* becomes ambiguous exactly when there is more than
        //    one, which is the case the list exists for.
        "form" => {
            if has_attribute_name(dom, node) {
                Role::Form
            } else {
                Role::Generic
            }
        }
        "article" => Role::Article,
        // HTML-AAM: `<section>` is only a `region` when it has an accessible name — and
        // `has_attribute_name` now RESOLVES `aria-labelledby` rather than trusting its presence.
        "section" => {
            if has_attribute_name(dom, node) {
                Role::Region
            } else {
                Role::Generic
            }
        }
        // ⚠ An `<aside>` nested in sectioning content is that SECTION's aside, not the page's, so
        // it is a landmark only when it carries a name to distinguish it. ⚠⚠ `<main>` is NOT in
        // this list though it IS in `<header>`/`<footer>`'s — an aside directly inside `<main>` is
        // still the page's complementary content. HTML-AAM says so element by element.
        "aside" => {
            if scoped_by(dom, node, &["article", "aside", "nav", "section"])
                && !has_attribute_name(dom, node)
            {
                Role::Generic
            } else {
                Role::Complementary
            }
        }
        "p" => Role::Paragraph,
        "hr" => Role::Separator,
        "html" => Role::Document,
        // ── ⭐⭐⭐ **THE `[popover]` MINIMUM ROLE — AND THIS ARM IS THE RULE'S OWN DEFINITION.**
        //
        // HTML-AAM raises a VISIBLE `[popover]` to `group`, but only when the element has **no role
        // mapping of its own** — and "no mapping" is precisely the set of elements that fall through
        // to this default arm. Putting the rule anywhere else would need a list; putting it HERE
        // makes the arm the list.
        //
        // ⭐⭐ It takes a PAIR to see that, and the pair is the whole finding. Chrome-measured, both
        // forced visible:
        //
        // ```text
        //   <div popover>      -> group        <span popover>    -> group
        //   <section popover>  -> generic      …and named        -> region
        //   <button popover>   -> button       <nav popover>     -> navigation
        // ```
        //
        // **`<div>` and an unnamed `<section>` BOTH compute to `generic` without the attribute**, so
        // a rule written against the computed role would have raised both and been wrong about one.
        // `<section>` HAS a mapping (region-when-named, generic otherwise); `<div>` and `<span>` have
        // none. The distinction is *does HTML-AAM map this tag*, not *what does it come out as*.
        //
        // ⚠ An explicit `role=` already returned above, so `role="none"` and `role="alert"` both win
        // over this — Chrome agrees on both.
        _ if el.attr("popover").is_some() && popover_is_rendered(dom, node) => Role::Group,
        _ => Role::Generic,
    })
}

/// Build an `id` → node index once, so `aria-labelledby` / `<label for>` are O(1).
///
/// ⚠ **DOCUMENT ORDER, and first-wins.** Both consumers are defined against *"the FIRST element in
/// tree order whose ID is X"*. The walk this replaces pushed children and popped LIFO, so it
/// visited the LAST matching element first and `or_insert` kept **that** one — the wrong node, on
/// any document that repeats an id. It was invisible because a document with no duplicate ids
/// cannot tell the two orders apart, and duplicate ids are exactly what this rule exists for.
fn id_index(dom: &Dom) -> HashMap<String, NodeId> {
    let mut map = HashMap::new();
    for n in document_order(dom, dom.root()) {
        if let Some(el) = dom.element(n) {
            if let Some(id) = el.id() {
                map.entry(id.to_string()).or_insert(n);
            }
        }
    }
    map
}

/// Trim and collapse the whitespace in an accessible name — **ASCII whitespace, and no other kind.**
///
/// ⚠⚠⚠ **`split_whitespace` SPLITS ON UNICODE WHITESPACE, AND U+00A0 IS UNICODE WHITESPACE.** accname
/// §4 (and HTML's own definition) collapse *ASCII* whitespace — space, tab, LF, FF, CR — and a
/// NO-BREAK SPACE is none of those: it is a character the author deliberately chose so the text would
/// NOT break there, and it must survive into the name. Rust's `split_whitespace` uses
/// `char::is_whitespace` (Unicode `White_Space`), which contains U+00A0, so every non-breaking space
/// in a name was silently rewritten to a plain space.
///
/// Measured against the accname suite — the expectation is the AUTHOR's string, byte for byte:
///
/// ```text
///   <button>button\u{a0}label</button>   expected "button\u{a0}label"   got "button label"
///   <div role=heading>…                  same
///   <span role=button>… mixed / leading / trailing nbsp   same
/// ```
///
/// It matters past conformance: a screen reader announces the name it is given, and **the agentic
/// surface matches on it** — an agent told to click "Sign\u{a0}up" and an engine that stored
/// "Sign up" do not find the same element. NBSP is common in exactly the short UI strings agents
/// target (prices, "Sign up", "Add to cart", French punctuation).
///
/// `split_ascii_whitespace` is the same function over the right alphabet.
fn normalize(s: &str) -> String {
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
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
/// ⚠⚠⚠ **THE RENDERED TEXT OF EACH `::before` / `::after`, KEYED BY ITS OWNER (t1097).**
///
/// Generated content is **not in the DOM by construction** — script must never see it — so this
/// tree, which is built from the DOM, could not reach it by any path and every pseudo was silently
/// missing from the accessible name. accname §4.3 step 2F requires it. Produced by
/// `manuk_layout::generated_text`, which resolves counters through the **same walk layout paints
/// with**, so the announced number and the painted number cannot drift apart.
pub type GeneratedText = HashMap<NodeId, (String, String)>;

/// **The computed-style facts the NAME walk needs, per node** — the same shape as
/// [`GeneratedText`] and threaded the same way, for the same reason.
///
/// ⭐⭐⭐ **THE ACCESSIBLE NAME IS A FUNCTION OF THE COMPUTED STYLE, NOT OF THE MARKUP**, and the
/// accname spec says so in two places the DOM alone cannot answer:
///
/// * **§ "Computed Name from Content" appends a space around a child that is not inline.**
///   `<button><span>one</span><span>two</span></button>` is `"onetwothree"` when the spans are
///   inline and `"one two three"` the moment CSS makes them `display:block` — same markup, and the
///   only difference is a stylesheet. Chrome-verified against `accname/name/comp_name_from_content`,
///   which asserts both spellings side by side.
/// * **`text-transform` applies**, because the name is the text a user is *read*: a heading styled
///   `text-transform: uppercase` is named `"CALL US"`, not `"Call us"`.
///
/// Built ONCE by [`name_styles`] and passed in, rather than looked up per node, because this walk
/// has **two entrances** — the tree builder and the bare `accessible_name` behind
/// `test_driver.get_computed_label()` — and a fact wired to one of them is the shape this file has
/// been caught by three times (t1097, t1350, t1355).
pub type NameStyles = HashMap<NodeId, NameStyle>;

/// The three computed facts the name walk reads off a node.
///
/// ⭐ **A THIRD FACT MADE THIS A STRUCT, WHICH IS THE SAME CALL t1365 MADE ONE LEVEL UP.** It was a
/// `(Display, TextTransform)` tuple; t1379 added `visibility`, and a three-element positional tuple
/// destructured at five sites is exactly how the fourth reader gets the second field and nobody
/// notices. Named fields also make the *absence* of a field legible: there is no `hidden` here,
/// because "hidden" is not a style — it is a conclusion drawn from `display` and `visibility` plus
/// the DOM's own `hidden` / `aria-hidden`, and that conclusion lives in [`node_visibility`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameStyle {
    pub display: manuk_css::Display,
    pub text_transform: manuk_css::TextTransform,
    pub visibility: manuk_css::Visibility,
}

/// The ALT half of `content` per owner — `(::before, ::after)`, each `None` when that pseudo's
/// declaration carried no `/`. See [`NameCtx`].
pub type GeneratedAlt = HashMap<NodeId, (Option<String>, Option<String>)>;

/// **Everything the accessible-name walk needs that is not in the DOM.**
///
/// ⭐⭐⭐ **THIS IS A CONTEXT STRUCT BECAUSE THE FOURTH FACT ARRIVED, AND t1365 SAID IT WOULD.**
/// Three facts had been threaded through this walk one parameter at a time — t1097's
/// [`GeneratedText`], t1355's `NameIndex` widening, t1365's [`NameStyles`] — and **each one left a
/// caller behind**, twice in the same unit test, invisibly, because `manuk-a11y` is a suite in no
/// wall (surface audit #78). t1365's own note read: *"a fourth fact should become a context struct
/// rather than a fourth parameter — the signature already carries an
/// `#[allow(clippy::too_many_arguments)]`."* This is that fourth fact.
///
/// The win is not tidiness: **adding a fifth is now a one-line change to this struct and its two
/// construction sites**, instead of an edit to eleven signatures and twenty call sites where
/// missing one compiles fine on every path but the one that matters.
#[derive(Clone, Copy)]
pub struct NameCtx<'a> {
    /// Rendered `::before`/`::after` text (t1097).
    pub generated: &'a GeneratedText,
    /// The ALT half of `content`, which is announced instead of the rendered text (t1369/t1371).
    pub alt: &'a GeneratedAlt,
    /// `display` and `text-transform` per node (t1365).
    pub styles: &'a NameStyles,
}

impl NameCtx<'_> {
    /// The name a pseudo contributes: its ALT text when the declaration had a `/`, else what it
    /// renders.
    ///
    /// ⚠ `Some("")` is a real answer — `content: "★" / ""` means *announce nothing* — so this is a
    /// three-way choice and not `unwrap_or(rendered)` on a string.
    fn pseudo_names(&self, n: NodeId) -> (String, String) {
        let (rb, ra) = self
            .generated
            .get(&n)
            .map(|(b, a)| (b.as_str(), a.as_str()))
            .unwrap_or(("", ""));
        let (ab, aa) = self
            .alt
            .get(&n)
            .map(|(b, a)| (b.as_deref(), a.as_deref()))
            .unwrap_or((None, None));
        (ab.unwrap_or(rb).to_string(), aa.unwrap_or(ra).to_string())
    }
}

/// An empty context — for callers with no style map and no generated content (the bare
/// [`accessible_name`], and unit tests whose subject is neither).
pub fn empty_name_ctx<'a>(
    generated: &'a GeneratedText,
    alt: &'a GeneratedAlt,
    styles: &'a NameStyles,
) -> NameCtx<'a> {
    NameCtx {
        generated,
        alt,
        styles,
    }
}

/// Extract [`NameStyles`] from a computed-style map. One builder, so the two entrances cannot drift.
pub fn name_styles(dom: &Dom, styles: &HashMap<NodeId, manuk_css::ComputedStyle>) -> NameStyles {
    dom.descendants(dom.root())
        .filter_map(|n| {
            styles.get(&n).map(|s| {
                (
                    n,
                    NameStyle {
                        display: s.display,
                        text_transform: s.text_transform,
                        visibility: s.visibility,
                    },
                )
            })
        })
        .collect()
}

/// Does the node at `n` contribute a SPACE on each side of its text, per [`NameStyles`]?
///
/// Public so a caller that cannot name `manuk_css`'s types can still ask — which is what a gate in
/// a crate that does not depend on `manuk-css` needs in order to assert its fixture is not vacuous.
pub fn name_separates(styles: &NameStyles, n: NodeId) -> bool {
    styles.get(&n).is_some_and(|s| separates_name(s.display))
}

/// Does the node at `n` compute to `display: none`, per [`NameStyles`]? And does it compute to a
/// hidden `visibility`?
///
/// Public for the same reason [`name_separates`] is: a gate in a crate that cannot name
/// `manuk_css`'s types still has to be able to assert that its **stylesheet reached the cascade**,
/// or a hidden-fragment row passes against an engine that never applied the rule at all.
pub fn name_display_none(styles: &NameStyles, n: NodeId) -> bool {
    styles
        .get(&n)
        .is_some_and(|s| s.display == manuk_css::Display::None)
}

/// See [`name_display_none`] — the `visibility` half.
pub fn name_visibility_hidden(styles: &NameStyles, n: NodeId) -> bool {
    styles
        .get(&n)
        .is_some_and(|s| s.visibility != manuk_css::Visibility::Visible)
}

/// Does a child with this `display` contribute a SPACE on each side of its text?
///
/// ⚠ **`inline-block` DOES**, and that row is why this is not `display != Inline`-by-eye: the WPT
/// fixture asserts `"one two three"` for `display:inline-block` spans as well as for `display:block`
/// ones. The rule is *"not an inline box"*, and an inline-block is an atomic inline — it is not an
/// inline box, it is a block box that participates in one.
fn separates_name(d: manuk_css::Display) -> bool {
    !matches!(d, manuk_css::Display::Inline | manuk_css::Display::None)
}

/// Apply `text-transform` to a name fragment. `capitalize` upper-cases the first typographic letter
/// of each word and leaves the rest as authored, which is why it is not `to_uppercase` on word[0].
fn transform_name(text: &str, t: manuk_css::TextTransform) -> String {
    match t {
        manuk_css::TextTransform::None => text.to_string(),
        manuk_css::TextTransform::Uppercase => text.to_uppercase(),
        manuk_css::TextTransform::Lowercase => text.to_lowercase(),
        manuk_css::TextTransform::Capitalize => {
            let mut out = String::with_capacity(text.len());
            let mut at_word_start = true;
            for c in text.chars() {
                if c.is_alphanumeric() {
                    if at_word_start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.push(c);
                    }
                    at_word_start = false;
                } else {
                    out.push(c);
                    at_word_start = true;
                }
            }
            out
        }
    }
}

pub fn accessible_name(dom: &Dom, node: NodeId, role: &Role) -> String {
    accessible_name_generated(
        dom,
        node,
        role,
        &empty_name_ctx(
            &GeneratedText::new(),
            &GeneratedAlt::new(),
            &NameStyles::new(),
        ),
    )
}

/// [`accessible_name`] **with the rendered `::before` / `::after` text** — see [`GeneratedText`].
///
/// ⚠⚠⚠ **THIS EXISTS BECAUSE t1097 WAS FIXED AT ONE ENTRANCE AND THIS IS THE OTHER ONE.** t1097
/// threaded generated content into the tree builder and gated it (`g_ax_generated_name`), which is
/// the path a live page's AX tree takes. The path WPT and `test_driver.get_computed_label()` take is
/// the bare [`accessible_name`], and it constructed an **EMPTY** map — so every
/// `button::before{content:"★ "}` was absent from every name the conformance suite could see, on a
/// mechanism the project had already built, gated and journaled.
///
/// The two-entrance shape has now appeared three times in one session (t1350's case fold, t1353's
/// content walk, this): **a fix belongs at the rule, and a rule reached through two doors needs
/// both doors walked, not one door tested.**
pub fn accessible_name_generated(
    dom: &Dom,
    node: NodeId,
    role: &Role,
    ctx: &NameCtx<'_>,
) -> String {
    let index = NameIndex::build(dom);
    accessible_name_with(dom, node, role, &index, ctx)
}

/// accname step 3 — the **host language's own** labelling mechanisms, factored out because
/// `aria-labelledby` needs exactly this when it dereferences to a CONTROL: a referenced
/// `<input type=checkbox>` is named by its `<label>`, and the caller that reads it has no other
/// way to ask.
fn host_language_name(
    dom: &Dom,
    node: NodeId,
    role: &Role,
    index: &NameIndex,
    ctx: &NameCtx<'_>,
) -> String {
    let Some(el) = dom.element(node) else {
        return String::new();
    };
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
            // ⭐ EVERY `<label>` associated with this control, in DOCUMENT order — the `for=`
            // spelling AND the encapsulating one — joined by a space, as HTML-AAM concatenates
            // them. See [`LabelIndex`] for what each of those three words was costing.
            let text = index
                .labels
                .get(&node)
                .map(|ls| {
                    ls.iter()
                        .map(|&l| content_text(dom, l, Some(node), index, ctx))
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            if !text.is_empty() {
                return text;
            }
            if el.name == "input" {
                // `<input type=image>` is a BUTTON whose face is a picture, so HTML-AAM names it by
                // its `alt` — the same attribute `<img>` uses. It was reaching neither arm: the
                // `alt` arm matches on the TAG (`img`/`area`) and this one only looked at `value`.
                if el
                    .attr("type")
                    .is_some_and(|t| t.eq_ignore_ascii_case("image"))
                {
                    if let Some(alt) = el.attr("alt") {
                        let alt = normalize(alt);
                        if !alt.is_empty() {
                            return alt;
                        }
                    }
                }
                // Button-ish inputs are named by their `value`.
                if matches!(role, Role::Button) {
                    if let Some(v) = el.attr("value") {
                        let v = normalize(v);
                        if !v.is_empty() {
                            return v;
                        }
                    }
                    // ── ⚠⚠⚠ **AND WHEN THERE IS NO `value`, THE UA SUPPLIES THE LABEL — WHICH IS
                    //    THE ONE AN AGENT SEES ON THE BUTTON.** `<input type=submit>` renders the
                    //    word *Submit* and `<input type=reset>` renders *Reset*; HTML-AAM names
                    //    them by that default. Without it the commonest submit button on the web —
                    //    the one whose author never wrote a `value` — is a **nameless button**, and
                    //    *"click Submit"* resolves to nothing.
                    //
                    //    ⚠ `type=button` is deliberately NOT here and it is the control row:
                    //    Chrome-measured, a valueless `<input type=button>` has **no name at all**,
                    //    because the UA renders no default label on it. Three button types, two
                    //    defaults — a blanket rule would invent a name for the third.
                    //
                    // ⚠⚠ **AND AN EXPLICIT `value=""` SUPPRESSES THE DEFAULT** — the same rule this
                    //    file already carries for `<img alt="">`: *an explicit empty host-language
                    //    label is an answer, not a missing one.* Chrome-measured:
                    //    `<input type=submit value="">` has **no name**, where a submit input with
                    //    no `value` attribute at all is named `Submit`. The attribute's PRESENCE is
                    //    the discriminator, not its content, which is why this tests `is_none()`.
                    if el.attr("value").is_none() {
                        let ty = el.attr("type").unwrap_or("").to_ascii_lowercase();
                        match ty.as_str() {
                            "submit" => return "Submit".to_string(),
                            "reset" => return "Reset".to_string(),
                            _ => {}
                        }
                    }
                }
            }
        }
        // A `<fieldset>` is named by its `<legend>` and a `<table>` by its `<caption>`. Same
        // host-language labelling clause as `<label>`, pointing one level IN rather than one level
        // OUT — and both are structures a form or a data page is built out of, so an unnamed one
        // is a region the agent cannot say the name of.
        "fieldset" | "table" => {
            let want = if el.name == "fieldset" {
                "legend"
            } else {
                "caption"
            };
            if let Some(c) = dom
                .children(node)
                .find(|&c| dom.element(c).is_some_and(|e| e.name == want))
            {
                let text = normalize(&dom.text_content(c));
                if !text.is_empty() {
                    return text;
                }
            }
            // ⚠ `<table summary="…">` — the pre-ARIA spelling, still in HTML-AAM because a decade
            // of pages use it, and it is a FALLBACK BEHIND the caption. Chrome-measured, all four
            // combinations: `summary` alone names the table `TS`; a `<caption>` beside it wins with
            // `CAP`; `aria-label` beats both. Putting it in its own `"table"` arm — which is what
            // this first was — SHADOWS the caption arm below and makes every captioned table
            // nameless, which is how the existing gate caught it.
            if el.name == "table" {
                if let Some(sm) = el.attr("summary") {
                    let sm = normalize(sm);
                    if !sm.is_empty() {
                        return sm;
                    }
                }
            }
        }
        _ => {}
    }
    String::new()
}

fn accessible_name_with(
    dom: &Dom,
    node: NodeId,
    role: &Role,
    index: &NameIndex,
    ctx: &NameCtx<'_>,
) -> String {
    let Some(el) = dom.element(node) else {
        return String::new();
    };

    // 1 + 2. aria-labelledby (dereferenced one level), then aria-label — the SAME rule a
    // descendant obeys inside `content_walk`, so it is written once.
    if let Some(name) = aria_name_of(dom, node, index, ctx) {
        return name;
    }

    // 3. native host-language labelling — see `host_language_name`.
    let host = host_language_name(dom, node, role, index, ctx);
    if !host.is_empty() {
        return host;
    }

    // 4. name from content (only for roles that allow it)
    if role.name_from_content() {
        // ⚠⚠⚠ accname §4.3 step 2F: the `::before` and `::after` text is PART OF THE CONTENT, in
        // that order around it. `button::before{content:"★ "}` is announced "★ Save"; ours said
        // "Save" until t1098, and where the pseudo carries the ONLY text — an
        // `a::after{content:" (opens in a new tab)"}`, a `counter(sec)` section number, an icon
        // glyph that IS the label — its absence was the whole name.
        // ⭐ THE SAME §4.3 WALK THE LABEL PATH USES. This was `text_content` + the root's own
        // pseudo text, which flattens away an `<img alt>` in the middle of a button and reads
        // straight through a descendant's `aria-label`.
        let text = content_text(dom, node, None, index, ctx);
        if !text.is_empty() {
            return text;
        }
    }

    // 5. title fallback
    //
    // ⚠⚠⚠ **AN EXPLICIT EMPTY HOST-LANGUAGE LABEL IS AN ANSWER, NOT A MISSING ONE.** accname's
    // `title` step is a LAST RESORT, and it is skipped when the host language already supplied a
    // label that came out empty on purpose. `<img alt="" title="x">` is the exact case: `alt=""` is
    // the author saying *"this picture says nothing"*, and a tooltip does not overrule that.
    //
    // ⚠ The two WPT suites look contradictory here and are not — they ask DIFFERENT questions.
    // `html-aam` says the element still has the `image` ROLE (a tooltip keeps it in the tree);
    // `accname` says its NAME is `""`. Conflating role-presence with name-presence is what made
    // one fix break the other, and it cost 6 subtests to find out.
    if matches!(el.name.as_str(), "img" | "area")
        && el.attr("alt").is_some_and(|a| normalize(a).is_empty())
    {
        return String::new();
    }
    if let Some(t) = el.attr("title") {
        let t = normalize(t);
        if !t.is_empty() {
            return t;
        }
    }

    // 6. `placeholder`, and it is LAST — after the tooltip, not before it.
    //
    // ⚠⚠⚠ **THIS WAS INSIDE STEP 3 AND THEREFORE BEAT `title`, WHICH IS THE WRONG WAY ROUND.**
    // HTML-AAM's input chain is `<label>` → `aria-label` → **`title`** → `placeholder`, and
    // Chrome-measured on `<input placeholder="PH" title="TT">` the name is **`TT`**. A placeholder
    // is the hint that disappears the moment the user types; a `title` is the author's stated
    // label. Ordering them the other way announces the transient one.
    //
    // ⭐ And it applies to `<textarea>` as well as `<input>` — the old placement was inside an
    // `el.name == "input"` branch, so `<textarea placeholder="…">` was **nameless**. One rule, two
    // elements, and only one of them had it.
    if matches!(el.name.as_str(), "input" | "textarea") {
        if let Some(p) = el.attr("placeholder") {
            let p = normalize(p);
            if !p.is_empty() {
                return p;
            }
        }
    }

    String::new()
}

/// **THE `<label>`s OF A CONTROL, IN DOCUMENT ORDER — AND THE SPELLING WE HAD WAS THE RARER ONE.**
///
/// HTML gives a form control two ways to acquire a label, and this engine implemented one of them:
///
/// ```html
///   <label for="a">Remember me</label><input id="a" type="checkbox">   <- was found
///   <label><input type="checkbox"> Remember me</label>                 <- WAS NOT FOUND
/// ```
///
/// The second is the **encapsulating** (implicit) form, and it is the one authors reach for: it
/// invents no `id`, and clicking the text toggles the control for free. Measured on WPT's own
/// `accname` corpus — the one all four engines score themselves on in Interop's accessibility
/// investigation — **35 subtests turned on nothing but this**, the single largest named mechanism
/// in the suite's failures, plus 13 more in `comp_embedded_control` whose fixtures are *all*
/// encapsulating labels.
///
/// ⚠⚠⚠ **IT IS NOT A CONFORMANCE POINT — IT IS THE AGENTIC SURFACE'S GROUND TRUTH.** CONSTITUTION
/// I3 makes this tree a load-bearing subsystem, and `manuk-agent` resolves *"tick the Remember me
/// box"* through exactly this name. A checkbox inside its own `<label>` had **no name at all**, so
/// it was an anonymous, unaddressable box to the agent — on the commonest form idiom on the web.
///
/// Two further defects lived in the one-line scan this replaces, and both are fixed here:
///
/// * **it returned the LAST label, not the first.** `find_label_for` walked with `stack.pop()`
///   after `stack.extend(children)` — reverse document order per level — and took the first match
///   it happened to *visit*.
/// * **it returned ONE label.** HTML-AAM concatenates **every** `<label>` associated with a
///   control, in tree order: `<label for=x>a</label><label for=x>b</label>` names it `"a b"`.
///
/// Built once per tree (or once per [`accessible_name`] call) exactly as [`id_index`] is, so the
/// per-control cost drops from **a whole-document walk each** to one hash lookup.
type LabelIndex = HashMap<NodeId, Vec<NodeId>>;

/// The two document-wide lookups an accessible name needs: `id` → node (for `aria-labelledby`) and
/// control → its `<label>`s.
///
/// ⚠ **They travel TOGETHER on purpose.** t1097's lesson in this very file was that `Page` builds
/// its AX tree through more than one entry point, and threading a new input into only one leaves
/// the other silently unfixed while the diff looks complete. Bundling them makes *"did every
/// caller get it"* a type error instead of a review.
struct NameIndex {
    ids: HashMap<String, NodeId>,
    labels: LabelIndex,
}

impl NameIndex {
    fn build(dom: &Dom) -> Self {
        let ids = id_index(dom);
        let labels = label_index(dom, &ids);
        Self { ids, labels }
    }
}

/// A subtree in **document order**. [`id_index`]'s stack walk is reverse order per level, which is
/// fine for a first-wins id map and useless for *"which label came first"*.
fn document_order(dom: &Dom, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        let kids: Vec<NodeId> = dom.children(n).collect();
        stack.extend(kids.into_iter().rev());
    }
    out
}

/// HTML's **labelable** elements — the ones a `<label>` can be associated with. A `<label>` wrapped
/// around anything else associates with nothing, which is why "first labelable descendant" has to
/// check rather than take the first element it meets.
fn is_labelable(dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    match el.name.as_str() {
        "button" | "meter" | "output" | "progress" | "select" | "textarea" => true,
        // `<input type=hidden>` is the one input that is not labelable.
        "input" => !el
            .attr("type")
            .is_some_and(|t| t.eq_ignore_ascii_case("hidden")),
        _ => false,
    }
}

/// Map every control to the `<label>`s that name it, in document order.
///
/// HTML: a `<label>`'s labeled control is the element its `for` names — **and `for` wins even when
/// it resolves to nothing**, so `<label for="typo"><input></label>` labels the input in no engine.
/// Without `for`, it is the label's **first labelable descendant**.
fn label_index(dom: &Dom, ids: &HashMap<String, NodeId>) -> LabelIndex {
    let mut out: LabelIndex = HashMap::new();
    for n in document_order(dom, dom.root()) {
        let Some(el) = dom.element(n) else { continue };
        if el.name != "label" {
            continue;
        }
        let control = match el.attr("for") {
            Some(f) => ids.get(f).copied().filter(|&c| is_labelable(dom, c)),
            None => document_order(dom, n)
                .into_iter()
                .find(|&d| d != n && is_labelable(dom, d)),
        };
        if let Some(c) = control {
            out.entry(c).or_default().push(n);
        }
    }
    out
}

/// The text a `<label>` contributes to `target`'s name — accname §4.3, and **two of its steps are
/// what make this more than `text_content`**.
///
/// 1. **The labelled control contributes NOTHING to its own label.** `<label><input type=checkbox
///    value="test">checkbox label</label>` is named *"checkbox label"*; folding the input's own
///    value back in would name the control after itself.
/// 2. ⭐ **A DIFFERENT control embedded in the label contributes its VALUE** (accname §4.3 step 2C).
///    `<label><input type=checkbox> Flash the screen <input value="3"> times</label>` is
///    *"Flash the screen 3 times"* — the number the user actually set is part of the sentence.
///    Plain `text_content` gives *"Flash the screen times"*, which is a different instruction.
/// The ARIA name an element carries as an ATTRIBUTE — `aria-labelledby` dereferenced one level,
/// else `aria-label`. accname §4.3 steps 2B and 2D.
///
/// ⚠ One level only, and that is a stated boundary rather than an accident: `aria-labelledby`
/// chains are cycles waiting to happen, and the spec stops the recursion at the first hop for the
/// same reason.
fn aria_name_of(dom: &Dom, node: NodeId, index: &NameIndex, ctx: &NameCtx<'_>) -> Option<String> {
    let el = dom.element(node)?;
    if let Some(refs) = el.attr("aria-labelledby") {
        let text = refs
            .split_ascii_whitespace()
            .filter_map(|id| index.ids.get(id))
            .map(|&n| referenced_name(dom, n, index, ctx))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() {
            return Some(text);
        }
    }
    let label = normalize(el.attr("aria-label")?);
    (!label.is_empty()).then_some(label)
}

/// ⭐⭐⭐ **A REFERENCED NODE CONTRIBUTES ITS NAME, NOT ITS TEXT — accname §4.3 step 2B.**
///
/// `aria-labelledby` was dereferenced with `dom.text_content()`, which is the t1353 defect one
/// level further out and it fails in three separate ways:
///
/// ```html
///   <button aria-labelledby="cb">Toggle</button>
///   <input type=checkbox id=cb><label for=cb>Checkbox Label Text</label>
/// ```
///
/// A referenced **CONTROL** has no text at all, so the button was named *"Toggle"* — its own
/// content — instead of *"Checkbox Label Text"*. A referenced node with a `display:none` fragment
/// inside it contributed that fragment. And a referenced node's own `aria-label` was ignored.
///
/// ⚠ **THE CYCLE GUARD IS THE WHOLE REASON THIS IS A SEPARATE FUNCTION.** `aria-labelledby="self"`
/// is a real, tested authoring pattern (*"my own label, then that heading"*), so the referenced
/// computation must NOT re-enter `aria-labelledby` — one hop, exactly as the spec says.
///
/// ⚠ And a referenced node is computed **as if name-from-content were allowed**, whatever its role:
/// a `<span id=x>text</span>` is a `generic`, and pointing at it is precisely how authors name
/// things.
fn referenced_name(dom: &Dom, node: NodeId, index: &NameIndex, ctx: &NameCtx<'_>) -> String {
    let Some(el) = dom.element(node) else {
        // A referenced text node contributes its own data.
        return normalize(&dom.text_content(node));
    };
    // Step 2D — the referenced element's OWN aria-label. (2B is skipped: one hop.)
    if let Some(l) = el
        .attr("aria-label")
        .map(normalize)
        .filter(|l| !l.is_empty())
    {
        return l;
    }
    // Step 2E — the host-language label, which is what makes a referenced CONTROL work.
    if let Some(role) = role_of(dom, node) {
        let host = host_language_name(dom, node, &role, index, ctx);
        if !host.is_empty() {
            return host;
        }
    }
    // Step 2F — name from content, ALLOWED here regardless of role. The hidden-node exemption
    // applies only when the REFERENCED element is itself hidden (see `content_text_rooted`).
    let (display_none, vis) = node_visibility(dom, node, ctx.styles);
    let root_hidden = is_hidden(dom, node) || display_none || vis == Some(false);
    content_text_rooted(dom, node, None, index, ctx, root_hidden)
}

/// ⭐⭐⭐ **ONE WALK, TWO CALLERS — AND ONLY ONE OF THEM USED TO RECURSE.**
///
/// accname §4.3 defines a single traversal, and this engine had it twice: `label_text` walked a
/// `<label>` properly (skipping the labelled control, substituting an embedded control's value),
/// while **name-from-content flattened its subtree with `dom.text_content()`**. A flatten cannot
/// see what the spec puts in the middle of a name:
///
/// ```html
///   <button><span>one</span> <img alt="two"> <span>three</span></button>   "one two three"
///   <h3>heading <a aria-label="link aria-label">ignored</a> heading</h3>   "heading link aria-label heading"
/// ```
///
/// `text_content` reads the first as *"one three"* — the picture in the middle of the button is
/// silently dropped — and the second as *"heading ignored heading"*, announcing the very text the
/// author overrode. Both are names an agent then cannot match on.
///
/// `skip` is the labelled control, which contributes nothing to its own label; `root` is the
/// element being named, whose own `aria-label` was already consulted by the caller and must not be
/// re-read here or every name would be its own attribute.
fn content_text(
    dom: &Dom,
    root: NodeId,
    skip: Option<NodeId>,
    index: &NameIndex,
    ctx: &NameCtx<'_>,
) -> String {
    content_text_rooted(dom, root, skip, index, ctx, false)
}

/// ⚠⚠⚠ **A HIDDEN NODE THAT `aria-labelledby` POINTS AT CONTRIBUTES ITS WHOLE SUBTREE.**
///
/// accname §4.3 step 2A: *"if the current node is hidden and is **not** directly referenced by
/// `aria-labelledby`, return the empty string."* Authors rely on this constantly — a
/// `<span id="lbl" hidden>Delete permanently</span>` exists **only** to be pointed at, and it is
/// how you attach a long name to an icon button without printing it on the page.
///
/// ⚠ **THE EXEMPTION IS THE SUBTREE, NOT THE ONE NODE, AND THE REASON IS MECHANICAL:** if the
/// referenced element is `display:none`, its children are hidden *because it is* — nothing can tell
/// a child's own `display:none` apart from the one it inherits, so pruning inside would make the
/// reference contribute nothing and defeat the exemption entirely. WPT tests exactly this pair:
///
/// ```html
///   <span id=span1 style="display:none"><span style="display:none">label</span></span>   -> "label"
///   <span id=span5><span style="visibility:hidden">label</span></span>                   -> ""
/// ```
///
/// The first is a hidden reference (exempt throughout); the second is a VISIBLE reference with a
/// hidden fragment inside it, which contributes nothing and falls back to the button's own
/// `aria-label`. One boolean tells them apart, and getting it wrong costs one of the two either way.
fn content_text_rooted(
    dom: &Dom,
    root: NodeId,
    skip: Option<NodeId>,
    index: &NameIndex,
    ctx: &NameCtx<'_>,
    exempt_hidden: bool,
) -> String {
    let mut out = String::new();
    content_walk(
        dom,
        root,
        root,
        skip,
        index,
        ctx,
        true,
        exempt_hidden,
        &mut out,
    );
    normalize(&out)
}

#[allow(clippy::too_many_arguments)]
fn content_walk(
    dom: &Dom,
    n: NodeId,
    root: NodeId,
    skip: Option<NodeId>,
    index: &NameIndex,
    ctx: &NameCtx<'_>,
    visible: bool,
    exempt_hidden: bool,
    out: &mut String,
) {
    // The control being named never names itself.
    if Some(n) == skip {
        return;
    }
    // Set once for the whole traversal, when the REFERENCED node is itself hidden.
    let exempt = exempt_hidden;
    if !dom.is_element(n) {
        // A text node's `text_content` is its own data; a comment's is empty. Both correct here.
        //
        // ⭐ **`text-transform` APPLIES, BECAUSE THE NAME IS THE TEXT A USER IS READ.** A heading
        // styled `text-transform: uppercase` is named `"CALL US"`, not `"Call us"` — the transform
        // is not decoration, it changes the characters the rendering produces, and accname computes
        // the name from what is rendered. Chrome-verified on all four keywords.
        //
        // ⚠ The transform is read from the text node's OWN entry when it has one (a text node
        // clones its parent's computed style) and otherwise from the nearest element ancestor that
        // does, because a `NameStyles` built from a partial style map must not silently drop the
        // parent's transform.
        if visible {
            out.push_str(&transform_name(
                &dom.text_content(n),
                inherited_transform(dom, n, ctx.styles),
            ));
        }
        return;
    }
    if !exempt && is_hidden(dom, n) {
        return;
    }
    // ⚠⚠⚠ **`display:none` PRUNES; `visibility:hidden` DOES NOT.** `visibility` is the one hiding
    // mechanism a descendant can UNDO — `visibility:visible` inside a hidden ancestor is shown, and
    // is in the name — so it flows down as a flag rather than ending the walk. Getting this
    // backwards either loses text the user can read or announces text they cannot.
    let (display_none, vis) = node_visibility(dom, n, ctx.styles);
    if display_none && !exempt {
        return;
    }
    let visible = if exempt { true } else { vis.unwrap_or(visible) };
    if n != root {
        // §4.3 step 2C — an embedded control speaks its VALUE, not its subtree.
        if let Some(v) = embedded_control_value(dom, n) {
            if visible {
                push_word(out, &v);
            }
            return;
        }
        // §4.3 steps 2B/2D — a descendant that carries its OWN ARIA name contributes that name and
        // its subtree is NOT descended into. Overriding a name is the whole point of `aria-label`;
        // reading through it announces the text the author replaced.
        if let Some(name) = aria_name_of(dom, n, index, ctx) {
            if visible {
                push_word(out, &name);
            }
            return;
        }
        // Host-language: an `<img>`/`<area>` contributes its `alt` and nothing else. An `alt=""`
        // contributes NOTHING — and either way we do not descend, because an image has no text.
        if let Some(el) = dom.element(n) {
            if matches!(el.name.as_str(), "img" | "area") {
                if let Some(alt) = el.attr("alt") {
                    let alt = normalize(alt);
                    if !alt.is_empty() && visible {
                        push_word(out, &alt);
                    }
                }
                return;
            }
        }
    }
    // ⚠⚠⚠ accname §4.3 step 2F: `::before`/`::after` text is PART OF THE CONTENT, in that order
    // around it (t1097) — and it applies at EVERY level of the walk, not only at the root.
    // ⭐ The ALT half wins over the rendered half when the declaration had a `/`, which is the
    // whole point of `content: "★" / ""` — draw a star, announce nothing. `pseudo_names` makes that
    // a three-way choice rather than `unwrap_or(rendered)`, because `Some("")` is a real answer.
    let (b_owned, a_owned) = ctx.pseudo_names(n);
    let (b, a) = (b_owned.as_str(), a_owned.as_str());
    if visible {
        out.push_str(b);
    }
    for c in dom.children(n) {
        // ⭐⭐⭐ **A CHILD THAT IS NOT AN INLINE BOX CONTRIBUTES A SPACE ON EACH SIDE OF ITS TEXT.**
        // accname's "Computed Name from Content" appends a separator around a non-inline node, and
        // it is the rule that makes the SAME MARKUP name two different things:
        //
        // ```text
        //   <button><span>one</span><span>two</span><span>three</span></button>
        //     spans inline               -> "onetwothree"
        //     spans display:block        -> "one two three"
        //     spans display:inline-block -> "one two three"
        // ```
        //
        // Only a stylesheet separates those, which is why the name walk needs the computed style at
        // all. ⚠ The `inline-block` row is the one that decides the predicate: it is an ATOMIC
        // inline — not an inline box — so `display != Inline` is the rule, not "is it block".
        let sep = ctx
            .styles
            .get(&c)
            .is_some_and(|s| separates_name(s.display) && dom.is_element(c));
        if sep && visible {
            out.push(' ');
        }
        content_walk(dom, c, root, skip, index, ctx, visible, exempt_hidden, out);
        if sep && visible {
            out.push(' ');
        }
    }
    if visible {
        out.push_str(a);
    }
}

/// Append a contribution as its own WORD. The markup around a substituted value or name carries no
/// space of its own (`…screen <input value="3"> times`), and `normalize` collapses the surplus.
/// The `text-transform` that applies to `n`'s text — its own entry, else the nearest ancestor with
/// one. `text-transform` is an inherited property, so a partial [`NameStyles`] must not read the
/// absence of an entry as `none`.
fn inherited_transform(dom: &Dom, n: NodeId, styles: &NameStyles) -> manuk_css::TextTransform {
    let mut cur = Some(n);
    while let Some(x) = cur {
        if let Some(s) = styles.get(&x) {
            return s.text_transform;
        }
        cur = dom.parent(x);
    }
    manuk_css::TextTransform::None
}

fn push_word(out: &mut String, s: &str) {
    out.push(' ');
    out.push_str(s);
    out.push(' ');
}

/// accname §4.3 step 2C: what a control **embedded in a label** contributes.
///
/// A widget in the middle of a label is not read out as its subtree — it is read out as its current
/// VALUE, because that is what the sentence means. `None` means *"not one of these"*, and the
/// ordinary text walk handles it: a `<span role=combobox>3</span>` needs no special case, because
/// its content already **is** its value.
fn embedded_control_value(dom: &Dom, n: NodeId) -> Option<String> {
    let el = dom.element(n)?;
    match role_of(dom, n)? {
        Role::TextBox => match el.name.as_str() {
            "input" => Some(normalize(el.attr("value").unwrap_or(""))),
            "textarea" => Some(normalize(&dom.text_content(n))),
            _ => None,
        },
        Role::ComboBox | Role::ListBox => {
            if el.name == "input" {
                return Some(normalize(el.attr("value").unwrap_or("")));
            }
            // ⚠ `.or_else(text)` and NOT `None`: step 2C outranks step 2D, so an ARIA-only
            // `<span role=combobox aria-label="number of times">3</span>` must contribute **3**,
            // not "number of times". Falling through to the ordinary walk used to give the same
            // answer by accident; once a descendant's `aria-label` is honoured, the accident stops
            // working and the control announces its NAME where the sentence wants its VALUE.
            selected_option_text(dom, n).or_else(|| {
                let t = normalize(&dom.text_content(n));
                (!t.is_empty()).then_some(t)
            })
        }
        // A range speaks the value its AUTHOR chose to speak: `aria-valuetext` exists precisely so
        // `3` can be announced as "3 stars", and it outranks the raw number.
        Role::Slider | Role::SpinButton => {
            if let Some(t) = el.attr("aria-valuetext") {
                return Some(normalize(t));
            }
            if let Some(v) = el.attr("aria-valuenow") {
                return Some(normalize(v));
            }
            if el.name == "input" {
                return Some(normalize(el.attr("value").unwrap_or("")));
            }
            None
        }
        _ => None,
    }
}

/// The selected option of a `<select>` or an ARIA `listbox`.
///
/// ⚠ The two differ and the difference is observable: a `<select>` with nothing explicitly selected
/// still **shows** its first option, so that is what it contributes; an ARIA listbox with no
/// `aria-selected="true"` shows no selection and contributes nothing.
fn selected_option_text(dom: &Dom, container: NodeId) -> Option<String> {
    let mut first = None;
    let mut selected = None;
    for n in document_order(dom, container) {
        if n == container {
            continue;
        }
        let Some(el) = dom.element(n) else { continue };
        let is_option = el.name == "option"
            || el
                .attr("role")
                .is_some_and(|r| r.split_ascii_whitespace().any(|t| t == "option"));
        if !is_option {
            continue;
        }
        if first.is_none() {
            first = Some(n);
        }
        if el.attr("selected").is_some()
            || el
                .attr("aria-selected")
                .is_some_and(|v| v.eq_ignore_ascii_case("true"))
        {
            selected = Some(n);
            break;
        }
    }
    let native = dom.element(container).is_some_and(|e| e.name == "select");
    let pick = selected.or(if native { first } else { None })?;
    Some(normalize(&dom.text_content(pick)))
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
    build_tree_with_visibility(dom, rects, z_index, &HashSet::new())
}

/// As [`build_tree_with_geometry`], plus the set of nodes whose computed `visibility` is
/// `hidden`/`collapse`.
///
/// **A `visibility:hidden` element is not exposed in the accessibility tree** (WAI-ARIA: it is not
/// perceivable, so it is not represented), and the consequence that matters here is that it cannot
/// be hit-tested either. `visibility` is a *style*, so this cannot be derived from the DOM the way
/// `hidden`/`aria-hidden` can — the caller, which holds the computed styles, has to supply it.
///
/// Without this, a **closed dropdown swallows clicks on the article underneath it**. That is not
/// hypothetical: the modern web hides menus, popovers and tooltips with `visibility:hidden` while
/// leaving them laid out at full size, so an anchored panel sits over real content permanently. It
/// surfaced when tick 272 fixed `position:absolute; width:max-content` — the panels grew to their
/// correct width, and G6 clickability went 98.9% → 97.9% because four more links had a hidden
/// Wikipedia menu on top of them. The occlusion was always wrong; the panels were previously just
/// too small to cover much.
pub fn build_tree_with_visibility(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    invisible: &HashSet<NodeId>,
) -> A11yNode {
    build_tree_full(dom, rects, z_index, invisible, &HashSet::new())
}

/// [`build_tree_with_visibility`] + the set of `pointer-events: none` nodes (which stay in the tree
/// but are dropped from hit-testing). The live page path — which holds the computed styles — uses
/// this; the plain builders pass an empty set.
pub fn build_tree_full(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    invisible: &HashSet<NodeId>,
    non_hittable: &HashSet<NodeId>,
) -> A11yNode {
    build_tree_generated(
        dom,
        rects,
        z_index,
        invisible,
        non_hittable,
        &empty_name_ctx(
            &GeneratedText::new(),
            &GeneratedAlt::new(),
            &NameStyles::new(),
        ),
    )
}

/// As [`build_tree_full`], plus the rendered `::before` / `::after` text per owner — see
/// [`GeneratedText`]. This is the I3 seam: a renderer change that adds CONTENT (rather than moving
/// a box) reaches the semantic model only through here, because the shared `node_rects` producer
/// every other subsystem rides carries geometry and nothing else (t1097).
pub fn build_tree_generated(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    invisible: &HashSet<NodeId>,
    non_hittable: &HashSet<NodeId>,
    ctx: &NameCtx<'_>,
) -> A11yNode {
    let index = NameIndex::build(dom);
    let root = dom.root();
    let children = build_children(
        dom,
        root,
        &index,
        rects,
        z_index,
        invisible,
        non_hittable,
        ctx,
    );
    A11yNode {
        node: root,
        role: Role::Document,
        name: String::new(),
        bbox: None,
        z: 0,
        hittable: true,
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
    build_tree_with_focus_and_visibility(dom, rects, z_index, focused, &HashSet::new())
}

/// [`build_tree_with_focus`] + [`build_tree_with_visibility`] — what a live page uses.
pub fn build_tree_with_focus_and_visibility(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    focused: Option<NodeId>,
    invisible: &HashSet<NodeId>,
) -> A11yNode {
    build_tree_full_with_focus(dom, rects, z_index, focused, invisible, &HashSet::new())
}

/// [`build_tree_with_focus_and_visibility`] + the `pointer-events: none` set — what a live page uses
/// for the agent's focus-aware, occlusion-aware, pointer-events-honest hit-test tree.
pub fn build_tree_full_with_focus(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    focused: Option<NodeId>,
    invisible: &HashSet<NodeId>,
    non_hittable: &HashSet<NodeId>,
) -> A11yNode {
    build_tree_generated_with_focus(
        dom,
        rects,
        z_index,
        focused,
        invisible,
        non_hittable,
        &empty_name_ctx(
            &GeneratedText::new(),
            &GeneratedAlt::new(),
            &NameStyles::new(),
        ),
    )
}

/// ⚠⚠ **THE SECOND COPY, and it is here because a fix that lands in ONE of two copies looks
/// complete in the diff.** `Page` builds its AX tree through two entry points — with and without a
/// known focus — and threading [`GeneratedText`] into only the first would leave every
/// focus-carrying caller (the shell, the agent's observation channel) announcing pseudo-less names
/// while the tests on the other path passed.
pub fn build_tree_generated_with_focus(
    dom: &Dom,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    focused: Option<NodeId>,
    invisible: &HashSet<NodeId>,
    non_hittable: &HashSet<NodeId>,
    ctx: &NameCtx<'_>,
) -> A11yNode {
    let mut tree = build_tree_generated(dom, rects, z_index, invisible, non_hittable, ctx);
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
    index: &NameIndex,
    rects: &HashMap<NodeId, Rect>,
    z_index: &ZIndex,
    invisible: &HashSet<NodeId>,
    non_hittable: &HashSet<NodeId>,
    ctx: &NameCtx<'_>,
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
        // ── ⚠⚠⚠ **`display: none` PRUNES THE SUBTREE, AND THIS TREE USED TO CONTAIN IT.** The
        //    caller supplies `invisible` for `visibility`, and `is_hidden` above reads the DOM
        //    (`hidden`, `aria-hidden`, `<input type=hidden>`, non-rendered tags) — **nothing asked
        //    about `display`**, so a closed mobile menu, a `display:none` modal and a `<dialog>`
        //    without `open` were all in the agent's a11y tree as fully-formed, addressable nodes.
        //    Chrome-measured (CDP `Accessibility.getFullAXTree`): its tree contains none of them.
        //
        //    ⭐ **THE ASYMMETRY WITH `visibility` IS THE WHOLE RULE, AND IT IS WHY THIS CANNOT JOIN
        //    THE `invisible` SET.** `visibility` INHERITS and is UNDOABLE, so that arm drops the
        //    node and KEEPS WALKING (a `visibility: visible` descendant survives). `display` does
        //    NOT inherit and cannot be undone: a child of a `display: none` box computes its own
        //    ordinary `display`, so a per-node test would never fire on the child — the prune has to
        //    happen at the ancestor by NOT DESCENDING. `continue` here is that.
        //
        //    ⭐ t1379 fixed the NAME walk to read the computed `display` and left this walk alone,
        //    which produced the symptom that names the bug: `<button style="display:none">Hidden
        //    inline</button>` was in the tree with an **EMPTY NAME**. A node whose name is correctly
        //    computed as nothing is a node that should not be there.
        //
        //    `node_visibility` is t1379's resolver, reused: the computed `display` when the caller
        //    has a style map, and the inline `style=` attribute when it does not — so `build_tree`
        //    on a bare DOM behaves exactly as it did.
        if node_visibility(dom, child, ctx.styles).0 {
            continue;
        }
        // `visibility:hidden` drops the NODE but **keeps walking**, because `visibility` is the one
        // hiding mechanism a descendant can undo: `visibility:visible` inside a hidden ancestor is
        // shown, and is in Chrome's accessibility tree. Pruning the subtree here would delete it.
        // (`display:none` and `hidden`/`aria-hidden` above are not undoable, so those do prune.)
        if invisible.contains(&child) {
            out.extend(build_children(
                dom,
                child,
                index,
                rects,
                z_index,
                invisible,
                non_hittable,
                ctx,
            ));
            continue;
        }
        // The tree root already *is* the document; `<html>` must not nest a second
        // `document` node inside it. Reparent its children instead.
        if dom.element(child).is_some_and(|e| e.name == "html") {
            out.extend(build_children(
                dom,
                child,
                index,
                rects,
                z_index,
                invisible,
                non_hittable,
                ctx,
            ));
            continue;
        }
        match role_of(dom, child) {
            Some(role) => {
                let name = accessible_name_with(dom, child, &role, index, ctx);
                let state = state_of(dom, child, &role);
                out.push(A11yNode {
                    node: child,
                    role,
                    name,
                    bbox: rects.get(&child).copied(),
                    z: z_index.get(&child).copied().unwrap_or(0),
                    // `pointer-events: none` — the node is announced but is not a hit target.
                    hittable: !non_hittable.contains(&child),
                    state,
                    children: build_children(
                        dom,
                        child,
                        index,
                        rects,
                        z_index,
                        invisible,
                        non_hittable,
                        ctx,
                    ),
                });
            }
            // presentational: drop the node, keep (reparent) its children
            None => out.extend(build_children(
                dom,
                child,
                index,
                rects,
                z_index,
                invisible,
                non_hittable,
                ctx,
            )),
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
            hittable: true,
            state: A11yState::default(),
            children: vec![],
        }
    }

    /// ⚠⚠⚠ **A DESCENDANT BEATS ITS ANCESTOR EVEN WHEN THE ANCESTOR'S BOX IS SMALLER** — the case
    /// the old flat area scan got wrong, and it is not hypothetical geometry.
    ///
    /// Wikipedia's `.hlist li { display: inline }` puts an `<a>` inside an inline `<li>`. Both are
    /// boxless, so both take their geometry from the same line, and once t853 gave each inline its
    /// own content area the `<li>` came out a third of a pixel wider and a quarter-pixel taller
    /// than the `<a>` it contains. Under smallest-area-wins the **ancestor** took the click, and
    /// the shell — which walks *up* from the hit node looking for an `<a href>` — found no link
    /// above an `<li>`. **16 links on the G6 page became unclickable on float dust.**
    ///
    /// Chrome's `elementFromPoint` has no such ambiguity: topmost, then deepest, full stop.
    ///
    /// Goes RED by restoring the area comparison for the ancestor/descendant pair — the parent
    /// `NodeId(1)` is returned and every nested link on the web becomes a click on its wrapper.
    #[test]
    fn hit_test_prefers_a_descendant_over_a_smaller_ancestor() {
        let link = A11yNode {
            node: NodeId(2),
            role: Role::Link,
            name: "Collie".into(),
            // Marginally LARGER than its parent, in both axes — the float dust that inverted this.
            bbox: Some(Rect {
                x: 740.0,
                y: 3193.75,
                width: 34.5,
                height: 16.25,
            }),
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![],
        };
        let li = A11yNode {
            node: NodeId(1),
            role: Role::ListItem,
            name: String::new(),
            bbox: Some(Rect {
                x: 740.33,
                y: 3193.75,
                width: 34.43,
                height: 16.0,
            }),
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![link],
        };
        let root = A11yNode {
            node: NodeId(0),
            role: Role::Document,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![li],
        };
        assert_eq!(
            root.hit_test(755.0, 3200.0).map(|n| n.node),
            Some(NodeId(2)),
            "the <a> inside an inline <li> must take the click even though the <li>'s box is \
             smaller — an ancestor is never a more specific target than its own descendant, and \
             resolving that pair by AREA is how 16 Wikipedia links became unclickable"
        );
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
            hittable: true,
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
            hittable: true,
            state: A11yState::default(),
            children: vec![],
        };
        let root = A11yNode {
            node: NodeId(0),
            role: Role::Document,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![button, overlay],
        };
        assert_eq!(root.hit_test(20.0, 15.0).map(|n| n.node), Some(NodeId(2)));
    }

    #[test]
    fn hit_test_passes_through_a_pointer_events_none_overlay() {
        // A high-`z` overlay covers a button — but the overlay is `pointer-events: none` (hittable
        // false), so a coordinate click passes THROUGH it to the button behind. Without the `hittable`
        // skip this returns the overlay (NodeId 2) — the occlusion test above proves that is what a
        // *hittable* overlay does — so this is the RED-prover for the pointer-events fix.
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
            hittable: true,
            state: A11yState::default(),
            children: vec![],
        };
        let ghost = A11yNode {
            node: NodeId(2),
            role: Role::Generic,
            name: "scrim".into(),
            bbox: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            }),
            z: 10,
            hittable: false, // pointer-events: none
            state: A11yState::default(),
            children: vec![],
        };
        let root = A11yNode {
            node: NodeId(0),
            role: Role::Document,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![button, ghost],
        };
        // The click lands on the button behind the ghost, not the ghost.
        assert_eq!(root.hit_test(20.0, 15.0).map(|n| n.node), Some(NodeId(1)));
    }

    #[test]
    fn a11y_diff_reports_added_and_removed() {
        let before = A11yNode {
            node: NodeId(0),
            role: Role::Generic,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
            state: A11yState::default(),
            children: vec![leaf(Role::Link, "Sign in"), leaf(Role::Button, "Menu")],
        };
        let after = A11yNode {
            node: NodeId(0),
            role: Role::Generic,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
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

    /// **A `visibility:hidden` panel must not swallow clicks on the content underneath it.**
    ///
    /// The modern web hides every dropdown, popover, menu and tooltip with `visibility:hidden`
    /// while leaving it **laid out at full size**, so an anchored panel sits permanently over real
    /// content. Hit-testing consulted only the box, so the click landed on the invisible menu — a
    /// link the user can see, aim at, and not click. Caught by G6 when tick 272 corrected
    /// `position:absolute; width:max-content` and the panels grew to their true width: clickability
    /// went 98.9% → 97.9%. The occlusion was always wrong; the panels had merely been too small to
    /// cover much.
    ///
    /// `visibility` is the one hiding mechanism a descendant can UNDO, so the second half of this
    /// matters as much as the first: `visibility:visible` inside a hidden ancestor is shown by
    /// Chrome and is in its accessibility tree. Pruning the subtree would delete it.
    #[test]
    fn visibility_hidden_boxes_are_not_exposed_and_do_not_swallow_clicks() {
        let mut shown_id = NodeId(0);
        let mut panel_id = NodeId(0);
        let dom = dom_with(|d, body| {
            let link = d.create_element("a");
            d.set_attr(link, "href", "/article");
            let t = d.create_text("Read the article");
            d.append_child(link, t);
            d.append_child(body, link);

            // A closed dropdown, laid out on top of the link.
            let panel = d.create_element("div");
            let hidden_btn = d.create_element("button");
            let ht = d.create_text("Menu item");
            d.append_child(hidden_btn, ht);
            d.append_child(panel, hidden_btn);
            // ...containing one descendant that turns visibility back ON.
            let shown = d.create_element("button");
            let st = d.create_text("Still visible");
            d.append_child(shown, st);
            d.append_child(panel, shown);
            d.append_child(body, panel);
            panel_id = panel;
            shown_id = shown;
        });

        let r = |x: f32, y: f32, w: f32, h: f32| Rect {
            x,
            y,
            width: w,
            height: h,
        };
        let mut rects: HashMap<NodeId, Rect> = HashMap::new();
        for (i, n) in dom.descendants(dom.root()).enumerate() {
            let _ = i;
            if let Some(el) = dom.element(n) {
                match el.name.as_str() {
                    "a" => {
                        rects.insert(n, r(0.0, 0.0, 200.0, 40.0));
                    }
                    "div" => {
                        rects.insert(n, r(0.0, 0.0, 200.0, 40.0));
                    }
                    "button" => {
                        rects.insert(n, r(0.0, 0.0, 200.0, 40.0));
                    }
                    _ => {}
                }
            }
        }
        let z = ZIndex::new();

        // Precondition: with NOTHING marked invisible the panel really does win the click, so the
        // assertion below is testing the visibility rule and not an accident of geometry.
        let visible_tree = build_tree_with_visibility(&dom, &rects, &z, &HashSet::new());
        let occluder = visible_tree
            .hit_test(100.0, 20.0)
            .expect("something is hit");
        assert_ne!(
            occluder.role,
            Role::Link,
            "precondition: the panel must be on top when it is NOT hidden, else this test proves \
             nothing about visibility"
        );

        // The panel and its ordinary child are hidden; the re-shown descendant is not.
        let invisible: HashSet<NodeId> = dom
            .descendants(dom.root())
            .filter(|&n| {
                n == panel_id
                    || (dom.parent(n) == Some(panel_id) && n != shown_id)
                    || dom.parent(n).and_then(|p| dom.parent(p)) == Some(panel_id) && n != shown_id
            })
            .filter(|&n| n != shown_id)
            .collect();
        let tree = build_tree_with_visibility(&dom, &rects, &z, &invisible);

        // 1 — the re-shown descendant SURVIVES a hidden ancestor.
        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("Still visible")),
            "visibility:visible inside a visibility:hidden ancestor must stay in the tree, got {names:?}"
        );

        // 2 — the hidden panel's own hidden child is gone.
        assert!(
            !names.iter().any(|n| n.contains("Menu item")),
            "a visibility:hidden node must not be exposed, got {names:?}"
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
    /// **A NO-BREAK SPACE SURVIVES INTO THE ACCESSIBLE NAME; ASCII WHITESPACE COLLAPSES.**
    ///
    /// accname §4 and HTML collapse *ASCII* whitespace. `split_whitespace` collapses *Unicode*
    /// whitespace, and U+00A0 is Unicode whitespace — so every non-breaking space in a name was
    /// silently rewritten to a plain space. It matters past conformance: the agentic surface matches
    /// on the accessible name, and an agent told to click "Sign\u{a0}up" against an engine that
    /// stored "Sign up" does not find the element. NBSP lives in exactly the short UI strings agents
    /// target — prices, "Sign\u{a0}up", "Add\u{a0}to\u{a0}cart", French punctuation.
    ///
    /// **To watch it go RED:** put `split_whitespace` back in `normalize`.
    /// `landing` clips the target to the VIEWPORT before aiming, because the centre of a box
    /// half-scrolled off the screen is off the screen. A coordinate no pointer can reach is not a
    /// click point, and publishing one is the same class of lie as publishing an obstructed one.
    #[test]
    fn a_click_point_is_inside_the_part_of_the_target_that_is_on_screen() {
        let target = A11yNode {
            node: NodeId(2),
            role: Role::Button,
            name: "Tall".into(),
            bbox: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 1000.0,
            }),
            z: 0,
            hittable: true,
            state: Default::default(),
            children: vec![],
        };
        let root = A11yNode {
            node: NodeId(1),
            role: Role::Document,
            name: String::new(),
            bbox: None,
            z: 0,
            hittable: true,
            state: Default::default(),
            children: vec![target],
        };

        // Scrolled to y=900: only the bottom 100px of the button is on screen, so the aimed point
        // must be in 900..1000 — NOT the box centre at y=500, which is a screen away.
        let vp = Rect {
            x: 0.0,
            y: 900.0,
            width: 100.0,
            height: 600.0,
        };
        let Landing::Clear { point } = root.landing(NodeId(2), Some(vp)) else {
            panic!("the visible part of the button is reachable");
        };
        assert!(
            (900.0..1000.0).contains(&point.1),
            "aimed at {point:?}, which is not in the visible band 900..1000"
        );

        // ⚠ **THIS ROW USED TO ASSERT `Unreachable`, AND THAT WAS THE BUG'S OWN PREMISE.**
        // "Scrolled past it entirely" is not "there is nothing to aim at" — it is "you are looking
        // at the wrong part of the document", and the answer an agent needs is the scroll that
        // fixes it, not a refusal. Conflating the two is what let `resolve_target` hand back
        // `Ready { point: <off-screen coordinate>, confidence: 1.0 }` for everything below the
        // fold. `dy` is negative here because the target is ABOVE the viewport: an agent that
        // scrolled too far is told to come back up.
        let past = Rect {
            x: 0.0,
            y: 1200.0,
            width: 100.0,
            height: 600.0,
        };
        assert_eq!(
            root.landing(NodeId(2), Some(past)),
            Landing::OffScreen { dy: -1200.0 }
        );

        // `Unreachable` keeps a real row, and it is the one no scroll can fix: parked outside the
        // viewport's HORIZONTAL band, where every `dy` is a lie.
        let aside = Rect {
            x: 400.0,
            y: 0.0,
            width: 100.0,
            height: 600.0,
        };
        assert_eq!(root.landing(NodeId(2), Some(aside)), Landing::Unreachable);

        // With no viewport the whole border box is fair game, so the centre stands.
        assert_eq!(
            root.landing(NodeId(2), None),
            Landing::Clear {
                point: (50.0, 500.0)
            }
        );
    }

    #[test]
    fn a_non_breaking_space_survives_name_normalisation() {
        // Collapsed: runs of ASCII whitespace become one space, and the ends are trimmed.
        assert_eq!(normalize("  button \t\n label  "), "button label");
        // PRESERVED: U+00A0 is not ASCII whitespace and is part of the author's string.
        assert_eq!(normalize("button\u{a0}label"), "button\u{a0}label");
        assert_eq!(
            normalize("\u{a0}lead"),
            "\u{a0}lead",
            "a LEADING nbsp is not trimmed either"
        );
        assert_eq!(
            normalize("trail\u{a0}"),
            "trail\u{a0}",
            "nor a trailing one"
        );
        // Mixed: the ASCII run collapses, the nbsp stays, and they do not merge into one space.
        assert_eq!(normalize("a \u{a0} b"), "a \u{a0} b");
        // CONTROL — an all-ASCII name is untouched by this change.
        assert_eq!(normalize("plain  name"), "plain name");
    }

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

    /// # G_AX_GENERATED_NAME — accname §4.3 step 2F: the pseudo text IS part of the name
    ///
    /// Generated content is **not in the DOM by construction**, and this tree is built from the
    /// DOM, so a `::before` could not reach `accessible_name` by any path (t1097). The result was
    /// silent and worst where the pseudo carries the ONLY text:
    ///
    /// ```text
    ///   button::before{content:"★ "}                   Chrome "★ Save"        ours "Save"
    ///   a::after{content:" (opens in a new tab)"}      Chrome "Docs (…)"       ours "Docs"
    /// ```
    ///
    /// ⚠⚠ **THE ORDER IS THE ASSERTION.** `before` precedes the content and `after` follows it —
    /// an implementation that concatenated them in either fixed order, or appended both, passes a
    /// "does the text appear" check and announces nonsense.
    ///
    /// ⚠ **And the negative arm: an owner with no generated content must be UNCHANGED.** This
    /// threading touches the name of every node in the tree; the common case is no pseudo at all.
    ///
    /// To watch it go RED: pass `&GeneratedText::new()` from `build_tree_generated`, or swap the
    /// `{b}{}{a}` order in the name-from-content branch.
    #[test]
    fn an_accessible_name_includes_its_before_and_after_content_in_order() {
        let dom = dom_with(|d, body| {
            let btn = d.create_element("button");
            let t = d.create_text("Save");
            d.append_child(btn, t);
            d.append_child(body, btn);

            let plain = d.create_element("button");
            let pt = d.create_text("Plain");
            d.append_child(plain, pt);
            d.append_child(body, plain);
        });
        let body = dom
            .children(dom.children(dom.root()).next().unwrap())
            .next()
            .unwrap();
        let kids: Vec<NodeId> = dom.children(body).collect();

        let mut generated = GeneratedText::new();
        generated.insert(kids[0], ("* ".to_string(), " (2)".to_string()));

        let name = |n: NodeId, g: &GeneratedText| {
            let r = role_of(&dom, n).unwrap();
            // t1355 widened this parameter from the bare id map to the full `NameIndex` (ids +
            // labels) and left this caller behind, which broke the WHOLE crate's `cfg(test)` build
            // — invisibly, because `manuk-a11y` is not in the wall's crate list.
            let index = NameIndex::build(&dom);
            // t1365 widened it again, with `NameStyles`. This unit test has no style map, and an
            // EMPTY one is the right argument rather than a constructed one: the rows below are
            // about `::before`/`::after` ordering, and a `NameStyles` that separated their children
            // would change the expected strings for a reason that has nothing to do with what they
            // assert. Surface audit #78 measured why this caller keeps being left behind —
            // `manuk-a11y` is a suite in no wall and no CI job.
            accessible_name_with(
                &dom,
                n,
                &r,
                &index,
                &empty_name_ctx(g, &GeneratedAlt::new(), &NameStyles::new()),
            )
        };
        assert_eq!(
            name(kids[0], &generated),
            "* Save (2)",
            "accname §4.3 step 2F: ::before precedes the content and ::after follows it. Bare \
             \"Save\" means the pseudo text never reached the tree — which it could not, until the \
             producer was threaded (t1098); a different ORDER means it reached it wrongly."
        );
        assert_eq!(
            name(kids[1], &generated),
            "Plain",
            "…and the negative arm: an owner with NO generated content is untouched. This \
             threading runs for every node in the tree and the common case is no pseudo at all."
        );
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

    /// **`<search>` is a LANDMARK, and an unmapped landmark is an AGENTIC gap, not only an a11y one.**
    ///
    /// `Role::Search` existed for the explicit `role="search"` attribute; the ELEMENT (Baseline Apr 2026,
    /// HTML-AAM `role=search`) was missing from the tag map and fell through to `Role::Generic`. Per
    /// CONSTITUTION VI.1 this tree already feeds `manuk-agent`'s observation channel, so on any site that
    /// adopted the wrapper the agent could not find "the search box" **by role** at all. Found by surface
    /// audit #29 (t558) — the audit that read what SHIPPED rather than what the vendors prioritised — and
    /// landed to discharge the I3 queue Constitution Checks #35/#36/#37 flagged three times running.
    #[test]
    fn the_search_element_is_a_search_landmark() {
        let dom = manuk_html::parse(
            r#"<html><body>
                 <search><input type="search" name="q"><button>Go</button></search>
                 <div role="search"><input></div>
                 <nav></nav>
               </body></html>"#,
        );
        // Locate by TAG rather than by child index: `<head>` is html's first element child, so an
        // index walk finds head and reports an empty role list — which is a misleading pass/fail either
        // way.
        let by_tag = |t: &str| -> Vec<Option<Role>> {
            dom.descendants(dom.root())
                .filter(|&n| dom.tag_name(n) == Some(t))
                .map(|n| role_of(&dom, n))
                .collect()
        };
        let roles = by_tag("search");
        assert_eq!(
            roles.first().cloned().flatten(),
            Some(Role::Search),
            "the <search> ELEMENT maps to the search landmark — Generic here means an agent cannot \
             find the search box by role: {roles:?}"
        );
        assert_eq!(
            by_tag("div").first().cloned().flatten(),
            Some(Role::Search),
            "…and the explicit role=\"search\" attribute still works (this fix must not shadow it)"
        );
        assert_eq!(
            by_tag("nav").first().cloned().flatten(),
            Some(Role::Navigation),
            "…and its landmark neighbours are untouched"
        );
    }
}
