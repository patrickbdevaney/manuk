# Retry one origin when the trees barely overlap

*Tick 1498. Two of five converted, two honestly refused, and one confirmed to be a different problem.*

## The widening

`capture_seen_all_paths` decides whether to try the one-origin reference **before** the reference is
captured, on a shell floor over the oracle's element count — *"the oracle built almost nothing."* That
floor cannot see the other shape the proxy exists for: **two engines that each build a page and barely
overlap**, which is not knowable until both trees are keyed.

t1497 measured the `tree-divergence` cohort and found half of it sitting just *above* the floor with
oracles of **13 and 18 elements** — shells by any reading, and one of them
(`experiencia.pichincha.com`) is in the floor's own comment table at *53 snapshot vs 567 live*.

*A sufficient condition used as a necessary one* — the same defect t903 fixed once already when the
trigger additionally demanded `type="module"`.

## The result, on exactly the cohort that motivated it

```text
  site                        oracle  shared   outcome
  dashboard.twitch.tv             11 -> 159   2 -> 59   CONVERTED · structural 37.1%
  experiencia.pichincha.com       13 ->  37   2 -> 12   CONVERTED · structural 32.4%
  tracker.shadowfax.in            18           1        PROXY REFUSED — kept its honest label
  sports.yahoo.com                11           3        PROXY REFUSED — kept its honest label
  www.villaggioposeidone.it      461           2        retry AGREED and shared no more — kept
```

**Two converted, two refused by the acceptance test, and one confirmed to be the index-shift half the
proxy cannot touch** — which is precisely the split t1497 predicted from the `TREE ALIGNMENT` line,
now validated by the fix rather than by reading.

## ⚠ Agreement with live is not improvement, and those are different claims

`renders_agree` vouches that the proxied render matches the LIVE page. It does **not** vouch that the
proxied tree shares any more of ours — and `villaggioposeidone.it` proves the gap: its retry was
**accepted** and came back *461 paths, 2 shared*, the same reference it already had, because that
site's divergence is in the KEY and not in the origin.

Swapping in a reference that is merely *different* would make the row's number depend on which Chrome
run produced it. The swap is therefore conditional on the overlap actually improving, and the row says
so out loud when it declines.

## ⚠ Two predicates, and neither is the other's fallback

`one_origin_worth_trying(probed, document)` answers *"is the reference a shell?"* before capture.
`one_origin_worth_retrying(probed, overlap)` answers *"do the two trees meet?"* after keying. An
**empty** reference returns `false` from the second: a zero-path oracle is the floor's own case and has
already had its try, and retrying there would double the Chrome bill for every unreachable origin in
the corpus. The unit test pins that row specifically.

## ⚠ A process defect, recorded

`retry_one_origin` itself landed **in tick 1497's commit**, whose message says *"no engine change"*. It
was written while that tick's wall was running, and `tick.sh` commits the working tree. The function
was unused dead code so nothing shipped behaved differently, but the commit record was wrong for one
tick. *Never edit the tree while the wall runs* (t1183–1188) — known, and broken here anyway.
