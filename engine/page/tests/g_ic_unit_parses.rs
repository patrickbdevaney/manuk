//! # G_IC_UNIT_PARSES — `ic`/`ric` are RECOGNISED units, and that is all this claims
//!
//! **The scope of this gate is the point of it.** t637 probed the `ic` unit, found `10ic` resolving
//! to exactly `10em`, wrote the fix that would compute the real ideographic advance, and then
//! **reverted it unshipped** — because every font on the machine measures U+6C34 at exactly 1em
//! (CJK faces are designed full-width), so a gate asserting `ic == 16.0` passes identically against
//! the unfixed engine. A gate that cannot fail the fake version of the capability is coverage
//! theatre.
//!
//! So this gate asserts the part that IS falsifiable, and refuses the part that is not:
//!
//! * **CLAIMED** — `ic` and `ric` are units this engine recognises, so a declaration using them is
//!   applied rather than dropped. That is what the constellation's `partial` means, and without
//!   this gate that `partial` is a bare assertion (which `map-reconcile.sh` correctly flagged).
//! * **NOT CLAIMED** — that `1ic` equals the styled face's real 水 advance. It does not; Stylo's
//!   `FontMetrics.ic_width` is left unset, so `ic` is the spec's `1em` fallback. Correct behaviour,
//!   asserted rather than computed, and unprovable either way on any available font.
//!
//! ## The bogus-unit control is what makes this measure anything
//!
//! `10ic` resolving to `10em` is equally consistent with *"the unit is unsupported and the whole
//! declaration was dropped"* — because a dropped `width` leaves `auto`, and the numbers only look
//! different if you check. So `#bogus { width: 10zz }` sits beside it: an unknown unit MUST be
//! dropped to `auto` and fill the container. The pair is the measurement; either line alone is not.
//!
//! ## RED probes run against this gate
//!
//! | mutation | result |
//! |---|---|
//! | `#ic`'s declaration changed to the bogus `10zz` | RED — `ic:160.0` becomes the container width |
//! | `#bogus`'s declaration changed to a valid `10em` | RED — `bogus:784.0` becomes 160, so the control proves it is a control |

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { font: 16px sans-serif; margin: 8px }
  /* CONTROL A — units resolved from REAL font metrics (t504-508). Each differs from 10em, which
     is what proves the metrics provider is wired at all and this fixture can tell units apart. */
  #ch  { width: 10ch }
  #cap { width: 10cap }
  #em  { width: 10em }
  /* UNDER TEST */
  #ic  { width: 10ic }
  #ric { width: 10ric }
  /* CONTROL B — a unit that does not exist. MUST be dropped -> width:auto -> fills the container.
     Without this line, "10ic == 10em" cannot be told from "the declaration was thrown away". */
  #bogus { width: 10zz }
</style></head><body>
  <div id="ch"></div><div id="cap"></div><div id="em"></div>
  <div id="ic"></div><div id="ric"></div><div id="bogus"></div>
  <div id="out">-</div>
  <script>
    var R = [];
    ['ch','cap','em','ic','ric','bogus'].forEach(function (id) {
      R.push(id + ':' + document.getElementById(id).getBoundingClientRect().width.toFixed(1));
    });
    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"##;

#[test]
fn ic_and_ric_are_recognised_units_not_dropped_declarations() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ic.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("IC UNIT PROBE: {got}");

    let val = |id: &str| -> f32 {
        got.split_whitespace()
            .find_map(|t| t.strip_prefix(&format!("{id}:")))
            .unwrap_or("nan")
            .parse()
            .unwrap_or(f32::NAN)
    };

    let (em, bogus) = (val("em"), val("bogus"));
    assert_eq!(
        em, 160.0,
        "10em at font-size 16px is 160px — the fixture's own arithmetic"
    );
    assert!(
        bogus > em * 2.0,
        "CONTROL: `width: 10zz` names no unit, so the declaration must be DROPPED and the block \
         fill its container ({bogus} should be ~784, not {em}). If this ever equals `em`, the \
         parser has started accepting nonsense and every claim below becomes meaningless"
    );

    for id in ["ic", "ric"] {
        let v = val(id);
        assert_eq!(
            v, em,
            "`{id}` must be RECOGNISED: the declaration is applied (so it is not {bogus}, the \
             dropped-declaration width) and resolves to Stylo's spec `1em` fallback, because \
             FontMetrics.ic_width is left unset. This gate deliberately does NOT claim the value \
             is the styled face's real U+6C34 advance — see t637: that fix was written and \
             reverted because no available font makes the two differ, so asserting it would pass \
             against the unfixed engine"
        );
    }

    // The metrics provider IS wired for the units it does compute — otherwise "ic equals em" would
    // be unremarkable, since everything would equal em.
    assert_ne!(val("ch"), em, "`ch` is a real zero-advance, not an em");
    assert_ne!(val("cap"), em, "`cap` is a real cap-height, not an em");
}
