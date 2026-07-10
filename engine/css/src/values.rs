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

/// Parse a `<length-percentage> | auto` into a [`Dim`]. Unitless `0` is accepted, and the
/// additive form of `calc()` (`calc(<len/%> ± <len/%> …)`) is reduced to [`Dim::Calc`].
pub fn parse_dim(input: &str, font_size: f32) -> Dim {
    let t = input.trim();
    if t.len() >= 5 && t[..5].eq_ignore_ascii_case("calc(") {
        if let Some(d) = parse_calc(t, font_size) {
            return d;
        }
    }
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

/// Evaluate a `calc()` expression: numbers, lengths, and percentages combined with
/// `+ - * /`, parentheses, and nested `calc()`. The result is the linear form
/// `px + pct% of the reference` (CSS forbids the non-linear combinations, e.g. `%*%`).
fn parse_calc(input: &str, font_size: f32) -> Option<Dim> {
    let low = input.trim().to_ascii_lowercase();
    let inner = low.strip_prefix("calc(")?;
    let inner = inner.strip_suffix(')')?;
    let toks = tokenize_calc(inner, font_size)?;
    let mut p = CalcParser { toks: &toks, i: 0 };
    let v = p.expr()?;
    if p.i != p.toks.len() {
        return None;
    }
    #[allow(clippy::redundant_guards)] // float literal patterns are not valid
    Some(match v {
        CalcVal::Num(n) => Dim::Px(n),
        CalcVal::Dim { px, pct } if pct == 0.0 => Dim::Px(px),
        CalcVal::Dim { px, pct } if px == 0.0 => Dim::Percent(pct),
        CalcVal::Dim { px, pct } => Dim::Calc { px, pct },
    })
}

/// A `calc()` operand: a dimensionless number or a length+percentage.
#[derive(Clone, Copy)]
enum CalcVal {
    Num(f32),
    Dim { px: f32, pct: f32 },
}

impl CalcVal {
    fn add(self, o: CalcVal, sub: bool) -> Option<CalcVal> {
        let s = if sub { -1.0 } else { 1.0 };
        match (self, o) {
            (CalcVal::Num(a), CalcVal::Num(b)) => Some(CalcVal::Num(a + s * b)),
            (CalcVal::Dim { px, pct }, CalcVal::Dim { px: p2, pct: c2 }) => {
                Some(CalcVal::Dim { px: px + s * p2, pct: pct + s * c2 })
            }
            _ => None, // number ± dimension is invalid
        }
    }
    fn mul(self, o: CalcVal) -> Option<CalcVal> {
        match (self, o) {
            (CalcVal::Num(a), CalcVal::Num(b)) => Some(CalcVal::Num(a * b)),
            (CalcVal::Num(n), CalcVal::Dim { px, pct })
            | (CalcVal::Dim { px, pct }, CalcVal::Num(n)) => {
                Some(CalcVal::Dim { px: px * n, pct: pct * n })
            }
            _ => None, // dimension * dimension is invalid
        }
    }
    fn div(self, o: CalcVal) -> Option<CalcVal> {
        let CalcVal::Num(d) = o else { return None }; // may only divide by a number
        if d == 0.0 {
            return None;
        }
        Some(match self {
            CalcVal::Num(a) => CalcVal::Num(a / d),
            CalcVal::Dim { px, pct } => CalcVal::Dim { px: px / d, pct: pct / d },
        })
    }
}

#[derive(Clone, Copy)]
enum CalcTok {
    Val(CalcVal),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

/// Tokenize a calc() body. Handles unit-suffixed numbers, `%`, operators, parens, and a
/// transparent nested `calc(`.
fn tokenize_calc(s: &str, fs: f32) -> Option<Vec<CalcTok>> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
        } else if c == '+' {
            out.push(CalcTok::Plus);
            i += 1;
        } else if c == '-' {
            out.push(CalcTok::Minus);
            i += 1;
        } else if c == '*' {
            out.push(CalcTok::Star);
            i += 1;
        } else if c == '/' {
            out.push(CalcTok::Slash);
            i += 1;
        } else if c == '(' {
            out.push(CalcTok::LParen);
            i += 1;
        } else if c == ')' {
            out.push(CalcTok::RParen);
            i += 1;
        } else if s[i..].starts_with("calc(") {
            out.push(CalcTok::LParen); // nested calc is transparent
            i += 5;
        } else if c.is_ascii_digit() || c == '.' {
            // A number, optionally with a unit or `%`.
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            let num: f32 = s[start..i].parse().ok()?;
            let ustart = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphabetic() || b[i] == b'%') {
                i += 1;
            }
            let unit = &s[ustart..i];
            let val = if unit.is_empty() {
                CalcVal::Num(num)
            } else if unit == "%" {
                CalcVal::Dim { px: 0.0, pct: num }
            } else {
                CalcVal::Dim { px: dimension_to_px(num, unit, fs)?, pct: 0.0 }
            };
            out.push(CalcTok::Val(val));
        } else {
            return None;
        }
    }
    Some(out)
}

struct CalcParser<'a> {
    toks: &'a [CalcTok],
    i: usize,
}

impl CalcParser<'_> {
    fn peek(&self) -> Option<CalcTok> {
        self.toks.get(self.i).copied()
    }
    fn expr(&mut self) -> Option<CalcVal> {
        let mut v = self.term()?;
        while let Some(op @ (CalcTok::Plus | CalcTok::Minus)) = self.peek() {
            self.i += 1;
            let rhs = self.term()?;
            v = v.add(rhs, matches!(op, CalcTok::Minus))?;
        }
        Some(v)
    }
    fn term(&mut self) -> Option<CalcVal> {
        let mut v = self.factor()?;
        while let Some(op @ (CalcTok::Star | CalcTok::Slash)) = self.peek() {
            self.i += 1;
            let rhs = self.factor()?;
            v = if matches!(op, CalcTok::Star) { v.mul(rhs)? } else { v.div(rhs)? };
        }
        Some(v)
    }
    fn factor(&mut self) -> Option<CalcVal> {
        match self.peek()? {
            CalcTok::Minus => {
                self.i += 1;
                self.factor()?.mul(CalcVal::Num(-1.0))
            }
            CalcTok::Plus => {
                self.i += 1;
                self.factor()
            }
            CalcTok::LParen => {
                self.i += 1;
                let v = self.expr()?;
                matches!(self.peek(), Some(CalcTok::RParen)).then_some(())?;
                self.i += 1;
                Some(v)
            }
            CalcTok::Val(v) => {
                self.i += 1;
                Some(v)
            }
            _ => None,
        }
    }
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
    if lower.starts_with("hsl") {
        return parse_hsl_func(&lower);
    }
    named_color(&lower)
}

/// Parse `hsl()` / `hsla()` — hue in degrees (bare number or `deg`), saturation/lightness
/// as percentages, optional `/ alpha` or 4th component. Comma- or space-separated.
fn parse_hsl_func(s: &str) -> Option<Rgba> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    let inner = s[open + 1..close].replace('/', " ");
    let parts: Vec<&str> = inner
        .split([',', ' '])
        .filter(|t| !t.trim().is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let hue: f32 = parts[0]
        .trim()
        .trim_end_matches("deg")
        .trim()
        .parse()
        .ok()?;
    let sat: f32 = parts[1].trim().trim_end_matches('%').trim().parse().ok()?;
    let light: f32 = parts[2].trim().trim_end_matches('%').trim().parse().ok()?;
    let a = if parts.len() >= 4 {
        let raw = parts[3].trim();
        let v: f32 = raw.trim_end_matches('%').parse().ok()?;
        let v = if raw.contains('%') { v / 100.0 } else { v };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    } else {
        255
    };
    let (r, g, b) = hsl_to_rgb(hue, sat / 100.0, light / 100.0);
    Some(Rgba::new(r, g, b, a))
}

/// HSL → RGB (CSS Color 4). `h` in degrees, `s`/`l` in `0..=1`.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let h = h.rem_euclid(360.0) / 360.0;
    let hue = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        (
            hue(p, q, h + 1.0 / 3.0),
            hue(p, q, h),
            hue(p, q, h - 1.0 / 3.0),
        )
    };
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
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
        let av = if nums[3].contains('%') {
            av / 100.0
        } else {
            av
        };
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
        assert_eq!(
            parse_color("rgb(10, 20, 30)"),
            Some(Rgba::new(10, 20, 30, 255))
        );
        assert_eq!(parse_color("rgba(0,0,0,0.5)").unwrap().a, 128);
        assert_eq!(
            parse_color("rebeccapurple"),
            Some(Rgba::new(102, 51, 153, 255))
        );
        assert_eq!(parse_color("nonsense"), None);
        // hsl(): red, green, a mid-grey, and hsla with alpha.
        assert_eq!(parse_color("hsl(0, 100%, 50%)"), Some(Rgba::new(255, 0, 0, 255)));
        assert_eq!(parse_color("hsl(120 100% 50%)"), Some(Rgba::new(0, 255, 0, 255)));
        assert_eq!(parse_color("hsl(0, 0%, 50%)"), Some(Rgba::new(128, 128, 128, 255)));
        assert_eq!(parse_color("hsla(240, 100%, 50%, 0.5)").unwrap().a, 128);
        assert_eq!(parse_color("hsl(240deg 100% 50%)"), Some(Rgba::new(0, 0, 255, 255)));
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

#[cfg(test)]
mod calc_tests {
    use super::*;
    #[test]
    fn calc_full_evaluator() {
        // additive
        assert_eq!(parse_dim("calc(100% - 60px)", 16.0), Dim::Calc { px: -60.0, pct: 100.0 });
        // multiplication / division by a number
        assert_eq!(parse_dim("calc(100% / 3)", 16.0), Dim::Percent(100.0 / 3.0));
        assert_eq!(parse_dim("calc(50px * 2)", 16.0), Dim::Px(100.0));
        assert_eq!(parse_dim("calc(2 * 30px)", 16.0), Dim::Px(60.0));
        // parens + precedence
        assert_eq!(parse_dim("calc((100% - 20px) / 2)", 16.0), Dim::Calc { px: -10.0, pct: 50.0 });
        // nested calc
        assert_eq!(parse_dim("calc(100% - calc(10px + 10px))", 16.0), Dim::Calc { px: -20.0, pct: 100.0 });
        // unary minus
        assert_eq!(parse_dim("calc(-5px + 10px)", 16.0), Dim::Px(5.0));
        // em units inside calc
        assert_eq!(parse_dim("calc(2em + 8px)", 16.0), Dim::Px(40.0));
    }
}
