//! Value parsing for the minimal cascade.
//!
//! Lengths/numbers go through `cssparser`'s tokenizer (the same one Stylo builds
//! on) so units, signs, and scientific notation are handled correctly. Colors are
//! parsed by hand for the common `#hex` / `rgb()` / named forms — the full CSS
//! Color grammar is Stylo's job.

use cssparser::{Parser, ParserInput, Token};

use crate::Dim;

/// 8-bit RGBA color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::new(0, 0, 0, 255);
    pub const WHITE: Rgba = Rgba::new(255, 255, 255, 255);
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }
}

/// Run `f` over the first token of `input`, if any.
fn with_first_token<T>(input: &str, f: impl FnOnce(&Token) -> Option<T>) -> Option<T> {
    let mut pi = ParserInput::new(input);
    let mut p = Parser::new(&mut pi);
    let tok = p.next().ok()?;
    f(tok)
}

/// Convert a dimension `value`+`unit` to px, resolving font-relative units against
/// `font_size`. Returns `None` for units outside the subset.
fn dimension_to_px(value: f32, unit: &str, font_size: f32) -> Option<f32> {
    match unit.to_ascii_lowercase().as_str() {
        "px" => Some(value),
        "em" | "rem" => Some(value * font_size),
        "pt" => Some(value * 96.0 / 72.0),
        "pc" => Some(value * 16.0),
        "in" => Some(value * 96.0),
        "cm" => Some(value * 96.0 / 2.54),
        "mm" => Some(value * 96.0 / 25.4),
        "q" => Some(value * 96.0 / 101.6),
        _ => None,
    }
}

/// Parse a `<length-percentage> | auto` into a [`Dim`]. Unitless `0` is accepted.
pub fn parse_dim(input: &str, font_size: f32) -> Dim {
    with_first_token(input, |tok| match tok {
        Token::Ident(id) if id.eq_ignore_ascii_case("auto") => Some(Dim::Auto),
        Token::Percentage { unit_value, .. } => Some(Dim::Percent(unit_value * 100.0)),
        Token::Dimension { value, unit, .. } => {
            dimension_to_px(*value, unit, font_size).map(Dim::Px)
        }
        Token::Number { value, .. } if *value == 0.0 => Some(Dim::Px(0.0)),
        _ => None,
    })
    .unwrap_or(Dim::Auto)
}

/// Parse a `<length>` to px (no percent/auto). Used for line-height etc.
pub fn parse_length_px(input: &str, font_size: f32) -> Option<f32> {
    with_first_token(input, |tok| match tok {
        Token::Dimension { value, unit, .. } => dimension_to_px(*value, unit, font_size),
        Token::Number { value, .. } if *value == 0.0 => Some(0.0),
        _ => None,
    })
}

/// Resolve a `font-size` value against the parent's font size (for `em`/`%`).
pub fn resolve_font_size(input: &str, parent_font_size: f32) -> Option<f32> {
    // Absolute-size keywords (approximate CSS scale).
    match input.to_ascii_lowercase().as_str() {
        "xx-small" => return Some(9.6),
        "x-small" => return Some(12.0),
        "small" => return Some(13.33),
        "medium" => return Some(16.0),
        "large" => return Some(18.0),
        "x-large" => return Some(24.0),
        "xx-large" => return Some(32.0),
        "smaller" => return Some(parent_font_size / 1.2),
        "larger" => return Some(parent_font_size * 1.2),
        _ => {}
    }
    with_first_token(input, |tok| match tok {
        Token::Percentage { unit_value, .. } => Some(parent_font_size * unit_value),
        Token::Dimension { value, unit, .. } => dimension_to_px(*value, unit, parent_font_size),
        _ => None,
    })
}

/// Parse a color: named, `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`, or `rgb()`/`rgba()`.
pub fn parse_color(input: &str) -> Option<Rgba> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("rgb") {
        return parse_rgb_func(&lower);
    }
    named_color(&lower)
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    let h = hex.as_bytes();
    let nib = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    match hex.len() {
        3 | 4 => {
            let mut v = [0u8; 4];
            v[3] = 15; // default alpha nibble
            for (i, &c) in h.iter().enumerate() {
                let n = nib(c)?;
                v[i] = n;
            }
            Some(Rgba::new(v[0] * 17, v[1] * 17, v[2] * 17, v[3] * 17))
        }
        6 | 8 => {
            let byte = |i: usize| -> Option<u8> { Some(nib(h[i])? * 16 + nib(h[i + 1])?) };
            let a = if hex.len() == 8 { byte(6)? } else { 255 };
            Some(Rgba::new(byte(0)?, byte(2)?, byte(4)?, a))
        }
        _ => None,
    }
}

fn parse_rgb_func(s: &str) -> Option<Rgba> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    let inner = &s[open + 1..close];
    // Accept comma- or space-separated; ignore an optional `/ alpha`.
    let inner = inner.replace('/', " ");
    let nums: Vec<&str> = inner
        .split([',', ' '])
        .filter(|t| !t.trim().is_empty())
        .collect();
    if nums.len() < 3 {
        return None;
    }
    let comp = |t: &str| -> Option<u8> {
        if let Some(pct) = t.strip_suffix('%') {
            let p: f32 = pct.trim().parse().ok()?;
            Some((p / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)
        } else {
            let n: f32 = t.trim().parse().ok()?;
            Some(n.round().clamp(0.0, 255.0) as u8)
        }
    };
    let r = comp(nums[0])?;
    let g = comp(nums[1])?;
    let b = comp(nums[2])?;
    let a = if nums.len() >= 4 {
        let av: f32 = nums[3].trim().trim_end_matches('%').parse().ok()?;
        let av = if nums[3].contains('%') { av / 100.0 } else { av };
        (av * 255.0).round().clamp(0.0, 255.0) as u8
    } else {
        255
    };
    Some(Rgba::new(r, g, b, a))
}

/// A compact subset of the CSS named colors — enough for common content.
fn named_color(name: &str) -> Option<Rgba> {
    let c = |r, g, b| Some(Rgba::new(r, g, b, 255));
    match name {
        "transparent" => Some(Rgba::TRANSPARENT),
        "black" => c(0, 0, 0),
        "white" => c(255, 255, 255),
        "red" => c(255, 0, 0),
        "green" => c(0, 128, 0),
        "blue" => c(0, 0, 255),
        "lime" => c(0, 255, 0),
        "yellow" => c(255, 255, 0),
        "cyan" | "aqua" => c(0, 255, 255),
        "magenta" | "fuchsia" => c(255, 0, 255),
        "gray" | "grey" => c(128, 128, 128),
        "silver" => c(192, 192, 192),
        "maroon" => c(128, 0, 0),
        "olive" => c(128, 128, 0),
        "teal" => c(0, 128, 128),
        "navy" => c(0, 0, 128),
        "purple" => c(128, 0, 128),
        "orange" => c(255, 165, 0),
        "pink" => c(255, 192, 203),
        "brown" => c(165, 42, 42),
        "gold" => c(255, 215, 0),
        "lightgray" | "lightgrey" => c(211, 211, 211),
        "darkgray" | "darkgrey" => c(169, 169, 169),
        "whitesmoke" => c(245, 245, 245),
        "lightblue" => c(173, 216, 230),
        "steelblue" => c(70, 130, 180),
        "dodgerblue" => c(30, 144, 255),
        "royalblue" => c(65, 105, 225),
        "rebeccapurple" => c(102, 51, 153),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(parse_color("#f00"), Some(Rgba::new(255, 0, 0, 255)));
        assert_eq!(parse_color("#00ff00"), Some(Rgba::new(0, 255, 0, 255)));
        assert_eq!(parse_color("#0000ff80").unwrap().a, 128);
        assert_eq!(parse_color("rgb(10, 20, 30)"), Some(Rgba::new(10, 20, 30, 255)));
        assert_eq!(parse_color("rgba(0,0,0,0.5)").unwrap().a, 128);
        assert_eq!(parse_color("rebeccapurple"), Some(Rgba::new(102, 51, 153, 255)));
        assert_eq!(parse_color("nonsense"), None);
    }

    #[test]
    fn lengths() {
        assert_eq!(parse_dim("12px", 16.0), Dim::Px(12.0));
        assert_eq!(parse_dim("2em", 16.0), Dim::Px(32.0));
        assert_eq!(parse_dim("50%", 16.0), Dim::Percent(50.0));
        assert_eq!(parse_dim("auto", 16.0), Dim::Auto);
        assert_eq!(parse_dim("0", 16.0), Dim::Px(0.0));
        assert_eq!(resolve_font_size("150%", 16.0), Some(24.0));
        assert_eq!(resolve_font_size("large", 16.0), Some(18.0));
    }
}
