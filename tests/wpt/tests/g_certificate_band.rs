//! **G_CERTIFICATE_BAND — the headline is an integer, a reader treats an integer as exact, and the
//! same binary does not give the same integer twice.**
//!
//! t1410 ran an identical 40-site slice of the CrUX corpus through the same binary three times, with
//! nothing changed between runs:
//!
//! ```text
//!   run1   scored 26   shape>=0.75  12          (raw)
//!   run2   scored 24   shape>=0.75  12
//!   run3   scored 24   shape>=0.75  13
//!   the certificate's own headline over the three:  scored 17..19 · shape>=0.75 9..11
//!   2 sites flip scored/unscored · 1 site flips the PASS
//!   shape spread up to 0.630 on one site (www.otomoto.pl: 0.000 / 0.000 / 0.630)
//!   median spread 0.002 — most sites are stable, and the tail is what moves the count
//! ```
//!
//! ⭐⭐⭐ **A BAND OF TWO SITES ON A FORTY-SITE SLICE, AND t1406 REPORTED SIXTY-ONE TICKS AS "+1
//! SITE".** The 200-site corpus contains five such slices. That reading is inside the instrument's own
//! scatter, and nothing in the tool said so — because it prints one integer per line.
//!
//! ⚠ Both scored/unscored flips were `oracle-timeout` rows: the REFERENCE browser's variance, which
//! t1409 taught the watchdog to name one tick earlier. Without that attribution they read as the
//! engine getting worse between two runs of the same binary — the two ticks compose, and neither is
//! complete alone.
//!
//! The rule this puts in the tool: **a delta no larger than the band is not a movement.** `<=`, not
//! `<`: a delta exactly equal to the observed scatter has been produced by doing nothing at all,
//! three times, on this very corpus.

use manuk_wpt::fidelity::{certificate_band, CertBand, Fidelity, Unmeasurable};

fn scored(name: &str, shape: f64) -> Fidelity {
    let mut f = Fidelity::unmeasured(name, Unmeasurable::RenderFailed);
    f.unmeasurable = None;
    f.shape = Some(shape);
    f.shape_n = 40;
    f
}

fn unscored(name: &str) -> Fidelity {
    Fidelity::unmeasured(name, Unmeasurable::OracleTimeout(150))
}

#[test]
fn the_headline_carries_its_own_run_to_run_band() {
    // ── 1. THREE RUNS THAT DISAGREE BY ONE PASS. The shape of the real experiment: a stable core,
    // one site that flips the floor, one that flips scored/unscored.
    let runs = vec![
        vec![
            scored("a", 0.90),
            scored("b", 0.80),
            scored("flip", 0.76),
            scored("gone", 0.99),
        ],
        vec![
            scored("a", 0.90),
            scored("b", 0.80),
            scored("flip", 0.74),
            unscored("gone"),
        ],
        vec![
            scored("a", 0.91),
            scored("b", 0.81),
            scored("flip", 0.74),
            unscored("gone"),
        ],
    ];
    let band = certificate_band(&runs);
    assert_eq!(
        band.runs, 3,
        "the band must know how many runs it is made of"
    );
    assert_eq!(
        band.scored,
        (3, 4),
        "`gone` is scored in one run and oracle-timed-out in two — the SCORED count is itself a band, \
         and it was being reported as an integer."
    );
    assert_eq!(
        band.passes,
        (2, 4),
        "and the headline moves with it: 4 passes in run 1, 2 in the others."
    );
    assert_eq!(band.width(), 2, "the half-width IS the refusal threshold");

    // ── 2. THE RULE. A delta inside the band is not a movement — and `<=` is deliberate.
    assert!(
        band.delta_is_noise(2),
        "a delta EQUAL to the band is noise: this instrument produced exactly that much scatter \
         doing nothing at all, which is what the three runs above are."
    );
    assert!(
        band.delta_is_noise(-2),
        "and it is symmetric — a LOSS inside the band is not a loss either"
    );
    assert!(
        !band.delta_is_noise(3),
        "a delta LARGER than the band is a movement and must not be explained away — the point is to \
         refuse over-claiming, not to make every number unfalsifiable."
    );

    // ── 3. THE CONTROL THAT MATTERS MOST: A SINGLE RUN REFUSES NOTHING. An unrepeated sweep has no
    // evidence about its own scatter, and a band of zero that silently rejected small deltas would be
    // strictly worse than the missing band it replaces.
    let one = certificate_band(&runs[..1].to_vec());
    assert_eq!(one.runs, 1);
    assert_eq!(one.width(), 0);
    assert!(
        !one.delta_is_noise(1) && !one.delta_is_noise(0),
        "ONE run cannot refute anything. `delta_is_noise` must be false for every delta when there \
         is nothing to compare — otherwise the band becomes a licence to dismiss real movement."
    );

    // ── 4. AND NO RUNS AT ALL IS NOT A PANIC. The certificate is arithmetic over files that may be
    // missing; it must degrade, not fall over.
    let none = certificate_band(&[]);
    assert_eq!(
        none,
        CertBand {
            runs: 0,
            scored: (0, 0),
            passes: (0, 0)
        }
    );
    assert!(!none.delta_is_noise(0));

    // ── 5. IDENTICAL RUNS GIVE A ZERO-WIDTH BAND — the arm that keeps arms 1-2 from passing on a
    // band that is just "always 2".
    let same = vec![runs[1].clone(), runs[1].clone(), runs[1].clone()];
    let b = certificate_band(&same);
    assert_eq!(b.width(), 0, "three identical runs disagree about nothing");
    assert!(
        !b.delta_is_noise(1),
        "and with a zero-width band, a one-site delta IS a movement."
    );
}
