//! # G_OBJECT_POSITION_COMPUTED — a percentage that does not survive its own round trip
//!
//! `getComputedStyle(img).objectPosition` answered **`20% 30.000002%`** for a sheet that said
//! `object-position: 20% 30%`. Measured, not theorised.
//!
//! `ObjectPosition` stores each axis as a free-space **fraction** — `30%` → `0.3` — because that is
//! what the paint path needs to place a cropped image. `0.3f32 * 100.0` is `30.000002`, and the
//! serializer did exactly that multiplication. **Every other percentage in this file is fine**,
//! because `Dim::Percent` stores the percentage itself; this property is the one that round-trips
//! through a fraction, which is why the defect is one property wide and invisible everywhere else.
//!
//! **Why a float artefact is a real failure and not cosmetic:** the standard way a library detects
//! its own write is to compare the string it set against the string it reads back. `"20% 30%"` and
//! `"20% 30.000002%"` are not equal, so the write looks lost and the library re-applies, or gives up.
//! It is the same class as `undefined + ' scale(2)'` producing `"undefined scale(2)"` (`G_TRANSFORM`)
//! — a value of the right *type* that no comparison will ever match.
//!
//! ## ⚠ WHAT THIS GATE DELIBERATELY DOES **NOT** CLAIM, and a correction to tick 1204
//!
//! Tick 1204 read a sample of `css/css-values` failures — all of them lengths or `calc()` — and
//! wrote that `object-position` *"is not being applied at all on the shipping (Stylo) path."*
//! **That was wrong.** A direct probe says the property applies correctly for percentages and for
//! every keyword:
//!
//! ```text
//!   object-position: 20% 30%       →  20% 30.000002%   ← applied; the float is the bug
//!   object-position: top           →  50% 0%           ← correct
//!   object-position: right bottom  →  100% 100%        ← correct
//!   object-position: 30px 50%      →  50% 50%          ← THE REAL GAP: a LENGTH is dropped
//! ```
//!
//! And the length gap is **a documented deliberate fallback, not an oversight** — the parser says so
//! in its own comment: *"percentages relative to length (px) aren't fraction-convertible without the
//! box, so they (and any unrecognized token) fall back to centered."* Carrying a length needs the
//! stored value to stop being a fraction, which is a type change with layout consumers in
//! `manuk-paint` and `manuk-layout`. It is named here and left for a tick of its own; the
//! `lengthStillFallsBackToCentre` claim below **pins the current honest behaviour** so that tick has
//! to come back and change this line on purpose.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #pct   { object-position: 20% 30%; }
  #third { object-position: 33.333% 66.667%; }
  #kw    { object-position: top; }
  #kw2   { object-position: right bottom; }
  #len   { object-position: 30px 50%; }
</style></head><body>
  <img id="pct"><img id="third"><img id="kw"><img id="kw2"><img id="len"><img id="init">
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function g(id) { return getComputedStyle(document.getElementById(id)).objectPosition; }

    // ── 1. THE LOAD-BEARING CLAIM: a percentage reads back as the percentage that was written.
    p('pct:' + g('pct'));
    p('third:' + g('third'));

    // ── 2. AND THE ROUND TRIP THE WEB ACTUALLY PERFORMS — write, read, compare.
    //    ⚠ The vacuity read comes FIRST: an image that already reported `10% 90%` would satisfy the
    //    comparison below by coincidence. (My first draft read it after the assignment, which is a
    //    vacuity guard that cannot fail.)
    var e = document.getElementById('init');
    p('initial:' + g('init').indexOf('10% 90%'));
    e.style.objectPosition = '10% 90%';
    p('roundTrip:' + (getComputedStyle(e).objectPosition === '10% 90%'));

    // ── 3. THE RATCHET: the keywords and the initial value are unchanged.
    p('kw:' + g('kw'));
    p('kw2:' + g('kw2'));

    // ── 4. THE HONEST BOUND, pinned so the tick that lifts it must edit this line.
    p('lengthStillFallsBackToCentre:' + g('len'));
  </script>
</body></html>"##;

#[test]
fn a_computed_object_position_percentage_survives_its_own_round_trip() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://objpos.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("OBJECT-POSITION: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_OBJECT_POSITION_COMPUTED: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "pct:20% 30%",
        "THE LOAD-BEARING CLAIM. This read `20% 30.000002%` — the axis is stored as a FRACTION for \
         the paint path (`30%` → `0.3`) and `0.3f32 * 100.0` is `30.000002`. A value of the right \
         type that no string comparison will ever match",
    ),
    (
        "third:33.333% 66.667%",
        "⚠ AND THE FIX MUST NOT BE A BLUNT ROUND. A genuinely fractional percentage has to survive \
         intact: rounding to 2 or 3 decimals would pass the claim above and silently destroy this \
         one",
    ),
    (
        "roundTrip:true",
        "the operation the web actually performs — write a value, read it back, compare the strings. \
         That is how every animation and layout library detects its own write, and an inequality \
         here reads as 'the write was lost'",
    ),
    ("kw:50% 0%", "THE RATCHET. `top` is `50% 0%`, unchanged"),
    (
        "kw2:100% 100%",
        "THE RATCHET. `right bottom` binds each keyword to its own axis, unchanged",
    ),
    (
        "initial:-1",
        "VACUITY GUARD: the untouched image must NOT already read `10% 90%` before the script \
         assigns it — otherwise `roundTrip` is satisfied by a coincidence",
    ),
    (
        "lengthStillFallsBackToCentre:50% 50%",
        "⚠ THE HONEST BOUND, PINNED ON PURPOSE. `object-position: 30px 50%` falls back to centred, \
         because the parser says in its own comment that a length is not fraction-convertible \
         without the box. This is a KNOWN, DOCUMENTED limit, not this tick's bug — and tick 1204's \
         journal was WRONG to call the property 'not applied at all on the Stylo path'. Carrying a \
         length means the stored value stops being a fraction, a type change with consumers in \
         `manuk-paint` and `manuk-layout`. Whoever does that tick must come back and edit this line",
    ),
];
