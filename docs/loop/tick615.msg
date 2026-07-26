capability(js): a feature-detect that THROWS is worse than one that answers no (+ constitution check #43)

TICK SHAPE: capability + the due constitution check. Bar 0 untouched; no ratchet floor moved.

CONSTITUTION CHECK #43 (due every 8 ticks; last at 607). Its finding: §VI.3's fourth clause — is this
rule implemented MORE THAN ONCE, and do the copies agree? — fired in EIGHT CONSECUTIVE TICKS:

  610  drain bounded        run_deferred | run_with_fetcher   -> the second had NO bound at all
  611  unreachable = a row  4 `continue`s in one loop         -> three dropped it silently
  612  rendered text set    outerText | innerText             -> innerText had no setter
  613  XHR event dispatch   streaming | buffered              -> buffered never fired loadend
  614  ratio needs a sample certificate() | the printed MEAN  -> the MEAN averaged vacuous rows

That stops being a heuristic and becomes the MODAL DEFECT. Common cause, worth naming because it
predicts the next one: this engine grew by ADDING A SECOND PATH BESIDE A WORKING ONE — a streaming
delivery beside a buffered one, a JS prelude object beside a Rust binding, a report function beside a
certificate function — and the second path is written by someone who read the BEHAVIOUR rather than the
RULE. The remedy that worked five times running is mechanical, not attentional: fix one, GREP FOR THE
OTHER, and route both through a single function, so there is nowhere left to write the rule twice.
t613 is the clean form — six open-coded dispatch sites collapsed into one `__xhrFire`.

Check #43 also adds §VI.3.5: BEFORE BELIEVING A MEASUREMENT, ESTABLISH WHETHER IT IS A PROPERTY OF THE
ENGINE OR OF THE INSTRUMENT. Earned three times this session — the wall that was a release relink (and
had already produced a written request to loosen a RATCHET MARK), the getter-only list that was a grep
artifact, the 66.7% coverage that was an oracle shell. Each was one command from disproof, and each was
believed because the number was large, or consistent, or came from code I had just written.

THE CAPABILITY HALF: `HTMLScriptElement.supports` was undefined, and that is a THROW at exactly the
moment a page is trying to degrade gracefully. A page calls it to choose between an ES-module bundle and
a classic fallback, or whether an import map is worth emitting. Calling a missing static is
`TypeError: HTMLScriptElement.supports is not a function`, so the page does not take the fallback branch
— IT DIES AT THE FEATURE-DETECT. The general shape:

  A missing FEATURE costs you the feature. A missing FEATURE-DETECT costs you the FALLBACK —
  and the fallback is the thing that was going to work.

The answers are ours to give truthfully, and one is a `false` on purpose. classic/module/importmap are
genuinely backed (t512-516 built the module graph with a real resolve hook and cycle-safe walk).
`speculationrules` answers FALSE, and that is load-bearing in the other direction: a page asking about
it is ASKING TO BE TOLD NO so it can prefetch itself. A flattering `true` means the page stops and we
never start — a capability claimed is a capability nobody provides.

RED-PROVEN, TWO MECHANISMS:
  · remove `supports` again      -> RED `threw:true` — welt.de's exact rung
  · answer `true` for everything -> RED `speculationrules:false` — the flattering-answer trap

HONESTY: welt.de STILL DOES NOT RENDER — third rung cleared without the door opening.
  t612 before: adblock: TypeError: setting getter-only property "innerText"
  t613 before: adblock: TypeError: a.addEventListener is not a function
  t615 before: adblock: TypeError: HTMLScriptElement.supports is not a function
  t615 after:  adblock: Error: Failed to execute packing script          <- still blank
One error remains: `a page module failed: SyntaxError: expected expression, got '<'` — a
`<script type=module>` request answered with HTML. That is not a missing API; a module URL resolved to a
document. Named rather than guessed at.

And the honest accounting of this arc: three ticks, three real TypeError-class gaps, one site still
blank. Each fix stands on its OWN measured population (t612: 4 of 16 sites write innerText; t613: 8 of
16 construct an XHR and 4 of 16 attach listeners), which is why they are claimed at all — not because
the site improved, because it has not. A chain-peeling arc has to be able to say that, or it becomes a
sunk-cost march.

Gate: G_SCRIPT_SUPPORTS (engine/page/tests/g_script_supports.rs) — it exists and does not throw, it is
STATIC (absent on instances), the three backed types answer true, the unimplemented one answers false,
and all four are real booleans since `=== true` is how feature detects are written.
WIKI: docs/wiki/js-engine.md — "a feature-detect that throws is worse than one that answers no".
PATTERN: [no-pattern] — third instance of t612's class (a site that probes the DOM and blanks itself on
a bad answer), not a new one.

NEXT: the module-answered-with-HTML error is the last visible rung on welt.de and the only one that is a
MECHANISM rather than a missing member. Still standing for the observer: manuk-wpt's tests (including
G_CERT_FALSIFIABLE, whose doc comment says "re-run on every wall") are NOT in the wall — 54 tests, 14s.
