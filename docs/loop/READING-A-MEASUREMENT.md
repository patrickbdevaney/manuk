# Reading a measurement

**This file exists because the loop re-derived a rule it had already banked.** t1511 and t1512 paid
for *"a `--jobs 2` sweep row is not evidence about a single site"* with a false regression they had to
chase, a false gain they nearly kept, and a false lost score — and the rule was already in
`CONSTELLATION.tsv`, marked `works`, with a gate name, added at constitution check #106 (tick 1128).
**384 ticks banked, and it did not reach the tick that needed it.**

Surface audit #93 named the mechanism: `CONSTELLATION.tsv` is indexed by CAPABILITY and consulted to
decide what to BUILD. Every rule about *reading the loop's own output* is filed in a map you can only
search by feature name. **A rule is only as good as the moment it is READ.**

So: this file is indexed by SITUATION. Each rule says what you are looking at, what is wrong with the
obvious reading, and where it was paid for.

---

## You are looking at a per-site number from a `--jobs N` sweep

**It is not evidence about that site.** A parallel sweep starves whichever side loses the race, and
the loss is not symmetric noise — it lands on one engine or the other.

* Banked at constitution check #106 (t1128): five readings across the t1121/t1127 sweeps failed to
  reproduce on either binary in the same hour.
* Re-paid at t1511-1512: `mayatoys.in` −1.7 (re-reads its banked 87.3 **three times** solo);
  `a1.ro` +6.2 (its own band is 56.2/62.5 on ONE binary); `beb88run.xyz` LOST SCORE with the row
  itself blaming `oracle-timeout-150s` (93.7 solo).

> **RE-RUN THE SITE ALONE before quoting any sweep delta.** What a `--jobs` sweep IS good for: the
> DENOMINATOR, and the membership diff — which sites crossed a bar, in both directions.

## You are looking at a delta between two banked rows files

**Check they came from the same instrument, and do not trust the fingerprint to tell you.** The
`instrument` column covers *the probes' own TEXT*. It does **not** cover the browser flags: t1514
changed the oracle's command line so profoundly that two refusal cohorts stopped existing, and the
fingerprint read `d3da5acb` on both sides (t1514, t1515).

> Every rows file banked before t1514 was taken against a reference that could not load an ES module
> or a self-hosted webfont.

## You are looking at a coverage percentage

**Ask what the denominator was.** Coverage is *"of the elements Chrome renders, the fraction we
render"* — so when the reference fails to build the page, near-perfect coverage means the two engines
agree about a page **neither of them built**. `www.datacareservices.com` read **98.7% against 533
elements**; with the reference fixed it reads **2.4%** of a page ~20× larger (t1515).

> A staleness that FLATTERS is not a smaller bug than one that breaks (t1425-1428).

## You are looking at a `font-resolution:` cluster

**The label names a subsystem and reads as an accusation, and twice the arbitration landed on the
oracle.** t1369 (`anaheim`) and t1510 (`fira_sansbook`) both ended with OUR number matching the font
file. t1513 found the cause — the oracle rendered from `file://`, where a self-hosted face is
CORS-blocked — and t1514 fixed it.

> **Arbitrate against the FONT FILE, never against the reference.** Generally: *the arbitration is
> owed to the ARTEFACT* — a font to the font file, a layout to the spec (t1433), a redirect to what
> the document says (t1504). The oracle proposes; it does not adjudicate.

## You are looking at a RED perf floor

**Ask for a negative control before believing it.** t1512's wall read `F2 8.98x` against a 7.5 floor;
`bench_page` times `layout_document` and never calls `node_rects`, so a `node_rects`-only change
**cannot** move it — and it moved it +9%. Six settled-box runs read 6.04–7.00x.

> F2's margin is smaller than this machine's noise. A floor a negative control can cross will
> eventually fail an innocent tick and pass a guilty one (wall-audit #60).

## You are looking at a shortfall in the exit certificate

**Read the HEADROOM, not the size of the hole.** `Cert::shortfalls` ranks by how many sites a term
could contribute *if solved completely*, and subtracts the refusals nobody is allowed to fix. A term
whose headroom is below its gap says so on its own line. Before t1506 the list was emitted in the
order it was coded and a standing mandate ranked the scorability term first on that reading; measured,
it is fourth of six.
