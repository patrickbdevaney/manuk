//! **G_A11Y_DOCUMENT_ROOT_NAME — the root node's accessible name is the document title, and ours was
//! empty.**
//!
//! It is the FIRST thing an assistive technology announces about a page, and the first thing an agent
//! reads out of `observe()` — *"which page am I on"*. Every document answered `""`.
//!
//! Found in the ranked remainder of t1404's real-site node-match survey: 5 misses across 6 corpus
//! pages, one per page, because there is exactly one root. Small in count and first in reading order.
//!
//! Headless Chrome 145.0.7632.116, one fixture per row, through CDP `Accessibility.getFullAXTree`:
//!
//! ```text
//!   <title>nfc</title>                 RootWebArea name='nfc'
//!   <title>  Padded Title  </title>    RootWebArea name='Padded Title'     ← trimmed
//!   no <title> at all                  RootWebArea name=''
//! ```
//!
//! ⚠ **One measurement is recorded and deliberately NOT emulated.** A `<title>` containing a NEWLINE
//! came back from Chrome as the page's URL rather than as its text, while a single-line title with the
//! same leading and trailing spaces trims correctly — so it is the newline that changes the answer and
//! nothing here explains why. Inventing a URL-fallback rule from one unexplained data point is how a
//! gate pins the engine to a guess (t1004), and a URL is not a useful name for an agent either. The
//! observation lives in `document_name`'s doc comment so the next reader has it without re-taking it.

use manuk_a11y::build_tree;

fn root_name(html: &str) -> String {
    let dom = manuk_html::parse(html);
    build_tree(&dom).name
}

#[test]
fn the_document_root_is_named_by_its_title() {
    assert_eq!(
        root_name("<!doctype html><html><head><title>nfc</title></head><body><p>x</p></body></html>"),
        "nfc",
        "the root node's name is the document title — the first thing an AT announces and the first \
         thing an agent reads about a page. It was empty for every document."
    );
    assert_eq!(
        root_name(
            "<!doctype html><html><head><title>  Padded Title  </title></head><body><p>x</p></body></html>"
        ),
        "Padded Title",
        "and it is NORMALISED, exactly as Chrome trims it — a name an agent matches on cannot carry \
         the author's incidental whitespace."
    );
    // ── THE CONTROL. Without it the fix passes by naming every document something.
    // ⭐ THE CONTROL CARRIES AN `<h1>`, AND A GREEN MUTATION IS WHY. The first version of this arm
    // was a title-less document containing only a `<p>`, and the mutation "read the first `<title>`
    // OR `<h1>`" passed it — because in a document that HAS a title the title is in `<head>` and
    // therefore always earlier in document order, so no arm above could tell the two rules apart.
    // The discriminating fixture is the one with a heading and NO title.
    assert_eq!(
        root_name("<!doctype html><html><body><h1>A Heading</h1><p>no title here</p></body></html>"),
        "",
        "CONTROL: no `<title>` means no name (Chrome-measured), EVEN WITH a heading present. A root \
         named from the first heading, from the body text, or from the URL is a name the page never \
         gave itself — and only this arm can see the difference."
    );
    // ── AND THE TITLE MUST COME FROM `<title>`, NOT FROM THE FIRST TEXT IN THE DOCUMENT.
    assert_eq!(
        root_name(
            "<!doctype html><html><head><title>Real Title</title></head><body><h1>A Heading</h1></body></html>"
        ),
        "Real Title",
        "CONTROL: a document with both a title and a heading takes the TITLE — reading the first \
         heading instead would pass the arms above and be wrong on every real page."
    );
}
