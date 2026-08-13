//! # G_ALPHA_SERIALIZATION — `rgba(0, 0, 0, 0.5)` read back as `rgba(0, 0, 0, 0.5019608)`
//!
//! Alpha is stored as a `u8` — `0.5` becomes `128` — and the serializer did `128 as f32 / 255.0`,
//! which is `0.5019608`. So **every translucent colour on every page failed its own round trip**:
//!
//! ```text
//!   el.style.color = 'rgba(0, 0, 0, 0.5)'
//!   getComputedStyle(el).color              →  "rgba(0, 0, 0, 0.5019608)"
//! ```
//!
//! Comparing the string you wrote against the string you read back is how a library detects its own
//! write, and translucent colour is not a corner of the web — it is every overlay, every disabled
//! control, every shadow, every hover tint. This is t1205's `object-position` defect (`20% 30%` →
//! `20% 30.000002%`) one property family wider, and the same class as `undefined + ' scale(2)'`:
//! **a value of the right type that no comparison will ever match.**
//!
//! ## The rule is "the SHORTEST decimal that round-trips", so the fix is a SEARCH
//!
//! CSS Color 4 serializes alpha as the shortest decimal that maps back to the same 8-bit value.
//! A fixed precision is wrong at **both** ends, which is why `alpha_css` tries 1, then 2, then 3
//! places and takes the first that re-quantises to the same byte:
//!
//! | fixed precision | what it breaks |
//! |---|---|
//! | 2 decimals | `2/255 = 0.008` becomes `0.01`, which quantises to **3**, not 2 |
//! | 6 decimals | reproduces `0.501961` — the artefact this exists to remove |
//!
//! `roundTripsEveryByte` below is the claim that makes this more than three spot checks: **all 256
//! byte values** are serialized and re-quantised, and every one must come back to itself. A search
//! that is subtly wrong at one end fails there rather than in the field.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var e = document.getElementById('out');
    function rt(v) { e.style.color = v; return getComputedStyle(e).color; }

    // ── 1. THE LOAD-BEARING CLAIM — the value the whole web writes.
    p('half:' + rt('rgba(0, 0, 0, 0.5)'));
    p('roundTrip:' + (rt('rgba(0, 0, 0, 0.5)') === 'rgba(0, 0, 0, 0.5)'));

    // ── 2. Other common alphas. `0.1` and `0.9` do not land on exact byte boundaries either.
    p('tenth:' + rt('rgba(1, 2, 3, 0.1)'));
    p('ninth:' + rt('rgba(1, 2, 3, 0.9)'));
    p('quarter:' + rt('rgba(1, 2, 3, 0.25)'));

    // ── 3. THE RATCHET — opaque still serializes as `rgb()` with no alpha at all.
    p('opaque:' + rt('rgb(1, 2, 3)'));
    p('opaqueAlpha:' + rt('rgba(1, 2, 3, 1)'));
    p('zero:' + rt('rgba(1, 2, 3, 0)'));

    // ── 4. ⚠ THE CLAIM A FIXED PRECISION FAILS. Every one of the 256 byte values must serialize to
    //    a decimal that re-quantises to itself — two decimals mangles the small end, six reproduces
    //    the artefact. This is the arm that makes the search a search.
    var bad = [];
    for (var i = 0; i < 256; i++) {
      var s = rt('rgba(1, 2, 3, ' + (i / 255) + ')');
      var m = /rgba?\([^,]+,[^,]+,[^,]+(?:,\s*([0-9.]+))?\)/.exec(s);
      var back = m && m[1] !== undefined ? Math.round(parseFloat(m[1]) * 255) : 255;
      if (back !== i) { bad.push(i + '->' + s); }
    }
    p('roundTripsEveryByte:' + (bad.length === 0 ? 'all256' : bad.slice(0, 4).join(',')));
  </script>
</body></html>"##;

#[test]
fn a_translucent_colour_survives_its_own_round_trip() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://alpha.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("ALPHA: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_ALPHA_SERIALIZATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "half:rgba(0, 0, 0, 0.5)",
        "THE LOAD-BEARING CLAIM. This read `rgba(0, 0, 0, 0.5019608)` — alpha is stored as a `u8` \
         (`0.5` → `128`) and the serializer divided it back by 255. Every overlay, disabled control, \
         shadow and hover tint on the web writes this value",
    ),
    (
        "roundTrip:true",
        "the operation the web actually performs: write, read back, compare. An inequality here \
         reads as 'the write was lost'",
    ),
    ("tenth:rgba(1, 2, 3, 0.1)", "`0.1` does not land on a byte boundary either"),
    ("ninth:rgba(1, 2, 3, 0.9)", "nor `0.9`"),
    ("quarter:rgba(1, 2, 3, 0.25)", "nor `0.25`"),
    (
        "opaque:rgb(1, 2, 3)",
        "THE RATCHET. A fully opaque colour serializes as `rgb()` with NO alpha component — adding \
         one would break every string comparison that already worked",
    ),
    ("opaqueAlpha:rgb(1, 2, 3)", "THE RATCHET. `alpha: 1` is the same as opaque"),
    ("zero:rgba(1, 2, 3, 0)", "THE RATCHET. Fully transparent keeps its `0`"),
    (
        "roundTripsEveryByte:all256",
        "⚠ THE CLAIM A FIXED PRECISION FAILS, and the reason the fix is a SEARCH rather than a \
         formula. All 256 byte values must serialize to a decimal that re-quantises to themselves: \
         two decimals mangles the small end (`2/255 = 0.008` → `0.01` → byte 3), six decimals \
         reproduces the `0.501961` artefact this gate exists to remove. Three spot checks cannot \
         see either end",
    ),
];
