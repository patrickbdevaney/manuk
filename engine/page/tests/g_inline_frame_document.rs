//! **G_INLINE_FRAME_DOCUMENT — an `<iframe>` with nothing to FETCH got nothing to LOAD, and
//! `typeof null === 'object'` is why nobody noticed.**
//!
//! `pending_iframes` is a *fetch* work-list, and it skips `srcdoc`, `src="about:blank"` and an
//! `<iframe>` with no `src` — correctly, because there is nothing to fetch. Nothing then loaded them
//! either. HTML §4.8.5 says an `<iframe>` with no `src` is **immediately navigated to `about:blank`**
//! and gets a fully-formed, same-origin document; ours got none, so `contentDocument` was `null`.
//!
//! ⚠ **The failure was invisible to feature detection, by construction.** Measured against headless
//! Chrome on one fixture, before the fix:
//!
//! ```text
//!   Chrome  dyn.contentDocument=object  dyn.doc.body=object  srcdoc.contentDocument=object  late.getById=found
//!   Manuk   dyn.contentDocument=object  dyn.doc.body=n/a     srcdoc.contentDocument=object  late.getById=no-doc
//! ```
//!
//! Every `typeof f.contentDocument === 'object'` check passed — and the next line threw
//! `can't access property "body", f1.contentDocument is null`. `typeof null` is `'object'`, so a
//! `null` document types exactly like a present one. That is the false-presence class this project
//! keeps meeting: the API answers YES and delivers nothing.
//!
//! **Why a document a page makes for itself is not a niche.** A hidden `about:blank` frame is the
//! standard way to obtain a *pristine* `window` (libraries lift unpatched natives out of one), to
//! sandbox untrusted markup, to relay `postMessage`, to host an OAuth or payment bridge — and,
//! measured on `www.welt.de` this session, to run an **ad-bait test**: create a frame, write
//! ad-shaped markup into its `contentDocument`, and see whether it survives. A frame with no document
//! fails that test the same way an ad blocker does, and welt blanks itself in response.
//!
//! `srcdoc` is the same mechanism with the markup inline, and it is what sandboxed previews,
//! documentation embeds and mail clients ship. It beats `src` per spec; this file's neighbour had
//! said so in a comment for as long as the code ignored it.
//!
//! ⚠ **NAMED RESIDUAL, PINNED BY ASSERTION (4).** These load on the host's next round, not
//! synchronously inside the `appendChild` that created the frame — so a script that appends a frame
//! and reads `contentDocument` **on the very next line** still sees `null`, while one that reads at
//! `DOMContentLoaded`, `load` or any later task sees a real document. Chrome has it immediately.
//! Closing that means building a child document from inside a JS binding, which is a different
//! change; the pin means it cannot land silently or be forgotten.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<div id="srcdoc-read">-</div>
<div id="blank-read">-</div>
<div id="nosrc-read">-</div>
<div id="sync-read">-</div>
<div id="parse-read">-</div>
<div id="src-read">-</div>
<div id="srcload-read">-</div>
<iframe id="f-srcdoc" srcdoc="<html><body><p id='inner'>hello from srcdoc</p></body></html>"></iframe>
<iframe id="f-blank" src="about:blank"></iframe>
<iframe id="f-nosrc"></iframe>
<iframe id="f-src" src="https://never.invalid/never-resolves" onload="window.__srcLoads=(window.__srcLoads||0)+1"></iframe>
<script>
  // (5) THE MOMENT THE PAGE'S OWN CODE LOOKS. Every assertion below this one reads at `load`;
  // this one reads HERE, in the parse-time script, which is the only moment WPT's viewport-unit
  // fixtures ever look — and the moment an ad-bait probe, an OAuth bridge and a pristine-window
  // lift all look too. All three frame kinds, and a WRITE, because a null-typed stub passes a
  // read-only check (`typeof null === 'object'`).
  (function () {
    var r = [];
    // ⚠ srcdoc is READ, never written. Assertions (1)-(3) below run at `load` against these same
    // three frames, and the first version of this probe wrote into all three — which DELETED the
    // `<p id=inner>` assertion (1) reads and turned a passing gate red. A shared fixture makes a
    // new WRITING assertion able to invalidate an older READING one; reading here is also the
    // stronger check for srcdoc, because it proves the markup was parsed, not merely that a
    // document exists.
    var fd = document.getElementById('f-srcdoc').contentDocument;
    var inner = fd && fd.getElementById && fd.getElementById('inner');
    r.push('f-srcdoc=' + (fd ? (inner ? inner.textContent : 'no-element') : 'NULL'));
    // The other two start empty by definition, so a WRITE is the only thing that can tell a live
    // document from a stub — `typeof null === 'object'` is what hid the original gap. (2) rewrites
    // this frame's innerHTML wholesale at `load`, so this cannot collide with it.
    ['f-blank', 'f-nosrc'].forEach(function (id) {
      var f = document.getElementById(id);
      var d = f && f.contentDocument;
      if (!d) { r.push(id + '=NULL'); return; }
      if (!d.body) { r.push(id + '=nobody'); return; }
      d.body.innerHTML = '<b class="probe">x</b>';
      r.push(id + '=wrote' + d.querySelectorAll('.probe').length);
    });
    document.getElementById('parse-read').textContent = r.join(' ');
  })();

  // (6) ⭐ THE FOURTH KIND, WHICH WAS NOT A KIND (t1350): a frame that HAS a `src` and has not
  //     navigated to it yet. HTML gives it a child browsing context at INSERTION, holding
  //     `about:blank`, and swaps it when the `src` lands. Read at PARSE TIME, before any fetch could
  //     possibly have returned — which is the only moment the distinction exists.
  (function () {
    var f = document.getElementById('f-src');
    var d = f && f.contentDocument;
    var bits = [];
    bits.push('cd=' + (d === null ? 'NULL' : (d === undefined ? 'UNDEF' : 'doc')));
    // ⚠ A WRITE, for the reason (2) gives: `typeof null === 'object'`, so a read-only check passes
    //   on the very stub this gate exists to refuse.
    if (d && d.body) { d.body.innerHTML = '<i class="pv">x</i>'; bits.push('wrote' + d.querySelectorAll('.pv').length); }
    else { bits.push('nobody'); }
    // …and it must COUNT as a child browsing context, which is the same fact one level up.
    bits.push('len=' + window.length);
    bits.push('idx=' + (window.frames[document.querySelectorAll('iframe').length - 1] ? 'obj' : 'none'));
    document.getElementById('src-read').textContent = bits.join(' ');
  })();

  // (4) THE RESIDUAL: append a frame and read it on the very next line.
  var dyn = document.createElement('iframe');
  document.body.appendChild(dyn);
  document.getElementById('sync-read').textContent =
    (dyn.contentDocument && dyn.contentDocument.body) ? 'SYNC-DOC' : 'sync-null';

  window.addEventListener('load', function () {
    function read(id, fn) {
      var f = document.getElementById(id);
      var out = 'no-doc';
      try { if (f && f.contentDocument) out = fn(f.contentDocument); } catch (e) { out = 'THROW:' + e.message; }
      return out;
    }
    document.getElementById('srcdoc-read').textContent =
      read('f-srcdoc', function (d) { var p = d.getElementById('inner'); return p ? p.textContent : 'no-element'; });
    // A blank document is not an empty object: it has a `<body>` you can write into, which is the
    // whole reason a page makes one.
    document.getElementById('blank-read').textContent =
      read('f-blank', function (d) {
        if (!d.body) return 'no-body';
        d.body.innerHTML = '<div class="bait">x</div>';
        return 'bait:' + d.querySelectorAll('.bait').length;
      });
    document.getElementById('nosrc-read').textContent =
      read('f-nosrc', function (d) { return d.body ? 'has-body' : 'no-body'; });
    // ⚠ THE COUNT OF `load` EVENTS ON THE UNRESOLVED `src` FRAME. Its provisional about:blank must
    //   fire NOTHING: Chrome's `load` for a `src`-bearing frame belongs to the document the `src`
    //   names, and this `src` never resolves, so the honest answer is ZERO.
    document.getElementById('srcload-read').textContent = 'srcLoads:' + (window.__srcLoads | 0);
  });
</script>
</body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist in the document");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — two SpiderMonkey contexts in one binary tear down messily and the
/// binary segfaults *sometimes*, which is worse than failing (see `g_defer`).
#[test]
fn a_frame_with_nothing_to_fetch_still_gets_a_document() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // Hermetic: not one of these frames touches the network, which is the entire point of them.
    let page = rt.block_on(async {
        let mut p =
            manuk_page::Page::load_async(HTML, "https://frames.test/index.html", &fonts, 800.0)
                .await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });

    let srcdoc = text(&page, "#srcdoc-read");
    let blank = text(&page, "#blank-read");
    let nosrc = text(&page, "#nosrc-read");
    let sync = text(&page, "#sync-read");
    let parse = text(&page, "#parse-read");
    let srcpre = text(&page, "#src-read");
    println!("INLINE-FRAME  srcdoc={srcdoc} blank={blank} nosrc={nosrc} sync={sync}");
    println!("INLINE-FRAME  parse-time={parse}");
    println!("INLINE-FRAME  src-pre-nav={srcpre}");

    // (1) **`srcdoc` is a document, not an attribute.** RED: drop the `srcdoc` branch from
    // `load_inline_frames` → `no-doc`. This is the case the neighbouring comment claimed for ticks.
    assert_eq!(
        srcdoc, "hello from srcdoc",
        "an <iframe srcdoc> must parse its markup into a real, readable document — got {srcdoc:?}"
    );

    // (2) **`src=\"about:blank\"` is a document you can WRITE INTO**, which is the only reason a page
    // makes one. Asserting `contentDocument != null` would pass on an empty stub; asserting a write
    // followed by a query is what proves the document is live. RED: drop the `about:blank` arm →
    // `no-doc`; return a documentless stub → `no-body`.
    assert_eq!(
        blank, "bait:1",
        "an <iframe src=about:blank> must expose a live document with a writable <body> — got {blank:?}"
    );

    // (3) **No `src` at all is the same case** (HTML §4.8.5 navigates it to `about:blank`), and it is
    // the one every hidden-frame idiom actually writes. RED: require a `src` attribute → `no-doc`.
    assert_eq!(
        nosrc, "has-body",
        "an <iframe> with NO src must still be navigated to about:blank and get a document — got {nosrc:?}"
    );

    // (6) ⭐⭐ **THE FOURTH KIND (t1350): A FRAME THAT HAS A `src` AND HAS NOT NAVIGATED TO IT.**
    //
    // HTML gives an `<iframe>` a child browsing context at INSERTION, holding `about:blank`, and
    // swaps it when the `src` lands. This engine built nothing until the fetch returned, so a
    // parse-time script read `contentDocument === null` — and `typeof null === 'object'`, which is
    // this file's own opening sentence, arriving in the one frame kind the fixture did not cover.
    // Chrome-measured on a `src` that never resolves, one such frame beside one bare frame:
    //
    // ```text
    //                                  Chrome    before
    //   s.contentDocument === null      false      true
    //   window.length                       2         1
    //   typeof window[1]               object  undefined
    // ```
    //
    // The WRITE is load-bearing for the reason (2) gives, and `len=4` is the same fact one level up:
    // no document means no child browsing context, so the frame was invisible to `window.length`
    // and `window.frames[i]` (t1349).
    assert_eq!(
        srcpre, "cd=doc wrote1 len=4 idx=obj",
        "an <iframe src> must expose a live, WRITABLE about:blank at parse time and count as a child \
         browsing context — got {srcpre:?}. `cd=NULL` is the pre-t1350 state; `nobody` is a stub \
         that types like a document and is not one; `len=3` means it was built but not counted."
    );

    // ⚠⚠ **AND IT MUST ANNOUNCE NOTHING.** Chrome fires no `load` for the initial `about:blank` of a
    // `src`-bearing frame; the event belongs to the document the `src` names, and this `src` never
    // resolves. Firing it would be worse than the silence it replaced: `<iframe onload>` is how every
    // embed, ad slot, payment frame and OAuth bridge decides it may start talking, and a frame that
    // announces readiness before a byte has arrived hands them an empty document to talk to.
    let srcload = text(&page, "#srcload-read");
    assert_eq!(
        srcload, "srcLoads:0",
        "an <iframe src> whose src never resolves must fire `load` ZERO times — got {srcload:?}. \
         `srcLoads:1` means the provisional about:blank announced itself."
    );

    // ⚠⚠⚠ **AND THE CONTROL THAT MAKES THE FIX SAFE, WHICH IS THE WHOLE RISK OF IT.** `Page::iframes`
    // is the "already rendered" marker `pending_iframes` reads. If the provisional document had been
    // entered there, the real fetch would SKIP the frame — every `<iframe src>` on the web would
    // silently stop loading, and every assertion above would still pass, because they all read the
    // about:blank this tick installs. So assert the frame is STILL PENDING A FETCH.
    let pending: Vec<String> = page
        .pending_iframes()
        .into_iter()
        .map(|(_, url, _, _)| url)
        .collect();
    assert!(
        pending.iter().any(|u| u.contains("never.invalid")),
        "G_INLINE_FRAME_DOCUMENT: `#f-src` must STILL be pending a fetch after its provisional \
         about:blank was installed — got pending={pending:?}. An empty list here means the \
         provisional document claimed the frame in `Page::iframes`, which is the one way this fix \
         breaks the entire web while leaving every other assertion in this file green."
    );

    // (4) **THE RESIDUAL IS CLOSED, AND THIS ASSERTION IS NOW A CLAIM (t1299).**
    //
    // It read `assert_eq!(sync, "sync-null")` from t512 until t1299 — an honest pin on a known gap,
    // carrying its own instruction: *"if this now reads `SYNC-DOC`, that half landed; update the
    // gate."* It read `SYNC-DOC`. The gate went red on a fix, exactly as designed, which is the
    // whole reason a known gap is pinned with an assertion rather than a comment.
    //
    // HTML §4.8.5 navigates a srcless `<iframe>` to `about:blank` **when it is inserted**, so
    // `appendChild(f); f.contentDocument` is a synchronous read of a document that already exists.
    // Built lazily, at the read: `el_content_document` asks the host to build the frame when
    // `IFRAME_DOCS` has no entry for it, so a frame nobody reads costs nothing and nothing hooks
    // `appendChild`.
    //
    // ⚠ This is the **ad slot** (create a frame, write the creative into it), the OAuth /
    // 3-D-Secure bridge, the sandboxed preview, and the pristine-`window` lift every library uses.
    //
    // RED: drop the `ensure_frame_doc` call in `el_content_document` -> `sync-null`, the value this
    // line asserted for 787 ticks.
    assert_eq!(
        sync, "SYNC-DOC",
        "a frame appended and read on the NEXT LINE must already have a document (HTML §4.8.5 \
         navigates a srcless <iframe> to about:blank at INSERTION). Got {sync:?}"
    );

    // (5) **THE PARSE-TIME READ — t1297.** Assertions (1)-(3) all read at `load`, and every one of
    // them passed for 785 ticks while this read `NULL NULL NULL`. The frames were built by
    // `load_inline_frames`, reachable only from `fetch_and_load_iframes`, which runs *after*
    // `DOMContentLoaded` — so the document's own blocking scripts, which `from_dom` runs, looked at
    // an empty `IFRAME_DOCS`. The work was done, correctly, one phase too late for the page to see
    // it: t1296's finding in a second subsystem.
    //
    // ⚠ The write is the assertion, not the non-null check. `typeof null === 'object'` is what made
    // the original gap invisible, so a `bait1` — innerHTML in, `querySelectorAll` out — is what
    // distinguishes a live document from a stub that types like one.
    //
    // RED: delete the `build_inline_frame_docs` call in `from_dom` → `NULL NULL NULL`. RED: keep it
    // but drop `publish_pre_script_frame_docs` → the same, because the documents exist and JS is
    // never told which arena is behind which element.
    assert_eq!(
        parse, "f-srcdoc=hello from srcdoc f-blank=wrote1 f-nosrc=wrote1",
        "a parser-inserted <iframe> must have a live, WRITABLE document by the time the very next \
         <script> runs — HTML §4.8.5 navigates it to about:blank at insertion. Got {parse:?}"
    );
}
