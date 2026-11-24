//! PERF.1.2.b — the dirty-set planning shared by the incremental restyle of both cascades
//! (`MinimalCascade::cascade_incremental` and the Stylo one in `stylo_engine`). See the long note in
//! `stylo_engine.rs` for why the plan is computed from a fingerprint of the `Dom` and not from the
//! dirty bits.

use crate::Stylesheet;
use manuk_dom::{Dom, NodeData, NodeId};

pub(crate) fn env_on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && v != "0")
}

#[inline]
fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
    // a separator so ("ab","c") and ("a","bc") differ
    *h ^= 0xff;
    *h = h.wrapping_mul(0x100_0000_01b3);
}

/// What the cache is keyed on besides the sheet list: viewport and quirks mode.
pub(crate) fn incr_base_key(dom: &Dom, vw: f32, vh: f32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, &vw.to_bits().to_le_bytes());
    fnv(&mut h, &vh.to_bits().to_le_bytes());
    fnv(&mut h, &[dom.quirks() as u8]);
    h
}

/// Sibling-dependence of the author CSS, as a bit set (see [`incr_sheet_flags`]).
pub(crate) const SIB_STRUCT: u8 = 1; // :first-child :last-child :only-child — a neighbour appearing/vanishing
pub(crate) const SIB_LATER: u8 = 2; // `+` `~` — an element's later siblings
pub(crate) const SIB_HAS: u8 = 8; // `:has(` — an element's style reads its descendants / following siblings
pub(crate) const SIB_EARLIER: u8 = 4; // :nth-last-* :last-of-type — an element's earlier siblings

/// The cache facts [`incr_prepare`] needs from the previous pass.
pub(crate) struct CacheMeta<'a> {
    pub html: &'a [(String, String)],
    pub base: u64,
    pub sheets: &'a [Stylesheet],
    pub sib: u8,
    pub empty: bool,
}

pub(crate) struct Prep {
    /// `<html>`'s attributes this pass (the next pass's `CacheMeta::html`).
    pub html: Vec<(String, String)>,
    /// `<html>`'s class list changed, no rule that looks at the changed classes matches it, and the
    /// elements such rules do match are in `extra`: `<html>` itself keeps its style.
    pub ignore_html: bool,
    /// `<html>`'s non-class attributes are as they were: if its own style is unchanged too, it counts as clean
    /// (see [`incr_prepare`]); the caller checks that with `MinimalCascade::style_of_root_alone`.
    pub html_recheck: bool,
    /// The sheets that appeared or vanished since the previous pass (when `defer`red: the caller matches them).
    pub moved: Vec<Stylesheet>,
    pub base: u64,
    /// The sheet list this pass runs with (the next pass's `CacheMeta::sheets`).
    pub sheets: Vec<Stylesheet>,
    /// May the previous pass's per-node results be reused at all?
    pub usable: bool,
    pub sib: u8,
    pub empty: bool,
    /// Elements the sheets that appeared or vanished since the previous pass can match: they must be
    /// re-walked.
    pub extra: Vec<NodeId>,
}

/// At-rules whose effect is not "the elements the rules' selectors match": a sheet using one is never
/// layered onto the previous pass's styles. `@keyframes` is deliberately NOT here: a keyframes block only
/// changes the style of an element that names it with `animation-name`, and a pass with such an element
/// is never cached (it is `dynamic`), while a newly matched rule that names one is in `extra` and aborts
/// the incremental pass when walked. `@counter-style` only defines a name that computed `list-style-type` and
/// `counter()` carry as an identifier.
fn appended_unsafe(src: &str) -> bool {
    appended_unsafe_why(src).is_some()
}

fn appended_unsafe_why(src: &str) -> Option<&'static str> {
    let l = src.to_ascii_lowercase();
    [
        "@import",
        "@layer",
        "@scope",
        "@container",
        "@property",
        "@starting-style",
        "@namespace",
        "@position-try",
        "@view-transition",
        "@font-palette-values",
    ]
    .into_iter()
    .find(|t| l.contains(t))
}

/// Pair up the sheets two lists share, in order (longest common subsequence on the source hash; a sheet
/// with no hash never pairs). Returns (old indices not paired, new indices not paired).
fn sheet_diff(old: &[Stylesheet], new: &[Stylesheet]) -> (Vec<usize>, Vec<usize>) {
    let h = |s: &Stylesheet| s.source_hash();
    let (n, m) = (old.len(), new.len());
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if h(&old[i]).is_some() && h(&old[i]) == h(&new[j]) {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut gone, mut came) = (Vec::new(), Vec::new());
    while i < n && j < m {
        if h(&old[i]).is_some() && h(&old[i]) == h(&new[j]) {
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            gone.push(i);
            i += 1;
        } else {
            came.push(j);
            j += 1;
        }
    }
    gone.extend(i..n);
    came.extend(j..m);
    (gone, came)
}

fn html_node(dom: &Dom) -> Option<NodeId> {
    dom.children(dom.root()).find(|&c| dom.is_element(c))
}

fn html_attrs(dom: &Dom) -> Vec<(String, String)> {
    let Some(h) = dom.children(dom.root()).find(|&c| dom.is_element(c)) else {
        return Vec::new();
    };
    let mut v: Vec<(String, String)> = dom
        .element(h)
        .map(|e| {
            e.attrs
                .iter()
                .map(|a| (a.name.to_string(), a.value.to_string()))
                .collect()
        })
        .unwrap_or_default();
    // the pointer/focus state is part of what a selector on `<html>` can see
    let st = (dom.is_hovered(h) as u8)
        | (dom.is_focused(h) as u8) << 1
        | (dom.is_focus_within(h) as u8) << 2
        | (dom.is_focus_visible(h) as u8) << 3
        | (dom.is_active(h) as u8) << 4;
    v.push(("\0state".to_string(), st.to_string()));
    v
}

/// Attributes whose changes are tracked apart from the fingerprint (see `MinimalCascade::cascade_incremental`):
/// they are what scripts flip on `<html>`/`<body>` (`class`, `data-theme`, `aria-*`, inline `style`), and a
/// selector can see them only through a class/attribute test, which [`MinimalCascade::matches_mentioning`] finds.
pub(crate) fn is_tracked_attr(name: &str) -> bool {
    name == "class" || name == "style" || name.starts_with("data-") || name.starts_with("aria-")
}

/// Textual count of `[name` (attribute selector) in the sheets' source.
fn attr_text_count(sheets: &[Stylesheet], names: &[String]) -> usize {
    let mut n = 0;
    for sh in sheets {
        for t in names {
            n += memo_count(sh, 1, t, || {
                sh.source()
                    .to_ascii_lowercase()
                    .matches(&format!("[{}", t.to_ascii_lowercase()))
                    .count()
            });
        }
    }
    n
}

thread_local! {
    /// (sheet source hash, kind, name) -> textual count. Sheets are stable across passes; lower-casing and
    /// scanning a megabyte of CSS per pass for a few names was most of the planning cost.
    static TEXT_COUNTS: std::cell::RefCell<std::collections::HashMap<(u64, u8, String), usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn memo_count(sh: &Stylesheet, kind: u8, name: &str, f: impl FnOnce() -> usize) -> usize {
    let Some(h) = sh.source_hash() else {
        return f();
    };
    let key = (h, kind, name.to_string());
    if let Some(v) = TEXT_COUNTS.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let v = f();
    TEXT_COUNTS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 20_000 {
            m.clear();
        }
        m.insert(key, v);
    });
    v
}

/// Textual count of `.token` (class selector, whole identifier) in the sheets' source.
fn class_text_count(sheets: &[Stylesheet], tokens: &[String]) -> usize {
    let mut n = 0;
    for sh in sheets {
        let src = sh.source().to_ascii_lowercase();
        let b = src.as_bytes();
        for t in tokens {
            let needle = format!(".{}", t.to_ascii_lowercase());
            let mut from = 0;
            while let Some(i) = src[from..].find(&needle) {
                let end = from + i + needle.len();
                let ident = |c: u8| {
                    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'\\') || c >= 0x80
                };
                if !b.get(end).is_some_and(|&c| ident(c)) {
                    n += 1;
                }
                from = end;
            }
        }
    }
    n
}

/// Decide whether (and how) the previous pass can be reused: same viewport/quirks and a sheet list that
/// differs from the cached one only by sheets that appeared or vanished (a script adding a `<style>` /
/// `<link>`, a stylesheet swapped out). Only the elements those sheets' rules can match can change their
/// style, so they become `extra`. Sibling flags only ever grow; if the new sheets need more than the
/// cache has, the fingerprints' shape changed and the previous pass is not reusable.
fn incr_prepare_sheets(
    prev: Option<&CacheMeta>,
    old_class: Option<&str>,
    defer: bool,
    dom: &Dom,
    sheets: &[Stylesheet],
    vw: f32,
    vh: f32,
) -> Prep {
    let base = incr_base_key(dom, vw, vh);
    let mut extra = Vec::new();
    let html = html_attrs(dom);
    if let Some(p) = prev {
        let (gone, came) = sheet_diff(p.sheets, sheets);
        if env_on("MANUK_INCR_TRACE") {
            eprintln!(
                "INCR prep: cached sheets={} now={} gone={} came={} base_ok={}",
                p.sheets.len(),
                sheets.len(),
                gone.len(),
                came.len(),
                p.base == base
            );
        }
        if p.base == base {
            let moved: Vec<Stylesheet> = gone
                .iter()
                .map(|&i| p.sheets[i].clone())
                .chain(came.iter().map(|&j| sheets[j].clone()))
                .collect();
            if moved.is_empty() {
                return Prep {
                    html: html.clone(),
                    ignore_html: false,
                    html_recheck: false,
                    moved: Vec::new(),
                    base,
                    sheets: sheets.to_vec(),
                    usable: true,
                    sib: p.sib,
                    empty: p.empty,
                    extra,
                };
            }
            if env_on("MANUK_INCR_TRACE") {
                for s in &moved {
                    let (fs, fe) = incr_sheet_flags(std::slice::from_ref(s));
                    eprintln!(
                        "INCR prep: moved sheet len={} unsafe={:?} flags=({fs},{fe}) cache=({},{})",
                        s.source().len(),
                        appended_unsafe_why(s.source()),
                        p.sib,
                        p.empty
                    );
                }
            }
            if !moved.iter().any(|s| appended_unsafe(s.source())) {
                let (fs, fe) = incr_sheet_flags(&moved);
                if fs | p.sib == p.sib && (!fe || p.empty) {
                    if !defer {
                        extra = crate::matches_before_and_after(html_node(dom), old_class, || {
                            crate::MinimalCascade.appended_matches(dom, &moved)
                        });
                    }
                    return Prep {
                        html: html.clone(),
                        ignore_html: false,
                        html_recheck: false,
                        moved,
                        base,
                        sheets: sheets.to_vec(),
                        usable: true,
                        sib: p.sib,
                        empty: p.empty,
                        extra,
                    };
                }
            }
        }
    }
    let (sib, empty) = incr_sheet_flags(sheets);
    Prep {
        html,
        ignore_html: false,
        html_recheck: false,
        moved: Vec::new(),
        base,
        sheets: sheets.to_vec(),
        usable: false,
        sib,
        empty,
        extra,
    }
}

/// The previous passes are kept in [`INCR_SLOTS`] slots because the page's own callers disagree about the
/// sheet list (some rebuild it from inline `<style>` only), so consecutive passes can alternate between two
/// lists. Pick the slot whose list is closest to this pass's (None = nothing reusable).
pub(crate) const INCR_SLOTS: usize = 2;

pub(crate) fn incr_pick(
    slots: &[CacheMeta],
    dom: &Dom,
    sheets: &[Stylesheet],
    vw: f32,
    vh: f32,
) -> Option<usize> {
    let base = incr_base_key(dom, vw, vh);
    slots
        .iter()
        .enumerate()
        .filter(|(_, m)| m.base == base)
        .min_by_key(|(_, m)| {
            let (g, c) = sheet_diff(m.sheets, sheets);
            g.len() + c.len()
        })
        .map(|(i, _)| i)
}

/// Did the parser see every rule of `sheets` that names one of `tokens` as a class? (parsed mentions vs a
/// textual count: a rule this parser dropped would otherwise be invisible to the matcher.)
pub(crate) fn incr_class_rules_parsed(
    sheets: &[Stylesheet],
    tokens: &[String],
    attrs: &[String],
) -> bool {
    let none: &[String] = &[];
    // `[class]` selectors are counted by `class_mentions` whatever the tokens; compare like with like
    let parsed_classes = crate::MinimalCascade::class_mentions(sheets, tokens, none);
    let parsed_attrs = crate::MinimalCascade::class_mentions(sheets, none, attrs)
        - crate::MinimalCascade::class_mentions(sheets, none, none);
    let ok = parsed_classes >= class_text_count(sheets, tokens)
        && (attrs.is_empty() || parsed_attrs >= attr_text_count(sheets, attrs));
    if env_on("MANUK_INCR_TRACE") && !ok {
        eprintln!("INCR class/attr rules: parsed fewer than the text names ({tokens:?} {attrs:?})");
    }
    ok
}

/// [`incr_prepare_sheets`] plus the `<html>` class-list case: a script flipping classes on `<html>`
/// (`client-nojs` -> `client-js`, feature flags) moves `<html>`'s fingerprint, which used to force the full
/// cascade (the root font size feeds every `rem`). When ONLY the class list changed and no rule that looks at
/// a changed class matches `<html>` itself, its style is what it was, and the elements those rules do match
/// are the only ones that can have moved. The textual/parsed class-count check bails if the parser dropped a
/// rule that names such a class.
pub(crate) fn incr_prepare(
    prev: Option<CacheMeta>,
    defer: bool,
    dom: &Dom,
    sheets: &[Stylesheet],
    vw: f32,
    vh: f32,
) -> Prep {
    // `<html>`'s class list before this pass, when it differs from now: the rules the moved sheets and the
    // class tokens reach must be looked up under both.
    let old_class: Option<String> = prev.as_ref().and_then(|p| {
        let class_of = |attrs: &[(String, String)]| {
            attrs
                .iter()
                .find(|(n, _)| n == "class")
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let now = class_of(&html_attrs(dom));
        let was = class_of(p.html);
        (now != was).then_some(was)
    });
    let mut prep = incr_prepare_sheets(
        prev.as_ref(),
        old_class.as_deref(),
        defer,
        dom,
        sheets,
        vw,
        vh,
    );
    if defer {
        // the caller handles class changes and the moved sheets itself (the generalised path)
        return prep;
    }
    let Some(p) = prev else { return prep };
    if !prep.usable {
        return prep;
    }
    if p.html == prep.html.as_slice() {
        prep.html_recheck = true; // only the moved sheets can have touched `<html>`
        return prep;
    }
    let class_of = |attrs: &[(String, String)]| {
        attrs
            .iter()
            .find(|(n, _)| n == "class")
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let rest = |attrs: &[(String, String)]| -> Vec<(String, String)> {
        attrs
            .iter()
            .filter(|(n, _)| n != "class")
            .cloned()
            .collect()
    };
    if rest(p.html) != rest(&prep.html) {
        return prep;
    }
    prep.html_recheck = true;
    let toks = |c: &str| -> std::collections::HashSet<String> {
        c.split_ascii_whitespace().map(str::to_string).collect()
    };
    let (old, new) = (toks(&class_of(p.html)), toks(&class_of(&prep.html)));
    let mut tokens: Vec<String> = old.symmetric_difference(&new).cloned().collect();
    tokens.sort();
    if tokens.is_empty() {
        // same set, different spelling/order: nothing any selector can tell apart
        prep.ignore_html = true;
        return prep;
    }
    let parsed = crate::MinimalCascade::class_mentions(sheets, &tokens, &[]);
    if parsed < class_text_count(sheets, &tokens) {
        return prep;
    }
    let hits = crate::matches_before_and_after(html_node(dom), old_class.as_deref(), || {
        crate::MinimalCascade.matches_mentioning(dom, sheets, &tokens, &[], usize::MAX)
    });
    if env_on("MANUK_INCR_TRACE") {
        eprintln!(
            "INCR prep: <html> class tokens {:?} -> {} elements to re-walk",
            tokens.len(),
            hits.len()
        );
    }
    let de = dom.children(dom.root()).find(|&c| dom.is_element(c));
    // `<html>` matching one of those rules is not fatal: the caller re-styles it alone and keeps the
    // incremental pass if its style did not move (`html_recheck`).
    prep.ignore_html = !de.is_some_and(|d| hits.contains(&d));
    prep.extra.extend(hits);
    prep
}

/// Does the author CSS contain selectors whose match depends on a node's SIBLINGS (combinators `+` `~`
/// or structural pseudo-classes), and ones that depend on whether it has children (`:empty`)?
/// Textual and deliberately conservative: only the prelude of a rule (text that ends in `{`) is
/// searched for `+`/`~`, so `calc(1px + 2px)` in a declaration does not count.
pub(crate) fn incr_sheet_flags(sheets: &[Stylesheet]) -> (u8, bool) {
    let (mut sib, mut empty) = (0u8, false);
    for sh in sheets {
        let src = sh.source().to_ascii_lowercase();
        // `:nth-child`/`:nth-of-type`/`:first-of-type` count the siblings BEFORE the subject (so a change moves
        // the later ones); `:nth-last-*`/`:last-of-type` count the ones after it; `:only-of-type` both.
        if src.replace(":nth-last-", "").contains(":nth-") || src.contains(":first-of-type") {
            sib |= SIB_LATER;
        }
        if src.contains(":nth-last-") || src.contains(":last-of-type") {
            sib |= SIB_EARLIER;
        }
        if src.contains(":only-of-type") {
            sib |= SIB_LATER | SIB_EARLIER;
        }
        if src.contains(":first-child")
            || src.contains(":last-child")
            || src.contains(":only-child")
        {
            sib |= SIB_STRUCT;
        }
        if src.contains(":empty") {
            empty = true;
        }
        if src.contains(":has(") {
            sib |= SIB_HAS;
        }
        if sib & SIB_LATER == 0 {
            let b = src.as_bytes();
            let mut seg_start = 0usize;
            let mut in_comment = false;
            let mut k = 0usize;
            while k < b.len() {
                if in_comment {
                    if b[k] == b'*' && b.get(k + 1) == Some(&b'/') {
                        in_comment = false;
                        k += 1;
                        seg_start = k + 1;
                    }
                } else if b[k] == b'/' && b.get(k + 1) == Some(&b'*') {
                    in_comment = true;
                    k += 1;
                } else if matches!(b[k], b';' | b'}') {
                    seg_start = k + 1;
                } else if b[k] == b'{' {
                    let prelude = &b[seg_start..k];
                    let at = prelude.iter().find(|c| !c.is_ascii_whitespace()) == Some(&b'@');
                    if !at {
                        for (q, &c) in prelude.iter().enumerate() {
                            if (c == b'+' || c == b'~') && prelude.get(q + 1) != Some(&b'=') {
                                sib |= SIB_LATER;
                            }
                        }
                    }
                    seg_start = k + 1;
                }
                k += 1;
            }
        }
    }
    (sib, empty)
}

/// Every element's tracked attributes, in attribute order (the fingerprints of a pass that tracks them
/// separately omit them).
pub(crate) type Tracked = std::collections::HashMap<NodeId, Vec<(String, String)>>;

pub(crate) fn incr_tracked(dom: &Dom) -> Tracked {
    let mut out = Tracked::new();
    for n in dom.flat_descendants(dom.root()) {
        if let Some(el) = dom.element(n) {
            let v: Vec<(String, String)> = el
                .attrs
                .iter()
                .filter(|a| is_tracked_attr(&a.name))
                .map(|a| (a.name.to_string(), a.value.to_string()))
                .collect();
            if !v.is_empty() {
                out.insert(n, v);
            }
        }
    }
    out
}

pub(crate) fn incr_fingerprints(
    dom: &Dom,
    sib: u8,
    empty: bool,
    with_tracked: bool,
) -> Vec<(NodeId, u64)> {
    let nodes = dom.flat_descendants(dom.root());
    let mut out = Vec::with_capacity(nodes.len());
    for n in nodes {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        fnv(&mut h, &dom.parent(n).map_or(0, |p| p.0).to_le_bytes());
        match dom.data(n) {
            NodeData::Element(el) => {
                fnv(&mut h, b"E");
                fnv(&mut h, el.name.as_bytes());
                if let Some(ns) = &el.namespace {
                    fnv(&mut h, ns.as_bytes());
                }
                for a in &el.attrs {
                    if !with_tracked && is_tracked_attr(&a.name) {
                        continue;
                    }
                    fnv(&mut h, a.name.as_bytes());
                    fnv(&mut h, a.value.as_bytes());
                }
                let st = (dom.is_hovered(n) as u8)
                    | (dom.is_focused(n) as u8) << 1
                    | (dom.is_focus_within(n) as u8) << 2
                    | (dom.is_focus_visible(n) as u8) << 3
                    | (dom.is_active(n) as u8) << 4;
                fnv(&mut h, &[st]);
                if empty || sib & SIB_HAS != 0 {
                    fnv(&mut h, &[dom.first_child(n).is_some() as u8]);
                }
            }
            NodeData::Text(t) => {
                fnv(&mut h, b"T");
                if empty {
                    fnv(&mut h, &[!t.trim().is_empty() as u8]);
                }
            }
            _ => fnv(&mut h, b"O"),
        }
        // The document element's siblings are the doctype and comments: no selector can see them.
        if sib != 0 && dom.parent(n) != Some(dom.root()) {
            fnv(
                &mut h,
                &dom.prev_sibling(n).map_or(0, |p| p.0).to_le_bytes(),
            );
            fnv(
                &mut h,
                &dom.next_sibling(n).map_or(0, |p| p.0).to_le_bytes(),
            );
        }
        out.push((n, h));
    }
    out
}

/// The elements whose style can depend on their position among their siblings: those a "position-sensitive"
/// compound of some selector can match, loosely (tag / id / classes only, ignoring its pseudos and the rest
/// of the selector — a superset). A compound is position-sensitive when it is the right side of `+`/`~`, or
/// carries `:nth-*`/`:first/last/only-*` (possibly inside `:not()`/`:is()`). `all` = a bare compound
/// (`*`, `:nth-child(odd)`) is position-sensitive, so every element is a candidate.
#[derive(Default)]
pub(crate) struct SibKeys {
    all: bool,
    loose: Vec<(Option<String>, Option<String>, Vec<String>)>,
    /// compounds carrying a `:has(...)`: the elements whose style reads what is below/after them
    has_all: bool,
    has_loose: Vec<(Option<String>, Option<String>, Vec<String>)>,
}

impl SibKeys {
    /// Memoised on the sheets' source hashes (rebuilt only when the list changes).
    pub(crate) fn of(sheets: &[Stylesheet]) -> std::rc::Rc<SibKeys> {
        thread_local! {
            static MEMO: std::cell::RefCell<Option<(Vec<u64>, std::rc::Rc<SibKeys>)>> =
                const { std::cell::RefCell::new(None) };
        }
        let hashes: Option<Vec<u64>> = sheets.iter().map(|s| s.source_hash()).collect();
        if let Some(h) = &hashes {
            if let Some(hit) = MEMO.with(|m| {
                m.borrow()
                    .as_ref()
                    .filter(|(k, _)| k == h)
                    .map(|(_, v)| v.clone())
            }) {
                return hit;
            }
        }
        let keys = std::rc::Rc::new(Self::compute(sheets));
        if let Some(h) = hashes {
            MEMO.with(|m| *m.borrow_mut() = Some((h, keys.clone())));
        }
        keys
    }

    fn compute(sheets: &[Stylesheet]) -> SibKeys {
        use crate::{Combinator, Pseudo, Selector};
        fn position_pseudo(p: &Pseudo) -> bool {
            match p {
                Pseudo::FirstChild
                | Pseudo::LastChild
                | Pseudo::OnlyChild
                | Pseudo::NthChild(..)
                | Pseudo::NthLastChild(..)
                | Pseudo::FirstOfType
                | Pseudo::LastOfType
                | Pseudo::OnlyOfType
                | Pseudo::NthOfType(..)
                | Pseudo::NthLastOfType(..) => true,
                Pseudo::Not(v) | Pseudo::Is(v) => v.iter().any(nested_sensitive),
                _ => false,
            }
        }
        fn nested_sensitive(sel: &Selector) -> bool {
            sel.parts.iter().enumerate().any(|(i, part)| {
                (i > 0
                    && matches!(
                        sel.combinators.get(i - 1),
                        Some(Combinator::NextSibling | Combinator::SubsequentSibling)
                    ))
                    || part.pseudos.iter().any(position_pseudo)
            })
        }
        fn has_pseudo(p: &Pseudo) -> bool {
            match p {
                Pseudo::Has(_) => true,
                Pseudo::Not(v) | Pseudo::Is(v) => v
                    .iter()
                    .any(|s| s.parts.iter().any(|c| c.pseudos.iter().any(has_pseudo))),
                _ => false,
            }
        }
        let mut k = SibKeys::default();
        let mut seen = std::collections::HashSet::new();
        for sh in sheets {
            for sel in sh.rules().iter().flat_map(|r| r.selectors.iter()) {
                for part in &sel.parts {
                    if part.pseudos.iter().any(has_pseudo) {
                        if part.tag.is_none() && part.id.is_none() && part.classes.is_empty() {
                            k.has_all = true;
                        } else {
                            k.has_loose.push((
                                part.tag.as_ref().map(|t| t.to_ascii_lowercase()),
                                part.id.clone(),
                                part.classes.clone(),
                            ));
                        }
                    }
                }
                for (i, part) in sel.parts.iter().enumerate() {
                    let sensitive = (i > 0
                        && matches!(
                            sel.combinators.get(i - 1),
                            Some(Combinator::NextSibling | Combinator::SubsequentSibling)
                        ))
                        || part.pseudos.iter().any(position_pseudo);
                    if !sensitive {
                        continue;
                    }
                    if part.tag.is_none() && part.id.is_none() && part.classes.is_empty() {
                        k.all = true;
                        continue;
                    }
                    let key = (
                        part.tag.as_ref().map(|t| t.to_ascii_lowercase()),
                        part.id.clone(),
                        part.classes.clone(),
                    );
                    if seen.insert(key.clone()) {
                        k.loose.push(key);
                    }
                }
            }
        }
        k
    }

    fn has_candidate(&self, dom: &Dom, n: NodeId) -> bool {
        let Some(el) = dom.element(n) else {
            return false;
        };
        self.has_all
            || self.has_loose.iter().any(|(tag, id, classes)| {
                tag.as_ref().is_none_or(|t| el.name.eq_ignore_ascii_case(t))
                    && id.as_ref().is_none_or(|i| el.id() == Some(i.as_str()))
                    && classes.iter().all(|c| el.has_class(c))
            })
    }

    fn candidate(&self, dom: &Dom, n: NodeId) -> bool {
        let Some(el) = dom.element(n) else {
            return false;
        };
        self.all
            || self.loose.iter().any(|(tag, id, classes)| {
                tag.as_ref().is_none_or(|t| el.name.eq_ignore_ascii_case(t))
                    && id.as_ref().is_none_or(|i| el.id() == Some(i.as_str()))
                    && classes.iter().all(|c| el.has_class(c))
            })
    }
}

/// `:has()`: a moved node changes what an ancestor (or an earlier sibling, for `:has(+ x)` / `:has(~ x)`)
/// that carries a `:has` compound matches. Those elements are not re-walked as subtrees: the caller derives
/// their style alone and re-walks them only if it moved. (The Stylo path re-runs `:has` over the whole map.)
pub(crate) fn incr_has_checks(
    dom: &Dom,
    cache_fp: &std::collections::HashMap<NodeId, u64>,
    new_fp: &[(NodeId, u64)],
    sib: u8,
    sheets: &[Stylesheet],
) -> Vec<NodeId> {
    if sib & SIB_HAS == 0 {
        return Vec::new();
    }
    let keys = SibKeys::of(sheets);
    let mut out: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for (c, h) in new_fp {
        if cache_fp.get(c) == Some(h) {
            continue;
        }
        let mut x = Some(*c);
        while let Some(n) = x {
            let mut sib_cur = dom.prev_sibling(n);
            while let Some(sb) = sib_cur {
                if keys.has_candidate(dom, sb) {
                    out.insert(sb);
                }
                sib_cur = dom.prev_sibling(sb);
            }
            x = dom.parent(n);
            if let Some(a) = x {
                if keys.has_candidate(dom, a) {
                    out.insert(a);
                }
            }
        }
    }
    out.into_iter().collect()
}

/// The subtree roots to re-walk and the nodes that left the document, or `None` = do the full cascade.
pub(crate) fn incr_plan(
    cache_fp: &std::collections::HashMap<NodeId, u64>,
    cache_sib: u8,
    dom: &Dom,
    new_fp: &[(NodeId, u64)],
    extra: &[NodeId],
    ignore_html: bool,
    sheets: &[Stylesheet],
) -> Option<(Vec<NodeId>, Vec<NodeId>)> {
    let mut changed: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for (n, h) in new_fp {
        if cache_fp.get(n) != Some(h) {
            changed.insert(*n);
        }
    }
    let live: std::collections::HashSet<NodeId> = new_fp.iter().map(|(n, _)| *n).collect();
    let removed: Vec<NodeId> = cache_fp
        .keys()
        .copied()
        .filter(|n| !live.contains(n))
        .collect();
    // A moved node can change what a sibling matches — but only on the side the sheets' structural
    // selectors look at: `+`/`~`/`:nth-child` read the siblings BEFORE the subject (so the LATER ones
    // move), `:nth-last-*` the ones after it (EARLIER ones move). `:first/:last/:only-child` need nothing
    // beyond the neighbours, whose fingerprints (which hold their prev/next ids) already moved.
    if cache_sib & (SIB_LATER | SIB_EARLIER) != 0 {
        let keys = SibKeys::of(sheets);
        let snapshot = changed.clone();
        let parents: std::collections::HashSet<NodeId> =
            snapshot.iter().filter_map(|&c| dom.parent(c)).collect();
        for p in parents {
            let kids: Vec<NodeId> = dom.children(p).collect();
            let first = kids.iter().position(|k| snapshot.contains(k));
            let last = kids.iter().rposition(|k| snapshot.contains(k));
            let (Some(first), Some(last)) = (first, last) else {
                continue;
            };
            let from = if cache_sib & SIB_EARLIER != 0 {
                0
            } else {
                first
            };
            let to = if cache_sib & SIB_LATER != 0 {
                kids.len()
            } else {
                last + 1
            };
            changed.extend(
                kids[from..to]
                    .iter()
                    .copied()
                    .filter(|&k| snapshot.contains(&k) || keys.candidate(dom, k)),
            );
        }
    }
    changed.extend(extra.iter().copied());
    if ignore_html {
        if let Some(de) = dom.children(dom.root()).find(|&c| dom.is_element(c)) {
            if !extra.contains(&de) {
                changed.remove(&de);
            }
        }
    }
    if changed.len() * 3 > new_fp.len().max(3) {
        if env_on("MANUK_INCR_TRACE") {
            eprintln!(
                "INCR plan: full, {} nodes after expansion of {}",
                changed.len(),
                new_fp.len()
            );
        }
        return None;
    }
    // `<html>` itself moved -> the root font size (every `rem`) may have moved with it.
    if let Some(de) = dom.children(dom.root()).find(|&c| dom.is_element(c)) {
        if changed.contains(&de) {
            if env_on("MANUK_INCR_TRACE") {
                eprintln!(
                    "INCR plan: full, <html> changed: attrs={:?}",
                    dom.element(de).map(|e| e
                        .attrs
                        .iter()
                        .map(|a| format!(
                            "{}={}",
                            a.name,
                            a.value.chars().take(24).collect::<String>()
                        ))
                        .collect::<Vec<_>>())
                );
            }
            return None;
        }
    }
    let mut roots = Vec::new();
    for &c in &changed {
        let mut cur = dom.parent(c);
        let mut covered = false;
        while let Some(p) = cur {
            if changed.contains(&p) {
                covered = true;
                break;
            }
            cur = dom.parent(p);
        }
        if !covered {
            roots.push(c);
        }
    }
    Some((roots, removed))
}

/// The counters are process-global, so the tests that diff them take turns.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) static INCR_PASSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_FULL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_RESTYLED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_MISMATCH: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// `(incremental passes, full passes, nodes restyled by incremental passes, verify mismatches)`.
pub fn incr_stats() -> (u64, u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        INCR_PASSES.load(Relaxed),
        INCR_FULL.load(Relaxed),
        INCR_RESTYLED.load(Relaxed),
        INCR_MISMATCH.load(Relaxed),
    )
}

/// The incremental MinimalCascade (the agent's cascade) must give, after every kind of script mutation,
/// exactly the style map the full cascade gives; the full arm is a fresh thread (the cache is
/// thread-local, so an empty cache can only do the full walk).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MinimalCascade, StyleMap};

    const HTML: &str = "<html><head><style>\
        .a .b { color: red } .open > p { margin-left: 7px } li + li { padding-top: 3px }\
        li:nth-child(odd) { font-weight: 700 } li:last-child { color: blue } p:empty { display: none }\
        div { opacity: .5 }\
        </style></head><body>\
        <div id=m class=a><span class=b>x</span><span id=s>y</span></div>\
        <ul id=l><li>1</li><li>2</li><li>3</li></ul>\
        <div id=o><p>t</p><p></p></div></body></html>";

    fn by_id(dom: &Dom, id: &str) -> NodeId {
        dom.descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.id()) == Some(id))
            .unwrap()
    }

    fn full_fresh(dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        std::thread::scope(|s| {
            s.spawn(|| MinimalCascade.cascade_incremental(dom, sheets, 800.0, 600.0))
                .join()
                .unwrap()
        })
    }

    /// Returns the number of nodes the incremental pass restyled.
    fn check(dom: &Dom, sheets: &[Stylesheet], what: &str) -> u64 {
        let before = incr_stats();
        let inc = MinimalCascade.cascade_incremental(dom, sheets, 800.0, 600.0);
        let after = incr_stats();
        let full = full_fresh(dom, sheets);
        assert_eq!(inc.len(), full.len(), "{what}: node count");
        for (n, cs) in &full {
            assert!(
                inc.get(n) == Some(cs),
                "{what}: node {:?} <{}> differs from the full cascade",
                n,
                dom.tag_name(*n).unwrap_or("#text")
            );
        }
        assert_eq!(after.0, before.0 + 1, "{what}: pass was not incremental");
        after.2 - before.2
    }

    #[test]
    fn incremental_minimal_cascade_equals_full_after_mutations() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(HTML);
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0); // full, primes
        let total = dom.flat_descendants(dom.root()).len() as u64;

        let m = by_id(&dom, "m");
        dom.set_attr(m, "class", "");
        check(&dom, &sheets, "class removed (descendant selector)");
        dom.set_attr(m, "class", "a");
        check(&dom, &sheets, "class restored");
        let o = by_id(&dom, "o");
        dom.set_attr(o, "class", "open");
        check(&dom, &sheets, "class added (child combinator)");
        let l = by_id(&dom, "l");
        let li = dom.create_element("li");
        let t = dom.create_text("4");
        dom.append_child(li, t);
        dom.append_child(l, li);
        check(&dom, &sheets, "li appended (sibling + nth + last-child)");
        let first = dom.first_child(l).unwrap();
        dom.detach(first);
        check(&dom, &sheets, "first li removed (nth-child parity flips)");
        let p2 = dom.children(o).nth(1).unwrap();
        let tx = dom.create_text("now not empty");
        dom.append_child(p2, tx);
        check(&dom, &sheets, "text added to an empty p (:empty)");
        let s = by_id(&dom, "s");
        dom.set_attr(s, "style", "color: green; opacity: .25");
        let walked = check(&dom, &sheets, "inline style on a leaf");
        assert!(
            walked * 2 < total,
            "a one-leaf change restyled {walked} of {total} nodes"
        );
    }

    const HTML2: &str = "<html><head><style>.x ~ i { color: red } b { margin-top: 2px }</style></head><body>\
        <div id=c><u>1</u><u id=x>2</u><i>3</i><i>4</i><i>5</i></div><p id=q class=k>z</p></body></html>";

    #[test]
    fn later_sibling_expansion_and_appended_sheet_equal_full() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(HTML2);
        let mut sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        // `~`: an earlier sibling gaining the class restyles every later one
        let x = by_id(&dom, "x");
        dom.set_attr(x, "class", "x");
        check(&dom, &sheets, "class on an earlier sibling (~)");
        let u1 = dom.first_child(by_id(&dom, "c")).unwrap();
        dom.set_attr(u1, "class", "x");
        check(&dom, &sheets, "class on the first sibling (~)");
        // a sheet appended after the cached ones: only what its rules match is re-walked
        let extra = Stylesheet::parse(".k { color: green } i { opacity: .3 }");
        sheets.push(extra);
        let before = incr_stats();
        let inc = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let after = incr_stats();
        let full = full_fresh(&dom, &sheets);
        assert!(inc == full, "appended sheet: differs from the full cascade");
        assert_eq!(
            after.0,
            before.0 + 1,
            "appended sheet: pass was not incremental"
        );
        // a sheet inserted in the middle of the list, then the first one removed
        sheets.insert(1, Stylesheet::parse("u { opacity: .5 } #q { color: blue }"));
        check(&dom, &sheets, "sheet inserted mid-list");
        sheets.remove(0);
        check(&dom, &sheets, "first sheet removed");
    }

    #[test]
    fn html_class_flip_restyles_only_what_the_classes_reach() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(
            "<html class=nojs><head><style>.js .k { color: red } .nojs .k { color: blue } \
             .z:not(.js) { opacity: .4 } .nojs .q { color: green } p { margin-top: 1px }</style></head><body>\
             <p class=k>a</p><p id=w class=z>b</p><p class=q>c</p><p>d</p><p>e</p><p>f</p><p>g</p>\
             <p>h</p><p>i</p><p>j</p></body></html>",
        );
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let html = dom
            .children(dom.root())
            .find(|&c| dom.is_element(c))
            .unwrap();
        dom.set_attr(html, "class", "js");
        let walked = check(&dom, &sheets, "html class nojs -> js");
        assert!(walked <= 10, "html class flip re-walked {walked} nodes");
        dom.set_attr(html, "class", "js other");
        check(&dom, &sheets, "html gains a class no rule names");
    }

    #[test]
    fn html_class_flip_that_changes_html_style_is_still_exact() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(
            "<html class=nojs><head><style>.js { color: red; font-size: 20px } .nojs { color: blue } \
             .js .k { opacity: .5 }</style></head><body><p class=k>a</p><p>b</p><p>c</p><p>d</p>\
             <p>e</p><p>f</p><p>g</p><p>h</p></body></html>",
        );
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let html = dom
            .children(dom.root())
            .find(|&c| dom.is_element(c))
            .unwrap();
        dom.set_attr(html, "class", "js");
        let inc = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let full = full_fresh(&dom, &sheets);
        assert!(
            inc == full,
            "html style changed: the pass must not reuse the children"
        );
    }

    #[test]
    fn has_selector_follows_a_descendant_change() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(
            "<html><head><style>.w:has(.x) { color: red } .w:has(> i) { opacity: .5 }</style></head><body>\
             <div class=w id=w><span id=s>a</span></div><p>1</p><p>2</p><p>3</p><p>4</p><p>5</p><p>6</p><p>7</p></body></html>",
        );
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let s = by_id(&dom, "s");
        dom.set_attr(s, "class", "x");
        let inc = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        assert!(
            inc == full_fresh(&dom, &sheets),
            ":has(.x) after a class flip"
        );
        let w = by_id(&dom, "w");
        let i = dom.create_element("i");
        dom.append_child(w, i);
        let inc = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        assert!(
            inc == full_fresh(&dom, &sheets),
            ":has(> i) after an insert"
        );
    }

    #[test]
    fn data_attribute_and_style_flips_on_html_are_exact_and_cheap() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut dom = manuk_html::parse(
            "<html data-theme=light><head><style>[data-theme=dark] .k { color: white } \
             [data-theme=light] .k { color: black } [data-theme=light] .m { opacity: .8 } p { margin-top: 1px }</style></head><body>\
             <p class=k>a</p><p class=m>b</p><p>c</p><p>d</p><p>e</p><p>f</p><p>g</p><p>h</p><p>i</p><p>j</p></body></html>",
        );
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        let html = dom
            .children(dom.root())
            .find(|&c| dom.is_element(c))
            .unwrap();
        dom.set_attr(html, "data-theme", "dark");
        let walked = check(&dom, &sheets, "data-theme light -> dark");
        assert!(walked <= 10, "data-theme flip re-walked {walked} nodes");
        // the style attribute on <html> moves its own style and what inherits it: still exact
        dom.set_attr(html, "style", "color: red");
        let inc = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0);
        assert!(inc == full_fresh(&dom, &sheets), "inline style on <html>");
    }
}
