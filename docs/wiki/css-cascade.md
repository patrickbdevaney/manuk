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
| `relayout_incremental` | on the dirty bits — correct trigger | rebuilds its sheet list from `MinimalCascade::collect_style_elements`, which sees inline `<style>` and **not `<link>`ed sheets**. Hover any link on any site with external CSS and **every external stylesheet drops out of the cascade.** |

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
