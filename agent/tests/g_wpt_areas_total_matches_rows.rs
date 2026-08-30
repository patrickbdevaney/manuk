//! **G_WPT_AREAS_TOTAL_MATCHES_ROWS — the primary metric's headline must equal its own table.**
//!
//! `docs/loop/WPT-AREAS.tsv` is the source of the loop's PRIMARY per-tick metric (owner decision
//! 2026-08-11: *"the monotonic WPT total"*). It is a hand-edited table with a `TOTAL` row, and
//! nothing checked that the row was the sum.
//!
//! ## ⚠⚠⚠ IT WAS NOT, AND THE DRIFT WAS MINE
//!
//! t1381 and t1382 each updated the `css/css-overflow` row after re-running the area — 481 → 508 →
//! 513 — and neither updated `TOTAL`. So for four ticks the headline read **268 passes behind its
//! own table**, in the direction that flatters nothing but is simply wrong:
//!
//! ```text
//!   TOTAL as written    484601 / 1281896
//!   sum of the rows     484869 / 1281970
//! ```
//!
//! > **A DERIVED FIGURE THAT IS STORED RATHER THAN COMPUTED NEEDS A CHECK, AND "I WILL REMEMBER TO
//! > UPDATE BOTH" IS NOT ONE.** The loop's own doctrine already says a metric a human maintains
//! > drifts; this is that, on the metric the whole board ranks by.
//!
//! ## WHAT THIS ASSERTS
//!
//! ```text
//!   1  TOTAL.pass  == sum of every row's pass
//!   2  TOTAL.total == sum of every row's total
//!   3  every row's pct is its own pass/total, to 0.05
//!   4  no area name appears twice
//! ```
//!
//! ⭐ Row 3 is not redundant with rows 1–2: a row can be internally consistent and still be summed
//! wrong, and a row can sum right and carry a stale percentage. The percentage is what the board
//! PRINTS, so a stale one mis-ranks a directory even when the totals are clean.
//!
//! ⭐ Row 4 is the one that catches an APERTURE mistake rather than an arithmetic one: adding a tree
//! that is already measured under another spelling double-counts it into the monotonic total, which
//! would look exactly like progress.
//!
//! ⚠ This gate reads a FILE, not the engine — it is an instrument gate, and it is here because
//! `agent/tests` is in the wall's crate list and `docs/` is checked in beside it.

use std::path::PathBuf;

fn areas_tsv() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the agent crate has a parent directory")
        .join("docs/loop/WPT-AREAS.tsv")
}

#[test]
fn the_wpt_total_is_the_sum_of_its_rows() {
    let path = areas_tsv();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("VACUOUS: cannot read {}: {e}", path.display()));
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    assert!(
        header.starts_with("area\tpass\ttotal\tpct"),
        "VACUOUS: {} does not have the expected header, so the columns below are being read by \
         position from a file with a different shape. Got: {header:?}",
        path.display()
    );

    let mut rows: Vec<(String, u64, u64, f64)> = Vec::new();
    let mut total: Option<(u64, u64)> = None;
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        assert!(
            f.len() >= 4,
            "VACUOUS: row {line:?} has {} columns, not the 4+ this gate reads",
            f.len()
        );
        let parse = |i: usize| -> u64 {
            f[i].parse()
                .unwrap_or_else(|_| panic!("row {line:?}: column {i} is not a number"))
        };
        if f[0] == "TOTAL" {
            assert!(total.is_none(), "two TOTAL rows in {}", path.display());
            total = Some((parse(1), parse(2)));
        } else {
            rows.push((
                f[0].to_string(),
                parse(1),
                parse(2),
                f[3].parse().unwrap_or(-1.0),
            ));
        }
    }

    // ── VACUITY. A table with a handful of rows would satisfy the arithmetic below and prove
    //    nothing about the metric this gate exists to protect.
    assert!(
        rows.len() >= 20,
        "VACUOUS: only {} area rows — the primary metric is bigger than that, so this file is not \
         the one this gate is about",
        rows.len()
    );
    let (tp, tt) = total.unwrap_or_else(|| panic!("VACUOUS: no TOTAL row in {}", path.display()));

    // 4. No area counted twice — an aperture mistake, not an arithmetic one, and it would look
    //    exactly like progress in a MONOTONIC total.
    let mut seen = std::collections::HashSet::new();
    for (name, _, _, _) in &rows {
        assert!(
            seen.insert(name.clone()),
            "G_WPT_AREAS_TOTAL: area {name:?} appears twice. A tree measured under two spellings is \
             double-counted into a total the ratchet reads as monotonic"
        );
    }

    // 3. Every row's percentage is its own arithmetic. The board PRINTS this column, so a stale one
    //    mis-ranks a directory even when the sums are clean.
    for (name, p, t, pct) in &rows {
        if *t == 0 {
            continue;
        }
        let want = 100.0 * (*p as f64) / (*t as f64);
        assert!(
            (want - pct).abs() < 0.05,
            "G_WPT_AREAS_TOTAL: row {name:?} says {pct}% but {p}/{t} is {want:.2}%"
        );
    }

    // 1 + 2. The headline is the sum.
    let sp: u64 = rows.iter().map(|r| r.1).sum();
    let st: u64 = rows.iter().map(|r| r.2).sum();
    assert_eq!(
        tp, sp,
        "G_WPT_AREAS_TOTAL: the TOTAL row claims {tp} passing subtests and its own rows sum to \
         {sp}. This is the loop's PRIMARY metric; a stored derived figure that nothing recomputes \
         drifts every time one row is refreshed and the other is not."
    );
    assert_eq!(
        tt, st,
        "G_WPT_AREAS_TOTAL: the TOTAL row claims {tt} subtests and its own rows sum to {st}"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  change any area row's `pass` without touching TOTAL (which is exactly what t1381 and t1382
//     did, twice, unnoticed)
//       -> the sum assertion fails and names both numbers.
// N2  change a row's `pct` column alone
//       -> the per-row arithmetic check fails and the two sums stay green: the printed column is a
//          separate claim from the counts.
// N3  duplicate an existing area row
//       -> the duplicate check fires BEFORE the sums, which would otherwise still agree with each
//          other while double-counting the tree.
