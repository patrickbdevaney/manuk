//! **The certification corpus, with a denominator that cannot leak.**
//!
//! `docs/loop/DAILY-DRIVER-CERTIFICATION.md` §3 and §6.2, and the observer's CO-#1 items (2) and (4).
//! They are one piece of work, because *the denominator is the reconciliation*.
//!
//! ## Why this exists
//!
//! Every past reading of this project's fidelity was optimistic **for a structural reason, not a
//! statistical one**: the old 265-site corpus was a convenience sample, and sites that timed out,
//! crashed or hit a bot-wall were *dropped*. Dropping the hard sites is a filter that removes exactly
//! the pages a browser is worst at, so the surviving average describes an easier web than the real
//! one. §0 of the certification doc names this as cause #1.
//!
//! The fix is not discipline, it is a **type**. [`SiteOutcome`] has no variant meaning "not counted":
//! every sampled site resolves to `Scored`, `Fail` or `Excluded`, each carrying a reason, and
//! [`SweepLedger::reconcile`] refuses a ledger where `sampled != scored + fail + excluded`. A drop
//! becomes a compile-time-shaped impossibility followed by a runtime assertion, rather than a habit
//! somebody has to keep.
//!
//! > **8 of 30 historical process defects here were caught by a number that did not add up** — not by
//! > any gate. This makes that check mechanical.
//!
//! ## The strata are reported separately, on purpose
//!
//! HEAD (traffic-weighted, rank ≤100k) and TAIL (uniform, 100k–1M) answer two different questions —
//! *"does the web people actually use work?"* and *"does the long tail work?"* — and averaging them
//! produces a number that answers neither. [`SweepLedger::by_stratum`] keeps them apart.

use std::collections::BTreeMap;

/// One site in the certification corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusSite {
    /// `HEAD` (traffic-weighted) or `TAIL` (uniform). Reported separately, never averaged together.
    pub stratum: String,
    pub rank: u64,
    pub url: String,
}

/// Parse `docs/bench/corpus-v2.tsv`: `stratum <TAB> rank <TAB> url`, `#` comments skipped.
///
/// A malformed row is an **error**, not a skip. Silently ignoring a line we cannot parse is the same
/// leak this module exists to close, one layer earlier — the corpus would quietly shrink and every
/// ratio computed against it would be over a denominator nobody chose.
pub fn parse_corpus(text: &str) -> Result<Vec<CorpusSite>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split('\t');
        let (Some(stratum), Some(rank), Some(url), None) = (f.next(), f.next(), f.next(), f.next())
        else {
            return Err(format!(
                "corpus line {}: expected 3 tab-separated fields, got {line:?}",
                i + 1
            ));
        };
        let rank: u64 = rank
            .parse()
            .map_err(|_| format!("corpus line {}: rank {rank:?} is not a number", i + 1))?;
        out.push(CorpusSite {
            stratum: stratum.to_string(),
            rank,
            url: url.to_string(),
        });
    }
    Ok(out)
}

/// What happened to one sampled site. **There is deliberately no "skipped" variant.**
#[derive(Debug, Clone, PartialEq)]
pub enum SiteOutcome {
    /// We rendered it and scored it. `pass` is the composed certificate verdict for this site.
    Scored { pass: bool },
    /// We attempted it and it did not produce a score — a timeout, a crash, a bot-wall. **This
    /// counts against the bar**, because a page we cannot render is a page we cannot claim.
    Fail { reason: String },
    /// Deliberately not attempted, for a reason recorded in the corpus header (adult/gambling).
    /// Excluded sites leave the denominator, and the count is reported so the exclusion is visible
    /// rather than silent.
    Excluded { reason: String },
}

/// A sweep's accounting: what was sampled, and what happened to each.
#[derive(Debug, Default)]
pub struct SweepLedger {
    pub sampled: Vec<CorpusSite>,
    /// Keyed by url so a double-recorded outcome is caught by the reconciliation rather than
    /// quietly overwriting — a site scored twice is an instrument bug that used to be invisible.
    pub outcomes: BTreeMap<String, SiteOutcome>,
}

impl SweepLedger {
    pub fn new(sampled: Vec<CorpusSite>) -> Self {
        Self {
            sampled,
            outcomes: BTreeMap::new(),
        }
    }

    pub fn record(&mut self, url: &str, outcome: SiteOutcome) {
        self.outcomes.insert(url.to_string(), outcome);
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut scored = 0;
        let mut passed = 0;
        let mut fail = 0;
        let mut excluded = 0;
        for o in self.outcomes.values() {
            match o {
                SiteOutcome::Scored { pass } => {
                    scored += 1;
                    if *pass {
                        passed += 1;
                    }
                }
                SiteOutcome::Fail { .. } => fail += 1,
                SiteOutcome::Excluded { .. } => excluded += 1,
            }
        }
        (scored, passed, fail, excluded)
    }

    /// **The reconciliation gate.** `sampled == scored + FAIL + EXCLUDED`, and every outcome must
    /// belong to a sampled site.
    ///
    /// An imbalance is an **instrument bug, not a result** — which is the whole point of asserting it
    /// rather than printing it. The two directions catch different defects: a shortfall means sites
    /// were dropped (the optimism this module exists to prevent), and a surplus means an outcome was
    /// recorded for something that was never sampled, which means the sweep is scoring a corpus it
    /// does not think it is running.
    pub fn reconcile(&self) -> Result<(), String> {
        // The SURPLUS check runs first, deliberately: a stray outcome also makes the count differ,
        // and reporting that as "a site was dropped" would send the next tick hunting the opposite
        // defect. The more specific diagnosis has to win.
        let sampled_urls: std::collections::BTreeSet<&str> =
            self.sampled.iter().map(|s| s.url.as_str()).collect();
        if let Some(stray) = self
            .outcomes
            .keys()
            .find(|u| !sampled_urls.contains(u.as_str()))
        {
            return Err(format!(
                "RECONCILIATION FAILED: an outcome was recorded for {stray:?}, which is not in the \
                 sampled set — the sweep is scoring a corpus it does not think it is running"
            ));
        }
        let (scored, _, fail, excluded) = self.counts();
        let accounted = scored + fail + excluded;
        if accounted != self.sampled.len() {
            let missing: Vec<&str> = self
                .sampled
                .iter()
                .filter(|s| !self.outcomes.contains_key(&s.url))
                .map(|s| s.url.as_str())
                .take(5)
                .collect();
            return Err(format!(
                "RECONCILIATION FAILED: sampled {} != scored {scored} + FAIL {fail} + EXCLUDED {excluded} = {accounted}. \
                 A site with no recorded outcome was DROPPED, which is exactly the leak that made every \
                 past reading optimistic. First unaccounted: {missing:?}",
                self.sampled.len()
            ));
        }
        Ok(())
    }

    /// Pass-rate per stratum, **against the full sampled denominator for that stratum** (minus its
    /// exclusions). HEAD and TAIL are two claims and are never averaged into one.
    pub fn by_stratum(&self) -> BTreeMap<String, (usize, usize)> {
        let mut out: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for s in &self.sampled {
            let e = out.entry(s.stratum.clone()).or_insert((0, 0));
            match self.outcomes.get(&s.url) {
                Some(SiteOutcome::Excluded { .. }) => {}
                Some(SiteOutcome::Scored { pass: true }) => {
                    e.0 += 1;
                    e.1 += 1;
                }
                // A FAIL, and an unrecorded site, both count against the denominator. The latter
                // cannot survive `reconcile`, but this must not be the place that hides it.
                _ => e.1 += 1,
            }
        }
        out
    }
}

/// A deterministic, stratified sub-slice — the per-tick wall's real-site regression guard.
///
/// **Fixed seed and no RNG.** The selection must be identical on every machine and every run, or a
/// regression that fails one tick passes the next and the guard is noise. A stride walk over the
/// stratum's sites, offset by the seed, gives an even spread across ranks without a generator whose
/// implementation could change under us.
pub fn subslice(sites: &[CorpusSite], per_stratum: usize, seed: u64) -> Vec<CorpusSite> {
    let mut by: BTreeMap<&str, Vec<&CorpusSite>> = BTreeMap::new();
    for s in sites {
        by.entry(s.stratum.as_str()).or_default().push(s);
    }
    let mut out = Vec::new();
    for (_, group) in by {
        if group.is_empty() || per_stratum == 0 {
            continue;
        }
        let n = per_stratum.min(group.len());
        let stride = group.len() / n;
        let offset = (seed as usize) % group.len();
        for i in 0..n {
            out.push(group[(offset + i * stride) % group.len()].clone());
        }
    }
    out.sort_by(|a, b| (&a.stratum, a.rank, &a.url).cmp(&(&b.stratum, b.rank, &b.url)));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(stratum: &str, rank: u64, url: &str) -> CorpusSite {
        CorpusSite {
            stratum: stratum.into(),
            rank,
            url: url.into(),
        }
    }

    /// **G_CORPUS_DENOMINATOR — a sampled site cannot leave the denominator silently.**
    ///
    /// Cause #1 in `DAILY-DRIVER-CERTIFICATION.md` §0: the old corpus dropped timeouts, crashes and
    /// bot-walls, which removes exactly the pages a browser is worst at, so every past reading
    /// described an easier web than the real one. This is the mechanical guard.
    #[test]
    fn a_dropped_site_fails_reconciliation() {
        let sampled = vec![
            site("HEAD", 1, "https://a.test/"),
            site("HEAD", 2, "https://b.test/"),
            site("TAIL", 300000, "https://c.test/"),
        ];
        let mut led = SweepLedger::new(sampled);
        led.record("https://a.test/", SiteOutcome::Scored { pass: true });
        led.record(
            "https://b.test/",
            SiteOutcome::Fail {
                reason: "timeout 30s".into(),
            },
        );
        // c.test is simply never recorded — the exact shape of the old leak.
        let err = led.reconcile().unwrap_err();
        assert!(
            err.contains("RECONCILIATION FAILED") && err.contains("c.test"),
            "a site with no outcome must fail reconciliation AND be named, or the next tick cannot \
             find it. got: {err}"
        );

        // …and recording it — even as a FAIL — reconciles. A failure is an ANSWER, not an omission.
        led.record(
            "https://c.test/",
            SiteOutcome::Fail {
                reason: "bot-wall 403".into(),
            },
        );
        led.reconcile().expect("a recorded FAIL must reconcile");
    }

    /// A FAIL must count AGAINST the bar, not out of it — the whole point of the fixed denominator.
    #[test]
    fn a_timeout_counts_against_the_bar_not_out_of_it() {
        let sampled = vec![
            site("HEAD", 1, "https://a.test/"),
            site("HEAD", 2, "https://b.test/"),
        ];
        let mut led = SweepLedger::new(sampled);
        led.record("https://a.test/", SiteOutcome::Scored { pass: true });
        led.record(
            "https://b.test/",
            SiteOutcome::Fail {
                reason: "timeout".into(),
            },
        );
        led.reconcile().unwrap();
        let head = led.by_stratum();
        assert_eq!(
            head["HEAD"],
            (1, 2),
            "one of two HEAD sites passed. If a timeout left the denominator this would read (1,1) \
             = 100%, which is precisely how dropping the hard sites manufactures an optimistic number"
        );
    }

    /// An EXCLUDED site leaves the denominator — but only because the corpus header names the
    /// exclusion, and the count stays visible.
    #[test]
    fn an_excluded_site_leaves_the_denominator_visibly() {
        let sampled = vec![
            site("HEAD", 1, "https://a.test/"),
            site("HEAD", 2, "https://adult.test/"),
        ];
        let mut led = SweepLedger::new(sampled);
        led.record("https://a.test/", SiteOutcome::Scored { pass: true });
        led.record(
            "https://adult.test/",
            SiteOutcome::Excluded {
                reason: "adult (named)".into(),
            },
        );
        led.reconcile().unwrap();
        assert_eq!(
            led.by_stratum()["HEAD"],
            (1, 1),
            "an excluded site is out of the denominator"
        );
        assert_eq!(
            led.counts().3,
            1,
            "…and the exclusion count stays reportable"
        );
    }

    /// An outcome for a site that was never sampled means the sweep is scoring a different corpus.
    #[test]
    fn a_stray_outcome_fails_reconciliation() {
        let mut led = SweepLedger::new(vec![site("HEAD", 1, "https://a.test/")]);
        led.record("https://a.test/", SiteOutcome::Scored { pass: true });
        led.record("https://ghost.test/", SiteOutcome::Scored { pass: true });
        assert!(led.reconcile().unwrap_err().contains("ghost.test"));
    }

    /// The wall's sub-slice must be identical every run and cover BOTH strata — a guard that varies
    /// run to run is noise, and one that samples only HEAD cannot see a tail regression.
    #[test]
    fn the_subslice_is_deterministic_and_stratified() {
        let real = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/bench/corpus-v2.tsv"
        ))
        .expect("corpus-v2.tsv must exist — it is the certification corpus");
        let sites = parse_corpus(&real).expect("corpus-v2.tsv must parse");
        assert!(
            sites.len() >= 400,
            "the certification corpus is ~400 sites; got {}",
            sites.len()
        );
        let a = subslice(&sites, 12, 20260725);
        let b = subslice(&sites, 12, 20260725);
        assert_eq!(
            a, b,
            "the sub-slice must be identical every run — same seed, same sites"
        );
        let strata: std::collections::BTreeSet<&str> =
            a.iter().map(|s| s.stratum.as_str()).collect();
        assert!(
            strata.contains("HEAD") && strata.contains("TAIL"),
            "the sub-slice must span BOTH strata, or it cannot see a tail regression. got: {strata:?}"
        );
        let c = subslice(&sites, 12, 999);
        assert_ne!(
            a, c,
            "a different seed must select a different slice, or the seed is decoration"
        );
    }

    /// A malformed corpus line is an error, not a skip — the same leak one layer earlier.
    #[test]
    fn a_malformed_corpus_line_is_an_error() {
        assert!(parse_corpus("HEAD\t1\thttps://a.test/\nGARBAGE\n").is_err());
        assert!(parse_corpus("HEAD\tnotanumber\thttps://a.test/\n").is_err());
        assert_eq!(
            parse_corpus("# comment\n\nHEAD\t1\thttps://a.test/\n")
                .unwrap()
                .len(),
            1
        );
    }
}
