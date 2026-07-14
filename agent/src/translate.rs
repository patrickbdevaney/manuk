//! E3 — **translate-page**, in place, structure preserved.
//!
//! Reuses E6's [`InferenceBackend`] (BYO endpoint, zero bundled model runtime), which is
//! why this is a small module rather than a new subsystem: §4c's local backends
//! (llama.cpp / Ollama) are the "local private tier" the item asks for, with no extra
//! code here.
//!
//! ## Why extraction is per *block*, not per text node
//!
//! Translating each text node alone shreds sentences: `<p>The <em>quick</em> fox</p>`
//! would be three fragments with no context, and a translator would produce nonsense
//! word order for most languages. Translating whole *blocks* keeps the sentence intact.
//! But then re-injecting it must not destroy the inline markup.
//!
//! So we do the standard thing: within a block, inline elements are replaced by numbered
//! **placeholders** (`<0>…</0>`), the model translates the marked-up sentence, and the
//! placeholders are expanded back to the original elements. The model is explicitly told
//! to preserve them. If it drops or mangles a placeholder we **do not** guess — the block
//! is left untranslated and the failure is reported, because a silently mangled DOM is
//! worse than an untranslated paragraph.
//!
//! **Documented gaps (not faked):** attributes (`alt`, `title`, `placeholder`) are not
//! translated; there is no language auto-detection (the caller names the source, or says
//! `auto` and lets the model decide); blocks are translated one request each (no
//! batching), which is simple and keeps a failure local to one block; a translated page
//! is not cached.

use anyhow::{bail, Context, Result};
use manuk_dom::{Dom, NodeId};

use crate::{InferenceBackend, Message, Role};

/// Tags whose text is never shown to a reader.
fn is_non_content(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "head" | "meta" | "link" | "noscript" | "template" | "title"
    )
}

/// Tags that establish a block-level unit worth translating as one sentence.
fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "td"
            | "th"
            | "blockquote"
            | "figcaption"
            | "dt"
            | "dd"
            | "caption"
            | "summary"
            | "label"
            | "button"
    )
}

/// A block of translatable content, with its inline children abstracted to placeholders.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The block element.
    pub node: NodeId,
    /// The source text, with `<0>…</0>` standing in for inline elements.
    pub marked_text: String,
    /// Placeholder index → the inline element it stands for, in order.
    pub inline_nodes: Vec<NodeId>,
}

/// Extract translatable blocks in document order.
///
/// A block whose text is empty or has no letters (e.g. `"—"`, `"2024"`) is skipped: there
/// is nothing to translate and a round trip would only risk mangling it.
pub fn extract_blocks(dom: &Dom) -> Vec<Block> {
    let mut out = Vec::new();
    collect(dom, dom.root(), &mut out);
    out
}

fn collect(dom: &Dom, node: NodeId, out: &mut Vec<Block>) {
    for child in dom.children(node).collect::<Vec<_>>() {
        let Some(tag) = dom.tag_name(child) else {
            continue;
        };
        if is_non_content(tag) {
            continue;
        }
        if is_block(tag) {
            if let Some(b) = mark_block(dom, child) {
                out.push(b);
            }
            // Do not descend: nested blocks inside a <li> are rare and translating the
            // outer block already covers the text.
            continue;
        }
        collect(dom, child, out);
    }
}

fn has_letters(s: &str) -> bool {
    s.chars().any(char::is_alphabetic)
}

/// Build the placeholder-marked source text for one block.
fn mark_block(dom: &Dom, block: NodeId) -> Option<Block> {
    let mut marked = String::new();
    let mut inline_nodes = Vec::new();

    for child in dom.children(block) {
        match dom.tag_name(child) {
            // A text child contributes its text verbatim.
            None => marked.push_str(&dom.text_content(child)),
            Some(tag) if is_non_content(tag) => {}
            Some(_) => {
                let i = inline_nodes.len();
                marked.push_str(&format!("<{i}>{}</{i}>", dom.text_content(child)));
                inline_nodes.push(child);
            }
        }
    }

    let collapsed = marked.split_whitespace().collect::<Vec<_>>().join(" ");
    if !has_letters(&collapsed) {
        return None;
    }
    Some(Block {
        node: block,
        marked_text: collapsed,
        inline_nodes,
    })
}

/// The instruction given to the model. Deliberately strict about placeholders.
fn system_prompt(source: &str, target: &str) -> String {
    format!(
        "You are a translation engine. Translate the user's text from {source} into {target}.\n\
         Rules:\n\
         - Output ONLY the translation. No commentary, no quotes, no explanation.\n\
         - The text contains numbered placeholders like <0>word</0>. Keep every \
           placeholder, keep its number, and keep the SAME number of them. Translate the \
           text inside a placeholder; never delete, merge, renumber, or reorder-away a \
           placeholder.\n\
         - Preserve punctuation and capitalization conventions of {target}."
    )
}

/// Placeholders present in `s`, as their indices, in order of appearance of the open tag.
fn placeholder_indices(s: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '<' {
            // Opening tag only: `<` digit+ `>`
            let mut j = i + 1;
            let mut num = String::new();
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                num.push(bytes[j]);
                j += 1;
            }
            if !num.is_empty() && j < bytes.len() && bytes[j] == '>' {
                if let Ok(n) = num.parse::<usize>() {
                    out.push(n);
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Split a translated block into `(text_before, inline_text, ...)` segments and rebuild
/// the DOM children, preserving the original inline elements.
///
/// Returns an error if the model's placeholders do not match the source's — the caller
/// then leaves the block alone rather than corrupting it.
pub fn apply_translation(dom: &mut Dom, block: &Block, translated: &str) -> Result<()> {
    let mut got = placeholder_indices(translated);
    let mut want: Vec<usize> = (0..block.inline_nodes.len()).collect();
    got.sort_unstable();
    want.sort_unstable();
    if got != want {
        bail!(
            "model returned placeholders {:?} but the block has {} inline element(s); \
             leaving it untranslated rather than corrupting the DOM",
            got,
            block.inline_nodes.len()
        );
    }

    // Walk the translated string, emitting text runs and inline elements in order.
    let mut new_children: Vec<NodeId> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = translated.chars().collect();
    let mut i = 0;

    let flush = |buf: &mut String, dom: &mut Dom, out: &mut Vec<NodeId>| {
        if !buf.is_empty() {
            let t = dom.create_text(std::mem::take(buf));
            out.push(t);
        }
    };

    while i < chars.len() {
        if chars[i] == '<' {
            // `<n>` … `</n>`
            let mut j = i + 1;
            let mut num = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }
            if !num.is_empty() && j < chars.len() && chars[j] == '>' {
                let n: usize = num.parse().expect("digits");
                let close: String = format!("</{n}>");
                let rest: String = chars[j + 1..].iter().collect();
                if let Some(end) = rest.find(&close) {
                    flush(&mut buf, dom, &mut new_children);
                    let inner = &rest[..end];

                    // Reuse the ORIGINAL inline element, replacing only its text. This is
                    // what preserves `href`, `class`, and every other attribute.
                    let el = block.inline_nodes[n];
                    let old_kids: Vec<NodeId> = dom.children(el).collect();
                    for k in old_kids {
                        dom.remove_child(el, k);
                    }
                    let t = dom.create_text(inner.to_string());
                    dom.append_child(el, t);
                    new_children.push(el);

                    i = j + 1 + end + close.chars().count();
                    continue;
                }
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, dom, &mut new_children);

    // Detach the block's current children, then attach the rebuilt list in order.
    let old: Vec<NodeId> = dom.children(block.node).collect();
    for k in old {
        dom.remove_child(block.node, k);
    }
    for k in new_children {
        dom.append_child(block.node, k);
    }
    Ok(())
}

/// The outcome of translating a page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TranslationReport {
    pub blocks_total: usize,
    pub blocks_translated: usize,
    /// Blocks left alone, with why. Never silently dropped.
    pub skipped: Vec<String>,
}

/// Translate every block of `dom` in place.
///
/// Blocks the model mishandles are **left untranslated** and recorded in the report.
/// `source` may be `"auto"`.
pub async fn translate_page(
    dom: &mut Dom,
    backend: &dyn InferenceBackend,
    source: &str,
    target: &str,
) -> Result<TranslationReport> {
    let blocks = extract_blocks(dom);
    let mut report = TranslationReport {
        blocks_total: blocks.len(),
        ..Default::default()
    };
    let system = system_prompt(source, target);

    for block in &blocks {
        let messages = vec![
            Message::text(Role::System, system.clone()),
            Message::text(Role::User, block.marked_text.clone()),
        ];
        let translated = match backend.complete(&messages).await {
            Ok(t) => t.trim().to_string(),
            Err(e) => {
                report
                    .skipped
                    .push(format!("{}: backend error: {e:#}", block.marked_text));
                continue;
            }
        };
        match apply_translation(dom, block, &translated) {
            Ok(()) => report.blocks_translated += 1,
            Err(e) => report.skipped.push(format!("{}: {e}", block.marked_text)),
        }
    }
    Ok(report)
}

/// Convenience: translate a whole [`manuk_page::Page`] in place and re-lay-out it.
pub async fn translate_page_and_relayout(
    page: &mut manuk_page::Page,
    fonts: &manuk_text::FontContext,
    viewport_width: f32,
    backend: &dyn InferenceBackend,
    source: &str,
    target: &str,
) -> Result<TranslationReport> {
    let report = translate_page(page.dom_mut(), backend, source, target)
        .await
        .context("translating page")?;
    page.relayout(fonts, viewport_width);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    /// A backend that maps each source string to a canned translation.
    struct Dict(Vec<(&'static str, &'static str)>);

    #[async_trait::async_trait]
    impl InferenceBackend for Dict {
        async fn complete(&self, messages: &[Message]) -> Result<String> {
            let user = match messages.last().map(|m| &m.content[0]) {
                Some(crate::Content::Text(t)) => t.clone(),
                _ => bail!("no user text"),
            };
            for (src, dst) in &self.0 {
                if *src == user {
                    return Ok((*dst).to_string());
                }
            }
            bail!("no canned translation for {user:?}")
        }
        fn name(&self) -> String {
            "dict".into()
        }
        fn supports_images(&self) -> bool {
            false
        }
    }

    #[test]
    fn extraction_skips_head_and_non_text_blocks_and_marks_inline_elements() {
        let dom = manuk_html::parse(
            r#"<title>T</title><style>.x{}</style>
               <body><h1>Hello</h1>
               <p>The <em>quick</em> brown <a href="/f">fox</a>.</p>
               <p>—</p><p>2024</p>
               <script>var x = "not text";</script></body>"#,
        );
        let blocks = extract_blocks(&dom);
        let texts: Vec<&str> = blocks.iter().map(|b| b.marked_text.as_str()).collect();

        // <title>, <style>, <script> excluded; letter-free blocks skipped.
        assert_eq!(texts, vec!["Hello", "The <0>quick</0> brown <1>fox</1>."]);
        assert_eq!(blocks[1].inline_nodes.len(), 2);
    }

    /// E3 acceptance: visible text is translated in place and **structure is preserved**
    /// — the `<a href>` survives with its attribute and its position in the sentence.
    #[tokio::test]
    async fn a_page_is_translated_in_place_with_structure_preserved() {
        let mut dom = manuk_html::parse(
            r#"<body><h1>Hello</h1><p>The <em>quick</em> brown <a href="/fox">fox</a>.</p></body>"#,
        );
        let backend = Dict(vec![
            ("Hello", "Bonjour"),
            (
                "The <0>quick</0> brown <1>fox</1>.",
                "Le <1>renard</1> brun <0>rapide</0>.",
            ),
        ]);

        let report = translate_page(&mut dom, &backend, "en", "fr")
            .await
            .unwrap();
        assert_eq!(report.blocks_total, 2);
        assert_eq!(report.blocks_translated, 2);
        assert!(report.skipped.is_empty());

        let h1 = dom.find_first("h1").unwrap();
        assert_eq!(dom.text_content(h1).trim(), "Bonjour");

        // The paragraph's text is translated AND reordered as the target language needs.
        let p = dom.find_first("p").unwrap();
        let text = dom.text_content(p);
        assert!(text.contains("Le renard brun rapide."), "got {text:?}");

        // The <a> kept its href and now holds the translated word.
        let a = dom.find_first("a").unwrap();
        assert_eq!(dom.element(a).unwrap().attr("href"), Some("/fox"));
        assert_eq!(dom.text_content(a), "renard");
        // The <em> likewise.
        let em = dom.find_first("em").unwrap();
        assert_eq!(dom.text_content(em), "rapide");

        // Serializing shows the markup survived, not just the text.
        let body = dom.find_first("body").unwrap();
        let html = manuk_html::serialize_inner(&dom, body);
        assert!(html.contains(r#"<a href="/fox">renard</a>"#), "got {html}");
        assert!(html.contains("<em>rapide</em>"), "got {html}");
    }

    /// If the model mangles the placeholders we must NOT guess — leave the block alone
    /// and report it. A silently corrupted DOM is worse than an untranslated paragraph.
    #[tokio::test]
    async fn a_block_with_mangled_placeholders_is_left_untouched_and_reported() {
        let mut dom = manuk_html::parse(r#"<body><p>A <b>bold</b> claim.</p></body>"#);
        // The model dropped the placeholder entirely.
        let backend = Dict(vec![(
            "A <0>bold</0> claim.",
            "Une affirmation audacieuse.",
        )]);

        let report = translate_page(&mut dom, &backend, "en", "fr")
            .await
            .unwrap();
        assert_eq!(report.blocks_translated, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("leaving it untranslated"));

        // The original DOM is intact — the <b> is still there with its text.
        let b = dom.find_first("b").unwrap();
        assert_eq!(dom.text_content(b), "bold");
        let p = dom.find_first("p").unwrap();
        assert!(dom.text_content(p).contains("A bold claim."));
    }

    /// A backend failure on one block does not abort the page.
    #[tokio::test]
    async fn a_backend_error_skips_only_that_block() {
        let mut dom = manuk_html::parse("<body><h1>Hello</h1><p>Untranslatable</p></body>");
        let backend = Dict(vec![("Hello", "Bonjour")]);
        let report = translate_page(&mut dom, &backend, "en", "fr")
            .await
            .unwrap();
        assert_eq!(report.blocks_total, 2);
        assert_eq!(report.blocks_translated, 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("backend error"));

        let h1 = dom.find_first("h1").unwrap();
        assert_eq!(dom.text_content(h1).trim(), "Bonjour");
    }

    #[test]
    fn placeholder_indices_reads_only_opening_tags() {
        assert_eq!(placeholder_indices("a <0>x</0> b <1>y</1>"), vec![0, 1]);
        assert_eq!(placeholder_indices("<1>y</1> then <0>x</0>"), vec![1, 0]);
        assert_eq!(placeholder_indices("no tags"), Vec::<usize>::new());
        // `</0>` alone must not count as a placeholder.
        assert_eq!(placeholder_indices("broken </0>"), Vec::<usize>::new());
    }
}
