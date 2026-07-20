//! `G_CAPTION_PAINT` — the UA draws the caption itself.
//!
//! The last step of the caption arc, and the one every earlier step was waiting on. Ticks 255-260
//! built the parser, the `TextTrack` that holds cues, the `cuechange` timeline, the `<track src>`
//! fetch, and cue placement in the spec's own vocabulary. Every one of those hands cues to *a
//! renderer* — and on a plain `<video>` with `<track default>` there is no renderer, because a page
//! with no player library never draws a caption itself. The browser is supposed to. Until this
//! gate, a correctly parsed, correctly timed, correctly placed cue reached the viewer as nothing.
//!
//! What is asserted here is the DISPLAY LIST, not pixels: `DisplayItem::Text` carries a plain
//! `String` (shaping happens later, in `CpuPainter::draw_text`), so the cue's own words are visible
//! at this layer and a pixel diff would only obscure which cue landed where.
//!
//! ONE `#[test]` PER FILE — `PageContext` is per-process; a second test in the same binary races
//! the first for the thread's JS runtime and takes the process down with it (see `g_cue_change.rs`).

use manuk_paint::DisplayItem;
use manuk_text::FontContext;

const W: f32 = 800.0;

/// Every `Text` item's `(baseline, text)`, in paint order.
fn texts(list: &manuk_paint::DisplayList) -> Vec<(f32, String)> {
    list.items
        .iter()
        .filter_map(|it| match it {
            DisplayItem::Text { baseline, text, .. } => Some((*baseline, text.clone())),
            _ => None,
        })
        .collect()
}

/// The baseline of the first `Text` item containing `needle`, or `None`.
fn caption_y(list: &manuk_paint::DisplayList, needle: &str) -> Option<f32> {
    texts(list)
        .into_iter()
        .find(|(_, t)| t.contains(needle))
        .map(|(y, _)| y)
}

const HTML: &str = r#"<!doctype html>
<html><body>
<video id="v" width="640" height="360" style="background:#123456"></video>
<script>
  var v = document.getElementById('v');
  var tr = v.addTextTrack('subtitles', 'English', 'en');
  // AUTO line — the default, and it must land at the BOTTOM of the frame.
  var bottom = new VTTCue(1, 2, 'SPOKEN AT THE BOTTOM');
  // line:0 — the TOP of the frame. The author lifted it because the bottom is occupied.
  var top = new VTTCue(1, 2, 'LIFTED TO THE TOP');
  top.line = 0;
  // A hard-wrapped cue: two authored lines, two painted lines.
  var wrapped = new VTTCue(3, 4, 'FIRST HALF\nSECOND HALF');
  tr.addCue(bottom); tr.addCue(top); tr.addCue(wrapped);
  tr.mode = 'showing';
  v.currentTime = 0;        // before every cue
</script>
</body></html>"#;

#[test]
fn the_ua_paints_showing_cues_over_the_video_where_the_author_placed_them() {
    let fonts = FontContext::new();
    let mut p = manuk_page::Page::load(HTML, "https://captionpaint.test/", &fonts, W);

    // --- 0. The video box is real -------------------------------------------------------------
    // Without this the whole gate is vacuous: `caption_items` returns nothing for a zero-sized box,
    // so every "no caption" assertion below would pass for the wrong reason.
    let root = p.dom().root();
    let v = manuk_css::query_selector_all(p.dom(), root, "#v")[0];
    let vr = p
        .node_rects()
        .get(&v)
        .copied()
        .expect("the <video> has a layout box");
    assert!(
        vr.width > 100.0 && vr.height > 100.0,
        "G_CAPTION_PAINT: the <video> box is {vr:?} — a degenerate box makes this gate vacuous"
    );
    let (box_top, box_bottom) = (vr.y, vr.y + vr.height);

    // --- 1. Before the cue starts, the UA paints nothing ---------------------------------------
    let before = p.display_list();
    assert_eq!(
        caption_y(&before, "BOTTOM"),
        None,
        "G_CAPTION_PAINT: a cue that is not yet active was painted at t=0"
    );

    // --- 2. Inside the cue, both cues paint, each where the author put them ---------------------
    p.eval_for_test("document.getElementById('v').currentTime = 1.5;");
    let during = p.display_list();
    let bot = caption_y(&during, "BOTTOM")
        .expect("G_CAPTION_PAINT: the active auto-line cue was not painted at all");
    let top = caption_y(&during, "TOP")
        .expect("G_CAPTION_PAINT: the active line:0 cue was not painted at all");

    // Both inside the video box...
    for (name, y) in [("auto", bot), ("line:0", top)] {
        assert!(
            y >= box_top && y <= box_bottom,
            "G_CAPTION_PAINT: the {name} caption baseline {y} is outside the video box \
             [{box_top}, {box_bottom}] — a caption painted off the picture is not a caption"
        );
    }
    // ...and `auto` is NOT `0`. This is the assertion that catches the single most damaging way to
    // get cue placement wrong: collapsing `auto` to `0` moves every default caption in every file
    // to the top of the frame, on top of the very thing the author with `line:0` was avoiding.
    assert!(
        bot > top,
        "G_CAPTION_PAINT: the auto-line caption ({bot}) is not BELOW the line:0 caption ({top}) — \
         `auto` has been collapsed to `0` and every default caption now paints at the top"
    );
    assert!(
        top < box_top + vr.height * 0.5,
        "G_CAPTION_PAINT: `line:0` painted at {top}, not in the top half of the box"
    );
    assert!(
        bot > box_top + vr.height * 0.5,
        "G_CAPTION_PAINT: an `auto` line painted at {bot}, not in the bottom half of the box"
    );

    // --- 3. A caption sits OVER the video, never under it --------------------------------------
    // The cue is painted after the element's own blit. A cue behind the frame is a cue nobody can
    // read, which is indistinguishable from the pre-tick state of painting no cue at all.
    let cue_idx = during
        .items
        .iter()
        .position(|it| matches!(it, DisplayItem::Text { text, .. } if text.contains("BOTTOM")))
        .expect("the caption is in the list");
    // The video's OWN paint. This was first written against `DisplayItem::Image` — and a `<video>`
    // with no poster decodes no bitmap, so there was no Image item, the `if let Some` skipped the
    // whole assertion, and it passed under a probe that painted captions BEHIND the frame. An
    // assertion that cannot fail measured nothing. The element is given a background colour so it
    // has an item of its own, found by that exact colour rather than by being "the first Rect"
    // (the captions emit Rects too).
    let video_idx = during
        .items
        .iter()
        .position(|it| {
            matches!(it, DisplayItem::Rect { color, .. }
                     if color.r == 0x12 && color.g == 0x34 && color.b == 0x56)
        })
        .expect(
            "G_CAPTION_PAINT: the <video>'s own background did not paint — this gate cannot \
                 tell 'over' from 'under' without it",
        );
    assert!(
        cue_idx > video_idx,
        "G_CAPTION_PAINT: the caption paints at {cue_idx}, BEHIND the video at {video_idx}"
    );

    // --- 4. A hard-wrapped cue paints one line per authored line -------------------------------
    p.eval_for_test("document.getElementById('v').currentTime = 3.5;");
    let wrapped = p.display_list();
    let first = caption_y(&wrapped, "FIRST HALF")
        .expect("G_CAPTION_PAINT: the first line of a two-line cue was not painted");
    let second = caption_y(&wrapped, "SECOND HALF")
        .expect("G_CAPTION_PAINT: the second line of a two-line cue was not painted");
    assert!(
        second > first,
        "G_CAPTION_PAINT: the second authored line ({second}) does not paint BELOW the first \
         ({first}) — a two-line cue is stacked in the wrong order or on top of itself"
    );
    // The newline is a break, not a character: neither painted line may contain the whole cue.
    assert!(
        texts(&wrapped).iter().all(|(_, t)| !t.contains('\n')),
        "G_CAPTION_PAINT: a cue's newline reached the display list as text"
    );

    // --- 5. `hidden` means DO NOT DISPLAY, and it is not the same as `disabled` ----------------
    // The mode a player sets when it draws captions itself. Its cues stay active and `cuechange`
    // still fires — so `activeCues` is unchanged and the cue-diff sees nothing happen — but the UA
    // must stop painting. Getting this wrong doubles every caption on every site that has a player.
    p.eval_for_test("document.getElementById('v').textTracks[0].mode = 'hidden';");
    let hidden = p.display_list();
    assert_eq!(
        caption_y(&hidden, "FIRST HALF"),
        None,
        "G_CAPTION_PAINT: a `hidden` track still painted — the UA is drawing captions over a \
         player that is already drawing its own"
    );

    // ...and turning it back on brings the same cue back, with no clock movement to prompt it.
    p.eval_for_test("document.getElementById('v').textTracks[0].mode = 'showing';");
    assert!(
        caption_y(&p.display_list(), "FIRST HALF").is_some(),
        "G_CAPTION_PAINT: re-showing a track left the picture blank until the next cue boundary"
    );
}
