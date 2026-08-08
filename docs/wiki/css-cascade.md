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

### `user-select` is behind the SHARED `layout.unimplemented` pref, not a per-property one (tick 464)

`user-select` (and its `-moz-`/`-webkit-` prefixes) carries `servo_pref = "layout.unimplemented"` in
Stylo 0.19's `longhands.toml` — the SAME pref ~35 other properties share. Off by default, so the servo
build drops `user-select` at parse and every element's computed value stays `auto`; `getComputedStyle(el)
.userSelect` was `undefined`. There is no `user-select`-specific pref, so the Option-1 fix flips
`layout.unimplemented` on — which ungates the whole set. That is SAFE here because **the pref gates
PARSING only, and we consume a fixed set of computed values via explicit `cv.clone_*()` calls**
(`user_select` is the sole addition; the other ~34 ungated properties — `object-fit`, `text-overflow`,
`content-visibility`, `contain`, `counter-*`, `mask-*`, … — are read, where we use them at all, from
MinimalCascade's `m`, never from Stylo's clone). Enabling their parse changes nothing we read; the parity
and CSSOM gates confirmed no regression. The keyword maps in `stylo_map.rs` beside `pointer_events` (the
identical two-cascade-free pattern) and serializes in `getComputedStyle` as `userSelect` + the
`webkitUserSelect` alias Chrome also exposes. Scope boundary, stated: this resolves the COMPUTED VALUE the
CSSOM reports; the geometry of a user mouse-drag selection honouring `user-select` is a layout/hit-test
concern the engine does not model — the same boundary the `Selection` shim documents.

### `color-scheme` rides the same pref and has a REAL paint tooth: the dark canvas default (tick 465)

`color-scheme` is in the same `layout.unimplemented` set (so t464's flip already lets it parse; it is
`inherited_ui`), but unlike `user-select` its computed value drives PAINT, not just the CSSOM. Stylo
computes it as a `ColorScheme { bits: LIGHT|DARK|ONLY }` bitfield; `stylo_map.rs` collapses it to
`Normal/Light/Dark/LightDark`. The load-bearing effect is in `Page::canvas_background()`: CSS propagates
the root background to the whole viewport, so a dark-only page (`color-scheme: dark`, no explicit
background) must paint the canvas dark — otherwise its content sits on a dark box floating in a WHITE void
below the fold (the identical failure the `<body>`-background-propagation fix addressed, just triggered by
the scheme). The used scheme is dark iff the page lists `dark` and NOT `light` — a dark-only page renders
dark regardless of the OS preference (Chrome's behaviour); `light dark` defers to `prefers-color-scheme`,
which defaults light here. **Scope boundary:** only the canvas default (the void) is modelled — the void
has no text so darkening it cannot make content unreadable. `color-scheme` also flips UA form-control/
scrollbar appearance and the default TEXT color under dark; those are deeper system-color used-value
adjustments not modelled, and pages declaring dark almost always set their own text/control colors.

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

### `:muted` is querySelector-only, and that is a *build* fence (tick 344)

`:muted` selects a muted `<video>`/`<audio>`. Our own `querySelectorAll` engine (`engine/css`, `Pseudo::Muted`)
matches it off the `muted` **content attribute** — the same attribute-vs-live-property approximation `:checked`
makes against `.checked`. But the CSS *cascade* cannot: the **servo** build of Stylo has no `Muted` variant in
`NonTSPseudoClass` (nor `Playing`/`Paused`/`Seeking` — they are gecko-only, verified in
`stylo-0.19.0/servo/selector_parser.rs`), so `video:muted { … }` **fails to parse and the whole rule is
discarded**, exactly like `:has()` above. So JS that finds muted players (`querySelectorAll('video:muted')`)
works; a stylesheet that styles them does not, until Stylo is vendored or the media state is plumbed into a
gecko-style state flag. The dynamic media pseudo-classes (`:playing`/`:paused`/`:seeking`) need live playback
state reachable from the DOM node and are deferred with the same note.

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

## The global `hidden` attribute needed its OWN rule — `input[type=hidden]` never covered it (tick 489)

`<div hidden>` reported `display: block` and painted its contents. The UA sheet only carried
`input[type=hidden] { display: none }` — the *value* on a specific control — while the **global boolean
`hidden` attribute** (valid on every element, and one of the most common visibility toggles on the web:
tab panels, initial-collapsed accordions, feature-detect fallbacks, `el.hidden = false` show/hide) had
no rule at all. Measured, not assumed: a probe showed `plain:block` before, `plain:none` after.

The spec rule (`#hidden-elements`) is `[hidden]:not([hidden="until-found"]) { display: none }`. The
`until-found` exception matters and is deliberately preserved as **visible**: the spec renders that value
with `content-visibility: hidden` (collapsed-but-findable), which we do not support yet — so collapsing
it to `display:none` would hide content a user could never reveal on find. Leaving it visible is the
honest first brick; falsely collapsing it would be a capability we cannot back up.

Same **two-cascade lockstep** as `<dialog>`/`[popover]`: the rule lives in `stylo_engine.rs` (the live
Stylo sheet) *and* in `apply_ua_defaults` (`css/src/lib.rs`), because a page whose two cascades disagree
about whether an element renders is the `<source>` bug again. And it must be *undoable*: `el.hidden =
false` removes the attribute, the cascade re-runs on the mutation, `[hidden]` stops matching, and the
element comes back — a rule that could not be reversed would break every toggle it exists to serve.

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

## `content: attr(name)` drew an EMPTY box until the extraction loop stopped keeping only strings

Generated content (`::before`/`::after`) is assembled in `cascade_pseudo` by walking Stylo's computed
`Content::Items` and concatenating them into the pseudo's text. The loop kept **only** `ContentItem::String`
and silently dropped every other item — including `ContentItem::Attr`, the `attr(name)` function. So
`a::after{content:" ("attr(href)")"}` produced `" ()"`, `[data-tip]::after{content:attr(data-tip)}` produced
`""`, and the pseudo box was generated but **blank**: present in the tree, invisible on the page. That is the
worst failure class — content nobody can see — and `attr()` in generated content is not a corner: print
stylesheets expand links with it (`a::after{content:" ("attr(href)")"}`), CSS-only tooltips read
`attr(data-tooltip)`, breadcrumbs and data tables label cells from their attributes.

The fix resolves `ContentItem::Attr(a)` against the **live element** via `StyloElement::attr(&a.attribute)` —
the same accessor the attribute-selector matcher already uses, so there is one source for "what does this
element's attribute say". A missing attribute pushes the **empty string**, never a dropped pseudo (CSS2.1):
`content:attr(missing)` still generates its (empty) box, matching Chrome. Namespace is ignored — attributes
are keyed by qualified, HTML-lowercased name here, and a namespaced `attr()` in `content` is vanishingly rare.
This is the CSS2.1 string form only; the Level-5 typed/fallback `attr(data-n number, 0)` is not in this
Stylo's `Attr` shape and stays an honest gap. Gated by `content_attr_resolves_the_elements_attribute`, proven
RED (revert the arm → `after` reads `" ()"`).

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


## `:hover` is a cascade INPUT, and the two relayout paths each miss it differently

`:hover` was hard-coded `false` in `stylo_dom.rs` alongside `:active` and `:focus` (tick 245,
`G_HOVER`), behind a comment that was correct about a static render and wrong about a browser: *"a
page is not being hovered when it is laid out."* True — and nothing fed it afterwards either.

**What it cost is a whole category of navigation, not a visual polish item.** `nav li:hover > ul {
display: block }` is how a large share of the desktop web builds top navigation **with no JavaScript
at all** — structurally the same trick as the checkbox hack that `:checked` unblocked. With `:hover`
never matching, every one of those menus is permanently closed: the links inside are unreachable to
a user, invisible to an agent, and **nothing reports a problem**, because the page renders exactly
what it was told to render.

### `:hover` matches ANCESTORS, and that half is the mechanism

The state lives on `Dom` (`hovered: Option<NodeId>`), for the same reason `quirks` does: every
consumer already holds a `&Dom`, so it reaches the cascade with no signature change anywhere.
`Dom::is_hovered` walks the ancestor chain, and matching only the exact hit target fails in a way
that *looks like it works*: the pointer enters the `<li>`, the submenu opens, the pointer moves one
pixel into that submenu — it is now over an `<a>` inside the `<ul>`, the `<li>` stops matching, and
the menu closes underneath the cursor. That is the flickering-menu bug.

The dirty bits follow the same rule. `set_hovered` marks **every node on both the old and the new
chain**, because the dirty bit is per node, not per subtree, and the element whose style actually
changes — the `<li>` — is the one the pointer is never over.

### The trap: neither existing relayout recascades a state change, and they fail oppositely

This is the part that cost the tick, and both halves are the *half-fix* shape (tick 243's index bug
again): the code compiles, reads as complete, and does nothing.

| path | when it recascades | how it fails a hover |
|---|---|---|
| `relayout` | only when the **tree GREW** (node count vs `styles.len()`) | a hover adds no nodes → re-lays-out the OLD styles. `:hover` matches, `hovered` is set, every piece of wiring is correct, **not one pixel moves.** |
| `relayout_incremental` | on the dirty bits — correct trigger | rebuilds its sheet list from `MinimalCascade::collect_style_elements`, which sees inline `<style>` and **not `<link>`ed sheets**. Hover any link on any site with external CSS and **every external stylesheet drops out of the cascade.** *(FIXED t654 — and it was not one path, it was eight; see "the stylesheets were on this machine the whole time" below.)* |

The second one had **no production callers** (tests only), so nothing had ever paid for that
limitation, and it is invisible to any fixture written with an inline `<style>` — which the first
version of `G_HOVER` was. The gate now puts the rules under test in an **external** sheet
specifically so the trap is inside its blast radius; the RED probe returns 800px rather than 100px,
because the base rule vanishes along with the hover rule.

`Page::recascade_all_sources` is the answer: recascade over the full source set without requiring
tree growth. It is extracted rather than inlined because **`:active` and `:focus` are exactly this
shape** and are the obvious next fills — they should not each rediscover this.

### The general form, worth carrying past this pseudo-class

**A cascade input can change while the tree does not.** Every incremental path here was built around
*tree* mutation — nodes added, attributes set — and answers "did the DOM change?" rather than "did
anything the cascade reads change?". State pseudo-classes are the first inputs that move without the
tree moving, and they will not be the last (`:focus-visible`, container queries, `@media` on a
resize). When adding one, the question is not "does it match" but **"what recomputes when it starts
matching?"**



## Focus was a DEAD-END WIRE, and `:focus` / `:focus-within` / `:focus-visible` are three questions

Tick 246, `G_FOCUS`. The shell has tracked focus for many ticks and publishes it into the JS world
through `Page::publish_view_state` — that is what backs `document.activeElement`. It never reached
the **style system**, so `:focus` answered a hard-coded `false` for the life of every page.

**This is the third instance of the same shape in five ticks**, and by now it is a named failure
mode rather than a coincidence: the parser's quirks verdict (tick 242, written and never read), the
`RuleIndex` case key (tick 243, computed then filtered away), and now focus. *The engine had the
answer and threw it away* — and **no capability probe can see it**, because the feature appears
present at every layer anyone would inspect. `document.activeElement` returns the right element. The
shell highlights the right control. Only the cascade was never told.

**What it costs is accessibility, not decoration.** The focus ring is the only thing telling a
keyboard user where they are on the page. And because authors spent twenty years writing
`:focus { outline: none }` to remove the ring *mouse* users did not want, on a great many sites the
only remaining cue is the author's own `:focus`/`:focus-visible` rule. With the pseudo-class never
matching, tabbing through those pages moves an invisible cursor.

### They are not one feature with three names

| selector | matches | the thing it is actually for |
|---|---|---|
| `:focus` | the exact element, **never an ancestor** | the control's own styling |
| `:focus-within` | the element **or any ancestor** | the expanding search box, the open combobox panel — the `<input>` takes focus, the `<div>` changes size |
| `:focus-visible` | focused **and** the ring is warranted | suppressing the ring on a mouse-clicked button, which is the noise that made authors strip `:focus` in the first place |

Collapsing `:focus` into `:focus-within` puts a ring around the whole form every time one field is
focused. Collapsing `:focus-visible` into `:focus` leaves the pseudo-class with no reason to exist.
Both are RED probes on `G_FOCUS`, and both fail on their own claim and nothing else.

**Only the caller knows how focus arrived**, so `Page::set_focus` takes `from_keyboard` rather than
guessing. `Dom::set_focused` marks **both** chains dirty for the same reason `set_hovered` does —
`:focus-within` matches ancestors, so an ancestor restyles without being the focus target.

`recascade_all_sources` (added for `:hover` one tick earlier) is reused unchanged, which is the
whole reason it was extracted rather than inlined.

## The two cascades drifted again — UA block margins (tick 268)

The first broad FID-SWEEP (observer, tick 267) put a number on the Phase-0 gap: **coverage 85.9%,
placement 4.5%** against a ≥75% exit bar. We render nearly every element Chrome does and place almost
none of them within 8px. Capability% (62%) cannot see this at all — every one of those features is
present and gated.

Its most tractable population is the **near-miss** group, and its signature is unmistakable:

| site | mdx | mdy |
|---|---|---|
| old.reddit.com | 0 | 12 |
| airbnb.com | 0 | 20 |
| en.wikipedia.org | 0 | 45 |
| usa.gov | 0 | 82 |

**Horizontal placement is exact. Only vertical drifts, and it grows with content density.** That is
not layout math — layout math errs on both axes. It is missing vertical *metrics*, applied per block,
accumulating down the document.

### Where it was

`apply_ua_defaults` (css/src/lib.rs, the `MinimalCascade` path) already set `ul`/`ol` to `1em 0` and
`body` to `8px`. The Stylo `UA_CSS` sheet — **the live path for every real page** — set neither. It
had `p`, `blockquote` and `h1`–`h6`, gave `ul, ol` a `padding-left` and no margin at all, and had no
rule whatsoever for `dl`, `dd`, `pre`, `hr`, `figure` or `body`.

So the two cascades had drifted apart on the property that decides where everything below a list
lands, and the one that was wrong was the one that runs. This is the third time this file's own
comments have had to say *keep in lockstep with `apply_ua_defaults`* — the `<source>` display bug and
the `<dialog>`/`<details>` pair were the earlier two. **A second cascade is a second source of truth,
and it silently becomes the stale one.**

### The numbers were measured, not recalled

Every value was read out of real Chrome (`createElement` + `getComputedStyle` per tag, headless) and
recorded in the gate:

```
body 8px all round   ul/ol/menu 1em 0 + pl 40px   dl 1em 0   dd ml 40px
pre 1em 0 (=13px, 1em of its OWN monospace font)  hr 0.5em 0
figure / blockquote  1em 40px                     NESTED ul   0
```

### The rule that makes it a fix rather than a trade

**A nested list gets NO vertical margin.** `ul ul, ul ol, ol ul, ol ol { margin-block: 0 }` is the
rule a from-memory implementation always misses, and it is load-bearing: giving every list `1em`
unconditionally fixes the top-level case and newly over-spaces every nested menu, sidebar and table
of contents on the web. Wikipedia's captured first divergence — `after #p-tb, element #n-randompage
is off by dy=-61` — is a *sidebar of nested lists*, i.e. exactly the shape that would have been
traded for a different error while the headline number improved. It carries its own assertion, and
deleting the rule fails only that assertion; the top-level ones stay green.

`blockquote` is the horizontal half of the same bug and worth stating precisely: ours said
`margin: 1em 0`, which does not *omit* the 40px indent — it **zeroes** it. A missing rule and a rule
that asserts the wrong value look identical in a diff and are not the same defect.

---

## `@media` was skipped, and it took a dozen properties with it (tick 273)

The minimal cascade's parser handled at-rules with one branch: capture `@font-face`, `skip_at_rule`
everything else. So **every rule inside every `@media` block was deleted at parse time.**

That should have been caught years of ticks ago, and the reason it wasn't is the interesting part.

### The bug and the test that covered it were about disjoint property sets

Under `--features stylo` — the shipping cascade — Stylo re-parses the sheet's own source with its
own parser and evaluates media queries correctly. So `display`, `width`, `color` and the rest of the
mainstream properties were **fine inside `@media`**, and `stylo_engine.rs` has a passing
`media_query_applies_by_viewport_width` test, written against `display` and `width`, that proves it.

But `cascade_via_stylo` ends with a second pass:

```rust
let minimal = MinimalCascade.cascade(dom, sheets);
for (node, cs) in map.iter_mut() {
    cs.visibility        = m.visibility;        // not exposed by Stylo's servo build
    cs.background_images = m.background_images.clone();
    cs.mask_image        = m.mask_image.clone();
    cs.border_style      = m.border_style;
    cs.text_shadow       = m.text_shadow;
    cs.object_fit        = m.object_fit;
    …
}
```

Twelve properties Stylo's *servo* build does not expose are recovered from the minimal cascade —
**the one that had just thrown the `@media` rules away.** The set of properties that failed and the
set a `@media` test naturally reaches for do not intersect. A green `@media` test sat in the same
repository as a total `@media` failure, and both were honest.

> **A property recovered from a second engine inherits that engine's bugs, silently and only for
> that property.** The recovery block is a list of twelve; it should be read as twelve places where
> the minimal cascade's correctness is load-bearing on the shipping path.

### What it cost

`.vector-dropdown .vector-dropdown-content { visibility: hidden }` — Wikipedia's closed-menu rule,
and the shape of every dropdown, popover, tooltip and autocomplete panel on the web, because
`visibility` is how you hide something that must stay animatable — lives inside an `@media` block.
So every one of those panels computed `visible`, stayed laid out at full size, painted over the page
and **swallowed clicks on the content underneath**. Tick 272 taught the a11y tree to prune
`visibility:hidden` boxes; it had nothing to prune, because nothing was ever marked hidden.

It is broader than one property. `Page`'s `wrap_media` deliberately wraps a conditional
`<link media="(prefers-color-scheme: dark)">` sheet in `@media … { }` so that *the cascade* decides
whether it applies, rather than that decision being reimplemented in a second place. With `@media`
skipped, every such sheet lost all twelve properties wholesale — every background image, gradient
and icon mask it defined.

### The fix, and why it evaluates at cascade time

`parse_rules_into` descends into `@media`, tagging each rule with the stack of enclosing conditions;
`Rule::media_applies` evaluates them during the cascade. Parse time would have been wrong: sheets
are parsed before `set_viewport_width` runs, and a resize must re-decide the query without
reparsing.

The conditions are a `Vec<String>`, not one stitched string, because nesting is conjunction and
there is no CSS syntax for the conjunction of a media *type* with a feature — `(screen) and
(min-width: 0)` is not a valid query, a media type cannot be parenthesised.

**Unknown media features evaluate FALSE.** The plausible wrong fix is "descend into `@media` and
apply what's inside", and it is not less wrong than skipping: it renders a print sheet on screen and
a dark-scheme sheet on a light display. The gate asserts both directions for exactly that reason —
`@media print`, `@media (max-width: 100px)` and `prefers-color-scheme: dark` must still not apply,
and a nested block whose inner query fails must not apply either.

The feature answers (`prefers-color-scheme: light`, `hover`, `pointer: fine`, `scripting: enabled`)
must agree with what `window.matchMedia` tells the page. A browser is allowed to be unusual; it is
not allowed to disagree with itself.

### Still skipped: `@supports` and `@layer`

Both still drop their contents in the minimal cascade, so the same twelve properties are still lost
inside them. `@supports` is the same defect with a different at-keyword and needs its own condition
evaluator; `@layer` additionally changes cascade *order*, which is a larger change than descent.
Written down rather than fixed, because the two need different work.

## One evaluator for `@media` and `matchMedia` (tick 275)

Media queries had **two** implementations: `manuk_css::media_matches` (the cascade) and a
hand-written one in the JS prelude backing `window.matchMedia`. Their unknown-feature defaults were
opposites — `false` in CSS, `true` in JS — so every feature the prelude's table omitted was a
guaranteed disagreement, and the prelude also could not parse `not`, `only`, or range syntax
(`(width >= 640px)`).

`matchMedia` is now `__matchMedia`, a host binding onto the cascade's own function. The prelude's
copy is deleted rather than synchronised, for the reason this file has now recorded three times:
**a second source of truth for one question silently becomes the stale one.**

The gate is a *consistency* gate and that shape is the reusable part: style N elements with N
queries, ask JS about the identical N, assert the two agree. It cannot be satisfied by a plausible
stub, it does not encode any particular answer (so it stays green if we later report a coarse
pointer or a dark scheme), and it fails precisely where a second implementation drifts — on the
features nobody thought to put in the second table. A gate that asserted specific values would have
tested `min-width`/`max-width`, which is the half a hand-written evaluator always gets right.

**And the same defect one layer up:** the JS prelude opened with a hardcoded
`var VW = 1280, VH = 720;`, so `window.innerWidth` reported 1280 on a page laid out at any other
width. `__viewportSize()` reads the cascade's own global, so `innerWidth`, `matchMedia` and `@media`
are three answers derived from one number. Two conformance assertions had been encoding the
disagreement — one loading at 800px and asserting 1280, the other saying "at 1280px wide" in its own
message while loading at 800. Rather than edit them to the new constants (which is how you retune a
gate to land your own tick), the load width is now threaded *through* the assertion, which is a
stronger claim than either constant and fails if the prelude is re-hardcoded.

## `@supports` / `@layer`, and answering a capability question honestly (tick 276)

The same skip as `@media`, with two more at-keywords. The interesting part is how `@supports` is
evaluated: **not** against a list of supported property names — that is a second source of truth
that goes stale the moment a property lands — but by parsing the declaration, applying it to
`ComputedStyle::initial()` and checking whether anything moved. The engine answers "do I support
this?" by trying it, so the answer maintains itself.

The probe is conservative by construction: a value that happens to equal the initial value reads as
unsupported and its block does not apply, which is exactly the pre-existing behaviour. It can be as
wrong as before, never newly wrong. An unparseable condition is `false`, matching `media_matches`.

`@supports` must be able to answer **no**: the author wrote the fallback for that case, and applying
both branches is worse than applying neither. `@layer` descends unconditionally and is knowingly
approximate — layered rules should lose to unlayered ones at equal specificity, which this cascade
cannot express — but deleting the contents was not approximate, it was absent.

**A statement at-rule has no block.** `@layer a, b;` ends at the `;`, so `rest.find('{')` finds a
*later* rule's brace and slices past the end. All four at-rule arms share one `block_open` that is
`None` unless the brace falls before the rule's end.

**At-rule keyword matching must be boundary-safe (tick 381).** The four arms matched their
keywords with `rest.len() >= n && rest[..n].eq_ignore_ascii_case(…)` — a BYTE-length guard, not a
char-boundary guard, so an at-rule named in multi-byte UTF-8 (netlify.com shipped one; found by
the tick-380 oracle crawl) landed the slice mid-character and panicked the engine. The arms now
share one `at_kw` closure over `str::get(..n)`: `None` on a non-boundary *is* "not this keyword",
so exotic or hostile at-rules fall through to skip-unknown — CSS's own error recovery — instead of
killing the process. RED proof: `multibyte_at_rule_names_never_panic` crosses every guarded prefix
length mid-character. The other prefix slices in the file index off `find()` results, which are
boundary-safe by construction.

---

## `CSS.supports()` — one question must not have two answers (tick 282)

`@supports` has been honest since tick 276: the cascade hands the condition to Stylo, and Stylo
really parses it. `CSS.supports()` — the JS half of the *identical* question — was
`function () { return true; }`. Measured on the same declarations before the fix:

| condition | `@supports` (Stylo) | `CSS.supports` (JS) |
|---|---|---|
| `display: grid` | applies | `true` |
| `notaproperty: 1` | does not apply | **`true`** |
| `container-type: inline-size` | does not apply | **`true`** |

Two sources of truth for one question, and the JS one wrong in the expensive direction. Progressive
enhancement is built on this call: a page asks whether a property works and, on yes, **hides its
fallback** and commits to the modern path. Told yes about `container-type`, a page throws away the
layout its author shipped and tested and renders the enhanced branch against a property the engine
ignores. A "no" would have left it looking right. `return true` is not a permissive default — it is
the answer that breaks pages, because it is only ever consulted by code preparing to act on it.

### The fix is a different door to the same evaluator

`stylo_engine::supports_condition` builds `@supports <condition> { … }`, parses it with the **same
`StyloStylesheet::from_str` the cascade uses**, and reads back the `enabled` flag Stylo itself
computed.

The obvious alternative — a list of supported properties — is a second source of truth by
construction. It is correct the day it is written, wrong the first time the engine gains or loses a
property, and silent when it drifts. This project's dominant bug class is exactly that drift (see the
UA_CSS / `apply_ua_defaults` pair), and the cheapest way to not have it is to not have the second
copy.

`manuk-js` has no CSS dependency and must not grow one, so the host installs the evaluator through a
`SupportsFn` hook — the same upward-call shape as `ReflowFn`. **With no hook installed the answer is
`false`**, deliberately: a build without a CSS engine cannot honour anything, and a conservative no
costs a page an enhancement while a wrong yes costs it its layout.

`and` / `or` / `not` were never implemented here and work anyway. That is the evidence the real
evaluator is being *reached* rather than imitated — a lookup table would have needed its own
boolean-expression parser and still would not have been the cascade's evaluator.

### Two things measured and pinned

**`display: grid` is pref-gated.** `Page::load` enables a Stylo runtime pref that a bare unit test
does not, so `supports_condition("display: grid")` is `false` standalone and `true` from a loaded
page — the same function, two configurations. They agree in every context where `CSS.supports`
exists, because JS only runs inside a page. Hence `G_CSS_SUPPORTS` asserts the agreement from inside
a real `Page::load`, and the unit tests stay off pref-gated properties rather than pinning a
configuration the browser never runs in.

**The limit of the proxy.** `CSS.supports` now mirrors what Stylo will *parse*, which stands in for
what the engine will *honour*. A property Stylo parses but layout ignores would still report true.
(`container-type` was the named watch item here while Stylo declined it; since tick 379 it parses
AND is honoured — see the container-queries section below — so the next gap of this shape has no
current example.)

## Container queries (tick 379): the sized re-pass and the source supplement

`@container` landed in two pieces, and the second was not the one tick 371's spec named.

**The re-pass.** A container condition is answered from the container's *laid-out* size, so the
cascade cannot know it on a first pass — the spec's own model is query-after-container-layout.
`restyle_and_layout` (engine/page) is the one join every restyle path now shares: cascade → layout →
if any sheet's source mentions `@container`, re-cascade with the pass-1 **content-box** sizes
(border and padding subtracted per CSS 2.1 §8, padding percentages against the containing block's
width) → re-layout. One re-pass per frame, never a fixpoint loop — a container-gated rule can change
the container's own size, and browsers converge on exactly this behaviour. On the unsized pass every
container-gated rule is held **off** wholesale: unknown must never style, which is also what keeps
feature-detecting fallback pages honest. Paths that restyle without a fresh layout
(`relayout_incremental`, external-CSS arrival) answer from the *previous* pass's geometry — the same
one-generation-behind model, corrected at the next restyle.

**The supplement.** Tick 371 measured "@container parses in stylo's servo build" and was wrong one
level deeper: the `ContainerRule` *type* ships, but rule_parser.rs guards the at-rule arm with
`cfg!(feature = "gecko")` — a compile-time cfg, not a pref, so the whole block is discarded as an
unknown at-rule before the cascade sees it. Vendoring stylo for one cfg is rung 4 cost; rung 3 is
`extract_container_blocks`: a comment/string-aware brace scanner lifts each `@container` block from
the raw sheet source, hands the prelude to **Stylo's own public** `ContainerCondition::parse` (full
grammar — names, cq units, and/or/not) and the body to `Stylesheet::from_str`, re-wrapped in any
enclosing `@media`/`@supports`/`@layer` preludes so their gates still apply. Nested `@container`
stacks conditions (levels AND, comma lists OR, unknown → off — Stylo's `container_condition_matches`
semantics, replicated over the per-rule condition stack in `RuleIndex`). Condition evaluation is
per-element at match time — `ContainerCondition::matches` walks `traversal_parent()` reading each
ancestor's `ElementData` primary style for `container-type`/`-name`, which is why the sized re-pass
also *publishes* every element's ComputedValues into the data store as the preorder walk computes
them, and answers the final size question through our `TElement::query_container_size`
(container-type axis-filtered: an `inline-size` container answers width only).

**Two prefs and a flipped answer.** `layout.container-queries.enabled` gates the `container-type`
property at parse time (rung 1, same as grid); it is now set on both the cascade path and
`supports_condition`, because a global pref set on only one of them made `@supports` order-dependent.
And the pinned `@supports (container-type: inline-size) == false` — the honest "no" of an engine
without container queries — flipped to `true` *with* the capability, per the honest-answer rule:
the gate follows the capability, never the reverse.

**Named residue.** `style()`/`scroll-state()` queries (machinery in place, sizes are the precedent);
`::before`/`::after` rules inside `@container` (the pseudo cascade path skips the supplement);
`@container` nested inside a *style rule* (`&`-relative selectors would mis-match standalone —
skipped, not guessed); supplement rules order after their sheet's own rules (a same-specificity base
rule written *after* its `@container` override wrongly loses; overrides overwhelmingly follow their
base); cq units (`cqw`/`cqi`…) outside `@container` blocks.

## `field-sizing: content` — a recovered property that must beat the hints (tick 388)

Baseline June 2026. `field-sizing: content` makes a form control size from its CONTENT — which in
this engine means the UA intrinsic-width hints (`<input size>` ~173px, `<textarea cols>` cols·8+13)
must stand down. Stylo 0.19 predates the property, so it rides the recovered-property route
(MinimalCascade parses it; `field_sizing_content` on ComputedStyle) — but unlike every other
recovered property it is copied onto the style INSIDE the Stylo walk, BEFORE
`apply_presentational_hints`, because its whole job is to veto a hint that fires in that pass;
the after-the-walk merge would arrive too late (the width is already Px by then). `@supports
(field-sizing: content)` flips to yes for free via the probe-style mechanism (the declaration
changes the probe from initial). Gate: `fieldsizing` in G_PROBE_CAPABILITIES measures BOTH halves
— the reference textarea keeps its ~333px cols width, the field-sized one hugs content (<150px).
RED-proven: recovery wire severed → fieldsizing:no. RESIDUE: the MinimalCascade-only (headless
fallback) path keeps its fixed 180×48 textarea default — its UA sizes are set before author rules
and there is no post-author hint phase there; the LIVE path is Stylo.

## `text-align: start`/`end` are LOGICAL — resolve them against direction, or the RTL web left-aligns (tick 414)

`text-align`'s initial value is `start`, and `start`/`end` are logical: they resolve to physical left/
right against the element's `direction` (`start` = left in LTR, **right in RTL**). `map_text_align`
mapped Stylo's `End`→`Right` unconditionally and `Start`→`Left` via the catch-all, so an RTL paragraph
with no explicit alignment — which is nearly every Arabic, Hebrew and Persian body paragraph — LEFT-
aligned, and `text-align:end` in RTL aligned right (both backwards).

The catch is ORDERING: `direction` is not known when `map_text_align` runs (the shipping path recovers
direction from MinimalCascade *after* the Stylo map produces the ComputedStyle). So the map now keeps
`start`/`end` as the logical `TextAlign::Start`/`End`, and `cascade_via_stylo_sized` resolves them per
node — via `TextAlign::resolve_physical(rtl)` — immediately after `cs.direction` is recovered. Layout
and `getComputedStyle` therefore still only ever see physical left/center/right/justify; the logical
variants never leak past the cascade. LTR is unchanged (`start`→left); RTL now right-aligns by default.
The resolution runs for every node (even those with no MinimalCascade entry). Gated by
`text_align_start_and_end_resolve_against_direction`, RED-proven (force `rtl=false` → the `dir=rtl`
default reads `Left`). Residue: `justify`'s last-line alignment does not yet follow direction; the
pure-MinimalCascade fallback path parses `start`/`end` but does not resolve them (it is not the shipping
cascade).

## Computed custom properties reach getComputedStyle from Stylo (tick 427)

CSS custom properties (`--brand`, `--gap`) are resolved by the SHIPPING cascade (Stylo), which inherits
them and expands `var()` — but until tick 427 none of that reached the CSSOM: `getComputedStyle(el)
.getPropertyValue('--x')` returned `''` because the computed-style JS object (`computed_style_js`) is
built from a FIXED longhand map and `ComputedStyle` carried no custom-property field.

The plumbing is three hops, all additive:

- `ComputedStyle.custom_properties: Vec<(String, String)>` (name includes the leading `--`).
- `to_computed_style` reads Stylo's `cv.custom_properties()` (`ComputedCustomProperties`), iterates it
  with `property_at(i)` across the inherited then non-inherited maps, and takes each UNIVERSAL
  (unregistered) value's `.css` string (`value.as_universal()`). This is where the cascade's inheritance
  and `var()` expansion have already happened, so a `:root { --brand }` is present on every descendant's
  computed values for free.
- `computed_style_js` emits a `__custom` object literal, and `getPropertyValue` short-circuits to it for
  any name starting with `--` (before the camelCase longhand lookup).

**Honest limits:** registered custom properties (`@property`, which are non-universal computed values)
are skipped — only unregistered `--vars` (the overwhelming common case) are exposed; and the link is
one-way (reading), so `el.style.setProperty('--x', …)` updates the inline declaration but does not
re-cascade into a later `getComputedStyle` read within the same synchronous turn.

## `:open` is taught to BOTH selector engines (tick 429)

There are two selector matchers and they must agree. The CSS cascade (Stylo) already knew `:open` via
`NonTSPseudoClass::Open` (stylo_dom.rs — matches the `open` content attribute), so `details:open { … }`
rendered. The querySelector/`matches`/`closest` engine (`manuk_css`, its own `Pseudo` enum) did not, so
`querySelectorAll('details:open')` returned nothing. Tick 429 added `Pseudo::Open` there — the enum
variant, the `pseudo_matches` arm (`el.attr("open").is_some()`, matching the attribute exactly as
`:checked` matches `checked` and `:muted` matches `muted`, not a runtime property), and the `"open" =>
Pseudo::Open` parser mapping. **Honest limit:** `<select>`'s open state is UI-only (no `open` attribute),
so it is out of reach here — the same runtime-property fence `:checked` documents. [[dom-semantics]]

## CSSStyleDeclaration: array-like + !important priority (tick 432)

A style declaration — inline `el.style` and computed `getComputedStyle(el)` — is spec'd as an array-like
object as well as a property map. Both now expose `.length`, `.item(i)` (the dash-case property NAME at
that index, `''` past the end), and the indexed getter `style[i]`. The computed snapshot's ordered name
list (`__n`) is the 49 standard longhands it exposes followed by the cascaded custom-property names, in
the same order `getPropertyValue` answers them.

Value and priority are separate. Inline `setProperty(k, v, 'important')` appends the flag;
`getPropertyValue` and a camelCase read (`el.style.color`) return the value WITHOUT it; `getPropertyPriority`
returns `'important'`; `cssText` keeps the raw text (the single source of truth — priority is stripped on
read and re-appended on write, never shadowed). Computed `getPropertyPriority` always returns `''`.
Gated by G_CSSOM_ENUMERATION.

### `contrast-color()` is a one-pref win: the resolution path was already wired (tick 466)

`contrast-color(<color>)` (CSS Color 5, Baseline 2026) is gated behind Stylo's own
`layout.css.contrast-color.enabled` pref (its OWN pref, not the shared `layout.unimplemented`). Off by
default → dropped at parse → the color declaration falls back. Flipping it on is an Option-1 pref flip like
`layout.grid.enabled`: Stylo then computes a `ComputedColor::ContrastColor(Box<Color>)` variant, which the
engine's `stylo_map` color path ALREADY resolves to the black/white companion through `resolve_to_absolute`
(the same call `background-color` uses at line 268). So no new resolution code — the value lands as
`rgb(255,255,255)` / `rgb(0,0,0)` in `getComputedStyle`. The lesson repeats: re-probe a `?`-unknown before
building it — `contrast-color()` was carried as unknown and worked end-to-end behind one pref.

### `scrollbar-color`/`scrollbar-width` are `engine="gecko"` — recovered from MinimalCascade, NOT a pref flip (tick 469)

The CSS Scrollbars-1 theming pair (Baseline 2024) — `scrollbar-width: auto|thin|none` and
`scrollbar-color: auto | <thumb> <track>` — looks like the same cheap Option-1 pref-flip win as
`user-select`/`color-scheme` (all three carried `servo_pref = "layout.unimplemented"` in a stray vendored
`stylo/` tree). **It is not, and the vendored tree is a decoy.** Manuk builds against the *crates.io*
`stylo 0.19`, whose `longhands.toml` marks BOTH properties `engine = "gecko"`. `data.py`'s
`declare_longhand` does `if engine and self.engine != engine: return`, so the servo build never generates
them at all — `cv.clone_scrollbar_width()` / `clone_scrollbar_color()` simply do not exist as methods, and
no pref can bring them back. This is the same wall `-webkit-line-clamp` hits.

So the fix follows the `-webkit-line-clamp` precedent exactly: parse the two properties in `MinimalCascade`
(`engine/css/src/lib.rs`) — `scrollbar-width` is a keyword; `scrollbar-color` splits at the first
paren-depth-0 space (so the commas/spaces inside `rgb(…)` do not fool the token boundary) and parses each
side with `values::parse_color`, falling back to `auto` on a malformed pair — then merge
`cs.scrollbar_width`/`cs.scrollbar_color` from `m` in the `stylo_engine` recovery loop, right beside
`scroll_snap_*`. `getComputedStyle` serializes both (`scrollbarWidth`, `scrollbarColor` as
`<thumb> <track>` rgb strings) plus their dash-case `getPropertyValue` routes. Scope, stated honestly like
`user-select`: this resolves the COMPUTED VALUE the CSSOM reports (what dark-mode themers feature-detect);
painting a themed scrollbar is a paint concern the engine does not model. RED proof: delete the two merge
lines and every element reads `auto`.

The lesson: **confirm which Stylo source actually compiles before assuming a `servo_pref` gate.** A
property gated by `servo_pref` reaches the servo build (pref-flippable); a property gated by
`engine = "gecko"` never does (MinimalCascade-recover it).

## `RuleIndex` was applied to ONE of the two matchers, and the other kept the O(elements × rules) defect for its whole life (tick 572)

`RuleIndex`'s own doc comment in `stylo_engine.rs` records the history: `cascade_one_element` used to
walk **every rule in every sheet for every element**, the cascade was therefore O(elements × rules), and
bucketing rules by their rightmost simple selector fixed it. That comment has been true and correct for
hundreds of ticks.

**`cascade_pseudo` sat a few dozen lines below it, doing exactly the thing the comment describes as
fixed** — and doing it *twice per element*, once for `::before` and once for `::after`. For every
element it re-descended all 69 stylesheets' rule trees, took a `read_with(guard)` lock read on every
nested rule list, **re-evaluated every `@media` query against a device that had not changed since the
last element**, and tested `sel.pseudo_element()` on every selector in the document — to find the handful
of rules that carry a pseudo at all.

**Measured**, on a wix.com snapshot (10,424 nodes, 1.8 MB of CSS across 68 `<style>` blocks), with the
`MANUK_CASCADE_PROFILE=1` phase timers added in the same tick:

| phase | before | after |
|---|---|---|
| `pseudo` | **9,000 ms (46%)** | **1,630 ms** |
| `element` (the indexed path) | 1,250 ms | 1,000 ms |
| `minimal` (the second, whole cascade) | 500 ms | 470 ms |
| `has` | 490 ms | 440 ms |
| `flush` (the `Stylist` that is never matched against) | 6 ms | 6 ms |
| **total per cascade** | **19,500 ms** | **11,300 ms** |

End to end the page load went **164.7 s → 101.8 s**. Note the multiplier that makes those numbers add
up: **the cascade runs ~8× per `Page::load`** on a scripted page (initial, post-script restyle, container
re-pass), so a second saved inside one cascade is eight seconds saved on the page.

**The fix is a hoist, not an algorithm.** `PseudoIndex::build` descends the sheet tree **once per
document**, evaluating `@media`/`@supports`/`@layer` once, and files each `::before`/`::after` rule into
one of two flat `Vec`s. Per element the cascade then iterates only those. Nothing about *matching*
changes — same selectors, same `ForStatelessPseudoElement` mode, same specificity, same source order —
which is what makes it safe: the work removed is precisely the work whose result could not vary by
element.

**Three things worth carrying.** **(1)** The general shape: *when you index one matcher, grep for the
others.* A fix applied to the path you were profiling leaves its twin untouched, and the twin has no
comment saying it is slow. **(2)** `@media` evaluation is **device-scoped**, so hoisting it out of the
per-element loop is free correctness-wise and was most of the win. **(3)** The profiler had to be built
first and it changed the answer: the ranked suspect list before measuring put a `PropertyDeclarationBlock`
merge quadratic first; the timers said pseudo, and the remaining **~7.8 s is still `unattributed`** —
inside the element walk, in `to_computed_style` and the recovery loops, and not yet broken down. Report
the unattributed remainder as its own line; an instrument whose parts silently sum to the whole cannot
tell you it is missing something.

## The loop that reads linear and is quadratic: `property_at(i)` over a chained map (tick 573)

Tick 572's phase timers said `to_computed_style` was **7,580 ms of an 11,300 ms cascade — 67%**, and a
sub-timer put essentially all of that in one block: the copy of computed custom properties into
`ComputedStyle::custom_properties`.

**The first hypothesis was allocation, and it was wrong.** 575 distinct custom properties inherited
across 10,424 elements is 1.44M `(String, String)` pairs per cascade — 2.9M heap allocations. Interning
both halves (575 distinct names, a small value vocabulary) took 7,580 ms to **7,461 ms: 2%.** Worth
recording as a negative result, because the arithmetic was persuasive and the measurement was not.

**The actual cause was the loop's shape.**

```rust
let mut i = 0;
while let Some((name, value)) = cp.property_at(i) { … ; i += 1; }
```

`property_at` forwards to `CustomPropertiesMap::get_index`, whose entire body is
`self.0.iter().nth(index)` — under a comment in Stylo that reads *"FIXME: This is O(n) which is a bit
unfortunate."* Indexing it in a `while` loop is therefore **O(n²)**, and nothing at the call site says
so: the only visible operation is `i += 1`. Custom properties **inherit**, so on a design-token page
`n` is the whole token vocabulary at *every* element. Switching to `iter()` — one linear walk —
took the phase to **233 ms (32×)**, the cascade to **2,570 ms**, and, because the transient allocation
went with it, wix.com's whole load from **101.8 s / 1308 MB to 26.5 s / 471 MB**.

> **The general shape: an index-addressed API over a linked or chained structure turns any `for i in
> 0..len` into a quadratic.** Grep for `get_index`, `nth`, and `at(i)` in loops. The give-away is that
> the *callee* documents its cost and the *caller* cannot see it.

## Custom properties are copy-on-write with a PARENT CHAIN, and the chain yields shadowed names twice (tick 573)

Fixing the above exposed a correctness bug underneath it, which had been there all along.

Above 8 own properties, `CustomPropertiesMap::should_expand_chain` stops copying into the child and
starts a **parent chain**: the redefining element's map holds only its own entry and points at its
ancestor's. The chain iterator then yields the shadowing entry **and, later, the ancestor's entry for
the same name.** Every consumer of `ComputedStyle::custom_properties` takes the *last* write — the
`__custom` object literal `getPropertyValue` reads, and the `item(i)` enumeration — so:

- `#shadow { --brand: green }` under `:root { --brand: red }` computed to **red**, and
- `--brand` **enumerated twice**.

The fix is *first occurrence wins* (own precedes ancestor in the chain walk), deduped through a reused
thread-local scratch set. **It predates the rewrite** — the original `property_at` walk produces the
identical wrong answer on the same fixture, which is worth verifying rather than assuming when a
refactor and a bug show up together.

**Why it hid for so long: the threshold.** Below 9 custom properties no chain forms, the child just
copies, and the bug cannot occur. `G_COMPUTED_CUSTOM_PROPERTIES`'s fixture had **two** tokens. A gate
whose fixture sits below the threshold of the mechanism it tests is green for a reason that has nothing
to do with the code being right — the fixture now declares twelve, deliberately, with a comment saying
why.

Found in passing by the same gate: `getComputedStyle(el).length` was the literal `50 + customs`, while
the standard-property list it counts had grown to **52** — so the last two custom properties were
unreachable through `item(i)`. The count is now derived from the list. **A hand-maintained count of a
list three hundred lines away drifts the moment someone appends to it, and nothing fails loudly.**

## Our matcher merged winners by `(specificity, order)` — the cascade's FIRST sort was missing (tick 575)

The engine does not use the Stylist's cascade. It builds its own `RuleIndex` from the parsed sheets,
matches candidates itself, and merges the winning declaration blocks in ascending priority before one
`compute_for_declarations` call. That merge sorted on:

```rust
winners.sort_by_key(|(spec, ord, _)| (*spec, *ord));   // ← origin is not in here
```

CSS Cascade §6 sorts by **origin and importance first**, specificity only *third*, document order
*fourth*. Leaving origin out is not a subtle ordering nicety — it inverts the single most common
override on the web:

```css
* { margin: 0; padding: 0 }     /* author, specificity (0,0,0) */
body { margin: 8px }            /* OUR UA sheet, specificity (0,0,1) — and it WON */
```

That is the first rule of Tailwind's preflight, of Normalize, and of every hand-rolled reset since
2004. **A reset is written with the weakest possible selector on purpose**, which is exactly the shape
that loses a specificity tie-break — so a UA sheet one origin too high beats the rules that exist to
override it. The 8px body margin was the *smallest* instance: every rule in `UA_CSS` has a type or
descendant selector, so `ul, ol { padding-left: 40px }` and `blockquote { margin: 1em 40px }` survived
the same reset. Measured against live Chromium at tick 556: Chromium `body [0 0 1200×92]`, ours
`[8 8 1184×91]`.

**The fix is two halves and both are needed.** The sheet is parsed with `Origin::UserAgent` instead of
`Origin::Author` — so the origin is *readable* off `contents.origin` — and `IndexedRule`/`PseudoRule`
carry an `origin_rank` that leads the sort key: `(origin_rank, spec, order)`. Changing only the parse
origin does nothing, because **the Stylist's own origin machinery is bypassed**: it is our index that
decides this page. That is the trap worth remembering here — *declaring* the origin to a dependency you
do not cascade through is a comment, not a behaviour change.

> The old comment read *"the UA sheet is matched first (lowest priority); author rules override it"*,
> and it was true of the append order and false of the outcome. **Document order is the cascade's LAST
> tie-break, not a way to express priority.** Anything that means "always loses" has to be a sort term.

Declaring the origin also makes the `!important` ordering expressible (a UA `!important` outranks an
author `!important`) — an ordering no amount of sheet re-ordering can produce. `UA_CSS` contains no
`!important` today, so nothing depends on it yet.

**The guard that matters more than the feature.** Winning this by *weakening* the UA sheet would be a
far worse browser than the bug: every unstyled document loses its metrics at once. `G_CASCADE_ORIGIN`
therefore asserts the same UA rules on a second document with **no author rules at all** — `body` 8px,
`ul` 40px, `blockquote` 40px — alongside the reset case, plus that specificity still decides *within*
the author origin and that author `!important` still beats author normal.

## `@supports` answered "does it PARSE", and one shared pref made 31 unread properties parseable (tick 576)

Tick 282 gave `@supports` and `CSS.supports()` a single evaluator. Both then answered the same
question — *"does Stylo parse this declaration?"* — which is the spec's definition and is right for
every property whose parseability we did not go out of our way to change. **We went out of our way.**

Stylo's servo build hides **35 longhands** behind one shared pref, `layout.unimplemented`. The cascade
flips it on because **four** of them are genuinely rendered here:

| ungated and RENDERED | how it reaches `ComputedStyle` |
|---|---|
| `user-select` | `clone_user_select` (the pref was flipped for this, tick 464) |
| `color-scheme` | `clone_color_scheme` |
| `mask-image` | the **MinimalCascade recovery block** — every icon is a mask |
| `text-overflow` | the MinimalCascade recovery block — the `…` on a clipped title |

The other **31** — `backdrop-filter`, `view-transition-name`, `offset-path`, `contain`, `zoom`, the
eight `corner-*-shape`s and the whole `mask-*` family — became *parseable* as a side effect. The
flip's own comment said this was harmless: *"we consume a fixed set of computed values via explicit
`clone_*` calls, so enabling the other properties it also ungates changes nothing we read."*
**`@supports` reads them.** So a page writing

```css
@supports (backdrop-filter: blur(8px)) { .bar { background: rgba(255,255,255,.4) } }
```

got a **yes**, threw away the opaque fallback it had written for browsers that cannot blur, and put
its text unreadably over a photograph. A false "yes" is strictly worse than a "no", because a "no"
keeps a working page — that is the whole reason `honest-answer-is-not-a-fixed-answer` is a standing
rule here, and this is the largest instance of it found so far.

**Note the grep that under-counts.** Two of the four honest properties arrive through the
MinimalCascade recovery block, not a `clone_*` accessor. Deriving the list from `clone_*` alone gives
**two**, and would have made `mask-image` and `text-overflow` answer "no" — trading a false yes for a
false no on two properties every page uses. The measurement that works is *"does it reach a
`ComputedStyle` field?"*.

### Composition is the whole difficulty, and it is delegated rather than re-implemented

`not (backdrop-filter: blur(1px))` must be **true** while `(backdrop-filter: blur(1px))` must be
**false**. Any filter shaped like *"does the condition text mention a banned property?"* gets that
backwards, and `and`/`or` mixtures make it worse. So `honest_supports` evaluates nothing:

1. walk the parsed `SupportsCondition` tree;
2. replace every `Declaration` naming a `PARSE_ONLY_LONGHANDS` property with `-manuk-not-a-property: 1`;
3. if nothing changed, return `None` — Stylo's own verdict already stands, and no second parse is paid;
4. otherwise serialise the rewritten tree and hand it **back to Stylo**, which already knows how
   `and`, `or` and `not` compose.

RED-proven on both halves, deliberately: delete `backdrop-filter` from the list and `bdf:false` fails
(the list is load-bearing); make step 4 return a bare `false` instead of re-asking Stylo and
`notvtn:true` fails (the tree rewrite is load-bearing, not decoration).

**It is a denylist, not an allowlist, and that direction is chosen.** A property Stylo adds behind
this pref in a future bump should default to *unsupported*: a missing denylist entry yields a false
"yes" that costs a page its fallback, while a stale entry yields a false "no" that keeps the page
working. The same verdict is applied at all three places the cascade descends into an `@supports`
block — `RuleIndex::add_rules`, `PseudoIndex::collect`, and the per-element `match_rules_recursive` —
because `CSS.supports()` and the cascade disagreeing about one declaration is the tick-282 bug one
level down: whichever the page consults, it gets a different browser.

## CSS Color 4 — `oklch()`, `lab()`, `color()` and `color-mix()` all work, and nobody had asked (tick 579)

Surface audit #31 filed this `unknown` rather than `missing`, on the grounds that **a grep is not a
measurement when the capability lives below you**: `oklch` and `color-mix` appear nowhere in `engine/`,
but Stylo is a *dependency* and may resolve them without this repository ever naming them.

It does. All of it, and to the integer:

| declaration | resolved sRGB |
|---|---|
| `oklch(0.7 0.15 250)` | `(75, 163, 247)` |
| `color-mix(in oklab, red 50%, blue)` | `(140, 83, 162)` |
| `color-mix(in srgb, black 50%, white)` | `(128, 128, 128)` |
| `lab(50% 40 30)` | `(187, 88, 70)` |
| `color(display-p3 1 0 0)` | `(255, 0, 0)` (clipped — P3 red is outside sRGB) |

Four of the five reproduce a from-scratch derivation off the CSS Color 4 matrices **exactly**, and the mix
honours its percentage (`black 25%` → `(191, 191, 191)`, which is 0.75 × 255).

**Why this mattered enough to spend a tick measuring.** Tailwind v4 does not *offer* `oklch` — it **emits
it by default**: every `text-slate-700` and `bg-blue-500` is an `oklch()` literal, and every opacity
utility (`bg-blue-500/50`) compiles to `color-mix(in oklab, … 50%, transparent)`. Had this been missing,
a large and rapidly growing population of sites would render in the fallback colour, and the failure would
have been *silent* — wrong colours, no error, nothing for a box-comparing gate to catch.

> **The generalisable half.** The fifth "already built" phantom this project has caught, and the first
> found by asking the right question about a *dependency* rather than about our own code. The map's status
> vocabulary earns its keep here: `missing` claims a measurement, `unknown` admits there isn't one, and
> collapsing the two would have filed this as a work item worth weeks.

**⚠ And the gate was wrong before the engine was.** It was first written asserting
`oklch(0.7 0.15 250) == (57, 137, 217)` — a number **recalled rather than derived** — and it failed
against an engine that was exactly right. A gate whose expected value came from memory tests the memory,
and it fails in the direction that costs most: a red gate on correct code invites someone to "fix" the
code. The values are now derived, with the derivation written into the gate's header so the next reader
can re-run it instead of trusting it.

## The `:has()` supplement re-filtered the stylesheets for every element (tick 580)

Third instance of the class t572 (`cascade_pseudo` re-walking 69 sheets twice per element) and t573
(`property_at(i)` indexing a linked structure in a loop) established: **work that depends only on the
stylesheet, done once per element.**

`apply_has_rules` walked, *per element*, every rule of every `:has()`-carrying sheet — re-evaluating each
rule's `@media` and re-asking each selector whether it was relative. Neither answer can change between
elements. `collect_relative_rules` now lifts the `:has()` selectors out once per cascade and the
per-element pass walks only them, so the cost is proportional to *the number of `:has()` selectors on the
page* — which is small, and is the number that should have governed it all along.

### The measurement, and the first attempt varied the wrong `n`

Quadrupling the rules **within** a sheet (600 → 2,400, same elements) moved the cascade barely at all: the
inner scan short-circuits on `has_relative()` and costs roughly 0.2 ns an iteration. On that evidence the
hypothesis looked refuted.

Multiplying the **sheets** is what costs, because the per-element loop is `for sh in &has_sheets` and pays
the whole scan again for each one. Same page, same element count, the only difference one `:has()` rule per
sheet, 60 sheets × 18,125 elements:

| | cascade, `:has()` absent | cascade, `:has()` present | delta |
|---|---|---|---|
| before | 19.66 / 20.66 / 21.90 ms | 22.74 / 24.29 / 23.82 ms | **+3.1 / +3.6 / +1.9** |
| after | 27.71 / 29.78 / 22.49 ms | 23.79 / 26.56 / 18.62 ms | −3.9 / −3.2 / −3.9 |

The **sign flips** in the identical setup; the consistent ~+14% is gone. (The absolute numbers wander with
machine load and with which page is measured first — a cold process penalises whichever page leads. Only
the within-run delta is meaningful, which is why both orders were run.)

> **A ratio is not a measurement until you know which `n` it is over.** This project's own standing lesson,
> and it nearly buried the fix: the first experiment scaled rules-per-sheet, saw nothing, and would have
> reported the lead dead. The cost was there — under a different variable.

### The hoist's real hazard is ORDERING, and the gate that caught it caught itself first

Source order used to be implicit in "sheet by sheet, rule by rule". It is now an explicit `order`, and a
per-sheet stride keeps a later sheet's rules sorting after an earlier sheet's. `G_HAS_CASCADE_ORDER` asserts
that, plus that specificity still beats source order and `!important` still beats both.

**Its first fixture could not detect the defect it was written for.** Both competing rules sat at
within-sheet index 0, so dropping the stride made them tie — and a *stable* sort preserves emission order,
which happens to be the right answer. The RED patch left the gate green. Moving sheet 1's rule to index 3
and sheet 2's to index 0 makes the stride the only thing that can order them, and the RED patch then fails
with the earlier sheet winning. **An assertion whose fixture cannot reach the mechanism is green for a
reason unrelated to the claim** — met here while writing the gate meant to catch exactly that.

## The SECOND category of `@supports` lie: parsed natively, never rendered (tick 591)

Tick 576 fixed `@supports` for the 35 longhands behind Stylo's `layout.unimplemented` pref, and scoped the
denylist to **that pref's property set** — the shape the bug presented in, and **one category too narrow.**

The general defect is *"Stylo parses it, we never consume it, and `@supports` says yes."* The pref is only
**one** reason a property lands in that state. These need no pref at all — the servo build parses and
computes them natively, and nothing reads the result:

| property | % of page loads |
|---|---|
| `filter` | **51.9%** |
| `clip-path` | **43.8%** |
| `backdrop-filter` | 34.3% |
| `isolation` | 18.0% |
| `mix-blend-mode` | 12.9% |
| `writing-mode` | 8.3% (+5.4% prefixed) |

Each verified by **all three routes** a computed value can reach us: no `clone_*` in `stylo_map.rs`, no
`ComputedStyle` field, and no entry in the MinimalCascade recovery block. Checking one route would have
under-counted — `mask-image` and `text-overflow` arrive through the recovery block, which is exactly how
t576's own honest set was nearly got wrong.

### Why `filter` is the costliest member, and worse than t576's cases

**There is no cascade-level workaround for a blur.** `appearance: none` turned out to be a no-op in this
engine because plain author CSS already achieves what it asks for (tick 590) — a page loses nothing by our
saying no. A page that wants a frosted-glass bar has no such path: it writes
`@supports (backdrop-filter: blur(8px))`, is told **yes**, drops the opaque background it shipped for
engines that cannot blur, and puts its text unreadably over a photograph.

> **A false "yes" is strictly worse than a "no"**, because a "no" keeps a working page. Stated at t576,
> still true, and still being discovered in new places.

`UNRENDERED_LONGHANDS` is deliberately **separate** from `PARSE_ONLY_LONGHANDS`. Both answer the same
question — *do we render this?* — and differ only in *why* the property became parseable, so each carries
its own evidence and can be shortened independently as capabilities land. Delete a line the moment its
property is genuinely rendered; `G_SUPPORTS_HONESTY` holds the answer either way.

### The lesson, which this session paid for four times

**A fix scoped to the shape the bug presented in is one category too narrow.** t578: the
break-opportunity bug was in *three* text-assembly consumers, not one. t581: gates live in *seven*
directories, not one. t588's own standing rule had the blind spot it was written to cure. t591: t576's
denylist covered one of *two* categories. The cheap version of the question is **"what else reads this, or
is in this state, and does it have the same problem?"** — a grep for the **class**, not the symptom.

## `undefined` from `getComputedStyle` is not a missing feature — it is a thrown exception in the caller (tick 596)

Ticks 592-595 made `filter`, `backdrop-filter`, `clip-path` and `mix-blend-mode` render. All four
still read back `undefined`, and **that is the worse half of the same lie, not the smaller one:**

```js
if (getComputedStyle(el).filter.indexOf('blur') !== -1) { … }   // TypeError → frame dead
```

A missing *rendering* degrades a page. A missing *string* stops the script. The CSSOM contract is
that every supported property is a string **always**, and `"none"` is a perfectly good answer. This is
the third sighting of one defect class — t576 found it on `getPropertyValue`, t590 re-found it on
`appearance` — and here it was four properties wide.

**The unset value must be the CSS initial keyword, never `""`.** An empty string is falsy, so
`if (cs.filter)` takes the wrong branch silently; `"none"` and `"normal"` are truthy and correct.
That is why `G_COMPUTED_VISUAL_EFFECTS` asserts `typeof === 'string'` *before* it asserts any value:
a test that only checked the *set* case would pass while every page that feature-detects before
styling still died, and that is most of them.

**Three places one set of properties lives, and they drift independently.** The object literal, the
`STD` name list behind `length`/`item(i)`, and the `getPropertyValue` dash→camel map. A property in
the first but not the second is enumerable-invisible (the drift `G_COMPUTED_CUSTOM_PROPERTIES` caught
when `length` was a hand-maintained `50` against a list of 52). And the map is not optional for
prefixed spellings: `getPropertyValue`'s fallback auto-camelCases, which turns `-webkit-filter` into
`WebkitFilter` — not a property, so it returns `""` **silently**. The gate asserts all three routes
agree, because `getPropertyValue` and the property disagreeing about one declaration is the tick-282
bug wearing new clothes: whichever the page consults, it gets a different browser.

Prefixed aliases resolve to the *same* value rather than to a duplicate serialization, so a page that
feature-detects on `webkitFilter` and then reads `filter` (or the reverse) cannot find a hole.

## 86 of 95 — the throw-class defect was never four properties wide (tick 597)

t596 closed `undefined`-from-`getComputedStyle` for `filter`, `backdrop-filter`, `clip-path` and
`mix-blend-mode`. A probe of **95 commonly-read properties then found 86 still returning
`undefined`** — t596 had fixed **4 of ~86**. This session's recurring failure at its widest yet: *a
fix scoped to the shape the bug presented in is one category too narrow — grep for the class.* The
cheap version of the question was always "how many properties are in this state?", and it takes one
probe to answer.

Almost every one of those 86 already had a true computed value sitting in `ComputedStyle`. **The
engine was rendering them and refusing to say so.**

### One list, three consumers — the structural half

The properties used to live in **three** places: the 60-argument `format!`, the `STD` name array
behind `length`/`item(i)`, and the dash→camel map for `getPropertyValue`. Those drift independently,
and they had: `length` was once a hand-maintained `50` against a list of 52, so the last two
properties were unreachable through `item(i)` and nothing failed loudly.

Everything added at t597 is emitted from **one** function that produces the object slots *and* the
enumeration names. A property added there **cannot** be enumerable-invisible, which is why the gate
can assert `lenMatchesNames` as a general property rather than spot-checking names.

### Two serializations the obvious implementation gets wrong

**`border-*-style` is not readable off `BorderStyle`.** That enum has no `none` or `hidden` variant —
the cascade collapses both to a **zero width** (`stylo_map` does exactly that). A naive `match` on the
enum therefore reports `solid` for *every element on the page*, including the overwhelming majority
with no border at all. It has to be recovered from the width. This is the `two-cascades` hazard in
miniature: the enum is not the source of truth for the question being asked.

**An unset `letter-spacing` is `normal`, not `0px`.** The difference is observable — `normal` permits
the font's own kerning and `0px` suppresses it — so printing the `f32` unconditionally reports a value
the author never wrote. Same for `word-spacing`.

The properties that remain `undefined` are now *measured* rather than assumed: they are the ones this
engine genuinely does not compute (grid shorthand forms, `transition`/`animation`, `contain`, the
logical-property spellings, `perspective`, `zoom`, `clip`). An honest `undefined` for an unimplemented
property is a different thing from an `undefined` for a rendered one — and only the second kind was
ever the bug.

## A FALSE NO costs a page its enhancement, exactly as a false yes costs it its fallback (tick 601)

Four ticks of this project have been about the danger of `CSS.supports` saying **yes** about
something we cannot render: the page drops the fallback it shipped and breaks. Tick 601 found the
mirror, and it had been live the whole time.

**`zoom` works. Fully. And we were denying it.** It sat on `PARSE_ONLY_LONGHANDS`, so
`CSS.supports('zoom', '2')` answered false — while a `zoom: 2` 50px box laid out at 100px, its
`font-size: 10px` computed to 20px, and a 20px child came out at 40px. Geometry, typography, and
inheritance: a complete implementation.

**It works because Stylo applies zoom inside its own length computation** (`effective_zoom`), so it
takes effect without this engine reading a `zoom` field at all. That is why a source-grep found
nothing — `grep clone_zoom` returns empty, and the capability is still there. **A grep can only find
capabilities this engine implements; it is blind to the ones the borrowed library implements for
us.** Only a behavioural probe can see those, which is the argument for measuring rather than reading
the code, stated one more time.

The general rule, which `honest-answer-is-not-a-fixed-answer` implied and did not spell out:

> The rule is not "never say yes to something you cannot do". It is **the answer must match the
> engine** — and it is violated symmetrically. A false yes costs the page the fallback it shipped
> and tested; a false no costs it the enhancement it wrote. Both are a page rendering worse than it
> was built to.

The same probe found the other direction in the same run: **`text-justify` was a false yes** —
parsed natively by Stylo, read by nothing here — and joined `UNRENDERED_LONGHANDS`. One probe, two
corrections, pointing opposite ways.

⚠ And the reason to check the whole denylist rather than one entry: `zoom` was on a list of 30
properties assumed unimplemented *as a group*, because the pref that ungated them was flipped for
four others. The list was right about 29 of them (`counter-increment`/`counter-reset` were re-checked
behaviourally in the same tick — `content: counter(x)` renders nothing, so they stay). It took a
measurement to find the one it was wrong about.

## The stylesheets were on this machine the whole time (tick 654)

Two defects in how external CSS reaches the cascade. Both had the same visible result — **the page
renders naked**, every box a full-width UA block in `serif/16`, the document several times too tall —
and neither had a symptom, because *nothing failed*: the sheets downloaded fine, and the layout of an
unstyled document is a perfectly successful layout of the wrong input.

### 1. The load deadline threw away sheets it had already downloaded

`finish_loading` wraps the whole phase sequence in the load budget as a **hard deadline, dropped
wherever it runs out, including in the middle of a fetch.** The justification is explicit and, for
most phases, correct: *"a dropped future loses that phase's ENHANCEMENT and never a half-mutated
document."* The stylesheet phase's internal order made that false:

```text
  fetch every <link> sheet  →  @import walk (up to 3 network rounds)
                            →  @font-face `src` fetches (SEQUENTIAL, per source, per face)
                            →  ...and only THEN cascade
```

The apply sat at the bottom, so the phase's **primary artifact was hostage to its enhancements.**

`keirin.jp` is the measured case, and the log is the confession: nine sheets, 375KB, all nine
`stylesheet applied` at **+0.2s** — then eleven and a half seconds inside font-awesome's per-face
`src` ladder — then `load budget of 12.0s exhausted mid-phase`, with the future dying two stages
above the cascade. Result: coverage **98%** against SHAPE **2.1%**. *We rendered every element
Chromium did, and put almost none of them where Chromium put them.*

**Fix:** call `apply_stylesheets` where the top-level sheets are COMPLETE, before the phase returns
to the network. Imports and fonts then arrive as the enhancements they are, and the apply at the
bottom re-cascades only if they moved the fingerprint — so a page with no `@import` and no new face
pays one hash rather than a second cascade, and a page that gets a late face pays one more cascade,
which is exactly what a browser does when a webfont lands. **SHAPE 2.1% → 40.7%.**

> **A page with no author CSS is not a degraded page; it is a different page.** When a phase is
> cancellable, ask which of its outputs is the *artifact* and which are the *enhancements*, and commit
> the artifact at the point where it is complete. "Fetch everything, then apply once" is only safe if
> the phase cannot be interrupted — and this one is designed to be.

### 2. One re-cascade rule, nine implementations, eight of them wrong

`Page::external_css` exists so that a **later** cascade can rebuild the full sheet list. Nine sites
rebuild one. Two of them did it right, in private, each with its own hand-rolled copy of the body —
and the comment at one of them already stated the rule in full: *"rebuilding from it would strip every
`<link>`ed stylesheet from the page, which is a far worse bug than the one being fixed."*

The other seven, plus `relayout_incremental`, rebuilt from
`MinimalCascade::collect_style_elements` — which sees inline `<style>` and **nothing else**:

```text
  resolve_fetch · dispatch_click · deliver_ws_event · deliver_fetch_stream
  deliver_message · fire_popstate · run_deferred_scripts · relayout_incremental
```

A resolved `fetch`/XHR, **a click**, a WebSocket frame, a streamed body chunk, `postMessage`,
`popstate` — any interaction at all, on any page whose CSS lives in a `<link>`, which is essentially
every page on the web — re-styled the document against UA defaults. All nine now call
`Page::all_sheets()`, which is the rule's only implementation.

*One rule with N implementations is one rule that is wrong N−1 times*, and the two that were right
are why it survived: a correct private copy fixes nothing and removes the pressure to find the
others. **When a comment states a rule, the rule wants to be a function.**

⚠ **The ninth site, named and not fixed.** `forced_reflow` — the synchronous layout a JS geometry
read (`offsetWidth`, `getBoundingClientRect`) forces mid-script — has the same defect, so those reads
answer from UA-default styles. It cannot use `all_sheets()`: it runs off a `*mut ReflowCtx` installed
at **19 call sites** with no route to `external_css`, and threading a raw pointer to a `self` field
across those while `&mut self.dom` is live is an aliasing question that deserves its own tick rather
than a ride on this one.

### The instrument note: read the ORACLE's column, not the score

The score said *"placement is bad on keirin"* for 42 ticks. The thing that named the organ in one
reading was the divergence text — `{Meiryo UI/20}` on Chromium's side against `{serif/16}` on ours,
with `x=8` and `width=1184` — because `serif/16` and the UA body margin are not values anybody
*writes*. They are what you get when no author sheet applied at all. **A per-element font column
turned a fidelity percentage into a named subsystem**, which is the job t563 added it for.

And the control run earned its keep on the same tick: the three other scored sites showed `welt.de`
reading-order 0 → 2 against the previous sweep, reproducible three times — a term that counts
*sites*, so it read as a real regression. Re-running with the one line disabled, on the same live
content minutes apart, showed reading-order 2 **both ways** (and SHAPE 66.6% → 67.2% in the fix's
favour). `welt.de`'s scored population has read 2957 / 3149 / 3060 across three sweeps: **against a
live corpus, the previous sweep is not a baseline — only a same-hour control is.**

## The CSSOM as a view over the element's text (tick 665)

**`styleEl.sheet` was `undefined`, and `undefined` is not the spec's absent value.**
`HTMLStyleElement.sheet` is typed `CSSStyleSheet?`, so the guard every consumer writes is
`if (el.sheet === null)`. Against `undefined` that guard is **false**, and the code proceeds into the
thing it just checked for. Worse, `typeof CSSStyleSheet === "function"` was **already true** — the
false-presence shape, where every feature detect passes and the page walks into the gap one line
later. `www.agoda.com` rendered blank behind exactly this: `insertRules → getTag → this.sheet`, then
`.length` on `undefined`.

### Why the deferral was priced wrong

The lever's own note said the tick-283 shim was reverted for wanting a **native accessor to reach the
cascade**. It does not need one, and that was settled by a probe before a line was written:

```text
  el.textContent = '#a { width: 222px }'      #a: 111px  ->  222px
```

**A `<style>`'s own TEXT is the cascade's source of truth** (`collect_style_sources`), so writing
`textContent` re-cascades through machinery that already exists. That makes the CSSOM a **view over
the element's text** instead of a parallel data model that has to be kept in sync with one — and the
parallel model was the subsystem. *When a capability is deferred as "needs X", re-price X: the reason
was true about one implementation, not about the capability.*

### The two things that are easy to get wrong

**Rule splitting must track brace DEPTH.** `@media screen { #b { … } }` is ONE rule. A naive
close-brace split reports the right count for flat sheets and silently shreds every responsive one —
which is the shape of bug that passes a small fixture and fails the web. Validated as a page script at
zero build cost before it went near the prelude.

**`insertRule` past the end must THROW `IndexSizeError`.** A CSS-in-JS runtime uses that to discover
its own bookkeeping is wrong; clamping silently hides a library bug inside a browser bug.

### Scope, stated rather than implied

`<style>` only. `<link>.sheet` stays `undefined` — what it is today — and deliberately **not** `null`,
because for an applied linked sheet `null` is a lie that reads as honest. Same reasoning as tick 663's
refusal.

### The gate asserts a BOX, and caught its own fixture

An `insertRule` that returns cleanly and changes nothing satisfies every shape test a library performs
and still renders the wrong page — *a gate that does not measure what the user feels reports green
while the user suffers*. So `G_CSSOM_SHEET_BRIDGE`'s central assertion is that `#a` is **456px** after
a rule is inserted at runtime into a `<style>` that did not exist at parse time. It also asserts
`deleteRule` **un**-cascades (or "the rule applied" could just mean "text was appended and never
removed") and that the authored sheet is untouched (or a bridge rewriting the wrong element's text
would pass everything else).

⚠ Its first draft read `document.styleSheets` *before* the injection, got 1, and asserted 2 — the gate
failed on its own fixture. Reading it after is a strictly stronger claim (**liveness**: a sheet
appended after load is in the list with no cache to invalidate), and it is now asserted as
`before == 1 && after == 2`.

### What it did and did not buy

```text
  agoda   before   render-failed   · TypeError … e/this.sheet<
          after    thin-overlap-5  · no `this.sheet` throw anywhere in the log
```

The throw is eliminated and the row changed class — but `thin-overlap-5` means *the oracle built the
page and we did not*, which the instrument itself labels as still **ours**. One blocker removed and
the next one visible behind it. That is not "agoda renders", and it is recorded as what it is.

## The catch-all that answers `inline` (tick 699)

`map_display`'s final arm is `_ => Display::Inline`. It has now caused three separate bugs, and the
comment documenting the first one sits directly above it.

**Why this particular catch-all is expensive.** It never errors and never logs, and `inline` is a
*plausible* answer — an inline box still participates in layout. So a display keyword nobody mapped
does not present as "unsupported value"; it presents as a **subtle geometry bug somewhere else on the
page**, usually as a container that shrank to its content or a subtree that stopped being laid out the
way its parent expected. `display: contents` was the first (an inline wrapper that stayed in the box
tree and collapsed every grid using the idiom).

**The fix is not the value you found; it is the sweep.** 23 keywords, one fixture, one Chrome run:

```text
                          chrome                ours (before)
    flow-root             flow-root             inline           <- eaten
    list-item             list-item             inline           <- eaten
    table-column          table-column          inline           <- eaten
    table-column-group    table-column-group    inline           <- eaten
    ruby                  ruby                  block            <- named, post-Phase-0
    math                  inline                block            <- named, post-Phase-0
                                                        6 of 23  ->  2 of 23
```

`table-column` / `table-column-group` had variants in our own enum the whole time and were simply never
mapped. `list-item` is a **modifier bit** in Stylo (`LIST_ITEM_MASK`), not a distinct display value, so
it matched no constant and fell straight through — every `<li>` an author re-declares became inline.

### `flow-root` is a gecko-gated CONSTANT, not a gecko-gated FEATURE

`StyloDisplay::FlowRoot` is `#[cfg(feature = "gecko")]` and we build Stylo's *servo* configuration —
the same shape as the `:has()` finding. But the **parser is not gated**: `"flow-root" =>
Inside(DisplayInside::FlowRoot)` is in the servo build too, so the computed value arrives correctly and
only the convenience constant is absent. Reading it through the public `outside()` / `inside()`
accessors closes the gap with **no fork and no patch**, and a Stylo bump cannot silently revert it —
the code would fail to compile. Constitution option 1, not option 2; the fork surface stays empty.

⚠ **Before assuming a capability is missing from a vendored dependency, check whether it is the
FEATURE that is gated or only the ACCESSOR.** Here the difference was one public method call versus
vendoring Stylo.

`flow-root` then needs two independent halves in layout, and each alone is a no-op: it must be
**block-level** (`is_block_level`) *and* it must **establish a BFC** (`establishes_bfc`). Block-level
alone leaves it a plain block, and a plain block does not contain its floats — which is the entire
reason the value exists.

[[box-layout]] [[conformance-and-oracles]]

---

## A presentational hint cannot be guarded on "the property is still at its initial value"

`apply_presentational_hints` (`engine/css/src/stylo_engine.rs`) exists because our Stylo `TElement`
wall does not synthesize HTML's presentational attributes, so `bgcolor`, `width="85%"`, `cellpadding`
and friends are re-applied by hand *after* the cascade has run. Everything in it is guarded on the
same idea — **apply this only where the author left the property alone** — and that guard is sound for
`background_color` (`Option`, so `None` really does mean "nobody set it") and unsound the moment the
property's *initial value is a legal author value*.

`<td>`'s 1px padding was in that function, guarded on `s.padding == 0`:

```rust
if matches!(tag, "td" | "th") && s.padding == Sides::all(Dim::Px(0.0)) {
    s.padding = Sides::all(Dim::Px(1.0));   // ← unfalsifiable guard
}
```

`0` **is** `padding`'s initial value, so that test cannot separate *"the author wrote `padding: 0`"*
from *"nobody mentioned padding"* — and the two need opposite answers. It answered the second one for
both. Every reset that zeroes padding — Tailwind's preflight, Normalize, every hand-rolled
`* { padding: 0 }` since 2004 — reached the cell with a correctly-cascaded `0` and had the UA 1px put
straight back, on all four sides:

```text
    td { padding: 0 }        chrome  43×20      ours (before)  45×22      ours (after)  43×20
    <td> (unstyled)          chrome  68×22      ours            67×22     unchanged
```

+2px on the cell is +2px on the row, and a row is a **`dy` term**: every row below it moved down by 2px,
the table by 2×rows, and the rest of the page with it. This is the `dy`-cascade law with the smallest
possible cause.

⚠⚠⚠ **The hint had silently un-done a fix that had already landed.** t556 corrected `UA_CSS` from
`Origin::Author` to `Origin::UserAgent` precisely so that a weak-selector reset (`* { padding: 0 }`,
specificity 0,0,0) would stop losing a *specificity* tie-break to a UA rule (`td { padding: 1px }`,
0,0,1) that it outranks by *origin*. That fix worked — the cascade produced 0 — and then a hint two
hundred lines away wrote 1px over the answer. **A correct cascade is not the last word if something
runs after it.** When a value is right in the cascade and wrong in the page, grep for post-cascade
writers of that field before re-reading the cascade.

The default now comes from `UA_CSS` alone (`td, th { display: table-cell; padding: 1px }`), which is
where a UA default can be expressed *without* having to guess what the author did — the origin sort
answers that. **The general rule: a post-cascade hint may only test a field that has a value no author
can write** (`Option::None`, a sentinel, an explicit `was_set` bit). If the field cannot answer
"did the author touch me?", the hint does not belong after the cascade; it belongs in the UA sheet.

Its twin in `MinimalCascade` (`apply_ua_defaults`, `engine/css/src/lib.rs`) was never wrong, and the
reason is ordering, not care: it runs **before** author declarations, so it is a default rather than an
override. Two implementations of one rule where only one is on the shipping path
([[two-cascades-stale-source-of-truth]] in memory) — the non-shipping one was the correct one.

[[box-layout]] [[conformance-and-oracles]]

## An unresolved `&` is not a no-op — it is a selector for the ROOT (tick 757)

`RuleIndex` flattens a stylesheet's rules into one indexed list, and since t659 it recurses into a style
rule's **nested** rules. It indexed their selectors verbatim. A verbatim `&` is
`Component::ParentSelector`, and `selectors-0.39.0/matching.rs` resolves it as:

```rust
Component::ParentSelector => match context.shared.scope_element {
    Some(ref scope_element) => element.opaque() == *scope_element,
    None => element.is_root(),        // no scope set -> `&` means <html>
},
```

We never set `scope_element`. **So every `&` in every stylesheet matched `<html>`.**

### Why it looked like it worked

This is the important part, and the reason it survived ~100 ticks after nesting was indexed. It did not
fail as *"nested rules are dropped"*:

| form | outcome |
|---|---|
| `& .child` (descendant) | **matches by accident** — `<html>` really is an ancestor of everything |
| `.child` (implicit, parses as `& .child`) | same accident |
| `&` bare, `&:not(.x)`, `&.active`, `&:hover` | never match (the element is not the root) |
| `& > span` (child combinator) | never match (the parent is not the root) |

And the accident is not benign. A descendant form applied to that selector **anywhere in the document**,
and `&` contributed **no specificity** where it should contribute the parent selector's. So
`#other { & .leak { width: 500px } }` alongside a later `.leak { width: 100px }` matched *and then lost
the tie*: measured 100 where Chrome says 500. **Over-matching, under-specified, and right often enough to
look fine.**

### The fix, and why substitution rather than a scope

Stylo's own `stylist` substitutes at rule-collection time, and so do we now:

```rust
let resolved = match parent {
    Some(p) => sr.selectors.replace_parent_selector(p),
    None    => sr.selectors.clone(),
};
```

Substitution beats setting `scope_element` at match time for a flattened index, because one pass fixes
three things at once: **matching**, **specificity**, and the **index key** — a substituted `& .leak` keys
on `.leak` with `#other` as a genuine ancestor constraint, so the bucket it lands in is right too.

Two details:

- **Thread the RESOLVED list into the recursion, not `sr.selectors`.** Nesting composes; `&` two levels
  down must resolve against its parent's already-substituted selector, never against another `&`.
- **`@media` / `@supports` / `@layer` pass their parent through unchanged** — an at-rule does not
  introduce a nesting level.

Verified Chrome-exact on all six forms above plus the no-leak case.

### The rule

**When you adopt a matcher's data structure, enumerate the components whose meaning depends on context
the caller must supply.** `ParentSelector`, `:scope`, `:host`, relative selectors — each has a *defined
default* when the context is absent, so none of them errors, none logs, and each is a silent wrong answer
waiting to be plausible. A placeholder that resolves to something plausible is worse than one that throws.

---

## The servo build REJECTS `-webkit-box`, and the clamp it gates was already built (tick 763)

`engine/css/src/stylo_engine.rs` ends its cascade with a **recovery merge**: ~25 properties whose values
stylo 0.19's *servo* build never computes are copied from the MinimalCascade (`visibility`,
`background-image`, `text-transform`, `-webkit-line-clamp`, `scrollbar-width`, …). Each one is there
because dropping it was visible on real pages.

`-webkit-line-clamp` had been in that list for many ticks. It never fired on a real site, because the
keyword that *switches it on* is behind the same conditional one file over:

```rust
// stylo-0.19.0/values/specified/box.rs
#[cfg(feature = "gecko")]
"-webkit-box" => Full(Display::WebkitBox),
#[cfg(feature = "gecko")]
"-webkit-inline-box" => Full(Display::WebkitInlineBox),
```

A rejected value is a **rejected declaration**, so the element keeps its default `inline` — and
`apply_line_clamp` runs in the block-inline path, on blocks. The card excerpt showed every one of its
lines. Measured vs live Chromium (200px card, `font:16px/20px sans-serif`, the SPAN's box):

| markup | Chrome | was | now |
|---|---|---|---|
| `-webkit-box` + `-webkit-line-clamp:2` | `200×40` (computes `flow-root`) | `195×57` | `200×40` |
| `-webkit-box` alone | `200×60` (computes `-webkit-box`) | `182×57` | `200×60` |
| `-webkit-inline-box` | `108×20` (shrink-to-fit) | `108×17` | `108×20` |

**The fix is a MARKER, not a display copy.** `ComputedStyle::legacy_webkit_box: Option<Display>` records
*"the author asked for `-webkit-box` here"*; the merge applies only that:

```rust
if let Some(d) = m.legacy_webkit_box { cs.display = d; }
```

Copying `m.display` instead would give the shipping path the MinimalCascade's opinion on the display of
**every element in the document** — the two-cascades trap (`docs/wiki/…`, memory
`two-cascades-stale-source-of-truth`). The marker is set only by the two legacy keywords and **cleared by
any other recognised `display` value**, so `display:-webkit-box;display:flex` computes `flex`; an
unrecognised value is an invalid declaration and disturbs neither.

**Gate.** `webkit_box_display_recovers_through_the_stylo_cascade`, RED-proven by neutering the recovery
(reads `Inline` where the assertion wants `Block`). Real-site: `momon-ga.com` shape 0.509 → **0.565**,
`marktplaats.nl` control byte-identical.

**The generalisation, and it is a standing audit item.** A capability gated behind a keyword the parser
rejects is not a capability. Our own clamp test set `-webkit-line-clamp` on a `<div>` — already a block —
so it was green for exactly as long as the real idiom was broken. **When a feature is switched on by
another property's value, its gate must assert the SWITCH, not the mechanism**, and every property in the
recovery list deserves the question: *is the value that activates it on the other side of the same
`cfg(feature = "gecko")`?*

## The cascade never saw a decoded stylesheet — `out.push(b[i] as char)`

`strip_comments` runs on the way IN to `Stylesheet::parse`, and it walked the source as **bytes**:

```rust
out.push(b[i] as char);   // identity for ASCII; Latin-1 widening for everything else
```

Each UTF-8 byte became its own code point, so `–` (U+2013, bytes `E2 80 93`) came out as `â€“`.
`Stylesheet::parse` stores the result as `source`, and `source` is the string handed verbatim to
`StyloStylesheet::from_str` — **so Stylo never received a correctly-decoded stylesheet.**

The DOM was fine the whole time, which is why the bug had nowhere to show itself. The chain that pinned
it, and the reason it took instrumentation rather than reasoning:

| observation point | code points for `–` |
|---|---|
| `style.textContent` from JS | `8211` ✓ |
| `dom.text_content()` in Rust | `8211` ✓ |
| `sheet.source()` at the `StyloStylesheet::from_str` call site | **`226, 128, 147`** ✗ |

### What it cost

* Every non-ASCII `content:` string — arrows, checkmarks, quotes, currency, the icon glyphs half the web
  puts in `::before`. Found on `255md.com`, whose bullets are `li::before { content: "–" }`: we drew
  `â` glued to each item.
* **`font-family` names written in their own script** — `"微软雅黑"`, `"ヒラギノ角ゴ"`, `"맑은 고딕"`.
  A mangled family name matches no font, so an entire CJK font stack silently falls through to a
  default. Nothing is logged, because from the font layer's side the name simply did not resolve.
* Custom properties, `quotes:`, non-ASCII identifiers, `url()` with a non-ASCII path — and **attribute
  selectors matching non-ASCII values**: with the defect restored, `#a[data-x="café"]` stops matching
  and the rule does not apply at all.

### The fix, and what deliberately did not change

Copy the whole character rather than one byte of it. Scanning for the `/*` and `*/` delimiters as bytes
is **still correct and is kept**: `/` and `*` are ASCII, and a UTF-8 continuation byte is always ≥ `0x80`,
so no multi-byte character can contain either delimiter. Advancing over the lead byte plus its
continuation bytes leaves the index on a char boundary, so the slice cannot panic.

### ⚠ Why it survived the entire project, which is the part worth keeping

**The escape form was never affected.** `content: "\2013"` is pure ASCII and always worked — and every
CSS test in this repository was written in ASCII. *A bug invisible to the entire alphabet your tests are
written in is not found by writing more tests of the same kind.* `G_CSS_UTF8` is written in four scripts
plus an astral emoji, and asserts **code points** rather than rendered text, because a mojibake'd string
still renders *something* and eyeballing it is exactly how this survived. It is red-proven by restoring
the original line, and the exhibit is that `esc:` stays green while everything else turns into raw bytes.

### And the headline metric could not see it either

Fixing this moved **zero** shape points on `255md.com` (0.698 before and after). Shape scores ELEMENT
geometry; a `::before` is not an element, its text draws inside the `<li>`'s box, and that box is
identical whether the marker says `–` or `â`. A visibly wrong, Chrome-differential rendering defect on a
real site is therefore invisible to the burndown — the same blindness as the parent-relative
cancellation of tick 762, in a different dimension. "No number moved" is sometimes a statement about the
instrument's frame, not about the fix.

## A nested `@media` lost its declarations, and only its declarations (t785)

CSS Nesting has two halves and this engine shipped one of them for 126 ticks.

```css
article {
  padding: 0 32px;
  max-width: 423px;
  @media screen and (min-width: 1018px) { max-width: 974px; padding: 0 80px; }
}
```

A nested **style** rule (`& .c { … }`) is a `CssRule::Style` with its own selectors, and t659 taught
the rule-index walk to recurse into `sr.rules` and substitute `&`. Declarations written *directly*
inside a nested group rule have no selectors of their own: the spec wraps them in an implicit
`& { … }`, and Stylo materialises that as a separate variant — **`CssRule::NestedDeclarations`**,
a block and nothing else. The walker had no arm for it and a trailing `_ => {}`.

**The rule that owns a selector survived; the one that borrows its parent's did not.** Nothing warned,
nothing threw, and the page still rendered — with one branch of a media query silently missing.

**What it cost, on a site the board had already ranked.** `secure5.entertimeonline.com`: Chrome lays
the `<article>` out at 487px (423 + 2×32), we gave it 1134px (974 + 2×80) — the page's whole content
column, and every descendant displaced with it (that site's #1 oracle cause was `displaced: x ~256px`
on the children, not a width error on the parent). Shape **69.2% → 79.5%**, which crosses the 0.75 M1
bar. `blog.rust-lang.org` did not move by a decimal (1664 paths, 73.6% both sides).

⚠ **The fix's own failure mode is worse than the bug, so the gate pins it.** "Apply the declarations
we were dropping" and "apply them *unconditionally*" produce identical results on every matching
media query and differ only on a non-matching one. `G_CSS_NESTING` therefore asserts a nested
`@media (min-width: 5000px)` does NOT apply on an 800px page, beside the one that does.

**Population:** 4 of 48 cached snapshots (8%) put an at-rule inside a style block in their inline
`<style>` alone. External sheets are not counted there and are where compiled framework CSS lives —
entertimeonline's rule is in one — so 8% is a floor.

**The transferable part.** The burndown's ranked #1 mechanism is *container-width errors launder into
`dy`*, and every previous attempt at it went looking for a sizing primitive. This box was the wrong
width because a declaration that would have sized it never entered the cascade. **The evidence was not
in the boxes; it was in the four lines of CSS the site actually served, one `curl` away.**

## A layer exists to LOSE, and ours won (t790)

`@layer` is how a page keeps a framework overridable: the vendor's rules go in a layer, the page's own
rules stay unlayered, and **unlayered beats layered regardless of document order.** That last clause is
the whole feature. We flattened layers into document order, so it read exactly backwards:

```css
#h { width: 100px }
@layer L { #h { width: 333px } }        Chrome 100   ·   ours 333   (before t790)
```

The fix is one term in the winner sort, between ORIGIN and SPECIFICITY: unlayered takes the top rank,
layers count up from zero in declaration order.

**Two clauses decide whether an implementation is right or merely plausible.**

1. **The `@layer reset, theme;` STATEMENT form fixes the order before either block exists.** It is
   written at the top of a sheet precisely so the blocks below can appear in any order — which is the
   idiom nearly all real usage takes. An engine that ranks layers by first BLOCK reads it backwards.
   In the gate: `theme` must win at 300 though its block is written *before* `reset`'s.
2. **"Layers lose" must not become "layers are ignored".** A declaration that exists ONLY in a layer
   still applies. A fix aimed at symptom 1 alone breaks this, and the two are indistinguishable on
   every page where both a layered and an unlayered rule exist — which is where the bug was found.

Named residues: the pseudo-element index carries no layer rank (every pseudo rule is unlayered, the
pre-t790 behaviour), and `!important` does not yet **reverse** layer order the way the spec requires.

⚠ **Where it came from is the transferable part.** Nobody went looking for cascade layers. The t785
nesting fix's own probe fixture had a nested `@layer` line in it as a *control*, Chrome disagreed on
that line, and pulling the thread found a top-level defect that had nothing to do with nesting. **A
capability's neighbours are the cheapest place to find the next defect, because the fixture is already
open and the reference is already running.**

## Where Chrome draws the form-control `box-sizing` line (t851)

Chrome's UA sheet computes **`border-box`** for `button`, `input[type=submit|reset|button]` and
`select`, and **`content-box`** for `input[type=text]`, `textarea` and every ordinary element. The
line is not intuitive — the controls that look most alike end up on opposite sides of it.

Measured at `height:50px; padding-top:20px`, used border-box height:

```text
              button  submit  text  select  textarea  div    author box-sizing:content-box
  Chrome        50      50     70     50       70      70            71
  before        70      70     70     70       70      70            71
  after         50      50     70     50       70      70            71
```

So a button, a submit input and a `<select>` were **too tall by exactly their vertical padding plus
borders** on every page that sets a height and padding on them — which is what every design system
does to a button. The rule is UA-origin, so an author's own `box-sizing` still wins (the last column).

### Both UA sheets were wrong, in OPPOSITE directions

This engine keeps two hand-maintained UA stylesheets: `engine/css/src/stylo_engine.rs` (CSS text, the
**shipping** cascade) and `apply_ua_defaults` in `engine/css/src/lib.rs` (`MinimalCascade`).

* `MinimalCascade` set `border-box` for **all four** form tags — too many.
* `stylo_engine.rs` had **no rule at all** — too few.

Neither matched Chrome. The known hazard (`two-cascades-stale-source-of-truth`) predicts one sheet
going *stale* relative to the other; what actually happened is worse, because **each sheet's error
concealed the other's from whichever test you happened to write.**

### A layout-crate test cannot see the shipping cascade

`manuk-layout`'s `layout_html` helper runs `MinimalCascade`. A unit test there would have gone green
on the half the browser does not use. The gate therefore lives in
`engine/page/tests/g_form_control_metrics.rs`, which loads through `Page::load`, and it is RED-proven
by commenting the rule out of **`stylo_engine.rs`**.

⚠ Running that gate **without** `--features stylo,spidermonkey` fails its two pre-existing assertions
with completely different numbers (`#a1` width 194 against Chrome's 53) — the same fact from the
other side, and worth recognising before mistaking it for a regression.

## A `_` arm with a REASON above it is the hardest kind to audit (t975)

`stylo_map.rs` maps Stylo's computed `transform` list onto our affine ops. Its catch-all carried a
justification, and the justification was the bug's alibi:

```rust
   // transform: map the 2D operations onto our affine list (3D/perspective skipped — our
   // paint model is 2D). …
   _ => {}          // <- Translate3D, Scale3D, Rotate3D, Matrix3D all landed here
```

**That sentence is true of a genuine 3D effect and false of `translate3d(x, y, 0)`**, which has no 3D
component at all — it is *the* idiom for putting an element on its own compositor layer, which is how
every animation library, carousel, drawer and sticky header on the modern web writes a plain
translation. Dropped, the element stays at its **untransformed position**, the largest error the
property can produce:

```text
   a 100x40 box                        Chrome        before        after
     translate3d(20px,10px,0)        [ 20, 1070]   [  0, 1060]   [ 20, 1070]
     scale3d(2,2,1)                  [-50, 1110]   [  0, 1130]   [-50, 1110]
     rotate3d(0,0,1,45deg)           [0.5, 1170]   [  0, 1200]   [0.5, 1170]
     matrix3d(… 30,15,0,1)           [ 30, 1355]   [  0, 1340]   [ 30, 1355]
     rotateZ(90deg)                  [ 30, 1240]   [ 30, 1240]   unmoved  <- ALWAYS worked
```

⚠⚠ **`rotateZ` being mapped the whole time is why the family looked handled from the outside**, and
the comment is why nobody re-checked. **A reason written above a catch-all does the work a
measurement should do** — it converts "we have not looked" into "we decided", and the two are
indistinguishable from the caller.

⚠⚠⚠ **AND THE FIRST FIX WENT TO THE WRONG FILE.** `parse_transform` in `engine/css/src/lib.rs` has
the same `_ => {}` and fixing it changed **nothing**, because that parser is the `MinimalCascade`
path and **the shipping cascade is Stylo** ([[live-cascade-is-stylo-not-minimal]]). The measurement —
rebuild, re-measure, still wrong — is what pointed at the real site. Both are now fixed, so the
JS-less/headless fallback agrees with the shipping path.

**The projection is exact, not an approximation.** With no `perspective` in force, `z` contributes
nothing to the on-screen position, so each 3D function's x/y terms *are* its rendered effect.
`rotate3d` is taken **only about the z axis** for the opposite reason — a rotation about x or y
foreshortens, which a 2D pipeline cannot express, and `G_TRANSFORM_3D` carries the row that fails if
the axis check is dropped (an X rotation becoming 99×99 where Chrome leaves the box 100×40).
`translateZ` and `perspective` are matched **explicitly** so their omission reads as a decision
rather than as the arm that hid this.

⚠ **Still unimplemented and measured: `transform-origin`.** We always transform about the box centre;
`origin: 0 0` gives Chrome [0, 220] against our [−50, 200], and `100% 100%` gives [−100, 810] against
[−50, 830]. `layout/lib.rs:922` already takes an `origin` parameter and is only ever handed the
centre.

### …and its sibling: a DEFAULTED PARAMETER no caller overrides (t976)

t975's `_` arm hid a dropped function. The same family's other half hid a whole property behind a
parameter that was never passed:

```rust
   /// Compose a `transform` … applied around `origin` (the transform-origin, default the box center).
   fn resolve_transform(fns: &[TransformFn], w: f32, h: f32, origin: (f32, f32)) -> [f32; 6]
   // ...and all THREE call sites:
   let origin = (rect.x + border_box_w / 2.0, rect.y + border_box_h / 2.0);
```

> **A defaulted parameter that no caller ever overrides is indistinguishable from an unimplemented
> property until something measures it.** The seam is built, the doc explains the semantics, grep
> finds the name — and the behaviour is a constant.

Chrome-measured, `scale(2)` on a 100×40 box (the left edge is the discriminator):

```text
                                    Chrome        before        after
   transform-origin: 0 0           [   0, 220]   [ -50, 200]   [   0, 220]
   transform-origin: 100% 100%     [-100, 810]   [ -50, 830]   [-100, 810]
   (no origin declared)            [ -50, 130]   [ -50, 130]   unmoved  <- CONTROL
```

⚠⚠ **`top`/`bottom` name the Y AXIS WHEREVER THEY APPEAR** — `top left` is as valid as `left top`,
and reading the two words positionally silently swaps that pair. Our parser handles it; **the gate
does not prove it**, because on the shipping path Stylo resolves the keywords and our copy is the
MinimalCascade fallback. The assertion says so rather than claiming a proof it does not have — the
second tick running where [[live-cascade-is-stylo-not-minimal]] decided where a fix belongs.

⚠ **Revert a RED proof by restoring the file you copied, never with `git checkout` on a file the tick
is also editing** — it discards the whole tick's work in that file, not the mutation.

## `compute_for_declarations` is FIRST-SEEN-WINS, and it maps logical to physical as it goes (t998)

`stylo_engine.rs` matches rules itself, then merges every winner's declarations into **one**
`PropertyDeclarationBlock` and hands it to `Stylist::compute_for_declarations`. Two properties of that
entry point are load-bearing and neither is in its name or its signature:

```text
  stylist.rs      compute_for_declarations
        -> properties::apply_declarations(.., block.declaration_importance_iter(), ..)   FORWARD
           (the rule-tree path uses DeclarationIterator::next, which calls next_back() — the OPPOSITE
            direction. Reading only the rule-tree path tells you the wrong contract.)

  properties/cascade.rs   Cascade::apply_one_longhand
        -> `if self.seen.contains(longhand_id) { return; }`
           ...and `seen` is keyed on the id AFTER `to_physical(writing_mode)`
           (apply_non_prioritary_properties does the mapping just before the call)
```

**So the block must be built highest-priority-FIRST.** For sixty ticks ours was built ascending, and
that was invisible, because `PropertyDeclarationBlock::push` de-duplicates on `id()` and moves the
newcomer to the end of the block: two declarations of the same longhand collapse to one, so the
direction of the walk cannot matter. Every ordinary property was therefore correct *by a property of
`push`*, not by the merge being right.

**The one case `push` cannot collapse is a logical/physical pair.** `margin-left` and
`margin-inline-start` are different `LonghandId`s, so both survive into the block, and forward
first-seen-wins hands the win to whichever was pushed first — the LOWEST-priority declaration.
Measured 7 of 7 against Chrome; `* { margin: 0 }` did not reset a UA or author `margin-inline`.

The fix is one function, `merge_ascending`:

- iterate the ascending list **in reverse**, `!important` pass first, then normal;
- skip any declaration whose `id()` is already in the block — keeping the FIRST occurrence in that
  descending order is keeping the highest-priority one;
- push the survivors.

**No writing-mode logic on our side, deliberately.** `writing-mode` and `direction` are prioritary
properties that Stylo applies *before* `apply_non_prioritary_properties`, so `to_physical` already sees
the final value. A merge that resolved logical→physical itself would need to re-derive the writing mode
from the parent plus the element's own declarations — a second source of truth for a mapping Stylo
already owns.

⚠ **Two residues named rather than fixed.** Every declaration handed to `compute_for_declarations`
carries the *same* `CascadePriority`, so (a) importance ordering is ours to do, which is why the
important pass exists, and (b) **UA-`!important` still does not outrank author-`!important`** — the
important pass is ordered by our `origin_rank` ascending like the normal one. That inversion predates
this tick and is unchanged by it; `revert` / `revert-layer` are degenerate for the same reason.

⚠⚠ **The general lesson, and it is the third time in this file.** *Read what the function DOES with
the thing you hand it, not what the function is called.* `compute_for_declarations` sounds like "give
me a block, get a style"; its contract is "give me a block **in descending cascade priority**, get a
style", and that sentence exists nowhere but in the body of two other files.

## A form control's UA box, and the two constants that cancelled (tick 1043)

`<button>` and `<input>` are the **#1 and #2 constructs of the burndown corpus** — 55.6% and 51.5% of
the 171 pages that produce M1 (`docs/loop/CORPUS-CONSTRUCTS.md`), beating `<table>` eight to one. A
control that is the wrong size is therefore a `dx`/`dy` error on more of the corpus than any other
single box, and it displaces everything laid out beside and below it.

### What the reference actually says

Chrome will recite its own UA sheet if you ask it (`getComputedStyle`, the tick-1028 method). Asked
about every form tag at once, it answers:

```text
   input[type=text]   padding 1px 2px   border 2px inset   box-sizing content-box
   input[checkbox]    padding 0         border 0           box-sizing BORDER-box   13x13
                      margin 3px 3px 3px 4px
   input[radio]       …the same, but    margin 3px 3px 0px 5px
   select             padding 0         border 1px solid   box-sizing border-box
   textarea           padding 2px       border 1px solid   box-sizing content-box
   button             padding 1px 6px   border 2px outset  box-sizing border-box
```

Three of those were wrong here, and the interesting part is *why they were invisible*.

### Two wrong constants that agree at exactly one point

Our sheet paired a **1px** text-field border with an intrinsic-width intercept of **2.925**; Chrome
pairs a **2px** border with **2.75**. A text field's border-box width is `fs·(size·0.6 + k) + 4 + 2·bw`,
so the two models are:

```text
   ours    2.925·fs + 6          Chrome   2.75·fs + 8
```

**They are equal at `fs = 13.333` — the UA control font — and at no other font size.** Every row
anybody had ever measured was taken at that font, so all four of them (`size=` 1, 5, 20, 40 →
53/85/205/365) were exact under *both* models for the life of the sheet. Tick 1038 measured the
border as wrong, measured the default width as already exact, and correctly declined to change one
without the other, calling it a trade. It was a trade; what closes it is **re-deriving the intercept
against the corrected border**, after which the same four rows are still exact and the height stops
being 2px short at every size.

> **A constant fitted at one point cannot tell you which of two models it fits.** The second point
> costs one fixture row: `<input style="font-size:20px">` is 303px in Chrome, 305 under the old pair.

### The same shape again, one rule down

`<textarea>`'s UA padding is `2px` on all four sides — the `1px 2px` above belongs to `<input>`, and
one shared rule had handed it to both. The missing 1px top and bottom was compensated by a `+ 2.0`
addend in the `rows` height formula, whose own comment gave a plausible and wrong reason for it
(*"Chrome's inner editor sits 1px clear … `getComputedStyle` must keep reporting `1px 2px`"* — Chrome
reports `2px`). The two cancelled on **every** intrinsic row (`rows=1` → 21, default → 36, `rows=3` →
51, all exact before and after), so only an author-specified height could see it:
`<textarea style="width:100px;height:40px;border:0">` is 104x44 in Chrome and was 104x42 here.

### The one control whose border we draw and Chrome does not

Chrome declares `border: 0` on a checkbox and paints the widget natively (`appearance: auto`). We have
no native control painter, so **our 1px border _is_ the checkbox** — which under `content-box` made
Chrome's 13x13 into 15x15, and an author's own `width:30px` into 32. `box-sizing: border-box` keeps
the border we need to draw and hands back Chrome's outer box. The other half is the **margin**
(`3px 3px 3px 4px`, and `3px 3px 0 5px` for a radio): asymmetric, per-type, and ours were zero, so a
row of controls accumulated the error rather than sharing one constant offset.

### How this was found, and what it says about method

A ~30-row differential battery per construct, all rows diffed against headless Chrome in one command,
**negative rows written first**. Across three batteries, 101 Chrome-diffed rows went **69 exact → 87
exact**. What the batteries also established, which no amount of reasoning would have:

- **`<button>` was already right.** 48 of its 55 rows were exact before this tick — including
  intrinsic sizing, `box-sizing`, both-axis content centring, flex-item behaviour and min/max clamps.
  The corpus's #1 construct needed nothing; its #2 needed four fixes.
- **Every remaining divergence is a `dy` and they are all one mechanism** — the *baseline* of a form
  control. Chrome gives an `<input>` the baseline of its inner text (12.0px from the top of a 15px
  content box); we use the bottom margin edge, which puts a default text field 3px high in every line
  of text it sits in. Named, measured, and left for its own tick, because it is a different rule from
  the box metrics above.

⚠ **`<select>`'s intrinsic content height is 17px at the control font and ours is 15**, and its
`font-size:20px` box is 26 against our 27. Unrelated to anything here, 11.1% of the corpus, banked
rather than guessed at.

## The baseline of a text field, and a term that cancelled (tick 1044)

The residue t1043 named, closed. **Every remaining `<input>` divergence in three form-control
batteries was a `dy`, and they were one mechanism.**

### The defect is a DOMAIN error, not a missing rule

CSS 2.1 §10.8.1 says an `inline-block`'s baseline is its **last in-flow line box's**, falling back to
the **bottom margin edge** when it has none. We implement both halves correctly. But an `<input>`'s
text lives in an inner editor that has no box in our tree, so the search found no line, took the
fallback, and put every default text field **3px high in every line of text it appears in** — on
51.5% of the burndown corpus.

**The control was already in the fixture, and it is the same box with the text put back:**

```text
   <input style="width:50px;padding:0;border:0">                        Chrome dy 3   ours dy 0
   <span style="display:inline-block;width:50px;height:15px;
         font:13.333px Arial">Ay</span>                                 Chrome dy 3   ours dy 3
```

Identical geometry, identical font, identical Chrome answer — and we were **already exact on the
second row**. `<button>Ay</button>` was exact for the mirror-image reason: its label *is* a DOM text
node. So this is not a new baseline rule; it is the rule we have, reaching a line the DOM does not
contain, and the fix is four lines.

### The model, and the ladder that fixes it

One text box of the control's own font, **centred in the content box**:

```text
   content h      6      10      15      20      30      (strut baseline 15.0)
   Chrome dy     7.5     5.5     3.0     0.5     0.0
   baseline      7.5     9.5    12.0    14.5    19.5   = (h - 15)/2 + 12
   before        9.0     5.0     0.0     0.0     0.0   = the bottom margin edge
```

Top-aligning gives a constant 12 and matches **only the middle rung** — which is the row a
natural-height field produces, and therefore the only row a fixture written from the obvious case
would have contained.

⚠ The **content** box, not the border box, proven three ways — `height:0;padding:5px`,
`height:0;border:5px` and `height:4px;padding:3px` are all `dy 5.5` in Chrome. Those rows exist
because a `padding:5px` field at natural height pins at `dy 0` under *either* model, so the obvious
frame row cannot tell them apart.

### ⚠⚠⚠ The falsification pass deleted a term, again

The first version centred a full **line box** (`ts.line_height`) and then placed the baseline within
it at `ascent + half_leading`. Mutating the half-leading term away left the gate **green** — t834's
rule says that makes it unreachable code to be deleted or made falsifiable, not admired. The algebra
says why:

```text
   (h - L)/2 + a + (L - a - d)/2   ==   (h - a - d)/2 + a       for every L
```

`line-height` cancels. And Chrome says it is not merely inert but *wrong* to be there:

```text
   <input style="height:0;padding:0;border:0">        Chrome dy 10.5
   …the same with line-height:30px                    Chrome dy 11.0
   …the same with line-height:6px                     Chrome dy 10.5
```

**24px of extra leading moves the baseline by half a pixel of rounding.** Chrome centres the inner
editor's *text* — `ascent + descent` — not a leaded line. Writing `a + d` rather than `line_height`
is now a claim the code makes and a fixture can refute; the two-term version made the same
prediction while asserting something false about the mechanism.

> **A mutation that leaves the gate green is not a weak gate — it is a sentence in the code that
> nothing is testing, and half the time the reason nothing can test it is that it does not mean
> anything.** Six mutations, six reds, after the pass removed a term and added the row that catches
> its replacement.

### Where a baseline actually shows up

The load-bearing gate row is not the control's own box. A baseline is a property of the **line**, so a
30px-tall field pushes the **text beside it** down 4.5px in Chrome — and the field's own `dy` is `0`
under both the right model and the wrong one. Asserting only the control would pass with the line
still wrong, which is precisely how a mis-baselined control feeds `reading_order`: it displaces its
neighbours, not itself.

⚠ **Named, measured, not fixed here.** `<select>` needs its own tick and now has numbers: intrinsic
content height **17** against our 15, and Chrome's UA `align-items: center` puts its option text in
the middle of a tall box where ours pins it to the top (`select height:30px` is `dy 0` in Chrome and
`dy 3` here). An **empty** `<button>` takes its content-box *bottom* as its baseline (`dy 12` on
`<button></button>`), a third rule again. And `<input type=range|color|image>` render as an **8x6
stub** — no widget at all — against Chrome's 129x16, 50x27 and 20x20; `type=image` is a replaced
element and is not treated as one.
