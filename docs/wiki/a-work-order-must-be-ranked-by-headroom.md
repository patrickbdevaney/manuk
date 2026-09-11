# A work order that cannot say which term is *capable* of closing the gap will rank the wrong one

**t1506.** The Phase-0 exit certificate has printed `CERTIFICATE NOT MET — shortfalls, in the order
to work them:` since it was written. It printed them in the order they were **coded** — the UNSCORED
term first, always — and the phrase *"in the order to work them"* was a claim the function did not
implement.

> ⚠⚠⚠ **A standing observer mandate ranked the scorability term as THE binding constraint, and the
> loop steered by that for twenty-odd ticks.** Its words: *"The BINDING constraint is the
> SCORABILITY/FUNCTION ceiling — ~82%, 24/135 in-scope sites never yield a scored tree — NOT layout
> placement."* It is the first line of the certificate's own work order.

## The arithmetic nobody had run

The exit bar is `shape ≥ 0.75 on ≥95% of sites`, and **unscored sites count against it, not out of
it**. So for any sweep there is a hole, in sites:

```text
  hole  =  ceil(0.95 × sites)  −  shape_ok
```

and each term has a **headroom** — the most sites it could contribute if it were solved *completely*:

```text
  the UNSCORED term   sites − scored     every unreachable page reached, every refusal converted,
                                         AND every one of them landing above the floor on arrival
  the SHAPE term      scored − shape_ok  every already-scored site below the floor lifted over it
  a JARRING term      sites − clean[i]   every dirty site cleaned
```

Measured on the in-scope CrUX trend corpus (200 sites, 59 ruled out at the origin as
bot-wall/unreachable/HTTP per `DAILY-DRIVER-CERTIFICATION.md` §3 → **141 in-scope**; scores from
`SWEEP-t1496-refusal-tags.tsv`):

```text
  in-scope                141
  shape ≥ 0.75             57      the bar needs 134
  scored but below          70     <- the SHAPE term's headroom
  unscored                  14     <- the SCORABILITY term's headroom
  ─────────────────────────────
  THE HOLE                  77
```

⭐⭐⭐ **The scorability term is worth at most 14 sites against a hole of 77.** Solved perfectly — every
unreachable page reached, every refusal converted, and every one of them landing above the shape
floor the moment it arrives — it moves the certificate **less than a fifth of the way**. And the
shape term, at 70, does not close it either: **this is a gap no single term can close**, and a work
order that names only the largest is still telling a reader something false.

**The conclusion does not depend on which scorability figure is right.** Under the mandate's own
number (24 unscored of 135) the term is worth 24 against a hole in the seventies. It was never
arithmetically capable of being the binding constraint, at either measurement.

## What the instrument does now

`Cert::shortfalls` ranks by headroom, descending, with a stable sort — and every term prints **its own
hole**, plus a verdict when its headroom is below that hole:

```text
  CERTIFICATE NOT MET — shortfalls, in the order to work them:
      · reading-order clean on 44.1% of sites (bar 95%) — the hole is 18 site(s)
      · overlap clean on 55.9% of sites (bar 95%) — the hole is 14 site(s)
      · shape ≥0.75 on 50.0% of sites (bar 95%) — the hole is 16 site(s)
            <== CANNOT CLOSE THE GAP: solved COMPLETELY this term is worth at most 12 site(s) against a hole of 16
      · h-overflow clean on 67.6% of sites (bar 95%) — the hole is 10 site(s)
      · 5 of 34 sites UNSCORED … — 1×css-starved-1, 1×oracle-module-shell-1, …
            <== CANNOT CLOSE THE GAP: solved COMPLETELY this term is worth at most 5 site(s) against a hole of 16
      · dead-target clean on 85.3% of sites (bar 95%) — the hole is 4 site(s)
```

## ⚠ EACH TERM IS MEASURED AGAINST ITS OWN HOLE — and the first version was not

The first implementation compared **every** term to the SHAPE hole. That is the right question only
for the two terms that feed the shape bar: the scored-but-low sites, and the unscored ones (an
unscored site can never be `shape_ok`, so its whole cohort is shape headroom).

A jarring invariant has its own bar and its own hole, and its headroom is `sites − clean`, which is
**always at least** that hole — *a jarring term can always close its own gap*. Measured against the
shape hole instead, the verdict would have fired on a perfectly reachable term whenever the shape
hole happened to be the larger number: **a confident false statement, which is the exact class of
error this gate was written to stop, committed by the gate itself.**
`a_jarring_term_never_carries_the_cannot_close_verdict` is the row that forbids it, and mutation M7
(measure the jarring term against `shape_gap`) turns it red.

## ⚠⚠ TWO FIXTURES ARGUED WITH THEIR OWN ASSERTION TEXT, AND BOTH TIMES THE TEXT WAS RIGHT

Writing the *symmetric* half — *a term that CAN close the gap must NOT carry the warning* — failed
twice, and each failure was the fixture's arithmetic, not the code's:

* 30 passing / 60 low / 10 unscored → hole 65, shape headroom 60. **Shape cannot close it either.**
  That is not a bad fixture; it is the shape of the real problem, and it became its own test
  (`the_measured_corpus_is_a_gap_no_single_term_can_close`) instead of being tuned away.
* 5 passing / 95 low → hole 90, shape headroom 95. The control asserted the verdict *would* fire; it
  does not, because 95 ≥ 90.

⭐⭐ **A control arm you have to compute to write is a control arm that checks your arithmetic as well
as the code's.** Both times the assertion *message* — which states the numbers in prose — is what
made the contradiction visible, before the code was ever suspected.

## Where it is

`tests/wpt/src/fidelity.rs` — `Cert::shape_gap`, `clean_gap`, `headroom_shape`, `headroom_unscored`,
`headroom_note`, and the ranked `shortfalls`.

Gate: three tests in `fidelity::shape_tests`, red under eight mutations — drop the sort, reverse it,
a note that is always silent, a note that always fires, a headroom that returns the site count, a gap
that forgets its own numerator (twice), and a jarring term measured against the wrong hole.

⚠ **This changes no engine behaviour and flips no site.** It changes what the next twenty ticks are
allowed to believe about which term to work, which is the only reason it is worth a tick.
