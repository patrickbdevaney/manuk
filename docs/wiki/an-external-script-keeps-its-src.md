# An external script keeps its `src`

*Tick 1481. A control flag was living in a web-facing attribute, and the attribute lost.*

## The mechanism

`fetch_external_scripts` fetched a `<script src>`, appended the source as a text child, and then:

```rust
dom.remove_attr(node, "src");
```

The **absence** of `src` was the signal `collect_inline_scripts` read as *"this node has text and
should run"*. So the control flag and the attribute the page reads were the same bit, and there was
no configuration in which the page could have it back.

*A sentinel that is also a legal value is not a sentinel* (t1424), one class further along: here the
sentinel was a legal **absence**.

⚠ **This engine had already fixed the same bug one path over.** `Page::dyn_scripts_ran` exists,
with a doc comment that says it in as many words:

> *"`fetch_and_run_dynamic_scripts` removed `src` before evaluating, so throughout its own execution
> a script's `document.currentScript.src` was the empty string where Chrome gives the URL. That is a
> wrong answer of the right type… `new URL(document.currentScript.src)` does not skip, it **throws**
> — `TypeError: Invalid URL: ` on 4 of 200 CrUX corpus sites."*

One rule, two implementations. The PARSER half — the half that runs on every page — was the stale one.

## What it cost, measured

The t1480 boot histogram's **top first-failure class** over 40 CrUX sites was one third-party bundle
at a byte-identical minified offset:

```text
  OneTrustStub</S.prototype.setOTDataLayer@https://www.ikea.com/   inline#156:1:12608
  OneTrustStub</S.prototype.setOTDataLayer@https://www.otomoto.pl/ inline#66:1:12608
  TypeError: can't access property "hasAttribute", p.stubScriptElement is null
```

Two families of entirely ordinary code break on this, and essentially nothing else does:

* **A bundle locating its own tag** — `document.querySelector('script[src*="otSDKStub"]')`. Every
  consent SDK and tag manager does this to read its own `data-*` configuration.
* **A bundle deriving its asset base** — `new URL(document.currentScript.src)`. This is literally
  what webpack's `publicPath: 'auto'` emits, so it is on a very large share of bundled sites.

## Arbitrated against headless Chrome

```text
                                       Chrome     before     after
  runs                                 1          1          1
  document.currentScript.src           has-src    ""         has-src
  …and it is ABSOLUTE                  abs        —          abs
  new URL(currentScript.src)           ok         THREW      ok
  querySelectorAll('script[src]')      1          0          1
  querySelectorAll('script[src*=…]')   1          0          1
  the data-* attribute                 abc-123    abc-123    abc-123
```

## The fix is three lines and one of them is the dangerous one

1. `fetch_external_scripts` no longer removes `src`.
2. `collect_inline_scripts(dom, inlined)` — run a node that still has `src` **if the host inlined its
   source**. The set is `PageContext::external_scripts`, which already existed for the `load` event;
   it simply had to be READ here. A failed fetch, an SRI mismatch and a CSP refusal all leave `src`
   AND stay out of that set, so each still takes the "there is nothing to run" path it always took.
3. **`Page::dyn_scripts_ran` is seeded with that same set.** `drain_injected_scripts` selects scripts
   by *"has `src` and is not in `dyn_scripts_ran`"* — so restoring `src` without this makes every
   external script on every page fetch a second time and **execute a second time**. A page's
   analytics, its consent gate and its router all booting twice is a Bar-0 shape dressed as a
   one-line attribute change. `runs=1` is the load-bearing row of the gate, not a formality; mutation
   3 turns it into `runs=2`.

## Result on the corpus, and what it does NOT claim

Same 40 sites, same binary in every other respect, `manuk-wpt fidelity`:

```text
                                  before   after
  boot CLEAN                        28       29
  documents with >=1 uncaught       11       10
  uncaught errors, all classes      18       11
  occurrences of "Invalid URL"       4        1
  first-failure `…:hasAttribute`     2        0   <- the class is gone
```

⚠ **The render numbers did not move, and they are not offered as if they had.** `ikea.com` and
`otomoto.pl` are byte-identical before and after on `structural` and `SHAPE` — the consent SDK now
runs, and that is a capability restored, but it is not what was shaping those two pages. The
sweep-level means (`MEAN COVERAGE 85.9 → 86.9`, `SHAPE 73.5 → 73.2`, `VISUAL 59.5 → 57.8`) are
**single runs over live origins with no error bar measured**, and per t1410 a delta no larger than
the band is not a movement. The boot-error counts are the evidence here because they are discrete,
per-site and reproducible; the means are not.

## Residue, named

⚠ The node now carries `src` **and** a text child, where Chrome's external script has `src` and
`textContent === ""`. Holding the fetched source in a side map instead of in the DOM is the complete
fix — the map has to reach `collect_inline_scripts` through `PageContext`, which is a larger change
than this one. This is the half the corpus actually fails on, and the gate claims only that half.

⚠ `document.getElementsByTagName('script').length` read from inside the first script is `2` here and
`1` in Chrome: this engine parses the whole document before running anything, so later `<script>`
elements are already in the tree. That is the parse/execute interleaving and is a much larger thing;
the gate deliberately does not pin it.

## The gate

`g_an_external_script_keeps_its_src` — the Chrome row above, byte-for-byte, with `runs=1` as the
vacuity arm. Red under three mutations: restore the `remove_attr`; drop the `inlined` exemption (the
script never runs at all); drop the `dyn_scripts_ran` seed (`runs=2`).
