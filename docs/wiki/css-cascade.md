# CSS AND THE CASCADE — Stylo realities and quirks actually encountered

## Stylo's *servo* build hardcodes `parse_has() -> false`

A selector containing `:has()` therefore **fails to parse**, and CSS error-recovery **discards the
whole rule** — its declarations never reach the cascade at all. (Gecko's build returns `true`; this is
a *build default*, not a capability limit.) **13% of the corpus uses `:has()`.**

**`./stylo` in this repo is a REFERENCE CHECKOUT THAT NOTHING BUILDS.** The workspace depends on
`stylo = "0.19"` **from crates.io**. Editing the local checkout changes nothing — a fact that cost a
tick to discover and that **re-prices every "just flip the flag" idea** into "vendor Stylo and pay the
tax on every bump."

**The resolution — and the ladder it established:** extend the selector engine we *already own* (the
one behind `querySelectorAll`) with `:has()`, and apply the rules Stylo discarded as a **second cascade
pass** ordered by `(specificity, source order)`. Contained, no fork.

> **The ladder, now settled:** pref → minimal flag delta → **hand-rolled supplement** → hand-rolled
> module. **Never: give up the capability.** (But never copy Blink/Gecko *code*, and never fork an
> engine's *algorithms*.)

**Known, bounded inaccuracy, stated rather than discovered later:** a low-specificity `:has()` rule
cannot currently *lose* to a higher-specificity normal rule, because Stylo does not report which rule
won each property. Strictly better than the rule not existing — and written down.

## An optimisation that makes a data structure smaller must be asked WHAT IT DROPPED

`RuleIndex` (a tick-14 cascade optimisation, 339ms → 199ms) walked each stylesheet's rules, read each
`StyleRule`'s `selectors` and `block`, and **never looked at its `rules` field** — the **nested** rules.
Stylo parses them correctly and always had. We threw every one of them away.

**≥41% of the corpus uses CSS nesting** in its inline `<style>` blocks *alone* (external sheets were not
even scanned, so that is a **floor**). It was the single largest cause of both real rendering
divergences the oracle found: *"we lose flex/grid on this node"* (11,324) and *"we show what Chrome
hides"* (2,433 — a nested `display: none` never applied, so menus and modals rendered on top of the
page).

> **A gate comparing boxes could not see it**, because the boxes it produced were internally consistent
> — they were just consistently wrong.

## The attribute-selector case flag was STRIPPED, not APPLIED — and the namespace prefix leaked into the name

Our own selector engine (`engine/css`, the one behind `querySelectorAll` and the `:has()` supplement)
parsed `[attr=val i]` by *deleting* the trailing ` i`/` s` and matching the value case-**sensitively**.
So `[foo='bar' i]` never matched `foo="BAR"` — `querySelector` returned `null` — and the same for every
operator (`~= ^= $= *=`). Separately, a namespaced name (`[*|foo]`, `[|foo]`) was carried into the match
verbatim, so it matched no attribute at all. **These two mechanisms were the single largest matching gap
in `css/selectors`: ~117 subtests (667 → 784), from one bounded fix, crash-free, no area regressed.**

The fix (Selectors §6.3): a `ci: bool` on the parsed attribute selector; `parse_attr_value` splits the
value from an optional `i`/`s` flag *respecting quotes* (`'bar'i`, `'bar' i`, `bar i` all parse); the
flag is itself ASCII case-insensitive (`I` == `i`); `strip_attr_ns` drops everything up to and including
`|` (HTML attributes are all null-namespace, so `*|foo`, `|foo`, `ns|foo` → local name `foo`). Matching
normalises both sides with a `Cow` — **borrowed on the common case-sensitive path, so the hot selector
loop allocates nothing** unless the `i` flag is actually present. Default and `s` stay case-sensitive.

> **A flag that is stripped rather than applied is worse than one that errors:** it silently downgrades
> a correct selector to a wrong-but-plausible one. The value looked right (`bar`); only the *case rule*
> was missing, and nothing said so — `querySelector` just returned `null`.

**Method note:** the fail-message histogram (`--show-failures` → normalise `"…"`/digits → `sort | uniq
-c`) put this cluster at the top by count, and a 14-case probe page (`foo="BAR"`) isolated the exact
mechanism *before* any engine edit — the `.sheet is undefined` cluster was 4× larger but a deep CSSOM
saga; this was bounded and Bar-0-safe, so it went first. Rank by **flip-per-risk**, not raw count.
[[parity-methodology]]

## `<body>`'s background propagates to the CANVAS

If it does not, **every dark-themed site is a dark box floating in a white void.** Found via an
iframe, because *a child document is "a page shorter than its viewport"* — the same condition, made
obvious.

## `visibility` and `opacity` interact with animation

An element with `opacity: 0` that *specifies an animation* is not hidden — it is **about to be shown**.
Treating the computed value as final hid ~a fifth of the web's content.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## Stylo's `grid_enabled()` reads `layout.grid.enabled`, which is OFF by default under `servo`

With the pref unset, **`display: grid` is silently dropped AT PARSE TIME** — the property never reaches
computed style, so grid pages auto-place in DOM source order and look catastrophically wrong **with no
error anywhere**. Flip it at cascade init via `stylo_static_prefs::set_pref!`.

## Skipping `@supports` renders the FALLBACK branch of every progressively-enhanced site

The modern idiom is `.thing { display: none } @supports (display: grid) { .thing { display: block } }` —
hide the legacy fallback, then reveal the real layout inside the feature query. **An engine that does not
descend into `@supports` silently renders the fallback**: Wikipedia's entire Vector-2022 TOC sidebar never
appeared.

Progressive enhancement **inverts the usual failure**: unsupported at-rules give you a consistently
*old-looking* web, not a *broken-looking* one — **so nothing screams.** The same applies to `@layer`
(modern design systems ship whole sheets inside one) and `@media`.

**Stylo evaluates the condition at parse time into `SupportsRule::enabled`**, so honouring it is just
descending when enabled.

## `@media` is a rule CONTAINER — a cascade that only matches top-level `CssRule::Style` applies zero responsive rules at ANY width

The Stylo viewport `Device` was already correct; **only the walk was missing.** And separately, the Device
itself was once **hardcoded to 1024×768**, so every media query evaluated against a fiction — the mechanism
behind a long-running "Wikipedia Vector-2022 is structurally broken" bug.

`matchMedia` must implement the **same** evaluation (comma = OR, ` and ` = AND) so JS feature-branches
**agree with the CSS cascade** rather than contradicting it.

## `match_non_ts_pseudo_class` returning `false` for everything freezes the entire no-JS-menu web SHUT

A stub that answers `false` to every pseudo-class kills `#toggle:checked ~ .panel` — **the checkbox hack** —
which is how a large part of the web builds menus, accordions, dropdowns and sidebars **with no JavaScript
at all**. Every one of them is frozen closed **forever**.

The statically-answerable set must come from the DOM: `:checked`, `:disabled`/`:enabled`,
`:required`/`:optional`, `:read-only`/`:read-write`, `:link`/`:any-link` (an `<a>` **with** an href),
`:placeholder-shown`, `:valid`, `:defined`, `:open`. The genuinely dynamic ones (`:hover`, `:active`,
`:focus`) correctly answer `false` for a static layout — and **`:visited` must answer `false`
deliberately: it is the web's oldest privacy leak.**

## Stylo's COMPUTED values are not its RESOLVED values — border-width and outline-width are traps

- Stylo computes **`border-width` at `medium` (3px) even when `border-style: none`** — it zeroes
  none/hidden only at *resolved-value* time. Taking the computed width at face value **draws a spurious
  3px border around every block on the page.**
- Identically, **`outline-width: medium` (3px) is computed even when `outline-style: none`**, and
  `outline-color` resolves to `currentColor` — so a naive mapping paints **a 3px black ring around every
  element on the page.**

**Missing accessors in Stylo 0.19:** `vertical-align` has **no computed longhand** (CSS-Inline-3 turned it
into a shorthand) and **`visibility` is not exposed** — both must be recovered from a second, simpler
cascade. That recovery pass is why `cascade_via_stylo` runs a **second full document walk**, and any
cascade profiling that ignores it understates the cost.

## Presentational hints are still load-bearing, and Stylo will NOT synthesize them

Stylo's cascade does not produce HTML presentational hints (they live behind the `TElement` wall). But:

- **Hacker News gets its ENTIRE visual identity** from `bgcolor="#ff6600"` / `#f6f6ef` on `<table>`/`<td>`.
- `<img width/height>` sizes **half the images on the web**.
- `<table width="85%">`, `<td width>`, `cellpadding`/`cellspacing` are everywhere.

Apply them **post-cascade, only where the property is still at its initial value**, so author CSS keeps
priority (per HTML's presentational-hints precedence).

## Icons on the modern web are an empty element with a `background-color` shaped by a `mask-image`

Paint the background and ignore the mask and you get **a solid black square where every icon should be.**

## `inline-flex` is a distinct display value, not block-level flex

Mapping it to block-level flex makes **every icon button fill its container.** Because it is one of the
three or four values that decide whether a control **shrink-wraps**, a missing `inline-flex` looks like a
layout-algorithm bug rather than a missing enum variant.

## A `background-image` is a DECORATION; an `<img>` is a REPLACED ELEMENT whose bitmap IS the box

Storing both in the same `node → decoded image` map made the **replaced-element blit** (which stretches the
bitmap to fill the box — correct for `<img>`) fire for **backgrounds too**, painting a scaled copy directly
over the correctly-tiled background beneath it. **Every sprite, texture, pattern and icon on the web was
blown up to the size of whatever element carried it.**

*The tiling code was never broken. It was simply being painted over, every time.*

## An unmodelled pseudo-element silently mis-styles its SUBJECT

`p::before` and `p::first-line` were parsed as a bare `p` selector, so **a rule intended for a
pseudo-element applied to the element itself.** **Dropping** selectors whose pseudo-element is not modelled
is the correct failure mode — *a rule that does nothing is strictly better than a rule that styles the
wrong box.*

## The `display` divergence number is ~25% representational NOISE

Of a 27% `display` disagreement against Chrome (**33,825 nodes**), **4,299 are replaced elements where
Chrome computes `inline` for `<img>`/`<svg>` and we use `inline-block` to make them atomic** — *same
rendering, different label.*

The genuinely real buckets were: **11,324** "we lose flex/grid on this node", **2,433** "we SHOW what
Chrome HIDES", and **2,033** "we HIDE what Chrome SHOWS" — *the smallest and the worst, because it is
content nobody can see.* **Split a divergence metric by whether each bucket is a real rendering difference
before optimising against it.**

## MinimalCascade's deficits are architectural, and a hybrid front-end matcher was REJECTED

Selector matching is O(rules × elements) with **no bucketing and no ancestor Bloom**; no `@media`/
`@supports`/`@layer`; **`var()` unsupported**; the `background` shorthand collapses to `background-color`;
specificity is approximate. Reaching conformance from there means rebuilding `SelectorMap` + a rule tree +
invalidation — **i.e. rebuilding Stylo by hand.**

The proposed hybrid (our selector fast-path in *front* of Stylo's compute) was **rejected**: Stylo's
`SelectorMap` + `AncestorHashes` Bloom fast-reject **already IS the industry's best fast-path**, and a
second matcher risks **divergence** (a rule that matches in one but not the other). *Stylo for everything;
MinimalCascade as a fallback engine, never a front-end.*

> ⚠ **Two cascades will disagree about whether text is VISIBLE.** `MinimalCascade` could not parse a
> **unitless zero**, so `font-size: 0` left the size *inherited* and text rendered at full size, while
> Stylo correctly zeroed it. Worse, the verification harness **defaulted to MinimalCascade while the shell
> shipped Stylo** — *the gates were testing a cascade no user ever sees.*

## `var()` and `@media` evaluation effectively exist ONLY inside full Stylo

`selectors` and `cssparser` are cleanly usable standalone (proven by `scraper`, `kuchikiki`,
`lightningcss`) — but they give **matching and parsing only**. **No lightweight standalone crate evaluates
custom properties or media queries** (the one dedicated media-query crate has been dead since 2017).
Everyone who needs a real cascade takes **full Stylo**. *"Just use `selectors` + `cssparser`" is not an
escape from the Stylo dependency.*

## `grid-template-areas` was entirely unparsed — and BOTH underlying engines already supported it

Named grid items **auto-placed in DOM source order**. The notable part: **no new algorithm was needed.**
Stylo already computes `NamedArea { name, rows, columns }` with pre-resolved ranges, and **taffy already
has `GridTemplateArea` + `GridPlacement::NamedLine`.** *The work was purely cascade plumbing.*

## Stylo's DOM trait wall is 126+ methods and compiles only as ONE indivisible unit

`TElement` (76) + `TNode` (20) + `selectors::Element` (30) over interlocking associated types. **The four
traits are mutually recursive**, so a half-written wall leaves the crate uncompilable — which is why it
must be **one dedicated multi-session commit**, not sliced across ticks.

**The hardest-to-discover impedance is `ElementDataWrapper`.** Stylo keeps per-node style in
`AtomicRefCell<ElementData>` (it uses `atomic_refcell` because **it restyles in parallel**), and the
returned `ElementDataRef`/`ElementDataMut` have **private fields, constructible only through
`stylo::data::ElementDataWrapper`**. An arena DOM therefore needs a **`NodeId`-indexed side-table** of
those — a bare `AtomicRefCell<ElementData>` **will not typecheck**.

## Full-page zoom scales ABSOLUTE lengths only

Percentages and `auto` resolve against an already-scaled containing block, so **scaling them too
compounds**. Because `font_size` scales, glyphs **rasterize at the larger size** — a genuine **reflow**,
which is what distinguishes full-page zoom from pinch-zoom (a compositor surface transform with no layout).
**Zoomed styles must always be derived from the BASE cascade**, never from the previously-zoomed one, or
repeated zooming compounds.

## Selector identifiers decode CSS escapes — `take_ident` used to stop at the backslash (tick 137)

The hand-rolled selector parser (`parse_selector` → `take_ident`, which backs both the cascade and JS
`querySelector`/`matches`) treated `\` as a **terminator**: `#has\.dot` parsed the id as `has` and
matched nothing, and every `CSS.escape`-produced selector on the web silently missed. The fix is
css-syntax **§4.3.7 "consume an escaped code point"** in two places:

- **`take_ident`** now decodes escapes (`consume_escaped_code_point`) and accepts raw non-ASCII (U+0080+)
  as ident chars. `\.`/`\!` → the literal punctuation; `\30 x` → `0x` (1–6 hex digits, one optional
  trailing whitespace); NUL and out-of-range → **U+FFFD**.
- **The pre-tokenizer** (which splits a selector into compounds on whitespace/combinators) is now
  escape-aware: on `\` it keeps the escape sequence — including a hex escape's trailing whitespace —
  **verbatim**, so `#\30 x` is one compound, not `#\30` descendant `x`. Without this the split happened
  *before* `take_ident` ever saw the escape.

**A surrogate-half escape (`\d83d`) is DROPPED, not mapped to U+FFFD — a named limitation.** The spec
says U+FFFD, but this engine stores attribute values as **UTF-8**: a lone-surrogate id set from JS is
already lossily collapsed to U+FFFD on the way into the DOM. Emitting U+FFFD from the selector too would
make `#\d83d x` **false-match** an id that only holds U+FFFD because its surrogate was lost — turning a
`ParentNode-querySelector-escapes` *"should never match"* green→red. Dropping the code point preserves
the non-match, so the tick regresses nothing; faithful surrogate handling is gated on WTF-8/UTF-16
attribute storage — **the same subsystem** as CharacterData surrogate splitting ([[dom-semantics]]).

**MEASURED:** dom/nodes 3245 → 3285 (**+40**), before/after FAIL sets diffed → **zero regressions**;
css/selectors held at its banked 784 (the cascade path is unaffected in behaviour). Bar 0 **0**. Test
`selector_ident_escapes_decode_per_css_syntax`.

## Quirks mode: the verdict travels ON the `Dom`, not through signatures (tick 242)

**Why this was a three-line-per-file change instead of a refactor.** The parser's quirks verdict has to
reach the style system, and the obvious shape — return it from `parse()` and thread it as a parameter —
would have touched `manuk_html::parse`/`parse_bytes`/`StreamParser`, `Page::from_dom`, the `Page` struct,
and **all 18 `cascade_styles` call sites** in `engine/page`. Putting a `quirks: bool` on `Dom` instead
costs one field and changes **no signature anywhere**, because every consumer already receives a `Dom`:

```
html5ever  --set_quirks_mode-->  ArenaSink (holds Rc<RefCell<Dom>>)  -->  Dom::quirks
                                                                            │
        engine/page: cascade_styles(&Dom, ..) ──────────────────────────────┤ (unchanged signature)
        engine/css:  cascade_via_stylo(dom: &Dom, ..) ──> let qm = dom.quirks()
                     StyloDocument { dom: &'a Dom } ──> TDocument::quirks_mode()
        engine/js:   doc_get_compat_mode ──> this_node(vp) ──> (*dom).quirks()
```

**The general rule: a value every consumer already has a handle to should ride on that handle.** The
signature-threading version is the one that looks more explicit and is the one that makes the change
too big to land in a tick.

### Stylo already implements the quirks — we were only failing to tell it which mode we were in

`QuirksMode` is an *input* to Stylo, not a feature request. Passing `QuirksMode::Quirks` enables, for
free: unitless lengths (`values/specified/mod.rs` `AllowQuirks::allowed`), case-insensitive id/class
matching (`selector_map.rs`), and the `<font size>` mapping table (`values/specified/font.rs`). That is
why this is plumbing rather than layout math.

**There are TWO parse paths and wiring one is not enough.** `StyloStylesheet::from_str` handles
`<style>` and linked CSS; `parse_style_attribute` handles the inline `style=` attribute. After wiring
only the first, `width: 100` still dropped on a quirks page while the identical rule inside a `<style>`
block worked — and **legacy markup, which is precisely the markup that lands in quirks mode, is
overwhelmingly inline-styled.** Both take a `QuirksMode`; both need the real one.

### Reporting and rendering are ONE capability

`document.compatMode` must flip in the same tick as the layout wiring. Reporting `BackCompat` while
still rendering standards is a *worse* failure than the hard-coded constant it replaces, because it is
**actionable by the page**: a site that branches on `compatMode` takes a quirks code path the engine
does not honour. `g_quirks_mode` therefore asserts both directions of both halves, plus a fifth claim
that the two modes actually *differ* — each of the first four can be satisfied by a constant; that one
cannot.

### `LimitedQuirks` folds to `false`, deliberately

html5ever has three states; `Dom::quirks` is a `bool`. "Almost standards" mode differs from full
standards only in the inline-image baseline rule and does **not** enable the unitless-length quirk, so
`false` is correct for every behaviour currently gated on it. Inventing a three-state enum before
anything reads the third state would be speculation; the note on the field says what to do if that
changes.

### Gate note

One `#[test]` function, not five. Multiple `#[test]`s each calling `Page::load` run on separate threads
and **SIGSEGV** — SpiderMonkey is not shared-thread-safe — and before crashing they produced a subtler
artifact: `compatMode` read back as the fixture's placeholder on one test and the real value on
another, i.e. a script that silently did not run. **A gate whose fixture races itself cannot tell a
regression from its own harness.**

### The half-fix trap: a custom rule index must be keyed the way it is queried (tick 243)

Enabling quirks' **case-insensitive id/class matching** is not "flip the `MatchingContext` constants".
This engine buckets rules in its own `RuleIndex` (`by_id`/`by_class`) *before* matching, as a cascade
optimisation. Telling Stylo's matcher "case-insensitive" while the index still keys by exact case means
`#FOO` is filed under `FOO`, the element `id="foo"` queries `foo`, the bucket misses, and **the rule is
discarded before matching ever runs.** The fix compiles, reads as complete, and does nothing.

**Proven, not reasoned about:** with `index_key` reverted to exact case and every `MatchingContext`
already passing `Quirks`, `g_quirks_mode` reports `#FOO` giving 800px instead of 250px. Both ends have
to agree, so both go through one `index_key(v, qm)` helper — applied when bucketing in `add_rules` and
when querying in `candidates`.

**The general shape, and this file already contains the other instance of it:** the CSS-nesting bug a
few sections up was the *same* index dropping rules it never looked at. **An index is a lossy copy of
the rule set, and every predicate you add to the matcher has to be reflected in the key — or the index
silently pre-filters the thing you just taught the matcher to accept.** Ask what the index dropped,
every time the matching semantics change.
