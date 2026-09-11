# The font file settled it: our number was right and the reference was using a fallback

**t1510.** t1509 ended with a quantified divergence and an explicit refusal to claim it:
`{fira_sansbook/14/129}` in the oracle against `{fira_sansbook/14/140}` here, 136 times on
`www.jatekshop.eu`, at a constant ratio across three sizes — and *"the arbitration is owed to the
FONT FILE, not the oracle."* This is that arbitration.

## The file

`https://www.jatekshop.eu/scripts/ext/fonts/firasans-book-webfont.ttf`, parsed directly —
`head.unitsPerEm`, the format-4 `cmap`, and `hmtx` advances for the 20 characters of the probe string
`Hamburgefonstiv 0123`:

```text
  upem = 2048        hhea asc 2150 desc -757 gap 0
  probe total        20509 units
  at 12px            120.17px      (oracle says 111, we say 120)
  at 14px            140.20px      (oracle says 129, we say 140)
```

⭐⭐⭐ **The file is OURS, to two decimal places, at both sizes.**

## And the control names what 129 actually is

A local HTTP-served probe in the same headless Chrome 145 the oracle uses, with the same `.woff2`
actually loaded (⚠ over **http://**, not `file://` — a `file://` fixture cannot fetch a cross-origin
webfont and Chrome silently renders the fallback, t1367-1374):

```text
  document.fonts.status            "loaded"
  document.fonts.check('14px fira_sansbook')   true
  canvas advance, fira_sansbook     140          <- the file, and ours
  canvas advance, a family that does not exist  129   <- the oracle's number, exactly
  layout width of the same string    139.72
```

> **129 is not a different reading of Fira Sans Book. It is Chrome's measurement for a family it does
> not have.** The oracle rendered `www.jatekshop.eu` with a fallback face; our engine had the real
> one (`WEBFONTS: 2 of 2 @font-face families delivered a usable face`).

Wider text wraps sooner, a wrapped box is taller, a taller box breaks the row it shared — so
**t1508's reading-order inversion on this site is the oracle's, not ours**, and so are the 136
`font-resolution` hits and an unknown share of its shape deficit.

## ⚠ The obvious fix was tried and REFUSED

The probe captures at `DOMContentLoaded`, at `load`, and on a 3s timeout, and never waits for
`document.fonts.ready` — while our own engine has waited for webfonts since t1490. That asymmetry is
the same shape as t1504/t1505 (the instrument following one kind of redirect and not the other), and
it looked like the answer.

**It changed nothing.** With `document.fonts.ready.then(capture)` added the oracle still reports
111/129/185. So it is **not a swap-timing race**, the cause is still unestablished, and the change was
reverted rather than kept on principle: it moves the probe's fingerprint — which is the probes' own
text, so every banked row becomes incomparable — and it bought no measured improvement.

## The second time, and that is what makes it a rule

**t1369, `www.a11yproject.com`:** `{anaheim/20/201}` in Chrome against `{anaheim/20/181}` here. **181
is what `Anaheim-Regular.woff2` says** (upem 2048, the probe's 20 glyphs summing to 18540 units), and
the same page load reported 181 for 12 elements and 201 for 11 — *the reference disagreed with
itself*.

Two independent arbitrations, 141 ticks apart, both ending on the oracle. The cluster label
`font-resolution:` names a subsystem, and **a named subsystem is read as an accusation** (t1415,
t1508) — so it now carries the instruction that makes it safe to read:

```text
  font-resolution: fira_sansbook/14/129 vs fira_sansbook/14/140   (<span>)
     [UNATTRIBUTED — arbitrate against the FONT FILE, not the oracle: twice (t1369 anaheim,
      t1510 fira_sansbook) the file agreed with US and the ORACLE was using a fallback]
```

Nothing is hidden, nothing is filtered, and no score moves — the cluster is ranked and counted
exactly as before. What changed is that the next tick cannot read it as a verdict about the engine.

## ⚠⚠ This is a finding about I5, and it is recorded in the constitution check

> *I5. The differential oracle is the discovery engine … maintained as first-class infrastructure.*

I5 does not say the oracle is right; it says the oracle is what the loop discovers work from. **A
discovery engine that is wrong in a systematic direction discovers work that does not exist.** The
invariant is not bent — the instrument it mandates has a named defect, and the constitution's own
words are the authority for fixing it. Check #146 carries it.

⭐⭐ **The general rule, which is nowhere in the constitution and belongs there:** *the arbitration is
owed to the ARTEFACT, never to the reference.* A font divergence is settled by the font file, a
layout divergence by the spec (t1433), a redirect by what the document says (t1504). **The oracle
proposes; it does not adjudicate.**

## Where it is

`tests/wpt/src/oracle.rs` — `FONT_RESOLUTION_UNATTRIBUTED`, appended by `signature_of`.

Gate: the existing `a_divergence_between_two_faces_is_not_a_geometry_cause` grew two rows — the
cluster must carry its arbitration instruction, and it must not send the next tick to the oracle.
Red under three mutations: drop the clause, point it at the oracle, lose the `font-resolution:`
prefix other consumers match on.

⚠ **No engine behaviour changed and no site moved.** The probe is byte-identical to t1509's, so every
banked row stays comparable.
