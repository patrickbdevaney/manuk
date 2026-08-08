//! **G_FORM — the browser must be writable.**
//!
//! Forms are **50% of the oracle corpus** (`docs/loop/CAPABILITIES.md`, measured over 237 real sites),
//! and they are the difference between a *reader* and a *browser*: without them you cannot search, log
//! in, or buy anything.
//!
//! The load-bearing assertion here is (3), and it is not about forms at all — it is about **not doing
//! the thing the author cancelled**. A form on a React/Vue/Svelte page is not submitted by the browser:
//! the page listens for `submit`, calls `preventDefault()`, and does its own `fetch`. With no `submit`
//! event ever dispatched, that handler never ran — so we performed the **full GET navigation the author
//! had explicitly cancelled**, throwing away the page and everything the user had typed. From the user's
//! side the site "reloads itself" whenever anyone presses a button, and nothing in any log says why.
//!
//! The rest is the serialization details that servers actually branch on, and that nobody would guess:
//!
//!   - A checked checkbox with **no `value`** submits the string **`"on"`** — not `""`. "The box was
//!     ticked" arriving as an empty string reads at the far end as "ticked, and the user typed nothing".
//!     Those are different claims.
//!   - An **unchecked** box is not a successful control at all: it contributes *nothing*, which is again
//!     different from contributing an empty string.
//!   - `application/x-www-form-urlencoded` encodes a space as **`+`**, not `%20`. `encodeURIComponent`
//!     alone gets this wrong — quietly, and only for values containing spaces, which is the worst
//!     possible distribution for a bug.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <form id="f" action="/search" method="get">
      <input name="q" value="rust">
      <input type="checkbox" name="safe" checked>
      <input type="checkbox" name="off">
      <input type="submit" value="Go">
    </form>
    <div id="probe"><input id="gd"><input id="g1" size="1"><input id="g20" style="font-size:20px"><input id="gcb" type="checkbox"><input id="gcb2" type="checkbox"><input id="grd" type="radio"><textarea id="gta"></textarea><textarea id="gta2" style="width:100px;height:40px;border:0"></textarea></div>
    <div id="bl0" style="font:16px/20px monospace"><span id="bs0">xx</span><input id="bi0" style="width:50px;padding:0;border:0"></div>
    <div id="bl1" style="font:16px/20px monospace"><span id="bs1">xx</span><input id="bi1" style="width:50px;height:6px;padding:0;border:0"></div>
    <div id="bl2" style="font:16px/20px monospace"><span id="bs2">xx</span><input id="bi2" style="width:50px;height:30px;padding:0;border:0"></div>
    <div id="bl3" style="font:16px/20px monospace"><span id="bs3">xx</span><span id="bx3" style="display:inline-block;width:50px;height:15px;font:13.333px Arial">Ay</span></div>
    <div id="bl4" style="font:16px/20px monospace"><span id="bs4">xx</span><input id="bi4" style="width:50px;height:0;padding:5px;border:0"></div>
    <div id="bl5" style="font:16px/20px monospace"><span id="bs5">xx</span><input id="bi5" type="checkbox" style="margin:0"></div>
    <div id="bl6" style="font:16px/20px monospace"><span id="bs6">xx</span><input id="bi6" style="width:50px;height:0;padding:0;border:0;line-height:30px"></div>
    <div id="out">-</div>
    <div id="fired">no</div>
    <script>
      var r = [];
      var f = document.getElementById('f');
      var fd = new FormData(f);
      r.push('q:' + (fd.get('q') === 'rust'));
      // A CHECKED box with no `value` is "on"; an UNCHECKED box is absent entirely.
      r.push('checked_on:' + (fd.get('safe') === 'on'));
      r.push('unchecked_absent:' + (fd.get('off') === null));
      // form-urlencoded: a space is `+`.
      r.push('urlencode_plus:' + (new URLSearchParams({a: 'x y'}).toString() === 'a=x+y'));
      r.push('formdata_serializes:' + (fd.toString().indexOf('q=rust') >= 0));
      // The interception EVERY modern form performs.
      f.addEventListener('submit', function (e) {
        e.preventDefault();
        document.getElementById('fired').textContent = 'yes';
      });
      r.push('submit_is_fn:' + (typeof f.submit === 'function'));
      r.push('requestSubmit_is_fn:' + (typeof f.requestSubmit === 'function'));
      document.getElementById('out').textContent = r.join(' ');
    </script>
  </body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    page.dom().text_content(hits[0])
}

/// One test, on purpose — two SpiderMonkey contexts in one binary tear down messily and segfault
/// nondeterministically, and a flaky gate gets ignored. (See `g_defer`.)
#[test]
fn forms_serialize_correctly_and_submit_is_cancellable() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);

    // (1)+(2) Serialization — the details a server branches on.
    let got = text(&page, "#out");
    for claim in [
        "q:true",                // a plain named control
        "checked_on:true",       // a checked box with no `value` is "on", not ""
        "unchecked_absent:true", // an unchecked box is not a successful control at all
        "urlencode_plus:true",   // form-urlencoded: a space is `+`, not %20
        "formdata_serializes:true",
        "submit_is_fn:true",
        "requestSubmit_is_fn:true",
    ] {
        assert!(
            got.contains(claim),
            "G_FORM: expected {claim} in {got:?}\n  \
             Forms are 50% of the corpus. These are the details servers actually branch on."
        );
    }

    // (3) **THE one.** A `submit` event fires, and `preventDefault()` is honoured — so the browser does
    //     NOT navigate. Without this, every AJAX form on the web performs the full page navigation its
    //     author explicitly cancelled, and the user loses what they typed while nothing says why.
    let root = page.dom().root();
    let form = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    let proceed = page.dispatch_submit(form, &fonts, 800.0);

    assert_eq!(
        text(&page, "#fired"),
        "yes",
        "G_FORM: the page's `submit` listener never ran. A form on any modern framework is submitted by \
         the PAGE, not the browser — with no event, its handler is dead code."
    );
    assert!(
        !proceed,
        "G_FORM: the page called preventDefault() and the browser is going to navigate ANYWAY.\n  \
         This throws away the page and everything the user typed, and does it for the majority of forms \
         on the web — every one that submits over fetch. From the user's side the site 'reloads itself' \
         whenever they press a button."
    );

    // ── (4) **THE CONTROL'S BOX ITSELF — Chrome's UA metrics, and the two constants that used to
    //        cancel** (t1043). `<button>` and `<input>` are the corpus's #1 and #2 constructs
    //        (55.6% / 51.5%, `docs/loop/CORPUS-CONSTRUCTS.md`), so a control that is the wrong size
    //        is a `dx`/`dy` error on more of the burndown corpus than any other single box.
    //
    //        ⚠ This rides in the EXISTING test rather than a new one, deliberately: the file
    //        comment above is not decoration — two SpiderMonkey contexts in one binary tear down
    //        messily and segfault nondeterministically, so this asserts against the SAME `page`.
    //
    //        Every number is headless Chrome's on this exact markup (`getBoundingClientRect`), not
    //        derived — the t1007 failure mode is a gate whose reference value was reasoned, which
    //        turns the next correct fix into a red wall:
    //
    // ```text
    //   <input>                                          205x21     was 205x19
    //   <input size=1>                                    53x21     was  53x19
    //   <input style="font-size:20px">                   303x29     was 305x27   <- the second point
    //   <input type=checkbox> / <input type=radio>        13x13     was  15x15
    //   <textarea>                                       182x36     was 182x36   CONTROL
    //   <textarea style="width:100px;height:40px;border:0">
    //                                                    104x44     was 104x42
    //   two adjacent checkboxes, x-delta                     20     was  13
    // ```
    //
    //        ⚠⚠⚠ **`<input>` at 205 and `<input style="font-size:20px">` at 303 are ONE claim with
    //        TWO points, and only the second one discriminates.** The old sheet paired a 1px border
    //        with an intrinsic-width intercept of 2.925; Chrome's pairs a 2px border with 2.75.
    //        `2.925·fs + 6` and `2.75·fs + 8` are equal at `fs = 13.333` and nowhere else — so the
    //        205 row was exact under BOTH models for the life of the sheet, and t1038 could measure
    //        the border as wrong and correctly decline to change it alone. **Asserting only the
    //        default size would leave this gate green against the defect it is named for.**
    //
    //        ⚠⚠ `<textarea>` at 182x36 is a CONTROL and is asserted as such: it was exact
    //        THROUGHOUT, because a missing 1px of vertical UA padding and a `+ 2.0` addend in the
    //        `rows` height formula cancelled. Only the author-height row (104x44) can see it.
    //
    //        **How it goes RED:** revert any one of `input:not(...){border-width:2px}`, the 2.75
    //        intercept, `input[type=checkbox]{box-sizing:border-box}`, its `margin`, or
    //        `textarea{padding:2px}` (with the addend) in `engine/css/src/stylo_engine.rs`.
    let rects = page.node_rects();
    let root = page.dom().root();
    let box_of = |sel: &str| {
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        let r = rects
            .get(&n)
            .expect("a form control must have a layout box");
        (r.width, r.height, r.x)
    };
    for (sel, w, h, why) in [
        ("#gd", 205.0, 21.0, "a default text field"),
        (
            "#g1",
            53.0,
            21.0,
            "`size=1` — the intercept, with the slope out of the way",
        ),
        (
            "#g20",
            303.0,
            29.0,
            "⚠ THE SECOND POINT. A 1px border reads 301 here, the old 2.925 intercept reads 305, \
             and the two together read 304.5 — while ALL THREE read 205 on #gd. This is the row \
             that tells the models apart",
        ),
        (
            "#gcb",
            13.0,
            13.0,
            "a checkbox is 13x13 BORDER box — the 1px border we draw (Chrome \
                              paints natively and declares none) made it 15x15 under content-box",
        ),
        ("#grd", 13.0, 13.0, "…and so is a radio"),
        (
            "#gta",
            182.0,
            36.0,
            "CONTROL — exact before and after; the missing UA padding and the height addend \
             cancelled here, which is why the defect needed the row below to be visible at all",
        ),
        (
            "#gta2",
            104.0,
            44.0,
            "an AUTHOR-specified height is the only row a cancelling pair cannot hide behind",
        ),
    ] {
        let (gw, gh, _) = box_of(sel);
        assert!(
            (gw - w).abs() < 0.51 && (gh - h).abs() < 0.51,
            "G_FORM: {sel} laid out at {gw}x{gh}; headless Chrome measures {w}x{h} — {why}"
        );
    }
    // The MARGIN half, read as a delta so it needs no absolute position: Chrome's checkbox carries
    // `margin: 3px 3px 3px 4px`, so two adjacent boxes sit 13 + 3 + 4 = 20 apart. Ours were zero,
    // which is what put every "☐ Remember me" label 4px left of where Chrome puts it — and a row of
    // controls ACCUMULATES it rather than sharing one constant offset.
    let gap = box_of("#gcb2").2 - box_of("#gcb").2;
    assert!(
        (gap - 20.0).abs() < 0.51,
        "G_FORM: two adjacent checkboxes are {gap}px apart; Chrome measures 20 (13px box + 3px \
         margin-right + 4px margin-left). With no UA margin at all this reads 13."
    );

    // ── (5) **WHERE A TEXT FIELD SITS IN A LINE OF TEXT** (t1044). An `<input>`'s inner editor has
    //        a real line in it and no box in our tree, so CSS 2.1 §10.8.1's baseline search found
    //        nothing and took the fallback — the bottom margin edge — putting every default text
    //        field **3px high in every line of text it appears in**, on 51.5% of the burndown corpus.
    //
    //        ⚠⚠⚠ **`#bx3` IS THE CONTROL AND IT IS THE SAME BOX WITH THE TEXT PUT BACK.** A
    //        50x15 `inline-block` with `font: 13.333px Arial` and a word in it — the input's exact
    //        geometry and exact font — was **already Chrome-exact at `dy 3`** throughout. That is
    //        what makes this "the rule we have, applied to a line the DOM does not contain" rather
    //        than a new baseline rule, and it is why the fix is four lines.
    //
    //        ⚠⚠ **`#bs2` IS THE LOAD-BEARING ROW, AND IT IS NOT THE CONTROL'S OWN BOX.** A baseline
    //        is a property of the LINE: when the field is 30px tall its baseline sits below the
    //        strut's, so Chrome pushes the **text beside it** down 4.5px. Asserting only the input's
    //        own `dy` would pass with the line still wrong, which is the whole failure mode here —
    //        a mis-baselined control displaces its neighbours, and that is what `reading_order`
    //        counts.
    //
    // ```text
    //   line, 16px/20px monospace (strut baseline 15.0)     Chrome    before
    //     <input> natural h=15                    its dy      3.0       0.0
    //     <input height:6px>                      its dy      7.5       9.0
    //     <input height:30px>                     its dy      0.0       0.0   ← unmoved…
    //       …the TEXT beside it                   its dy      4.5       0.0   ← …and this is not
    //     <span inline-block 50x15 Arial>Ay</span> its dy      3.0       3.0   CONTROL
    // ```
    //
    //        **How it goes RED:** delete the `.or_else(|| self.form_control_inner_baseline(…))` in
    //        `engine/layout/src/lib.rs` (all four rows revert), or top-align instead of centring the
    //        synthetic line (`#bi1` reads 3.0 and `#bi0` stays right — the ladder's middle rung is
    //        the one a natural-height fixture would have produced, and it cannot tell the two
    //        models apart).
    for (sel, dy, why) in [
        (
            "#bi0",
            3.0,
            "a natural-height text field sits ON the text baseline, not below the line",
        ),
        (
            "#bi1",
            7.5,
            "a SHORT field — the synthetic line is CENTRED in the content box, so it drops. \
             Top-aligning reads 3.0; the old bottom-margin-edge fallback reads 9.0",
        ),
        ("#bi2", 0.0, "a TALL field pins the top of the line"),
        (
            "#bs2",
            4.5,
            "⚠ THE LOAD-BEARING ROW: the TEXT BESIDE the 30px field. A baseline belongs to the \
             LINE, so a mis-baselined control displaces its neighbours — which is exactly what \
             `reading_order` counts. This reads 0.0 with the field's own dy still correct",
        ),
        (
            "#bx3",
            3.0,
            "CONTROL — the input's exact box and font as a plain inline-block WITH text. It was \
             Chrome-exact before this tick and must stay so: the fix is the existing rule reaching \
             a line the DOM does not contain, not a new rule",
        ),
        // ⚠⚠⚠ THE NEXT TWO EXIST BECAUSE THE FIRST FIVE LEFT TWO HALVES OF THE FIX
        // UNFALSIFIABLE. Running the mutations found that swapping the CONTENT box for the
        // BORDER box, and widening the rule to EVERY input type, both left this gate GREEN —
        // t834's rule: falsify every half separately, including the ones that look obviously
        // right. Each row below is the cheapest markup that separates one of them, and both
        // numbers are Chrome's.
        (
            "#bi4",
            5.5,
            "⚠ THE FRAME ROW: `height:0;padding:5px` — a box whose content and border heights \
             DIFFER while the baseline still lands under the strut. The synthetic line is centred \
             in the CONTENT box (0 tall, offset 5) → 9.5; centring it in the BORDER box (10) reads \
             14.5 and this row goes to dy 0.5. Chrome measures the same 5.5 with `border:5px` and \
             with `height:4px;padding:3px`, so it is the content box three ways",
        ),
        (
            "#bi5",
            2.0,
            "⚠ THE TYPE ROW: a checkbox has NO inner text line — it is a tick box — so it must \
             keep §10.8.1's bottom-margin-edge fallback, which was already Chrome-exact for it. \
             Giving it the synthetic line moves it. `range`/`color`/`file`/`image` are excluded \
             for the same reason and are separately WRONG in this engine (see the journal): they \
             have no widget at all, which is a different tick and not a baseline question",
        ),
        (
            "#bi6",
            11.0,
            "⚠ THE LEADING ROW: `line-height:30px` on a zero-height field. Chrome moves the \
             baseline from 10.5 to 11.0 — half a pixel for 24px of extra leading, i.e. it centres \
             the inner editor's TEXT (ascent+descent) and not a leaded line box. Centring \
             `line_height` instead sends this to dy 18. The first version of the fix had \
             `line-height` in twice, where it CANCELLED, and the mutation pass is what found it",
        ),
    ] {
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        let parent = page.dom().parent(n).expect("a probe row has a container");
        let (r, p) = (
            rects.get(&n).expect("the probe must have a layout box"),
            rects.get(&parent).expect("its row must have one too"),
        );
        let got = r.y - p.y;
        assert!(
            (got - dy).abs() < 0.51,
            "G_FORM: {sel} sits {got}px down its line; headless Chrome measures {dy} — {why}"
        );
    }
}
