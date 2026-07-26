capability(js): the drain's ceiling was a task COUNT, and the harm it defends against is a CLOCK

TICK SHAPE: capability (Bar 0 strengthened). No ratchet floor moved; no capability traded.

t609 closed the network half of mangago.me's 85s load and named what was left, measured rather than
guessed: ~30s draining the JS event loop to its 20,000-task ceiling. This tick went at that. It is
the trap-adjacent one — PART VII's standing warning is "fast because we never ran the script", and a
budget on script execution is exactly the shape that lie takes — so the order was measurement first,
then a gate whose load-bearing claim is the NEGATIVE.

MEASURED FIRST — the per-drain distribution, clock bound disabled:

  en.wikipedia.org       1 task,      4ms      <- a page that converges
  mangago.me         20000 tasks, 30216ms      <- and it did this FIVE times in one load

Four orders of magnitude. MAX_TASKS_PER_DRAIN has capped run_deferred since Bar 0 was written and its
own comment states the purpose in wall-clock terms — "the alternative is a frozen tab" — but it
enforces that with a COUNT, and the two are related by no fixed factor. One declared policy handed
wikipedia 4ms of grace and mangago half a minute of it, five times over, for reasons having nothing
to do with what either had cost the user. WHEN A LIMIT'S STATED PURPOSE AND ITS UNIT DISAGREE, THE
UNIT WINS SILENTLY, and the disagreement stays invisible until a page arrives that separates them.

AND THE SECOND HALF HAD NO BOUND AT ALL. run_with_fetcher — which run() delegates to and which
dom_bindings drives — had neither a count ceiling nor a clock. The exact runaway Bar 0 forbids ran
forever there, and its did_io arm is actively hostile: "a delivered result may have scheduled more
work" continues past the task check unconditionally, so a page that re-fetches on every delivery
loops indefinitely. One rule, two implementations, only one enforcing it.

RESULT (two runs per arm, quiet box, mangago.me):

  budget=0 (shipped)    did not finish in 400s   ·   114874ms, visual 0.2%
  budget=5000            35743ms, visual 0.9%    ·    36863ms, visual 0.2%

Latency 3-11x and stable; visual UNCHANGED. That pairing is exactly the claim this tick makes.

WHY THIS IS NOT THE "fast because we never ran the script" TRAP, argued from what was already true
rather than from a fidelity number: these pages were ALREADY BEING CUT — the 20,000-task ceiling
terminated them, just 30 seconds per drain later. The capability outcome is identical by
construction, which the unchanged visual score independently shows; what changed is how much of the
user's clock a page spends proving it will never converge. The clock and the count are complementary
and both are kept: a tight loop of cheap tasks trips the count first, a handful of expensive ones
trips the clock first.

A NUMBER I HAD WRITTEN DOWN AND HAD TO RETRACT: a first A/B showed theguardian.com 22.9% -> 52.4%
visual, and the entry, wiki page and pattern were drafted around it. It does not reproduce — guardian
does not reliably finish in EITHER arm, so nothing can be differenced across them. The first reading
was taken while release LTO shared the box. Booked three times this session in total: the first run
of an A/B pays for the build.

AND THE THIRD BOOKING WAS THE WALL ITSELF. I called the WALL mark bricked and formally asked the
observer to re-baseline it, on four consistent readings (544/752/714/692s against 85s). All four were
the same artifact: engine/js is upstream of the release manuk-wpt, so this tick's own edit forced a
~10-minute release LTO relink inside the gate phase. The receipt said so and I read past it —
build_seconds 33, unattributed_seconds 692. Disproof cost one command: a warm no-op
`cargo build -p manuk-wpt --release` returned in 0.45s, and the same verify.sh on the same tree then
ran in 64s, under the 245s ceiling I had just asked to have raised. A false regression report aimed
at a RATCHET MARK is worse than one aimed at code, because the remedy it requests is permanently
loosening the thing that catches regressions. Before asking for a mark to move, prove the reading is
a property of the ENGINE and not of the BUILD.

RED-PROVEN, three ways — vary the MECHANISM, not the threshold:
  · delete the clock check from run_with_fetcher   -> RED, the runaway is unbounded again
  · make the counterfactual arm bounded too        -> RED, the A/B shows no difference
  · tighten DEFAULT_MAX_DRAIN_MS to 1ms            -> RED, THE PROMISE fails: the converging page no
    longer runs its 5,000 tasks. This is the assertion standing between the budget and the trap, and
    a later tick that tightens the constant to make a load number look better hits it first.
A vacuity guard fires ahead of both if the runaway never spun. (G_DRAIN_BUDGET's first draft passed
with its subject deleted — a gate can be green because a SECOND mechanism produces the same
observable.)

HONESTY ON THE NUMBERS: mangago.me stays UNCLAIMED as a site, for the third tick running and the same
reason — it is bot-walled, Chromium is scored against a different page than we get, and its visual
score is noise in both arms. What this tick claims is the DRAIN, measured directly, plus the latency
pairing above. Not the site.

Gates: +1 G_DRAIN_BUDGET (engine/page/tests/g_drain_budget.rs) — a 5,000-task CONVERGING page that
must complete every task and land every node, plus a bounded-vs-unbounded counterfactual on a runaway.
WIKI: docs/wiki/js-engine.md — "the event-loop drain is bounded by the clock, not only by a count".
PATTERN: docs/loop/WEB-PATTERNS.md — a self-rescheduling timer bounded by a count that is not the harm.

RESIDUE: 5s is a FIRST constant, set from a distribution whose converging end (4ms) and runaway end
(30,000ms) are four orders apart — safe rather than tuned, and the space between them is unmeasured.
The honest follow-up is the per-drain distribution across the corpus-v2 HEAD slice, which would say
whether any real page's legitimate drain is anywhere near seconds. Until that exists, treat 5s as a
Bar-0 backstop and not as a scheduling policy. Also open from t609: manuk-wpt firstpaint still panics
on .expect("prefetch").
