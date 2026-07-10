//! manuk-css — the style engine.
//!
//! CLAUDE.md names **Stylo** (Servo/Firefox's production CSS engine) as the reuse
//! target for CSS parsing + cascade. Stylo is heavy to build and drive, so it sits
//! behind the [`StyleEngine`] trait and the `stylo` cargo feature. The default
//! build ships [`MinimalCascade`] — a from-scratch cascade over a documented CSS
//! subset — so the whole workspace compiles, runs, and is testable without it.
//!
//! The subset is deliberately small (tag/id/class/`*` selectors, the descendant
//! combinator, and the box/text properties layout+paint consume). It is enough to
//! render real content; it is **not** a conformance target. Conformance is Stylo's
//! job, verified against the WPT `css/` suites (CLAUDE.md § verification).
//!
//! `cssparser` (the tokenizer Stylo itself is built on) is reused for robust
//! length/number tokenization; see [`values`].

use std::collections::HashMap;

use manuk_dom::{Dom, NodeData, NodeId};

pub mod values;

pub use values::Rgba;

/// A resolved length in one of the forms layout understands. `em`/`rem` are
/// resolved to `Px` during the cascade (font sizes are known there); `%` and
/// `Auto` are resolved later against the containing block by layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Auto,
    Px(f32),
    Percent(f32),
}

impl Dim {
    /// Resolve to px against a containing-block reference length. `Auto` -> `auto_px`.
    pub fn resolve(self, reference: f32, auto_px: f32) -> f32 {
        match self {
            Dim::Auto => auto_px,
            Dim::Px(v) => v,
            Dim::Percent(p) => reference * p / 100.0,
        }
    }
    pub fn is_auto(self) -> bool {
        matches!(self, Dim::Auto)
    }
}

/// The `display` outer type, subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    TableRowGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumn,
    TableColumnGroup,
    None,
}

/// `table-layout` (CSS2 §17.5.2): fixed uses the first row / explicit widths; auto
/// sizes columns to content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TableLayout {
    #[default]
    Auto,
    Fixed,
}

/// `float`, which pulls a box out of normal flow to one side (CSS2 §9.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

/// `clear`, which pushes a box below preceding floats on the named side(s).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

/// `position` (CSS2 §9.3 + CSS-Position sticky).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

/// `white-space`, which drives inline wrapping/collapsing in layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
}

/// Four-sided box values (margin, padding, border widths).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Sides<T> {
    pub fn all(v: T) -> Self {
        Sides {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// The fully-resolved style of one element, as consumed by layout and paint.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: Rgba,
    pub background_color: Option<Rgba>,
    pub font_size: f32,
    pub font_weight: u16,
    pub italic: bool,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    pub margin: Sides<Dim>,
    pub padding: Sides<Dim>,
    pub border_width: Sides<f32>,
    pub border_color: Rgba,
    pub width: Dim,
    pub height: Dim,
    pub float: Float,
    pub clear: Clear,
    pub position: Position,
    /// `top`/`right`/`bottom`/`left` insets; `Dim::Auto` means "not set".
    pub inset: Sides<Dim>,
    /// `z-index`; `None` = `auto`.
    pub z_index: Option<i32>,
    pub table_layout: TableLayout,
    /// `border-spacing` (px) between table cells in the separated-borders model.
    pub border_spacing: f32,
}

impl ComputedStyle {
    /// The CSS initial values, used as the root's starting point and for
    /// non-inherited resets.
    pub fn initial() -> Self {
        ComputedStyle {
            display: Display::Inline,
            color: Rgba::BLACK,
            background_color: None,
            font_size: 16.0,
            font_weight: 400,
            italic: false,
            line_height: 16.0 * 1.2,
            text_align: TextAlign::Left,
            white_space: WhiteSpace::Normal,
            margin: Sides::all(Dim::Px(0.0)),
            padding: Sides::all(Dim::Px(0.0)),
            border_width: Sides::all(0.0),
            border_color: Rgba::BLACK,
            width: Dim::Auto,
            height: Dim::Auto,
            float: Float::None,
            clear: Clear::None,
            position: Position::Static,
            inset: Sides::all(Dim::Auto),
            z_index: None,
            table_layout: TableLayout::Auto,
            border_spacing: 0.0,
        }
    }

    /// Produce a child's starting style: inherited properties flow down, everything
    /// else resets to initial. (CSS inheritance model.)
    fn inherit_from(parent: &ComputedStyle) -> Self {
        let mut s = ComputedStyle::initial();
        s.color = parent.color;
        s.font_size = parent.font_size;
        s.font_weight = parent.font_weight;
        s.italic = parent.italic;
        s.line_height = parent.line_height;
        s.text_align = parent.text_align;
        s.white_space = parent.white_space;
        s
    }
}

/// Map from DOM node to its computed style. Text nodes inherit their parent's.
pub type StyleMap = HashMap<NodeId, ComputedStyle>;

// ---------------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
struct Compound {
    universal: bool,
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

/// A descendant-combinator chain; `parts[last]` is the subject (rightmost).
#[derive(Clone, Debug, PartialEq)]
struct Selector {
    parts: Vec<Compound>,
}

impl Selector {
    /// (#id, #class/attr, #type) specificity, packed big-endian into a u32.
    fn specificity(&self) -> u32 {
        let (mut a, mut b, mut c) = (0u32, 0u32, 0u32);
        for p in &self.parts {
            if p.id.is_some() {
                a += 1;
            }
            b += p.classes.len() as u32;
            if p.tag.is_some() {
                c += 1;
            }
        }
        (a.min(255) << 16) | (b.min(255) << 8) | c.min(255)
    }
}

fn compound_matches(c: &Compound, dom: &Dom, node: NodeId) -> bool {
    let Some(el) = dom.element(node) else {
        return false;
    };
    if let Some(tag) = &c.tag {
        if !el.name.eq_ignore_ascii_case(tag) {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if el.id() != Some(id.as_str()) {
            return false;
        }
    }
    for class in &c.classes {
        if !el.has_class(class) {
            return false;
        }
    }
    true
}

fn selector_matches(sel: &Selector, dom: &Dom, node: NodeId) -> bool {
    let Some((subject, ancestors)) = sel.parts.split_last() else {
        return false;
    };
    if !compound_matches(subject, dom, node) {
        return false;
    }
    // Walk ancestors, matching the remaining compounds right-to-left. Descendant
    // combinator: each must match *some* ancestor, in order.
    let mut cursor = dom.parent(node);
    for compound in ancestors.iter().rev() {
        loop {
            let Some(anc) = cursor else {
                return false;
            };
            cursor = dom.parent(anc);
            if compound_matches(compound, dom, anc) {
                break;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Stylesheet parsing (subset)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    value: String,
    important: bool,
}

#[derive(Clone, Debug)]
struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<Declaration>,
}

/// A parsed stylesheet (subset). Build one with [`Stylesheet::parse`].
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
}

impl Stylesheet {
    /// Parse CSS source into rules. Comments and `@`-rules are skipped; unknown
    /// selectors/properties are ignored rather than aborting the sheet (CSS's
    /// forward-compatible error recovery).
    pub fn parse(src: &str) -> Stylesheet {
        let src = strip_comments(src);
        let mut rules = Vec::new();
        let bytes = src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            // Skip @-rules (media/font-face/etc.) — subset does not handle them.
            if bytes[i] == b'@' {
                i = skip_at_rule(&src, i);
                continue;
            }
            // Read up to the opening brace: the selector list.
            let sel_start = i;
            while i < bytes.len() && bytes[i] != b'{' {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            let selector_text = &src[sel_start..i];
            i += 1; // consume '{'
            let decl_start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            let decl_text = &src[decl_start..i.min(bytes.len())];
            if i < bytes.len() {
                i += 1; // consume '}'
            }

            let selectors = parse_selector_list(selector_text);
            if selectors.is_empty() {
                continue;
            }
            let declarations = parse_declarations(decl_text);
            if !declarations.is_empty() {
                rules.push(Rule {
                    selectors,
                    declarations,
                });
            }
        }
        Stylesheet { rules }
    }
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

fn skip_at_rule(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut i = start;
    // Skip to ';' (statement at-rule) or a balanced '{...}' (block at-rule).
    while i < b.len() {
        match b[i] {
            b';' => return i + 1,
            b'{' => {
                let mut depth = 0;
                while i < b.len() {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                return i;
            }
            _ => i += 1,
        }
    }
    i
}

fn parse_selector_list(text: &str) -> Vec<Selector> {
    text.split(',')
        .filter_map(|s| parse_selector(s.trim()))
        .collect()
}

fn parse_selector(text: &str) -> Option<Selector> {
    if text.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for token in text.split_whitespace() {
        // Combinators like > + ~ are not in the subset; bail so we don't
        // mis-match (better to drop the rule than apply it wrongly).
        if matches!(token, ">" | "+" | "~") {
            return None;
        }
        parts.push(parse_compound(token)?);
    }
    if parts.is_empty() {
        None
    } else {
        Some(Selector { parts })
    }
}

fn parse_compound(token: &str) -> Option<Compound> {
    let mut c = Compound::default();
    let mut chars = token.chars().peekable();
    // Optional leading type or universal.
    if let Some(&ch) = chars.peek() {
        if ch == '*' {
            c.universal = true;
            chars.next();
        } else if ch.is_ascii_alphabetic() {
            let mut tag = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '.' || ch == '#' {
                    break;
                }
                tag.push(ch);
                chars.next();
            }
            c.tag = Some(tag.to_ascii_lowercase());
        }
    }
    while let Some(&ch) = chars.peek() {
        match ch {
            '.' => {
                chars.next();
                let name = take_ident(&mut chars);
                if name.is_empty() {
                    return None;
                }
                c.classes.push(name);
            }
            '#' => {
                chars.next();
                let name = take_ident(&mut chars);
                if name.is_empty() {
                    return None;
                }
                c.id = Some(name);
            }
            // `[attr]`, `:hover`, `::before` — out of subset; drop the selector.
            _ => return None,
        }
    }
    Some(c)
}

fn take_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    s
}

fn parse_declarations(text: &str) -> Vec<Declaration> {
    let mut decls = Vec::new();
    for chunk in text.split(';') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let Some((name, value)) = chunk.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let mut value = value.trim().to_string();
        let important = value.to_ascii_lowercase().ends_with("!important");
        if important {
            let cut = value.len() - "!important".len();
            value = value[..cut]
                .trim_end()
                .trim_end_matches('!')
                .trim()
                .to_string();
        }
        if name.is_empty() || value.is_empty() {
            continue;
        }
        decls.push(Declaration {
            name,
            value,
            important,
        });
    }
    decls
}

// ---------------------------------------------------------------------------
// The StyleEngine boundary + minimal cascade
// ---------------------------------------------------------------------------

/// The pluggable cascade boundary. `MinimalCascade` is the default; the `stylo`
/// feature provides a Stylo-backed implementation with the same signature.
pub trait StyleEngine {
    /// Compute a style for every node in `dom`, applying UA defaults, the given
    /// author `sheets`, and inline `style=""` attributes.
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap;
}

/// From-scratch cascade over the documented subset. See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalCascade;

impl StyleEngine for MinimalCascade {
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        let mut map = StyleMap::new();
        let root = dom.root();
        // The document root is an anonymous initial containing block.
        for child in dom.children(root) {
            self.cascade_node(dom, child, &ComputedStyle::initial(), sheets, &mut map);
        }
        map
    }
}

impl MinimalCascade {
    /// Gather author stylesheets embedded in the document's `<style>` elements.
    pub fn collect_style_elements(dom: &Dom) -> Vec<Stylesheet> {
        dom.descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("style"))
            .map(|n| Stylesheet::parse(&dom.text_content(n)))
            .collect()
    }

    // `self` (a unit struct) threads through the recursion for call-site symmetry
    // with the public `cascade`; not a real parameter smell.
    #[allow(clippy::only_used_in_recursion)]
    fn cascade_node(
        &self,
        dom: &Dom,
        node: NodeId,
        parent_style: &ComputedStyle,
        sheets: &[Stylesheet],
        map: &mut StyleMap,
    ) {
        let style = match dom.data(node) {
            NodeData::Element(el) => {
                let mut s = ComputedStyle::inherit_from(parent_style);
                apply_ua_defaults(&mut s, &el.name);

                // Author rules, ordered by (specificity, source order).
                let mut matched: Vec<(u32, usize, &Declaration)> = Vec::new();
                let mut order = 0usize;
                for sheet in sheets {
                    for rule in &sheet.rules {
                        if let Some(spec) = rule
                            .selectors
                            .iter()
                            .filter(|sel| selector_matches(sel, dom, node))
                            .map(|sel| sel.specificity())
                            .max()
                        {
                            for d in &rule.declarations {
                                matched.push((spec, order, d));
                                order += 1;
                            }
                        }
                    }
                }
                // Inline style has the highest weight.
                let inline = el.attr("style").map(parse_declarations).unwrap_or_default();

                matched.sort_by_key(|(spec, ord, _)| (*spec, *ord));
                let parent_fs = parent_style.font_size;
                for (_, _, d) in &matched {
                    apply_declaration(&mut s, d, parent_fs);
                }
                for d in &inline {
                    apply_declaration(&mut s, d, parent_fs);
                }
                // !important pass (author important beats normal), applied last.
                for (_, _, d) in matched.iter().filter(|(_, _, d)| d.important) {
                    apply_declaration(&mut s, d, parent_fs);
                }
                s
            }
            // Text/comment/doctype inherit their parent's computed style.
            _ => ComputedStyle::inherit_from(parent_style),
        };

        map.insert(node, style.clone());
        for child in dom.children(node) {
            self.cascade_node(dom, child, &style, sheets, map);
        }
    }
}

/// The user-agent default stylesheet, reduced to what the layout slice needs:
/// which elements are block vs inline vs display:none, and their default margins.
fn apply_ua_defaults(s: &mut ComputedStyle, tag: &str) {
    use Display::*;
    let (display, top_bottom_em, weight, scale): (Display, f32, u16, f32) = match tag {
        "html" | "body" | "div" | "section" | "article" | "header" | "footer" | "nav" | "main"
        | "aside" | "figure" | "figcaption" | "address" => (Block, 0.0, 400, 1.0),
        "p" | "blockquote" => (Block, 1.0, 400, 1.0),
        "h1" => (Block, 0.67, 700, 2.0),
        "h2" => (Block, 0.75, 700, 1.5),
        "h3" => (Block, 0.83, 700, 1.17),
        "h4" => (Block, 1.12, 700, 1.0),
        "h5" => (Block, 1.5, 700, 0.83),
        "h6" => (Block, 1.67, 700, 0.75),
        "ul" | "ol" => (Block, 1.0, 400, 1.0),
        "li" | "dd" | "dt" => (Block, 0.0, 400, 1.0),
        "pre" => (Block, 1.0, 400, 1.0),
        "hr" => (Block, 0.5, 400, 1.0),
        "b" | "strong" => (Inline, 0.0, 700, 1.0),
        "table" => (Table, 0.0, 400, 1.0),
        "thead" | "tbody" | "tfoot" => (TableRowGroup, 0.0, 400, 1.0),
        "tr" => (TableRow, 0.0, 400, 1.0),
        "td" => (TableCell, 0.0, 400, 1.0),
        "th" => (TableCell, 0.0, 700, 1.0),
        "caption" => (TableCaption, 0.0, 400, 1.0),
        "colgroup" => (TableColumnGroup, 0.0, 400, 1.0),
        "col" => (TableColumn, 0.0, 400, 1.0),
        "head" | "title" | "meta" | "link" | "script" | "style" | "base" | "noscript" => {
            (None, 0.0, 400, 1.0)
        }
        // Default for unknown/other elements is inline (per CSS).
        _ => (Inline, 0.0, 400, 1.0),
    };
    s.display = display;
    if weight != 400 {
        s.font_weight = weight;
    }
    if scale != 1.0 {
        s.font_size *= scale;
        s.line_height = s.font_size * 1.2;
    }
    if tag == "body" {
        s.margin = Sides::all(Dim::Px(8.0));
    } else if top_bottom_em != 0.0 {
        let m = Dim::Px(top_bottom_em * s.font_size);
        s.margin.top = m;
        s.margin.bottom = m;
    }
    if tag == "pre" {
        s.white_space = WhiteSpace::Pre;
    }
    if matches!(tag, "ul" | "ol") {
        s.padding.left = Dim::Px(40.0);
    }
}

/// Apply one declaration onto a computed style. Unknown properties/values are
/// silently ignored (CSS error recovery). `parent_fs` resolves `em`/`%` fonts.
fn apply_declaration(s: &mut ComputedStyle, d: &Declaration, parent_fs: f32) {
    let v = d.value.trim();
    match d.name.as_str() {
        "display" => {
            s.display = match v {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "table" | "inline-table" => Display::Table,
                "table-row-group" | "table-header-group" | "table-footer-group" => {
                    Display::TableRowGroup
                }
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-caption" => Display::TableCaption,
                "table-column" => Display::TableColumn,
                "table-column-group" => Display::TableColumnGroup,
                "none" => Display::None,
                _ => s.display,
            }
        }
        "color" => {
            if let Some(c) = values::parse_color(v) {
                s.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(c) = values::parse_color(v) {
                s.background_color = Some(c);
            }
        }
        "font-size" => {
            s.font_size = values::resolve_font_size(v, parent_fs).unwrap_or(s.font_size);
            s.line_height = s.font_size * 1.2;
        }
        "font-weight" => {
            s.font_weight = match v {
                "bold" | "bolder" => 700,
                "normal" => 400,
                "lighter" => 300,
                n => n.parse().unwrap_or(s.font_weight),
            }
        }
        "font-style" => s.italic = v == "italic" || v == "oblique",
        "line-height" => {
            if let Ok(n) = v.parse::<f32>() {
                s.line_height = n * s.font_size; // unitless multiplier
            } else if let Some(px) = values::parse_length_px(v, s.font_size) {
                s.line_height = px;
            } else if v == "normal" {
                s.line_height = s.font_size * 1.2;
            }
        }
        "text-align" => {
            s.text_align = match v {
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                "justify" => TextAlign::Justify,
                _ => TextAlign::Left,
            }
        }
        "white-space" => {
            s.white_space = match v {
                "nowrap" => WhiteSpace::NoWrap,
                "pre" | "pre-wrap" | "pre-line" => WhiteSpace::Pre,
                _ => WhiteSpace::Normal,
            }
        }
        "width" => s.width = values::parse_dim(v, s.font_size),
        "height" => s.height = values::parse_dim(v, s.font_size),
        "margin" => set_shorthand(&mut s.margin, v, s.font_size, true),
        "margin-top" => s.margin.top = values::parse_dim(v, s.font_size),
        "margin-right" => s.margin.right = values::parse_dim(v, s.font_size),
        "margin-bottom" => s.margin.bottom = values::parse_dim(v, s.font_size),
        "margin-left" => s.margin.left = values::parse_dim(v, s.font_size),
        "padding" => set_shorthand(&mut s.padding, v, s.font_size, false),
        "padding-top" => s.padding.top = values::parse_dim(v, s.font_size),
        "padding-right" => s.padding.right = values::parse_dim(v, s.font_size),
        "padding-bottom" => s.padding.bottom = values::parse_dim(v, s.font_size),
        "padding-left" => s.padding.left = values::parse_dim(v, s.font_size),
        "float" => {
            s.float = match v {
                "left" => Float::Left,
                "right" => Float::Right,
                _ => Float::None,
            }
        }
        "clear" => {
            s.clear = match v {
                "left" => Clear::Left,
                "right" => Clear::Right,
                "both" => Clear::Both,
                _ => Clear::None,
            }
        }
        "position" => {
            s.position = match v {
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                "sticky" => Position::Sticky,
                _ => Position::Static,
            }
        }
        "top" => s.inset.top = values::parse_dim(v, s.font_size),
        "right" => s.inset.right = values::parse_dim(v, s.font_size),
        "bottom" => s.inset.bottom = values::parse_dim(v, s.font_size),
        "left" => s.inset.left = values::parse_dim(v, s.font_size),
        "z-index" => s.z_index = if v == "auto" { None } else { v.parse().ok() },
        "table-layout" => {
            s.table_layout = match v {
                "fixed" => TableLayout::Fixed,
                _ => TableLayout::Auto,
            }
        }
        "border-spacing" => {
            // Only the first (horizontal) length is used in this slice.
            if let Some(px) = v
                .split_whitespace()
                .next()
                .and_then(|t| values::parse_length_px(t, s.font_size))
            {
                s.border_spacing = px;
            }
        }
        _ => {}
    }
}

/// Expand a 1–4 value `margin`/`padding` shorthand.
fn set_shorthand(sides: &mut Sides<Dim>, v: &str, fs: f32, allow_auto: bool) {
    let vals: Vec<Dim> = v
        .split_whitespace()
        .map(|t| {
            let d = values::parse_dim(t, fs);
            if !allow_auto && d.is_auto() {
                Dim::Px(0.0)
            } else {
                d
            }
        })
        .collect();
    match vals.as_slice() {
        [a] => *sides = Sides::all(*a),
        [a, b] => {
            *sides = Sides {
                top: *a,
                bottom: *a,
                right: *b,
                left: *b,
            }
        }
        [a, b, c] => {
            *sides = Sides {
                top: *a,
                right: *b,
                left: *b,
                bottom: *c,
            }
        }
        [a, b, c, d, ..] => {
            *sides = Sides {
                top: *a,
                right: *b,
                bottom: *c,
                left: *d,
            }
        }
        [] => {}
    }
}

#[cfg(feature = "stylo")]
pub mod stylo_engine;

#[cfg(test)]
mod tests {
    use super::*;

    fn build_dom() -> Dom {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.set_attr(p, "class", "lead");
        let span = dom.create_element("span");
        dom.set_attr(span, "id", "x");
        let t = dom.create_text("hi");
        dom.append_child(dom.root(), body);
        dom.append_child(body, p);
        dom.append_child(p, span);
        dom.append_child(span, t);
        dom
    }

    fn styled(css: &str) -> (Dom, StyleMap) {
        let dom = build_dom();
        let sheets = vec![Stylesheet::parse(css)];
        let map = MinimalCascade.cascade(&dom, &sheets);
        (dom, map)
    }

    #[test]
    fn ua_defaults_and_inheritance() {
        let (dom, map) = styled("");
        let p = dom.find_first("p").unwrap();
        assert_eq!(map[&p].display, Display::Block);
        assert_eq!(map[&p].color, Rgba::BLACK);
        // p default margins are 1em = 16px top/bottom.
        assert_eq!(map[&p].margin.top, Dim::Px(16.0));
    }

    #[test]
    fn author_rules_cascade_by_specificity() {
        let css = "p { color: red } .lead { color: green } #x { color: blue }";
        let (dom, map) = styled(css);
        let p = dom.find_first("p").unwrap();
        let span = dom.find_first("span").unwrap();
        // .lead (0,1,0) beats p (0,0,1).
        assert_eq!(map[&p].color, Rgba::new(0, 128, 0, 255));
        // #x id selector wins on the span.
        assert_eq!(map[&span].color, Rgba::new(0, 0, 255, 255));
    }

    #[test]
    fn descendant_combinator() {
        let css = "body span { color: red }";
        let (dom, map) = styled(css);
        let span = dom.find_first("span").unwrap();
        assert_eq!(map[&span].color, Rgba::new(255, 0, 0, 255));
    }

    #[test]
    fn float_clear_position_insets_parse() {
        let (dom, map) = styled(
            "p { float: right; clear: both; position: absolute; top: 10px; left: 5%; z-index: 3 }",
        );
        let p = dom.find_first("p").unwrap();
        let s = &map[&p];
        assert_eq!(s.float, Float::Right);
        assert_eq!(s.clear, Clear::Both);
        assert_eq!(s.position, Position::Absolute);
        assert_eq!(s.inset.top, Dim::Px(10.0));
        assert_eq!(s.inset.left, Dim::Percent(5.0));
        assert_eq!(s.inset.right, Dim::Auto); // unset stays auto
        assert_eq!(s.z_index, Some(3));
    }

    #[test]
    fn table_display_and_properties_parse() {
        let (dom, map) = styled("p { display: table; table-layout: fixed; border-spacing: 4px }");
        let p = dom.find_first("p").unwrap();
        let s = &map[&p];
        assert_eq!(s.display, Display::Table);
        assert_eq!(s.table_layout, TableLayout::Fixed);
        assert_eq!(s.border_spacing, 4.0);
    }

    #[test]
    fn table_ua_defaults() {
        // Build a tiny table DOM and confirm UA display defaults.
        let mut dom = Dom::new();
        let root = dom.root();
        let table = dom.create_element("table");
        let tr = dom.create_element("tr");
        let td = dom.create_element("td");
        let th = dom.create_element("th");
        dom.append_child(root, table);
        dom.append_child(table, tr);
        dom.append_child(tr, td);
        dom.append_child(tr, th);
        let map = MinimalCascade.cascade(&dom, &[]);
        assert_eq!(map[&table].display, Display::Table);
        assert_eq!(map[&tr].display, Display::TableRow);
        assert_eq!(map[&td].display, Display::TableCell);
        assert_eq!(map[&th].display, Display::TableCell);
        assert_eq!(map[&th].font_weight, 700, "th is bold by default");
    }

    #[test]
    fn inline_style_wins() {
        let mut dom = build_dom();
        let p = dom.find_first("p").unwrap();
        dom.set_attr(p, "style", "color: rgb(1,2,3); width: 50%");
        let map = MinimalCascade.cascade(&dom, &[Stylesheet::parse("p{color:red}")]);
        assert_eq!(map[&p].color, Rgba::new(1, 2, 3, 255));
        assert_eq!(map[&p].width, Dim::Percent(50.0));
    }
}
