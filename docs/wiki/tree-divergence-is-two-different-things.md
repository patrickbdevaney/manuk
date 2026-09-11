# `tree-divergence` is two different things

*Tick 1497. The largest evidenced term splits, and the two halves need different fixes.*

## Why this term

The compass, now measured twice (t1485, t1496), ranks the refusals on the 200-site corpus:

```text
  ORIGIN 58/60  ·  METHOD 23/17  ·  ENGINE 6/5
```

`METHOD` is ~3× `ENGINE` and is the largest term anyone can act on. Its biggest remaining members are
`tree-divergence` (7 → 5) and `probe-blocked` (8 → 4, already halved by t1486/t1487).

## The instrument already separates them, and nobody had read the line

`TREE ALIGNMENT` prints for exactly these rows — added at t783 because *"a `tree-divergence` row says
the two path spaces do not line up and deliberately does NOT say why; the two candidates cost a
subsystem each and have nothing in common."* Read across all five:

```text
  site                        oracle  ours  exact  tag-multiset  depth   verdict
  www.villaggioposeidone.it     461    645     2    336 (73%)      1     INDEX SHIFT
  dashboard.twitch.tv            11     59     2      8 (73%)      1     INDEX SHIFT
  tracker.shadowfax.in           18     48     1      1  (6%)      2     MIXED
  experiencia.pichincha.com      13     22     2      2 (15%)      1     MIXED
  sports.yahoo.com                —      —     —      —            —     (no line this run)
```

**They are two populations, not one.**

### Half is the proxy's own cohort, already named in its own doc

`tracker.shadowfax.in` (oracle 18) and `experiencia.pichincha.com` (oracle 13) have oracles that built
**thirteen and eighteen elements**. Those are shells. `experiencia.pichincha.com` is *in the proxy's
own table* — `one_origin_worth_trying`'s doc comment lists it at **53 snapshot vs 567 live**.

They are filed `tree-divergence` rather than `shell-only` only because our side's count is modest too,
so the classifier reads "comparable sizes, thin overlap". The one-origin proxy is exactly the fix, and
its acceptance test (`renders_agree`) already makes widening safe.

⚠ **But the trigger is a FLOOR on the ORACLE's count**, and these sit just above it. *A sufficient
condition used as a necessary one* — which is the same defect t903 fixed once already when the trigger
demanded `type="module"`. The floor cannot see a thin overlap, because overlap is only known at
compare time, after the reference has been chosen.

### The other half is a KEY problem, and the proxy would not touch it

`villaggioposeidone.it`: **461 oracle paths, 336 of them (73%) present in ours as a tag-path multiset,
and exactly 2 matching as keys.** Both engines built the page. One index near the root differs, and
every `:nth-of-type` below it shifts — 459 elements reported MISSING on a site that rendered.

```text
  Chrome's key:  body:nth-of-type(1)/div:nth-of-type(1)/footer:nth-of-type(1)
  first differing depth: 1        ours 645 elements against the oracle's 461
```

This is t550's finding one component over — *"ONE differing class on `<body>` invalidates every
descendant key"* — with the index rather than the class doing the invalidating. **It is an instrument
defect, not a rendering one**, and on a site whose structural score reads **0.4%**.

⚠ **Foster parenting was the obvious suspect and is refuted.** `html5ever` logs *"foster parenting not
implemented"* elsewhere in this corpus, and misnested table content is exactly the kind of thing that
shifts a root-level index. **Zero** of these five sites emit that warning.

## What each half needs

1. **The proxy cohort** — widen the trigger from "the oracle built almost nothing" to "the oracle built
   almost nothing **OR the two trees barely overlap**". The second is only knowable at compare time, so
   the proxy has to be retriable from there rather than chosen before the reference is captured.
2. **The index shift** — find the depth-1 element the two engines disagree about. 645 against 461 says
   we build ~184 more; the tag multiset says three quarters of the oracle's tree is present in ours
   under a different index. Neither the proxy nor any rendering fix touches this.

⚠ These are ranked in that order because the first has a built mechanism and an acceptance test, and
the second does not yet have a named mechanism at all.
