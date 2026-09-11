# A work order ranked by headroom is only as good as its definition of "reachable"

**t1507, and it corrects t1506 — one tick old.** t1506 taught the Phase-0 certificate to rank its
shortfalls by **headroom**, the most sites a term could contribute if solved completely, and to say
so when a term cannot close its gap. Then it was run on the first real 200-site corpus, and the
ranking it produced was wrong in two nameable ways.

> ⭐⭐⭐ **Both defects were found by USE, in the first five minutes of consuming the instrument, and
> neither would have been found by reading it.** t1419's rule, fourth instance: *instruments are
> validated by consumption, not by inspection.*

## Defect 1 — an unscored site is not addressable by the term it fails

`certificate` skips an unmeasurable row **before** it reaches the jarring loop, so an unscored site
is counted clean on nothing. Its rows therefore sit inside all four jarring holes *and* the shape
hole at once. t1506 ranked jarring terms on `sites − clean[i]`, which counts that one shared
blockage once per term:

```text
  sites - clean     reading-order 147 > overlap 137 > h-overflow 122 > dead-target 103 > shape 54
  scored - clean    shape 54 > reading-order 52 > overlap 42 > h-overflow 27 > dead-target 8
```

On `SWEEP-t1406-rows.tsv` the naive model put `reading-order` at the top of the work order with a
hole of 137 — **when only 52 of that is reachable by working reading-order at all.** The other 85 are
pages we cannot score, and no amount of reading-order work touches them.

**The unscored cohort is not a competing term; it is the shared PREREQUISITE of all five others.** A
ranking that lets it be double-counted into every one of them ranks by how much of the same blockage
each term happens to contain.

## Defect 2 — a site nobody is ALLOWED to fix is not work

With defect 1 corrected the unscored term led with **95**, which is the number the standing observer
mandate quotes. But of those 95, **60 are bot-wall, unreachable, HTTP or empty-body** — the ORIGIN's
refusal, ruled out of scope by `DAILY-DRIVER-CERTIFICATION.md` §3, not convertible by any tick.

The loop has known this since t1485 and has applied it **by hand at every sweep**: the
`ORIGIN 58 / METHOD 23 / ENGINE 6` histograms are literally awk over the tag column. **The
certificate itself had never known about it.** `Unmeasurable::is_origins` puts the rule where the
ranking happens, and the line now does the subtraction in front of the reader:

```text
  · 95 of 200 sites UNSCORED … — 60 of them refused BY THE ORIGIN (bot wall / dead host / HTTP /
    empty), out of scope per DAILY-DRIVER-CERTIFICATION.md §3 and not convertible by any tick, so
    only 35 of this cohort is work — 35×bot-wall-403, 16×unreachable, 9×timeout-150s, …
```

⚠ **`Timeout` is filed as OURS deliberately** — t1409 established that a watchdog firing on a clock
cannot name a cause, and the engine is the default suspect for its own clock. `OracleTimeout` is the
instrument's debt and is also kept in the addressable column, so it stays visible rather than
disappearing into the origin's.

## The corrected work order, on the real corpus

```text
  · shape ≥0.75 on 25.5% (bar 95%) — hole 139   <== at most 54; 85 of the remainder are UNSCORED
  · reading-order clean on 26.5%  — hole 137    <== at most 52; 85 …
  · overlap clean on 31.5%        — hole 127    <== at most 42; 85 …
  · 95 of 200 UNSCORED — 60 the origin's, so only 35 is work   <== at most 35
  · h-overflow clean on 39.0%     — hole 112    <== at most 27; 85 …
  · dead-target clean on 48.5%    — hole 93     <== at most 8;  85 …
```

⭐⭐⭐ **The scorability term is FOURTH.** The standing mandate ranks it first — *"the BINDING
constraint is the SCORABILITY/FUNCTION ceiling … NOT layout placement"* — and that is now refuted by
the instrument on its own corpus rather than by an argument. It also matches t1506's independent
in-scope arithmetic exactly, which did the same subtraction in awk and got shape 70 > unscored 14 on
141 in-scope sites. **Two derivations, two corpora slices, same order.**

⚠ **And no single term closes the gap — every one of the six carries the verdict.** That is the
honest shape of the problem: the certificate is met by a conjunction of several terms or not at all.

## ⚠ Two more defects the fixtures caught, both in the instrument's own voice

* **The unscored term was being told it was waiting on itself.** `headroom_note` appended *"N of the
  remainder are sites that are UNSCORED and must be SCORED first"* to every term including the
  scorability term — a sentence that cannot be acted on. The prerequisite is now a parameter, zero
  for the term that IS the prerequisite.
* **The first fixture could not reproduce the shape it was about.** `row(.., None, ..)` gives a row
  with no shape *and no `unmeasurable` reason*, which still reaches the jarring loop and is counted
  clean on all four invariants: `clean = [173, 158, 148, 192]` against the real corpus's
  `[78, 63, 53, 97]`. ⭐⭐ *An unscored row and a REFUSED row are different states, and only the
  second is the one the corpus is made of.* `refused()` and `refused_ours()` now exist so a fixture
  cannot make that mistake silently.

## Where it is

`tests/wpt/src/fidelity.rs` — `Unmeasurable::is_origins`, `Cert::unscored_origin`,
`Cert::headroom_clean`, and `headroom_note`'s `prereq` parameter.

Gate: `an_unscored_site_is_not_addressable_by_the_term_it_fails` reproduces `SWEEP-t1406`'s
certificate exactly (200 sites, 105 scored, shape_ok 51, clean `[78,63,53,97]`, 60 origin refusals
and 35 ours) and pins the scorability term at **rank 4**. Red under twelve mutations, including
`headroom_clean` reverted to `sites − clean`, `is_origins` returning `false`, `is_origins` swallowing
`RenderFailed` as well, and the unscored term told it waits on itself.

⚠ **No engine behaviour changed and no site moved.** ⚠ **RESIDUE:** a row with a valid but
sub-sample shape (`CERT_MIN_SHAPE_SAMPLE`) is not `scored` and yet still reaches the jarring loop, so
`clean[i]` can exceed `scored`. That is zero rows on today's corpus and is handled with
`saturating_sub` rather than by changing the certificate's counting, which would move banked marks.
