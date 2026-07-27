//! **G_SCRIPT_ERROR_HAS_A_LOCATION — an uncaught page-script error must say WHERE it happened.**
//!
//! `G_SILENT_FAIL` established that an error on the load/render/script path must never be swallowed.
//! This is the step after it, and the gap between them is where several ticks have been spent: the
//! error was surfaced and could not be acted on.
//!
//! ```text
//!   before   a page <script> threw  error=TypeError: can't access property "length", t is undefined
//! ```
//!
//! That is a sentence with no address. On minified production JavaScript — which is what the web is —
//! `t` is every variable on the page. `www.agoda.com` fails to render behind exactly this throw, and
//! the report gave nothing to pull on. **The engine knew precisely where it happened and had nobody
//! to tell**, which is the same shape `G_SILENT_FAIL` forbids one step earlier: *an error that is
//! REPORTED but not ATTRIBUTABLE is a status, not a finding.* `manuk-wpt diag` exists because a status
//! told this project nothing three separate times while it guessed at causes.
//!
//! `pending_exception` stringified the thrown value and stopped, discarding the `fileName`,
//! `lineNumber` and `stack` that SpiderMonkey had already attached. It lifts them now.
//!
//! Hermetic: an inline document, no network, and the throw is on a line this file chooses.

use manuk_text::FontContext;

/// The throw is inside a named function, and the function is called from a different line, so both
/// halves of the claim have something specific to match: a line that is not the script's first, and a
/// frame name only a real stack could produce.
///
/// ⚠ The numbering is **relative to the inline script**, not to the document — SpiderMonkey compiles
/// each inline `<script>` as its own source (`inline.js`), so the throw is at `inline.js:2` and its
/// caller at `inline.js:3`. Asserting a document-relative line here would have been a gate written
/// from an assumption about someone else's numbering.
const HTML: &str = r##"<!doctype html><html><body>
<script>
  function theFrameThatThrew() { var t; return t.length; }
  theFrameThatThrew();
</script>
</body></html>"##;

/// Capture what the engine logged while `f` ran. `tracing` is how every one of these reports reaches
/// a human, so the gate asserts on the same channel a developer reads rather than on an internal.
fn logs_from(f: impl FnOnce()) -> String {
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = Sink(buf.clone());
    let sub = tracing_subscriber::fmt()
        .with_writer(move || sink.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    tracing::subscriber::with_default(sub, f);
    let v = buf.lock().unwrap().clone();
    String::from_utf8_lossy(&v).to_string()
}

#[test]
fn an_uncaught_script_error_names_its_line_and_carries_a_stack() {
    let tmp = std::env::temp_dir().join(format!("manuk-errloc-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let fonts = FontContext::new();
    let logged = logs_from(|| {
        let _ = manuk_page::Page::load(HTML, "https://example.invalid/index.html", &fonts, 800.0);
    });
    println!("SCRIPT-ERROR PROBE:\n{logged}");

    // ── PRECONDITION: the script really did throw and the throw really was reported. Without this a
    //    green could mean "nothing ran", which is a green about the empty case.
    assert!(
        logged.contains("threw") && logged.contains("TypeError"),
        "the gate's own fixture failed: no uncaught TypeError was reported at all, so this run never \
         produced the report whose CONTENT is under test.\n--- log ---\n{logged}"
    );

    // ── THE CLAIM, part 1: a line number. `2` is the throw and `3` is its caller, script-relative
    //    (see the note above); either identifies the site, and neither is line 1 — which is what a
    //    report that had invented a location rather than read one would say.
    assert!(
        logged.contains(":2:") || logged.contains(":3:"),
        "G_SCRIPT_ERROR_HAS_A_LOCATION: the reported error names no line. `pending_exception` \
         stringifies the thrown value; SpiderMonkey has already attached `fileName`/`lineNumber` and \
         they must be lifted, or every throw on minified production JavaScript is unactionable — the \
         engine knows where it happened and tells nobody.\n--- log ---\n{logged}"
    );

    // ── THE CLAIM, part 2: a real stack, not a fabricated location. A frame name can only come from
    //    the engine's own `Error.stack`, so this cannot be satisfied by formatting a guess.
    assert!(
        logged.contains("theFrameThatThrew"),
        "the report carries no stack frame — `Error.stack` was not lifted, so the report can say \
         WHERE but not THROUGH WHAT, and a bug two frames below its throw site stays \
         unattributable.\n--- log ---\n{logged}"
    );
}
