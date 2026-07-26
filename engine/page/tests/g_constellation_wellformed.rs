//! **G_CONSTELLATION_WELLFORMED — the capability map must be MACHINE-READABLE, not just written.**
//!
//! `docs/loop/CONSTELLATION.tsv` is the loop's model of what this browser can do. Every consumer of
//! it — the lever board, `phase0-progress.sh`, the surface audit, and every judgement made from a
//! `--gaps` listing — parses it **by column**. So a row whose columns are wrong is not a typo: it is
//! a capability the loop cannot see, and it fails **silently**, because a TSV with a shifted field
//! still parses into *something*.
//!
//! ## What surface audit #33 found, which is why this gate exists
//!
//! Two rows had been joined by a missing newline, producing an **11-field row**. The second half was
//! the tick-587 `G_STORAGE_PATCHABLE` row — a landed capability, with a receipt, **invisible to
//! every column-based reader for nine ticks**, because it was living in fields 7-11 of its
//! neighbour. There was also a stray empty line and a row whose status read `measured`, which is not
//! one of the five values anything downstream understands.
//!
//! None of that is catchable by reading the file: it renders fine, the prose is all present, and the
//! defect is entirely in the *shape*. It is the same class as the tick-113 lesson that a ledger's ✅
//! was never tested — **a document the loop reasons from needs a test like any other input.**
//!
//! ## Why the assertions are what they are
//!
//! - **Field count.** Six columns, exactly, on every row. This is the one that catches the join.
//! - **Status vocabulary.** Five values. Anything else silently drops out of every tally, so a
//!   capability with a creative status is a capability nobody counts.
//! - **No blank rows**, which shift nothing but do inflate every `NR`-based line reference.
//! - **Named gates exist on disk.** A row claiming `gated` by a gate file that is not there is the
//!   honest-answer rot this repo has been bitten by twice — a claim outliving its evidence.

use std::collections::HashSet;

const STATUSES: [&str; 5] = ["gated", "works", "partial", "missing", "unknown"];

fn map_path() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is engine/page.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/loop/CONSTELLATION.tsv")
}

#[test]
fn the_capability_map_is_machine_readable() {
    let text = std::fs::read_to_string(map_path()).expect("CONSTELLATION.tsv must be readable");
    let mut rows = 0usize;
    let mut bad_shape = Vec::new();
    let mut bad_status = Vec::new();
    let mut blank = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if n == 1 {
            continue; // header
        }
        if line.trim().is_empty() {
            blank.push(n);
            continue;
        }
        rows += 1;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            bad_shape.push((
                n,
                f.len(),
                f[1..].first().copied().unwrap_or("").to_string(),
            ));
            continue;
        }
        if !STATUSES.contains(&f[3]) {
            bad_status.push((n, f[3].to_string(), f[1].to_string()));
        }
    }

    println!("CONSTELLATION: {rows} capability rows");

    assert!(
        bad_shape.is_empty(),
        "every row must have exactly 6 tab-separated fields. A row with more is TWO ROWS JOINED by a \
         missing newline, and the second one is invisible to every column-based reader — which is \
         how a landed capability (t587's G_STORAGE_PATCHABLE) sat unseen for nine ticks. \
         Offenders (line, field-count, text): {bad_shape:?}"
    );
    assert!(
        bad_status.is_empty(),
        "status must be one of {STATUSES:?}. Any other value silently drops out of every tally, so \
         the capability stops being counted by anything — including the readiness percentage the \
         phase gate is judged on. Offenders: {bad_status:?}"
    );
    assert!(
        blank.is_empty(),
        "blank rows shift every line-number reference into the map and mean nothing. Lines: {blank:?}"
    );
    assert!(
        rows > 300,
        "the map has {rows} rows — a sudden collapse means a truncating write, not a real deletion"
    );
}

// ── WHY THERE IS NO "every cited gate exists" TEST HERE, and what it would take ────────────────
//
// The obvious companion assertion is that a `gated` row names a gate that exists. It was written,
// it found two real defects (rows citing `g_mse_join` — prose from an unrelated MEDIA row — for
// cross-document View Transitions and for promise-returning scroll methods, neither of which it
// tests), and then it was **deleted rather than tuned**, which is the part worth recording.
//
// It cannot be made precise against the map as it stands. The gate column's vocabulary is genuinely
// heterogeneous by design: file-backed gates (`G_CLIP_PATH`), crate-internal unit-test function
// names (`user_select_computed_value_reflects_the_cascade`), perf floors (`F1/F2`), bare subsystem
// names (`video_decode`), and multi-gate expressions (`G_IFRAME + G_IFRAME_RERENDER`). Every version
// of the check that admitted all of those also admitted the prose that caused the bug, and every
// version that rejected the prose also rejected a few dozen legitimate rows.
//
// **A gate tuned until it is green is the thing this repo refuses**, so this is left undone and
// named instead of shipped weak. Making it real needs a canonical gate registry — one list, emitted
// by the harness, that every gate registers into and the map cites BY KEY. That is a genuine
// improvement (it would also make `verify.sh`'s coverage countable, which memory records as an open
// question: "gated" is not the same as "watched"), and it is observer-owned territory.
//
// The shape test above is kept because it is the one with no judgement in it, and it is the one that
// caught the consequential defect: a nine-tick-old joined row hiding an entire landed capability.
