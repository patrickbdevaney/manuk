reliability(cert): the sweep that checked this session's work put back the row it had just closed

t651 owed a follow-up in its own words — "agoda was found BY EYE, and found-by-eye means the
population was never swept." Re-ran HEAD-20 with t650-652 in place: the first sweep to exercise the
durable ledger, the blank-render guard and the script `load` event on real sites at once.

THE GUARD FIRED WHERE IT WAS BUILT TO. www.agoda.com moved `verdict ok` -> `render-failed` and scored
went 5 -> 4, exactly as t651 predicted. Nothing else in the population tripped it — the sweep is the
check that it was a guard and not a sledgehammer.

AND THE SWEEP PUT BACK AN UNEXPLAINED ROW, one tick after I drove that count to zero:
unexplained unscored t611 4 -> t626 1 -> t650 0 -> t653 1.

www.ebay.com came back `probed 25 · common 4`. The ORACLE built the page — no shell — and WE rendered
16% of it. So ShellOnly could not fire, because it only ever looked at the ORACLE's count; the sample
floor refused to score it; and the row went out with nothing to say.

The rule could name the oracle failing us and had no way to name US failing. Both are "too few
elements to compare", both are refused by the same floor, and only one had a word. The cause is that
the check was asked TOO EARLY — at the producer, on `probed` alone, before the comparable count
existed in the function, so it could not ask "did we render the page?", only "did the oracle?". A
check placed where half the evidence is missing answers the half it can see and looks complete doing
it. An asymmetric rule is invisible while the asymmetry matches reality: the oracle is the thing that
usually breaks, so this passed review for 39 ticks and failed the first time WE were the broken one.

unscoreable_reason(probed, common) now decides both in one place, retiring a second implementation
along the way. The two reasons blame opposite parties, which is the point: ThinOverlap is OURS, and
reads as a coverage failure wearing an `unscored` label.

⚠ The live re-run of ebay came back `timeout-300s` — a THIRD outcome, after probe-blocked at t650 and
thin-overlap at t653. So the gate asserts against the RECORDED numbers from the sweep row, never a
re-fetch: a gate whose expected value comes from today's network is a gate that measures the network.

t652 MOVED NO SITE, which is what I said it would do — recorded rather than quietly hoped away.
Shape >=0.75 is 0 sites at t611, t626, t650 AND t653: four measurements, 42 ticks, no movement, and
the instrument has been rebuilt around it three times this session without it budging.

Gates: G_UNSCOREABLE_REASON, RED-proven by mutation — removing the thin-overlap arm returns None for
ebay's exact recorded numbers, the precise pre-fix silence. Every case in the gate is a real row.

WIKI: docs/wiki/conformance-and-oracles.md — "the rule could name the oracle failing us, and had no
word for us failing".
