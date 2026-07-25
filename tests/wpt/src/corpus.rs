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

// ─────────────────────────────────────────────────────────────────────────────────────────────
// LAYER C-FUNCTION (observer CO-#1 item 3; DAILY-DRIVER-CERTIFICATION.md §4)
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// How one capability behaved on one site.
///
/// **The three-way split is the whole design.** A capability the site never touches is not a claim we
/// have to make — that is what keeps the certificate finite (§5: *"turning 'MDN lists 4,000 APIs' into
/// 'these are what this site calls'"*). A capability it touches and that works is a pass. A capability
/// it touches that **throws or no-ops** fails the site, and the distinction between those two is why
/// `NoOp` is not folded into `Threw`:
///
/// - **`Threw`** is the IndexedDB-class killer: `indexedDB.open()` on an engine without it raises, the
///   exception escapes into the site's own init path, and **unrelated page scripts die with it**.
///   Firebase and Firestore do exactly this. One missing API takes down a page that had nothing else
///   wrong with it.
/// - **`NoOp`** is quieter and, for a certificate, just as disqualifying: `IntersectionObserver` that
///   never fires leaves a lazy-loaded feed permanently empty with no error anywhere. The old
///   `works`-with-no-gate rows in the capability map were all this shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapOutcome {
    /// The site never used it — no claim required, and it is not counted for or against.
    Untouched,
    /// Touched and exercised green.
    Works,
    /// Touched and raised — takes the surrounding script down with it.
    Threw,
    /// Touched and silently did nothing — no error, no effect.
    NoOp,
}

/// The FUNCTION capabilities the certificate requires, when a site touches them.
///
/// Deliberately the **throw-class killers first** (§5's "Required — FUNCTION" list, ordered by what
/// breaks a page hardest rather than by spec size). This list is expected to grow; every addition
/// must also be falsified, which `G_CERT_FALSIFIABLE`'s term-count assertion enforces.
pub const FUNCTION_CAPS: [&str; 8] = [
    "indexeddb",
    "intersection-observer",
    "resize-observer",
    "mutation-observer",
    "fetch",
    "local-storage",
    "history",
    "form-submit",
];

/// One site's FUNCTION result: what it touched, and how each behaved.
#[derive(Debug, Clone, Default)]
pub struct SiteFunction {
    pub site: String,
    /// Outcome per [`FUNCTION_CAPS`] entry, same order.
    pub caps: Vec<(String, CapOutcome)>,
}

impl SiteFunction {
    /// Does this site FUNCTION? Every capability it touches must work.
    ///
    /// **A site that touches nothing passes**, and that is correct rather than a loophole: a static
    /// document really does work without IndexedDB. The loophole would be counting an *unprobed* site
    /// as touching nothing, which is why `functions()` is only ever called on a site the probe
    /// actually ran — the reconciliation in [`SweepLedger`](crate::corpus::SweepLedger) is what
    /// guarantees that, and a site the probe could not run is a `Fail`, not a pass.
    pub fn functions(&self) -> bool {
        self.caps
            .iter()
            .all(|(_, o)| !matches!(o, CapOutcome::Threw | CapOutcome::NoOp))
    }

    /// The capabilities that failed this site, named — an unmet certificate must say which.
    pub fn failures(&self) -> Vec<String> {
        self.caps
            .iter()
            .filter(|(_, o)| matches!(o, CapOutcome::Threw | CapOutcome::NoOp))
            .map(|(c, o)| format!("{c} {o:?}"))
            .collect()
    }
}

/// **The composed per-site verdict: `daily-driver-pass(site) = renders(site) ∧ functions(site)`.**
///
/// §4's composition, as one function, so no caller can compose it differently. Both legs must be
/// present: a site with no FUNCTION probe is **not** a render-only pass — it is unproven on half the
/// certificate, and returning `renders` alone there is precisely the shape of optimism this whole
/// redesign exists to remove.
pub fn daily_driver_pass(renders: bool, function: Option<&SiteFunction>) -> bool {
    match function {
        Some(f) => renders && f.functions(),
        None => false,
    }
}

#[cfg(test)]
mod function_tests {
    use super::*;

    fn caps(pairs: &[(&str, CapOutcome)]) -> SiteFunction {
        SiteFunction {
            site: "s".into(),
            caps: pairs
                .iter()
                .map(|(c, o)| (c.to_string(), o.clone()))
                .collect(),
        }
    }

    /// **G_CERT_FUNCTION — a capability a site TOUCHES that throws or no-ops fails that site.**
    ///
    /// The FUNCTION leg of `daily-driver-pass(site) = renders(site) ∧ functions(site)`, and the reason
    /// the certificate needed a second axis at all: a page can be laid out perfectly and still be
    /// useless. `DAILY-DRIVER-CERTIFICATION.md` §4 calls this "the IndexedDB-class killer" — absence
    /// that **throws** does not degrade the feature, it kills the surrounding script, and Firebase and
    /// Firestore both open IndexedDB during init.
    #[test]
    fn a_touched_capability_that_throws_fails_the_site() {
        assert!(
            caps(&[
                ("indexeddb", CapOutcome::Works),
                ("fetch", CapOutcome::Works)
            ])
            .functions(),
            "everything the site touches works — it functions"
        );
        assert!(
            !caps(&[("indexeddb", CapOutcome::Threw)]).functions(),
            "a THROW takes the site's own init path down with it — Firebase/Firestore open IndexedDB \
             during init, so this is not a degraded feature, it is a dead page"
        );
        assert!(
            !caps(&[("intersection-observer", CapOutcome::NoOp)]).functions(),
            "a silent NO-OP is just as disqualifying and harder to notice: an IntersectionObserver \
             that never fires leaves a lazy-loaded feed permanently empty with no error anywhere"
        );
        assert!(
            caps(&[("indexeddb", CapOutcome::Untouched)]).functions(),
            "a capability the site never touches is not a claim we have to make — that is what keeps \
             the certificate finite, and a static document really does work without IndexedDB"
        );
        assert_eq!(
            caps(&[("indexeddb", CapOutcome::Threw), ("resize-observer", CapOutcome::NoOp)])
                .failures()
                .len(),
            2,
            "an unmet certificate must NAME which capabilities failed, or the next tick is guesswork"
        );
    }

    /// The composition is one function so no caller can compose it differently — and a site with no
    /// FUNCTION probe is NOT a render-only pass.
    #[test]
    fn the_composition_requires_both_legs() {
        let ok = caps(&[("fetch", CapOutcome::Works)]);
        let bad = caps(&[("indexeddb", CapOutcome::Threw)]);
        assert!(daily_driver_pass(true, Some(&ok)), "renders AND functions");
        assert!(
            !daily_driver_pass(false, Some(&ok)),
            "functions but does not render"
        );
        assert!(
            !daily_driver_pass(true, Some(&bad)),
            "renders but does not function"
        );
        assert!(
            !daily_driver_pass(true, None),
            "a site with NO function probe must not pass on the render leg alone — it is unproven on \
             half the certificate, and counting it as a pass is exactly the optimism this redesign \
             exists to remove"
        );
    }
}

/// **The touch-recording probe — the producer that fills [`SiteFunction`] from a real page.**
///
/// §4: *"the on-page capability probe records which capabilities **this site actually touches** —
/// every global read, every method call."* Until this existed, `CapOutcome` was decided by nothing:
/// the FUNCTION leg was a shape a caller could fill with whatever it liked, which is precisely the
/// defect the certification redesign exists to remove.
///
/// **Three properties, and each one is load-bearing.**
///
/// 1. **It observes without altering.** Every wrapper re-throws after recording, so a page that
///    crashes on `indexedDB.open()` still crashes exactly where it did — the instrument must not
///    change the thing it measures. Swallowing the throw would turn a `Threw` site into a passing
///    one *and* make the page work better under measurement than in a browser.
/// 2. **`Untouched` is the default and is never upgraded.** A capability is recorded only when the
///    page reaches for it. That is what keeps the certificate finite and what makes a static
///    document legitimately pass.
/// 3. **A `NoOp` is detected by EFFECT, not by presence.** An observer that never fires and an
///    observer that does not exist are the same thing to a user; `typeof X === 'function'` cannot
///    tell them apart, and this repo ships inert stubs that would pass such a check. So the
///    observers are recorded `Works` only once a callback has actually run.
///
/// Injected as a page script ahead of the document, the same shape `chrome.rs` already uses for
/// instrumented copies. It writes its record into `#__manuk_caps` for the Rust side to read.
pub const TOUCH_PROBE_JS: &str = r#"
(function () {
  var REC = {};                                  // cap -> "works" | "threw" | "noop"
  var g = globalThis;
  function mark(c, v) {
    // A failure never downgrades to works: once a capability has thrown on a page, that page is
    // failed by it, even if a later call happens to succeed.
    if (REC[c] === 'threw') { return; }
    if (REC[c] === 'noop' && v === 'noop') { return; }
    REC[c] = v;
  }
  // `touch` records the reach-for, runs the real thing, and RE-THROWS — see property 1.
  function touch(c, fn) {
    try { var r = fn(); mark(c, 'works'); return r; }
    catch (e) { mark(c, 'threw'); throw e; }
  }
  function wrapMethod(owner, name, cap) {
    if (!owner) { return; }
    var orig = owner[name];
    if (typeof orig !== 'function') { return; }
    owner[name] = function () {
      var self = this, args = arguments;
      return touch(cap, function () { return orig.apply(self, args); });
    };
  }
  // ── The observer trio. Constructing one is a TOUCH; it only counts as WORKS once its callback
  //    has actually fired, because an observer that never fires is indistinguishable from a missing
  //    one to the user (property 3).
  ['IntersectionObserver', 'ResizeObserver', 'MutationObserver'].forEach(function (n) {
    var cap = n.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
    var Orig = g[n];
    if (typeof Orig !== 'function') { return; }
    g[n] = function (cb) {
      mark(cap, 'noop');                         // touched; not yet proven to fire
      var wrapped = function () { mark(cap, 'works'); return cb.apply(this, arguments); };
      try { return new Orig(wrapped); } catch (e) { mark(cap, 'threw'); throw e; }
    };
  });
  // ── IndexedDB: the throw-class killer. Reaching for the global at all is the touch.
  if (g.indexedDB) { wrapMethod(g.indexedDB, 'open', 'indexeddb'); }
  else {
    Object.defineProperty(g, 'indexedDB', {
      configurable: true,
      get: function () { mark('indexeddb', 'threw'); throw new Error('indexedDB is not available'); }
    });
  }
  wrapMethod(g, 'fetch', 'fetch');
  // ── localStorage needs a DIFFERENT wrap point, and finding out why was worth the detour.
  //    MEASURED (t586): `localStorage.setItem = fn` **silently does nothing** in this engine — the
  //    assignment is accepted and the original stays in place (`own=false`, `protoHas=undefined`,
  //    `wrapStuck=false`), while `indexedDB.open = fn` wraps fine. So the storage object cannot be
  //    patched through its own properties, and the probe must go one level up: the GLOBAL binding is
  //    a plain configurable value, so it is redefined to a delegating façade.
  //    That divergence is a real engine finding in its own right, not just a probe constraint —
  //    patching storage is what every quota-shim, SSR guard and analytics wrapper on the web does.
  if (g.localStorage) {
    var realLS = g.localStorage;
    try {
      Object.defineProperty(g, 'localStorage', {
        configurable: true,
        get: function () {
          return {
            setItem: function (k, v) { return touch('local-storage', function () { return realLS.setItem(k, v); }); },
            getItem: function (k) { return touch('local-storage', function () { return realLS.getItem(k); }); },
            removeItem: function (k) { return touch('local-storage', function () { return realLS.removeItem(k); }); },
            clear: function () { return touch('local-storage', function () { return realLS.clear(); }); },
            get length() { return realLS.length; },
            key: function (i) { return realLS.key(i); }
          };
        }
      });
    } catch (e) {}
  }
  if (g.history) {
    wrapMethod(g.history, 'pushState', 'history');
    wrapMethod(g.history, 'replaceState', 'history');
  }
  wrapMethod(g.HTMLFormElement && g.HTMLFormElement.prototype, 'submit', 'form-submit');
  g.__manukCapsFlush = function () {
    var out = [];
    for (var k in REC) { if (REC.hasOwnProperty(k)) { out.push(k + '=' + REC[k]); } }
    out.sort();
    var n = document.getElementById('__manuk_caps');
    if (!n) { n = document.createElement('div'); n.id = '__manuk_caps'; document.body.appendChild(n); }
    n.textContent = out.join(' ');
    return n.textContent;
  };
})();
"#;

/// Parse the probe's record (`cap=state cap=state …`) into a [`SiteFunction`].
///
/// Capabilities the record does not mention are [`CapOutcome::Untouched`] — the default that keeps
/// the certificate finite. An unrecognised state is an **error**, not a silent `Untouched`: that
/// would turn an instrument bug into a passing site, which is the leak `SweepLedger` closes one
/// level up.
pub fn parse_touch_record(site: &str, record: &str) -> Result<SiteFunction, String> {
    let mut caps: Vec<(String, CapOutcome)> = FUNCTION_CAPS
        .iter()
        .map(|c| (c.to_string(), CapOutcome::Untouched))
        .collect();
    for tok in record.split_ascii_whitespace() {
        let (cap, state) = tok
            .split_once('=')
            .ok_or_else(|| format!("touch record: expected `cap=state`, got {tok:?}"))?;
        let outcome = match state {
            "works" => CapOutcome::Works,
            "threw" => CapOutcome::Threw,
            "noop" => CapOutcome::NoOp,
            other => return Err(format!("touch record: unknown state {other:?} for {cap:?}")),
        };
        match caps.iter_mut().find(|(c, _)| c == cap) {
            Some(slot) => slot.1 = outcome,
            // A capability the probe reports but the certificate does not list is an instrument
            // drift, not a result — the two lists must be kept in step.
            None => return Err(format!("touch record: {cap:?} is not in FUNCTION_CAPS")),
        }
    }
    Ok(SiteFunction {
        site: site.to_string(),
        caps,
    })
}
