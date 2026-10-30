//! Textual scan for the module specifiers a script names (static `import … from`, bare `import 'm'`,
//! and literal `import('m')`). Lives here, not in `manuk-page`, because both the document's up-front
//! module-graph pre-fetch (page) and the dynamic-import host hook's on-demand fetch (this crate) need it.

/// Whether a `/` after the previous token `prev` starts a regex literal rather than a division.
fn regex_allowed_after(prev: &str) -> bool {
    match prev {
        "return" | "typeof" | "case" | "in" | "of" | "delete" | "void" | "throw" | "new"
        | "else" | "do" | "instanceof" | "yield" | "await" => true,
        p if p.len() == 1 && !p.as_bytes()[0].is_ascii_alphanumeric() && p != "_" && p != "$" => {
            !matches!(p, ")" | "]" | "}")
        }
        _ => false,
    }
}

/// Index just past the template literal whose opening backtick is at `i - 1`. Handles `\\` escapes and
/// `${ … }` substitutions, which are code and may contain strings, braces and further templates.
fn skip_template_literal(b: &[u8], mut i: usize) -> usize {
    let n = b.len();
    while i < n {
        match b[i] {
            b'\\' => i += 2,
            b'`' => return i + 1,
            b'$' if i + 1 < n && b[i + 1] == b'{' => {
                i += 2;
                let mut depth = 1usize;
                while i < n && depth > 0 {
                    match b[i] {
                        b'`' => i = skip_template_literal(b, i + 1),
                        q @ (b'\'' | b'"') => {
                            i += 1;
                            while i < n && b[i] != q {
                                i += if b[i] == b'\\' { 2 } else { 1 };
                            }
                            i += 1;
                        }
                        b'{' => {
                            depth += 1;
                            i += 1;
                        }
                        b'}' => {
                            depth -= 1;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
    n
}

/// **Scan JS source for the specifiers of its STATIC `import` / `export … from` statements (B3b).**
///
/// The page-path module-graph pre-fetch needs to know *what a module imports* before any JS runs, so it
/// can fetch the whole transitive graph up front (there is no synchronous network on the JS thread — the
/// authoritative walk in `esm_load_graph` reads a *pre-fetched* map). SpiderMonkey gives the exact
/// specifiers only *after* a module is compiled, and compiling needs the JS engine on the UI thread —
/// so this is a lightweight textual pre-scan whose only job is to seed the fetch. It is deliberately a
/// SUPERSET-or-miss heuristic, not a parser: an over-match fetches a URL nothing imports (harmless), and
/// a miss means that dependency is absent from the map and its importer fails loud-but-safe at
/// `ModuleLink` — the same failure policy `esm_load_graph` already documents. `esm_load_graph` remains
/// the source of truth for which modules are actually linked; this only decides what gets fetched.
///
/// Matches the specifier string of `import x from 'm'`, `import {a} from 'm'`, `import * as n from 'm'`,
/// `import 'm'` (side-effect), and `export … from 'm'` / `export * from 'm'`. Skips comments and string
/// bodies so a `from`/`import` inside them is not mistaken for a keyword, and skips `import(` (a dynamic
/// import — resolved lazily at runtime, not part of the static graph) and `import.meta`.
pub fn scan_static_import_specifiers(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut out = Vec::new();
    // The last significant token in code context: an identifier word, or a single punctuation byte. A
    // specifier is captured when this is exactly `from` (any import/export … from) or `import` (a bare
    // side-effect import). Tracking it — rather than the string alone — is what keeps `{`, `}`, `,`, `*`
    // between `import` and `from` from masquerading as the keyword, and what makes `import(` and
    // `import.meta` not fire the bare-import rule (the `(` / `.` becomes the previous token instead).
    let mut prev = String::new();
    while i < n {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Comments — skipped so a `//`/`/* */` `from` is never read as a keyword.
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            i += 2;
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(n);
            continue;
        }
        // A regex literal is skipped WHOLE. Without this `/opr\//i` reads `//` as a line comment and swallows
        // the rest of a minified file, hiding every later `import()` (a Vike bundle lost its 404 chunk).
        // `/` opens a regex only where a division cannot stand: after punctuation other than `)`/`]`/`}`,
        // or after one of these keywords.
        if c == b'/' && regex_allowed_after(&prev) {
            i += 1;
            let mut in_class = false;
            while i < n && b[i] != b'\n' {
                match b[i] {
                    b'\\' => i += 1,
                    b'[' => in_class = true,
                    b']' => in_class = false,
                    b'/' if !in_class => break,
                    _ => {}
                }
                i += 1;
            }
            i = (i + 1).min(n);
            prev = "/re/".to_string();
            continue;
        }
        // String literal — capture its body; a `'`/`"` body that directly follows `from` or `import`
        // is a specifier. Backtick strings are consumed but never captured (a static specifier is a
        // plain string literal, never a template).
        // A template literal is skipped WHOLE, nesting included: `${ … `inner` … }` holds code, and code holds
        // more templates. Ending at the next backtick (the old behaviour) flipped the parity of every
        // quote after a nested template, so `import("./pages.js")` in a Vike entry went unseen.
        if c == b'`' {
            i = skip_template_literal(b, i + 1);
            prev.clear();
            continue;
        }
        if c == b'\'' || c == b'"' {
            let quote = c;
            i += 1;
            let mut content: Vec<u8> = Vec::new();
            while i < n {
                let d = b[i];
                if d == b'\\' && i + 1 < n {
                    content.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                if d == quote {
                    i += 1;
                    break;
                }
                content.push(d);
                i += 1;
            }
            if (quote == b'\'' || quote == b'"')
                && (prev == "from" || prev == "import" || prev == "import(")
            {
                let s = String::from_utf8_lossy(&content).trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
            }
            prev.clear();
            continue;
        }
        // Identifier / keyword.
        if c == b'_' || c == b'$' || c.is_ascii_alphabetic() {
            let s = i;
            i += 1;
            while i < n && (b[i] == b'_' || b[i] == b'$' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            prev = String::from_utf8_lossy(&b[s..i]).to_string();
            continue;
        }
        // Punctuation. `import(` is a dynamic import — clear so the bare-import rule cannot fire on the
        // string inside. Any other single byte becomes the previous token.
        if c == b'(' && prev == "import" {
            // **A LITERAL `import("m")` specifier IS collected (t624).** The dynamic-import host hook
            // resolves from this same pre-fetched map, so a specifier missing from it is one
            // `import()` cannot satisfy. `import(` becomes its own marker so the string branch can
            // tell it from a bare side-effect `import 'm'`. A COMPUTED specifier still cannot be seen
            // by a textual scan and rejects at runtime — over-collecting is the safe direction, since
            // a fetched module nothing imports costs one request and a missing one costs the feature.
            prev.clear();
            prev.push_str("import(");
        } else {
            prev = (c as char).to_string();
        }
        i += 1;
    }
    out
}
