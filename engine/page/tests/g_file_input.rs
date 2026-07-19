//! **G_FILE_INPUT — an agent can choose a file, and the bytes reach the wire.**
//!
//! Uploading was the one common interaction on the web that had **no door**. The bytes normally
//! arrive through a native OS file-picker dialog, which has no scriptable surface, so every
//! avatar / attachment / document / photo flow was undrivable — not broken, just unreachable.
//! `Page::set_input_files` is that door, and this gate is what proves it opens onto something real.
//!
//! ## Two failures land here, and the second is the dangerous one
//!
//! 1. **`input.files` did not exist and `FileList` was an INERT stub** — a name that existed and
//!    claimed nothing. A page guarding on `input.files && input.files.length` took the "no file
//!    chosen" branch permanently: the upload button stayed disabled and **nothing threw**.
//!
//! 2. **`new FormData(form)` harvested `e.value` for every control, including `type=file`.** The
//!    spec makes a file input's `value` the deliberately-useless `C:\fakepath\a.txt`, so the field
//!    was submitted as **that literal string** and the file's bytes were dropped — one layer above
//!    `__multipart`, which was already fully capable of carrying them. This is the worse half:
//!    absence is visible, whereas an upload that "succeeds" and delivers the string
//!    `"C:\fakepath\a.txt"` where a JPEG should be is a **silent corruption** at the far end.
//!
//! So the assertions below are deliberately not "files exists". They are: the list has the right
//! LENGTH, the entries carry the right NAMES and BYTES, `files` is `null` (not empty) on a
//! non-file control, and the multipart body actually contains the file's CONTENTS. Each is a claim
//! an inert stub satisfies by accident in none of the cases.
//!
//! ## The RED probes (run, not imagined)
//!
//! * **Restoring the `e.value` harvest for `type=file` flips `mp:` and `nopath:` and NOTHING else.**
//!   `len:`, `name0:`, `type0:`, `size0:` and `value:` all stay green — the page can still see the
//!   file perfectly while the server receives the literal string `C:\fakepath\a.txt`. That isolation
//!   is the whole reason the multipart claim is a separate assertion, and it is what a gate asserting
//!   only "the page can see the file" would have reported green on.
//! * **Making `files` return an empty `FileList`** takes the gate RED at `fired:`. Recorded honestly
//!   because it falsifies through a *coarser* signal than the probe above: `fl[0].name` throws inside
//!   the change handler, so `#ev` is never written at all and the first missing claim is the one that
//!   merely says the handler ran. The gate fails — but it does not point at the real break.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<form id="f">
  <input type="file" name="avatar" id="up">
  <input type="text" name="caption" id="cap" value="hi">
</form>
<div id="out">-</div>
<div id="ev">-</div>
<script>
  var R = [];
  var $ = function (i) { return document.getElementById(i); };
  try {
    var up = $('up');
    // ── Before anything is chosen. A file input with no selection has an EMPTY list, not a
    //    missing one — the difference is what every `files.length` guard reads.
    R.push('ctor:' + (typeof FileList === 'function'));
    R.push('empty:' + (up.files && up.files.length === 0));
    // `files` is null on a control that is not a file input. NOT an empty list: pages branch on
    // `input.files === null` to tell a text field from a file field, so an empty FileList here
    // would answer "a file input with nothing chosen" about an <input type=text>.
    R.push('nullfortext:' + ($('cap').files === null));

    // ── The change handler. This is where a real upload widget lives, and it runs only because
    //    the host fired the event — the page cannot choose a file for itself.
    up.addEventListener('change', function () {
      var out = [];
      var fl = up.files;
      out.push('fired:true');
      out.push('len:' + (fl.length === 2));
      out.push('name0:' + (fl[0].name === 'a.txt'));
      out.push('name1:' + (fl[1].name === 'b.png'));
      out.push('type0:' + (fl[0].type === 'text/plain'));
      out.push('size0:' + (fl[0].size === 5));
      out.push('item:' + (fl.item(1).name === 'b.png' && fl.item(9) === null));
      // The fake path is spec, not whimsy: the real path is withheld (it leaks the user's
      // directory layout) and the `C:\fakepath\` prefix is mandated because sites already parsed
      // a Windows path out of `value`.
      out.push('value:' + (up.value === 'C:\\fakepath\\a.txt'));

      // ── THE CLAIM THIS GATE EXISTS FOR. The bytes have to survive the trip to the wire.
      var fd = new FormData($('f'));
      var body = fd.__multipart('BOUND');
      out.push('mp:' + (body.indexOf('filename="a.txt"') >= 0 &&
                        body.indexOf('hello') >= 0 &&
                        body.indexOf('filename="b.png"') >= 0));
      // The plain field still rides along — a file harvest that ate its siblings would be a
      // regression this gate would otherwise not see.
      out.push('field:' + (body.indexOf('name="caption"') >= 0 && body.indexOf('hi') >= 0));
      // And the fake path must NOT be what got sent. This is the exact bug: `value` harvested
      // instead of the file.
      out.push('nopath:' + (body.indexOf('fakepath') < 0));
      $('ev').textContent = out.join(' ');
    });

    $('out').textContent = R.join(' ');
  } catch (e) {
    $('out').textContent = 'THREW:' + e;
  }
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_visibility`, `g_mse`, `g_globals`).
#[test]
fn an_agent_can_choose_a_file_and_the_bytes_reach_the_multipart_body() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://upload.test/", &fonts, 800.0);

    let read = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        page.dom().text_content(n)
    };
    let find = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        manuk_css::query_selector_all(page.dom(), root, sel)[0]
    };

    let got = read(&page, "#out");
    for (claim, why) in [
        ("ctor:true", "FileList must be a real constructor — it was an INERT stub, a name that existed and claimed nothing"),
        ("empty:true", "a file input with no selection has an EMPTY FileList, not a missing one; `input.files.length` is the guard every upload widget writes"),
        ("nullfortext:true", "files is NULL on a non-file control — an empty list here would tell a page that an <input type=text> is a file input with nothing chosen"),
    ] {
        assert!(
            got.contains(claim),
            "G_FILE_INPUT: missing {claim:?} — {why}.\n  got: {got}"
        );
    }

    // ── The actuation. This is the half the page cannot do for itself: choosing a file is an act
    //    of the host, and before `set_input_files` there was no way to perform it.
    let up = find(&page, "#up");
    let chosen = page.set_input_files(
        up,
        &[
            (
                "a.txt".to_string(),
                "text/plain".to_string(),
                "hello".to_string(),
            ),
            (
                "b.png".to_string(),
                "image/png".to_string(),
                "PNGDATA".to_string(),
            ),
        ],
        &fonts,
        800.0,
    );
    assert!(
        chosen,
        "set_input_files must report success on an <input type=file>"
    );

    // It must REFUSE a control that cannot hold files, rather than silently storing them where
    // nothing will ever read them back.
    let cap = find(&page, "#cap");
    assert!(
        !page.set_input_files(
            cap,
            &[("x".into(), "text/plain".into(), "y".into())],
            &fonts,
            800.0
        ),
        "set_input_files must return false for a non-file input — storing files on a text field \
         would be a write that no getter can ever surface"
    );

    let ev = read(&page, "#ev");
    for (claim, why) in [
        ("fired:true", "a real picker fires input then change; a file chosen with no event leaves the page unaware and the upload button disabled"),
        ("len:true", "both files must arrive — `multiple` uploads are the common case for attachments and photo pickers"),
        ("name0:true", "the FileList entries carry their real names"),
        ("name1:true", "the SECOND entry is real too — a list that only ever fills index 0 passes a length check and fails every gallery"),
        ("type0:true", "the MIME type survives; upload widgets branch on it to reject or preview"),
        ("size0:true", "size reflects the actual bytes (\"hello\" is 5) — a size of 0 makes client-side validation reject a valid file"),
        ("item:true", "FileList.item(i) works and returns null past the end, as the indexed getter does"),
        ("value:true", "value is the spec's C:\\fakepath\\<name> — pages split it on backslash to show the filename"),
        ("mp:true", "THE CLAIM THIS GATE EXISTS FOR: the multipart body carries filename=\"a.txt\" AND the file's actual bytes. Harvesting `value` instead sent the literal string C:\\fakepath\\a.txt where a file should be — an upload that SUCCEEDS and delivers garbage"),
        ("field:true", "the ordinary text field still rides along — a file harvest that ate its siblings is a regression nothing else here would catch"),
        ("nopath:true", "the fake path must NOT appear in the body; if it does, `value` was harvested and the bytes were dropped"),
    ] {
        assert!(
            ev.contains(claim),
            "G_FILE_INPUT: missing {claim:?} — {why}.\n  got: {ev}"
        );
    }
}
