//! **G_CANVAS_IMAGE — `ctx.drawImage` composites real pixels, from both kinds of source.**
//!
//! `drawImage` was an *honest* no-op (`event_loop.rs`: "no image source plumbing yet"), and honest is
//! the right word — it never claimed to work. But it is the same worst-shape failure `G_CANVAS` was
//! built for: a page feature-detects canvas, is told **yes**, blits its sprite sheet / thumbnail /
//! chart glyph, and nothing appears, with no error thrown. Sprite games, image editors, thumbnail
//! grids, PDF.js-style tile renderers and every chart library that draws a legend icon all funnel
//! through this one method.
//!
//! The gap it closed was **directional**. Canvas had only ever pushed pixels *outward*
//! (`canvas_bitmaps` → the image map the painter reads). `drawImage` is the first operation needing
//! host pixels to come *in* — the decoded bytes of an `<img>` that a script cannot produce for itself.
//! So this gate deliberately exercises **both** source kinds, because they travel different paths:
//!
//!   · **canvas → canvas** reads the live `CANVASES` registry (the double-buffer idiom).
//!   · **`<img>` → canvas** requires `Page::publish_image_sources` to have run *before* the script.
//!
//! Deleting the publish hook leaves every canvas→canvas claim GREEN and fails only `img*`, which is
//! what makes the two independent rather than one claim asserted twice.
//!
//! Every claim below is a **pixel read back**, never an API shape. `typeof ctx.drawImage === 'function'`
//! was true for the entire life of the no-op.

use manuk_text::FontContext;

/// 4×4, four hard quadrants: TL red, TR green, BL blue, BR yellow.
///
/// Deliberately ASYMMETRIC in both axes. A symmetric fixture cannot tell a correct draw from one that
/// is mirrored, transposed, or crops the wrong corner — every one of those bugs would pass.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAGklEQVR42mP4z8AARAz/kRCMAtP//wMxKgcAyncT7YAZSdwAAAAASUVORK5CYII=";

fn html() -> String {
    format!(
        r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<img id="pic" src="{PNG}">
<img id="bad" src="https://nowhere.invalid/never.png">
<canvas id="c" width="100" height="100"></canvas>
<canvas id="s" width="40" height="40"></canvas>
<canvas id="c2" width="50" height="50"></canvas>
<script>
  var R = [];
  var c = document.getElementById('c'), x = c.getContext('2d');
  var s = document.getElementById('s'), sx = s.getContext('2d');
  var c2 = document.getElementById('c2'), x2 = c2.getContext('2d');

  function px(ctx, cx, cy) {{
    var d = ctx.getImageData(cx, cy, 1, 1).data;
    return [d[0], d[1], d[2], d[3]];
  }}
  // Sampled well inside each quadrant, so bilinear blending at the seams cannot decide a claim.
  function near(p, r, g, b) {{
    return Math.abs(p[0]-r) < 40 && Math.abs(p[1]-g) < 40 && Math.abs(p[2]-b) < 40 && p[3] > 200;
  }}

  // The SOURCE canvas: the same four quadrants as the PNG, at 20px each.
  sx.fillStyle = '#ff0000'; sx.fillRect(0, 0, 20, 20);
  sx.fillStyle = '#00ff00'; sx.fillRect(20, 0, 20, 20);
  sx.fillStyle = '#0000ff'; sx.fillRect(0, 20, 20, 20);
  sx.fillStyle = '#ffff00'; sx.fillRect(20, 20, 20, 20);

  // ── 1. canvas -> canvas, 1:1, three arguments. No scaling, so the colours must come back EXACT.
  x.drawImage(s, 0, 0);
  R.push('blit:' + (near(px(x,10,10),255,0,0) && near(px(x,30,10),0,255,0) &&
                    near(px(x,10,30),0,0,255) && near(px(x,30,30),255,255,0)));

  // ── 2. Nine-argument SOURCE CROP. Take only the top-RIGHT (green) quadrant. If the crop were
  //       ignored, the whole 4-quadrant image would land here and (10,60) would read red instead.
  x.drawImage(s, 20, 0, 20, 20, 0, 50, 20, 20);
  R.push('crop:' + (near(px(x,10,60),0,255,0) ? 'green' : px(x,10,60).join(',')));

  // ── 3. Five-argument SCALE, 40x40 source down into a 20x20 box. Quadrants must survive the
  //       resample in the right places — proof the dst/src ratio is applied, not just the offset.
  x.drawImage(s, 50, 0, 20, 20);
  R.push('scale:' + (near(px(x,53,3),255,0,0) && near(px(x,67,3),0,255,0) &&
                     near(px(x,53,17),0,0,255)));

  // ── 4. NEGATIVE destination width MIRRORS. This is how a sprite sheet draws a character facing
  //       the other way, and it is the one case where a negative extent is not a no-op. Left half
  //       must now be GREEN (the top-right quadrant reflected across).
  x.drawImage(s, 50 + 20, 50, -20, 20);
  R.push('flip:' + (near(px(x,53,53),0,255,0) && near(px(x,67,53),255,0,0)));

  // ── 5. globalAlpha composites the image rather than being ignored.
  x.globalAlpha = 0.5;
  x.drawImage(s, 0, 0, 20, 20, 0, 80, 15, 15);
  x.globalAlpha = 1.0;
  var a = px(x, 7, 87);
  R.push('alpha:' + (a[3] > 90 && a[3] < 165));

  // ── 6. The context transform applies to the destination. Without it every sprite draws at 0,0.
  //
  //       All FOUR quadrants are checked, not just one, and that is deliberate. Checking a single
  //       corner cannot tell a correctly-transformed draw from a DOUBLY-transformed one: applying the
  //       matrix to both the path and the pattern pushes the sampled region off the image entirely, and
  //       the `Pad` spread then clamps every pixel to the source's top-left texel — which is red, so a
  //       lone `near(...,255,0,0)` reads GREEN on a genuinely broken draw. Demanding all four distinct
  //       quadrants makes a flat clamped fill impossible to mistake for an image.
  x.save(); x.translate(80, 80);
  x.drawImage(s, 0, 0, 10, 10);
  x.restore();
  R.push('xform:' + (near(px(x,82,82),255,0,0) && near(px(x,88,82),0,255,0) &&
                     near(px(x,82,88),0,0,255) && near(px(x,88,88),255,255,0) &&
                     px(x,84,60)[3] === 0));

  // ── 7. An image that never decoded draws NOTHING and does not throw. Per spec this is a silent
  //       no-op — a page that blits a broken thumbnail must not lose the rest of its render.
  var threw = false;
  try {{ x.drawImage(document.getElementById('bad'), 0, 95, 5, 5); }} catch (e) {{ threw = true; }}
  R.push('undecoded:' + (!threw && px(x, 2, 97)[3] === 0));

  // ── 8. THE <img> PATH. A different journey entirely: these pixels came off the network and through
  //       the image decoder, and only reach a script because the host published them BEFORE it ran.
  var pic = document.getElementById('pic');
  x2.drawImage(pic, 0, 0, 40, 40);
  R.push('imgblit:' + (near(px(x2,8,8),255,0,0) && near(px(x2,30,8),0,255,0) &&
                       near(px(x2,8,30),0,0,255) && near(px(x2,30,30),255,255,0)));
  // The same crop machinery on an <img>: bottom-left (blue) quadrant only.
  x2.drawImage(pic, 0, 2, 2, 2, 0, 42, 8, 8);
  R.push('imgcrop:' + (near(px(x2,4,46),0,0,255) ? 'blue' : px(x2,4,46).join(',')));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##
    )
}

#[test]
fn draw_image_composites_from_canvas_and_img_sources() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html(), "https://canvas.test/", &fonts, 400.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "blit:true",
            "canvas -> canvas at 1:1 is the double-buffer idiom every game loop and every chart \
             redraw uses. It reads the LIVE canvas registry; a stale snapshot would composite the \
             previous frame",
        ),
        (
            "crop:green",
            "the nine-argument source rect is how a SPRITE SHEET works — one image, many frames. \
             Ignoring the crop draws the whole sheet into the frame's box, and every sprite in the \
             game becomes the entire sheet shrunk down",
        ),
        (
            "scale:true",
            "dst/src ratio must be applied. Without it the image lands at intrinsic size and every \
             thumbnail grid overflows its cells",
        ),
        (
            "flip:true",
            "a NEGATIVE destination extent MIRRORS, it does not no-op. On the SOURCE rect a negative \
             extent merely re-anchors the same region — conflating the two is the classic drawImage \
             bug, and it silently leaves every sprite facing the same way",
        ),
        (
            "alpha:true",
            "globalAlpha must reach the blit. A fade-in that jumps straight to opaque is the visible \
             symptom, and it looks like a timing bug rather than a compositing one",
        ),
        (
            "xform:true",
            "the context matrix applies to the destination rect — otherwise every drawImage lands at \
             0,0 regardless of translate/scale, which is `G_CANVAS`'s `xform` claim all over again",
        ),
        (
            "undecoded:true",
            "an image that has not decoded draws nothing and throws nothing. Throwing would take out \
             the rest of the script; drawing garbage would be worse",
        ),
        (
            "imgblit:true",
            "THE HOST->JS DIRECTION. These pixels came off the network and through the decoder, and \
             exist for the script only because `publish_image_sources` ran BEFORE it. Delete that hook \
             and every canvas->canvas claim above still passes while this one fails",
        ),
        (
            "imgcrop:blue",
            "an <img> crops through the same path a canvas does — one implementation, not two that \
             drift",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CANVAS_IMAGE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
