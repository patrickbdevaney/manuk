//! **Is this string a syntactically valid selector?** — a question distinct from *"can we match it"*,
//! and the engine had no way to ask it.
//!
//! `document.querySelectorAll('[')` returned an **empty NodeList**. So did `querySelector('div,')`,
//! `matches('::example')` and `closest('^|div')`. The spec says all four **throw a `SyntaxError`
//! `DOMException`**, and the difference is not pedantry: *try/catch around a selector is how the web
//! feature-detects selector support*, so an engine that never throws reports **every** selector as
//! supported — including the ones it silently cannot match. That is the "ask what a library
//! BELIEVES" shape again, and it is worth ~272 `dom` subtests on its own.
//!
//! # Why this is a SEPARATE pass and not a return value from the matcher's parser
//!
//! `parse_selector` returns `None` for two completely different reasons, and collapsing them is
//! exactly the bug:
//!
//! | input | valid? | `parse_selector` | correct answer |
//! |---|---|---|---|
//! | `p::first-line` | **yes** — a real pseudo-element we do not model | `None` | empty list, **no throw** |
//! | `p:hover` | **yes** | `NeverStatic` | empty list, no throw |
//! | `::example` | **no** — an unknown pseudo-element | `None` | **throw `SyntaxError`** |
//! | `[` | **no** | `None` | **throw `SyntaxError`** |
//!
//! Throwing whenever the matcher declines would break every page that queries a pseudo-element or a
//! construct we have not implemented yet — turning a missing capability into a **thrown exception in
//! the page's own code**, which is strictly worse than the empty list it gets today. So validity is
//! answered here, by grammar, and modelling stays the matcher's business.
//!
//! # Why NOT Stylo, even though Stylo has a real selector parser
//!
//! `SelectorParser::parse_author_origin_no_namespace` is right there and is the authority for the
//! cascade. It is the wrong authority for this, for a reason this project already paid for once:
//! **Stylo's *servo* build returns `false` from `parse_has()`**, so it rejects `:has()` — the
//! construct we deliberately hand-rolled a supplement for because 13% of the corpus uses it.
//! Delegating validity to Stylo would make `querySelector(':has(.x)')` **throw**, converting a
//! shipped capability into an exception. The grammar below is ours for the same reason the matcher is.
//!
//! # The bias is deliberate: when unsure, say VALID
//!
//! A false "invalid" throws inside a real page's script and takes the page down; a false "valid"
//! returns the empty list that was already being returned. Those are not symmetric, so every
//! ambiguous case here resolves to valid — vendor-prefixed pseudos are accepted without being on the
//! list, an unrecognised functional pseudo whose *name* is known is accepted whatever its argument,
//! and anything this grammar does not positively recognise as malformed passes.

/// The pseudo-CLASSES that are real CSS, whether or not this engine models them. An unknown
/// pseudo-class is a **syntax error** per Selectors — `div:linkexample` is invalid, not merely
/// unsupported — so this list is load-bearing for the throw, and its errors are asymmetric: a name
/// missing from it throws inside a real page, a name wrongly on it merely fails to throw.
const PSEUDO_CLASSES: &[&str] = &[
    // structural
    "root",
    "empty",
    "first-child",
    "last-child",
    "only-child",
    "nth-child",
    "nth-last-child",
    "first-of-type",
    "last-of-type",
    "only-of-type",
    "nth-of-type",
    "nth-last-of-type",
    "nth-col",
    "nth-last-col",
    "scope",
    "host",
    "host-context",
    // logical / functional
    "not",
    "is",
    "where",
    "has",
    "matches",
    "any",
    "has-slotted",
    // link & user action
    "link",
    "visited",
    "any-link",
    "local-link",
    "target",
    "target-within",
    "hover",
    "active",
    "focus",
    "focus-visible",
    "focus-within",
    "current",
    "past",
    "future",
    "playing",
    "paused",
    "seeking",
    "buffering",
    "stalled",
    "muted",
    "volume-locked",
    // input
    "enabled",
    "disabled",
    "read-only",
    "read-write",
    "placeholder-shown",
    "default",
    "checked",
    "indeterminate",
    "blank",
    "valid",
    "invalid",
    "in-range",
    "out-of-range",
    "required",
    "optional",
    "user-invalid",
    "user-valid",
    "autofill",
    "open",
    "closed",
    "picture-in-picture",
    "fullscreen",
    "modal",
    "popover-open",
    "defined",
    "dir",
    "lang",
    "state",
    // tree/UI
    "left",
    "right",
    "first",
    "active-view-transition",
    "active-view-transition-type",
    "heading",
];

/// The pseudo-ELEMENTS that are real CSS. Same asymmetry as [`PSEUDO_CLASSES`]. `::example` is a
/// syntax error; `::first-line` is valid and simply matches nothing through `querySelectorAll`.
const PSEUDO_ELEMENTS: &[&str] = &[
    "before",
    "after",
    "first-line",
    "first-letter",
    "selection",
    "target-text",
    "spelling-error",
    "grammar-error",
    "highlight",
    "marker",
    "placeholder",
    "file-selector-button",
    "backdrop",
    "part",
    "slotted",
    "cue",
    "cue-region",
    "details-content",
    "checkmark",
    "picker-icon",
    "picker",
    "scroll-marker",
    "scroll-marker-group",
    "scroll-button",
    "column",
    "view-transition",
    "view-transition-group",
    "view-transition-image-pair",
    "view-transition-old",
    "view-transition-new",
];

/// **The one entry point.** `None` = syntactically valid (whether or not we can match it);
/// `Some(msg)` = a `SyntaxError` `DOMException` is owed, with `msg` as its message.
pub fn selector_syntax_error(text: &str) -> Option<String> {
    // ⚠⚠⚠ **COMMENTS FIRST, and skipping this cost 289 `css/selectors` subtests.**
    //
    // `/* … */` is legal anywhere whitespace is, and `css/selectors/attribute-selectors` writes it
    // *inside the selector it is testing* — `[foo='BAR'] /* sanity check (match) */`. A validator
    // that has not stripped comments calls those malformed and throws on them, which is the exact
    // false-positive this file's own doc comment says is the dangerous direction. Measured before
    // the fix: `dom` +272 and `css/selectors` **−289**, a net LOSS that the ratchet refuses.
    let stripped = strip_comments(text);
    let scan = stripped.as_str();
    if scan.trim().is_empty() {
        return Some(bad(text));
    }
    // An EMPTY member is invalid — `div,` and `,div` and `a,,b` are all syntax errors, and this is
    // the one place a naive "split and filter empties" is silently wrong rather than merely lossy.
    for member in split_commas(scan) {
        let m = member.trim();
        if m.is_empty() || complex_is_bad(m) {
            return Some(bad(text));
        }
    }
    None
}

/// Replace every `/* … */` with a single space, leaving quoted strings and escapes alone. An
/// unterminated comment runs to end of input, which is what CSS does with it.
fn strip_comments(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        if c == '\\' {
            out.push(c);
            if i + 1 < b.len() {
                out.push(b[i + 1]);
            }
            i += 2;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            out.push(c);
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push(' ');
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn bad(text: &str) -> String {
    format!("'{text}' is not a valid selector")
}

/// Split on top-level commas, **keeping empty members** — the difference from the matcher's splitter,
/// and the reason `div,` is detected here and nowhere else.
fn split_commas(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut paren, mut bracket) = (0i32, 0i32);
    let mut quote: Option<char> = None;
    let mut esc = false;
    for ch in text.chars() {
        if esc {
            cur.push(ch);
            esc = false;
            continue;
        }
        match ch {
            '\\' => {
                esc = true;
                cur.push(ch);
            }
            c if Some(c) == quote => {
                quote = None;
                cur.push(c);
            }
            '"' | '\'' if quote.is_none() => {
                quote = Some(ch);
                cur.push(ch);
            }
            _ if quote.is_some() => cur.push(ch),
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            '[' => {
                bracket += 1;
                cur.push(ch);
            }
            ']' => {
                bracket -= 1;
                cur.push(ch);
            }
            ',' if paren == 0 && bracket == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// A complex selector: compounds joined by combinators. Rejects a leading or trailing combinator
/// (`>*`), a doubled one (`div ++ address`) and a character that is no combinator at all
/// (`div % address`).
fn complex_is_bad(text: &str) -> bool {
    let mut compound = String::new();
    let mut seen_compound = false;
    let mut pending_comb = false; // a combinator is waiting for its right-hand compound
    let (mut paren, mut bracket) = (0i32, 0i32);
    let mut quote: Option<char> = None;
    let mut esc = false;

    macro_rules! close_compound {
        () => {
            if !compound.is_empty() {
                if compound_is_bad(&compound) {
                    return true;
                }
                compound.clear();
                seen_compound = true;
                pending_comb = false;
            }
        };
    }

    for ch in text.chars() {
        if esc {
            compound.push(ch);
            esc = false;
            continue;
        }
        match ch {
            '\\' => {
                esc = true;
                compound.push(ch);
            }
            c if Some(c) == quote => {
                quote = None;
                compound.push(c);
            }
            '"' | '\'' if quote.is_none() => {
                quote = Some(ch);
                compound.push(ch);
            }
            _ if quote.is_some() => compound.push(ch),
            '(' => {
                paren += 1;
                compound.push(ch);
            }
            ')' => {
                paren -= 1;
                compound.push(ch);
            }
            '[' => {
                bracket += 1;
                compound.push(ch);
            }
            ']' => {
                bracket -= 1;
                compound.push(ch);
            }
            _ if paren > 0 || bracket > 0 => compound.push(ch),
            c if c.is_whitespace() => close_compound!(),
            '>' | '+' | '~' => {
                close_compound!();
                // Nothing to the LEFT (`>*`) or two in a row (`div ++ p`) — both invalid. The `~`
                // case is not reachable as `~=` because that only appears inside `[...]`, handled above.
                if !seen_compound || pending_comb {
                    return true;
                }
                pending_comb = true;
            }
            _ => compound.push(ch),
        }
    }
    // ⚠ **AN UNCLOSED BLOCK AT END-OF-INPUT IS NOT AN ERROR.** CSS Syntax closes `[`, `(` and an
    // open string at EOF, so `[align="center"` and `::slotted(foo` are **valid** selectors — WPT
    // lists both in its VALID corpus, and rejecting them was this validator's first false positive.
    // A stray CLOSER (`)`, `]`) is still invalid; it reaches `compound_is_bad`'s catch-all.
    if compound.is_empty() {
        // Ended on a combinator, or on nothing at all.
        return pending_comb || !seen_compound;
    }
    compound_is_bad(&compound)
}

/// One compound: an optional namespace-qualified type selector followed by any number of
/// `#id` / `.class` / `[attr]` / `:pseudo` / `::pseudo` parts.
fn compound_is_bad(text: &str) -> bool {
    let b: Vec<char> = text.chars().collect();
    let mut i = 0usize;

    // ── The namespace prefix, and it is a rejection this engine is REQUIRED to make.
    //
    // `querySelector` has no way to declare a namespace prefix — there is no `@namespace` in scope —
    // so ANY non-empty, non-`*` prefix is UNDECLARED and the selector is invalid. `ns|div` is a
    // syntax error in every browser for exactly this reason, while `*|div` and `|div` are fine.
    if let Some(bar) = top_level_bar(&b) {
        let prefix: String = b[..bar].iter().collect();
        if !(prefix.is_empty() || prefix == "*") {
            return true;
        }
        i = bar + 1;
    }

    // ── The type selector.
    if i < b.len() && b[i] == '*' {
        i += 1;
    } else if i < b.len() && is_ident_start(b[i]) {
        // ⚠ `scan_ident`, NOT a bare `is_ident_char` loop. `G\:TEST` is a TYPE selector whose name
        // contains an escaped colon — real markup, found on `www.unoeste.br` by the t1203 corpus
        // sweep — and a scan that stops at the backslash reads `:TEST` as an unknown pseudo-class
        // and throws inside the page's own script. t1200 taught escapes to `#`, `.` and pseudo names
        // and NOT to this one; a partial fix in the direction that THROWS is the dangerous one.
        i = scan_ident(&b, i);
    }

    let mut parts = 0usize;
    while i < b.len() {
        match b[i] {
            '#' | '.' => {
                i += 1;
                let start = i;
                if b.get(i).copied().map(is_ident_start) != Some(true) {
                    return true; // `#`, `.`, `..test`, `.5cm`, `.bar.`
                }
                i = scan_ident(&b, i);
                if i == start {
                    return true;
                }
            }
            '[' => {
                let end = matching(&b, i, '[', ']');
                let inner: String = b[i + 1..end.min(b.len())].iter().collect();
                if attr_is_bad(&inner) {
                    return true;
                }
                i = (end + 1).min(b.len());
            }
            ':' => {
                let double = b.get(i + 1) == Some(&':');
                i += if double { 2 } else { 1 };
                // `:::before` — a third colon — and `:: before` (a space before the name) are both
                // syntax errors, and both are reachable only here.
                let start = i;
                i = scan_ident(&b, i);
                if i == start {
                    return true;
                }
                let name: String = b[start..i].iter().collect::<String>().to_ascii_lowercase();
                if !pseudo_is_known(&name, double) {
                    return true;
                }
                if b.get(i) == Some(&'(') {
                    let end = matching(&b, i, '(', ')');
                    let inner: String = b[i + 1..end.min(b.len())].iter().collect();
                    // ⚠ RECURSE. `:not(ns|div)` is invalid for what is inside it, and a validator
                    // that only matched the parens reported it valid — the one WPT invalid case
                    // this file missed on its first run. `:has()` members may lead with a
                    // combinator (a RELATIVE selector), so its argument is checked leniently.
                    if takes_selector_list(&name) && !inner.trim().is_empty() {
                        for member in split_commas(&inner) {
                            let m = member.trim();
                            if m.is_empty() || complex_is_bad(m) {
                                return true;
                            }
                        }
                    }
                    i = (end + 1).min(b.len());
                }
            }
            _ => return true, // `{`, `}`, `<`, `%`, `)` , `]`, a stray anything
        }
        parts += 1;
    }
    // A compound has to BE something: `*` and a bare type both consumed above, so reaching here with
    // nothing consumed at all means the text was empty or pure junk.
    i == 0 && parts == 0
}

/// `::before` is only ever a pseudo-element and `:hover` only ever a pseudo-class, except that the
/// four CSS2 pseudo-elements are legal with a single colon — which real sheets still write.
fn pseudo_is_known(name: &str, double_colon: bool) -> bool {
    // ⚠ FAIL OPEN on a vendor prefix. `-webkit-any-link`, `-moz-focusring` and their kin are not on
    // any list we can keep current, they appear in real sheets, and a false "invalid" throws inside
    // the page. An unrecognised vendor pseudo therefore does NOT throw.
    if name.starts_with("-") {
        return true;
    }
    if double_colon {
        PSEUDO_ELEMENTS.contains(&name)
    } else {
        PSEUDO_CLASSES.contains(&name)
            || matches!(name, "before" | "after" | "first-line" | "first-letter")
    }
}

/// `[name]`, `[name op value]`, `[name op value i]`. Rejects `[*=test]` (`*` is not a name),
/// `[*|*=test]` (same, behind a namespace) and `[class= space unquoted ]` (an unquoted value is a
/// single identifier and cannot contain whitespace).
fn attr_is_bad(inner: &str) -> bool {
    let t = inner.trim();
    if t.is_empty() {
        return true;
    }
    let b: Vec<char> = t.chars().collect();
    let mut i = 0usize;

    // Optional namespace on the ATTRIBUTE name. `[*|attr]` is legal here (unlike a type selector's
    // prefix, `*|` on an attribute means "any namespace" and needs no declaration).
    if let Some(bar) = top_level_bar(&b) {
        let prefix: String = b[..bar].iter().collect();
        if !(prefix.is_empty() || prefix == "*") {
            return true;
        }
        i = bar + 1;
    }
    let start = i;
    if b.get(i).copied().map(is_ident_start) != Some(true) {
        return true; // `[*=test]` lands here: `*` is not an identifier start
    }
    i = scan_ident(&b, i);
    if i == start {
        return true;
    }
    while i < b.len() && b[i].is_whitespace() {
        i += 1;
    }
    if i == b.len() {
        return false; // `[name]`
    }
    // The operator.
    let op_len = match (b[i], b.get(i + 1)) {
        ('=', _) => 1,
        ('~', Some('='))
        | ('|', Some('='))
        | ('^', Some('='))
        | ('$', Some('='))
        | ('*', Some('=')) => 2,
        _ => return true,
    };
    i += op_len;
    while i < b.len() && b[i].is_whitespace() {
        i += 1;
    }
    if i >= b.len() {
        return true; // an operator with no value
    }
    // The value: a quoted string, or a single identifier.
    if b[i] == '"' || b[i] == '\'' {
        let q = b[i];
        i += 1;
        let mut closed = false;
        while i < b.len() {
            if b[i] == '\\' {
                i += 2;
                continue;
            }
            if b[i] == q {
                closed = true;
                i += 1;
                break;
            }
            i += 1;
        }
        if !closed {
            return true;
        }
    } else {
        let s = i;
        // An unquoted value is an IDENT. `[class= space unquoted ]` fails here — not on the first
        // word, which parses fine, but on the second, which has nowhere to go.
        //
        // ⚠ And it is an ident WITH ESCAPES: `a[href*=\#]` is the anchor-link idiom every
        // smooth-scroll and tab script on the web is written with, and it was found throwing on
        // `www.unoeste.br` by the t1203 corpus sweep. Same omission as the type selector above.
        if !is_ident_start(b[i]) && !b[i].is_ascii_digit() {
            return true;
        }
        i = scan_ident(&b, i);
        if i == s {
            return true;
        }
    }
    while i < b.len() && b[i].is_whitespace() {
        i += 1;
    }
    if i == b.len() {
        return false;
    }
    // Only a case-sensitivity flag may follow.
    // The flag may be written ESCAPED — `\\s`, or as a hex code point `\\73` / `\\49`. An escape is a
    // way to SPELL a character, not a different character, so it is decoded before the comparison.
    // `css/selectors/attribute-selectors` writes all three forms, and treating `\\73` as the literal
    // text `73` rejected six of them.
    let rest: String = css_unescape(&b[i..].iter().collect::<String>())
        .trim()
        .to_ascii_lowercase();
    rest != "i" && rest != "s"
}

/// The index of a `|` that is a namespace separator — i.e. not part of `|=`, and not preceded by one
/// of the other operator characters.
fn top_level_bar(b: &[char]) -> Option<usize> {
    for (i, c) in b.iter().enumerate() {
        if *c == '|' && b.get(i + 1) != Some(&'=') {
            return Some(i);
        }
        if matches!(c, '[' | '(' | ':' | '#' | '.') {
            return None;
        }
    }
    None
}

/// The index of the closer, or `b.len()` when the input ENDS first — CSS closes an open block at
/// EOF rather than invalidating it, which is why `[align="center"` and `::slotted(foo` are valid.
fn matching(b: &[char], open: usize, o: char, c: char) -> usize {
    let mut depth = 0i32;
    for (i, ch) in b.iter().enumerate().skip(open) {
        if *ch == o {
            depth += 1;
        } else if *ch == c {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    b.len()
}

/// Scan an identifier starting at `i`, honouring CSS escapes: a `\\` consumes whatever follows it,
/// which is how `.foo\\:bar`, `#\\#foo\\:bar` and `.test\\.foo\\[5\\]bar` are single class/id names
/// rather than a class followed by a pseudo. Missing this rejected four of WPT's VALID selectors.
fn scan_ident(b: &[char], mut i: usize) -> usize {
    while i < b.len() {
        if b[i] == '\\' {
            i += 2;
            continue;
        }
        if !is_ident_char(b[i]) {
            break;
        }
        i += 1;
    }
    i.min(b.len())
}

/// The functional pseudos whose argument is a selector list **and which are NOT forgiving** — so an
/// invalid member makes the whole selector invalid. `:not(ns|div)` is the WPT case, and it is the
/// only one this validator missed on its first run.
///
/// ⚠⚠ **`:is()`, `:where()` and `:has()` are deliberately ABSENT.** They are *forgiving* selector
/// lists: an unparsable member is DROPPED and the rest still apply, so `:is(:total-nonsense)` is a
/// **valid** selector that matches nothing. Recursing strictly into them threw on 5 `css/selectors`
/// files — the same asymmetry `:not()` has for the opposite reason (dropping one of ITS members
/// would match strictly MORE, so it must fail closed).
fn takes_selector_list(name: &str) -> bool {
    matches!(name, "not" | "host" | "host-context" | "slotted")
    // ⚠ NOT `:nth-child`/`:nth-last-child`. Their argument is **An+B**, not a selector — only the
    // `An+B of <selector>` tail is one. Listing them here rejected `:nth-child(3n)` as invalid,
    // which is eight of WPT's VALID corpus and would have thrown on zebra striping across the web.
}

/// CSS identifiers may start with a letter, `_`, `-` (not followed by a digit), an escape, or any
/// non-ASCII character. A leading digit is what makes `.5cm` invalid.
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-' || c == '\\' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// Decode CSS escapes: `\\` followed by 1–6 hex digits (and an optional single trailing whitespace)
/// is that code point; `\\` followed by anything else is that character literally.
fn css_unescape(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != '\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < b.len() && i - start < 6 && b[i].is_ascii_hexdigit() {
            i += 1;
        }
        if i > start {
            let hex: String = b[start..i].iter().collect();
            if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(c);
            }
            // A single whitespace terminates the escape and is consumed.
            if i < b.len() && b[i].is_whitespace() {
                i += 1;
            }
        } else if i < b.len() {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod wpt_corpus {
    use super::selector_syntax_error;

    /// **WPT's own `dom/nodes/selectors.js` invalid list, verbatim.** Every one must throw.
    const INVALID: &[&str] = &[
        "",
        "[",
        "]",
        "(",
        ")",
        "{",
        "}",
        "<",
        ">",
        "#",
        "div,",
        ".",
        ".5cm",
        "..test",
        ".foo..quux",
        ".bar.",
        "div % address, p",
        "div ++ address, p",
        "div ~~ address, p",
        "[*=test]",
        "[*|*=test]",
        "[class= space unquoted ]",
        "div:example",
        ":example",
        "div:linkexample",
        "div::example",
        "::example",
        ":::before",
        ":: before",
        "ns|div",
        ":not(ns|div)",
        "^|div",
        "$|div",
        ">*",
    ];

    /// **WPT's valid list, verbatim, plus every selector `css/selectors` was observed passing to
    /// `querySelector` while this validator was being written.** None may throw.
    ///
    /// ⚠ The second corpus exists because the first was not enough: calibrated on WPT's 207 alone,
    /// this validator scored `dom` +272 and `css/selectors` **−289** — a NET LOSS the ratchet
    /// refuses — because `attribute-selectors` writes `/* … */` comments inside the selector under
    /// test and `:is()` is a FORGIVING list. Neither is visible in the first corpus.
    const VALID: &[&str] = &[
        // ⚠⚠⚠ **FOUND THROWING ON REAL SITES BY THE t1203 CORPUS SWEEP, and neither corpus above
        // contains their shape.** A type selector whose name carries an escaped colon, and the
        // anchor-link attribute idiom with an escaped `#`. Both are ESCAPE handling that t1200 taught
        // to `#`/`.`/pseudo names and not to the type selector or the unquoted attribute value — a
        // partial fix in the direction that THROWS. THIRD population, third class of miss.
        "G\\:TEST",
        "a[href*=\\#]:not([href=\\#]):not(.scroll-ignore):not([data-tab]):not([data-toggle])",
        "html",
        "html",
        "body",
        "body",
        "*",
        "#universal>*",
        "#universal>*>*",
        "#empty>*",
        "#universal *",
        ".attr-presence-div1[align]",
        ".attr-presence-div2[align]",
        "#attr-presence [*|TiTlE]",
        "#attr-presence [*|TiTlE]",
        "[data-attr-presence]",
        ".attr-presence-div3[align], .attr-presence-div4[align]",
        "ul[data-中文]",
        "#attr-presence-select1 option[selected]",
        "#attr-presence-select2 option[selected]",
        "#attr-presence-select3 option[selected]",
        "#attr-value [align=\"center\"]",
        "#attr-value [align=\"center\"",
        "#attr-value [align=\"\"]",
        "#attr-value [align=\"c\"]",
        "#attr-value [align=\"centera\"]",
        "[data-attr-value=\"\\e9\"]",
        "[data-attr-value\\_foo=\"\\e9\"]",
        "#attr-value input[type='hidden'],#attr-value input[type='radio']",
        "#attr-value input[type=\"hidden\"],#attr-value input[type='radio']",
        "#attr-value input[type=hidden],#attr-value input[type=radio]",
        "[data-attr-value=中文]",
        "#attr-whitespace [class~=\"div1\"]",
        "#attr-whitespace [class~=\"\"]",
        "[data-attr-whitespace~=\"div\"]",
        "[data-attr-whitespace~=\"\\0000e9\"]",
        "[data-attr-whitespace\\_foo~=\"\\e9\"]",
        "#attr-whitespace a[rel~='bookmark'],  #attr-whitespace a[rel~='nofollow']",
        "#attr-whitespace a[rel~=\"bookmark\"],#attr-whitespace a[rel~='nofollow']",
        "#attr-whitespace a[rel~=bookmark],    #attr-whitespace a[rel~=nofollow]",
        "#attr-whitespace a[rel~=\"book mark\"]",
        "#attr-whitespace [title~=中文]",
        "#attr-hyphen-div1[lang|=\"en\"]",
        "#attr-hyphen-div2[lang|=\"fr\"]",
        "#attr-hyphen-div3[lang|=\"en\"]",
        "#attr-hyphen-div4[lang|=\"es-AR\"]",
        "#attr-begins a[href^=\"http://www\"]",
        "#attr-begins [lang^=\"en-\"]",
        "#attr-begins [class^=\"\"]",
        "#attr-begins [class^=apple]",
        "#attr-begins [class^=' apple']",
        "#attr-begins [class^=\" apple\"]",
        "#attr-begins [class^= apple]",
        "#attr-ends a[href$=\".org\"]",
        "#attr-ends [lang$=\"-CH\"]",
        "#attr-ends [class$=\"\"]",
        "#attr-ends [class$=apple]",
        "#attr-ends [class$='apple ']",
        "#attr-ends [class$=\"apple \"]",
        "#attr-ends [class$=apple ]",
        "#attr-contains a[href*=\"http://www\"]",
        "#attr-contains a[href*=\".org\"]",
        "#attr-contains a[href*=\".example.\"]",
        "#attr-contains [lang*=\"en-\"]",
        "#attr-contains [lang*=\"-CH\"]",
        "#attr-contains [class*=\"\"]",
        "#attr-contains [class*=' apple']",
        "#attr-contains [class*='orange ']",
        "#attr-contains [class*='ple banana ora']",
        "#attr-contains [class*=\" apple\"]",
        "#attr-contains [class*=\"orange \"]",
        "#attr-contains [class*=\"ple banana ora\"]",
        "#attr-contains [class*= apple]",
        "#attr-contains [class*=orange ]",
        "#attr-contains [class*= banana ]",
        ":root",
        ":root",
        "#pseudo-nth-table1 :nth-child(3)",
        "#pseudo-nth li:nth-child(3n)",
        "#pseudo-nth li:nth-child(2n+4)",
        "#pseudo-nth-p1 :nth-child(4n-1)",
        "#pseudo-nth-table1 :nth-last-child(3)",
        "#pseudo-nth li:nth-last-child(3n)",
        "#pseudo-nth li:nth-last-child(2n+4)",
        "#pseudo-nth-p1 :nth-last-child(4n-1)",
        "#pseudo-nth-p1 em:nth-of-type(3)",
        "#pseudo-nth-p1 :nth-of-type(2n)",
        "#pseudo-nth-p1 span:nth-of-type(2n-1)",
        "#pseudo-nth-p1 em:nth-last-of-type(3)",
        "#pseudo-nth-p1 :nth-last-of-type(2n)",
        "#pseudo-nth-p1 span:nth-last-of-type(2n-1)",
        "#pseudo-nth-p1 em:first-of-type",
        "#pseudo-nth-p1 :first-of-type",
        "#pseudo-nth-table1 tr :first-of-type",
        "#pseudo-nth-p1 em:last-of-type",
        "#pseudo-nth-p1 :last-of-type",
        "#pseudo-nth-table1 tr :last-of-type",
        "#pseudo-first-child div:first-child",
        ".pseudo-first-child-div2:first-child, .pseudo-first-child-div3:first-child",
        "#pseudo-first-child span:first-child",
        "#pseudo-last-child div:last-child",
        ".pseudo-last-child-div1:last-child, .pseudo-last-child-div2:first-child",
        "#pseudo-last-child span:last-child",
        "#pseudo-only :only-child",
        "#pseudo-only em:only-child",
        "#pseudo-only :only-of-type",
        "#pseudo-only em:only-of-type",
        "#pseudo-empty p:empty",
        "#pseudo-empty :empty",
        "#pseudo-link :link, #pseudo-link :visited",
        "#head :link, #head :visited",
        "#head :link, #head :visited",
        ":link:visited",
        ":target",
        ":target",
        "#pseudo-lang-div1:lang(en)",
        "#pseudo-lang-div1:lang(en)",
        "#pseudo-lang-div2:lang(fr)",
        "#pseudo-lang-div3:lang(en)",
        "#pseudo-lang-div4:lang(es-AR)",
        "#pseudo-ui :enabled",
        "#pseudo-link :enabled",
        "#pseudo-ui :disabled",
        "#pseudo-link :disabled",
        "#pseudo-ui :checked",
        "#not>:not(div)",
        "#not * :not(:first-child)",
        ":not(*)",
        ":not(*|*)",
        "#not>:not( div )",
        "#pseudo-element:first-line",
        "#pseudo-element::first-line",
        "#pseudo-element:first-letter",
        "#pseudo-element::first-letter",
        "#pseudo-element:before",
        "#pseudo-element::before",
        "#pseudo-element:after",
        "#pseudo-element::after",
        ".class-p",
        "#class .apple.orange.banana",
        "div.apple.banana.orange",
        ".\\u53F0\\u5317Ta\\u0301ibe\\u030Ci",
        ".\\u53F0\\u5317",
        ".\\u53F0\\u5317Ta\\u0301ibe\\u030Ci.\\u53F0\\u5317",
        ".foo\\:bar",
        ".test\\.foo\\[5\\]bar",
        "#id #id-div1",
        "#id-div1, #id-div1",
        "#id-div1, #id-div2",
        "div#id-div1, div#id-div2",
        "#id #none",
        "#none #id-div1",
        "#id-li-duplicate",
        "#\\u53F0\\u5317Ta\\u0301ibe\\u030Ci",
        "#\\u53F0\\u5317",
        "#\\u53F0\\u5317Ta\\u0301ibe\\u030Ci, #\\u53F0\\u5317",
        "#\\#foo\\:bar",
        "#test\\.foo\\[5\\]bar",
        "#any-namespace *|div",
        "#no-namespace |div",
        "#no-namespace |*",
        "#descendant div",
        "body #descendant-div1",
        "div #descendant-div1",
        "#descendant #descendant-div2",
        "#descendant .descendant-div2",
        ".descendant-div1 .descendant-div3",
        "#descendant-div1 #descendant-div4",
        "#descendant\t\n#descendant-div2",
        "#child>div",
        "div>#child-div1",
        "#child>#child-div1",
        "#child-div1>.child-div2",
        ".child-div1>.child-div2",
        "#child>#child-div3",
        "#child-div1>.child-div3",
        ".child-div1>.child-div3",
        "#child-div1\t\n>\t\n#child-div2",
        "#child-div1>\t\n#child-div2",
        "#child-div1\t\n>#child-div2",
        "#child-div1>#child-div2",
        "#adjacent-div2+div",
        "div+#adjacent-div4",
        "#adjacent-div2+#adjacent-div4",
        "#adjacent-div2+.adjacent-div4",
        ".adjacent-div2+.adjacent-div4",
        "#adjacent div+p",
        "#adjacent-div2+#adjacent-p2, #adjacent-div2+#adjacent-div1",
        "#adjacent-p2\t\n+\t\n#adjacent-p3",
        "#adjacent-p2+\t\n#adjacent-p3",
        "#adjacent-p2\t\n+#adjacent-p3",
        "#adjacent-p2+#adjacent-p3",
        "#sibling-div2~div",
        "div~#sibling-div4",
        "#sibling-div2~#sibling-div4",
        "#sibling-div2~.sibling-div",
        "#sibling div~p",
        "#sibling>p~div",
        "#sibling-div2~#sibling-div3, #sibling-div2~#sibling-div1",
        "#sibling-p2\t\n~\t\n#sibling-p3",
        "#sibling-p2~\t\n#sibling-p3",
        "#sibling-p2\t\n~#sibling-p3",
        "#sibling-p2~#sibling-p3",
        "#group em\t\n \n,\t\n \n#group strong",
        "#group em,\t\n#group strong",
        "#group em\t\n,#group strong",
        "#group em,#group strong",
        "::slotted(foo)",
        "::slotted(foo",
        ":has-slotted",
        ":has-slotted(#id)",
        ":has-slotted(*)",
        ":has-slotted(.class)",
        ":has-slotted(:first-child)",
        ":has-slotted(:hover)",
        ":has-slotted(:not(:nth-last-of-type(2)):not([slot=\"foo\"]))",
        ":has-slotted(:not(foo))",
        ":has-slotted([attr=\"foo\"])",
        ":has-slotted(bar)",
        ":has-slotted(div + div)",
        ":has-slotted(div:has(> span))",
        ":has-slotted(foo) + :has-slotted(bar)",
        ":has-slotted(foo):dir(ltr)",
        ":has-slotted(foo):first-child",
        ":has-slotted(foo):focus",
        ":has-slotted(foo):hover",
        ":has-slotted(foo):lang(en)",
        ":heading",
        ":heading(-1)",
        ":heading(0)",
        ":heading(0, 1, 2)",
        ":heading(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)",
        ":heading(1)",
        ":heading(2)",
        ":heading(3)",
        ":heading(4)",
        ":heading(5)",
        ":heading(6)",
        ":heading(6, 7)",
        ":heading(7)",
        ":heading(8)",
        ":heading(9)",
        ":heading(99999)",
        ":is(:total-nonsense)",
        ":not(:has-slotted(foo))",
        ":not(:is(svg|div))",
        "[*|lang='A'] /* sanity check (no match) */",
        "[*|lang='a'] /* sanity check (no match) */",
        "[align='LEFT'] /* sanity check (match HTML) */",
        "[align='left'] /* sanity check (match HTML) */",
        "[align='left'] /* sanity check (match) */",
        "[baz='quux'\ts\t] /* \\t */",
        "[baz='quux' /**/ s]",
        "[baz='quux' \\53]",
        "[baz='quux' \\73]",
        "[baz='quux' \\s]",
        "[baz='quux' s /**/ ]",
        "[baz='quux'/**/s/**/]",
        "[baz='quux'] /* sanity check (valid) */",
        "[baz=quux/**/s]",
        "[class~='A'] /* sanity check (match) */",
        "[class~='A'] /* sanity check (no match) */",
        "[class~='a'] /* sanity check (match) */",
        "[class~='a'] /* sanity check (no match) */",
        "[foo$='AÌ' i] /* COMBINING in selector */",
        "[foo$='AÌ' s] /* COMBINING in selector */",
        "[foo*='Ã¤' i] /* COMBINING in attribute */",
        "[foo*='Ã¤' s] /* COMBINING in attribute */",
        "[foo='\t' i] /* tab in selector */",
        "[foo='\t' s] /* tab in selector */",
        "[foo=' ' i] /* tab in attribute */",
        "[foo=' ' s] /* tab in attribute */",
        "[foo='' i] /* \\0 in attribute */",
        "[foo='' s] /* \\0 in attribute */",
        "[foo='0' i] /* \\0 in selector */",
        "[foo='0' s] /* \\0 in selector */",
        "[foo='A' i] /* COMBINING in attribute */",
        "[foo='A' s] /* COMBINING in attribute */",
        "[foo='AÌ' i] /* COMBINING in both */",
        "[foo='AÌ' i] /* COMBINING in selector */",
        "[foo='AÌ' s] /* COMBINING in both */",
        "[foo='AÌ' s] /* COMBINING in selector */",
        "[foo='BAR'] /* sanity check (match) */",
        "[foo='BAR'] /* sanity check (valid) */",
        "[foo='a' i] /* COMBINING in attribute */",
        "[foo='a' s] /* COMBINING in attribute */",
        "[foo='aÌ' i] /* COMBINING in both */",
        "[foo='aÌ' i] /* COMBINING in selector */",
        "[foo='aÌ' s] /* COMBINING in both */",
        "[foo='aÌ' s] /* COMBINING in selector */",
        "[foo='bar'\ti\t] /* \\t */",
        "[foo='bar' /**/ i]",
        "[foo='bar' \\49]",
        "[foo='bar' \\69]",
        "[foo='bar' \\i]",
        "[foo='bar' i /**/ ]",
        "[foo='bar'/**/i/**/]",
        "[foo='bar'] /* sanity check (match) */",
        "[foo='bar'] /* sanity check (no match) */",
        "[foo='Ã' i] /* COMBINING in attribute */",
        "[foo='Ã' s] /* COMBINING in attribute */",
        "[foo=bar/**/i]",
        "[foo^='AÌ' i] /* COMBINING in selector */",
        "[foo^='AÌ' s] /* COMBINING in selector */",
        "[foo|='Ã¤' i] /* COMBINING in attribute */",
        "[foo|='Ã¤' s] /* COMBINING in attribute */",
        "[foo~='aÌ' i] /* COMBINING in selector */",
        "[foo~='aÌ' s] /* COMBINING in selector */",
        "[id$='A'] /* sanity check (match) */",
        "[id^='a'] /* sanity check (match) */",
        "[id^='a'] /* sanity check (no match) */",
        "[lang*='A'] /* sanity check (match HTML) */",
        "[lang*='A'] /* sanity check (match) */",
        "[lang|='a'] /* sanity check (match HTML) */",
        "[lang|='a'] /* sanity check (match) */",
        "[missingattr] /* sanity check (no match) */",
        "h1:heading",
        "h1:heading(1)",
        "h1:heading(2)",
        "[foo='bar'\ni\n]",
        "[baz='quux'\ns\n]",
    ];

    #[test]
    fn every_wpt_invalid_selector_is_rejected_and_no_valid_one_is() {
        let missed: Vec<&str> = INVALID
            .iter()
            .copied()
            .filter(|s| selector_syntax_error(s).is_none())
            .collect();
        assert!(
            missed.is_empty(),
            "these are INVALID per WPT and must throw SyntaxError: {missed:?}"
        );
        let false_positives: Vec<&str> = VALID
            .iter()
            .copied()
            .filter(|s| selector_syntax_error(s).is_some())
            .collect();
        assert!(
            false_positives.is_empty(),
            "⚠ THE DANGEROUS DIRECTION. These are VALID selectors and this validator would throw \
             inside a real page's script for each of them: {false_positives:?}"
        );
    }
}
