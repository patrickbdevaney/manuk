# The trace printed the rects and withheld the one field that discriminates the causes

**t1509.** t1508 ended holding a pair of rects — `[91x64]` in Chrome, `[91x79]` here — and no way to
say why. `ro_trace` printed position and `display` and not the **font**.

## The rule was already in the file, and it had one implementation too few

`tests/wpt/src/oracle.rs` carries t562's rule verbatim, written for geometry instances:

> *"A geometry instance must NAME THE FONT on both sides, or a 2px height divergence stays
> unattributable. `martinfowler.com` reported `[74×16] vs [76×18]` and the 2px could equally be a
> different face, a different used size, or a different line-box rule — three different fixes,
> indistinguishable in a rect."*

`fontsuffix` exists for exactly that, absence semantics and all (`{/0}` would read like a measured
zero, so an absent font prints as nothing). **`ro_trace` never called it.** One rule, two
implementations, and the second had no consumer to notice it was incomplete — t1403, again.

## It named the mechanism on the first run

```text
  chrome  img [906 811 50 50] static/block  {fira_sansbook/14/129}   div [966 804 91 64] static/flex  {fira_sansbook/14/129}
  ours    img [906 811 50 50] static/block  {fira_sansbook/14/140}   div [966 797 91 79] static/flex  {fira_sansbook/14/140}
```

The third field is the measured advance of `'Hamburgefonstiv 0123'` in that computed font. Same
declared family, same used size, and **our text measures 8.5% wider**. Run whole, `www.jatekshop.eu`
reports it 136 times across `<li>`, `<div>` and `<span>`, at two sizes, with a constant ratio:

```text
  fira_sansbook/12/111  vs  fira_sansbook/12/120     120/111 = 1.081    79 hits on <li>, 29 on <div>
  fira_sansbook/14/129  vs  fira_sansbook/14/140     140/129 = 1.085    28 hits on <span>
```

Wider text wraps sooner, a wrapped box is taller, and a taller box breaks the row it shared — which
is the inversion t1508 was looking at. **The reading-order symptom is two layers downstream of a text
measurement.**

⚠ **And we DID load the webfont**: the same run prints `WEBFONTS: 2 of 2 @font-face families
delivered a usable face`. This is not the fallback-metrics failure of t1342-1343; both engines have a
face for the declared family and measure it differently.

## ⚠⚠ NOT YET CLAIMED AS OUR BUG — t1369 is the reason

t1369 investigated this exact cluster on `www.a11yproject.com` and the answer was the opposite of the
obvious one: **our number was what the font file said**, verified against `Anaheim-Regular.woff2`
(upem 2048, the probe's 20 glyphs summing to 18540 units), and **Chrome's reference disagreed with
itself** — 12 elements at `{anaheim/20/181}` and 11 at `{anaheim/20/201}` on one page load.

Two things here are unlike a11yproject, and they are why this is worth pursuing rather than filing:

* **Neither engine disagrees with itself.** Every Chrome reading is 111/129 and every one of ours is
  120/140, across 136 elements.
* **The ratio is constant across two sizes** (1.081 and 1.085), which is a face difference and not a
  rounding or a fallback event.

⚠ The arbitration is still owed **to the font file**, not to the oracle (t1367-1374). Until that run,
the honest statement is *"two engines with a usable face for one declared family measure it 8.3%
apart, consistently"*, and nothing more.

## ⚠⚠ AN INSTRUMENT HAZARD FOUND ON THE WAY, AND IT IS t1416 IN A NEW PLACE

`www.jatekshop.eu` was in t1507's 34-site slice. **Its 136-hit font cluster does not appear in that
run's cluster list at all**, and appears immediately when the site is run alone — same binary, same
site. The chunk-wide list ranks by **site count first** and truncates, so:

```text
  5 site(s) ·   8 hit(s)   display: block → flex   (<div>)     <- printed
  1 site(s) · 136 hit(s)   font-resolution: …                  <- cut
```

⭐⭐ **A mechanism that is 136 hits on ONE site is invisible beside one that is 8 hits on five.** That
is t1416's rule — *check concentration before ranking by count* — appearing in the cluster ranker
rather than in a WPT area table. Recorded here; not changed in this tick, because reordering the
ranker moves what every future sweep reads first and that deserves its own tick with its own control.

## Where it is

`tests/wpt/src/oracle.rs` — `instance_line`, extracted from `ro_trace`'s closure so the wiring can be
gated at all.

Gate: `a_trace_instance_names_its_font` — the signature is printed, an ABSENT font prints as absence
rather than `{/0}`, and an unreported position prints `?` rather than an invented `static`. Red under
three mutations: drop the `fontsuffix` call, fabricate `{/0}`, drop the position/display suffix.

⚠ **No engine behaviour changed and no site moved.**
