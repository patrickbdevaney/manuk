capability(dom): `innerText` had no setter — and a getter-only accessor is a THROW, not a gap

TICK SHAPE: capability. Bar 0 untouched; no ratchet floor moved; no capability traded.

t611's sweep left a ranked residue and named the only rendering item on it: www.welt.de, 3,181 of
3,182 elements missing. Not a fidelity gap — a WHITE SCREEN on a top-1000 site, from a document
Chromium renders completely out of the same bytes. The oracle is controlled by construction here
(both engines score the SAME fetched snapshot), so a divergence that large could not be the corpus.

THE ENGINE SAID WHAT WAS WRONG, IN ONE LINE, THE FIRST TIME IT WAS ASKED:

  ERROR page.console: Failed to load website due to adblock:
                      TypeError: setting getter-only property "innerText"
  structural: 0.0% (3182 paths, 3181 missing, 1 misplaced)
  MISSING by tag: div×940  a×447  li×390  span×346  h4×103  ul×100  article×98  svg×89

`innerText` was registered `prop_guarded!(prop, c"innerText", el_get_inner_text, None)` — getter, no
setter. The sibling on the very next line, `outerText`, has had a setter the whole time. One rule,
two implementations, only one of them written: the FOURTH instance of that shape in four ticks (t591's
@supports second list, t610's run_with_fetcher, t611's fixed denominator, this).

A MISSING SETTER IS NOT A MISSING FEATURE — IT IS AN EXCEPTION, and that distinction is the tick.
Assigning to a getter-only accessor THROWS in strict mode, killing the assigning frame and everything
it was going to do. Then welt.de makes it worse than a lost assignment: its own anti-adblock check
writes through innerText, CATCHES the throw, concludes a blocker is present, and deliberately blanks
the document. We were not failing to render welt.de. welt.de was refusing to render for us — and on
the evidence it had, it was right to think something was wrong with the DOM.

MEASURED THE POPULATION BEFORE WRITING THIS UP, which moved the claim from one site to a class.
Scanning each HEAD site's HTML plus up to 12 of its external bundles for `.innerText =`:

  bbs.ruliweb.com   9      www.welt.de     2
  www.desitales2    6      www.aparat.com  1
  -> 4 of 16 scanned sites WRITE innerText

A quarter of the HEAD stratum writes through a property this engine made unwritable — and that is a
LOWER bound three ways: only 12 bundles per site, computed access (`el[p]=v`) uncounted, and five of
the twenty answered with bot-wall pages carrying none of their real scripts.

⚠ THE GATE'S HEADLINE ASSERTION WAS VACUOUS, AND THE RED PROBE IS THE ONLY REASON I KNOW IT. I wrote
`threw:false` as the load-bearing claim — the exact branch welt.de blanks itself in — put the
assignment at script top level, and it PASSED WITH THE BUG STILL PRESENT. Top-level script is SLOPPY
MODE, where assigning to a getter-only accessor silently no-ops instead of throwing. Every other
assertion went correctly red; the one the whole gate is about could not fail. Real bundles are strict
(a module body always is; every minifier emits 'use strict'), so the fix is an IIFE with 'use strict'
— not decoration, but the only thing that makes the assertion falsifiable. t610's lesson, in my own
gate, one tick later.

RED-PROVEN THREE WAYS, EACH A DIFFERENT MECHANISM:
  · restore `None` as the setter        -> RED `threw:true` — welt.de's exact state, plus 8 others
  · make the setter parse markup        -> RED `noParse:1` — an <img> appeared; innerText became
    innerHTML, a SECURITY property since innerText is where pages put untrusted text
  · drop CR/CRLF normalisation from the -> RED `crBr:0`, and it fires through the SHARED helper, so
    shared rendered-text-fragment helper   it also covers the `outerText` setter that reuses it

HONESTY: welt.de STILL DOES NOT RENDER, AND THIS TICK DOES NOT CLAIM IT DOES. With the setter in place
the innerText TypeError is gone and the adblock check advances to the next rung:

  before:  ... adblock: TypeError: setting getter-only property "innerText"
  after:   ... adblock: Error: Failed to execute packing script

That is the aljazeera pattern — one layer peels per fix — and the remaining layers are recorded rather
than implied, from the same run: `TypeError: a.addEventListener is not a function`,
`TypeError: HTMLScriptElement.supports is not a function`, and `a page module failed SyntaxError:
expected expression, got '<'` (a module request answered with HTML). What this tick claims is the
SETTER and the CLASS, measured at 4 of 16 sites; it does not claim the site.

Gate: G_INNER_TEXT_SET (engine/page/tests/g_inner_text_set.rs) — the strict-mode assignment does not
throw, replace-all semantics, rendered-text-fragment <br> splitting agreeing across LF/CRLF/CR, the
no-markup security property, and the outerText sibling still replacing the ELEMENT.
WIKI: docs/wiki/dom-semantics.md — "a getter-only accessor is a TypeError, and one site turns it into
a white screen".
PATTERN: docs/loop/WEB-PATTERNS.md — a site that detects a broken DOM write and blanks itself.

NEXT: the two named missing surfaces are cheap and both are TypeErrors of the same class —
HTMLScriptElement.supports (static feature-detect: 'classic'/'module'/'importmap') and whatever object
welt.de calls addEventListener on. The getter-only sweep also priced the rest of the class: nodeValue
(spec-settable), style and classList (both [PutForwards=], so `el.style='color:red'` and
`el.classList='a b'` throw here), and document.body.
