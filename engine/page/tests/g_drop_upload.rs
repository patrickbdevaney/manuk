//! **G_DROP_UPLOAD — files can be DROPPED on a page, and the dropzone sees them.**
//!
//! The other half of `G_FILE_INPUT` (tick 247). That one opened the `<input type=file>` door; this
//! one opens the dashed rectangle, which on the modern web is the *more* common of the two: Gmail
//! attachments, GitHub issue images, Slack, Drive and every uploader built in the last decade put a
//! dropzone on the screen and read `e.dataTransfer.files`.
//!
//! ## The failure was not "the drop was ignored" — the handler THREW
//!
//! `DataTransfer` was an INERT stub, so `e.dataTransfer` was `undefined` and the first line of every
//! dropzone — `e.dataTransfer.files` — was a **TypeError inside the `drop` handler**. A page that
//! throws in its drop handler does not fall back to anything: the dashed rectangle stays lit, the
//! upload never starts, and the console error is the only trace.
//!
//! ## Why all three events, and why the `dragover` claim is not ceremony
//!
//! The HTML drag protocol makes a page **opt in** to being a drop target: a dropzone that does not
//! `preventDefault()` its `dragover` never receives a `drop` at all. That is why the standard
//! dropzone is written as a *pair* of handlers, and why dispatching `drop` alone would test a path
//! **no real browser can reach**. It would also skip the `dragenter`/`dragover` handlers that set
//! the "drag active" styling every dropzone uses to highlight itself — so the visible half of the
//! interaction would silently not happen.
//!
//! The `types` claim is here for the same reason: `types.indexOf('Files') >= 0` is the standard
//! "file drag or text drag" test, and a dropzone that gets it wrong goes down the text branch and
//! reads `getData('text/plain')` instead of ever touching `files`.
//!
//! ## The RED probes (run, not imagined)
//!
//! * **Firing only `drop`** (dropping the `dragenter`/`dragover` pair) takes the gate RED at
//!   `enter:`. The files still arrive — the page simply never got to say it wanted them, which is
//!   the state a real browser would never deliver a drop into.
//! * **Building `types` as `[]` for a file drag** also goes RED at `enter:`, not at `types:` as
//!   first predicted — recorded because the prediction was wrong and the reason is worth keeping:
//!   `enter:` asserts the same `indexOf('Files')` token, so it is simply the earlier of the two
//!   claims to notice. Both probes are real falsifications; neither isolates the way
//!   `G_FILE_INPUT`'s multipart probe does, because the dropzone's handlers are sequential and the
//!   first bad read masks the rest.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="zone" style="width:200px;height:100px">drop here</div>
<div id="ev">-</div>
<script>
  var seen = [];
  var $ = function (i) { return document.getElementById(i); };
  var zone = $('zone');
  var held = null;

  // A dropzone written the way the web writes them: cancel dragover to opt in, stash the
  // DataTransfer on enter, read the files on drop.
  zone.addEventListener('dragenter', function (e) {
    // If dataTransfer is undefined this line throws, which is precisely what used to happen.
    seen.push('enter:' + (e.dataTransfer.types.indexOf('Files') >= 0));
    held = e.dataTransfer;
    e.preventDefault();
  });
  zone.addEventListener('dragover', function (e) {
    seen.push('over:true');
    e.preventDefault();   // THE OPT-IN. Without this a real browser never delivers the drop.
  });
  zone.addEventListener('drop', function (e) {
    var out = [];
    var fl = e.dataTransfer.files;
    out.push('files:' + (fl.length === 2));
    out.push('name0:' + (fl[0].name === 'shot.png'));
    out.push('type0:' + (fl[0].type === 'image/png'));
    out.push('size1:' + (fl[1].size === 5));
    // One DataTransfer for the whole sequence — a dropzone that stashed it on enter must find the
    // same object carrying the files on drop.
    out.push('same:' + (held === e.dataTransfer));
    out.push('types:' + (e.dataTransfer.types.indexOf('Files') >= 0));
    // The `items` surface must agree with `files`; a dropzone written against either one works.
    out.push('items:' + (e.dataTransfer.items.length === 2 &&
                         e.dataTransfer.items[0].kind === 'file' &&
                         e.dataTransfer.items[0].getAsFile().name === 'shot.png'));
    // A file drag carries no text; a dropzone branching on this must not get a phantom string.
    out.push('nodata:' + (e.dataTransfer.getData('text/plain') === ''));
    e.preventDefault();
    $('ev').textContent = seen.join(' ') + ' ' + out.join(' ');
  });
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_file_input`, `g_visibility`, `g_mse`).
#[test]
fn files_dropped_on_a_dropzone_arrive_through_a_real_datatransfer() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://drop.test/", &fonts, 800.0);

    let root = page.dom().root();
    let zone = manuk_css::query_selector_all(page.dom(), root, "#zone")[0];

    let proceed = page.dispatch_drop(
        zone,
        &[
            (
                "shot.png".to_string(),
                "image/png".to_string(),
                "PNGDATA".to_string(),
            ),
            (
                "notes.txt".to_string(),
                "text/plain".to_string(),
                "hello".to_string(),
            ),
        ],
        &fonts,
        800.0,
    );
    assert!(
        !proceed,
        "the dropzone called preventDefault() on the drop, so the host must NOT also perform its \
         default action — a browser that navigated to the dropped file here would replace the page \
         the user was uploading to, which is the classic 'my app vanished' drag bug"
    );

    let root = page.dom().root();
    let ev = manuk_css::query_selector_all(page.dom(), root, "#ev")[0];
    let got = page.dom().text_content(ev);

    for (claim, why) in [
        ("enter:true", "dragenter must arrive with a real DataTransfer — with the inert stub this line was `undefined.types`, a TypeError INSIDE the drop handler, so the page did not ignore the drop, it threw"),
        ("over:true", "dragover must be delivered: it is how a page OPTS IN to being a drop target, and a dropzone that never gets one never gets a drop in a real browser either"),
        ("files:true", "both dropped files arrive — multi-file drop is the common case for attachments and image uploads"),
        ("name0:true", "the entries carry their real names"),
        ("type0:true", "the MIME type survives; dropzones branch on it to reject non-images"),
        ("size1:true", "size reflects the actual bytes (\"hello\" is 5) — a 0 makes client-side validation reject a valid file"),
        ("same:true", "ONE DataTransfer across the sequence: a dropzone that stashes it on dragenter must find the same object carrying files on drop"),
        ("types:true", "types contains the literal token 'Files' — `types.indexOf('Files')>=0` is the standard file-drag test, and getting it wrong sends the handler down the TEXT branch where it never looks at files at all"),
        ("items:true", "the items surface agrees with files, so a dropzone written against getAsFile() works too"),
        ("nodata:true", "a file drag carries no text/plain; a phantom string here sends a branching dropzone down the wrong path"),
    ] {
        assert!(
            got.contains(claim),
            "G_DROP_UPLOAD: missing {claim:?} — {why}.\n  got: {got}"
        );
    }
}
