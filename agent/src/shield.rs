//! Injection shield: text a human cannot see, computed from the **laid-out fragment tree** (the
//! same geometry and colours the painter uses), not from the DOM or the source.
//!
//! A page can hide instructions for an agent in ways a reader never perceives — `opacity:0`,
//! `visibility:hidden`, a box pushed off-canvas, a zero-size or 1px-font box, text the colour of
//! its own background. `display:none` is already gone (no fragment). This walks the fragments and
//! sorts them into *seen* and *withheld*; callers show the seen text and a `⚠hidden-text` marker
//! counting what was withheld. Everything here is a heuristic over paint truth, so it is
//! deliberately conservative about what counts as "invisible" (see the thresholds).

use std::collections::{BTreeMap, HashMap, HashSet};

use manuk_css::Rgba;
use manuk_dom::NodeId;
use manuk_layout::{BoxContent, LayoutBox, TextFragment};

/// Contrast ratio (WCAG relative luminance) below which text counts as camouflaged. WCAG's own
/// failing line is 3.0; 1.5 is "a reader cannot tell it is there", so ordinary muted grey text
/// (about 2-3) is kept.
pub const MIN_CONTRAST: f32 = 1.5;
/// Fonts at or below this many px are not legible to a human.
pub const MIN_FONT_PX: f32 = 2.0;

#[derive(Debug, Clone, Default)]
pub struct Shield {
    /// The page text with every withheld fragment removed (same joining rule as `visible_text`).
    pub text: String,
    /// `(reason, text)` for each withheld fragment, in document order.
    pub withheld: Vec<(&'static str, String)>,
    /// Elements all of whose text is withheld, with the reason — the a11y tree drops these.
    pub nodes: HashMap<NodeId, &'static str>,
}

impl Shield {
    /// `⚠hidden-text: 3 withheld (offscreen 1, opacity 2) …`, or `None` when nothing was withheld.
    pub fn marker(&self) -> Option<String> {
        if self.withheld.is_empty() {
            return None;
        }
        let mut by: BTreeMap<&str, usize> = BTreeMap::new();
        for (r, _) in &self.withheld {
            *by.entry(r).or_default() += 1;
        }
        let parts: Vec<String> = by.iter().map(|(r, n)| format!("{r} {n}")).collect();
        Some(format!(
            "⚠hidden-text: {} text fragment(s) a human cannot see were withheld ({}); pass include_hidden=true to read them",
            self.withheld.len(),
            parts.join(", ")
        ))
    }
}

fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Rgba) -> f32 {
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio, 1.0 (identical) to 21.0.
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[derive(Clone, Copy)]
struct Ctx {
    /// Why this whole subtree is invisible, when it is.
    box_reason: Option<&'static str>,
    /// Nearest opaque-ish background up the chain.
    bg: Rgba,
}

fn classify(f: &TextFragment, ctx: Ctx, viewport_w: f32) -> Option<&'static str> {
    if let Some(r) = ctx.box_reason {
        return Some(r);
    }
    if f.style.font_size <= MIN_FONT_PX {
        return Some("tiny-font");
    }
    if f.x + f.width <= 0.0 || f.line_top + f.style.line_height <= 0.0 || f.x >= viewport_w {
        return Some("offscreen");
    }
    if f.style.color.a == 0 {
        return Some("transparent");
    }
    if contrast(f.style.color, ctx.bg) < MIN_CONTRAST {
        return Some("low-contrast");
    }
    None
}

struct Walk {
    viewport_w: f32,
    seen: Vec<TextFragment>,
    out: Vec<(&'static str, TextFragment)>,
    hidden_nodes: HashMap<NodeId, &'static str>,
    seen_nodes: HashSet<NodeId>,
}

fn walk(b: &LayoutBox, mut ctx: Ctx, w: &mut Walk) {
    if ctx.box_reason.is_none() {
        if b.opacity <= 0.01 {
            ctx.box_reason = Some("opacity");
        } else if b.hidden {
            ctx.box_reason = Some("visibility");
        } else if (b.rect.width <= 1.0 || b.rect.height <= 1.0)
            && !matches!(&b.content, BoxContent::Inline(f) if f.is_empty())
        {
            ctx.box_reason = Some("zero-size");
        }
    }
    if let Some(bg) = b.background {
        if bg.a >= 200 {
            ctx.bg = bg;
        }
    }
    match &b.content {
        BoxContent::Inline(frags) => {
            for f in frags {
                if f.text.trim().is_empty() {
                    // Spacing still participates in the join below, so keep it with the seen runs.
                    if ctx.box_reason.is_none() {
                        w.seen.push(f.clone());
                    }
                    continue;
                }
                match classify(f, ctx, w.viewport_w) {
                    Some(r) => {
                        if let Some(n) = f.node.or(b.node) {
                            w.hidden_nodes.entry(n).or_insert(r);
                        }
                        w.out.push((r, f.clone()));
                    }
                    None => {
                        if let Some(n) = f.node.or(b.node) {
                            w.seen_nodes.insert(n);
                        }
                        w.seen.push(f.clone());
                    }
                }
            }
        }
        BoxContent::Block(children) => {
            for c in children {
                walk(c, ctx, w);
            }
        }
    }
}

/// Sort every text run under `root` into seen/withheld. `viewport_w` is the layout width (text
/// starting right of it is off-canvas); the canvas is taken as white.
pub fn analyze(root: &LayoutBox, viewport_w: f32) -> Shield {
    let mut w = Walk {
        viewport_w,
        seen: Vec::new(),
        out: Vec::new(),
        hidden_nodes: HashMap::new(),
        seen_nodes: HashSet::new(),
    };
    let white = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    walk(
        root,
        Ctx {
            box_reason: None,
            bg: white,
        },
        &mut w,
    );

    let mut text = String::new();
    let mut prev: Option<&TextFragment> = None;
    for f in &w.seen {
        if f.text.is_empty() {
            continue;
        }
        if prev.is_some_and(|p| !f.continues(p)) {
            text.push(' ');
        }
        text.push_str(&f.text);
        prev = Some(f);
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let nodes = w
        .hidden_nodes
        .into_iter()
        .filter(|(n, _)| !w.seen_nodes.contains(n))
        .collect();
    Shield {
        text,
        withheld: w.out.into_iter().map(|(r, f)| (r, f.text)).collect(),
        nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_black_white_is_21_and_same_is_1() {
        let k = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        let w = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        assert!((contrast(k, w) - 21.0).abs() < 0.01);
        assert!((contrast(w, w) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn near_white_on_white_is_camouflage_grey_is_not() {
        let w = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let near = Rgba {
            r: 250,
            g: 250,
            b: 250,
            a: 255,
        };
        let grey = Rgba {
            r: 119,
            g: 119,
            b: 119,
            a: 255,
        };
        assert!(contrast(near, w) < MIN_CONTRAST);
        assert!(contrast(grey, w) >= MIN_CONTRAST);
    }
}
