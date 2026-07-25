//! **test262 — the ECMAScript conformance suite, run against the JS engine we actually ship.**
//!
//! ## Why this exists, and why it is not "just run the tests"
//!
//! `docs/loop/CONSTELLATION.tsv` has carried a `?` on test262 since surface audit t83: *"Ladybird
//! tracks 97.8% of 53,207 subtests. We have NEVER RUN IT."* That `?` is the more expensive kind of
//! unknown, because it sits under a **headline claim** — we embed SpiderMonkey, so the intuition is
//! that JS conformance is somebody else's solved problem. **An absent measurement is not a positive
//! measurement**, and this project has been wrong in that exact direction four times.
//!
//! It is not free, either, which is the other half of the honesty. test262 is not a directory of
//! scripts you `eval`. Every case carries YAML frontmatter that changes what running it even *means*:
//! which harness files must be prepended, whether it runs sloppy or strict or **both** (so one file
//! is *two* subtests), whether it is expected to **throw** and with which error type, and whether it
//! needs host facilities (`$262.agent`, a second realm) that an embedder must supply.
//!
//! ## The probity rules this runner is built to satisfy
//!
//! They are the same ones `scripts/fidelity-sweep.sh` learned the hard way, restated for a
//! conformance suite:
//!
//! 1. **A test we cannot run is a SKIP with a NAMED reason, never a silent drop and never a pass.**
//!    A runner that quietly omits what it cannot handle reports a pass rate for a suite it did not
//!    run — and the tests an embedder finds hard are exactly the ones an engine finds hard.
//! 2. **Two numbers, always.** The flattering one (`passed / executed`) and the honest one
//!    (`passed / every subtest the suite defines`, skips counted as *not passed*). If they diverge,
//!    the gap IS the finding.
//! 3. **The runner's own limits are printed with the number**, not filed in a doc nobody reads. A
//!    conformance number contaminated by its own harness is worse than no number, because it is
//!    believed.
//!
//! ## Stated limits of THIS runner (print them; do not let them become folklore)
//!
//! * **Module-goal tests are SKIPPED.** `flags: [module]` needs the module loader wired to a file
//!   resolver; our eval seam is script-goal. Counted and named, never dropped.
//! * **`async` tests are SKIPPED.** They complete through `$DONE`/`doneprintHandle.js`, which needs
//!   a `print` host function and a drain of the microtask queue after evaluation.
//! * **Host-API tests are SKIPPED** — `$262.createRealm`, `$262.agent`, `$262.detachArrayBuffer`.
//!   Failing them would score the *embedder's* missing host object as an *engine* defect, which is
//!   the harness-contamination failure mode rule 3 exists to prevent.
//! * **`negative` is scored on the error TYPE, not the PHASE.** A `phase: parse` case that throws
//!   the right error type at *runtime* is recorded as a pass here. Real, and stated: closing it
//!   needs a compile-without-execute seam, which is its own brick.
//!
//! Everything else — `built-ins`, `language`, `annexB`, `intl402`, both sloppy and strict — runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A `negative:` block — the test is expected to throw, and *which* error is the whole verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Negative {
    pub phase: String,
    pub ty: String,
}

/// The YAML frontmatter of one test262 case, reduced to the fields that change how it is RUN.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frontmatter {
    pub negative: Option<Negative>,
    pub includes: Vec<String>,
    pub flags: Vec<String>,
    pub features: Vec<String>,
}

impl Frontmatter {
    pub fn has_flag(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
    pub fn has_feature(&self, f: &str) -> bool {
        self.features.iter().any(|x| x == f)
    }
}

/// Parse the `/*--- … ---*/` frontmatter.
///
/// A hand-rolled reader for the subset test262 actually uses, NOT a YAML engine — and the reason is
/// worth stating, because "just add a YAML crate" is the obvious move. The `info:` field is a free-form
/// block scalar containing spec prose, which regularly includes lines that are not valid YAML in
/// isolation; a strict parser has to be fed the whole document correctly or it errors out, and an
/// erroring parser here silently mis-classifies a test as ordinary. We need exactly four keys, all at
/// indent 0, in a fixed vocabulary. So: read the top-level keys, ignore every nested block except
/// `negative`'s two fields, and treat an unparseable frontmatter as an empty one — which runs the test
/// in its most demanding form (both modes, no expected throw) rather than skipping it.
pub fn parse_frontmatter(src: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    let Some(start) = src.find("/*---") else {
        return fm;
    };
    let rest = &src[start + 5..];
    let Some(end) = rest.find("---*/") else {
        return fm;
    };
    let block = &rest[..end];

    let mut in_negative = false;
    let mut list_key: Option<&'static str> = None;
    for raw in block.lines() {
        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // A `- item` line continues whichever list key opened above it.
        if indented && line.starts_with("- ") {
            if let Some(k) = list_key {
                let v = line[2..]
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                if !v.is_empty() {
                    match k {
                        "includes" => fm.includes.push(v),
                        "flags" => fm.flags.push(v),
                        "features" => fm.features.push(v),
                        _ => {}
                    }
                }
            }
            continue;
        }
        if indented {
            if in_negative {
                if let Some(v) = line.strip_prefix("phase:") {
                    fm.negative
                        .get_or_insert(Negative {
                            phase: String::new(),
                            ty: String::new(),
                        })
                        .phase = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("type:") {
                    fm.negative
                        .get_or_insert(Negative {
                            phase: String::new(),
                            ty: String::new(),
                        })
                        .ty = v.trim().to_string();
                }
            }
            continue;
        }
        // A new top-level key closes any open list/block.
        in_negative = false;
        list_key = None;
        if line.starts_with("negative:") {
            in_negative = true;
            fm.negative.get_or_insert(Negative {
                phase: String::new(),
                ty: String::new(),
            });
            continue;
        }
        for key in ["includes", "flags", "features"] {
            if let Some(v) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) {
                let v = v.trim();
                if let Some(inner) = v.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                    for item in inner.split(',') {
                        let item = item.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !item.is_empty() {
                            match key {
                                "includes" => fm.includes.push(item.to_string()),
                                "flags" => fm.flags.push(item.to_string()),
                                _ => fm.features.push(item.to_string()),
                            }
                        }
                    }
                } else {
                    // Block form: the items are on the following `- x` lines.
                    list_key = Some(match key {
                        "includes" => "includes",
                        "flags" => "flags",
                        _ => "features",
                    });
                }
                break;
            }
        }
    }
    fm
}

/// How one file is executed. A single case is `Sloppy` **and** `Strict` unless it says otherwise —
/// which is why the subtest count is roughly twice the file count and why quoting the file count as
/// the suite size understates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `flags: [raw]` — no harness, no strict wrapper, source exactly as written.
    Raw,
    Sloppy,
    Strict,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Raw => "raw",
            Mode::Sloppy => "sloppy",
            Mode::Strict => "strict",
        }
    }
}

/// The modes one file runs in.
pub fn modes(fm: &Frontmatter) -> Vec<Mode> {
    if fm.has_flag("raw") {
        return vec![Mode::Raw];
    }
    if fm.has_flag("onlyStrict") {
        return vec![Mode::Strict];
    }
    if fm.has_flag("noStrict") {
        return vec![Mode::Sloppy];
    }
    vec![Mode::Sloppy, Mode::Strict]
}

/// Why a case is not executed — a NAMED reason, because an unnamed skip is a silent drop.
pub const SKIP_MODULE: &str = "module-goal (no module loader on the eval seam)";
pub const SKIP_ASYNC: &str = "async ($DONE protocol not wired)";
pub const SKIP_HOST: &str = "host API ($262.agent / createRealm / detachArrayBuffer)";
pub const SKIP_SLOW: &str =
    "exceeds the per-case budget (see SLOW_CASES — a HANG, recorded as one)";

/// Cases measured to run past **240 seconds** and never observed to finish.
///
/// This is a skip list, which is the shape of thing that quietly becomes a way to make a number look
/// better, so it is fenced by three rules: every entry is here because it was **measured**, every
/// entry is counted in the honest denominator as **not passed**, and the list is **short and named**
/// — a growing one is the signal that it is being abused rather than that the suite is hard.
///
/// **What it actually records is a Bar-0 finding, not a test we dislike.**
/// `RGI_Emoji.js` runs `/^\p{RGI_Emoji}+$/v` across the whole Unicode space via
/// `regExpUtils.js`, which is expensive but *finite* — and it did not finish in four minutes, at
/// 100% CPU, with RSS climbing into the gigabytes. We cannot yet say whether that is slow or
/// non-terminating, **and the reason we cannot is itself the finding**: there is no
/// `JS_AddInterruptCallback` on this engine, so a synchronous script cannot be interrupted, timed
/// out, or asked how far it got. `STATUS.md` has carried *"production interruptibility (a
/// cancellable long task) is still not built"* under Bar 0 for hundreds of ticks; this is the first
/// instrument that walked into it and could not walk back out. With an interrupt callback this list
/// becomes a per-case deadline and stops being a list at all.
pub const SLOW_CASES: &[&str] =
    &["built-ins/RegExp/property-escapes/generated/strings/RGI_Emoji.js"];

/// The skip verdict for a case, or `None` to run it.
///
/// **Host-API detection reads the SOURCE, not just the frontmatter**, because `features:` is not a
/// reliable index of it: plenty of cases reach for `$262.createRealm` without a feature naming it.
/// Skipping on the text is the conservative direction — the failure it prevents (scoring OUR missing
/// host object as SpiderMonkey's defect) is the one that would corrupt the number.
pub fn skip_reason(fm: &Frontmatter, src: &str) -> Option<&'static str> {
    if fm.has_flag("module") || fm.has_flag("dynamic-import") {
        return Some(SKIP_MODULE);
    }
    if fm.has_flag("async") || fm.has_flag("CanBlockIsFalse") {
        return Some(SKIP_ASYNC);
    }
    if fm.includes.iter().any(|i| i == "detachArrayBuffer.js" || i == "asyncHelpers.js")
        || fm.has_feature("cross-realm")
        || fm.has_feature("IsHTMLDDA")
        // ANY reach for `$262` at all, not just the three well-known members. Measured: the
        // member-name list let 33 subtests through to fail with `$262 is not defined`, which is our
        // missing host object being recorded as SpiderMonkey's defect — the exact contamination this
        // function exists to prevent. The host object is a NAMED follow-on, so the honest state is
        // "skipped, and here is why", not "failed".
        || src.contains("$262")
        || src.contains("$DETACHBUFFER")
    {
        return Some(SKIP_HOST);
    }
    None
}

/// Is this case on the measured [`SLOW_CASES`] list? Matched on the suite-relative path so the entry
/// names one file exactly — a directory-prefix rule would silently grow to cover its neighbours.
pub fn is_slow(rel: &str) -> bool {
    let rel = rel.replace('\\', "/");
    SLOW_CASES.contains(&rel.as_str())
}

/// Build the source actually handed to the engine: harness, then includes, then the test body,
/// with `"use strict"` at the very front for a strict run.
///
/// `harness` resolves a harness file name to its text; it returns `None` when the file is missing,
/// and a missing harness file **aborts the case** rather than running it without — a test run
/// without `assert.js` does not fail, it throws `assert is not defined`, which would be recorded as
/// an engine defect. That is precisely the contamination rule 3 forbids.
///
/// Order is test262's own (`INTERPRETING.md`): `assert.js`, then `sta.js`, then the case's
/// `includes`, then the body. `raw` gets the body alone.
pub fn assemble(
    fm: &Frontmatter,
    body: &str,
    mode: Mode,
    harness: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<String> {
    if mode == Mode::Raw {
        return Some(body.to_string());
    }
    let mut out = String::with_capacity(body.len() + 8192);
    if mode == Mode::Strict {
        out.push_str("\"use strict\";\n");
    }
    for name in ["assert.js", "sta.js"]
        .iter()
        .map(|s| s.to_string())
        .chain(fm.includes.clone())
    {
        out.push_str(&harness(&name)?);
        out.push('\n');
    }
    out.push_str(body);
    Some(out)
}

/// The outcome of one subtest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// The reason, already shaped for clustering: the leading token is the error TYPE where there
    /// is one, so `failures by type` groups without further parsing.
    Fail(String),
}

/// Score one execution against the case's `negative` expectation.
///
/// `outcome` is `Ok(())` when the source ran to completion and `Err(msg)` when it threw, where
/// `msg` is the exception stringified — which, for an `Error`, begins with the constructor name.
/// That is exactly the discriminator a negative test is scored on, and it is why the SpiderMonkey
/// runtime had to be taught to report the real message (tick 546): with the old
/// `"uncaught exception while evaluating <file>"` every negative test in the suite is
/// indistinguishable from every other, and ~4,000 of them would have been scored on a coin flip.
pub fn verdict(fm: &Frontmatter, outcome: Result<(), String>) -> Verdict {
    match (&fm.negative, outcome) {
        (None, Ok(())) => Verdict::Pass,
        (None, Err(m)) => Verdict::Fail(first_line(&m)),
        (Some(n), Ok(())) => Verdict::Fail(format!("{}: expected throw, none happened", n.ty)),
        (Some(n), Err(m)) => {
            if throws_type(&m, &n.ty) {
                Verdict::Pass
            } else {
                Verdict::Fail(format!(
                    "{}: expected {}, got {}",
                    n.ty,
                    n.ty,
                    first_line(&m)
                ))
            }
        }
    }
}

/// Does a stringified exception name `ty`? `"SyntaxError: unexpected token"` names `SyntaxError`;
/// `"TypeError"` (a bare `String(e)` of an error with no message) names `TypeError`. Substring
/// matching would be wrong in the one case that matters — `ReferenceError` contains no other type
/// name, but a *message* mentioning "SyntaxError" would falsely satisfy a `SyntaxError` expectation
/// — so this anchors at the start.
pub fn throws_type(msg: &str, ty: &str) -> bool {
    let m = msg.trim_start();
    m == ty || m.starts_with(&format!("{ty}:")) || m.starts_with(&format!("{ty} "))
}

fn first_line(m: &str) -> String {
    m.lines().next().unwrap_or("").chars().take(160).collect()
}

/// Running totals. Skips are keyed by their reason so the report can never print a bare "skipped: N".
#[derive(Debug, Default)]
pub struct Tally {
    pub files: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: BTreeMap<&'static str, usize>,
    /// Failure count per top-two-path-component area (`built-ins/RegExp`), the actionable grouping:
    /// 4,000 failures are a handful of area-shaped causes, never 4,000 bugs.
    pub fail_by_area: BTreeMap<String, usize>,
    /// Failure count per leading error type — the other axis of the same question.
    pub fail_by_type: BTreeMap<String, usize>,
    pub samples: Vec<String>,
}

impl Tally {
    pub fn skip(&mut self, reason: &'static str, n: usize) {
        *self.skipped.entry(reason).or_default() += n;
    }
    pub fn skipped_total(&self) -> usize {
        self.skipped.values().sum()
    }
    pub fn executed(&self) -> usize {
        self.passed + self.failed
    }
    /// The flattering number: of the subtests we actually ran, how many passed.
    pub fn pass_pct_executed(&self) -> f64 {
        if self.executed() == 0 {
            return 0.0;
        }
        self.passed as f64 * 100.0 / self.executed() as f64
    }
    /// The honest number: skips counted as NOT passed, because a suite we could not run is not a
    /// suite we passed.
    pub fn pass_pct_defined(&self) -> f64 {
        let d = self.executed() + self.skipped_total();
        if d == 0 {
            return 0.0;
        }
        self.passed as f64 * 100.0 / d as f64
    }
    pub fn record_fail(&mut self, rel: &str, reason: &str) {
        self.failed += 1;
        *self.fail_by_area.entry(area_of(rel)).or_default() += 1;
        *self.fail_by_type.entry(type_of(reason)).or_default() += 1;
        if self.samples.len() < 25 {
            self.samples.push(format!("{rel}  —  {reason}"));
        }
    }
}

/// `built-ins/RegExp/prototype/exec/x.js` → `built-ins/RegExp`. Two components is the level that
/// names a CAUSE (one is too coarse to act on, three splinters one bug across twenty rows).
pub fn area_of(rel: &str) -> String {
    let mut it = rel.split('/');
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}/{b}"),
        (Some(a), None) => a.to_string(),
        _ => "(root)".to_string(),
    }
}

/// The leading error-type token of a failure reason, or `(assertion)` when there is none.
pub fn type_of(reason: &str) -> String {
    let head = reason.split([':', ' ']).next().unwrap_or("");
    if head.ends_with("Error") {
        head.to_string()
    } else {
        "(other)".to_string()
    }
}

/// Every `.js` case under `root`, sorted, with fixtures and unratified staging excluded.
///
/// `_FIXTURE.js` files are **imports of other tests**, not tests — counting them would inflate the
/// denominator with files that have no verdict of their own. `staging/` is test262's own holding
/// pen for un-ratified material and is not part of the number anyone quotes. Both exclusions are
/// returned as counts so the report states them rather than hiding them.
pub fn discover(root: &Path) -> (Vec<PathBuf>, usize, usize) {
    let mut out = Vec::new();
    let (mut fixtures, mut staging) = (0usize, 0usize);
    let mut stack = vec![root.join("test")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "staging") {
                    staging += count_js(&p);
                    continue;
                }
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "js") {
                if p.to_string_lossy().ends_with("_FIXTURE.js") {
                    fixtures += 1;
                } else {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    (out, fixtures, staging)
}

fn count_js(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "js") {
                n += 1;
            }
        }
    }
    n
}

/// Take `limit` files spread EVENLY across the sorted list rather than the first `limit`.
///
/// The same lesson `scripts/fidelity-sweep.sh` records in its own header: the corpus is grouped, so
/// `--limit` on the head samples one group and calls it a read of the whole. `test/annexB/**` sorts
/// first here, and it is the least representative directory in the suite.
pub fn stride_sample(files: Vec<PathBuf>, limit: usize) -> Vec<PathBuf> {
    if limit == 0 || files.len() <= limit {
        return files;
    }
    let step = files.len() as f64 / limit as f64;
    (0..limit)
        .map(|i| files[((i as f64 * step) as usize).min(files.len() - 1)].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frontmatter reader against the four shapes the suite actually uses: inline lists, block
    /// lists, a nested `negative`, and a case with no frontmatter at all.
    #[test]
    fn frontmatter_reads_the_shapes_test262_uses() {
        let inline = parse_frontmatter(
            "/*---\nesid: sec-x\nincludes: [compareArray.js, propertyHelper.js]\nflags: [onlyStrict]\nfeatures: [Symbol.iterator]\n---*/\nvar x;",
        );
        assert_eq!(
            inline.includes,
            vec!["compareArray.js", "propertyHelper.js"]
        );
        assert_eq!(inline.flags, vec!["onlyStrict"]);
        assert_eq!(inline.features, vec!["Symbol.iterator"]);
        assert_eq!(inline.negative, None);

        let block = parse_frontmatter(
            "/*---\ninfo: |\n  prose that is not yaml: [unbalanced\nnegative:\n  phase: parse\n  type: SyntaxError\nflags:\n  - raw\n  - noStrict\n---*/\n",
        );
        assert_eq!(
            block.negative,
            Some(Negative { phase: "parse".into(), ty: "SyntaxError".into() }),
            "the nested negative block is the whole verdict of ~4k cases — it must survive an `info:` \
             block scalar containing text a YAML parser would reject"
        );
        assert_eq!(block.flags, vec!["raw", "noStrict"]);

        assert_eq!(parse_frontmatter("var x = 1;"), Frontmatter::default());
    }

    /// One file is one OR TWO subtests, and getting this wrong misreports the suite size before a
    /// single test has run.
    #[test]
    fn a_plain_case_is_two_subtests_and_the_flags_cut_it_to_one() {
        assert_eq!(
            modes(&Frontmatter::default()),
            vec![Mode::Sloppy, Mode::Strict]
        );
        let only = Frontmatter {
            flags: vec!["onlyStrict".into()],
            ..Default::default()
        };
        assert_eq!(modes(&only), vec![Mode::Strict]);
        let no = Frontmatter {
            flags: vec!["noStrict".into()],
            ..Default::default()
        };
        assert_eq!(modes(&no), vec![Mode::Sloppy]);
        let raw = Frontmatter {
            flags: vec!["raw".into()],
            ..Default::default()
        };
        assert_eq!(modes(&raw), vec![Mode::Raw]);
    }

    /// The verdict table, including the case the whole `pending_exception_message` change exists
    /// for: a negative test is scored on WHICH error was thrown, so a runner that cannot read the
    /// type cannot score it.
    #[test]
    fn negative_cases_are_scored_on_the_error_type() {
        let plain = Frontmatter::default();
        assert_eq!(verdict(&plain, Ok(())), Verdict::Pass);
        assert!(matches!(
            verdict(&plain, Err("Test262Error: nope".into())),
            Verdict::Fail(_)
        ));

        let neg = Frontmatter {
            negative: Some(Negative {
                phase: "parse".into(),
                ty: "SyntaxError".into(),
            }),
            ..Default::default()
        };
        assert_eq!(
            verdict(&neg, Err("SyntaxError: unexpected token".into())),
            Verdict::Pass
        );
        assert!(
            matches!(verdict(&neg, Err("TypeError: x is not a function".into())), Verdict::Fail(_)),
            "the RIGHT throw for the WRONG reason is a FAIL — an engine that throws TypeError where \
             the spec says SyntaxError is non-conformant, and a runner that only asks 'did it throw' \
             reports it green"
        );
        assert!(
            matches!(verdict(&neg, Ok(())), Verdict::Fail(_)),
            "a negative test that does not throw is the failure the case was written to catch"
        );
        // Anchored, not substring: a MESSAGE that mentions another type must not satisfy it.
        assert!(!throws_type(
            "TypeError: expected a SyntaxError here",
            "SyntaxError"
        ));
    }

    /// A missing harness file must abort the case, not run it bare. Running `assert.sameValue(...)`
    /// with no `assert.js` throws `ReferenceError` — which would be filed as an ENGINE defect, and
    /// that is the exact way a runner corrupts the number it exists to produce.
    #[test]
    fn a_missing_harness_file_aborts_the_case_instead_of_scoring_it() {
        let fm = Frontmatter {
            includes: vec!["compareArray.js".into()],
            ..Default::default()
        };
        let mut present = |n: &str| Some(format!("/*{n}*/"));
        let asm =
            assemble(&fm, "body();", Mode::Sloppy, &mut present).expect("all harness present");
        assert!(
            asm.starts_with("/*assert.js*/"),
            "assert.js precedes sta.js precedes the includes"
        );
        assert!(asm.contains("/*sta.js*/") && asm.contains("/*compareArray.js*/"));
        assert!(asm.trim_end().ends_with("body();"));
        assert!(!asm.starts_with("\"use strict\""));

        let strict = assemble(&fm, "body();", Mode::Strict, &mut present).unwrap();
        assert!(
            strict.starts_with("\"use strict\";\n"),
            "strict applies to the WHOLE source, harness included"
        );

        let mut missing = |n: &str| (n != "compareArray.js").then(|| format!("/*{n}*/"));
        assert_eq!(assemble(&fm, "body();", Mode::Sloppy, &mut missing), None);

        // `raw` takes the body alone — no harness, no strict prologue, ever.
        let raw = Frontmatter {
            flags: vec!["raw".into()],
            ..Default::default()
        };
        assert_eq!(
            assemble(&raw, "body();", Mode::Raw, &mut missing).as_deref(),
            Some("body();")
        );
    }

    /// Skips must be NAMED, and host-API cases must be caught from the SOURCE — `features:` does not
    /// reliably index them, and scoring our own missing `$262` as an engine failure is the
    /// contamination this runner is built to refuse.
    #[test]
    fn unrunnable_cases_are_named_skips_not_failures() {
        let m = Frontmatter {
            flags: vec!["module".into()],
            ..Default::default()
        };
        assert_eq!(skip_reason(&m, ""), Some(SKIP_MODULE));
        let a = Frontmatter {
            flags: vec!["async".into()],
            ..Default::default()
        };
        assert_eq!(skip_reason(&a, ""), Some(SKIP_ASYNC));
        assert_eq!(
            skip_reason(&Frontmatter::default(), "var r = $262.createRealm();"),
            Some(SKIP_HOST),
            "detected from the SOURCE — plenty of cross-realm cases carry no feature naming it"
        );
        assert_eq!(
            skip_reason(&Frontmatter::default(), "assert.sameValue(1, 1);"),
            None
        );
    }

    /// Both numbers, and the gap between them, from one tally.
    #[test]
    fn the_honest_number_counts_skips_as_not_passed() {
        let mut t = Tally::default();
        t.passed = 90;
        t.record_fail("built-ins/RegExp/prototype/exec/x.js", "TypeError: nope");
        t.skip(SKIP_MODULE, 9);
        assert_eq!(t.executed(), 91);
        assert!((t.pass_pct_executed() - 98.9).abs() < 0.1);
        assert!(
            (t.pass_pct_defined() - 90.0).abs() < 0.1,
            "the honest number must move when skips grow — a runner that skips more and reports a \
             HIGHER pass rate is the failure mode this pair exists to expose"
        );
        assert_eq!(t.fail_by_area.get("built-ins/RegExp"), Some(&1));
        assert_eq!(t.fail_by_type.get("TypeError"), Some(&1));
    }

    /// `--limit` samples ACROSS the suite. Taking the head would read `annexB` and call it test262.
    #[test]
    fn limit_samples_across_the_suite_not_off_the_front() {
        let files: Vec<PathBuf> = (0..100)
            .map(|i| PathBuf::from(format!("{i:03}.js")))
            .collect();
        let s = stride_sample(files.clone(), 4);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], PathBuf::from("000.js"));
        assert_eq!(s[3], PathBuf::from("075.js"));
        assert_eq!(
            stride_sample(files.clone(), 0).len(),
            100,
            "0 means no limit"
        );
        assert_eq!(stride_sample(files, 500).len(), 100);
    }
}
