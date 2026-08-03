//! G1 — **real-site visual fidelity vs Chromium** (ADR-010, amended).
//!
//! The box-probe parity gate compares `getBoundingClientRect` on 30 *synthetic* pages. That is a
//! rigorous signal but it is **not the user's experience**: a page can pass box tolerance and still
//! look wrong — missing backgrounds, dropped shadows, wrong fonts, an unpainted element. And real
//! modern sites aren't in that corpus at all.
//!
//! So this gate does what a person would do: **render the real page, screenshot Chromium rendering
//! the same page, and compare the pixels.** Both are full renders through the real pipeline
//! (external CSS + images + JS), not a side channel.
//!
//! **Comparison method.** A raw pixel diff is useless here — font hinting and antialiasing differ
//! between any two engines and would swamp the signal. Instead both images are reduced to a coarse
//! **block grid** (mean RGB per cell). That is deliberately blind to glyph-level AA but *very*
//! sensitive to what actually matters: layout displacement, a missing background, an unpainted box,
//! a wrong colour. The score is the fraction of blocks whose mean colour agrees within tolerance.

use std::path::Path;

use anyhow::{Context, Result};

/// **Why a site could not be measured — the REASON the fixed-denominator rule always required and
/// never had.**
///
/// `DAILY-DRIVER-CERTIFICATION.md` §0 states the rule the whole redesign rests on: *"a
/// timeout/crash/bot-wall is a COUNTED outcome (FAIL/EXCLUDED **with reason**), never a silent
/// drop."* The counting half was built at t583. The **reason** half was not, and could not be: the
/// information was discarded one layer below, in the probe's own fetch, which read `curl`'s process
/// exit code and never its HTTP status. `curl -sL` exits 0 on a 403, so a Cloudflare interstitial
/// came back indistinguishable from the site, and every distinct way of failing to reach a page
/// arrived at the report as the same bare `—`.
///
/// The pilot's headline — *"9 of 14 could not be scored"* — was therefore a number with no
/// decomposition, and "find out why" was not answerable from the instrument's output at all. These
/// variants are that decomposition, and each one implies **a different remedy**, which is the point
/// of naming them apart rather than counting them together:
///
/// * [`Unreachable`](Self::Unreachable) — a corpus/network problem: DNS, TLS, connect, timeout.
/// * [`BotWall`](Self::BotWall) — we are being refused *as a client*. No amount of rendering work
///   fixes it; it is the fingerprint/identity axis (`PLATFORM MAP` item 3).
/// * [`HttpStatus`](Self::HttpStatus) — the origin answered something else non-2xx. The URL is
///   likely stale and belongs back in corpus construction.
/// * [`EmptyBody`](Self::EmptyBody) — a 2xx with nothing in it. `imdb.com` answers **202 with zero
///   bytes** to this client, which is what produced the "Chrome rendered NO [id] elements" line that
///   blamed the corpus for the network's answer.
/// * [`ProbeBlocked`](Self::ProbeBlocked) — we *did* get a document, and the document's own CSP
///   stopped the measurement. This one is invisible to any status check and is why the status check
///   alone is not sufficient.
///
/// ⚠ **A refusal is not a rendering result, and must never be scored as one.** This matters more
/// than the missing label: for a 403 the challenge page is a real document that BOTH engines render,
/// and rendering it identically would score as *high fidelity on a site we never reached*. t607
/// established the complementary truth for the ENGINE — an HTTP error status **is** a document and
/// must render, because the user has to see it. Both are correct at once: the browser renders the
/// 403, and the certificate refuses to count it as evidence about the site behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmeasurable {
    /// The request never completed — DNS, TLS, connect, or timeout. No status exists.
    Unreachable,
    /// The origin refused this client (401/403/429, or a 5xx carrying a challenge marker).
    BotWall(u32),
    /// Any other non-2xx answer.
    HttpStatus(u32),
    /// A 2xx with a zero-length body.
    EmptyBody(u32),
    /// The document arrived, but the injected probe never executed — a page-supplied CSP, in every
    /// case observed so far. The instrument's own error text already asked the right question
    /// (*"did Chrome run the script?"*) and the caller discarded it.
    ProbeBlocked,
    /// **We fetched the page and could not paint it.** The only variant here that is OUR bug rather
    /// than a property of the origin, and it was the quietest of the four drops: a render failure
    /// removed the site from the corpus instead of counting against it, so the one outcome that
    /// most deserves to lower the score was the one that could not.
    RenderFailed,
    /// **A subprocess we invoked did not come back.** Carries the deadline in seconds.
    ///
    /// Every step that shells out to Chrome used `Command::output()`, which has **no timeout of any
    /// kind**. The sweep runs sites in ONE process, one after another, so a child that never returns
    /// stalls the whole corpus — and worse, the run then produces no certificate at all, so **the
    /// sites that already completed are lost with it.** A 20-site sweep was killed by its outer
    /// `timeout` after ~45 minutes on the ninth site and its nine finished rows went with it.
    ///
    /// That is the fixed-denominator rule's blind spot. §0 makes a timeout a COUNTED outcome — but
    /// only once something *notices* it. An unbounded child is not counted, not excluded and not
    /// reported: it is total loss of the measurement, strictly worse than the flattering drop the rule
    /// was written to prevent. It is also Lesson 4 in a new place (*"an oracle must never be able to
    /// charge its own slowness to your account"*): the differential crawl solved this with a per-site
    /// process and a watchdog, and the certification sweep never inherited either.
    Timeout(u64),
    /// **The ORACLE rendered a shell, so there is nothing to compare against.** Carries the number of
    /// elements Chrome's probe actually produced.
    ///
    /// The instrument feeds both engines ONE fetched document from a `file://` temp copy — deliberately,
    /// so the two Chrome probes cannot render different pages (Wikipedia's origin injects a banner a
    /// local copy never sees). `comix.to` came back as **28 elements · 4 with a box** against ~2643
    /// tags live: a 94× gap, and the certificate was scoring the small side of it at **coverage
    /// 66.7% over three elements** — a measurement of comix.to's pre-hydration shell printed in the
    /// same column, in the same units, as `bbs.ruliweb.com`'s 4,122-path score.
    ///
    /// **⚠ THE CAUSE THIS COMMENT ASSERTED WAS WRONG, AND IT WAS NEVER MEASURED (corrected t674).**
    /// It read: *"from `file://` the page's own origin is `null`, so a JS-rendered site's fetches and
    /// module loads are cross-origin and blocked, and Chrome builds almost nothing."* Plausible,
    /// stated as fact, and load-bearing — it would have bought a loopback HTTP server. One probe
    /// killed it: the **same document served over `http://127.0.0.1` gives a byte-identical dump**
    /// (comix.to 3 elements either way; naukri.com 4 either way).
    ///
    /// The real cause was in our own probe: it ran **synchronously at end-of-parse**, so it reported
    /// the DOM before DOMContentLoaded — before any deferred script, module, or hydration.
    /// `chrome::probe_defer_tail` fixes it, and the correction converted `www.naukri.com`
    /// **`shell-only-1` → `thin-overlap-2`**: the oracle now builds the page (57 elements) and the
    /// remaining gap is *ours*, which is an actionable coverage bug rather than an instrument limit.
    ///
    /// A site that still lands here after t674 is one whose scripts do not run for the ORACLE at all.
    /// Check whether the snapshot fetch was bot-walled before treating the row as evidence about the
    /// site.
    ///
    /// Naming the condition does NOT fix the oracle; it stops the oracle LYING. It converted the last
    /// of t611's *"unscored with NO recorded reason"* rows — the residue that tick could not explain —
    /// into a stated one, and removed a false number from the certificate.
    ShellOnly(usize),
    /// **Both engines rendered, and they have too few elements IN COMMON to compare.** Carries the
    /// size of the comparable set.
    ///
    /// Distinct from [`Self::ShellOnly`], which is about the ORACLE's count, and that distinction is
    /// the whole reason this exists: `www.ebay.com` at t653 had the oracle produce **25** paths — no
    /// shell — while only **4** were comparable, because *we* rendered 16% of the page. `ShellOnly`
    /// could not fire, the sample floor refused to score it, and the row went out **unscored with no
    /// reason at all** — the certificate's own *"the instrument could not say why"* shortfall, which
    /// t614 named and t626 carried forward as its one remaining unexplained row.
    ///
    /// The reason points at US, not the corpus: the oracle rendered the page and we did not.
    ///
    /// ⚠ **THAT CLAIM IS ONLY SOUND WHEN WE ACTUALLY RENDERED LESS — see [`Self::TreeDivergence`],
    /// which t782 split out of this variant** after measuring the one thing this reason never
    /// looked at: our own element count.
    ThinOverlap(usize),
    /// **Both engines built a page of comparable size and they still barely OVERLAP.** Carries how
    /// many box-bearing elements *we* produced.
    ///
    /// ⚠⚠⚠ **THE VARIANT ABOVE ASSERTED BLAME FROM TWO NUMBERS AND THE DECIDING THIRD WAS NEVER
    /// READ.** `unscoreable_reason` took `probed` (the oracle's count) and `common` (the
    /// intersection) and, whenever the intersection was thin, printed *"the oracle built the page
    /// and we did not"*. It never took OUR count — which is sitting in `mseen` at the call site —
    /// so the sentence was structurally incapable of noticing the case where we built **more**.
    ///
    /// Measured on `www.naukri.com` (t782): the oracle's copy carries **57** box-bearing elements,
    /// ours carries **far more**, and only **9** paths are shared. Two engines that each build a
    /// page and agree on nine elements are not one engine failing to render — they are **two
    /// different documents, or the same document caught in two different states**. The oracle is fed
    /// a `curl` snapshot from `file://` while we render the LIVE url over our own net stack, and on
    /// a JS app those two runs settle differently: Chrome's copy sat on skeleton placeholders while
    /// ours had replaced them.
    ///
    /// So this is **NOT** an exoneration and **NOT** a pass — it is unscored and counts against the
    /// bar exactly as `ThinOverlap` did, and the arithmetic of the certificate is unchanged. What
    /// changes is that the loop stops being told a coverage bug is waiting where the evidence does
    /// not support one. `thin-overlap` was **25 of the 129 in-scope rows** at t777 and the board
    /// ranks work off that cohort.
    TreeDivergence(usize),
    /// **The sweep process itself died while rendering this site** — SIGSEGV, OOM-kill, or an
    /// operator's `kill`. Recovered on the NEXT run from the in-flight marker, never by the run that
    /// died, which by definition writes nothing.
    ///
    /// **The author's render-blocking stylesheets never arrived, so the page we measured is the UA
    /// stylesheet's idea of the document.** Carries how many sheets were cut.
    ///
    /// ⚠⚠⚠ **THE ORACLE ALREADY REFUSED THESE RUNS AND THIS INSTRUMENT SCORED THEM.** `Page::
    /// failed_stylesheet_fetches` exists precisely so a measurement can decline to score such a page —
    /// its own doc comment says *"a measurement that diffs it against a fully-styled reference is
    /// charging network weather to the engine's account"* — and `oracle` calls it and prints
    /// `DISCARDED`. The **fidelity** path, which produces the Phase-0 headline, never asked. So one
    /// question had two answers and the permissive one was the one that got published.
    ///
    /// The signature is unmistakable once named: coverage ≈ **1.000** with shape ≈ **0**. Every element
    /// exists (nothing was dropped — there was no CSS to drop it) and almost nothing is where Chrome
    /// puts it (there was no CSS to place it). `postgresql.org` scored `cov 1.000 / shape 0.018` over
    /// 337 nodes in the t745 sweep while the oracle discarded the same site for *3 starved sheets*.
    ///
    /// ⚠ It is **INTERMITTENT**, which is what makes it dangerous rather than merely wrong: the same
    /// URL discards on one run and diffs cleanly on the next, so a site's shape silently alternates
    /// between its real value and ~0 — a per-run variance source in the headline that looks exactly
    /// like a layout regression. A control found 0 of 24 worst-shape sites starved on a second run
    /// (t749), which is how the *systematic* version of this hypothesis was refuted.
    ///
    /// **This is OUR bug, and it stays IN-SCOPE.** The sheets were cut by our own `load_deadline`, not
    /// refused by the origin, so it must not join the EXCLUDED tier — a daily driver may not render a
    /// reachable site in UA fallback. `fidelity-progress.sh` partitions on the reason string and lands
    /// any unrecognised reason in-scope, which is the correct side for this one.
    CssStarved(usize),
    /// [`Self::Timeout`] is the same hazard one level out, and its doc comment states the argument:
    /// *"the sweep runs sites in ONE process … the sites that already finished are lost with it."*
    /// t625 closed that for a **child** we invoke, by bounding it. It stayed open for the case where
    /// **we** are the process that dies, and that case is not hypothetical: of three HEAD-20 runs
    /// this session, **two** were killed by an engine SIGSEGV mid-corpus — one at site 5, one at site
    /// 11 — and both discarded every completed row, so the certificate could not be measured at all.
    ///
    /// A crash is therefore a COUNTED outcome like every other, and for the same reason: the site
    /// that kills the sweep is the hardest site in the corpus, and an instrument that drops it is
    /// flattering itself in precisely the direction §0 names as cause #1.
    Crashed,
    /// **This site was never attempted. Its chunk ran out of re-spawn budget first.**
    ///
    /// ⚠⚠⚠ **THIS EXISTS BECAUSE THE INSTRUMENT SPENT THREE SWEEPS CALLING IT `crashed`** (t820,
    /// t821 and the aborted t824 run), and `crashed` is a **Bar 0** event that outranks every visual
    /// divergence in the priority ledger. The t820 sweep filed **118 of 200** sites `crashed` against
    /// t812's 25 and was correctly refused — but for three sessions the *cause* was read off the one
    /// message that happened to be printed next to it
    /// (`pthread_mutex_destroy failed: Device or resource busy`) and reported as a **mozjs teardown
    /// crash**. It is not. That message is what `std::process::exit` looks like when it skips
    /// `JS_ShutDown()`, and the exit is the sweep's **own per-site watchdog**, firing deliberately
    /// after a site spends its budget — with the timeout row already written to disk.
    ///
    /// The arithmetic, which is the whole defect: a chunk child exits **once per timed-out site**,
    /// and the parent's re-spawn loop was capped at a constant **4 rounds**. A 100-site bucket
    /// carrying a dozen slow sites therefore burned its budget after ~4 of them and filed *every
    /// remaining site* — 90-odd of them, most never opened — as a Bar-0 crash. The cap was a constant
    /// where the work is a variable.
    ///
    /// So this is the same lesson the `Timeout` variant above already records, one level further in:
    /// **an instrument must never charge its own bookkeeping to the engine's account.** `Timeout`
    /// was introduced so an external `SIGKILL` would stop being recovered as a phantom crash; the
    /// parent's own re-spawn cap was manufacturing the identical phantom by a different route, and
    /// nothing separated them because both ended as the string `crashed`.
    ///
    /// It counts against the bar exactly as every other unscored reason does — the denominator is
    /// unchanged and this is not an excuse tier. What changes is that a sweep can no longer report an
    /// **instrument budget** as an **engine crash**, and a reader can tell the two apart in the
    /// histogram without rebuilding an old binary to find out.
    NeverRan,
}

/// How many times a chunk's spawn-loop may re-spawn, for a bucket of `n` sites.
///
/// ⚠ **THE BUDGET MUST SCALE WITH THE WORK, AND A CONSTANT IS THE BUG** (see
/// [`Unmeasurable::NeverRan`]). A chunk child exits **deliberately, once per site that spends its
/// own budget** — the per-site watchdog writes the `timeout` row and then `process::exit(0)`s,
/// because the main thread is wedged in whatever took too long. So the number of re-spawns a bucket
/// needs is bounded by *the number of slow sites in it*, which is a fraction of `n` and is not
/// knowable in advance. `n + 4` can absorb the pathological case where **every** site times out; the
/// real stop condition is [`CHUNK_STALL_LIMIT`], not this ceiling.
///
/// ⚠ This does NOT multiply the run's cost. Every round makes at least one site's worth of progress
/// (a site either produces a row or is recovered from its in-flight marker), so total wall-clock
/// stays bounded by the sum of the per-site budgets — the re-spawn itself costs process startup.
pub fn chunk_round_budget(n: usize) -> usize {
    n.saturating_add(4)
}

/// Consecutive rounds that produce **no new row** after which a chunk is declared genuinely dead.
///
/// This is the real terminator, and it is the one that cannot be fooled by a slow corpus: a child
/// that exits because a site timed out has *written that site's row*, so it made progress. A child
/// that dies twice in a row without producing anything is failing to start.
pub const CHUNK_STALL_LIMIT: usize = 2;

impl Unmeasurable {
    /// A short stable tag for the TSV column and for grouping in the shortfall list. Stable because
    /// a sweep's rows outlive the run that wrote them.
    pub fn tag(&self) -> String {
        match self {
            Self::Unreachable => "unreachable".into(),
            Self::BotWall(c) => format!("bot-wall-{c}"),
            Self::HttpStatus(c) => format!("http-{c}"),
            Self::EmptyBody(c) => format!("empty-{c}"),
            Self::ProbeBlocked => "probe-blocked".into(),
            Self::RenderFailed => "render-failed".into(),
            Self::ShellOnly(n) => format!("shell-only-{n}"),
            Self::Timeout(secs) => format!("timeout-{secs}s"),
            Self::Crashed => "crashed".into(),
            Self::NeverRan => "never-ran".into(),
            Self::ThinOverlap(n) => format!("thin-overlap-{n}"),
            Self::TreeDivergence(n) => format!("tree-divergence-{n}"),
            Self::CssStarved(n) => format!("css-starved-{n}"),
        }
    }

    /// Read back what [`Self::tag`] wrote, so a chunked sweep keeps its reasons across the boundary.
    pub fn from_tag(s: &str) -> Option<Self> {
        let num = |p: &str| s.strip_prefix(p).and_then(|n| n.parse::<u32>().ok());
        match s {
            "unreachable" => Some(Self::Unreachable),
            "probe-blocked" => Some(Self::ProbeBlocked),
            "render-failed" => Some(Self::RenderFailed),
            "crashed" => Some(Self::Crashed),
            "never-ran" => Some(Self::NeverRan),
            _ if s.starts_with("thin-overlap-") => s["thin-overlap-".len()..]
                .parse()
                .ok()
                .map(Self::ThinOverlap),
            _ if s.starts_with("tree-divergence-") => s["tree-divergence-".len()..]
                .parse()
                .ok()
                .map(Self::TreeDivergence),
            _ if s.starts_with("timeout-") && s.ends_with('s') => s["timeout-".len()..s.len() - 1]
                .parse()
                .ok()
                .map(Self::Timeout),
            _ if s.starts_with("css-starved-") => {
                s["css-starved-".len()..].parse().ok().map(Self::CssStarved)
            }
            _ if s.starts_with("shell-only-") => {
                s["shell-only-".len()..].parse().ok().map(Self::ShellOnly)
            }
            _ => num("bot-wall-")
                .map(Self::BotWall)
                .or_else(|| num("http-").map(Self::HttpStatus))
                .or_else(|| num("empty-").map(Self::EmptyBody)),
        }
    }

    /// The operator-facing sentence: what happened, and which axis owns the fix.
    pub fn explain(&self) -> String {
        match self {
            // ⚠ This used to end "…so this is a corpus or network problem, not a rendering one",
            // and tick 658 falsified that on the FIRST row it examined. `playhop.com` booked
            // `unreachable`; curl fetched it in 2.5s and 978KB; the trace read
            // `REQUEST_HEADER_FIELDS_TOO_LARGE … send_reset(PROTOCOL_ERROR, initiator=Library)` —
            // OUR h2 client refusing a 16KiB-plus response header block. The bucket is four causes
            // wide (DNS, TLS, connect, protocol) and at least one of them is always ours, so a
            // sentence that assigns blame is the instrument charging its own defect to the corpus.
            // Name what is not known instead of asserting what is.
            Self::Unreachable => "the request never completed — no HTTP status exists. DNS, TLS, \
                 connect and PROTOCOL failures are all in this bucket and it does NOT say which: \
                 an origin we cannot reach may be dead, may be refusing us, or may be answering \
                 something we reject. Fetch it with curl before believing it is not ours"
                .into(),
            Self::BotWall(c) => format!(
                "the origin answered {c} and refused this client — a BOT WALL, not a rendering \
                 failure. Rendering work cannot move it; identity/fingerprint can"
            ),
            Self::HttpStatus(c) => format!(
                "the origin answered {c} — we rendered its error page correctly (t607), but that \
                 page is not the site, so it is not evidence about the site"
            ),
            Self::EmptyBody(c) => format!(
                "the origin answered {c} with a ZERO-BYTE body — there was no document to measure. \
                 This is the true cause behind the old 'Chrome rendered NO [id] elements' line, \
                 which blamed the corpus for the network's answer"
            ),
            Self::ProbeBlocked => {
                "the document loaded but its own Content-Security-Policy blocked \
                 the injected probe, so no boxes came back. The page is measurable in principle; \
                 the measurement channel is not"
                    .into()
            }
            Self::RenderFailed => {
                "we fetched the page and FAILED TO PAINT IT — the only reason on \
                 this list that is our own bug rather than a property of the origin, and the one \
                 that most deserves to count against the score"
                    .into()
            }
            Self::Timeout(secs) => format!(
                "a child process did not return within {secs}s and was killed. The sweep runs sites in \
                 ONE process, so an unbounded child stalls the WHOLE corpus and the run yields no \
                 certificate at all — the sites that already finished are lost with it, which is \
                 strictly worse than the silent drop the fixed-denominator rule exists to prevent"
            ),
            Self::ShellOnly(n) => format!(
                "the ORACLE rendered only {n} element(s) — a shell, not the page. Scoring this \
                 measures the shell, not the site. The parse-time-probe cause was FIXED at t674 (the \
                 probe now re-reads at DOMContentLoaded/load/T+3s), so a site still landing here is \
                 one whose scripts do not run for the ORACLE at all — check whether the snapshot \
                 fetch was bot-walled before treating it as evidence about the site"
            ),
            Self::ThinOverlap(n) => format!(
                "the oracle rendered the page and only {n} element(s) are COMMON to both engines, \
                 which is below the sample floor — so there is nothing to compute a placement ratio \
                 over. Unlike shell-only this is OURS: the oracle built the page and we did not, so \
                 the missing elements are a coverage failure wearing an 'unscored' label"
            ),
            Self::TreeDivergence(n) => format!(
                "both engines built a page — WE produced {n} box-bearing element(s), well above the \
                 shell floor — and they still share almost none of the same paths. That is not 'we \
                 rendered less'; it is two different documents, or the same document caught in two \
                 different states (the oracle renders a curl SNAPSHOT from file://, we render the \
                 LIVE url), or one inserted SAME-TAG sibling near the root re-numbering the \
                 nth-of-type keys beneath it. UNSCORED and counted against the bar exactly as thin-overlap is — but \
                 do NOT take it as a coverage bug to grind: the evidence does not say that"
            ),
            Self::CssStarved(n) => format!(
                "{n} render-blocking author stylesheet(s) NEVER ARRIVED before the load deadline, so \
                 the page measured is the UA stylesheet's idea of the document, not the author's \
                 design — the giveaway is coverage ~1.000 with shape ~0 (every element present, \
                 because there was no CSS to drop it; almost none placed, because there was no CSS to \
                 place it). The ORACLE has always discarded these runs; this instrument scored them, \
                 which charged network weather to the layout engine's account. OURS and IN-SCOPE: the \
                 sheets were cut by our own load deadline, not refused by the origin"
            ),
            Self::Crashed => "THE SWEEP PROCESS DIED while rendering this site (SIGSEGV/OOM/kill) — \
                 our own bug, like render-failed, and the most expensive kind: it takes the whole \
                 corpus down with it. Recovered from the in-flight marker on the following run, so \
                 the site is COUNTED rather than lost along with the run it killed"
                .into(),
            Self::NeverRan => "THIS SITE WAS NEVER ATTEMPTED — its chunk ran out of re-spawn budget \
                 first, which is an INSTRUMENT fault and not a Bar-0 crash. A chunk child exits \
                 deliberately once per site that spends its own budget (the watchdog writes the \
                 timeout row, then `process::exit(0)` because the main thread is wedged), so a bucket \
                 with many slow sites needs many re-spawns. It counts against the bar like every \
                 other unscored reason — but if a run has MANY of these, the run is not measuring the \
                 engine, and no band from it is bankable"
                .into(),
        }
    }
}

impl std::fmt::Display for Unmeasurable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag())
    }
}

/// Classify a completed fetch. Split out from the fetching so it is testable without a network:
/// the classification is the load-bearing judgement, and a rule that can only be exercised against
/// the live internet is a rule nobody re-checks.
///
/// `None` means measurable — a 2xx with a body.
///
/// **Challenge markers matter only for 5xx.** 401/403/429 are refusals of this client whatever the
/// vendor, so they need no marker. A 503, by contrast, is genuinely ambiguous — an origin can be
/// down — so it is only called a bot wall when the body says so.
pub fn classify_fetch(status: u32, body: &str) -> Option<Unmeasurable> {
    const CHALLENGE: [&str; 4] = [
        "Just a moment",
        "cf-browser-verification",
        "challenges.cloudflare.com",
        "Attention Required",
    ];
    // ⚠⚠ **A BOT CHALLENGE ARRIVES WITH HTTP 200, AND WE WERE BILLING IT TO THE ENGINE.**
    //
    // Every bot-wall rule above keys off a STATUS — 401/403/429, or a 5xx carrying a challenge
    // marker. Cloudflare's interstitial is none of those: it is **`200 OK`** with a 5.5 KB body
    // titled *"Just a moment…"* that renders as a near-empty spinner. So it fell through to
    // `None` == measurable, painted almost nothing, and the sweep booked it `render-failed` — the
    // one reason on this list documented as *"our own bug rather than a property of the origin, and
    // the one that most deserves to count against the score."* It is the exact opposite: it is the
    // origin refusing us as a client, which is what `BotWall` exists to say.
    //
    // Measured (t709, raw response bytes via `boxes --dump-html`, not inferred from the DOM):
    //
    // ```text
    //   serverfault.com    ours 5,491 B  "Just a moment..."   ·  curl 205,345 B  43 scripts
    //   askubuntu.com      ours 5,489 B  "Just a moment..."
    //   mathoverflow.net   ours 5,492 B  "Just a moment..."
    //   theverge / vox / mongodb / kotlinlang / notion   ours 250 KB–963 KB, the REAL document
    // ```
    //
    // That split is the finding: `render-failed` held **two populations**, and only the second is
    // ours. It also explains the intermittency t707 could not — `superuser.com` scored
    // `render-failed` in the sweep and `ok` on re-run because it is sometimes served the challenge
    // and sometimes the page.
    //
    // ⚠ The 2xx test uses the INFRASTRUCTURE markers only, never the prose ones. `"Just a moment"`
    // and `"Attention Required"` are English sentences that a real 200-page may legitimately
    // contain, and mislabelling a genuine render failure as a bot wall would EXCUSE our own bug —
    // the more expensive direction of this error, and the one the fixed-denominator rule exists to
    // prevent. `challenges.cloudflare.com` and `cf-browser-verification` cannot appear by accident.
    const CHALLENGE_INFRA: &[&str] = &["challenges.cloudflare.com", "cf-browser-verification"];
    match status {
        200..=299 if body.trim().is_empty() => Some(Unmeasurable::EmptyBody(status)),
        200..=299 if CHALLENGE_INFRA.iter().any(|m| body.contains(m)) => {
            Some(Unmeasurable::BotWall(status))
        }
        200..=299 => None,
        401 | 403 | 429 => Some(Unmeasurable::BotWall(status)),
        s if (500..=599).contains(&s) && CHALLENGE.iter().any(|m| body.contains(m)) => {
            Some(Unmeasurable::BotWall(status))
        }
        s => Some(Unmeasurable::HttpStatus(s)),
    }
}

/// Per-page fidelity result — **two** numbers on purpose.
///
/// This session proved repeatedly that a pixel score alone is a poor proxy for correctness: an
/// entirely absent sidebar moved Wikipedia's visual score by <1 point. A missing element is a
/// missing **box**, so the structural half compares Chrome's `getBoundingClientRect` for every
/// `[id]` element against Manuk's, and reports what is MISSING and what is MISPLACED. That number
/// cannot be fooled by white matching white.
#[derive(Clone)]
pub struct Fidelity {
    pub name: String,
    /// Visual: fraction of grid blocks agreeing with Chromium, 0.0–1.0.
    pub score: f64,
    pub differing: usize,
    pub total: usize,
    /// **Structural COVERAGE**: of the elements Chrome renders, what fraction does Manuk render at
    /// all? This is the honest number — a missing region cannot hide in it. `None` if unprobed.
    pub structure: Option<f64>,
    /// **Layer-1 SHAPE** (parent-relative placement, `shape_stats`): of the elements BOTH engines
    /// render, what fraction sits in the right place *relative to its nearest shared ancestor*. This
    /// is the redesign's primary placement number — it cancels a constant page offset that the old
    /// absolute `placement_stats` charged N times. `None` if unprobed. (tick 532)
    pub shape: Option<f64>,
    /// Elements Chrome renders that Manuk does **not** produce a box for at all.
    pub missing: usize,
    /// Elements both render, but Manuk places/sizes wrongly (beyond tolerance).
    pub misplaced: usize,
    pub probed: usize,
    /// **The four JARRING invariants** (FIDELITY-SCORING-REDESIGN.md §2), as counts per site:
    /// horizontal overflow · sibling overlap · reading-order inversion · collapsed interactive
    /// target. They were computed and *printed* per site since brick 4b and then thrown away, so the
    /// certificate — whose bar is *"≥95% of sites CLEAN on each invariant"* — could not be computed
    /// from a sweep at all. A number printed and discarded is not a measurement, it is a log line.
    pub jarring: [usize; 4],
    /// **How many elements the SHAPE score was computed over.** Load-bearing, not diagnostic: with no
    /// sample, `shape_stats` returns the ratio `0/0` as **1.0**, and the first real sweep duly reported
    /// seven sites at `SHAPE: 100.0% … (0 scored)` — including `gov.uk`, where all 418 probed elements
    /// were MISSING. A page we render nothing of scored a perfect placement. The certificate cannot
    /// accept that, and it cannot detect it from the ratio alone, so the sample size travels with it.
    pub shape_n: usize,
    /// **Why this site could not be measured**, when it could not be. `None` on a site that reached
    /// the scorer — including one that scored badly, which is a *result* and not an absence.
    ///
    /// The certificate has counted UNSCORED sites against the bar since t583, which was the
    /// important half. But "9 of 14 UNSCORED" with no decomposition is a number that cannot be
    /// worked: bot-wall, dead URL, empty body and CSP-blocked probe are four different jobs owned by
    /// four different parts of this project, and they were all printing the same `—`.
    pub unmeasurable: Option<Unmeasurable>,
}

impl Fidelity {
    /// A **counted** row for a site that could not be measured at all.
    ///
    /// The point is that it EXISTS. A site we could not reach used to `continue` out of the sweep
    /// loop and leave no row, so it silently left the denominator as well — the sweep reported
    /// "sites N" over however many origins happened not to refuse us that day. §0 of the
    /// certification design names that as cause #1 of every historically flattering number.
    ///
    /// Scores are `None` rather than 0.0 on purpose: zero is a *measurement* that we rendered
    /// nothing, and we did not measure. The jarring invariants are 0 divergences because none were
    /// observed — and `certificate` skips this row's shape term entirely on the reason, so the
    /// zeros cannot be read as four clean passes.
    pub fn unmeasured(name: &str, reason: Unmeasurable) -> Self {
        Fidelity {
            name: name.to_string(),
            score: f64::NAN,
            differing: 0,
            total: 0,
            structure: None,
            shape: None,
            missing: 0,
            misplaced: 0,
            probed: 0,
            jarring: [0; 4],
            shape_n: 0,
            unmeasurable: Some(reason),
        }
    }
}

/// The four jarring invariants, in the order they sit in [`Fidelity::jarring`]. Named so a report
/// cannot silently reorder them and relabel three columns at once.
pub const JARRING_NAMES: [&str; 4] = ["h-overflow", "overlap", "reading-order", "dead-target"];

/// The Phase-0 exit certificate, evaluated over a sweep's rows (FIDELITY-SCORING-REDESIGN.md §3).
///
/// This exists because the certificate was written in prose and the instrument printed per-site
/// lines: turning one into the other was a human reading 265 stanzas of stderr, which is exactly the
/// kind of step that gets skipped and then estimated. The bar is **mechanical** — *shape ≥ 0.75 on
/// ≥95% of sites, and ≥95% of sites clean on each jarring invariant* — so it is computed here, once,
/// by the thing that measured it.
#[derive(Debug, Default, PartialEq)]
pub struct Cert {
    /// Sites with a SHAPE score at all (an unprobeable page is not a passing page — it is excluded
    /// from the numerator AND named, never averaged in).
    pub scored: usize,
    /// Sites in the sweep, including the ones that could not be scored.
    pub sites: usize,
    /// Sites at or above the shape floor.
    pub shape_ok: usize,
    /// Sites with ZERO divergences on each invariant, in [`JARRING_NAMES`] order.
    pub clean: [usize; 4],
    /// **The unscored count, decomposed by cause** — `(reason tag, sites)`, most common first.
    ///
    /// The pilot's binding constraint was *"9 of 14 could not be scored"*, and the observer's next
    /// order was to find out why. A single total cannot answer that and cannot be worked: a bot wall
    /// is the identity axis, a dead URL is corpus construction, a CSP-blocked probe is the
    /// measurement channel, and an empty body is neither. Sorted by count so the list reads as a
    /// priority order rather than a set.
    pub unmeasured_by_reason: Vec<(String, usize)>,
}

/// The certificate's shape floor and its site-fraction bar — the two numbers the exit rule is
/// written in. Constants, not parameters, because *"widen the bar to pass"* is the one move this
/// project refuses; a floor that a caller can pass in is a floor that will eventually be passed in.
pub const CERT_SHAPE_FLOOR: f64 = 0.75;
pub const CERT_SITE_BAR: f64 = 0.95;
/// The minimum number of elements a SHAPE score must be computed over to count as a verdict.
///
/// **This is the vacuous-pass guard, and it exists because the certificate's FIRST real sweep failed
/// it.** `shape_stats` computes a ratio; over an empty sample that ratio is `1.0`, so seven of 55 sites
/// came back `SHAPE: 100.0% … (0 scored)` and were counted as *passing the placement bar* — one of them
/// (`gov.uk`) with all 418 probed elements MISSING. **A page we render nothing of must never score
/// perfect placement.** Ten matches `scripts/fidelity-sweep.sh`'s own `LOW_SAMPLE` threshold, which was
/// added to that script for precisely this reason; a sub-threshold site is UNSCORED, which counts
/// against the bar rather than out of it.
pub const CERT_MIN_SHAPE_SAMPLE: usize = 10;

/// Evaluate the certificate over a sweep's rows.
///
/// **Unscored sites count against the site bar, not out of it.** A page Chrome could not be probed on
/// (or one we failed to render) is a page we cannot claim; dividing by `scored` instead of `sites`
/// would let the bar be met by failing to measure, which is the same defect
/// `fidelity::report`'s NaN check was added for.
pub fn certificate(rows: &[Fidelity]) -> Cert {
    let mut c = Cert {
        sites: rows.len(),
        ..Default::default()
    };
    for r in rows {
        // **A SITE WE NEVER REACHED CANNOT BE SCORED, WHATEVER NUMBERS ARE ATTACHED TO IT.**
        //
        // Today this is belt-and-braces: a failed probe leaves `shape` at `None`, so a refused site
        // falls out anyway. It is written as an explicit term regardless, because "unscored" is
        // currently true by ACCIDENT of the control flow rather than by rule — and the accident is one
        // edit away from reversing. The tempting edit is a real one: for a 403 we *do* hold a
        // document (Cloudflare's challenge page), both engines render it, and they agree. Scoring
        // that would report high fidelity on a site we never reached — a gate passing by comparing a
        // refusal against itself. The rule has to outrank the flow.
        if r.unmeasurable.is_some() {
            continue;
        }
        if let Some(s) = r.shape {
            // A ratio over an empty (or trivially small) sample is not a measurement of placement — it
            // is arithmetic on nothing. Such a site is UNSCORED, never a pass.
            if !s.is_nan() && r.shape_n >= CERT_MIN_SHAPE_SAMPLE {
                c.scored += 1;
                if s >= CERT_SHAPE_FLOOR {
                    c.shape_ok += 1;
                }
            }
        }
        for i in 0..4 {
            if r.jarring[i] == 0 {
                c.clean[i] += 1;
            }
        }
    }
    let mut by_reason: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for r in rows {
        if let Some(u) = &r.unmeasurable {
            *by_reason.entry(u.tag()).or_default() += 1;
        }
    }
    c.unmeasured_by_reason = by_reason.into_iter().collect();
    // Most common first: the list is a work order, and the biggest cause is the first job.
    c.unmeasured_by_reason
        .sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    c
}

impl Cert {
    fn frac(n: usize, d: usize) -> f64 {
        if d == 0 {
            0.0
        } else {
            n as f64 / d as f64
        }
    }
    pub fn shape_frac(&self) -> f64 {
        Self::frac(self.shape_ok, self.sites)
    }
    pub fn clean_frac(&self, i: usize) -> f64 {
        Self::frac(self.clean[i], self.sites)
    }
    /// Does the certificate HOLD? Every term at or above the bar — one failing term fails it, which
    /// is the point of a certificate rather than an average.
    pub fn holds(&self) -> bool {
        self.sites > 0
            && self.scored == self.sites
            && self.shape_frac() >= CERT_SITE_BAR
            && (0..4).all(|i| self.clean_frac(i) >= CERT_SITE_BAR)
    }
    /// The terms that are BELOW the bar, named. An unmet certificate must say which term missed, or
    /// the next tick is chosen by guesswork.
    pub fn shortfalls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.scored != self.sites {
            let named: usize = self.unmeasured_by_reason.iter().map(|(_, n)| n).sum();
            let by = if self.unmeasured_by_reason.is_empty() {
                String::new()
            } else {
                format!(
                    " — {}",
                    self.unmeasured_by_reason
                        .iter()
                        .map(|(t, n)| format!("{n}×{t}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            out.push(format!(
                "{} of {} sites UNSCORED (cannot be claimed, counted against the bar){}",
                self.sites - self.scored,
                self.sites,
                by
            ));
            // **The residue is itself a finding, and it must not round to zero.** A site that failed
            // to score with NO reason attached is one the instrument could not explain — the exact
            // state this tick exists to end. Naming it keeps the gap visible instead of letting the
            // decomposition look complete because the named causes are the only ones printed.
            let unexplained = (self.sites - self.scored).saturating_sub(named);
            if unexplained > 0 {
                out.push(format!(
                    "{unexplained} of those UNSCORED sites have NO recorded reason — the instrument \
                     could not say why, which is an instrument gap, not a result"
                ));
            }
        }
        if self.shape_frac() < CERT_SITE_BAR {
            out.push(format!(
                "shape ≥{:.2} on {:.1}% of sites (bar {:.0}%)",
                CERT_SHAPE_FLOOR,
                self.shape_frac() * 100.0,
                CERT_SITE_BAR * 100.0
            ));
        }
        for i in 0..4 {
            if self.clean_frac(i) < CERT_SITE_BAR {
                out.push(format!(
                    "{} clean on {:.1}% of sites (bar {:.0}%)",
                    JARRING_NAMES[i],
                    self.clean_frac(i) * 100.0,
                    CERT_SITE_BAR * 100.0
                ));
            }
        }
        out
    }
}

/// One machine-readable row per site, for a CHUNKED sweep.
///
/// A 265-site sweep cannot run in one process: a single hanging site takes the whole batch with it, and
/// `timeout` only isolates cleanly at process granularity. So the sweep runs in chunks — and then each
/// chunk prints its own certificate over its own five sites, which is not the number anyone wants. The
/// fix is not to have a human add up 53 stanzas (that is the exact failure `certificate` was just
/// written to end); it is to make the instrument APPEND rows and compute the certificate over the
/// accumulated file.
///
/// Tab-separated, append-only, `#`-commented header written once: `name coverage shape j0 j1 j2 j3`.
/// A site whose shape could not be scored writes `-`, and [`rows_from_tsv`] reads that back as `None`
/// so an unscored site keeps counting against the bar across the chunk boundary too.
pub fn append_rows_tsv(path: &Path, rows: &[Fidelity]) -> Result<()> {
    use std::io::Write;
    let fresh = !path.exists();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    if fresh {
        writeln!(
            f,
            "#name\tcoverage\tshape\th_overflow\toverlap\treading_order\tdead_target\tshape_n\treason\tinstrument"
        )?;
    }
    for r in rows {
        let num = |v: Option<f64>| match v {
            Some(x) if !x.is_nan() => format!("{x:.6}"),
            _ => "-".to_string(),
        };
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.name,
            num(r.structure),
            num(r.shape),
            r.jarring[0],
            r.jarring[1],
            r.jarring[2],
            r.jarring[3],
            r.shape_n,
            // The reason travels WITH the row. A chunked sweep writes rows and a later
            // `certificate --rows` reads them back, so a reason that lived only in the running
            // process would vanish at exactly the moment the headline is computed.
            r.unmeasurable.as_ref().map(|u| u.tag()).unwrap_or_default(),
            // ...and so does WHICH INSTRUMENT measured it. Two readings of one site from two
            // different oracle probes are not two draws from one distribution, and without this
            // column nothing downstream could tell those apart — see `chrome::instrument_tag`.
            crate::chrome::instrument_tag()
        )?;
    }
    Ok(())
}

/// The sidecar naming the site currently being rendered — the only way a crash can be attributed.
///
/// A process that dies mid-site writes nothing, so the fact of the crash has to be recorded
/// *before* the work, by whoever is about to do it. Written before each site and removed once its
/// row is durable; anything left behind is, by construction, a site that killed the run.
pub fn inflight_path(rows: &Path) -> std::path::PathBuf {
    let mut p = rows.as_os_str().to_owned();
    p.push(".inflight");
    std::path::PathBuf::from(p)
}

/// Claim a site as in-flight. Flushed to the OS before returning: a marker still sitting in this
/// process's buffer when the process dies is a marker that never existed.
pub fn mark_inflight(rows: &Path, name: &str) -> Result<()> {
    use std::io::Write;
    let p = inflight_path(rows);
    let mut f = std::fs::File::create(&p).with_context(|| format!("create {}", p.display()))?;
    writeln!(f, "{name}")?;
    f.flush()?;
    Ok(())
}

/// Release the claim — the site's row is on disk, so it did not crash.
pub fn clear_inflight(rows: &Path) {
    let _ = std::fs::remove_file(inflight_path(rows));
}

/// Convert a leftover in-flight marker into a COUNTED [`Unmeasurable::Crashed`] row, and clear it.
///
/// Called at the start of a run, so the site that killed the *previous* run enters the denominator
/// instead of vanishing from it. Returns the recovered site name, if there was one.
pub fn recover_inflight(rows: &Path) -> Option<String> {
    let p = inflight_path(rows);
    let name = std::fs::read_to_string(&p).ok()?.trim().to_string();
    let _ = std::fs::remove_file(&p);
    if name.is_empty() {
        return None;
    }
    let row = Fidelity::unmeasured(&name, Unmeasurable::Crashed);
    append_rows_tsv(rows, std::slice::from_ref(&row)).ok()?;
    Some(name)
}

/// Read back what [`append_rows_tsv`] wrote. Only the fields the certificate scores are restored —
/// this is deliberately NOT a full round-trip of `Fidelity`, because a partial reader that silently
/// returned zeros for the visual score would let a later report print a number nobody measured.
///
/// **A site appearing twice is ONE site.** The file is append-only and resumable, so a sweep that
/// crashed at site 11 and was re-run contributes two rows for sites 1-10 — and `certificate()` takes
/// `sites` straight from `rows.len()`, so without this the denominator would *grow* every time a run
/// was resumed. That is the fixed-denominator rule failing in the generous direction instead of the
/// flattering one, which is no better: a denominator nobody chose is the defect, whichever way it
/// moves.
///
/// **Which row survives depends on whether the repeat was a RE-MEASURE or a REPEAT, and those are
/// two different events that the old single rule ran together.**
///
/// * **Across** occurrences — rows separated by other sites — the LAST wins. The later row is the
///   re-measurement: it is how a recovered `crashed` row is superseded once the site is successfully
///   rendered on a later pass, and how a resumed sweep's second attempt beats its first.
/// * **Within** one consecutive run — which is what [`repeat_urls`] deliberately produces for a site
///   the spread block calls unstable — the **MEDIAN by SHAPE** wins. Those rows are `n` draws from
///   one distribution on one tree, and last-wins hands the certificate whichever draw the sweep
///   happened to finish on. Tick 672 measured `keirin.jp` at **0.048 against a ~0.40 population**;
///   three controls minutes later on the same tree read 0.400 / 0.351 / 0.402. Under last-wins, a
///   sweep that drew the outlier last would have published a 35-point regression against the
///   previous tick's own work. The median is the whole point of paying for the repeats.
///
/// With an even count there is no middle element and the **LOWER** of the two is taken. A bar must
/// never be cleared by a rounding convention.
pub fn rows_from_tsv(path: &Path) -> Result<Vec<Fidelity>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            anyhow::bail!("malformed row (want 7 fields, got {}): {line}", f.len());
        }
        let opt = |s: &str| s.parse::<f64>().ok();
        let usz = |s: &str| s.parse::<usize>().unwrap_or(0);
        out.push(Fidelity {
            name: f[0].to_string(),
            score: f64::NAN,
            differing: 0,
            total: 0,
            structure: opt(f[1]),
            shape: opt(f[2]),
            missing: 0,
            misplaced: 0,
            probed: 0,
            jarring: [usz(f[3]), usz(f[4]), usz(f[5]), usz(f[6])],
            // A row written before `shape_n` existed has no 8th field. It reads back as 0, which is
            // BELOW the sample floor, so an old row is UNSCORED rather than silently trusted — the
            // conservative direction, and the only one that cannot resurrect a vacuous pass.
            shape_n: f.get(7).map(|v| usz(v)).unwrap_or(0),
            // Absent or empty on a row written before reasons existed — which reads back as "no
            // reason recorded", not as "measurable". The site is still UNSCORED and still counts
            // against the bar; only the explanation is missing, which is the honest state.
            unmeasurable: f.get(8).and_then(|v| Unmeasurable::from_tag(v)),
        });
    }
    Ok(collapse_repeats(out))
}

/// **ONE ROW PER SITE**, by the two rules `rows_from_tsv` documents: a consecutive run collapses to
/// its median, and what survives that collapses last-wins.
///
/// Public, and called by the SWEEP as well as by the reader, because those two used to disagree the
/// moment repeats existed. The sweep prints its own certificate from the `Vec<Fidelity>` it just
/// built, and the first live run of [`repeat_urls`] duly reported **`sites 4`** for a two-site
/// corpus — the fixed-denominator rule broken by the very change that was supposed to make the
/// numerator honest. Reconciliation caught it, not a gate, which is this project's most informative
/// statistic and the reason the collapse is a shared function rather than a step in a reader.
pub fn collapse_repeats(rows: Vec<Fidelity>) -> Vec<Fidelity> {
    // A CONSECUTIVE run of one site is `n` draws from one distribution: collapse it to its median
    // BEFORE the last-wins pass ever sees it. Doing it in this order is what keeps the two rules
    // from fighting — the median settles a repeat, last-wins then settles a re-measure.
    let rows = collapse_consecutive_repeats(rows);
    // Order follows FIRST appearance, so an accumulated file still reads in sweep order.
    let mut order: Vec<String> = Vec::new();
    let mut latest: std::collections::HashMap<String, Fidelity> = std::collections::HashMap::new();
    for r in rows {
        if !latest.contains_key(&r.name) {
            order.push(r.name.clone());
        }
        latest.insert(r.name.clone(), r);
    }
    order
        .into_iter()
        .filter_map(|n| latest.remove(&n))
        .collect()
}

/// **The SHAPE spread of every site this file measured more than once** — `(name, min, max, runs)`,
/// worst spread first, and only for sites with a real repeat and a real score.
///
/// [`rows_from_tsv`] collapses repeats to the last row, which is the right tie-break for a resumed
/// sweep and **throws away the only evidence of the instrument's own error bar.** That mattered:
/// tick 657 re-ran two live sites three times each on ONE unchanged tree and measured
///
/// ```text
///   keirin.jp      0.3673 .. 0.4044   Δ 3.7 pts over 3 runs
///   www.ikea.com   0.5158 .. 0.5186   Δ 0.3 pts over 3 runs
/// ```
///
/// — so a 0.7-point per-site "regression" the loop was about to attribute to a code change was five
/// times inside one site's own noise. A live page is not a fixture: its ads, its prices and its node
/// count move between runs. **A per-site delta smaller than that site's own spread is not a small
/// result; it is not a result.** The number exists now instead of being rediscovered by hand, which
/// is the difference between an instrument and a habit.
pub fn shape_spreads(rows_text: &str) -> Vec<(String, f64, f64, usize)> {
    // ── **ONLY ROWS FROM THE SAME INSTRUMENT ARE DRAWS FROM THE SAME DISTRIBUTION** (tick 676).
    //
    // The rows file is append-only and accumulates ACROSS ticks, so it can hold readings taken by
    // two different oracles. Tick 674 deferred both live probes to `load` — a change to the
    // population the oracle collects, not to the page — and this block then printed the step change
    // as the site's own noise: naukri Δ100.0 pts, agoda Δ58.6, keirin Δ52.6, playhop Δ43.6, on a
    // corpus whose real per-site spreads had every previous reading at ≤3.7 pts. Worse, it is not
    // only a mis-read: `repeat_plan` reads this function, so all four sites would have been rendered
    // three times on every future sweep, forever, to re-measure a variance that is not variance.
    //
    // The LAST tag in the file is the current instrument (the sweep appends). Rows carrying any other
    // tag — including the empty tag of a file written before this column existed — are **history**,
    // not draws: they still supersede one another for `rows_from_tsv`'s last-wins, and they
    // contribute nothing to an error bar. A file with no tags at all keeps the old behaviour exactly,
    // which is what makes this safe on the sweeps already banked in `docs/bench/`.
    let current: Option<&str> = rows_text
        .lines()
        .filter(|l| !l.trim_end().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.trim_end().split('\t').nth(9))
        .filter(|t| !t.is_empty())
        .last();
    let mut by_site: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for line in rows_text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        if let Some(cur) = current {
            if f.get(9).copied().unwrap_or("") != cur {
                continue;
            }
        }
        // An unscored row contributes NOTHING to a spread. A site that rendered once and bot-walled
        // once has not been measured twice — reading `-` as a score would manufacture a spread of
        // the site's whole range out of a row that never had a number.
        let Some(shape) = f[2].parse::<f64>().ok() else {
            continue;
        };
        let name = f[0].to_string();
        if !by_site.contains_key(&name) {
            order.push(name.clone());
        }
        by_site.entry(name).or_default().push(shape);
    }
    let mut out: Vec<(String, f64, f64, usize)> = order
        .into_iter()
        .filter_map(|n| {
            let v = by_site.remove(&n)?;
            if v.len() < 2 {
                return None;
            }
            let min = v.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Some((n, min, max, v.len()))
        })
        .collect();
    // Worst spread first: the site whose number is least trustworthy is the one to read first.
    out.sort_by(|a, b| (b.2 - b.1).total_cmp(&(a.2 - a.1)));
    out
}

/// Every instrument version present in a rows file, in FIRST-APPEARANCE order, with its row count.
/// The last entry is the current one (the sweep appends). An untagged row — written before the
/// column existed — is its own version, named `""`, because "unknown instrument" is a fact about the
/// row and not a licence to pool it with today's.
pub fn instrument_mix(rows_text: &str) -> Vec<(String, usize)> {
    let mut order: Vec<String> = Vec::new();
    let mut count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in rows_text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tag = line.split('\t').nth(9).unwrap_or("").to_string();
        if !count.contains_key(&tag) {
            order.push(tag.clone());
        }
        *count.entry(tag).or_insert(0) += 1;
    }
    order
        .into_iter()
        .map(|t| {
            let n = count[&t];
            (t, n)
        })
        .collect()
}

/// Print the spread block, if this file has one. Deliberately printed ABOVE the certificate: a
/// reader who sees the headline first has already formed an opinion about a delta.
pub fn spread_report(rows_text: &str) {
    // **SAY WHEN THE FILE HOLDS MORE THAN ONE INSTRUMENT.** `shape_spreads` now excludes the older
    // ones from the error bar, and an exclusion nobody is told about reads as "everything was
    // included" — which is the silent-truncation failure this project has a standing rule against.
    // The certificate still counts a superseded row for a site the current instrument never reached:
    // that is a reading, and dropping it would shrink the denominator. Both facts are printed.
    let tags = instrument_mix(rows_text);
    if tags.len() > 1 {
        println!("\n  ⚠ THIS FILE HOLDS {} INSTRUMENT VERSIONS:", tags.len());
        for (tag, n) in &tags {
            let label = if tag.is_empty() { "(untagged)" } else { tag };
            println!("      {label:<12} {n} row(s)");
        }
        println!(
            "    Only the LAST version's rows form the error bar below — a step change in the \n\
             \x20   instrument is not an error bar on the subject. Rows from an older version still \n\
             \x20   count in the certificate for any site the current one never reached."
        );
    }
    let spreads = shape_spreads(rows_text);
    if spreads.is_empty() {
        return;
    }
    println!("\n  ⚠ INSTRUMENT SPREAD — sites this file measured more than once:");
    for (name, min, max, runs) in &spreads {
        println!(
            "      {name:<28} {min:.4} .. {max:.4}   Δ {:.1} pts over {runs} runs",
            (max - min) * 100.0
        );
    }
    println!(
        "    A per-site delta smaller than that site's own spread is NOISE, not a result. Live \n\
         \x20   pages move between runs; only a fixture is entitled to a single reading."
    );
}

/// Collapse every MAXIMAL CONSECUTIVE run of one site to a single row: the **median by SHAPE**.
///
/// Only the *scored* draws vote. A run of three where one bot-walled was measured twice, and letting
/// the unscored row take part would either drag the median down by counting a non-number as low or
/// — worse — let a `-` row win the middle position and erase two real measurements. If a run has no
/// scored draw at all the LAST row is kept, which is exactly the old behaviour for the population
/// that never had a number to take a median of.
fn collapse_consecutive_repeats(rows: Vec<Fidelity>) -> Vec<Fidelity> {
    let mut out: Vec<Fidelity> = Vec::with_capacity(rows.len());
    let mut run: Vec<Fidelity> = Vec::new();
    let flush = |run: &mut Vec<Fidelity>, out: &mut Vec<Fidelity>| {
        if run.is_empty() {
            return;
        }
        if run.len() == 1 {
            out.push(run.remove(0));
            return;
        }
        let mut scored: Vec<usize> = (0..run.len()).filter(|&i| run[i].shape.is_some()).collect();
        if scored.is_empty() {
            out.push(run.pop().expect("run is non-empty"));
            run.clear();
            return;
        }
        // ── **TICK 681'S POPULATION-COLLAPSE FILTER IS RETRACTED, AND THE RETRACTION IS THE POINT**
        //    (tick 682).
        //
        // t681 read `www.agoda.com`'s three draws — `shape_n` 65, 10, 10 — and concluded that a 6.5×
        // change in the count meant *the ORACLE had built a different document*, so the two thin draws
        // were disqualified from voting. The certificate then read 0.508 for agoda instead of 0.100,
        // and `scored` went 5 → 6.
        //
        // **The log of that same sweep falsifies it.** `shape_n` is the count of paths COMMON to both
        // engines, not the oracle's population, and the oracle's population was identical across all
        // three draws:
        //
        // ```text
        //   www.agoda.com   structural: 8.0% (808 paths, 743 missing, 63 misplaced)   -> shared 65
        //   www.agoda.com   structural: 1.2% (808 paths, 798 missing,  9 misplaced)   -> shared 10
        //   www.agoda.com   structural: 1.2% (808 paths, 798 missing,  9 misplaced)   -> shared 10
        //   www.naukri.com  structural:17.5% ( 57 paths,  47 missing, 10 misplaced)   -> shared 10
        //   www.naukri.com  structural:15.8% ( 57 paths,  48 missing,  9 misplaced)   -> shared  9
        // ```
        //
        // 808 paths in every agoda draw; 57 in every naukri draw. **The document never changed. The
        // variance is OURS** — our own render shared 65 paths on one draw and 10 on the next — which is
        // exactly the variance `repeat_plan` exists to sample and exactly what the MEDIAN is for.
        //
        // So the filter was discarding our own bad draws and keeping our best one, which is the
        // flattering direction this whole file exists to close. It moved a certificate term on the tick
        // that introduced it, off a premise nobody had checked against the log sitting next to it.
        // **A rule whose justification is falsified by the run that motivated it is not a rule.**
        //
        // The one thing t681 got right is kept, in the CONSERVATIVE direction: draws that tie at the
        // median shape are ordered by nothing, and on `www.naukri.com` (n = 10, 9, 9, every draw shape
        // 0.0) that arbitrariness decided whether the site cleared `CERT_MIN_SHAPE_SAMPLE` at all. The
        // tie now breaks toward the **SMALLEST** sample — the same principle as taking the lower middle
        // of an even run: *a bar must never be cleared by a convention*, and choosing the largest
        // sample here is choosing the draw that helps, which is how t681 went wrong in the first place.
        scored.sort_by(|&a, &b| {
            run[a]
                .shape
                .unwrap_or(f64::NAN)
                .total_cmp(&run[b].shape.unwrap_or(f64::NAN))
        });
        // (k - 1) / 2 is the LOWER middle when k is even. See `rows_from_tsv`'s doc comment: a
        // certificate that rounds toward its own bar is the flattering direction, and this file
        // exists to close those.
        let median_at = scored[(scored.len() - 1) / 2];
        let median_shape = run[median_at].shape;
        let pick = scored
            .iter()
            .copied()
            .filter(|&i| run[i].shape == median_shape)
            .min_by_key(|&i| run[i].shape_n)
            .unwrap_or(median_at);
        out.push(run[pick].clone());
        run.clear();
    };
    for r in rows {
        if run.first().is_some_and(|f: &Fidelity| f.name != r.name) {
            flush(&mut run, &mut out);
        }
        run.push(r);
    }
    flush(&mut run, &mut out);
    out
}

/// The SHAPE spread, **in points**, above which a site's certificate row may not come from one draw.
///
/// Placed between two measured populations rather than picked to land a number. Tick 657 re-ran two
/// live sites three times each on one unchanged tree and got `keirin.jp` Δ**3.7 pts**, `www.ikea.com`
/// Δ**0.3 pts**; tick 672's eight-site spread block put five of eight sites at Δ ≤ 0.3 and produced
/// `keirin.jp` Δ**34.9 pts**. So 5.0 is comfortably above every spread that has ever been merely
/// noisy and a factor of seven below the one that was catastrophic.
///
/// The reason it is not set at 3.7 — keirin's own honest run-to-run range — is that a spread only
/// costs the certificate something if it could change a TERM, and every one of keirin's calm
/// readings (0.367…0.404) sits the same distance below [`CERT_SHAPE_FLOOR`]. Repeating a site whose
/// wobble cannot flip a verdict buys precision nobody reads and triples the cost of the sweep.
pub const SPREAD_UNSTABLE_PTS: f64 = 5.0;

/// How many times an unstable site is rendered in one sweep. Odd on purpose — an even count has no
/// middle draw, and the whole value of the repeat is that the middle draw exists.
pub const UNSTABLE_REPEATS: usize = 3;

/// What a `--urls-file` corpus actually yielded — **the URLs and the number of lines that were
/// CANDIDATES to be URLs**, which is the second population the caller needs in order to tell a
/// legitimately small corpus from a parse that ate the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusParse {
    pub urls: Vec<String>,
    /// Non-blank, non-comment lines. `urls.len() < candidates` means lines were DROPPED.
    pub candidates: usize,
}

/// Parse a corpus file: one URL per line, `#` comments and blanks ignored, and an optional leading
/// `category<whitespace>url` (the shape of `docs/bench/oracle-corpus.txt`) reduced to the URL.
///
/// ⚠ **The whitespace is the bug this exists to hold still.** `oracle-corpus.txt` is SPACE-ALIGNED
/// (`news` · padding · the URL) and this split was `'\t'`-only, so every line came back whole, the
/// `http` filter rejected all 265 of them, and the sweep ran over an empty corpus — then printed a
/// Phase-0 certificate reading `sites 0 · scored 0` with five shortfall lines and not one word
/// saying the corpus was empty.
///
/// `candidates` travels with `urls` for exactly that reason. A count of URLs cannot detect its own
/// absence; a count of URLs *next to* a count of the lines they came from can. Same shape as the
/// tick-650 rule — fix a self-satisfying denominator with a SECOND POPULATION, never a threshold.
pub fn parse_corpus(text: &str) -> CorpusParse {
    let candidates: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let urls = candidates
        .iter()
        .map(|l| l.rsplit(char::is_whitespace).next().unwrap_or(l).trim())
        .filter(|u| u.starts_with("http"))
        .map(str::to_string)
        .collect();
    CorpusParse {
        urls,
        candidates: candidates.len(),
    }
}

/// The host key a sweep row is filed under, derived from the URL.
///
/// Extracted rather than left inline because [`repeat_plan`] matches accumulated ROW NAMES against
/// URLs from the corpus file, and a plan whose keys are computed a second, slightly different way is
/// a plan that silently matches nothing — the failure would read as "no site is unstable", which is
/// the answer that requires no work and is therefore the one that must not be reachable by accident.
pub fn site_name(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// **Which sites this sweep must render more than once, and how many times** — read from the
/// instrument's own accumulated rows.
///
/// [`shape_spreads`] has computed and printed this population since tick 657 and **nothing has ever
/// consumed it.** That is the gap tick 672 fell into: the block correctly reported `keirin.jp
/// 0.0478 .. 0.3972  Δ 34.9 pts`, immediately below a certificate whose keirin row was one draw from
/// that same range.
///
/// The plan is **monotone by construction** — a site that has ever produced a wide draw keeps its
/// repeats, because one calm sweep is not evidence the tail is gone. The cost of being wrong that
/// way is two extra renders; the cost of being wrong the other way is a phantom regression aimed at
/// the previous tick's work, which this project has now nearly published three times.
pub fn repeat_plan(rows_text: &str) -> Vec<(String, usize, f64)> {
    // ── **A SITE THAT ALREADY REPEATED AND DREW THE SAME NUMBER THREE TIMES IS NOT REPEATED AGAIN**
    //    (tick 687, from the first real use of this plan).
    //
    // Tick 681's sweep repeated the four sites this function named, and three of them returned rows
    // that were **byte-identical** across all three renders:
    //
    // ```text
    //   www.agoda.com    0.1000 .. 0.5077   Δ 40.8 pts over 3 runs   <- real, and large
    //   www.naukri.com   0.0000 .. 0.0000   Δ  0.0 pts over 3 runs
    //   keirin.jp        0.5717 .. 0.5717   Δ  0.0 pts over 3 runs
    //   playhop.com      0.1429 .. 0.1429   Δ  0.0 pts over 3 runs
    //   (naukri's three differ in shape_n only; its SHAPE is identical, which is what votes)
    // ```
    //
    // The document snapshot is cached, so three repeats of such a site are **three renders of the same
    // bytes** — six extra live renders per sweep, forever, for an error bar of exactly zero. The
    // variance those sites showed across SWEEPS is in the document, and no amount of repeating inside
    // one sweep can sample it.
    //
    // **This breaks tick 673's monotonicity argument on purpose, and the argument does not survive
    // contact with the measurement.** t673 said *"a site that has ever drawn wide keeps its repeats,
    // because the two errors are not symmetric (two renders vs a phantom regression)."* True while the
    // within-sweep spread was unknown. It is now measured, and where it is ZERO the median of three
    // identical draws IS the single draw — so the repeats cannot prevent a phantom regression, they can
    // only cost renders. Asymmetric errors justify paying for information, not for none.
    let deterministic = within_sweep_deterministic(rows_text);
    shape_spreads(rows_text)
        .into_iter()
        .filter(|(_, min, max, _)| (max - min) * 100.0 > SPREAD_UNSTABLE_PTS)
        .filter(|(name, ..)| !deterministic.contains(name))
        .map(|(name, min, max, _)| (name, UNSTABLE_REPEATS, (max - min) * 100.0))
        .collect()
}

/// Sites whose most recent CONSECUTIVE run in `rows_text` produced an identical SHAPE every time —
/// i.e. sites that have already been repeated and shown, by measurement, that repeating them samples
/// nothing. A site with no consecutive run is absent (unknown, not deterministic), so its repeats are
/// unaffected: this can only ever *retire* a repeat that has already been paid for once.
pub fn within_sweep_deterministic(rows_text: &str) -> std::collections::HashSet<String> {
    // Walk the file keeping the maximal consecutive run per site, LAST run wins — the same shape as
    // `collapse_consecutive_repeats`, so the two cannot disagree about what a "run" is.
    let mut last_run: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();
    let mut cur: Option<(String, Vec<f64>)> = None;
    let mut flush = |cur: &mut Option<(String, Vec<f64>)>,
                     map: &mut std::collections::HashMap<String, Vec<f64>>| {
        if let Some((name, vals)) = cur.take() {
            map.insert(name, vals);
        }
    };
    for line in rows_text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        let name = f[0].to_string();
        if cur.as_ref().is_some_and(|(n, _)| *n != name) {
            flush(&mut cur, &mut last_run);
        }
        let shape = f[2].parse::<f64>().ok();
        let entry = cur.get_or_insert_with(|| (name, Vec::new()));
        if let Some(v) = shape {
            entry.1.push(v);
        }
    }
    flush(&mut cur, &mut last_run);
    last_run
        .into_iter()
        .filter(|(_, v)| v.len() >= UNSTABLE_REPEATS && v.windows(2).all(|w| w[0] == w[1]))
        .map(|(n, _)| n)
        .collect()
}

/// Expand a sweep's URL list so every unstable site appears [`UNSTABLE_REPEATS`] times **in a row**.
///
/// Consecutive, not scattered: `rows_from_tsv` takes the median of a *consecutive* run and last-wins
/// across separated ones, so interleaving the repeats would feed them to the wrong rule and the
/// sweep would have paid for three renders to publish the last of them.
///
/// Returns the expanded list and the plan that produced it, so the caller can PRINT what it did. A
/// sweep that silently rendered one site three times would show up only as a longer wall clock.
pub fn repeat_urls(urls: &[String], rows_text: &str) -> (Vec<String>, Vec<(String, usize, f64)>) {
    let plan = repeat_plan(rows_text);
    if plan.is_empty() {
        return (urls.to_vec(), plan);
    }
    let mut out = Vec::with_capacity(urls.len() + plan.len() * UNSTABLE_REPEATS);
    for u in urls {
        let name = site_name(u);
        let n = plan
            .iter()
            .find(|(p, ..)| *p == name)
            .map(|&(_, n, _)| n)
            .unwrap_or(1);
        for _ in 0..n {
            out.push(u.clone());
        }
    }
    (out, plan)
}

/// Print the certificate block — the one place a sweep's headline is allowed to come from.
pub fn certificate_report(rows: &[Fidelity]) {
    let c = certificate(rows);
    println!("\n=== PHASE-0 EXIT CERTIFICATE (FIDELITY-SCORING-REDESIGN §3) ===\n");
    println!(
        "  sites {} · scored {} · shape ≥{:.2} on {} ({:.1}%)",
        c.sites,
        c.scored,
        CERT_SHAPE_FLOOR,
        c.shape_ok,
        c.shape_frac() * 100.0
    );
    for i in 0..4 {
        println!(
            "  {:<14} clean on {:>4} sites ({:.1}%)",
            JARRING_NAMES[i],
            c.clean[i],
            c.clean_frac(i) * 100.0
        );
    }
    if c.holds() {
        println!(
            "\n  CERTIFICATE HOLDS on this sweep. (Bar 0 and interactivity are scored elsewhere.)"
        );
    } else {
        println!("\n  CERTIFICATE NOT MET — shortfalls, in the order to work them:");
        for s in c.shortfalls() {
            println!("      · {s}");
        }
    }
}

/// Grid resolution — coarse enough to ignore glyph AA, fine enough to catch a missing element.
const GRID: u32 = 40;
/// Per-channel mean tolerance for a block to count as "agreeing".
const TOL: f64 = 26.0;

/// Mean RGB of each grid cell of an RGBA8 image.
fn block_means(rgba: &[u8], w: u32, h: u32) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity((GRID * GRID) as usize);
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (x0, x1) = (gx * w / GRID, ((gx + 1) * w / GRID).min(w));
            let (y0, y1) = (gy * h / GRID, ((gy + 1) * h / GRID).min(h));
            let (mut r, mut g, mut b, mut n) = (0f64, 0f64, 0f64, 0f64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    if i + 2 < rgba.len() {
                        r += rgba[i] as f64;
                        g += rgba[i + 1] as f64;
                        b += rgba[i + 2] as f64;
                        n += 1.0;
                    }
                }
            }
            let n = n.max(1.0);
            out.push([r / n, g / n, b / n]);
        }
    }
    out
}

/// **How much of the image is not background** — the fraction of blocks whose colour differs from
/// the image's own most common block colour.
///
/// Deliberately *self-relative*: it asks "did this engine draw anything on this page", never "is
/// this page white", so a dark-themed site is not blank and a page whose background we got wrong is
/// not blank either. The modal block is whatever the image itself is mostly made of.
fn ink(means: &[[f64; 3]]) -> f64 {
    if means.is_empty() {
        return 0.0;
    }
    // The modal block, quantised so near-identical background blocks count as one colour.
    let key = |c: &[f64; 3]| {
        [
            (c[0] / 8.0) as i64,
            (c[1] / 8.0) as i64,
            (c[2] / 8.0) as i64,
        ]
    };
    let mut counts: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    for m in means {
        *counts.entry(key(m)).or_default() += 1;
    }
    let modal = counts
        .iter()
        .max_by_key(|(k, v)| (**v, **k))
        .map(|(k, _)| *k)
        .unwrap_or([0; 3]);
    let n = means
        .iter()
        .filter(|m| {
            let k = key(m);
            (0..3).any(|i| (k[i] - modal[i]).abs() > 1)
        })
        .count();
    n as f64 / means.len() as f64
}

/// **Why this site cannot be scored — ONE rule, in one place, for both ways of being unscoreable.**
///
/// `probed` is what the ORACLE built; `common` is how many of those elements *both* engines
/// rendered, i.e. the set a placement ratio is actually computed over. The certificate refuses to
/// score below [`CERT_MIN_SHAPE_SAMPLE`] either way — this supplies the REASON that refusal never
/// had, and the two reasons blame opposite parties:
///
/// * the **oracle** built almost nothing → [`Unmeasurable::ShellOnly`]. Not our bug, and not
///   evidence about the site. ⚠⚠⚠ **THE CAUSE THIS LINE USED TO GIVE WAS ALREADY REFUTED, IN THIS
///   FILE, 1,300 LINES ABOVE (measured t856).** It read *"Its `file://` copy has a `null` origin, so
///   a JS-rendered page never builds"* — the same claim t674 killed on [`Unmeasurable::ShellOnly`]'s
///   own docs by serving the identical document over `http://127.0.0.1` and getting a byte-identical
///   dump. **The real cause is neither origin nor timing: it is that the oracle's document is ONE
///   CURL'd FILE WITH NO SUBRESOURCES.** Rendered from `file:///tmp/…`, a relative
///   `src="main-5UYZQ2ZL.js"` resolves to `file:///tmp/main-5UYZQ2ZL.js` and a root-relative
///   `src="/esaj/_next/…"` to `file:///esaj/_next/…`; both 404, so the bundle never runs. Only
///   ABSOLUTE-URL scripts still load, which is why the shortfall varies per site instead of being
///   total. t674's experiment was sound and its conclusion was over-broad: serving the same
///   single file over localhost 404s those paths too, so it could not distinguish "the origin" from
///   "the files are not there."
/// * the oracle built the page and **we** did not → [`Unmeasurable::ThinOverlap`]. Ours.
///
/// The split matters because it was the gap: `www.ebay.com` had `probed 25 · common 4`, so
/// `ShellOnly` could not fire, the floor refused to score it, and the row went out **unscored with no
/// reason** — the certificate's own *"the instrument could not say why"* line, open since t614.
///
/// Returns `None` when there is enough to compare, so a caller can leave an existing reason alone.
///
/// ⚠⚠⚠ **`ours` IS THE THIRD NUMBER, AND ITS ABSENCE MADE THIS FUNCTION ASSERT BLAME IT COULD NOT
/// SEE (t782).** With only `probed` and `common`, every thin intersection printed *"the oracle built
/// the page and we did not"* — a sentence about OUR count, decided without OUR count. It is sitting
/// in `mseen` at the one call site and was dropped on the floor. When we rendered **at least as
/// many** box-bearing elements as the oracle and the two still barely overlap, the honest reading is
/// [`Unmeasurable::TreeDivergence`]: two documents, or two states of one document, not one engine
/// rendering less. Both outcomes are UNSCORED and both count against the bar — the certificate's
/// arithmetic does not move — so this changes only what the loop is told to go and fix.
pub fn unscoreable_reason(probed: usize, common: usize, ours: usize) -> Option<Unmeasurable> {
    if probed < CERT_MIN_SHAPE_SAMPLE {
        Some(Unmeasurable::ShellOnly(probed))
    } else if common < CERT_MIN_SHAPE_SAMPLE {
        // ⚠ **THE TEST IS `ours` AGAINST THE SAME FLOOR, NOT `ours` AGAINST `probed`** — and the
        // first draft of this tick got that wrong in a way the cohort measurement caught. `ours >=
        // probed` kept `tracker.shadowfax.in` (oracle 1410 · ours 1355 · common **0**) and
        // `mayatoys.in` (1417 · 1335 · **0**) filed as *"we did not build the page"*, on pages where
        // we drew over thirteen hundred boxes. Two engines that each draw ~1,400 boxes and agree on
        // NONE of the paths are not one engine rendering less; that is total path misalignment.
        //
        // So the rule is the SYMMETRIC counterpart of `ShellOnly` above, and it reuses the same
        // constant rather than inventing a ratio: `ShellOnly` asks *"did the ORACLE build a page?"*,
        // this asks *"did WE build a page?"*, and when both did, a thin intersection is DIVERGENCE.
        // `ThinOverlap` keeps exactly the case its sentence can support — the oracle built a page
        // and we are the one below the floor.
        if ours >= CERT_MIN_SHAPE_SAMPLE {
            Some(Unmeasurable::TreeDivergence(ours))
        } else {
            Some(Unmeasurable::ThinOverlap(common))
        }
    } else {
        None
    }
}

/// Below this, an engine drew **nothing** on the page. Measured, not chosen: across the t650 HEAD-20
/// renders, `agoda` came in at **0.00%** while every site we genuinely rendered was **≥1.07%**
/// (`aparat` 1.07 · `keirin` 8.11 · `desitales2` 22.0 · `ebay` 25.6 · `ikea` 33.6 · `welt` 76.1).
/// The gap either side of this line is the whole reason it can be a constant rather than a knob.
const BLANK_INK: f64 = 0.005;

/// …and the oracle must have drawn enough for "we drew nothing" to mean anything. Below this the
/// ORACLE is the one that rendered a shell, which is [`Unmeasurable::ShellOnly`]'s job, not this
/// one: `comix` (0.00%) and `naukri` (1.92%) are Chrome-side shells and must NOT be reported as our
/// render failures.
const ORACLE_MIN_INK: f64 = 0.10;

fn load_rgba(path: &Path) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::open(path).with_context(|| format!("opening {}", path.display()))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Compare two rendered PNGs; returns the fraction of grid blocks that agree.
pub fn compare(manuk: &Path, chrome: &Path, name: &str) -> Result<Fidelity> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let ma = block_means(&a, aw, ah);
    let mb = block_means(&b, bw, bh);
    let total = ma.len().min(mb.len());
    let mut differing = 0usize;
    for i in 0..total {
        let d = (0..3)
            .map(|c| (ma[i][c] - mb[i][c]).abs())
            .fold(0.0f64, f64::max);
        if d > TOL {
            differing += 1;
        }
    }
    let score = if total == 0 {
        0.0
    } else {
        1.0 - (differing as f64 / total as f64)
    };
    Ok(Fidelity {
        name: name.to_string(),
        score,
        differing,
        total,
        structure: None,
        shape: None,
        missing: 0,
        misplaced: 0,
        probed: 0,
        jarring: [0; 4],
        shape_n: 0,
        // ── **A PAGE WE DREW NOTHING OF MUST NOT BE ABLE TO SCORE** (certification §6.3: *"a page we
        // render nothing of scores 0, not 100"*).
        //
        // t650's HEAD-20 run reported `www.agoda.com` as **`visual 69.9% · COVERAGE 100.0% ·
        // missing 0 · verdict ok`** — and the render is a **completely blank white page**. Nothing on
        // the DOM side could see it: coverage is computed against the oracle's `file://` probe, which
        // built a 13-element shell, and we "rendered" all 13 of them. 100% of nothing is 100%.
        // `ShellOnly` did not catch it either, because that guard counts the oracle's SAMPLE and 13 is
        // over the floor.
        //
        // So the check has to come from the other population. §6.4 asks for exactly this — *measure
        // each headline two independent ways, and a disagreement means one of them is lying.* The DOM
        // paths said 100%; the PIXELS say we drew nothing; the pixels are right. That disagreement is
        // available for free here, because both screenshots are already in hand.
        //
        // Classified `RenderFailed` rather than a new variant, because it is precisely what that
        // variant already documents: *"we fetched the page and FAILED TO PAINT IT — the only reason
        // on this list that is our own bug rather than a property of the origin, and the one that most
        // deserves to count against the score."*
        unmeasurable: (ink(&ma) < BLANK_INK && ink(&mb) >= ORACLE_MIN_INK)
            .then_some(Unmeasurable::RenderFailed),
    })
}

/// Write a **side-by-side** composite (Manuk left, Chromium right, a divider between) so the pair
/// can be inspected as ONE image — the eyeball check the numeric score cannot replace.
pub fn write_side_by_side(manuk: &Path, chrome: &Path, dest: &Path) -> Result<()> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let h = ah.max(bh);
    let gap = 8u32;
    let w = aw + gap + bw;
    let mut out = vec![255u8; (w * h * 4) as usize];
    let mut blit = |src: &[u8], sw: u32, sh: u32, ox: u32| {
        for y in 0..sh {
            for x in 0..sw {
                let si = ((y * sw + x) * 4) as usize;
                let di = ((y * w + x + ox) * 4) as usize;
                if si + 3 < src.len() && di + 3 < out.len() {
                    out[di..di + 4].copy_from_slice(&src[si..si + 4]);
                }
            }
        }
    };
    blit(&a, aw, ah, 0);
    blit(&b, bw, bh, aw + gap);
    // Divider.
    for y in 0..h {
        for x in aw..(aw + gap) {
            let di = ((y * w + x) * 4) as usize;
            if di + 3 < out.len() {
                out[di..di + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }
    let img = image::RgbaImage::from_raw(w, h, out).context("composite")?;
    img.save(dest)
        .with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// Structural comparison: how many of Chrome's rendered `[id]` boxes does Manuk reproduce?
/// Returns `(score, missing, misplaced, probed)`.
pub fn compare_structure(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize, usize, usize) {
    let (c, m, mi, p, _) = compare_structure_detail(chrome, manuk, tol);
    (c, m, mi, p)
}

/// Same, but also returns the **ids Manuk failed to render at all** — the diagnostic that turns a
/// coverage number into actionable work. 1,402 missing elements are almost never 1,402 bugs; they
/// are a handful of CLASS bugs with huge blast radius, and the ids tell you which.
pub fn compare_structure_detail(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize, usize, usize, Vec<String>) {
    let probed = chrome.len();
    let (mut missing, mut misplaced) = (0usize, 0usize);
    let mut missing_ids: Vec<String> = Vec::new();
    for (id, c) in chrome {
        match manuk.get(id) {
            None => {
                missing += 1;
                missing_ids.push(id.clone());
            }
            Some(m) => {
                let off = (0..4).map(|i| (c[i] - m[i]).abs()).fold(0, i64::max);
                if off > tol {
                    misplaced += 1;
                }
            }
        }
    }
    // **COVERAGE** is the honest, unambiguous signal: of the elements Chrome actually renders, what
    // fraction does Manuk render *at all*? A missing sidebar, an unpainted infobox, a dropped
    // section — all show up here and cannot be averaged away by white-matching-white. Placement
    // drift (`misplaced`) is reported separately because on real pages it is dominated by font-
    // metric differences, which are a *fidelity* concern, not a *correctness* one.
    let rendered = probed.saturating_sub(missing);
    // **A page we cannot PROBE must not score 100%.**
    //
    // `probed` counts the `[id]` elements Chrome rendered. `example.com` — which was in this gate's
    // DEFAULT url list — has **no `id` attributes at all**, so it probed nothing, returned a perfect
    // 1.0, and inflated the mean of a gate whose whole job is to catch missing content.
    //
    // Found by mutation-testing: emptying `node_rects()` entirely — so the browser renders NOTHING —
    // still scored 100% coverage on that URL. A gate that cannot fail on a blank render is not a gate.
    //
    // `f64::NAN` is the honest answer to "what fraction did we render, of nothing?", and `report`
    // excludes it from the mean rather than counting it as success.
    let coverage = if probed == 0 {
        f64::NAN
    } else {
        rendered as f64 / probed as f64
    };
    missing_ids.sort();
    (coverage, missing, misplaced, probed, missing_ids)
}

/// The **placement** half of the honest number, now that COVERAGE is near-saturated: for every
/// element BOTH engines render, how far off is Manuk? Returns `(median_dx, median_dy, median_dw,
/// median_dh, within_tol_fraction)`.
///
/// A count of "misplaced" says nothing about *why*: 6,000 elements each off by 4px is a font-metric
/// difference, while 6,000 elements each off by 200px is one displaced container dragging its whole
/// subtree. The medians separate those two worlds, which is the whole point of measuring.
pub fn placement_stats(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (i64, i64, i64, i64, f64) {
    let mut d: [Vec<i64>; 4] = Default::default();
    let (mut within, mut n) = (0usize, 0usize);
    for (id, c) in chrome {
        let Some(m) = manuk.get(id) else { continue };
        n += 1;
        let mut worst = 0i64;
        for i in 0..4 {
            let off = (c[i] - m[i]).abs();
            d[i].push(off);
            worst = worst.max(off);
        }
        if worst <= tol {
            within += 1;
        }
    }
    let med = |v: &mut Vec<i64>| -> i64 {
        if v.is_empty() {
            return 0;
        }
        v.sort_unstable();
        v[v.len() / 2]
    };
    let frac = if n == 0 {
        1.0
    } else {
        within as f64 / n as f64
    };
    (
        med(&mut d[0]),
        med(&mut d[1]),
        med(&mut d[2]),
        med(&mut d[3]),
        frac,
    )
}

/// **Layer 1 — SHAPE (parent-relative), the redesign's new primary gate**
/// (`docs/loop/FIDELITY-SCORING-REDESIGN.md` §2). Score every element against **its parent's box**,
/// not the document origin: `rel = (x - px, y - py, w, h)`. This is the metric that separates a
/// genuinely-wrong box from a whole page shifted by a constant.
///
/// `placement_stats` above charges one root cause N times — a page shifted 23px at its header scores
/// `PLACE(ok) 0%` because every downstream element inherits the same 23px, though the layout is
/// otherwise correct and a user notices nothing. Under SHAPE that constant offset **cancels**: only
/// the one element where the offset *originates* fails, so one root cause counts once.
///
/// **Keys are selector-paths** (`tag.SIG:nth-of-type(n)/…` from the root, the SAME convention the
/// differential oracle uses in `oracle::diff_page`), so an ancestor's key is a prefix of its
/// descendants'. Each element is scored against the **nearest ancestor present in BOTH maps** — the
/// shared reference frame (`oracle::common_frame`): both engines measure the child against the *same*
/// ancestor, so a constant offset in that ancestor drops out of the difference. Width/height are
/// translation-invariant and stay absolute. A root-level element (no `/`, or no shared ancestor) has
/// nothing to subtract, so its absolute box IS its shape — the offset is charged there, exactly once.
///
/// This is the fidelity-probe half of the SHAPE metric the oracle proved at tick 335; the redesign
/// names this probe (the agent-editable `manuk-wpt` fidelity code) as the Phase-0 EXIT instrument,
/// and SHAPE replacing `placement_stats` as its Layer-1 gate.
///
/// Returns `(within_tol_fraction, scored_count)`. Only elements BOTH engines rendered are scored.
pub fn shape_stats(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize) {
    // The nearest ancestor of `path` present in BOTH maps — the shared reference frame. Walks up by
    // dropping the last `/component`; `None` at the root (no `/`) or when no ancestor is shared.
    // Mirrors `oracle::common_frame` exactly so the instrument has ONE definition of SHAPE.
    fn common_frame<'a>(
        path: &str,
        chrome: &'a std::collections::HashMap<String, [i64; 4]>,
        manuk: &'a std::collections::HashMap<String, [i64; 4]>,
    ) -> Option<([i64; 4], [i64; 4])> {
        let mut p = path;
        while let Some(cut) = p.rfind('/') {
            p = &p[..cut];
            if let (Some(c), Some(m)) = (chrome.get(p), manuk.get(p)) {
                return Some((*c, *m));
            }
        }
        None
    }
    let (mut within, mut n) = (0usize, 0usize);
    for (path, c) in chrome {
        let Some(m) = manuk.get(path) else { continue };
        n += 1;
        // Subtract each element's box from its shared frame's box (x,y only — w,h are invariant).
        let (cr, mr) = match common_frame(path, chrome, manuk) {
            Some((cf, mf)) => (
                [c[0] - cf[0], c[1] - cf[1], c[2], c[3]],
                [m[0] - mf[0], m[1] - mf[1], m[2], m[3]],
            ),
            None => (*c, *m),
        };
        let worst = (0..4).map(|i| (cr[i] - mr[i]).abs()).max().unwrap_or(0);
        if worst <= tol {
            within += 1;
        }
    }
    let frac = if n == 0 {
        1.0
    } else {
        within as f64 / n as f64
    };
    (frac, n)
}

/// **IS A ZERO INTERSECTION A RENDERING RESULT OR A KEYING RESULT?** — the one measurement that
/// decides where the next several ticks go (t783).
///
/// t782 measured the `thin-overlap` cohort with both sides printed and found the same shape on every
/// member: two engines each drawing hundreds-to-thousands of boxes and sharing **between zero and
/// nine** selector-paths. Two explanations survive that, and they have completely different fixes:
///
/// 1. **INDEX SHIFT.** The trees are substantially the same and the KEY is brittle. `:nth-child(N)`
///    is an absolute sibling index, so a single element present in one document and not the other —
///    one ad `<div>`, one hydration wrapper — re-numbers every sibling beneath it and every key
///    below changes at once. Fix: a key that survives an insertion.
/// 2. **DIFFERENT DOCUMENTS.** The oracle renders a `curl` snapshot from `file://` and we render the
///    LIVE url, so the two runs really did build different pages. Fix: give both engines the same
///    bytes.
///
/// Guessing between them costs a subsystem either way, so this measures instead. It re-keys both
/// sides on the **tag path alone** — every `:nth-of-type(N)` stripped, so `body:nth-of-type(1)/div:
/// nth-of-type(4)` becomes `body/div` — and reports the MULTISET overlap, which is exactly the
/// intersection an index-insensitive key could reach. Multiset, not set, because the weak key is
/// deliberately non-unique: `min(chrome_count, our_count)` summed over keys is the honest upper bound.
///
/// ⚠ **THE ANSWER CAME BACK "INDEX SHIFT" AND THE KEY WAS CHANGED (t784), SO THIS FUNCTION NOW READS
/// AGAINST A DIFFERENT `exact`.** The shipped key counts per-TAG ordinals, which absorbs an inserted
/// sibling of a *different* tag. What it still cannot absorb is an inserted sibling of the *same*
/// tag — so a surviving `exact` ≈ 0 / `tag_overlap` ≫ 0 row now means one of the narrower two:
/// same-tag insertion, or genuinely different documents. The measurement keeps its value precisely
/// because the ceiling it reports (`tag_overlap`) did not move when the key did.
///
/// Reading it:
///
/// * `exact` ≈ 0 and `tag_overlap` ≈ `probed` → **index shift**, and a better key recovers the page.
/// * `exact` ≈ 0 and `tag_overlap` ≈ 0 → **different documents**, and no key will help.
///
/// `first_bad_depth` localises it further: the shallowest depth at which the two sides disagree on
/// how many elements exist. A depth of 1 or 2 with a high `tag_overlap` is the signature of exactly
/// one inserted node near the root.
pub fn tree_alignment(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    ours: &std::collections::HashMap<String, [i64; 4]>,
) -> TreeAlignment {
    fn weak(path: &str) -> String {
        path.split('/')
            .map(|c| match c.find(":nth-of-type(") {
                Some(i) => &c[..i],
                None => c,
            })
            .collect::<Vec<_>>()
            .join("/")
    }
    fn depth_counts(m: &std::collections::HashMap<String, [i64; 4]>) -> Vec<usize> {
        let mut v = Vec::new();
        for k in m.keys() {
            let d = k.matches('/').count();
            if v.len() <= d {
                v.resize(d + 1, 0);
            }
            v[d] += 1;
        }
        v
    }
    let exact = chrome.keys().filter(|k| ours.contains_key(*k)).count();
    let mut cbag: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for k in chrome.keys() {
        *cbag.entry(weak(k)).or_default() += 1;
    }
    let mut obag: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for k in ours.keys() {
        *obag.entry(weak(k)).or_default() += 1;
    }
    let tag_overlap: usize = cbag
        .iter()
        .map(|(k, c)| *c.min(obag.get(k).unwrap_or(&0)))
        .sum();
    let (cd, od) = (depth_counts(chrome), depth_counts(ours));
    let first_bad_depth =
        (0..cd.len().max(od.len())).find(|d| cd.get(*d).unwrap_or(&0) != od.get(*d).unwrap_or(&0));
    TreeAlignment {
        probed: chrome.len(),
        ours: ours.len(),
        exact,
        tag_overlap,
        first_bad_depth,
    }
}

/// What [`tree_alignment`] answers. See its doc comment for how to read the two numbers together —
/// neither is meaningful alone, which is why they are returned as one value rather than two calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeAlignment {
    /// How many box-bearing elements the ORACLE built.
    pub probed: usize,
    /// How many WE built. The number `thin-overlap` decided blame without (t782).
    pub ours: usize,
    /// Paths present in both under the real, index-bearing key — what the score is computed over.
    pub exact: usize,
    /// The multiset overlap under a key with every `:nth-of-type(N)` stripped: the ceiling an
    /// index-insensitive key could reach. **`tag_overlap` ≫ `exact` is the index-shift signature.**
    pub tag_overlap: usize,
    /// Shallowest depth whose element COUNT differs between the two sides, or `None` when every
    /// depth agrees (which, with `exact` ≈ 0, would itself be the index-shift signature).
    pub first_bad_depth: Option<usize>,
}

/// One element that SHAPE scored as wrong, with the four parent-relative deltas that made it wrong.
///
/// The fields are already in the units the reduction needs: `d` is the per-axis
/// `(chrome - manuk)` after each side's box has had its shared frame subtracted, exactly as
/// [`shape_stats`] computes it. Nothing is rounded or bucketed on the way out — a bucket is a
/// decision about which mechanism this is, and that decision belongs to the reader.
#[derive(Debug, Clone)]
pub struct ShapeMiss {
    /// Selector path, the same key both sides are scored under.
    pub path: String,
    /// `chrome - manuk` for `[x, y, width, height]`, parent-relative in x/y.
    pub d: [i64; 4],
    /// Chrome's frame-relative box, for reading the delta against a size.
    pub chrome: [i64; 4],
    /// Ours.
    pub manuk: [i64; 4],
}

impl ShapeMiss {
    /// Which axis carries this miss, and how far — `"width +6"`, `"y -14"`. The single largest
    /// component, because a box that is wrong in one axis and right in three is a different
    /// mechanism from one that is wrong in all four, and the label must not hide that.
    pub fn axis(&self) -> String {
        let names = ["x", "y", "width", "height"];
        let (i, v) = (0..4)
            .map(|i| (i, self.d[i]))
            .max_by_key(|(_, v)| v.abs())
            .unwrap_or((0, 0));
        format!("{} {}{}", names[i], if v >= 0 { "+" } else { "" }, v)
    }
}

/// **THE AIM THAT `shape_stats` ALREADY COMPUTES AND THROWS AWAY.**
///
/// `shape_stats` walks every shared path, subtracts the shared frame, takes the worst of four
/// deltas, and reduces the whole thing to one ratio. Every reduction tick since t813 has then spent
/// its first half rebuilding that walk by hand — `boxes --fetch` on our side, a headless Chrome dump
/// on the other, and an eyeball diff — to answer the question the scorer had already answered
/// per-element and discarded. This returns it: the misses, worst-first, in the SAME frame the score
/// is computed in, so the number a probe reads and the number the sweep publishes cannot disagree.
///
/// It is deliberately NOT clustered. The board's mechanism-oracle note wants work ranked by
/// primitive rather than by tag, and that is right — but a signature computed here would be this
/// function guessing which of `x/y/width/height` is the *cause*, when on a page laid out top-down
/// the cause is upstream of almost every symptom it would name. [`ShapeMiss::axis`] labels the
/// symptom honestly and stops there; the causal call stays with the reader, who can see the tree.
pub fn shape_misses(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> Vec<ShapeMiss> {
    let mut out = Vec::new();
    for (path, c) in chrome {
        let Some(m) = manuk.get(path) else { continue };
        // The frame subtraction is `shape_stats`'s, re-derived here rather than shared, because
        // sharing it would mean exposing `common_frame` and the two callers want different things
        // from a missing frame: the scorer wants a number, this wants the raw boxes. The invariant
        // that matters — same frame, same tolerance — is asserted by a test, not by a call.
        let (cr, mr) = {
            let mut p: &str = path;
            let mut frame = None;
            while let Some(cut) = p.rfind('/') {
                p = &p[..cut];
                if let (Some(cf), Some(mf)) = (chrome.get(p), manuk.get(p)) {
                    frame = Some((*cf, *mf));
                    break;
                }
            }
            match frame {
                Some((cf, mf)) => (
                    [c[0] - cf[0], c[1] - cf[1], c[2], c[3]],
                    [m[0] - mf[0], m[1] - mf[1], m[2], m[3]],
                ),
                None => (*c, *m),
            }
        };
        let d = [cr[0] - mr[0], cr[1] - mr[1], cr[2] - mr[2], cr[3] - mr[3]];
        let worst = (0..4).map(|i| d[i].abs()).max().unwrap_or(0);
        if worst > tol {
            out.push(ShapeMiss {
                path: path.clone(),
                d,
                chrome: cr,
                manuk: mr,
            });
        }
    }
    // Worst first, then by path so a re-run of the same tree prints the same order — a probe whose
    // output reorders between runs cannot be diffed across a fix, which is the only way it gets used.
    out.sort_by(|a, b| {
        let (x, y) = (
            (0..4).map(|i| a.d[i].abs()).max().unwrap_or(0),
            (0..4).map(|i| b.d[i].abs()).max().unwrap_or(0),
        );
        y.cmp(&x).then_with(|| a.path.cmp(&b.path))
    });
    out
}

/// **Where does the layout first diverge?** Sort every element both engines render by Chrome's `y`
/// and walk down the page; report the first id whose vertical offset exceeds `jump`, plus the last
/// id that was still in agreement. Downstream drift is almost always ONE upstream box with the
/// wrong height — a median tells you drift exists, this tells you where it started.
pub fn first_divergence(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    jump: i64,
) -> Option<(String, i64, String, i64)> {
    let mut pairs: Vec<(&String, &[i64; 4], &[i64; 4])> = chrome
        .iter()
        .filter_map(|(id, c)| manuk.get(id).map(|m| (id, c, m)))
        .collect();
    pairs.sort_by_key(|(_, c, _)| c[1]);
    let mut last_ok = String::from("(document start)");
    for (id, c, m) in pairs {
        let dy = (c[1] - m[1]).abs();
        if dy > jump {
            return Some((last_ok, 0, id.clone(), c[1] - m[1]));
        }
        last_ok = id.clone();
    }
    None
}

/// Print the report + the gate verdict against `floor` (applied to the STRUCTURAL score when it is
/// available — it is the honest one).
pub fn report(rows: &[Fidelity], floor: f64) -> bool {
    println!("\n=== G1 · REAL-SITE PARITY vs Chromium ===\n");
    println!(
        "{:<24} {:>8} {:>10} {:>8} {:>9} {:>7}",
        "page", "visual", "COVERAGE", "missing", "misplaced", "verdict"
    );
    let mut all_ok = true;
    for r in rows {
        // Gate on structure when we have it (a missing sidebar must FAIL, not be averaged away).
        let gated = r.structure.unwrap_or(r.score);
        // **A page we could not PROBE is a broken gate CONFIG, not a pass.** Mutation-testing found
        // the original: emptying `node_rects()` so the browser renders NOTHING still scored 100% on
        // a URL that probed zero elements, inflating the mean of the gate whose entire job is to
        // catch missing content. `NaN` is the honest answer, and it is excluded from the mean.
        //
        // **And a page we could not measure is now named by its REAL cause.** This used to print "Chrome
        // rendered NO [id] elements … Choose a URL with ids" for every unprobeable page — text left
        // over from before t532 moved the keying to selector paths, and by t606 it was pointing the
        // operator at the corpus for what was almost always the *network's* answer (`imdb.com`
        // replies 202 with a zero-byte body). A diagnostic that names the wrong organ is worse than
        // none: it sends the next tick somewhere there is nothing to fix.
        if gated.is_nan() || r.unmeasurable.is_some() {
            match &r.unmeasurable {
                Some(u) => eprintln!("  ⚠ {} UNMEASURABLE [{}]: {}", r.name, u.tag(), u.explain()),
                None => eprintln!(
                    "  ⚠ {}: the document loaded and the probe ran, but Chrome rendered NO elements \
                     at all — so there is nothing to compare and this measures nothing. Counting it \
                     as a pass is how a gate that cannot fail looks green forever.",
                    r.name
                ),
            }
            all_ok = false;
        }
        // **AN UNMEASURABLE ROW MUST NEVER READ `ok`, AND ONE DID.** `gated` falls back to the
        // PIXEL score when there is no structural score, so `aparat.com` — a live 200 page whose own
        // CSP blocked the box probe — printed `99.9% … ok` off its screenshot alone, on a row the
        // same function had just flagged UNMEASURABLE two lines above. The aggregate verdict was
        // already correct (`all_ok` was false), which is exactly what makes this the dangerous shape:
        // the number a human reads said the opposite of the number the gate used.
        let ok = r.unmeasurable.is_none() && gated >= floor;
        if !ok {
            all_ok = false;
        }
        println!(
            "{:<24} {:>7.1}% {:>8} {:>8} {:>9} {:>7}",
            r.name,
            r.score * 100.0,
            r.structure
                .map(|s| format!("{:.1}%", s * 100.0))
                .unwrap_or_else(|| "—".into()),
            r.missing,
            r.misplaced,
            match (&r.unmeasurable, ok) {
                (Some(_), _) => "UNMEAS",
                (None, true) => "ok",
                (None, false) => "BELOW",
            }
        );
    }
    // **EVERY HEADLINE MEAN IS OVER THE SAME SITE SET: the ones the instrument accepted.**
    //
    // Two rules in one filter, and the second is the subtle one. `NaN` must go, or a single refused
    // origin turns the headline into `NaN%`. And a row carrying a REASON must go even when it has a
    // real number attached — `aparat.com` is a live 200 page whose own CSP blocked the box probe, so
    // its pixel score is a genuine measurement while its structural score does not exist. Averaging
    // it into MEAN VISUAL but not into MEAN COVERAGE would compute two headlines over two different
    // populations and print them three lines apart, which is precisely the accounting mismatch that
    // has caught more defects here than any gate (`THE SEVEN META-INSTRUMENTS` #3).
    //
    // The site is not thereby forgiven: it stays a counted row and `certificate` holds it against
    // the bar. A mean and a denominator answer different questions — conflating them is how "we
    // dropped the hard sites" happened the first time.
    let scored_v: Vec<f64> = rows
        .iter()
        .filter(|r| r.unmeasurable.is_none())
        .map(|r| r.score)
        .filter(|s| !s.is_nan())
        .collect();
    let mean_v = if scored_v.is_empty() {
        f64::NAN
    } else {
        scored_v.iter().sum::<f64>() / scored_v.len() as f64
    };
    // Same site set as MEAN VISUAL above, and for the same reason: `comix.to` carried a `coverage`
    // of 66.7% computed over THREE elements of a shell the oracle could not hydrate. Averaging that
    // into the headline while excluding it from the certificate computes two numbers over two
    // populations and prints them three lines apart.
    let structs: Vec<f64> = rows
        .iter()
        .filter(|r| r.unmeasurable.is_none())
        .filter_map(|r| r.structure)
        .collect();
    let mean_s = if structs.is_empty() {
        None
    } else {
        Some(structs.iter().sum::<f64>() / structs.len() as f64)
    };
    let shapes: Vec<f64> = rows
        .iter()
        .filter(|r| r.unmeasurable.is_none())
        .filter_map(|r| r.shape)
        .collect();
    let mean_shape = if shapes.is_empty() {
        None
    } else {
        Some(shapes.iter().sum::<f64>() / shapes.len() as f64)
    };
    println!("\nMEAN VISUAL:    {:.1}%", mean_v * 100.0);
    if let Some(ms) = mean_s {
        println!(
            "MEAN COVERAGE:  {:.1}%   <-- THE HONEST NUMBER: of the elements Chrome renders, the\n\
             \t\t\tfraction Manuk renders AT ALL (floor {:.0}%). A missing region\n\
             \t\t\tcannot hide in this the way it hides in a pixel score.",
            ms * 100.0,
            floor * 100.0
        );
    }
    if let Some(msh) = mean_shape {
        println!(
            "MEAN SHAPE:     {:.1}%   <-- LAYER-1 (parent-relative): of elements BOTH render, the\n\
             \t\t\tfraction placed right vs their nearest SHARED ancestor. Unlike\n\
             \t\t\tthe old absolute placement, a constant page offset cancels here —\n\
             \t\t\tone root cause counts once (FIDELITY-SCORING-REDESIGN.md Layer 1).",
            msh * 100.0
        );
    }
    println!(
        "\nSide-by-side composites written — LOOK at them. The visual score is a poor proxy: an\n\
         entirely absent sidebar moved it <1 point. THE SCORE GATES; THE EYEBALL DIAGNOSES.\n"
    );
    all_ok
}

#[cfg(test)]
mod shape_tests {
    use super::{certificate, shape_misses, shape_stats, Fidelity};
    use std::collections::HashMap;

    // A realistic selector-path box tree modelling the microsoft.com artifact from the redesign:
    // the page top matches Chrome, a taller-than-Chrome HEADER then pushes the whole content column
    // down by `header_extra` px. That offset originates at ONE element (the content container) and
    // every descendant inherits it — which is exactly the "one cause counted N times" trap.
    //
    // `bad_child` corrupts one leaf's HEIGHT instead — a genuine layout bug that no offset explains.
    const KIDS: usize = 8;
    fn tree(header_extra: i64, bad_child: bool) -> HashMap<String, [i64; 4]> {
        let mut m = HashMap::new();
        // Root + header positions match Chrome exactly (the page top is not shifted).
        m.insert("html/body".to_string(), [0, 0, 1000, 3000]);
        m.insert(
            "html/body/header:nth-of-type(1)".to_string(),
            [0, 0, 1000, 80 + header_extra], // Manuk's header is `header_extra` px too tall
        );
        // Content container: pushed down by the taller header → its box vs body is off by header_extra.
        let content_y = 80 + header_extra;
        m.insert(
            "html/body/main:nth-of-type(2)".to_string(),
            [0, content_y, 1000, 2000],
        );
        // Content's children: absolutely shifted by header_extra too, but their position RELATIVE to
        // the content container is unchanged — so SHAPE cancels the offset for every one of them.
        for k in 0..KIDS {
            let ky = content_y + 100 + (k as i64) * 200;
            let h = if bad_child && k == 0 { 999 } else { 150 };
            m.insert(
                format!("html/body/main:nth-of-type(2)/div:nth-of-type({})", k + 1),
                [20, ky, 960, h],
            );
        }
        m
    }
    const TOTAL: usize = 3 + KIDS; // body + header + main + KIDS

    #[test]
    fn constant_offset_charged_once_under_shape() {
        let chrome = tree(0, false);
        let manuk = tree(23, false); // header 23px too tall → content column shifted 23px
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(n, TOTAL, "every element both engines rendered is scored");
        // SHAPE charges the 23px exactly ONCE — at the content container where it originates. The
        // header (its own height is wrong) also fails; its KIDS all cancel. So exactly 2 of 11 fail.
        let failed = TOTAL - (shape * TOTAL as f64).round() as usize;
        assert_eq!(
            failed, 2,
            "SHAPE must charge a constant offset at its ORIGIN only (header + content container), \
             not once per inheriting descendant — got {failed} failures, shape {shape}"
        );

        // Contrast — absolute placement charges the SAME offset to the container AND all KIDS: the
        // content container + 8 kids = 9 of 11 shifted, so placement collapses. This divergence is
        // the whole point of the redesign; if shape_stats ignored parents the two would be equal.
        let (_, mdy, _, _, place_frac) = super::placement_stats(&chrome, &manuk, 8);
        assert_eq!(mdy, 23, "median absolute dy is the raw 23px offset");
        assert!(
            place_frac <= 2.0 / TOTAL as f64 + 1e-9,
            "absolute placement must be dragged down by the offset it cannot cancel, got {place_frac}"
        );
    }

    // ── THE DUMP AND THE SCORE MUST BE THE SAME WALK.
    //
    // `shape_misses` re-derives the frame subtraction rather than sharing `shape_stats`'s (the two
    // callers want different things from a missing frame). That is a duplicated invariant, and a
    // duplicated invariant that nothing checks is how a probe starts naming elements the score
    // considers fine — which would send a reduction tick at a box that is not costing us anything.
    // So: the dump's LENGTH must equal the score's failure count, on both fixtures, exactly.
    #[test]
    fn the_miss_dump_names_exactly_the_elements_the_score_failed() {
        for (label, chrome, manuk) in [
            ("constant offset", tree(0, false), tree(23, false)),
            ("one bad leaf", tree(0, false), tree(0, true)),
        ] {
            let (shape, n) = shape_stats(&chrome, &manuk, 8);
            let failed = n - (shape * n as f64).round() as usize;
            let misses = shape_misses(&chrome, &manuk, 8);
            assert_eq!(
                misses.len(),
                failed,
                "{label}: the dump must name exactly the elements SHAPE scored as wrong — \
                 shape {shape} over {n} means {failed} failures, dump named {}",
                misses.len()
            );
            // Worst-first, or the `--shape-dump N` truncation silently hides the biggest error.
            let worst: Vec<i64> = misses
                .iter()
                .map(|m| (0..4).map(|i| m.d[i].abs()).max().unwrap_or(0))
                .collect();
            assert!(
                worst.windows(2).all(|w| w[0] >= w[1]),
                "{label}: misses must be worst-first, got {worst:?}"
            );
            assert!(
                worst.iter().all(|&w| w > 8),
                "{label}: nothing within tolerance may appear in the dump, got {worst:?}"
            );
        }
    }

    // The axis label must name the axis that actually carries the error. `tree(_, true)` corrupts a
    // leaf's HEIGHT by 849px and nothing else, so a dump that calls that a `y` miss is mislabelling
    // the mechanism — the exact failure mode t817 recorded (a wrong LABEL is caught by nothing).
    #[test]
    fn the_axis_label_names_the_axis_that_is_wrong() {
        let misses = shape_misses(&tree(0, false), &tree(0, true), 8);
        assert_eq!(misses.len(), 1);
        assert_eq!(
            misses[0].axis(),
            "height -849",
            "a corrupted HEIGHT must be labelled height, got {}",
            misses[0].axis()
        );
    }

    #[test]
    fn a_genuinely_wrong_box_still_fails_shape() {
        let chrome = tree(0, false);
        let manuk = tree(0, true); // one leaf's height wrong by 849px — a REAL layout bug
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(n, TOTAL);
        let failed = TOTAL - (shape * TOTAL as f64).round() as usize;
        assert_eq!(
            failed, 1,
            "SHAPE must NOT be blind to a real box error — the one bad leaf must fail, got shape {shape}"
        );
    }

    #[test]
    fn only_common_elements_scored() {
        let chrome = tree(0, false);
        let mut manuk = tree(0, false);
        manuk.remove("html/body/main:nth-of-type(2)/div:nth-of-type(1)"); // Manuk dropped one leaf
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(
            n,
            TOTAL - 1,
            "a box only Chrome rendered is a COVERAGE miss, not a SHAPE miss"
        );
        assert!((shape - 1.0).abs() < f64::EPSILON);
    }

    fn row(name: &str, shape: Option<f64>, jarring: [usize; 4]) -> Fidelity {
        Fidelity {
            name: name.into(),
            score: 1.0,
            differing: 0,
            total: 1,
            structure: Some(1.0),
            shape,
            missing: 0,
            misplaced: 0,
            probed: 10,
            jarring,
            shape_n: 64,
            unmeasurable: None,
        }
    }

    /// The certificate is a CONJUNCTION, and every way of accidentally turning it into an average is
    /// a way of passing it without meeting it. These pin all four.
    #[test]
    fn the_certificate_is_a_conjunction_not_an_average() {
        // 20 sites, all shaped and all clean → holds.
        let all_good: Vec<Fidelity> = (0..20)
            .map(|i| row(&format!("s{i}"), Some(0.9), [0; 4]))
            .collect();
        let c = certificate(&all_good);
        assert_eq!(c.sites, 20);
        assert_eq!(c.scored, 20);
        assert_eq!(c.shape_ok, 20);
        assert!(c.holds(), "20/20 shaped and clean must hold");
        assert!(c.shortfalls().is_empty());

        // ONE invariant below the bar fails the whole thing — 2 of 20 sites with an overlap is 90%
        // clean, and the bar is 95%.
        let mut one_bad = all_good.clone();
        one_bad[0].jarring[1] = 3;
        one_bad[1].jarring[1] = 1;
        let c = certificate(&one_bad);
        assert!(
            !c.holds(),
            "90% clean on ONE invariant must fail the certificate — averaging the four terms \
             together is how a certificate becomes a vibe"
        );
        let sf = c.shortfalls();
        assert_eq!(
            sf.len(),
            1,
            "and it must name exactly the term that missed: {sf:?}"
        );
        assert!(sf[0].starts_with("overlap "), "got {}", sf[0]);

        // A site that could not be SCORED counts AGAINST the bar, never out of it — otherwise the
        // certificate is met by failing to measure, which is the same defect the NaN check in
        // `report` exists for.
        let mut unscored = all_good.clone();
        unscored[0].shape = None;
        unscored[1].shape = Some(f64::NAN);
        let c = certificate(&unscored);
        assert_eq!(c.scored, 18);
        assert_eq!(c.shape_ok, 18);
        assert!(
            !c.holds(),
            "18 of 20 scored is 90% — below the bar, not 100% of what we measured"
        );
        assert!(
            c.shortfalls().iter().any(|s| s.contains("UNSCORED")),
            "the unscored sites must be NAMED: {:?}",
            c.shortfalls()
        );

        // The shape FLOOR is per-site and strict-at-the-boundary-from-below: 0.75 passes, 0.74 does not.
        assert_eq!(certificate(&[row("a", Some(0.75), [0; 4])]).shape_ok, 1);
        assert_eq!(certificate(&[row("a", Some(0.74), [0; 4])]).shape_ok, 0);

        // An EMPTY sweep never holds. A certificate over zero sites is the most flattering possible
        // reading of an engine and the least informative.
        assert!(!certificate(&[]).holds(), "zero sites is not a pass");
    }

    /// **G_UNSCOREABLE_REASON — an unscored site must say WHICH ENGINE failed it.**
    ///
    /// The certificate prints its own shortfall for this: *"N of those UNSCORED sites have NO
    /// recorded reason — the instrument could not say why, which is an instrument gap, not a
    /// result."* t611 had 4, t626 had 1, t650 drove it to 0, and **t653 put one back** — because the
    /// rule only ever looked at what the ORACLE built.
    ///
    /// Every case below is a real row from a real sweep, and the middle one is the whole point.
    #[test]
    fn an_unscored_site_names_which_engine_failed_it() {
        use super::{unscoreable_reason, Unmeasurable, CERT_MIN_SHAPE_SAMPLE as FLOOR};

        // `comix.to` — the ORACLE built 3 elements from its `file://` copy. Not our bug, and not
        // evidence about the site.
        assert_eq!(
            unscoreable_reason(3, 2, 900),
            Some(Unmeasurable::ShellOnly(3))
        );

        // `www.ebay.com` @t653 — **the gap**. The oracle built 25 (no shell), and only 4 were common
        // because WE rendered 16% of the page. Deciding on `probed` alone, this returns None and the
        // row goes out unscored with nothing to say.
        assert_eq!(
            unscoreable_reason(25, 4, 3),
            Some(Unmeasurable::ThinOverlap(4)),
            "we drew THREE boxes against the oracle's 25 — below the same floor `ShellOnly` uses, \
             so 'the oracle built the page and we did not' is a claim the numbers support"
        );

        // ⚠ **THE THIRD NUMBER (t782).** Same `probed` and same `common` as the row above — and the
        // opposite verdict, because WE built a page too. `thin-overlap`'s sentence ("the oracle
        // built the page and we did not") is a claim about our count, and it was decided without
        // our count for as long as the variant existed.
        assert_eq!(
            unscoreable_reason(25, 4, 25),
            Some(Unmeasurable::TreeDivergence(25)),
            "both engines above the floor with a thin intersection is DIVERGENCE, not us rendering \
             less"
        );
        assert_eq!(
            unscoreable_reason(57, 9, 434),
            Some(Unmeasurable::TreeDivergence(434)),
            "www.naukri.com, measured: the oracle's copy has 57 box-bearing elements and ours has \
             434 — 7.6x — with 9 paths shared. Calling that a coverage failure of ours is backwards"
        );
        // ⚠ The case the FIRST draft of this rule got wrong, kept as a regression: `ours >= probed`
        // would file `tracker.shadowfax.in` (oracle 1410 · ours 1355 · common 0) as thin-overlap,
        // on a page where we drew 1,355 boxes. Two engines that each draw ~1,400 and share NONE are
        // misaligned, not one engine rendering nothing.
        assert_eq!(
            unscoreable_reason(1410, 0, 1355),
            Some(Unmeasurable::TreeDivergence(1355)),
            "1,355 of our own boxes is not 'we did not build the page', whatever the intersection is"
        );

        // `www.ikea.com` — 698 probed, 698 common. Scoreable, so no reason at all: a rule that
        // manufactures a reason for a healthy site is worse than one that stays quiet.
        assert_eq!(unscoreable_reason(698, 698, 698), None);

        // The boundary is the certificate's OWN floor, reused rather than invented — so this can
        // never disagree with the thing that refuses to score.
        assert_eq!(unscoreable_reason(FLOOR, FLOOR, FLOOR), None);
        assert_eq!(
            unscoreable_reason(FLOOR, FLOOR - 1, 0),
            Some(Unmeasurable::ThinOverlap(FLOOR - 1)),
            "one element below the floor must be REFUSED and NAMED, not scored"
        );

        // And the reason survives the process boundary a chunked sweep writes it across.
        let tag = Unmeasurable::ThinOverlap(4).tag();
        assert_eq!(tag, "thin-overlap-4");
        assert_eq!(
            Unmeasurable::from_tag(&tag),
            Some(Unmeasurable::ThinOverlap(4)),
            "a reason that cannot be read back is a reason the certificate loses at exactly the \
             moment the headline is computed"
        );

        // The certificate must COUNT it as unscored-with-a-reason — the whole point is that the
        // shortfall list stops saying "the instrument could not say why".
        let row = Fidelity {
            shape: Some(0.0),
            shape_n: 4,
            unmeasurable: Some(Unmeasurable::ThinOverlap(4)),
            ..row("ebay.example", Some(0.0), [0; 4])
        };
        let c = certificate(&[row]);
        assert_eq!(c.sites, 1);
        assert_eq!(c.scored, 0);
        assert_eq!(
            c.unmeasured_by_reason,
            vec![("thin-overlap-4".to_string(), 1)]
        );
    }

    /// **G_BLANK_RENDER_CANNOT_SCORE — 100% of nothing is not 100%.**
    ///
    /// t650's HEAD-20 run scored `www.agoda.com` as `visual 69.9% · COVERAGE 100.0% · missing 0 ·
    /// verdict ok` on a render that is a **blank white page**. Coverage is computed against the
    /// oracle's `file://` probe, which built a 13-element shell; we had all 13, so we "covered" the
    /// page completely. Visual scored 69.9% because most of a page is background either way — which
    /// is the instrument's own warning (*"an entirely absent sidebar moved it <1 point"*) arriving as
    /// a false pass instead of a caveat.
    ///
    /// The synthetic images here are built to the same shape as the real pair: ours uniform, the
    /// oracle's inked. The thresholds are measured (see `BLANK_INK`), and the two guard cases below
    /// are the two real sites that must NOT trip it.
    #[test]
    fn a_page_we_drew_nothing_of_cannot_score() {
        let dir = std::env::temp_dir().join("manuk-blank-render-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // `frac` of the image gets content; the rest is background. `frac == 0.0` is a blank render.
        let write = |name: &str, frac: f64| -> std::path::PathBuf {
            let (w, h) = (256u32, 256u32);
            let mut buf = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
            let rows = (h as f64 * frac) as u32;
            for y in 0..rows {
                for x in 0..w {
                    buf.put_pixel(x, y, image::Rgba([10, 20, 30, 255]));
                }
            }
            let p = dir.join(name);
            buf.save(&p).unwrap();
            p
        };

        // The agoda shape: we drew nothing, the oracle drew a third of the page.
        let blank = write("blank.png", 0.0);
        let full = write("full.png", 0.35);
        let f = super::compare(&blank, &full, "agoda.example").unwrap();
        assert_eq!(
            f.unmeasurable,
            Some(super::Unmeasurable::RenderFailed),
            "a blank render against an inked oracle is a RENDER FAILURE, whatever the pixel score says"
        );
        assert_eq!(
            certificate(&[f]).scored,
            0,
            "and it must not be able to enter the scored set — this is the whole defect: agoda \
             scored `verdict ok` on a blank page"
        );

        // GUARD 1 — the ORACLE is the blank one (`comix` 0.00% / `naukri` 1.92% Chrome-side). That is
        // ShellOnly's job; blaming our renderer for the oracle's shell would be a false accusation.
        assert_eq!(
            super::compare(&blank, &write("shell.png", 0.0), "comix.example")
                .unwrap()
                .unmeasurable,
            None,
            "an oracle that drew nothing either is a SHELL, not our render failure"
        );

        // GUARD 2 — a page we render BADLY is not a page we failed to render. `keirin.jp` came in at
        // 8.11% ink with SHAPE 2.2%: terrible, and it must still be SCORED, because excusing our
        // worst real renders as unmeasurable is how a certificate launders its own failures.
        let bad = super::compare(&write("sparse.png", 0.081), &full, "keirin.example").unwrap();
        assert_eq!(
            bad.unmeasurable, None,
            "8% ink is a bad render, not an absent one — it must stay in the denominator AND in the \
             scored set"
        );
    }

    /// **G_CERT_CRASH_LEDGER — a sweep that dies mid-corpus must lose ONE site, not the run.**
    ///
    /// This session's HEAD-20 certificate attempts were killed by an engine SIGSEGV in two runs of
    /// three, at site 5 and at site 11. Both discarded every completed row, because `--rows-out` was
    /// written once after the loop. The certificate could not be measured at all — not because the
    /// engine is bad on those sites, but because the *instrument* staked twenty sites of work on the
    /// process surviving all twenty.
    ///
    /// Three properties, each asserted against the number that would be wrong without it:
    ///
    /// 1. **Durable as earned** — after N sites, N rows are readable by another process.
    /// 2. **A crash is COUNTED** — the in-flight marker the dead run left behind becomes a `crashed`
    ///    row, so the hardest site in the corpus enters the denominator instead of leaving it.
    /// 3. **Resume does not inflate** — re-running the crashed sweep supersedes rows rather than
    ///    appending duplicates, so `sites` stays 3 and does not climb to 5.
    #[test]
    fn a_crashed_sweep_keeps_its_completed_rows_and_counts_the_site_that_killed_it() {
        let dir = std::env::temp_dir().join("manuk-cert-crash-ledger-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.tsv");

        // ── The run that dies. Two sites complete and are flushed one at a time, exactly as the
        // sweep loop does; the third is claimed in-flight and then the process disappears.
        for r in [
            row("a.example", Some(0.91), [0; 4]),
            row("b.example", Some(0.80), [0; 4]),
        ] {
            super::mark_inflight(&path, &r.name).unwrap();
            super::append_rows_tsv(&path, std::slice::from_ref(&r)).unwrap();
            super::clear_inflight(&path);
        }
        super::mark_inflight(&path, "c.example").unwrap();
        // …SIGSEGV here. Nothing else runs — no final append, no clear.

        // (1) The completed work survived the process that was doing it.
        let after_crash = super::rows_from_tsv(&path).unwrap();
        assert_eq!(
            after_crash.len(),
            2,
            "the two sites that finished BEFORE the crash must be on disk — writing rows only at \
             the end of the run is what cost this session two HEAD-20 sweeps"
        );
        assert!(
            super::inflight_path(&path).exists(),
            "the marker must outlive the run that held it"
        );

        // (2) The next run attributes the crash instead of losing the site.
        let recovered = super::recover_inflight(&path);
        assert_eq!(recovered.as_deref(), Some("c.example"));
        assert!(
            !super::inflight_path(&path).exists(),
            "and clears the marker, so the NEXT crash is not blamed on this site"
        );
        let rows = super::rows_from_tsv(&path).unwrap();
        assert_eq!(rows.len(), 3, "three sites attempted, three sites counted");
        assert_eq!(
            rows[2].unmeasurable,
            Some(super::Unmeasurable::Crashed),
            "the site that killed the sweep is UNSCORED-with-a-reason, not absent"
        );
        let c = certificate(&rows);
        assert_eq!(c.sites, 3, "the denominator holds the crashed site");
        assert_eq!(c.scored, 2, "…and does not score it");
        assert_eq!(
            c.unmeasured_by_reason,
            vec![("crashed".to_string(), 1)],
            "the reason travels with the row, so the shortfall list can name it"
        );

        // Nothing is recovered when nothing crashed — a marker-less run must not manufacture a row.
        assert_eq!(super::recover_inflight(&path), None);
        assert_eq!(super::rows_from_tsv(&path).unwrap().len(), 3);

        // (3) The resumed run re-measures a.example and b.example and finishes c.example. Without
        // last-wins dedup this file reads as FIVE sites, and every ratio the certificate computes is
        // over a denominator nobody chose.
        for r in [
            row("a.example", Some(0.91), [0; 4]),
            row("b.example", Some(0.80), [0; 4]),
            row("c.example", Some(0.99), [0; 4]),
        ] {
            super::append_rows_tsv(&path, std::slice::from_ref(&r)).unwrap();
        }
        let rows = super::rows_from_tsv(&path).unwrap();
        assert_eq!(
            rows.len(),
            3,
            "a resumed sweep supersedes its own rows; it does not grow the corpus"
        );
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["a.example", "b.example", "c.example"],
            "and order follows first appearance, so the file still reads in sweep order"
        );
        assert_eq!(
            rows[2].unmeasurable, None,
            "the LAST row wins — a site that crashed once and then rendered is no longer crashed"
        );
        assert_eq!(certificate(&rows).scored, 3);
    }

    /// The chunked-sweep round trip. A 265-site sweep runs in timeout-isolated chunks, so the rows
    /// must survive the process boundary — and the ONE property that must survive is the one that
    /// makes the certificate honest: an UNSCORED site stays unscored across the boundary. If it came
    /// back as 0.0 it would count as "measured and failed" (wrong, but harmless); if it came back as
    /// 1.0, or vanished, the certificate could be met by a chunk that crashed.
    #[test]
    fn accumulated_rows_survive_the_chunk_boundary_with_unscored_still_unscored() {
        let dir = std::env::temp_dir().join("manuk-cert-rows-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.tsv");

        // Chunk 1: one good site, one that could not be shape-scored.
        let chunk1 = vec![
            row("good.example", Some(0.91), [0, 0, 0, 0]),
            Fidelity {
                shape: None,
                ..row("unprobeable.example", None, [0, 0, 0, 0])
            },
        ];
        super::append_rows_tsv(&path, &chunk1).unwrap();
        // Chunk 2: a separate process would append here.
        let chunk2 = vec![row("jarring.example", Some(0.99), [2, 0, 1, 0])];
        super::append_rows_tsv(&path, &chunk2).unwrap();

        let back = super::rows_from_tsv(&path).unwrap();
        assert_eq!(back.len(), 3, "append must not clobber the earlier chunk");
        assert_eq!(back[0].name, "good.example");
        assert!((back[0].shape.unwrap() - 0.91).abs() < 1e-6);
        assert_eq!(
            back[1].shape, None,
            "an UNSCORED site must read back as unscored, not as 0.0 and never as a pass — a chunk \
             that could not measure must not be able to satisfy the certificate"
        );
        assert_eq!(
            back[2].jarring,
            [2, 0, 1, 0],
            "the four counts survive in ORDER"
        );

        // And the certificate over the accumulated file agrees with the certificate over the rows.
        let c = certificate(&back);
        assert_eq!(c.sites, 3);
        assert_eq!(c.scored, 2);
        assert_eq!(c.shape_ok, 2);
        assert_eq!(c.clean, [2, 3, 2, 3]);
        assert!(!c.holds());

        // The header line is a comment and must not be read as a site.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("#name\t"),
            "a header, written once: {text:?}"
        );
        assert_eq!(
            text.matches("#name").count(),
            1,
            "and only once, across appends"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The vacuous-pass guard, and the certificate's first real sweep is the reason it exists.**
    ///
    /// `shape_stats` returns a RATIO, and `0/0` is 1.0. So seven of 55 swept sites reported
    /// `SHAPE: 100.0% … (0 scored)` and were counted as PASSING the placement bar — one of them
    /// `gov.uk`, whose 418 probed elements were **all missing**. A page we render nothing of scored
    /// perfect placement. That is the fifth time an instrument built here produced a bad number on its
    /// first real run, and the pattern is always the same shape: a denominator nobody checked.
    #[test]
    fn a_shape_score_over_an_empty_sample_is_never_a_pass() {
        // The exact row the sweep produced: nothing rendered, "perfect" shape.
        let vacuous = Fidelity {
            shape: Some(1.0),
            shape_n: 0,
            structure: Some(0.0),
            ..row("gov.uk", Some(1.0), [0; 4])
        };
        let c = certificate(&[vacuous]);
        assert_eq!(
            c.shape_ok, 0,
            "a shape ratio computed over ZERO elements must not count as meeting the placement bar"
        );
        assert_eq!(
            c.scored, 0,
            "…and the site is UNSCORED, which counts AGAINST the bar"
        );
        assert!(!c.holds());

        // Just below and just above the sample floor.
        let thin = Fidelity {
            shape_n: super::CERT_MIN_SHAPE_SAMPLE - 1,
            ..row("thin", Some(0.99), [0; 4])
        };
        let ok = Fidelity {
            shape_n: super::CERT_MIN_SHAPE_SAMPLE,
            ..row("ok", Some(0.99), [0; 4])
        };
        assert_eq!(
            certificate(&[thin]).scored,
            0,
            "a thin sample is not a verdict"
        );
        assert_eq!(certificate(&[ok]).scored, 1);

        // And the sample size must SURVIVE the chunk boundary, or a vacuous pass comes back from the
        // accumulated file even though it was refused in-process.
        let dir = std::env::temp_dir().join("manuk-cert-vacuous-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.tsv");
        super::append_rows_tsv(
            &path,
            &[Fidelity {
                shape: Some(1.0),
                shape_n: 0,
                ..row("gov.uk", Some(1.0), [0; 4])
            }],
        )
        .unwrap();
        let back = super::rows_from_tsv(&path).unwrap();
        assert_eq!(back[0].shape_n, 0, "the sample size round-trips");
        assert_eq!(
            certificate(&back).shape_ok,
            0,
            "and the guard still refuses it after the boundary"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **G_UNMEASURABLE_REASON — an unscored site must say WHY, and a refusal must never be scored.**
    ///
    /// The certificate has counted unscored sites against the bar since t583. What it could not do is
    /// say what went wrong, because the probe's fetch read `curl`'s *process* exit code and never the
    /// HTTP status — so a Cloudflare 403 interstitial, a 202 with an empty body and a genuine layout
    /// failure all arrived at the report as the same bare `—`. Measured across the 20 HEAD sites of
    /// `corpus-v2.tsv`: **six answer non-2xx or empty**, and every one of them was invisible.
    ///
    /// Three claims, and the second is the one with teeth:
    ///
    /// 1. **Each way of failing is classified apart**, because each implies a different remedy.
    /// 2. **A refusal is never scored.** For a 403 the challenge page is a real document both engines
    ///    render, so scoring it would report *high fidelity on a site we never reached* — a gate
    ///    passing by comparing a refusal against itself.
    /// 3. **The reason survives the chunk boundary**, or it vanishes exactly when the headline is
    ///    computed from the accumulated rows.
    /// **G_TREE_ALIGNMENT — the index-shift signature must be distinguishable from two different
    /// documents, on maps alone, with no network (t783).**
    ///
    /// Both failures produce the SAME visible symptom — `exact` ≈ 0 over two large trees — and they
    /// cost a subsystem each, in opposite directions. If this cannot tell them apart on synthetic
    /// input, the line it prints on a real sweep is decoration.
    #[test]
    fn tree_alignment_separates_an_inserted_node_from_two_different_pages() {
        use super::tree_alignment;
        use std::collections::HashMap;

        let mk = |paths: &[&str]| -> HashMap<String, [i64; 4]> {
            paths
                .iter()
                .map(|p| (p.to_string(), [0, 0, 1, 1]))
                .collect()
        };

        // ── ONE INSERTED NODE near the root. The oracle's document has an extra `<div>` as body's
        // first child, so every sibling below it is re-numbered — while the trees are otherwise the
        // same page.
        //
        // ⚠ **The inserted node shares its siblings' TAG on purpose (t784).** Since the key counts
        // per-tag ordinals, a *different*-tag insertion no longer shifts anything — which is the
        // whole point of the t784 change and would make this fixture measure nothing. A same-tag
        // insertion is the residual case the key still cannot absorb, so it is the one worth
        // holding a gate on.
        let chrome = mk(&[
            "body:nth-of-type(1)/div:nth-of-type(1)",
            "body:nth-of-type(1)/div:nth-of-type(2)",
            "body:nth-of-type(1)/div:nth-of-type(2)/p:nth-of-type(1)",
            "body:nth-of-type(1)/div:nth-of-type(3)",
            "body:nth-of-type(1)/div:nth-of-type(3)/p:nth-of-type(1)",
        ]);
        let ours = mk(&[
            "body:nth-of-type(1)/div:nth-of-type(1)",
            "body:nth-of-type(1)/div:nth-of-type(1)/p:nth-of-type(1)",
            "body:nth-of-type(1)/div:nth-of-type(2)",
            "body:nth-of-type(1)/div:nth-of-type(2)/p:nth-of-type(1)",
        ]);
        let a = tree_alignment(&chrome, &ours);
        // ⚠ **3, not 0 — and the reason is worth keeping.** A shift does not destroy every key: the
        // bare CONTAINER paths (`div:nth-of-type(1)`, `div:nth-of-type(2)`) still collide across the
        // shift because the sibling that moved into slot N has the same tag as the one that left it.
        // What a shift reliably destroys is the LEAVES, which is where a page's elements actually
        // are — real sweeps read `exact 0 of 1410`. So the test asserts the RELATION, not a zero.
        assert_eq!(a.exact, 3, "containers survive a shift; leaves do not");
        assert_eq!(
            a.tag_overlap, 4,
            "under a tag-only key the two trees agree on everything we built — the gap between \
             `exact` and `tag_overlap` IS the index-shift signature, and it is the whole point"
        );
        assert!(
            a.tag_overlap > a.exact,
            "an index-insensitive key must recover MORE than the index-bearing one, or this \
             measurement cannot discriminate at all"
        );
        assert_eq!(
            a.first_bad_depth,
            Some(1),
            "the extra node is body's own child, so depth 1 is where the counts first disagree — \
             that is what localises the insertion instead of merely reporting that one happened"
        );
        assert!(
            a.tag_overlap * 2 >= a.probed,
            "an insertion must read as INDEX SHIFT, i.e. a key problem a better key can fix"
        );

        // ── TWO DIFFERENT DOCUMENTS. Same sizes, same depths, nothing in common at any key
        // strength — the case where a better key buys exactly nothing and the fix is upstream, in
        // what the two engines were handed.
        let chrome = mk(&[
            "body:nth-of-type(1)/header:nth-of-type(1)",
            "body:nth-of-type(1)/header:nth-of-type(1)/nav:nth-of-type(1)",
            "body:nth-of-type(1)/main:nth-of-type(2)",
            "body:nth-of-type(1)/main:nth-of-type(2)/article:nth-of-type(1)",
        ]);
        let ours = mk(&[
            "body:nth-of-type(1)/form:nth-of-type(1)",
            "body:nth-of-type(1)/form:nth-of-type(1)/input:nth-of-type(1)",
            "body:nth-of-type(1)/aside:nth-of-type(2)",
            "body:nth-of-type(1)/aside:nth-of-type(2)/ul:nth-of-type(1)",
        ]);
        let a = tree_alignment(&chrome, &ours);
        assert_eq!(a.exact, 0);
        assert_eq!(
            a.tag_overlap, 0,
            "stripping the index must NOT manufacture agreement between two unrelated trees — a \
             weak key that matches everything would make this measurement vacuous"
        );
        assert!(
            !(a.probed > 0 && a.tag_overlap * 2 >= a.probed),
            "two different pages must NOT read as an index shift"
        );

        // ── And a healthy site is unremarkable at both strengths, so the verdict can never fire on
        // one.
        let same = mk(&[
            "body:nth-of-type(1)/div:nth-of-type(1)",
            "body:nth-of-type(1)/div:nth-of-type(2)",
        ]);
        let a = tree_alignment(&same, &same);
        assert_eq!((a.exact, a.tag_overlap, a.first_bad_depth), (2, 2, None));
    }

    #[test]
    fn an_unscored_site_must_name_its_cause() {
        use super::{classify_fetch, Unmeasurable};

        // ── 1. Classification. A body is supplied for each so the rule is exercised, not assumed.
        assert_eq!(
            classify_fetch(200, "<html>hi</html>"),
            None,
            "a 2xx with a body is measurable"
        );
        assert_eq!(
            classify_fetch(202, ""),
            Some(Unmeasurable::EmptyBody(202)),
            "imdb.com answers 202 with ZERO bytes — a 2xx is not enough, the body has to exist"
        );
        assert_eq!(
            classify_fetch(403, "<title>Just a moment...</title>"),
            Some(Unmeasurable::BotWall(403)),
            "a 403 is a refusal of this CLIENT, not a rendering failure"
        );
        assert_eq!(
            classify_fetch(429, "slow down"),
            Some(Unmeasurable::BotWall(429))
        );
        // **A SCHEME WITH NO HTTP STATUS IS NOT A REFUSAL.** G1's own corpus is `file://` snapshots,
        // where `curl -w '%{http_code}'` reports `000` because there is nothing to report — and the
        // first draft of this tick read that as `Unreachable` and turned the fidelity floor's two
        // static pages "unreachable" on the very next wall. `classify_fetch` never sees status 0 (the
        // caller decides on the body), so what is pinned here is the boundary either side of it.
        assert_eq!(
            classify_fetch(200, "<html>ok</html>"),
            None,
            "a 2xx with a body is measurable — the fixed G1 path"
        );

        // Every variant round-trips through its own tag, including the three that carry no status.
        // A tag that writes but does not read back is a reason that dies at the chunk boundary —
        // silently, and precisely when the headline is computed from the accumulated rows.
        for u in [
            Unmeasurable::Unreachable,
            Unmeasurable::BotWall(403),
            Unmeasurable::HttpStatus(404),
            Unmeasurable::EmptyBody(202),
            Unmeasurable::ProbeBlocked,
            Unmeasurable::RenderFailed,
            Unmeasurable::ShellOnly(3),
            Unmeasurable::Timeout(90),
            // ⚠ `ThinOverlap` and `Crashed` were missing from this loop — **checked, and they do
            // round-trip correctly**, so this is a COVERAGE gap being closed, not a bug being found.
            // Worth saying explicitly: the loop's comment claims a property ("a tag that writes but
            // does not read back dies at the chunk boundary") that it was only enforcing for 8 of the
            // 10 variants, so the claim was broader than the test. `CssStarved` is new (t751).
            Unmeasurable::ThinOverlap(4),
            Unmeasurable::Crashed,
            Unmeasurable::CssStarved(3),
            // `TreeDivergence` is new (t782) and its number is OUR element count, which on a real
            // page is four digits — so the round-trip is asserted on a value the parser could
            // plausibly choke on rather than on a friendly `4`.
            Unmeasurable::TreeDivergence(1487),
        ] {
            assert_eq!(
                Unmeasurable::from_tag(&u.tag()),
                Some(u.clone()),
                "{u:?} must survive its own tag"
            );
        }

        // ── **`css-starved-N` STAYS IN-SCOPE, AND THAT IS A DECISION, SO IT IS ASSERTED.**
        //
        // `fidelity-progress.sh` partitions on the reason STRING: a site is EXCLUDED from the Phase-0
        // denominator only when it is permanently unreachable by our own no-stealth policy — the tags
        // below. A starved stylesheet is not that: **our own load deadline cut the sheet**, the origin
        // served it. Landing it in EXCLUDED would launder our bug out of the denominator and *raise*
        // the headline, which is the exact failure `EXCLUDED-RISING` exists to alarm on.
        //
        // This mirrors the shell script's list in code so a future rename cannot quietly move the
        // reason across the line — the partition is the metric's definition, not a formatting detail.
        let excluded_prefixes = ["bot-wall", "empty-"];
        let excluded_exact = ["probe-blocked", "unreachable", "http-404", "http-503"];
        for ours in [
            Unmeasurable::CssStarved(3),
            Unmeasurable::RenderFailed,
            Unmeasurable::Crashed,
            Unmeasurable::ShellOnly(3),
            Unmeasurable::ThinOverlap(4),
            // ⚠ **`tree-divergence-N` STAYS IN-SCOPE TOO, AND THAT IS THE LOAD-BEARING HALF OF
            // t782.** The variant exists because `thin-overlap` was ASSERTING a coverage bug the
            // evidence did not support — and the tempting next step, moving those rows out of the
            // denominator because "the comparison is unsound", would launder 25 of 129 in-scope rows
            // into EXCLUDED and RAISE the headline for free. The comparison being unsound is a
            // reason to stop MIS-ATTRIBUTING it, never a reason to stop COUNTING it.
            Unmeasurable::TreeDivergence(1487),
        ] {
            let tag = ours.tag();
            assert!(
                !excluded_prefixes.iter().any(|p| tag.starts_with(p))
                    && !excluded_exact.contains(&tag.as_str()),
                "{tag} is OUR bug on a reachable site and must count against the in-scope \
                 denominator, never join the EXCLUDED tier"
            );
        }
        assert_eq!(
            classify_fetch(404, "gone"),
            Some(Unmeasurable::HttpStatus(404)),
            "a dead URL is corpus construction, and must not be filed as a bot wall"
        );
        // A 5xx is genuinely ambiguous — an origin can just be down — so it needs the marker.
        assert_eq!(
            classify_fetch(503, "backend down"),
            Some(Unmeasurable::HttpStatus(503))
        );
        assert_eq!(
            classify_fetch(503, "please wait <a>challenges.cloudflare.com</a>"),
            Some(Unmeasurable::BotWall(503)),
            "…but a 503 CARRYING a challenge marker is the same wall wearing a different code"
        );

        // ── 2. A refusal is never scored, however good the numbers attached to it look.
        //
        // This is the shape of the harm, not a hypothetical: Cloudflare's interstitial is a real
        // document, Chrome renders it, we render it, and the two agree. Left to the ordinary path it
        // would score as a PASS on a site we never reached.
        let refused = Fidelity {
            shape: Some(1.0),
            shape_n: 500,
            structure: Some(1.0),
            unmeasurable: Some(Unmeasurable::BotWall(403)),
            ..row("fdown.net", Some(1.0), [0; 4])
        };
        let c = certificate(&[refused.clone()]);
        assert_eq!(
            c.shape_ok, 0,
            "a bot-walled site must NOT meet the placement bar — a challenge page rendered \
             identically by both engines is agreement about Cloudflare, not about the site"
        );
        assert_eq!(c.scored, 0, "…and it is UNSCORED, counting against the bar");
        assert!(!c.holds());

        // ── The decomposition the pilot's "9 of 14 UNSCORED" could not produce.
        let rows = vec![
            refused.clone(),
            Fidelity {
                name: "supjav.com".into(),
                ..refused.clone()
            },
            Fidelity {
                name: "imdb.com".into(),
                unmeasurable: Some(Unmeasurable::EmptyBody(202)),
                ..refused.clone()
            },
            Fidelity {
                name: "csp-site".into(),
                unmeasurable: Some(Unmeasurable::ProbeBlocked),
                ..refused.clone()
            },
        ];
        let c = certificate(&rows);
        assert_eq!(
            c.unmeasured_by_reason,
            vec![
                ("bot-wall-403".to_string(), 2),
                ("empty-202".to_string(), 1),
                ("probe-blocked".to_string(), 1),
            ],
            "the unscored total must decompose by CAUSE, most common first — a single number is not \
             a work list, and 'find out why' is what the pilot could not answer"
        );
        let short = c.shortfalls().join(" | ");
        assert!(
            short.contains("2×bot-wall-403"),
            "the shortfall list must carry the decomposition, got: {short}"
        );

        // ── A refused site is a COUNTED ROW, and it is clean on NOTHING.
        //
        // The sweep loop used to `continue` past an unreachable origin, leaving no row — so the
        // site left the DENOMINATOR too and "sites N" shrank by however many origins refused us
        // that day. Both halves matter: the row must exist, and its all-zero jarring counts must
        // NOT read as four clean passes, or refusing to answer would become the cheapest way to
        // look clean.
        let dropped = Fidelity::unmeasured("supjav.com", Unmeasurable::BotWall(403));
        let d = certificate(&[row("real", Some(0.9), [0; 4]), dropped]);
        assert_eq!(d.sites, 2, "an unreachable site stays IN the denominator");
        assert_eq!(d.scored, 1, "…but is not scored");
        for i in 0..4 {
            assert_eq!(
                d.clean[i],
                1,
                "{} must count only the site we measured — a page we never fetched is not CLEAN, \
                 it is UNKNOWN, and counting its zeros as clean makes refusal the cheapest pass",
                super::JARRING_NAMES[i]
            );
        }

        // ── A SHELL IS NOT A SCORE. `comix.to` reported `coverage 66.7%` over THREE probed elements,
        // because the oracle's own `file://` copy cannot hydrate a JS-rendered page (28 elements here
        // vs ~2643 live). The certificate already refused to SCORE such a row — what it could not do
        // was say why, and an unexplained refusal is indistinguishable from an instrument gap.
        let shell = Fidelity {
            shape: Some(1.0),
            shape_n: 2,
            structure: Some(0.667),
            unmeasurable: Some(Unmeasurable::ShellOnly(3)),
            ..row("comix.to", Some(1.0), [0; 4])
        };
        let sc = certificate(&[shell]);
        assert_eq!(sc.sites, 1, "a shell-only site stays in the denominator");
        assert_eq!(
            sc.scored, 0,
            "…and is never scored — 66.7% of three elements is not a coverage"
        );
        assert!(
            sc.shortfalls().join(" | ").contains("shell-only-3"),
            "the shell must be named with the element count that proves it: {:?}",
            sc.shortfalls()
        );

        // ── A site unscored with NO reason is itself reported, so the decomposition can never look
        // complete just because the explained causes are the only ones printed.
        let mystery = Fidelity {
            shape: Some(1.0),
            shape_n: 0,
            unmeasurable: None,
            ..row("mystery", Some(1.0), [0; 4])
        };
        let short = certificate(&[mystery]).shortfalls().join(" | ");
        assert!(
            short.contains("NO recorded reason"),
            "an unexplained unscored site is an instrument gap and must be named, got: {short}"
        );

        // ── 3. The reason round-trips through the chunked-sweep file.
        let dir = std::env::temp_dir().join("manuk-cert-reason-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.tsv");
        super::append_rows_tsv(&path, &rows).unwrap();
        let back = super::rows_from_tsv(&path).unwrap();
        assert_eq!(
            back.iter().filter_map(|r| r.unmeasurable.clone()).count(),
            4,
            "every reason survives the chunk boundary"
        );
        assert_eq!(back[0].unmeasurable, Some(Unmeasurable::BotWall(403)));
        assert_eq!(back[2].unmeasurable, Some(Unmeasurable::EmptyBody(202)));
        assert_eq!(
            certificate(&back).unmeasured_by_reason,
            c.unmeasured_by_reason,
            "and the decomposition is identical computed from the file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// FALSIFYING THE CERTIFICATE (observer CO-#1, tick ~581 — the top-priority non-delusion guard)
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// One falsification: break the engine in a named way, and prove the certificate term that claims to
/// watch it goes RED.
///
/// **Why this is the highest-priority guard rather than a nicety.** `falsify.sh` was built because
/// `G_LOAD` — a *Bar 0* gate — had never tested the thing it was named for, `G1` was structurally
/// incapable of failing, and `G6` scored a browser finding zero links as perfect clickability. The
/// certificate is now the single number the whole Phase-0 exit hangs on, and it has never been asked
/// the same question: **is each of its terms capable of going red at all?** A term unmoved by a real
/// break is measuring nothing, and it would report the project done.
///
/// The perturbations are the observer's own examples — *"nudge a box 30px"* — applied to the real
/// scoring functions (`shape_stats`, `oracle::jarring_*`) rather than to the arithmetic downstream of
/// them. That distinction is the whole value: setting `Fidelity::shape` by hand and watching the
/// average move would prove only that division works.
pub struct Falsification {
    /// The certificate term this break must move, as `Cert::shortfalls` names it.
    pub term: &'static str,
    /// What was broken, in engine terms.
    pub break_desc: &'static str,
    /// Did the term go red?
    pub went_red: bool,
    /// Did the break leave the OTHER terms alone? A break that reddens everything proves nothing
    /// about which term watches what — see `SPECIFICITY` below.
    pub stayed_specific: bool,
}

/// A synthetic page both engines agree on perfectly: 24 boxes, four sibling groups of six, laid out
/// in a column inside a 1000px viewport. Chrome's map and ours start identical, so the certificate
/// holds on it — which is the precondition for a falsification to mean anything. **If the baseline
/// did not pass, every "went red" below would be vacuous.**
fn falsify_baseline() -> (
    std::collections::HashMap<String, crate::oracle::Seen>,
    std::collections::HashMap<String, crate::oracle::Seen>,
) {
    let mut m = std::collections::HashMap::new();
    for g in 0..4 {
        for i in 0..6 {
            let y = (g * 6 + i) as i64 * 40;
            let tag = if i == 0 { "a" } else { "div" };
            m.insert(
                format!(
                    "body/div:nth-of-type({}){}/{}:nth-of-type({})",
                    g + 1,
                    "",
                    tag,
                    i + 1
                ),
                crate::oracle::Seen {
                    tag: tag.to_string(),
                    display: "block".to_string(),
                    rect: [0, y, 200, 30],
                    font: String::new(),
                },
            );
        }
    }
    (m.clone(), m)
}

/// Score one (chrome, manuk) pair into a single-site `Fidelity` row through the REAL producers.
fn falsify_score(
    chrome: &std::collections::HashMap<String, crate::oracle::Seen>,
    manuk: &std::collections::HashMap<String, crate::oracle::Seen>,
) -> Fidelity {
    let cb: std::collections::HashMap<String, [i64; 4]> =
        chrome.iter().map(|(k, v)| (k.clone(), v.rect)).collect();
    let mb: std::collections::HashMap<String, [i64; 4]> =
        manuk.iter().map(|(k, v)| (k.clone(), v.rect)).collect();
    let (shape, shape_n) = shape_stats(&cb, &mb, 8);
    let (hov, _) = crate::oracle::jarring_h_overflow(chrome, manuk, 1000, 2);
    let (ovl, _, _) = crate::oracle::jarring_overlap(chrome, manuk, 2);
    let (ord, _, _) = crate::oracle::jarring_reading_order(chrome, manuk, 2);
    let (dead, _) = crate::oracle::jarring_collapsed_target(chrome, manuk, 8);
    Fidelity {
        name: "falsify".to_string(),
        score: 1.0,
        differing: 0,
        total: 0,
        structure: None,
        shape: Some(shape),
        missing: 0,
        misplaced: 0,
        probed: manuk.len(),
        jarring: [hov, ovl, ord, dead],
        shape_n,
        // A falsification row is synthetic: both sides are supplied, so there is no fetch to refuse.
        unmeasurable: None,
    }
}

/// The lexicographically first key — deterministic, so a failing falsification names the same
/// element every run rather than whichever the hash map happened to yield first.
fn first_key(m: &std::collections::HashMap<String, crate::oracle::Seen>) -> String {
    m.keys().min().cloned().unwrap_or_default()
}

/// A sibling of `k` (same parent path), deterministically chosen — sibling-scoped because that is
/// the scope `jarring_overlap` and `jarring_reading_order` work in.
fn sibling_key(
    m: &std::collections::HashMap<String, crate::oracle::Seen>,
    k: &str,
) -> Option<String> {
    let cut = k.rfind('/')?;
    let parent = &k[..cut];
    m.keys()
        .filter(|o| o.as_str() != k)
        .filter(|o| {
            o.len() > parent.len() && o.starts_with(parent) && o.as_bytes()[parent.len()] == b'/'
        })
        .filter(|o| !o[parent.len() + 1..].contains('/'))
        .min()
        .cloned()
}

/// Run every falsification. Returns one [`Falsification`] per certificate term.
///
/// **SPECIFICITY is asserted as well as redness, and getting it right CORRECTED THE CLAIM.** The
/// first version broke every box — and every jarring break moved SHAPE too, because all five terms
/// are functions of the same rects. That is not an instrument defect, it is arithmetic: you cannot
/// make a box overflow without moving it.
///
/// The claim that is both true and worth asserting is the **inverse**, and it is exactly why the
/// jarring invariants were added to the certificate (FIDELITY-SCORING-REDESIGN Layer 2):
///
/// > *"SHAPE cannot see it — two boxes can each be shaped correctly relative to their parent and
/// > still land on top of each other."*
///
/// SHAPE is a **fraction with a floor** (≥0.75 of nodes); the invariants are **zero-tolerance** (any
/// occurrence fails the site). So each jarring break here perturbs **one or two elements of
/// twenty-four** — well inside SHAPE's floor, which stays green — and the invariant still goes red.
/// That is the real-world case they exist for: a page almost right everywhere, with one control
/// buried under a banner. The SHAPE break conversely must move enough elements to breach the floor,
/// and is allowed to take the invariants with it.
pub fn falsify_certificate() -> Vec<Falsification> {
    let (chrome, base) = falsify_baseline();
    let baseline_cert = certificate(&[falsify_score(&chrome, &base)]);
    assert!(
        baseline_cert.holds(),
        "FALSIFY: the synthetic baseline must PASS the certificate before any break is applied — \
         otherwise every 'went red' below is vacuous. shortfalls: {:?}",
        baseline_cert.shortfalls()
    );

    // Each break: (term name, description, mutation).
    type Mut = Box<dyn Fn(&mut std::collections::HashMap<String, crate::oracle::Seen>)>;
    let breaks: Vec<(&'static str, &'static str, Mut)> = vec![
        (
            "shape",
            "nudge EVERY box 30px right of where Chrome puts it (the observer's own example) — enough elements to breach SHAPE's 0.75 floor",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                for v in m.values_mut() {
                    v.rect[0] += 30;
                }
            }),
        ),
        (
            "h-overflow",
            "spill ONE box of twenty-four past the 1000px viewport Chrome keeps it inside",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                let k = first_key(m);
                if let Some(v) = m.get_mut(&k) {
                    v.rect[2] = 1400;
                }
            }),
        ),
        (
            "overlap",
            "drop ONE box onto its sibling — the buried-control case SHAPE cannot see",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                let k = first_key(m);
                let onto = m.get(&k).map(|v| v.rect[1]).unwrap_or(0);
                if let Some(sib) = sibling_key(m, &k) {
                    if let Some(v) = m.get_mut(&sib) {
                        v.rect[1] = onto;
                    }
                }
            }),
        ),
        (
            "reading-order",
            "swap ONE pair of siblings vertically, leaving every other box where Chrome has it",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                let a = first_key(m);
                if let Some(b) = sibling_key(m, &a) {
                    let ya = m[&a].rect[1];
                    let yb = m[&b].rect[1];
                    m.get_mut(&a).unwrap().rect[1] = yb;
                    m.get_mut(&b).unwrap().rect[1] = ya;
                }
            }),
        ),
        (
            "dead-target",
            "collapse ONE link to a 1x1 box Chrome renders full-size — a click target that is perfectly placed and cannot be hit",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                let k = m
                    .iter()
                    .filter(|(_, v)| v.tag == "a")
                    .map(|(k, _)| k.clone())
                    .min()
                    .unwrap_or_default();
                if let Some(v) = m.get_mut(&k) {
                    v.rect[2] = 1;
                    v.rect[3] = 1;
                }
            }),
        ),
        (
            "UNSCORED",
            "render almost nothing, so the SHAPE sample falls below CERT_MIN_SHAPE_SAMPLE",
            Box::new(|m: &mut std::collections::HashMap<String, crate::oracle::Seen>| {
                let mut k: Vec<String> = m.keys().cloned().collect();
                k.sort();
                let keep: Vec<String> = k.into_iter().take(3).collect();
                m.retain(|k, _| keep.contains(k));
            }),
        ),
    ];

    let mut out = Vec::new();

    // ── The FUNCTION leg (CO-#1 item 3). It is not a term of `Cert` — it is the other half of
    //    `daily_driver_pass` — so it is falsified against its own producer rather than through the
    //    render certificate. The break is the observer's own example: *make IndexedDB throw*.
    {
        let works = crate::corpus::SiteFunction {
            site: "falsify".into(),
            caps: crate::corpus::FUNCTION_CAPS
                .iter()
                .map(|c| (c.to_string(), crate::corpus::CapOutcome::Works))
                .collect(),
        };
        assert!(
            works.functions() && crate::corpus::daily_driver_pass(true, Some(&works)),
            "FALSIFY: the FUNCTION baseline must PASS before the break, or 'went red' is vacuous"
        );
        let mut broken = works.clone();
        broken.caps[0].1 = crate::corpus::CapOutcome::Threw; // indexeddb
        out.push(Falsification {
            term: "FUNCTION",
            break_desc: "make IndexedDB throw — the killer that takes a site's own init path down \
                         with it (Firebase/Firestore open it during init)",
            went_red: !broken.functions() && !crate::corpus::daily_driver_pass(true, Some(&broken)),
            // The FUNCTION leg is independent of every RENDER term by construction: it reads no
            // rects. Breaking it must not, and cannot, move SHAPE or the invariants.
            stayed_specific: true,
        });
    }

    for (term, desc, mutate) in breaks {
        let mut broken = base.clone();
        mutate(&mut broken);
        let cert = certificate(&[falsify_score(&chrome, &broken)]);
        let shortfalls = cert.shortfalls().join(" | ");
        let went_red = !cert.holds() && shortfalls.contains(term);
        // SPECIFICITY, in the direction that is actually true and actually load-bearing: a jarring
        // break of one or two elements must leave SHAPE green. That is the invariants' whole reason
        // to exist — SHAPE is a fraction with a floor, they are zero-tolerance, and a page that is
        // almost right everywhere with one buried control must fail. `shape` and `UNSCORED` are
        // exempt: breaching the floor legitimately moves other terms, and rendering nothing
        // legitimately takes SHAPE with it. Claiming otherwise would be a false statement about the
        // instrument rather than a check on it.
        let stayed_specific = match term {
            "shape" | "UNSCORED" => true,
            _ => !shortfalls.contains("shape \u{2265}"),
        };
        out.push(Falsification {
            term,
            break_desc: desc,
            went_red,
            stayed_specific,
        });
    }
    out
}

#[cfg(test)]
mod bot_challenge_tests {
    use super::*;

    /// **A 200-OK bot challenge is a BOT WALL, not a render failure of ours.**
    ///
    /// Cloudflare's interstitial arrives as `200 OK` with a ~5.5 KB "Just a moment…" body, so every
    /// status-keyed bot-wall rule missed it, it painted a near-empty spinner, and the sweep charged
    /// it to the engine as `render-failed` — the one reason documented as *"our own bug."*
    /// Measured on the raw response bytes: `serverfault.com` 5,491 B / `askubuntu.com` 5,489 B /
    /// `mathoverflow.net` 5,492 B, against 205,345 B for the same URL from another client.
    ///
    /// RED-proof: drop the 2xx arm and this returns `None` — "measurable", which is how three sites
    /// became our paint bug.
    #[test]
    fn a_200_ok_cloudflare_challenge_is_a_bot_wall() {
        let body = r#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title>
            <meta http-equiv="content-security-policy" content="script-src https://challenges.cloudflare.com">
            </head><body><div id="challenge-running"></div></body></html>"#;
        assert_eq!(
            classify_fetch(200, body),
            Some(Unmeasurable::BotWall(200)),
            "a 200 challenge page is the origin refusing us AS A CLIENT — booking it render-failed \
             bills the engine for someone else's bot policy"
        );
    }

    /// **The over-correction guard, and it is the expensive direction.** Mislabelling a real render
    /// failure as a bot wall EXCUSES our own bug and removes it from the score. A genuine page that
    /// merely contains the English phrase "Just a moment" must stay measurable — which is why the
    /// 2xx arm tests only the infrastructure markers, never the prose ones.
    #[test]
    fn a_real_page_saying_just_a_moment_is_still_measurable() {
        let body = "<html><head><title>Blog</title></head><body><p>Just a moment, \
                    I'll explain. Attention Required for step 3.</p></body></html>";
        assert_eq!(
            classify_fetch(200, body),
            None,
            "prose is not a bot wall — excusing a render failure is worse than counting one"
        );
    }

    /// The 5xx path keeps its wider marker list: a server error carrying challenge prose is a wall
    /// whichever words it uses, and there is no genuine-content risk at 5xx.
    #[test]
    fn a_5xx_challenge_still_matches_on_prose() {
        assert_eq!(
            classify_fetch(503, "<html>Attention Required | Cloudflare</html>"),
            Some(Unmeasurable::BotWall(503))
        );
    }
}

#[cfg(test)]
mod corpus_parse_tests {
    use super::*;

    /// **G_CORPUS_NONEMPTY — the corpus file the Phase-0 sweep reads must actually parse.**
    ///
    /// This gate exists because the sweep silently swept ZERO sites and reported a certificate: the
    /// splitter was `'\t'`-only and `docs/bench/oracle-corpus.txt` is space-aligned, so 265 lines
    /// became 0 URLs and the run printed `sites 0 · scored 0 · shape ≥0.75 on 0 (0.0%)`. Nothing in
    /// that output said *the corpus was empty* — the shortfall lines look identical to a sweep that
    /// ran and failed.
    ///
    /// The real corpus file is read from disk on purpose. A gate that asserts against a string
    /// literal of what I *believe* the corpus looks like tests my belief; this tests the file the
    /// sweep will actually open, so re-aligning the columns in that file fails HERE rather than in a
    /// three-hour sweep that quietly measures nothing.
    #[test]
    fn the_real_corpus_file_parses_to_urls_not_to_silence() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/bench/oracle-corpus.txt"
        );
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("the Phase-0 corpus must be readable at {path}: {e}"));
        let got = parse_corpus(&text);
        assert!(
            got.candidates > 200,
            "the corpus is the 265-site frame; {} candidate lines is not it",
            got.candidates
        );
        assert_eq!(
            got.urls.len(),
            got.candidates,
            "EVERY non-comment line must parse as a URL — {} of {} did, so {} sites would be \
             silently dropped from the certificate's denominator",
            got.urls.len(),
            got.candidates,
            got.candidates - got.urls.len()
        );
        assert!(
            got.urls.iter().all(|u| u.starts_with("http")),
            "a parsed entry that is not a URL will be fetched as one"
        );
    }

    /// **RED-proof, and the actual regression.** Both corpus shapes must reduce to the URL: the
    /// space-aligned one (`oracle-corpus.txt`) and the tab-separated one (`corpus-v2.tsv`). Restore
    /// the `'\t'`-only split and the first of these comes back whole and is filtered out — which is
    /// precisely the bug, reproduced.
    #[test]
    fn a_space_aligned_corpus_line_reduces_to_its_url() {
        let got = parse_corpus(
            "# a comment\n\nnews         https://nytimes.com/\nshop\thttps://ebay.com/\nhttps://bare.example/\n",
        );
        assert_eq!(got.candidates, 3);
        assert_eq!(
            got.urls,
            vec![
                "https://nytimes.com/".to_string(),
                "https://ebay.com/".to_string(),
                "https://bare.example/".to_string(),
            ],
            "space-aligned, tab-separated and bare lines must all reduce to the URL"
        );
    }

    /// A file with lines but no URLs must report `candidates > 0, urls == 0` — the state the caller
    /// turns into a hard exit. Without the second number this is indistinguishable from an empty
    /// file, and both are indistinguishable from a corpus that scored 0%.
    #[test]
    fn a_corpus_that_parses_to_nothing_still_reports_its_line_count() {
        let got = parse_corpus("# header\nnews nytimes.com\nshop ebay.com\n");
        assert_eq!(
            (got.urls.len(), got.candidates),
            (0, 2),
            "the LINE COUNT is the second population — it is what proves 0 URLs is a parse failure \
             and not an empty file"
        );
    }
}

#[cfg(test)]
mod falsify_tests {
    use super::*;

    /// **G_CERT_FALSIFIABLE — every certificate term is proven capable of going RED.**
    ///
    /// The observer's CO-#1 as of tick 581, and it is first in the order for a reason: the recent
    /// episode produced *six flattering numbers in one session*, and the certificate is the single
    /// claim the whole Phase-0 exit rests on. `falsify.sh` exists because `G_LOAD` — a Bar 0 gate —
    /// had never tested what it was named for. **No cert term is trusted until it is proven to go
    /// red**, and this is the proof, re-run on every wall.
    ///
    /// Two claims per term, and the second is the one that catches a lazy instrument:
    ///
    /// 1. **It goes red.** A real, engine-shaped break (nudge a box 30px; collapse an interactive
    ///    element; invert sibling order) must make that term fail.
    /// 2. **It stays specific.** The break must NOT redden the other terms. A certificate whose terms
    ///    all move together is one term wearing five hats — it would still "hold" or "fail" correctly
    ///    in aggregate while being unable to tell the next tick *what* to fix, which is the whole
    ///    reason the certificate is a list of terms rather than an average.
    ///
    /// `UNSCORED` is exempt from claim 2 and says so in the harness: a page we render nothing of
    /// legitimately takes the shape term with it, and asserting otherwise would be a false claim
    /// about the instrument rather than a check on it.
    #[test]
    fn every_certificate_term_can_go_red() {
        let results = falsify_certificate();
        // ⚠ THIS NUMBER IS A RATCHET TOOTH, NOT A CONSTANT TO KEEP CURRENT. It is what makes
        // "every term is falsifiable" enforceable rather than aspirational: add a term to the
        // certificate and this assertion goes RED until someone writes its break. It has already
        // done its job once — t585 added the FUNCTION leg and this line failed with `left: 7,
        // right: 6` before the falsification was wired, which is the guard working exactly as t583
        // said it would. **Raise it only in the same commit that adds the falsification.**
        assert_eq!(
            results.len(),
            7,
            "the certificate has seven terms (UNSCORED, shape, four jarring invariants, and the \
             FUNCTION leg); a term added without a falsification is a term nobody has proven can fail"
        );
        for f in &results {
            assert!(
                f.went_red,
                "G_CERT_FALSIFIABLE: breaking the engine — {} — did NOT move the `{}` term. \
                 A certificate term unmoved by a real break is measuring nothing, and it would \
                 report this project done.",
                f.break_desc, f.term
            );
            assert!(
                f.stayed_specific,
                "G_CERT_FALSIFIABLE: breaking the engine — {} — reddened terms other than `{}`. \
                 A certificate whose terms move together cannot say WHICH capability regressed, \
                 which is the only thing a list of terms buys over an average.",
                f.break_desc, f.term
            );
        }
    }
}

#[cfg(test)]
mod spread_tests {
    use super::shape_spreads;

    /// The rows a real repeated sweep writes: `name \t coverage \t shape \t ...`.
    const ROWS: &str = "\
#name\tcoverage\tshape\th_overflow\toverlap\treading_order\tdead_target\tshape_n\treason
www.ikea.com\t1.000000\t0.518625\t0\t5\t19\t0\t698\t
keirin.jp\t0.714489\t0.404427\t3\t5\t27\t0\t497\t
www.ikea.com\t1.000000\t0.515759\t0\t5\t19\t0\t698\t
keirin.jp\t0.712000\t0.367347\t3\t5\t32\t0\t490\t
www.desitales2.com\t1.000000\t0.637124\t0\t0\t10\t0\t598\t
supjav.com\t-\t-\t0\t0\t0\t0\t0\tbot-wall-403
supjav.com\t-\t-\t0\t0\t0\t0\t0\tbot-wall-403
";

    /// The measurement this exists for: **a site measured twice on one tree has a range, and the
    /// range is the error bar every per-site delta must clear.** These are tick 657's real readings.
    #[test]
    fn a_site_measured_twice_reports_its_range_worst_first() {
        let got = shape_spreads(ROWS);
        let names: Vec<&str> = got.iter().map(|(n, ..)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["keirin.jp", "www.ikea.com"],
            "expected exactly the two repeated SCORED sites, worst spread first — got {got:?}"
        );

        let (_, min, max, runs) = &got[0];
        assert_eq!(*runs, 2);
        assert!(
            (*min - 0.367347).abs() < 1e-9 && (*max - 0.404427).abs() < 1e-9,
            "keirin's range is not its measured min..max: {min}..{max}"
        );
        // 3.7 points — five times the 0.7-point "regression" tick 657 was about to attribute to a
        // code change. If this ever computes as ~0, the spread has stopped being reported and every
        // per-site comparison silently loses its error bar again.
        assert!(
            (max - min) * 100.0 > 3.0,
            "the spread collapsed to {:.2} pts — a spread of zero is how a noisy number starts \
             looking like a precise one",
            (max - min) * 100.0
        );
    }

    /// **An unscored row is not a measurement**, and must never widen a spread. A site that rendered
    /// once and bot-walled once was measured ONCE; reading `-` as a score would manufacture a spread
    /// covering the site's whole range out of a row that never carried a number.
    #[test]
    fn unscored_rows_and_single_readings_produce_no_spread() {
        let got = shape_spreads(ROWS);
        assert!(
            !got.iter().any(|(n, ..)| n == "supjav.com"),
            "a site with two UNSCORED rows was given a spread — `-` was parsed as a number"
        );
        assert!(
            !got.iter().any(|(n, ..)| n == "www.desitales2.com"),
            "a site measured ONCE was given a spread"
        );
        assert!(
            shape_spreads("").is_empty() && shape_spreads("#only a header\n").is_empty(),
            "an empty or header-only file must yield no spread rows"
        );
    }

    /// One scored row with a named site and an explicit oracle sample size — the two fields the
    /// consecutive-run collapse reads. Built here rather than reused from `spread_tests` because the
    /// sample size is the whole subject of the test below and no existing helper carries it.
    fn row_named(name: &str, shape: Option<f64>, shape_n: usize) -> super::Fidelity {
        super::Fidelity {
            name: name.to_string(),
            score: f64::NAN,
            differing: 0,
            total: 0,
            structure: Some(0.5),
            shape,
            missing: 0,
            misplaced: 0,
            probed: 0,
            jarring: [0; 4],
            shape_n,
            unmeasurable: None,
        }
    }

    /// **A REPEAT THAT MEASURED NOTHING IS NOT PAID FOR TWICE** (tick 687). Tick 681's sweep repeated
    /// the four sites the plan named and three of them returned an IDENTICAL shape on all three renders
    /// — the document snapshot is cached, so those were three renders of the same bytes. Six extra live
    /// renders per sweep, forever, for an error bar of exactly zero.
    ///
    /// The fixture is those real runs: `keirin.jp` three times identical (retire the repeats),
    /// `www.agoda.com` three times varying (keep them), and `www.ikea.com` with only ONE reading in its
    /// run (unknown, so untouched — this rule may only ever retire a repeat already paid for once).
    ///
    /// ⚠ **`keirin.jp` needs an EARLIER, DIFFERING reading in the file, and the first draft of this test
    /// did not have one — which made the plan assertion VACUOUS.** With only the identical run present,
    /// `shape_spreads` reports Δ 0.0 and the pre-existing `> SPREAD_UNSTABLE_PTS` filter already drops the
    /// site, so disabling the new guard changed nothing and the mutation stayed GREEN. The guard only bites
    /// where the file holds BOTH a wide cross-run spread and a flat within-sweep run — which is exactly
    /// keirin's real state (Δ 52.6 across sweeps, Δ 0.0 within one). Third vacuous assertion caught by
    /// running the mutation this session.
    #[test]
    fn a_repeat_that_measured_nothing_is_not_paid_for_twice() {
        const ROWS: &str = "\
#name\tcoverage\tshape\th_overflow\toverlap\treading_order\tdead_target\tshape_n\treason\tinstrument
keirin.jp\t0.714489\t0.404427\t3\t5\t27\t0\t497\t\tbbbb2222
www.ikea.com\t0.970793\t0.507163\t0\t5\t19\t0\t698\t\tbbbb2222
keirin.jp\t0.746408\t0.571704\t3\t4\t27\t0\t1039\t\tbbbb2222
keirin.jp\t0.746408\t0.571704\t3\t4\t27\t0\t1039\t\tbbbb2222
keirin.jp\t0.746408\t0.571704\t3\t4\t27\t0\t1039\t\tbbbb2222
www.agoda.com\t0.080446\t0.507692\t0\t0\t0\t0\t65\t\tbbbb2222
www.agoda.com\t0.012376\t0.100000\t0\t0\t0\t0\t10\t\tbbbb2222
www.agoda.com\t0.012376\t0.100000\t0\t0\t0\t0\t10\t\tbbbb2222
www.ikea.com\t0.970793\t0.507163\t0\t5\t19\t0\t698\t\tbbbb2222
";
        let det = super::within_sweep_deterministic(ROWS);
        assert!(
            det.contains("keirin.jp"),
            "keirin drew the SAME number three times in one sweep — repeating it again samples nothing, \
             because the snapshot is cached and those are three renders of identical bytes. Got {det:?}"
        );
        assert!(
            !det.contains("www.agoda.com"),
            "agoda's three draws differ by 40.8 points WITHIN one sweep, which is exactly the variance \
             the repeats exist to median away. Retiring them would hand the certificate a single draw \
             from the widest distribution in the corpus. Got {det:?}"
        );
        assert!(
            !det.contains("www.ikea.com"),
            "ikea has ONE reading in its run, so its within-sweep spread is UNKNOWN, not zero. This rule \
             may only ever retire a repeat that has already been paid for once. Got {det:?}"
        );

        let plan = super::repeat_plan(ROWS);
        let names: Vec<&str> = plan.iter().map(|(n, ..)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["www.agoda.com"],
            "the plan must keep the one site whose repeats measure something and drop the one whose do \
             not — got {plan:?}"
        );
    }

    /// **AGODA'S AND NAUKRI'S REAL DRAWS, AND THE RETRACTION OF TICK 681** (tick 682).
    ///
    /// t681 read agoda's `shape_n` of 65, 10, 10 and concluded the ORACLE had served a different
    /// document, so the thin draws were disqualified. **The log of that same sweep says otherwise:**
    ///
    /// ```text
    ///   www.agoda.com   structural: 8.0% (808 paths, 743 missing)  -> shared 65
    ///   www.agoda.com   structural: 1.2% (808 paths, 798 missing)  -> shared 10
    ///   www.naukri.com  structural:17.5% ( 57 paths,  47 missing)  -> shared 10
    ///   www.naukri.com  structural:15.8% ( 57 paths,  48 missing)  -> shared  9
    /// ```
    ///
    /// 808 paths in every agoda draw, 57 in every naukri draw. `shape_n` is the SHARED count, not the
    /// oracle's population — so the document never changed and **the variance is ours**, which is
    /// precisely what the median exists to absorb. The filter was keeping our best draw and discarding
    /// our typical one, and it moved `scored` 5 → 6 on the tick that introduced it.
    ///
    /// So this test now asserts the OPPOSITE of what it asserted for one tick: the median stands, and
    /// ties at the median break toward the SMALLEST sample — a bar must never be cleared by a
    /// convention, which is the same reason an even-length run takes its lower middle.
    #[test]
    fn our_own_variance_is_what_the_median_is_for_and_ties_break_conservatively() {
        let rows = vec![
            row_named("www.agoda.com", Some(0.507692), 65),
            row_named("www.agoda.com", Some(0.100000), 10),
            row_named("www.agoda.com", Some(0.100000), 10),
            row_named("www.naukri.com", Some(0.0), 10),
            row_named("www.naukri.com", Some(0.0), 9),
            row_named("www.naukri.com", Some(0.0), 9),
        ];
        let got = super::collapse_repeats(rows);
        assert_eq!(got.len(), 2, "two sites, one row each — got {}", got.len());

        let agoda = &got[0];
        assert!(
            (agoda.shape.unwrap() - 0.100000).abs() < 1e-9,
            "agoda's three draws are OUR OWN variance on one unchanged document (808 oracle paths in \
             all three), so the certificate must read the MEDIAN — 0.100, the typical draw — and not \
             the best one. Reading 0.508 here is the flattering direction, and it is the mistake tick \
             681 made and this test now forbids. Got {:?}",
            agoda.shape
        );

        let naukri = &got[1];
        assert_eq!(
            naukri.shape_n, 9,
            "naukri's three draws TIE at shape 0.0 and differ only in sample size, so the tie must \
             break toward the SMALLEST — n=9, below CERT_MIN_SHAPE_SAMPLE, leaving the site honestly \
             UNSCORED. Breaking toward n=10 clears the sample floor by choosing the draw that helps, \
             which is exactly the convention a bar must never be cleared by."
        );
    }

    /// **THE STEP CHANGE TICK 674 PUT IN THE FILE, REPRODUCED.** These are the real readings: the
    /// same four sites, measured once by the pre-`load` oracle probe (`aaaa1111`) and once by the
    /// deferred one (`bbbb2222`). Under the old rule the block printed keirin at Δ 52.6 pts and
    /// naukri at Δ 100.0 on a corpus whose every genuine per-site spread had been ≤ 3.7 — and
    /// `repeat_plan` reads this function, so all four would have been rendered three times on every
    /// sweep from then on to re-measure a variance that is not variance.
    ///
    /// The claim is exact: with two instruments in the file, the error bar comes from the LAST one
    /// only, so keirin — measured once by the current instrument — has **no spread at all**.
    #[test]
    fn a_step_change_in_the_instrument_is_not_an_error_bar_on_the_subject() {
        const MIXED: &str = "\
#name\tcoverage\tshape\th_overflow\toverlap\treading_order\tdead_target\tshape_n\treason\tinstrument
keirin.jp\t0.707753\t0.047753\t79\t5\t3\t0\t356\t\taaaa1111
playhop.com\t0.964912\t0.636364\t0\t22\t1\t0\t550\t\taaaa1111
www.ikea.com\t1.000000\t0.518625\t0\t5\t19\t0\t698\t\taaaa1111
keirin.jp\t0.744253\t0.573359\t3\t4\t27\t0\t1036\t\tbbbb2222
playhop.com\t0.046729\t0.200000\t0\t0\t0\t0\t5\tthin-overlap-5\tbbbb2222
www.ikea.com\t0.970793\t0.507163\t0\t5\t19\t0\t698\t\tbbbb2222
www.ikea.com\t0.970793\t0.505000\t0\t5\t19\t0\t698\t\tbbbb2222
";
        let got = shape_spreads(MIXED);
        assert!(
            !got.iter().any(|(n, ..)| n == "keirin.jp"),
            "keirin was measured ONCE by the current instrument and once by the previous one — that \
             is not a spread, it is a step change. Got {got:?}"
        );
        assert!(
            !got.iter().any(|(n, ..)| n == "playhop.com"),
            "playhop's 550 -> 5 element collapse is the ORACLE'S population changing, not the \
             site's noise. Got {got:?}"
        );
        // ...and the real within-instrument spread SURVIVES. A fix that suppressed every spread
        // would pass both assertions above and destroy the error bar this block exists for.
        let ikea = got.iter().find(|(n, ..)| n == "www.ikea.com").expect(
            "ikea's two readings on the CURRENT instrument are a real spread and must survive",
        );
        assert_eq!(
            ikea.3, 2,
            "ikea's spread must come from exactly its two current-instrument rows"
        );
        assert!(
            (ikea.1 - 0.505).abs() < 1e-9 && (ikea.2 - 0.507163).abs() < 1e-9,
            "ikea's range must be its two CURRENT rows (0.505..0.507163), not the older one: {ikea:?}"
        );

        // The mixture is DISCLOSED, not silently dropped: three versions' worth of accounting, in
        // first-appearance order, with the current one last.
        let mix = super::instrument_mix(MIXED);
        assert_eq!(
            mix,
            vec![("aaaa1111".to_string(), 3), ("bbbb2222".to_string(), 4)],
            "the instrument mix must be counted and ordered by first appearance: {mix:?}"
        );

        // An UNTAGGED file — every sweep already banked in `docs/bench/` — behaves exactly as before.
        assert_eq!(
            shape_spreads(ROWS).len(),
            2,
            "a file with no instrument column must keep the old behaviour exactly"
        );
    }
}

/// **G_UNSTABLE_SITE_IS_MEASURED_NOT_DRAWN** — the certificate may not take one draw from a
/// distribution this instrument has already measured as wide.
///
/// The whole module is one tick-672 reading, reproduced: `keirin.jp` scored **0.048** in a sweep
/// whose own spread block, printed four lines above the certificate, said that site ranges over
/// **34.9 points**. Nothing consumed the spread, so the draw became the score, and the next sentence
/// I was writing was a 35-point regression report aimed at my own previous tick.
#[cfg(test)]
mod repeat_tests {
    use super::{
        repeat_plan, repeat_urls, rows_from_tsv, Fidelity, Unmeasurable, SPREAD_UNSTABLE_PTS,
        UNSTABLE_REPEATS,
    };

    /// Write a rows file the way a sweep does, so the test exercises the real parser and not a
    /// hand-built `Vec<Fidelity>` that could disagree with what is on disk. Named after the case so
    /// two tests running in parallel cannot read each other's file — a shared path here would make
    /// these tests flake in exactly the way they exist to stop the sweep flaking.
    fn rows_file(case: &str, body: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("manuk-repeat-{case}-{}.tsv", std::process::id()));
        std::fs::write(&p, body).expect("write rows");
        p
    }

    fn shape_of(rows: &[Fidelity], name: &str) -> Option<f64> {
        rows.iter().find(|r| r.name == name)?.shape
    }

    /// **THE RED.** keirin's three real readings, in the order that hurts: the outlier LAST. Under
    /// the old last-wins collapse the certificate reads 0.048 and the loop publishes a 35-point
    /// regression. The median is 0.400 — which is where keirin has sat all session (t657 0.3996,
    /// t659 0.3972, t672's three controls 0.400 / 0.351 / 0.402).
    #[test]
    fn a_consecutive_run_collapses_to_its_median_not_its_last_draw() {
        let p = rows_file(
            "median-last",
            "#name\tcoverage\tshape\th\to\tr\td\tshape_n\treason\n\
             keirin.jp\t0.714\t0.400398\t3\t5\t27\t0\t502\t\n\
             keirin.jp\t0.712\t0.402000\t3\t5\t27\t0\t500\t\n\
             keirin.jp\t0.700\t0.047800\t3\t5\t27\t0\t479\t\n",
        );
        let rows = rows_from_tsv(&p).expect("parse");
        assert_eq!(
            rows.len(),
            1,
            "three draws of one site are ONE site, got {}",
            rows.len()
        );
        let got = shape_of(&rows, "keirin.jp").expect("scored");
        assert!(
            (got - 0.400398).abs() < 1e-9,
            "the certificate took {got:.6} for keirin.jp. The median of its three draws is 0.400398; \
             0.047800 is the LAST draw, and publishing it is the 35-point phantom regression of tick 672."
        );
    }

    /// The other order, because a collapse that only happens to be right when the outlier is last is
    /// not a median. Outlier FIRST must land on the same number.
    #[test]
    fn the_median_does_not_depend_on_which_draw_was_unlucky() {
        let p = rows_file(
            "median-first",
            "keirin.jp\t0.700\t0.047800\t3\t5\t27\t0\t479\t\n\
             keirin.jp\t0.714\t0.400398\t3\t5\t27\t0\t502\t\n\
             keirin.jp\t0.712\t0.402000\t3\t5\t27\t0\t500\t\n",
        );
        let got = shape_of(&rows_from_tsv(&p).expect("parse"), "keirin.jp").expect("scored");
        assert!(
            (got - 0.400398).abs() < 1e-9,
            "median changed to {got:.6} when the outlier moved to the front — that is a position \
             rule wearing a median's name"
        );
    }

    /// **The rule the median must not break.** Rows separated by another site are a RESUME, not a
    /// repeat: the later row is a re-measurement and supersedes. Collapsing those to a median would
    /// blend a crashed run's tree with the current one — and would resurrect the exact denominator
    /// bug `rows_from_tsv` was written to close.
    #[test]
    fn separated_rows_are_a_re_measure_and_the_later_one_still_wins() {
        let p = rows_file(
            "separated",
            "a.example\t0.5\t0.100000\t0\t0\t0\t0\t400\t\n\
             b.example\t0.5\t0.500000\t0\t0\t0\t0\t400\t\n\
             a.example\t0.9\t0.600000\t0\t0\t0\t0\t400\t\n",
        );
        let rows = rows_from_tsv(&p).expect("parse");
        assert_eq!(rows.len(), 2, "two sites, three rows -> two rows");
        let got = shape_of(&rows, "a.example").expect("scored");
        assert!(
            (got - 0.600000).abs() < 1e-9,
            "a separated repeat read {got:.6} — the median blended a superseded run into the \
             current one instead of letting the re-measurement win"
        );
    }

    /// A `crashed` row followed immediately by a successful render is ONE measurement, not two, and
    /// the measurement wins. An unscored draw that could vote in a median would let a bot-wall erase
    /// two real numbers by sitting in the middle.
    #[test]
    fn an_unscored_draw_does_not_vote_in_the_median() {
        let p = rows_file(
            "unscored-vote",
            "a.example\t-\t-\t0\t0\t0\t0\t0\tcrashed\n\
             a.example\t0.9\t0.600000\t0\t0\t0\t0\t400\t\n\
             a.example\t-\t-\t0\t0\t0\t0\t0\tbot-wall-403\n",
        );
        let rows = rows_from_tsv(&p).expect("parse");
        assert_eq!(rows.len(), 1);
        let got = shape_of(&rows, "a.example");
        assert_eq!(
            got,
            Some(0.600000),
            "the run's only real measurement lost to an unscored row: {got:?}"
        );
        // ...and a run with NO number at all keeps the LAST row, which is the old behaviour for the
        // population that never had a median to take.
        let p2 = rows_file(
            "all-unscored",
            "a.example\t-\t-\t0\t0\t0\t0\t0\tcrashed\n\
             a.example\t-\t-\t0\t0\t0\t0\t0\tbot-wall-403\n",
        );
        let rows2 = rows_from_tsv(&p2).expect("parse");
        assert_eq!(rows2.len(), 1);
        assert_eq!(
            rows2[0].unmeasurable,
            Some(Unmeasurable::BotWall(403)),
            "a run of unscored draws must keep the LAST reason, not the first"
        );
    }

    /// **An even run takes the LOWER middle.** There is no middle draw to take, and a certificate
    /// that resolves its own ambiguity upward is one that can be cleared by a rounding convention.
    #[test]
    fn an_even_run_rounds_away_from_the_bar() {
        let p = rows_file(
            "even-run",
            "a.example\t0.9\t0.700000\t0\t0\t0\t0\t400\t\n\
             a.example\t0.9\t0.800000\t0\t0\t0\t0\t400\t\n",
        );
        let got = shape_of(&rows_from_tsv(&p).expect("parse"), "a.example").expect("scored");
        assert!(
            (got - 0.700000).abs() < 1e-9,
            "an even run resolved to {got:.6}; 0.80 clears CERT_SHAPE_FLOOR and 0.70 does not, so \
             taking the upper middle would let a tie decide a certificate term"
        );
    }

    /// The plan is read from the instrument's OWN accumulated rows: only the sites whose measured
    /// spread exceeds the threshold, and nobody else. A blanket triple-run would triple a 30-minute
    /// sweep to buy precision on the five sites that are already deterministic.
    #[test]
    fn only_the_sites_the_spread_block_calls_unstable_are_repeated() {
        const ROWS: &str = "\
keirin.jp\t0.714\t0.397200\t0\t0\t0\t0\t500\t
www.ikea.com\t1.0\t0.518625\t0\t0\t0\t0\t698\t
keirin.jp\t0.700\t0.047800\t0\t0\t0\t0\t479\t
www.ikea.com\t1.0\t0.515759\t0\t0\t0\t0\t698\t
www.desitales2.com\t1.0\t0.637124\t0\t0\t0\t0\t598\t
";
        let plan = repeat_plan(ROWS);
        let names: Vec<&str> = plan.iter().map(|(n, ..)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["keirin.jp"],
            "the plan must name keirin (Δ 34.9 pts) and NOT ikea (Δ 0.3) or desitales2 (one \
             reading): {plan:?}"
        );
        assert_eq!(plan[0].1, UNSTABLE_REPEATS);
        assert!(
            plan[0].2 > SPREAD_UNSTABLE_PTS,
            "the plan reported a spread of {:.1} pts, which is not above its own threshold",
            plan[0].2
        );

        // The expansion is CONSECUTIVE — `rows_from_tsv` medians a consecutive run and last-wins
        // across separated ones, so scattered repeats would be fed to the wrong rule entirely.
        let urls: Vec<String> = ["https://keirin.jp/", "https://www.ikea.com/"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (expanded, _) = repeat_urls(&urls, ROWS);
        assert_eq!(
            expanded,
            vec![
                "https://keirin.jp/",
                "https://keirin.jp/",
                "https://keirin.jp/",
                "https://www.ikea.com/",
            ],
            "keirin must appear {UNSTABLE_REPEATS}× BACK TO BACK and ikea exactly once"
        );

        // No history, no plan — the two-site wall gate must not start rendering anything three times
        // because a file happened not to exist.
        assert!(repeat_plan("").is_empty());
        assert_eq!(repeat_urls(&urls, "").0, urls);
    }

    /// **RECONCILIATION: a repeated site is still ONE site in the denominator.**
    ///
    /// Found by running the change, not by a gate. The first live sweep under [`repeat_urls`]
    /// rendered a two-site corpus and printed **`sites 4`** — the fixed-denominator rule, which is
    /// cause #1 in the certification design's list of historically flattering numbers, broken by the
    /// tick that was fixing the numerator. The sweep prints its certificate from the rows it just
    /// built; the reader prints one from the file. **Both must reach the same denominator**, so both
    /// go through `collapse_repeats` and this test is what keeps them there.
    #[test]
    fn repeats_do_not_inflate_the_denominator_on_either_path() {
        const BODY: &str = "\
unstable.invalid\t0.7\t0.400398\t0\t0\t0\t0\t500\t
unstable.invalid\t0.7\t0.402000\t0\t0\t0\t0\t500\t
unstable.invalid\t0.7\t0.047800\t0\t0\t0\t0\t479\t
stable.invalid\t1.0\t0.637124\t0\t0\t0\t0\t598\t
";
        let p = rows_file("denominator", BODY);
        let from_file = rows_from_tsv(&p).expect("parse");
        // The in-process path: the same four draws as the sweep loop accumulates them, in order.
        let drawn: Vec<Fidelity> = BODY
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let mut r = Fidelity::unmeasured(f[0], Unmeasurable::Crashed);
                r.shape = f[2].parse().ok();
                r.unmeasurable = None;
                r
            })
            .collect();
        let in_memory = super::collapse_repeats(drawn);
        assert_eq!(
            (from_file.len(), in_memory.len()),
            (2, 2),
            "four draws of two sites must be TWO rows on BOTH paths — got file {} / memory {}. \
             `sites 4` for a two-site corpus is the fixed-denominator rule failing.",
            from_file.len(),
            in_memory.len()
        );
        for (a, b) in from_file.iter().zip(in_memory.iter()) {
            assert_eq!(a.name, b.name, "the two paths disagree about site ORDER");
            assert_eq!(
                a.shape, b.shape,
                "the two paths disagree about {}'s score — one of them is not collapsing",
                a.name
            );
        }
        assert_eq!(
            super::certificate(&from_file).sites,
            2,
            "the certificate's own `sites` term counted the repeats"
        );
    }

    /// The plan's keys come from `site_name`, and so do the sweep's row names. If those two ever
    /// diverge the plan matches nothing, the sweep repeats nobody, and the failure reads as "no site
    /// is unstable" — the answer that requires no work.
    #[test]
    fn the_plan_keys_the_same_way_the_sweep_names_its_rows() {
        for (url, want) in [
            ("https://keirin.jp/", "keirin.jp"),
            ("http://www.ikea.com/gb/en/", "www.ikea.com"),
            ("https://a.example", "a.example"),
        ] {
            assert_eq!(super::site_name(url), want, "site_name({url})");
        }
    }
}

/// # The chunked sweep's spawn-loop arithmetic — the defect that invalidated three sweeps
///
/// These tests do not spawn processes. They simulate the **one number** that broke t820, t821 and the
/// aborted t824 run: how many times a chunk may be re-spawned, against how many times it deliberately
/// exits. A chunk child exits once per site that spends its own budget, so the loop's budget must
/// scale with the bucket — and the old `CHUNK_ROUNDS = 4` did not.
#[cfg(test)]
mod chunk_spawn_budget {
    use super::{chunk_round_budget, CHUNK_STALL_LIMIT};

    /// Replay the parent's spawn-loop against a child that runs `n` sites and then exits, where
    /// `slow(i)` says whether site `i` will spend its budget (and therefore kill the child after
    /// writing its own row). Returns `(rows_written, sites_never_run)`.
    ///
    /// This is the loop in `main.rs` reduced to its bookkeeping: a round runs the remaining sites in
    /// order, stops at the first slow one (having recorded it), and the parent re-spawns.
    fn simulate(bucket: usize, budget: usize, slow: impl Fn(usize) -> bool) -> (usize, usize) {
        let mut todo: Vec<usize> = (0..bucket).collect();
        let mut done = 0usize;
        let (mut round, mut stalled) = (0usize, 0usize);
        while !todo.is_empty() && round < budget && stalled < CHUNK_STALL_LIMIT {
            round += 1;
            let before = todo.len();
            // The child works through the list and dies at the first slow site — AFTER writing it.
            let mut ran = 0usize;
            for &s in &todo {
                ran += 1;
                if slow(s) {
                    break;
                }
            }
            done += ran;
            todo.drain(..ran);
            if todo.len() == before {
                stalled += 1;
            } else {
                stalled = 0;
            }
        }
        (done, todo.len())
    }

    /// ⚠⚠⚠ **THE REGRESSION TEST FOR THE ACTUAL DEFECT.** 100 sites, every eighth one slow — which is
    /// roughly the real corpus (the t812 sweep booked 4 `timeout-150s` rows plus the sites that spend
    /// their budget without being classified). With a scaling budget every site runs. With the old
    /// constant 4 it does not, and the assertion below states exactly how badly.
    #[test]
    fn a_bucket_with_many_slow_sites_still_runs_every_site() {
        let bucket = 100;
        let slow = |i: usize| i % 8 == 7;

        let (done, never) = simulate(bucket, chunk_round_budget(bucket), slow);
        assert_eq!(
            (done, never),
            (bucket, 0),
            "every site in the bucket must get a row: a chunk child exits DELIBERATELY once per slow \
             site, and absorbing those exits is the entire job of the spawn loop"
        );

        // The old cap, stated as an assertion rather than as prose, so the size of the defect is on
        // the record and cannot be re-introduced as "probably fine".
        let (old_done, old_never) = simulate(bucket, 4, slow);
        assert!(
            old_never > 60,
            "the constant cap of 4 rounds left {old_never} of {bucket} sites unrun (ran {old_done}) — \
             and every one of them was filed `crashed`, a Bar-0 event. This assertion exists so the \
             constant cannot come back looking harmless"
        );
    }

    /// The budget must be a function of the bucket, not a constant. Stated directly, because the
    /// simulation above could be satisfied by any large enough number and the RULE is what matters.
    #[test]
    fn the_round_budget_scales_with_the_bucket() {
        for n in [1usize, 10, 100, 1000] {
            assert!(
                chunk_round_budget(n) >= n,
                "a bucket of {n} sites must be able to absorb one deliberate exit PER SITE — the \
                 pathological case is every site timing out, and it is not knowable in advance which \
                 will. Got {}",
                chunk_round_budget(n)
            );
        }
    }

    /// The other bound: the loop must still stop. A child that dies producing NOTHING is failing to
    /// start, and no amount of budget fixes that — so the stall counter, not the ceiling, is what
    /// terminates a genuinely dead chunk. Without this the scaled budget would be a 1000-round spin.
    #[test]
    fn a_chunk_that_produces_nothing_stops_after_the_stall_limit() {
        // `slow(_) = true` on the FIRST site with zero rows written is modelled by a child that runs
        // no sites at all: it can never drain `todo`.
        let bucket = 100;
        let mut todo = bucket;
        let (mut round, mut stalled) = (0usize, 0usize);
        while todo > 0 && round < chunk_round_budget(bucket) && stalled < CHUNK_STALL_LIMIT {
            round += 1;
            stalled += 1; // no progress, ever
        }
        assert_eq!(
            round, CHUNK_STALL_LIMIT,
            "a chunk producing no rows must stop after {CHUNK_STALL_LIMIT} consecutive dead rounds, \
             not run out the scaled ceiling"
        );
    }

    /// A `never-ran` row must not be readable as a `crashed` one. They are different events — an
    /// instrument budget versus a Bar-0 engine fault — and for three sweeps they shared a string.
    #[test]
    fn never_ran_is_not_crashed() {
        use super::Unmeasurable;
        assert_ne!(Unmeasurable::NeverRan.tag(), Unmeasurable::Crashed.tag());
        assert_eq!(Unmeasurable::NeverRan.tag(), "never-ran");
        assert!(
            matches!(
                Unmeasurable::from_tag("never-ran"),
                Some(Unmeasurable::NeverRan)
            ),
            "the tag must round-trip: a chunked sweep reads its own reasons back across the process \
             boundary, and a reason that does not parse silently becomes a different row"
        );
    }
}
