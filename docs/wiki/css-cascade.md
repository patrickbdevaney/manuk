# CSS AND THE CASCADE — Stylo realities and quirks actually encountered

## CSS COUNTERS ARE LOST AT A `_ => {}` IN THE CASCADE, NOT AT LAYOUT (t1095)

Three `<h2>` under `counter-reset:sec`, with `h2::before{content:"S" counter(sec) ". "}`, measured
against Chrome in the product path:

```text
                     Chrome    manuk    delta
   #a (S1. Alpha)       87       77      -10        exactly one monospace character
   #b (S2. Beta)        77       67      -10
   #c (S3. Gamma)       87       77      -10
```

**The uniform one-character deficit is the diagnosis, not the symptom.** If `content` were unparsed
we would draw the literal text `counter(sec)` and be *wider* than Chrome; if the declaration were
dropped we would draw nothing. Narrow by exactly the counter's own width means the content LIST is
already assembled correctly and only the counter TERM evaporates — at one line:

```rust
   // engine/css/src/stylo_engine.rs — flattening a pseudo's content
   ContentItem::String(sv) => out.push_str(sv),
   ContentItem::Attr(a)    => { … }          // t409
   _ => {}                                   // ← Counter / Counters, silently
```

Stylo parses counters perfectly well. We discard them, **in the cascade**.

### The decomposition, and its first brick is not its subject

1. **`cs.content` is `Option<String>`, and a string cannot hold a counter.** The flattening runs at
   cascade time, where the value is *not yet knowable* — it depends on document order. The type must
   carry unresolved terms, in **both cascades** (Stylo and MinimalCascade). This is the blocker, and
   it is a type change rather than an algorithm.
2. `counter-reset` / `counter-increment` sit in the property-name list (`stylo_engine.rs:2982-2983`)
   and are **never mapped onto `ComputedStyle`** — parse-only.
3. *Then* the document-order walk: scoping, nesting, `counters()` separators.

> **A subsystem's first brick is not always its subject.** This was priced as "a tree-walk with real
> semantics" and would have been started at step 3 — which cannot be written first, because with
> `content` already flattened to a `String` there is nothing left for a walk to resolve. One `grep`
> for the drop site said so in three minutes.

Worth 73 of the 1,843 remaining CSS 2.1 failures and 15% of the burndown corpus's pages
(`docs/loop/CORPUS-PSEUDO-t1094.tsv`).

### LANDED at t1096, brick-1-first — and 28 of the 31 gains were in a chapter I was not aiming at

`content` became `Option<Vec<ContentPart>>` (7 consumer sites, compiler-enumerated), both cascades
gained `counter_reset` / `counter_increment`, and `Ctx::counter_values` does one memoised
document-order walk — reset then increment — snapshotting only at nodes whose pseudo names a
counter. Chrome-exact: `87 / 77 / 87` where we gave `77 / 67 / 77`. `css/CSS2` **3,812 → 3,843, +31
and 0 lost**.

```text
   lists               37 → 65   +28    ← chapter 12 IS "generated content, numbering and LISTS"
   generated-content   67 → 70    +3
```

**The by-CHAPTER ranking hid what the by-FAMILY ranking showed.** `CSS2-RANK-t1091.tsv` puts `lists`
at 45.1% and it reads like a list-marker chapter; `CSS2-FAILFAMILY-t1091.tsv` had
`counter-increment 24` and `counter-reset 16` in plain sight, and the gained files are named
`lists/counter-increment-005…`. Bank both rankings — they fail differently.

### ⚠⚠⚠ A mutation came back GREEN: the layout-crate gate is blind to the SHIPPING cascade

Deleting the `ContentPart::Counter` arm from `stylo_engine` left the new `#[test]` **passing**, because
a layout battery is styled by `MinimalCascade` while the product ships Stylo. The two halves need two
instruments, and both mutations were run:

```text
   the WALK + MinimalCascade parse   the #[test]      increment-before-reset → RED ("got 0|x")
   the STYLO content mapping         boxes --html     drop the arm           → 77/67/77
```

> A half-blind gate is more dangerous than a vacuous one: mutate the obvious line, watch it stay
> green, and conclude the gate tests nothing — when in fact it tests the *other* half perfectly.

**Deliberately flat**: one global counter map, exact for section/figure/table numbering, ordered
steps and breadcrumbs, and wrong for nesting — `counters(c, ".")` prints the flat value, not
`2.1.3`. Scoping a reset to its subtree and following siblings is the next brick.

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

## A `@keyframes` ANIMATION IS A STATIC FUNCTION OF THE CASCADE (t1273)

Until t1273 this engine **interpolated nothing, anywhere**. `@keyframes` parsed, `animation-*`
cascaded, and every one of those values was discarded; the sole consumer was the reveal-hack above.
So an element half-way through `@keyframes grow { from { width: 0 } to { width: 100px } }` rendered
its **base rule**, and `getComputedStyle` agreed with the base rule.

⭐ **The insight that makes this a tick rather than a subsystem: the value needs no clock.** A
`100s` animation with `animation-delay: -50s` is at its half-way point **at time zero** — no
compositor, no frame loop, no rAF, nothing to schedule. So the animated computed value is a *pure
function of the cascade plus one scalar*, and that scalar defaults to 0 for a static render. WPT's
`css/support/interpolation-testcommon.js` is built on exactly this idiom, which is why one static
evaluation reaches **194 `*-interpolation.html` files across twelve CSS areas**.

**Everything numeric is borrowed** — `STATUS.md`'s ladder, option 1, no fork and no patch:

| need | borrowed from |
|---|---|
| `@keyframes` → ordered steps + the set of properties that change | `Stylist::lookup_keyframes` → `KeyframesAnimation` |
| a computed value in animatable form | `AnimationValue::from_computed_values` |
| the interpolation, per property type | `Animate::animate(Procedure::Interpolate)` |
| back to a declaration the cascade can take | `AnimationValue::uncompute` |
| `cubic-bezier` / `steps` / `linear()` | `ComputedTimingFunction::calculate_output` |

Hand-rolled is only what the *servo* build does not ship: **where in its own timeline is this
animation right now** (`animation.rs::iteration_progress` — delay, iteration count, direction,
fill-mode).

⚠⚠ **The two bracketing keyframes are reached by CASCADING, not by reading the keyframe block.** A
keyframe declares *specified* values (`width: 50%`, `color: currentcolor`) and interpolation is
defined on *computed* ones, so each side is the element's own cascade re-run with that keyframe's
declarations appended at the animation origin. This is not extra work for its own sake — **it is
what gives the spec's fill-in for free**: a property the `0%` keyframe never mentions still carries
the element's underlying value on the from-side, because that cascade still contains every rule that
produced it. The gate's `w2=60` row is that fill-in, and 60 is a number *neither keyframe contains*.

⚠ **The animation origin is a SLOT, not a rewrite.** `cascade_one_element` already builds an
ascending `Vec<(&PropertyDeclaration, Importance)>` and merges it; the animation's declarations are
appended last, which is exactly CSS Cascade §6.2's position (above every normal author declaration).
The whole seam is one parameter.

⚠ **Two things this does NOT do, named so nobody re-derives them.** (1) There is still no clock:
`animation::set_time_ms` exists and every caller leaves it at 0, so a real page's animations sit at
their start rather than running — which is why the `opacity: 0` reveal-hack above stays in force and
is pinned by the gate rather than deleted. (2) CSS **transitions** and the Web Animations shim are
separate legs of the same harness and are untouched; they need the previous computed value and a
`currentTime` respectively, and both reuse this interpolation core.

## `animation-composition: add` — the keyframe ADDS to the underlying value (tick 1287)

Each endpoint is built by re-running the element's cascade with the keyframe's block appended, which
is **replacement by construction** — correct for the default `replace`, and the whole of the defect
for `add`. WPT read `100px → 150px → 200px` where it wanted `150px → 200px → 250px`: **the
interpolation already exact, every value short by precisely the underlying.** One missing term, not a
broken engine — and it sat under **59 files across 11 CSS areas** (`grep -rl test_composition css/`).

Borrowed end to end (ladder option 1, no fork): `Procedure::Add`, `Procedure::Accumulate { count }`,
and `AnimationComposition` as a real Stylo longhand. The mode is read out of **the same declaration
block the endpoint is built from** — WPT's harness writes it inside the keyframe
(`from {${prop}:${v}; animation-composition:${c}}`), and reading it from anywhere else would let the
two drift.

The underlying value is `cv`: the element's own cascade with no keyframe mixed in, i.e. the style
that would have been published had it not been animating. That is the spec's definition, and it is
already in hand at the call site.

⚠⚠⚠ **ONLY THE PROPERTIES AN ENDPOINT ACTUALLY DECLARES MAY BE COMPOSITED, and this is the one way
the fix can be worse than the bug.** `Sample::properties` is the union over *all* keyframes, and the
spec's fill-in for an unmentioned property IS the re-cascade — so that side already carries the
underlying value, and compositing it adds underlying to underlying and **silently doubles it**, on a
property nobody animated additively. Each side therefore carries `(mode, declared-longhands)`, not a
mode alone. The gate's `mixed:` row is written for precisely this and is the only row that moves when
the check is removed.

⚠ **Compositing is PER ENDPOINT, not per animation** — `from add … to replace` is a real and common
combination in the suite; an animation-wide flag reads the `to` side wrong.

⚠ `Err` from `animate` on the composite step means the pair does not compose (discrete or mismatched
types); the spec's answer is that the composite is a no-op and the endpoint stands, which is why this
cannot turn a working property into a broken one.

**Measured:** `css/css-position` 689 → **778**, `css/css-transforms` 2411 → **2704**, `css/css-sizing`
3028 → **3123**, `css/css-backgrounds` 4087 → **4141**, `css/css-values` 3230 → **3231** — **+532**,
`HANG/CRASH 0`, and the failing-title diff shows 89 gone and 0 new in `css/css-position`.
⚠ `css/css-values` barely moved because its compositing files are under `calc-size/animation`, where
the tests fail at a **parse** gap before they reach interpolation: **a shared mechanism does not reach
an area whose tests fail before they get to it.**

⚠ **A FIXTURE LESSON WORTH MORE THAN THE FIX.** The first draft of the gate read every row with the
*relationships* exact and the progress at ~0.8023 instead of 0.5 — which looks at a glance like a
broken sampler. It is `ease`, the default `animation-timing-function`, whose output at input 0.5 is
0.8023. **An expectation that hard-codes a number must pin every input that number depends on.** WPT's
own harness sets the easing explicitly for exactly this reason.

Held by `engine/page/tests/g_animation_composition.rs`, RED three ways (force `Replace`; drop the
declared-set check; use one mode for both sides), each landing on its predicted number.

⚠ **`visibility` is not plain-discrete, and the gate row that says so was written expecting the
opposite.** Its interval is `visible` whenever *either* endpoint is visible; Stylo implements that
and Chrome agrees. Separately, Stylo's `animate_discrete` returns the 50% flip as `Ok`, so the
`Err(())` arm is reached only by genuinely **non-interpolable** pairs like `auto` ↔ `100px`.

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

## A dropdown ignores its leading, and the constant that was fitted at one point (tick 1045)

Two batteries on two constructs that had never had a geometry differential. One came back completely
clean; the other found a 25px error and a residual worth more than the fix.

### `var()` is clean — 30 of 30

CSS custom properties are **31.6% of the burndown corpus** (the #7 construct) and had only ever had
CSSOM attention. A 30-row battery in geometry-bearing positions is Chrome-exact on every row,
including every case that would have been worth its own tick:

- substitution into `width`/`height`/`padding`/`margin`/`border-width`
- `var(--nope, 90px)` fallback, and a present variable *ignoring* its fallback
- **guaranteed-invalid**: `width:100px; width:var(--nope)` resolves to the **inherited** value
  (400px here) — not the earlier declaration and not `auto`
- a fallback that is not a length (`var(--nope, 3px 9px)`), and one that is a `calc()`
- inside `calc()`, `min()`, a shorthand; a variable whose value is itself a `var()`
- inheritance, scoping, specificity, and `--W` vs `--w` (custom properties are case-**sensitive**)
- `--x: ;` (empty but valid — substitutes nothing, so the fallback fires) and `--x: initial`

**A cleared construct is worth recording precisely because the next tick will otherwise look here.**
Frequency ranks where to look; a probe says whether anything is there. This is the probe saying no,
at the #7 construct, for fifteen minutes and no build.

### A `<select>`'s box ignores `line-height`; an `<input>`'s does not

Chrome draws a dropdown with the native widget's own metrics, so leading declared on it never reaches
its box. Same declaration, same page, border and padding zeroed:

```text
   <select style="line-height:40px"><option>a</option></select>   Chrome 17   ours 40
   <select>                              (the reference row)      Chrome 17   ours 15
   <input  style="line-height:40px">                              Chrome 40   ours 40  ← CONTROL
```

**The `<input>` row is the whole argument.** It carries the identical declaration and was already
exact, so the rule cannot be *"a form control ignores leading"* — which is what the select row alone
would have supported. A global `line-height` on `body` is ordinary authoring, and against it every
dropdown on the page was a full line-height tall instead of one line.

⚠ The gate asserts the two select rows **equal to each other**, not to 17, because our plain
`<select>` is 15 and Chrome's is 17. Banking a number would either assert one we do not produce or
bank 15 and force a re-bank when the residual below lands — the t1007 failure mode, where a gate's
reasoned reference value turns the next correct fix into a red wall.

### ⚠⚠⚠ The residual: a constant fitted at the one font size every fixture uses

`border:0; padding:0`, so the box *is* the content:

```text
   font-size      8      10    13.333    16      20    26.666     40
   Chrome        11      13      17      19      24      32       46
   ours           9      11      15      18      23      31       46
```

Ours is the face's own `ascent + descent + lineGap` — right at 40, drifting everywhere below.
`<textarea>` has the same shape: its `rows` height formula carries a `1.125` factor whose own comment
admits it is *"Chrome's own ratio at the control font"*, and it is exact at `rows=1` in the UA font
and 24-against-23 at `font-size: 20px`.

> **A CONSTANT FITTED AT THE ONE FONT SIZE EVERY FIXTURE USES IS INDISTINGUISHABLE FROM A MODEL.**
> The UA control font is 13.333px, so *every* form-control fixture anybody writes lands on the single
> point where a wrong constant and the right one agree. Vary the font size — one extra row.

This is the **third** instance in one session: t1043's border/intercept pair (`2.925·fs + 6` and
`2.75·fs + 8`, equal at 13.333 and nowhere else), t1044's cancelled leading term, and now this. The
real fix is named rather than guessed: the ratios exist because `stylo_engine` cannot reach a
`FontContext` to resolve `line-height: normal` — a plumbing tick. Shipping a fudge factor tuned to two
points instead of one would be the same mistake with a smaller error bar.

## A negative length is a parse error, and the INSTRUMENT parsed it differently from the product (t1059)

`width`, `height` and the four min/max sizing properties take `<length-percentage [0,∞]>`. A value
outside that range makes the **declaration invalid**, and an invalid declaration is *dropped* — the
cascade keeps whatever it already had. That is a different observable from clamping to zero:

```text
                                          Chrome   MinimalCascade   LIVE (Stylo)
   width:-5px                               400          0              400  ✓
   width:200px; width:-5px                  200          0              200  ✓   <- DECISIVE
   width:-5%                                400          0              400  ✓
   width:200px; max-width:-5px              200          0              200  ✓
   min-width:-5px; width:50px                50         50               50  ✓   <- CONTROL
   width:calc(50% - 300px)                    0          0                0  ✓   <- CONTROL
```

### The third column is the point

Every layout battery in this loop is styled through `MinimalCascade` — it is what the layout crate's
`layout_html` test helper builds. Running the same rows through the **live** binary's Stylo path,
*before any fix*, returns Chrome's answers on all four failing rows. The shipping renderer was never
wrong.

> **THE INSTRUMENT THE LOOP MEASURES LAYOUT WITH PARSED CSS DIFFERENTLY FROM THE ENGINE IT SHIPS.** A
> fixture row containing a negative length would have reported a layout defect the product does not
> have, and the search would have started in `layout_block`. This is the *"instruments lie"* class
> arriving through the one door nobody had checked — not the oracle, not the harness, not the score,
> but **the cascade the fixtures are styled with.**

It is still a real capability fix and precisely bounded: `engine/css`'s header says the
`--no-default-features` build **ships** `MinimalCascade`, and the wall compiles and gates that build.
So it lands on the headless engine and on the measuring instrument, and **moves the corpus by exactly
zero, by construction** — the default build's width comes from Stylo via `stylo_map::size_to_dim`.
No A/B was run: an A/B whose result is determined in advance is theatre, not evidence.

### Two rows locate the fix, and neither is the row that made you look

⚠⚠ **`width:200px; width:-5px`** is what puts the fix at the point of *application*. Reinterpreting a
negative width as `auto` down in layout answers 400 on the first row and **400 on the second**, where
Chrome says 200. Only declining to apply the declaration leaves the earlier one standing — and no row
that tests a single declaration in isolation can tell those two implementations apart.

⚠⚠ **`min-width` was already right for the wrong reason: its initial value *is* 0**, so clamping a
negative to zero and dropping the declaration agree at exactly one point. `max-width` initialises to
`none`, so the identical clamp takes the box to **zero width** — a blank element. Checking the min
half and inferring the max half would have cleared this.

⚠ `calc()` is deliberately not rejected — a negative *result* is legal at computed-value time and
clamps to 0 at used-value time. It is a control, and the wrong fix that rejects it was RED-run.

## The battery that agreed on 7 of 8 rows while the feature was unimplemented (t1062)

CSS Display L3's two-value `display: <display-outside> <display-inside>` is not a new set of layout
modes — it is the existing ones spelled as the pair they always were. `inline flow-root` **is**
`inline-block`; `block flow` **is** `block`. `MinimalCascade` parsed only single keywords, so every
two-value declaration was **invalid and dropped**, leaving the element at its previous value.

### Why the first fixture could not see that

A dropped declaration leaves an element at its UA default. So a `<div>` asked to be `block flow` is
400px wide whether the pair parsed or not, and `block flex` on a `<div>` with an explicit width
measures identically either way. **The battery was reading the UA stylesheet, not the parser** — and
it agreed with Chrome on 7 of 8 rows.

Rebuilt so every row is an element whose **default display differs from the one the pair asks for**:

```text
                                          Chrome    before    after
   <div  display:inline flow>x              8x17    400x18     8x17
   <span display:block flow>               400x20     0x0     400x20
   <span display:block flex>               400x20     0x0     400x20
   <div  display:inline flex>               50x20   400x20     50x20
   <div  display:inline grid>               50x20   400x20     50x20
   <span display:block flow-root>          400x50     0x0     400x50
   <div  display:inline flow-root>          50x20   400x20     50x20
   <div  display:inline table>              50x20   400x20     50x20
   <div  display:block flow list-item>     400x20   400x20    400x20   <- agrees by ACCIDENT
```

8 of 9 wrong, where the first fixture said 1 of 8. The `list-item` row is **kept on purpose**: it is
the marker for the class of rows the battery had to discard.

> **A ROW THAT AGREES BECAUSE THE ELEMENT ALREADY DEFAULTED TO THE ANSWER IS NOT A ROW.** The
> one-point-constant trap with the UA stylesheet as the constant.

### The live path was already right — the third instrument split of the session

Stylo parses the syntax natively, and `boxes --html` returns Chrome's numbers on the same fixture
before any change. This is `MinimalCascade` being brought up to the engine it ships beside. It still
matters: the `--no-default-features` build **ships** `MinimalCascade`, and **every layout battery in
this loop is styled through it**, so a future fixture using two-value `display` would have measured
the UA default and reported a layout defect that does not exist. (t1059 found the same split on
negative lengths; t1061 found the reftest runner blind to a path the product has.)

### Canonicalise, don't re-map

`two_value_display_to_legacy` rewrites the pair to the legacy keyword and falls through to the
existing single-keyword table, so there is **one** mapping. A second table is how two spellings of one
computed value drift apart — *one rule, N implementations*, installed on purpose.

## An XHTML `<style><![CDATA[ … ]]></style>` sheet was dropped in its entirety (t1075)

Found while chasing a single regressed reftest. `CSS2/tables/row-visibility-002` passed before t1073's
paint layers and failed after — and the cause was neither: **its reference rendered as nothing.**

```text
   the reference's <div>      expected 100×100 green     ours: 1184×0, unstyled
```

Its stylesheet is wrapped in `<![CDATA[ … ]]>`, the standard XHTML idiom for keeping `<` and `&`
away from the XML parser. **2,191 of the CSS 2.1 suite's 10,501 files use it**, tests and references
alike. The test had been passing because **both sides rendered blank**, and the moment the test side
became correct the pair diverged.

### The two cascades lose different amounts, and only one of them loses everything

```text
   MinimalCascade   `<![CDATA[` is a junk selector -> the FIRST rule is dropped, the rest parse
   Stylo (shipping) the sheet is rejected WHOLESALE -> the page renders completely unstyled
```

Chrome, on a three-rule fixture: **all three apply.** Ours applied none on the shipping path. The
fix is one function at `Stylesheet::parse`'s entry — strip a leading `<![CDATA[` and a trailing
`]]>` — and it reaches both cascades because `Stylesheet` stores the stripped text as its `source`,
which is exactly what `stylo_engine` re-parses.

⚠ Only the **outermost** tokens are removed, and the closing marker is the **last** `]]>`, not the
first: a `]]>` inside a string is content. An unterminated wrapper still yields its rules — dropping
the opener alone recovers everything, which is strictly better than dropping the sheet. All three are
negative rows in the gate, and all three are RED-proven.

### ⚠⚠⚠ The number, fully attributed — and 100% of the "losses" are the lie being deleted

`css/CSS2`, 9,221 reftests, before and after:

```text
   1606 passed · 4040 failed      ->      2272 passed · 3374 failed        net +666
     gained 834 — 820 (98%) have CDATA in the test or its reference
     lost   168 — 168 (100%) do
```

Every single one of the 168 losses is a CDATA file. They are tests that were passing **because both
the test and its reference rendered unstyled**, and a blank page matches a blank page. Making the
stylesheet work makes the reference correct, which reveals a divergence that was always there:
`borders` 57, `backgrounds` 28, `tables` 18, `css1` 11, `normal-flow` 11.

> **DELETING A LIE EXPOSES THE GAP IT HID** (t1027), and the 100% figure is what turns that from an
> assertion into a measurement. A −168 that is 100% explained by the fix's own population is not a
> regression; it is 168 defects that were invisible an hour ago, and they now have a work-list.

⚠ No banked `WPT:` directory moves — `css-position` (99/311), `css-display` (124/151) and `css-text`
(1235/2212) are identical before and after. `css/CSS2` has no ratchet row at all, which is surface
audit #47's finding and the reason none of this was visible until it was looked for.

## `dir="rtl"` was a layout input and not a CASCADE input, so every logical property resolved LTR (t1086)

Stylo maps every **logical** property to a physical one *inside* `compute_for_declarations`, against
its own `WritingMode` — the section above (t998) is about the ORDER that mapping runs in, and this is
about its INPUT. Manuk implemented `dir="rtl"` in `MinimalCascade`'s presentational hints only, and
then recovered the value onto the computed style **after** Stylo had finished:

```rust
    cs.direction = m.direction;   // stylo_engine.rs, the post-cascade recovery pass
```

That is enough for everything **we** resolve from `direction` — the bidi inline reorder, `text-align:
start/end` — and it is not enough for the one thing **Stylo** resolves from it. With no `direction`
declaration in any sheet, Stylo's writing mode was LTR on every element of every page.

**The measurement is a two-column battery, and the second column is the finding.** The same eight
logical properties, declared twice — once with the `dir` attribute, once with a `direction: rtl`
declaration (28 rows, headless Chrome, a 100px block in a 400px container):

```text
  row                              dir="rtl" ATTRIBUTE        direction:rtl STYLESHEET
  margin-inline-start:25px       Chrome 275  ours 300  ✗      Chrome 275  ours 275  ✓
  margin-inline-end:25px         Chrome 300  ours 275  ✗      Chrome 300  ours 300  ✓
  inset-inline-start:25px (abs)  Chrome 275  ours  25  ✗      Chrome 275  ours 275  ✓
  margin-inline:25px 60px        Chrome 275  ours 240  ✗      Chrome 275  ours 275  ✓
```

⚠⚠⚠ **THE STYLESHEET COLUMN WAS ALREADY PERFECT.** Stylo's logical resolution works and always did;
it was never told the direction. **A battery that tested only `direction: rtl` — the spelling a CSS
test writes — would have reported the area clean**, and the spelling the RTL web actually uses is the
attribute: `<html dir="rtl">` is how essentially every Arabic, Hebrew, Persian and Urdu site declares
itself. The general form is worth keeping: *when one feature has two spellings and only one of them
reaches a given subsystem, a fixture that picks the wrong spelling proves the opposite of the truth.*

The fix is two UA rules, and both are RED-proven by a different row:

```css
[dir="ltr" i] { direction: ltr; }
[dir="rtl" i] { direction: rtl; }
```

⚠⚠ **A CORRECT RTL IMPLEMENTATION IS WHAT HID IT.** A 61-row RTL battery run the same hour scored
**58/61** — inline reorder, alignment, wrapping, Arabic text, mirrored margins and nested `dir`
islands all Chrome-exact — because every one of those reads `ComputedStyle::direction`, which the
recovery had already fixed. Only the properties Stylo itself must map were wrong, and nothing in the
RTL area pointed at the cascade.

⚠⚠ **THE ` i` FLAG IS DOCUMENTATION, NOT BEHAVIOUR — and only running the mutation says so.** Dropping
it left the gate GREEN: `dir` is on HTML's list of attributes whose values selectors match ASCII
case-insensitively, and Stylo implements the list. Verified from the other side on the same build so
the green is not a blanket insensitivity of ours: `[data-x="abc"]` does **not** match `data-x="ABC"`,
with Chrome agreeing on all six rows. The gate's write-up originally asserted the flag was
load-bearing; the mutation refuted the author, not the code.

**Priced against the corpus, honestly.** Of 182 cached corpus sites, 4 (2.2%) carry `dir="rtl"` and 90
(49.5%) use a logical property; **1–2 do both**. The M1 anchor `m.youm7.com` moved by **zero** —
shape 84.9%, reading-order 24, identical to the banked sweep row before and after. This is a
high-usage, low-corpus-mass fix of exactly the kind [[constitution-check]] #72 names: the honest
report is *"the instrument cannot price this"*, not *"this bought nothing"*.

**Residue, measured and named, NOT fixed here.** Two further RTL defects the batteries found, each a
different mechanism and each unaffected by this fix:

- `float: inline-start` / `clear: inline-start` map to `Float::Left` unconditionally in
  `stylo_map.rs` — direction-blind, and it fails in **both** columns above (300 vs 0), which is what
  proves it independent of the `dir` attribute. Same shape as the `text-align: start/end` fix: the
  value must stay logical through mapping and resolve once direction is known.
- `refine_inline_static_positions` is skipped outright under an RTL base direction
  (`if bcs.direction != Rtl` at its call site), so an insetless `position:absolute` box in an RTL
  inline context takes the content box's LEFT edge. Chrome 300/380, ours 0/0.

## `getComputedStyle(el, '::before')` was answered about the ELEMENT (t1101)

The CSSOM's second argument was **read and discarded**. `getComputedStyle(div, '::before')` returned
the *div's* style object, and that is strictly worse than not supporting the argument at all:

```text
     <div id=x style="width:200px">          Chrome            what we returned
       #x::before { content: "sm" }
     ─────────────────────────────────────────────────────────────────────────
       cs.content                            "sm"              undefined
       cs.display                            inline            block     ← the DIV's
       cs.width                              auto              200px     ← the DIV's
```

Every value is present, plausible and about **a different box**. A caller cannot detect it. This is
the *wrong answer of the right type* — the same shape as t733 and as t1096's `S0.`/`S1.`, and the
reason the fix is gated on a row (`absent-display=inline`) whose whole job is to be `block` when the
bug is back.

### What reads it, and what the `undefined` costs

The breakpoint-detection idiom, which predates `matchMedia` in JS and still ships in Bootstrap-era
and Foundation-era code and in every hand-rolled copy of it:

```css
  body::before { content: "sm"; display: none }
  @media (min-width: 768px) { body::before { content: "md" } }
```
```js
  var bp = getComputedStyle(document.body, '::before').content.replace(/["']/g, '');
```

`undefined.replace(…)` is a **TypeError**, so the frame dies at boot — a throw-class killer, which
the board ranks above shape work. `content` was absent from `getComputedStyle` on **elements** too,
so this half was broken with or without the second argument.

### The parse surface has three quirks, and all three were MEASURED, not derived

Three batteries against Chrome (`::`/`:`/bare × known/unknown/miscased, plus non-string arguments).
The spec text does not predict any of these, and each has a plausible wrong answer:

```text
   '::before'  ':before'  'before'  '::BeFoRe'   → the PSEUDO
   ':BEFORE'   '::bogus'  '::'  '::before '      → an EMPTY declaration (length 0, every prop '')
   'Before'    'bogus'    '::part(x)'  0  {}     → the ELEMENT's own style
   ' ::before'                                    → the ELEMENT (no trimming, either side)
```

1. The `::` form is ASCII **case-insensitive**; the one-colon legacy form is case-**sensitive**.
   Lower-casing both arms is the obvious implementation and diverges on `:BEFORE`.
2. A **bare** name is honoured only as an exact lowercase legacy name and is otherwise *ignored*
   (element), not *rejected* (empty) — so `bogus` and `::bogus` take opposite branches.
3. A **functional** pseudo (`::part(x)`, `::slotted(x)`) also falls back to the element.

An unknown pseudo-element returns an **empty `CSSStyleDeclaration`**, and every property on it reads
`''` — a string, not `undefined`, for exactly the TypeError reason above. Ours is a `Proxy` so all
~124 published names answer without materialising them.

### `normal` computes to `none` on ::before/::after, and ONLY there

Chrome, asked to recite it rather than recalled:

```text
   getComputedStyle(div).content                  "normal"
   getComputedStyle(div, '::before').content      "none"     ← no rule at all
   getComputedStyle(div, '::first-line').content  "normal"   ← still normal
```

So the serialiser takes a flag rather than reading `cs.content` alone. And a pseudo with **no
declarations** is answered from `ComputedStyle::absent_pseudo_of` (= `inherit_from` the originating
element), never from the element's own style: Chrome reports the div's colour and font-size there,
because those inherit, and reports `display:inline · width:auto`, because those do not.

### Named limitations, recorded rather than approximated

- **A generated box has no `NodeId`**, so `layout_rect` cannot find it and every pseudo is serialised
  with `rect: None`. It costs exactly one row: an auto-sized **block** pseudo reports `auto` where
  Chrome reports the used px. Everywhere else it is exact — Chrome itself reports `auto` for an
  *inline* pseudo, and a specified `width:50px` falls through `used_dim_css` to the computed value on
  both engines. (This is the same structural fact as the fidelity probe's blindness to pseudos,
  surface audit #50: *no DOM node, no rect, no key*.)
- Two **adjacent string terms** (`content: "a" "b"`) are concatenated by the content parser into one
  part, so they read back `"ab"` where Chrome says `"a" "b"`. `content: "a" counter(x)` is exact.
  The rendering is identical; the fix is in the parser, not this seam, and asserting the current
  answer in the gate would pin the engine to it.
- `ContentPart` has no image term (`content: url(…)` reports `none`) and `Counter` keeps only the
  counter's name (`counter(c, upper-roman)` reports `counter(c)`).
- A pseudo with declarations but **no `content`** (`::before { color: red }`) is dropped by the
  cascade — it generates no box — so we report the inherited colour where Chrome reports `red`.
- `::first-line`, `::marker`, `::placeholder`, `::selection`, `::backdrop` are recognised as real
  pseudo-element names and answered with the inherited placeholder, which is Chrome's answer for the
  overwhelmingly common case of no author rule. `::before`, `::after` and `::first-letter` come from
  the real cascade.

**GATE** `G_COMPUTED_PSEUDO` — `get_computed_style_reports_the_pseudo_element_not_the_originating_element`,
23 rows, negative rows first. RED-proven twice: returning `PseudoReq::Element` unconditionally (the
pre-t1101 behaviour) reproduces the defect exactly — `absent-display=block`, `absent-width=200px`,
`blk-width=400px`, every `content` back to `normal` — and lower-casing the one-colon arm flips
`badcase` from `0` to `124`.

## The monospace default size is a property of the FAMILY, not of five tag names (t1103)

`font-size: medium` — the initial value — resolves against a **per-generic** base size: 16px for the
variable-width default, **13px when the computed generic family is monospace**. That is why `<code>`
famously renders smaller than the prose around it.

The engine wrote that rule as a UA **declaration**:

```css
  pre, code, kbd, samp, tt { font-size: 13px; }
```

and the comment directly above it stated the real rule correctly while the code underneath said
something else. Chrome, asked to recite it rather than recalled:

```text
                                                     Chrome         ours, before
   <code> in  body { font: 16px monospace }        16px  38.53      13px  31    TOO SMALL
   <code> in  div  { font-size: 20px }             20px  48.17      13px  31    TOO SMALL
   <code> at the default size                      13px  31.31      13px  31    control ✓
   <span style="font-family:monospace"> default    13px  31.31      16px  39    TOO BIG
```

### Wrong in both directions at once, and the tag list is why

A UA declaration **beats inheritance** by construction. So it pinned every `<code>` and `<pre>` to
13px across the huge majority of the web that sets a body font-size — documentation, wikis, blogs,
every site with a design system. And because it keys on the TAG it *missed* the element that
actually asks for monospace.

**The tag list is a constant fitted at one point** (the standing t1042-1045 trap): it agrees with
Chrome on exactly the row where nobody has set a font size — which is the row every fixture ever
written for it used. Varying the parameter that was held fixed (the ancestor's font-size) is what
made both errors visible in one battery.

### The fix is option 1 on the borrowed-engine ladder — a hook we already implement, stubbed

Stylo calls `Device::base_size_for_generic(generic)` from
`specified::FontSize::to_computed_value`. Our `StubFontMetrics` returned `16.0` and ignored the
argument. Returning **13px for `GenericFontFamily::Monospace`** makes Stylo do the whole rule,
including the half no UA declaration can express: *only while the size is still `medium`*. No fork,
no vendored patch, one match arm, and the UA sheet **loses** a rule rather than gaining one.

### What it is worth, measured

Product path (`boxes --html`, Stylo cascade): 4 of 4 rows Chrome-exact, and a 16-row table/intrinsic
battery went **15/16 → 16/16**.

`css/CSS2`: **+1 gained, 0 lost**, by a per-test state diff of all 5,660 rows against a same-hour
control binary. ⚠ The net against the *journal's* last banked number said **+5** — the old binary
actually scores 3,862, not the 3,858 recorded. *Diff the state, not the net* applies to the
BASELINE as much as to the delta.

The suite is not where this fix lives; a documentation page is. Same-hour A/B, three draws per arm,
both deterministic:

```text
   doc.rust-lang.org/book   cov 0.960916  shape 0.791024  n 713   OLD
                            cov 0.960916  shape 0.877980  n 713   NEW    +8.70 points
   html.spec.whatwg.org     72.2% → 72.2%      martinfowler.com   77.8% → 77.8%
```

Same instrument tag, same coverage, same 713-element sample — **ATTRIBUTABLE** by `sweep_diff`'s own
classifier (t1102), and more than twice the ±3.7-point spread t654 measured on an unchanged tree.

### Residue, named: reading_order 4 → 5 on that page, deterministic, and ours

Three draws per arm, no churn. It is not called noise here. The site is **not jarring-clean in
either arm** (TOL 2, and 4 and 5 are both non-zero), so no M1 site and no gate verdict moves; the two
pairs the report names are inline `<code>` siblings inside one `<p>`, the class whose line-breaking
this fix necessarily changes; and shape rose **+8.70 on the identical sample**, which a fix that
misplaced boxes could not do. The likeliest reading is the recurring one — **a correct fix makes a
pre-existing divergence measurable** — a pair previously *"too close to call"* at both engines'
tolerance and now a decided disagreement. That is a hypothesis. The experiment that settles it is
*dump all five pairs*; the report truncates its examples at two, which is a small instrument gap
recorded here rather than fixed.

### The route in, recorded because the subject was not what made me look

Wikipedia's dominant term is a nested table at **4430px where Chrome says 397** — 2,254 of 4,843
shape misses and 363 h-overflow on one anchor. The obvious hypothesis (an auto table never clamped
to its available width) was **refuted by a 4-row fixture in two minutes**: plain nested tables come
out byte-exact against Chrome. The battery then widened to 16 rows over everything that could make a
cell's min-content unshrinkable, and the one row that diverged was not about tables at all.
**Wikipedia's nested-table blow-up is still open and still unexplained.**

**GATE** `G_MONOSPACE_BASE_SIZE` — `the_monospace_default_size_follows_the_family_not_the_tag`, the
four measured rows with the two negative ones FIRST. RED-proven twice, and each arm fails a
*different* pair of rows: returning `16.0` for `Monospace` fails the two default-size rows;
restoring the UA declaration fails the two inherited-size rows. Either half alone leaves the other
direction broken, which is what makes both arms necessary.

## `:has()` was quadratic, and the metric that would have caught it was FROZEN (tick 1161)

Two findings, and the second is only reachable through the first.

**The metric's source had not been re-run since Jul 16.** `docs/loop/WPT-AREAS.tsv` is what the
primary per-tick progress metric is computed from, and it had gone twenty-six days and ~100 ticks
without a refresh while the loop steered by the total it produces. Refreshing it costs **~15 seconds
per area**. Every row had moved — `css/css-sizing` 12.0% → 27.6%, `css/css-fonts` 32.4% → 57.7%,
`css/css-flexbox` 6.2% → 36.3% — and one row came back carrying a **Bar 0 crash**.

> **A frozen metric is not a slow metric. It is a metric that cannot report a crash.**

**The crash is `css/selectors/invalidation/has-complexity.html`, and its title is the diagnosis:**
*":has() invalidation should not be O(n^2)"*. It builds 75,000 elements under one `<main>`. The
runner reported `CRASH (killed by a signal)` — the watchdog killing a page that had stopped
responding.

**Measured before theorising.** Each doubling of `n` cost 4×:

```text
      n      250    500   1000   2000   4000        75000 (the test)
    BEFORE    41    133    551   2074   8176 ms     ~48 MINUTES, extrapolated
    AFTER      8      9     20     36     68 ms     (linear on out: 8000->137, 25000->452)
```

The cascade visits every node, and `main:has(span) span` sends every one of those spans up to the
single `<main>` to re-run its subtree search: the work is `nodes × subtree`. **The `:has()` question
is asked of the ANCHOR, and the anchor is asked the same question over and over** — `main:has(span)`
has one answer for `<main>`, recomputed once per span. A memo keyed `(that :has() pseudo, that node)`
collapses it. **104× at n=4000.**

**The memo is SCOPED, not ambient, and that is the whole of its safety.** `HasMemoScope` is an RAII
guard that a cascade pass opens over a DOM it does not mutate. With no scope open there is no cache
and every call computes — so a caller that mutates between queries (`querySelectorAll` from script)
cannot read a stale answer, because it never had one to read. **Both cascade implementations open one
in the same tick** (`MinimalCascade::cascade_scoped` and `stylo_engine`'s `:has()` loop) — the *one
rule, N implementations* trap paid for at t720, t1027, t1131 and t1134.

⚠⚠⚠ **The first draft of the gate was blind to its own subject and reported the bug fixed with the
fix removed.** It carried only `main:has(...) .subject` rules, and the rule index buckets by the
**rightmost** compound — so exactly one element ever asked the question, and the count was three
either way. The rule that creates the quadratic is the one whose **subject is the repeated element**,
`main:has(span) span`. Restored, the gate reads **53 evaluations at 50 spans and 2003 at 2000**
without the memo.

⚠⚠ **The gate is a COUNTER, not a stopwatch** — a timing assertion on a shared box is a flake, while
*"how many times did the expensive thing run"* is exact and is what the fix is about. It asserts
correctness first: a memo that returns the wrong answer is a worse bug than the one it fixes, and a
pure speed assertion cannot see it.

⚠⚠⚠ **The Bar 0 is NOT closed.** The WPT test still crashes, because a second quadratic dominates it
in a different subsystem — named rather than suspected: `Page::relayout` *"recascades only when the
node count outgrew the style map"* (`engine/page/src/lib.rs:6167`), so each of the test's 75,000
`appendChild` calls triggers a **full re-cascade**. This fix makes each of those cascades linear; it
does not make there be fewer of them. **Incremental style invalidation is what closes it.**

⚠ **Two honest residues.** `domparsing` fell 188 → 149 on an unchanged denominator and **cannot be
attributed** — the old number came from a binary that no longer exists, so no same-hour control is
possible. And the WPT total is **92% `encoding` by test count** (1,127,434 of 1,225,493), so a
+1,300-subtest gain across the whole CSS surface moves the headline ~0.1pt: it is a good monotonicity
check and a poor sensitivity one.

## An invalid declaration is IGNORED — and `_ => Initial` applies it

CSS 2.1 §4.2: *"User agents must ignore a declaration with an illegal value."* Every keyword arm in
`MinimalCascade`'s `apply_declaration` was written as:

```rust
   s.text_transform = match v {
       "uppercase" => Uppercase, "lowercase" => Lowercase, "capitalize" => Capitalize,
       _ => TextTransform::None,        // ← garbage APPLIED as the initial value
   }
```

That is invisible until a **valid declaration came first**, at which point the invalid one silently
overrides it. Measured against live Chromium, `<span style="display:inline-block">wwwww</span>` at
16px proportional:

```text
                                  CHROME    before    after
   uppercase; banana               75.52      58        76     <- THE DEFECT
   uppercase                       75.52      76        76     <- control: we DO apply it
   banana only                     57.78      58        58     <- control: only-invalid IS initial
   uppercase; none                 57.78      58        58     <- control: a VALID override wins
```

> **The last two rows are what make this a rule about DROPPING rather than about garbage.** A fix
> that made the property *sticky* satisfies row one and breaks row four; one that made an
> unknown-only value *inherit* satisfies row one and breaks row three. Leaving the field untouched
> (`_ => return`) is the only shape that satisfies all four, and it is what "ignore the declaration"
> literally means.

⚠ **Each arm had to gain the keyword it was falling through to.** `_ => TextTransform::None` was
doing double duty as *"garbage"* and as *"`none`"*; deleting the fall-through without writing
`"none" => TextTransform::None` turns a real keyword into a no-op. Six arms, six restored keywords:
`none`, `normal` ×2, `none` ×2, `auto`, `ltr`.

### Why this is a SHIPPING bug and not just a fallback one

The shipping cascade is Stylo (`live-cascade-is-stylo-not-minimal`) and Stylo drops invalid
declarations correctly. The blast radius is exactly the properties `stylo_engine` **recovers** from
`MinimalCascade` because Stylo's servo build cannot express them — `text-transform`, `text-overflow`,
`object-fit`, `overflow-wrap`, `word-break`, `scroll-snap-*`, `scrollbar-*`, `-webkit-line-clamp`,
`letter-spacing`, `word-spacing`, `direction`. For those, this cascade's answer **is** the shipping
answer.

⚠⚠ **It moved zero WPT subtests — a full sweep of all twenty areas was byte-identical.** The suite
does not exercise "an invalid declaration after a valid one" for these six properties. Per check #72
the honest report is *"the instrument cannot price this"*, not *"this bought nothing"*: the pattern
(a design system shipping a vendor value the engine does not know, after the fallback it does) is
common, and the cost is a whole property silently reverting.

⚠ **Residual:** `apply_declaration` has ~200 arms and the `_ => Initial` shape is not unique to the
six fixed here; the rest are decided by Stylo on the shipping path, so they are fallback-correctness
debt rather than live defects. Sweeping them is a tick, with the same entry criterion — an arm may
drop its fall-through only once every real keyword it was absorbing has been written out.

## `:nth-child` worked, so the whole An+B family looked implemented (t1199)

**`document.querySelectorAll('em:nth-of-type(3)')` returned an EMPTY LIST.** So did
`li:nth-last-child(3n)`, `:first-of-type`, `:last-of-type` and `:only-of-type`. Measured on a
six-`<li>` list and a mixed `<p>`, against Chrome's answers:

```text
  li:nth-child(2n)          3   ← correct, and the reason nobody looked
  em:nth-of-type(3)         0   (Chrome 1)
  li:nth-last-child(3n)     0   (Chrome 2)
  #p :last-of-type          0   (Chrome 2)
  #p :nth-last-of-type(2n)  0   (Chrome 2)
```

`manuk_css`'s `Pseudo` enum carried **`NthChild` alone**. Every sibling of it fell through
`parse_pseudo`'s `_ => return None` arm — which drops the **whole selector**, not the unknown part.
An empty NodeList from a valid selector is the hardest failure shape to notice, because it is
indistinguishable from a page that genuinely has nothing to match. Same mechanism as t1194's
`:is()`, one enum away.

⚠⚠⚠ **THE WORKING MEMBER IS WHAT HID THE OTHER FIVE.** `:nth-child` answering correctly is a
positive result about the family that is true of exactly one of its six members, and a probe that
asks *"do An+B selectors work?"* gets `yes`. The general form, and it is the third time this project
has met it: **a family is not covered by its representative.** `parse_pseudo` is a flat match arm
list, so "which of these six are here" is a two-second read that nobody had a reason to do.

### `:first-of-type` is NOT `:first-child`, and a homogeneous fixture cannot tell them apart

In `<p><em>a</em><span>b</span><em>c</em><b>f</b></p>` the `<span>` and the `<b>` are each **first of
their type** and neither is anybody's first child. A fix that counted *all* element siblings returns
`e1` alone and passes every list-of-`<li>` fixture, which is why `G_STRUCTURAL_PSEUDOS`'s subject is
deliberately heterogeneous and the `<li>` list is only the control. RED-proven: with the type filter
removed, `firstOfType` reads `1:e1` instead of `3:e1,s1,b1`, `nthOfType2` reads `0`, and
`matches(':nth-of-type(2)')` reads `false` — while `homogeneousAgrees` stays `true`.

### One rule, two implementations — and only one of them was broken

The live cascade is **Stylo's**, which has always resolved these pseudos. So
`em:nth-of-type(2) { color: … }` *rendered* correctly the entire time while
`querySelectorAll('em:nth-of-type(2)')` found nothing: the page looked right and every script that
asked about it got an empty answer. The `cascadeAgrees` claim asserts the two engines now name the
same element rather than leaving the agreement to inference.

**Measured, same binary, same hour:** `dom` **6383 → 6671 (+288)**, `css/selectors`
**3547 → 3643 (+96)**, `html/dom` **56445 → 56445 (unchanged)**, 0 crashes in all three.

## An invalid selector must THROW — and calibrating on ONE corpus cost 289 subtests (t1200)

**`document.querySelectorAll('[')` returned an empty NodeList.** So did `querySelector('div,')`,
`matches('::example')` and `closest('^|div')`. All four are specified to throw a `SyntaxError`
`DOMException`, and the gap is not pedantry:

> **try/catch around a selector is how the web feature-detects selector support.** An engine that
> never throws answers *"supported"* for **every** selector — including the ones it silently cannot
> match — so the library takes the modern branch and gets an empty list forever.

Same shape as jQuery's `support.cors`: **ask what a library BELIEVES, not what it can detect.**

### Validity is NOT "did the matcher understand it", and conflating them is a capability regression

The one-line implementation is *"throw when `parse_selector` returns `None`"*. That parser returns
`None` for two unrelated reasons:

| selector | valid? | `parse_selector` | correct answer |
|---|---|---|---|
| `p::first-line` | **yes** — a real pseudo-element we do not model | `None` | empty list, **no throw** |
| `div:hover` | **yes** | `NeverStatic` | empty list, no throw |
| `::example` | **no** — unknown pseudo-element | `None` | **throw** |
| `[` | **no** | `None` | **throw** |

Throwing on the first two turns *"unimplemented"* into **an exception inside the page's own
script** — strictly worse than the empty list it had. So `manuk_css::selector_syntax_error` answers
**grammar only**, and modelling stays the matcher's business.

⚠⚠ **And Stylo is the wrong authority, even though it has a real selector parser.** Its *servo*
build returns `false` from `parse_has()`, so it **rejects `:has()`** — the construct this engine
hand-rolled a supplement for because 13% of the corpus uses it. `SelectorParser::parse_…` would have
made `querySelector(':has(.x)')` throw, deleting a shipped capability. `hasStillMatches` in
`G_SELECTOR_SYNTAX_ERROR` pins that trap as an assertion.

### ⚠⚠⚠ THE MEASUREMENT THAT CAUGHT THE REAL BUG: one corpus is not a corpus

Calibrated against WPT's `dom/nodes/selectors.js` alone — **34/34 invalid rejected, 0/207 valid
falsely rejected**, a clean score — the first landing measured:

```text
  dom            6671 → 6943   (+272)   ← exactly as predicted
  css/selectors  3643 → 3354   (−289)   ← a NET LOSS
```

**`css/selectors/attribute-selectors` writes CSS comments inside the selector under test** —
`[foo='BAR'] /* sanity check (match) */` — and a validator that has not stripped `/* … */` calls
those malformed. Two more fell out of the same re-run: `:is()`/`:has()` are **forgiving** selector
lists (`:is(:total-nonsense)` is valid and matches nothing, so recursion into them must not fail
closed the way `:not()` must), and attribute case-flags may be written as **hex escapes** (`\73`,
`\49`). None of the three is visible in the first corpus.

With comments stripped: `dom` **+272**, `css/selectors` **+38**, `html/dom` and `domparsing`
**unchanged**, 0 crashes — and no false throw on any of three real sites (`news.ycombinator`,
`en.wikipedia`, `theguardian`: identical box counts against the old binary, zero rejected selectors).
Both corpora — WPT's 241 and the 112 selectors `css/selectors` was observed passing in — are now
`selector_syntax.rs`'s own unit test, whole rather than sampled.

**Three rules exist only because a corpus refused an earlier draft**, and each is a general CSS fact
worth keeping: an unclosed `[` or `(` **at end of input is VALID** (CSS closes an open block at EOF,
so `[align="center"` and `::slotted(foo` are in WPT's *valid* list); **escapes are identifier
characters** (`.foo\:bar` is one class name); and `:nth-child()`'s argument is **An+B, not a selector
list** — recursing into it rejected `:nth-child(3n)`, which is zebra striping across the whole web.

## Three populations, three classes of miss: only the open web writes the shapes nobody would test (t1203)

The selector-syntax validator (t1200) was calibrated against WPT's own list, corrected against a
second, and **still threw on two valid selectors on a real site**:

```text
   www.unoeste.br   'G\:TEST'
   www.unoeste.br   'a[href*=\#]:not([href=\#]):not(.scroll-ignore):not([data-tab])…'
```

Both are escapes, and both are valid: `G\:TEST` is a **type selector whose name contains an escaped
colon** (VML-era markup, still shipping), and `a[href*=\#]` is the anchor-link idiom every
smooth-scroll and tab script is written with. t1200 taught escape handling to `#`, `.` and pseudo
names — and not to the type selector or the unquoted attribute value. **A partial fix, in the one
direction that throws.**

### The escalation, and it is the transferable part

```text
   WPT's 34 invalid + 207 valid   → perfect score, blind to CSS comments INSIDE a selector
   css/selectors' observed 112    → caught comments, forgiving :is(), hex-escape flags
   200 REAL SITES                 → caught escapes in the TYPE SELECTOR and the ATTRIBUTE VALUE
```

Each population caught what the previous ones structurally could not. *"One corpus is not a corpus"*
needed a third clause:

> **A spec corpus and a test corpus are both written by people who know the grammar. Only the OPEN
> WEB writes the shapes nobody would think to test.**

All three populations are now committed as `selector_syntax.rs`'s own unit test. RED-proven by
reverting the type-selector scan to a bare `is_ident_char` loop, which the gate reports in its own
words: `⚠ THE DANGEROUS DIRECTION … ["G\\:TEST"]`.

⚠ **The instrument was cheap and the bias held:** 3 rejections across 200 real sites, and no other
page lost a selector.

## A failure histogram cannot tell you what WORKS (t1205)

Tick 1204 histogrammed `css/css-values`' failing assertions, read the top rows, and published:
*`object-position` "is not being applied at all on the shipping (Stylo) path."* A four-minute direct
probe says that is wrong twice:

```text
   object-position: 20% 30%       →  20% 30.000002%   ← APPLIED. the float is the bug
   object-position: top           →  50% 0%           ← correct
   object-position: right bottom  →  100% 100%        ← correct
   object-position: 30px 50%      →  50% 50%          ← the length IS dropped …
```

…and the length fallback is **documented and deliberate** — the parser's own comment says
*"percentages relative to length (px) aren't fraction-convertible without the box, so they … fall
back to centered."*

**Why the histogram misled, and it generalises:** every `object-position` row in that suite happens
to involve a length, a `calc()`, or a Selectors-5 keyword. So a sample of *failures* showed a 100%
failure rate for a property that works for the majority case **the suite never tests**.

> **A failure histogram tells you what is broken among the things a suite CHOSE to test. It cannot
> tell you the property works — or does not — for anything else.**

This is *"grep the artefact, infer the engine"* one level up, and it is the same rule that caught the
UA-sheet count and the `zoom: 1` frequency: **name and RUN the code path before publishing a claim
about it.**

### The real defect, which nothing in that suite was testing

`getComputedStyle(img).objectPosition` answered **`20% 30.000002%`** for `object-position: 20% 30%`.
`ObjectPosition` stores each axis as a free-space **fraction** (`30%` → `0.3`) because that is what
the paint path needs, and the serializer did `0.3f32 * 100.0`. Every other percentage in the file is
fine — `Dim::Percent` stores the percentage itself — so this property is the one that round-trips
through a fraction.

Not cosmetic: comparing the string you wrote against the string you read back is how every animation
and layout library detects its own write, and `"20% 30%"` ≠ `"20% 30.000002%"` reads as *the write
was lost*. Same class as `undefined + ' scale(2)'` → `"undefined scale(2)"`.

`G_OBJECT_POSITION_COMPUTED` gates it, **refuses a blunt round** (`33.333% 66.667%` must survive
intact, so rounding to 2–3 decimals would pass the headline and destroy this), and **pins the
documented length limit** so the tick that widens the type must edit that line on purpose.
⚠ **+0 WPT** — no test in the area writes a plain percentage and reads it back, which is exactly why
the defect survived.

## The conversions existed; the CSSOM was lossy (t1210)

Constitution check #115 ranked `css/css-color` as the board's largest coherent mechanism — 94% of its
4,745 failures are `color()`, `color-mix()`, `oklch/lch/lab/hwb` — and steered *"port the
colour-space conversions"*. **One probe refuted that in four minutes:**

```text
   oklch(0.7 0.15 200)  → rgb(0, 185, 195)      lab(50% 40 -30)  → rgb(165, 91, 171)
   hwb(90 10% 20%)      → rgb(115, 204, 26)     color(srgb …)    → rgb(51, 102, 153)
   color-mix(in srgb, red 50%, blue) → rgb(128, 0, 128)
```

Every CSS Color 4/5 function resolves, correctly. **Stylo has had this all along**;
`values.rs::parse_color` (hex/rgb/hsl/named) is the *MinimalCascade fallback*, and reading the
fallback to infer the engine is *grep the artefact, infer the engine* wearing a different hat.

**The real mechanism, read from the assertions rather than inferred:**

```text
   expected "color(srgb 1 0.5 0.5)"   but got "rgb(255, 128, 128)"   ← the right colour
   expected "oklch(0 0 0)"            but got "rgb(0, 0, 0)"         ← the right colour
```

**The pixels are correct and the CSSOM is lossy.** CSS Color 4 says a computed colour keeps its
space; only legacy `rgb()`/`hsl()`/hex/named compute to `rgb()`. Ours flattens everything to `Rgba`
(four `u8`s), so the space is gone before `getComputedStyle` can see it. **A type change through the
cascade, not a conversion port.**

### The defect that WAS fixable today, sitting beside them

```text
   expected "oklch(0 0 0 / 0.5)"  but got  "rgba(0, 0, 0, 0.5019608)"
```

Alpha is a `u8` (`0.5` → `128`) and the serializer did `128 as f32 / 255.0`. Every translucent colour
failed its own round trip — every overlay, disabled control, shadow and hover tint on the web.

**The rule is "the shortest decimal that round-trips", so the fix is a SEARCH, not a precision**, and
`G_ALPHA_SERIALIZATION` proves it over **all 256 byte values**:

```text
   RED 1  the bare division                FAILED `half:rgba(0, 0, 0, 0.5)`
   RED 2  a FIXED 2 decimals (plausible)   FAILED `roundTripsEveryByte:all256`
```

Two decimals passes every hand-picked value and turns `2/255 = 0.008` into `0.01` → byte **3**; six
reproduces the artefact. **`css/css-color` 6260 → 6299 (+39)**, controls flat, 0 crashes.

## Half the computed-style surface is silent, and only 15 of it is a defect (t1214)

Surface audit #61 named **CSSOM lossiness** as a class — four instances in one session
(`object-position`, colour spaces, `characterSet`, `field-sizing`), each found by a different
accident — and prescribed the instrument: *enumerate the properties the cascade resolves, ask
`getComputedStyle` for each, list the ones it cannot say.*

**215 properties, one element, one run:**

```text
   SILENT (getPropertyValue returns "")            107 / 215   =  49.8%
     ├─ LOSSY   the cascade resolves it and the CSSOM will not say     15
     └─ HONEST  not in `ComputedStyle` at all — we do not model it     92
```

⚠⚠⚠ **The split is the whole product.** Silence is the *correct* answer for a property the engine
does not implement; it is a defect only when the answer exists and the serializer will not say it.
Cross-checking each silent name against `ComputedStyle`'s own fields separates them mechanically:

```text
   LOSSY (15):  align-content · background-position · justify-items · justify-self
                rotate · scale · translate · transform-origin · tab-size
                grid-auto-columns · grid-auto-flow · grid-auto-rows
                grid-template-areas · grid-template-columns · grid-template-rows
```

**`rotate`/`scale`/`translate`/`transform-origin` head the list by real-web weight** — the individual
transform properties every animation library reads before animating, and this project already carries
the scar: `undefined + ' scale(2)'` is `"undefined scale(2)"`, which is why `G_TRANSFORM` exists.
Same shape, four properties over.

⚠ **Six of the fifteen are the grid family, whose absence is recorded as DELIBERATE** (t1171-74):
Chrome reports the **used** track sizes, so echoing the specified value would be *a wrong answer of
the right type*. Re-read that decision before touching those six — which is why this instrument's
output is a **worklist with a question attached to each row**, not a list of bugs.

**The 92 are not a backlog.** They are properties the engine does not model; turning one from silent
into a value is a *capability* tick, not a serializer tick. The two look identical in this table and
are completely different work.

⚠ **What it cannot see:** one element, one document — a property that answers only under some
condition (a flex item, a grid child, a replaced element) reads as present. It measures *will the
serializer ever say this*, not *does it say the right thing* — and the four instances that motivated
it were all cases where the serializer **did** answer, wrongly. **A silence census and a correctness
census are different instruments; this is the first.**

## The census produced a worklist and the worklist paid (t1215)

t1214's census: 215 properties asked, **107 silent**, split into **15 LOSSY** (the cascade resolves
it, the serializer omits it) and **92 HONEST** (not modelled). This closes the seven lossy names with
real-web weight and no recorded reason to stay silent:

```text
   rotate · scale · translate · transform-origin · align-content · justify-items · justify-self
```

**`rotate`/`scale`/`translate` are the individual transform properties every animation library reads
before animating** — and `undefined + ' scale(2)'` is `"undefined scale(2)"`, which is
`G_TRANSFORM`'s whole reason for existing. **The same failure, four properties over**, invisible
until a census looked.

⚠⚠⚠ **`transform-origin` resolves its percentages.** The initial is `50% 50%` and Chrome reports
**used pixels**; a serializer echoing `50% 50%` would have added the property and kept the defect. On
a 200×100 box: `100px 50px`.

```text
   RED  transform-origin echoes the percentage    FAILED `originResolvesToPx:100px 50px`
   RED  rotate/scale/translate report "" not none FAILED `rotateNone:none`
```

The second matters more than it looks: **the initial value must be `none`, not `""`** — a property
answering the empty string is indistinguishable from one the engine does not support, which is
exactly what the census measured.

**Measured:** `css/css-transforms` **240 → 313 (+73)**, `css/css-flexbox` **1475 → 1482 (+7)**,
`css/css-grid` and `dom` unchanged as controls, 0 crashes.

⚠ **Not done, with reasons:** six of the remaining eight are the grid family, whose silence is
deliberate (t1171-74 — Chrome reports **used** track sizes). `background-position` and `tab-size`
await a tick that can price their serialization forms. **A census's value is that it makes "not doing
something" a decision with a reason.**

⚠ **`dom` first read 8140, a −2, and did not reproduce** — solo it is 8142. Second instance this
session of a −N evaporating on a solo re-run (t1212 was the first). **Re-run solo before believing a
−N.**

## The same round-trip bug for the third time (t1216)

`background-position` and `tab-size` were the two lossy names t1214's census left unpriced. The first
carried a trap this file had already paid for twice:

```text
   t1205  object-position      x/y stored as a FRACTION of free space   "20% 30.000002%"
   t1210  alpha                stored as a u8, divided by 255           "rgba(…, 0.5019608)"
   t1216  background-position  x/y stored as a FRACTION of free space   "30.000002% 70%"
```

> **Every property that stores a NORMALISED value for the paint path and serializes by multiplying
> back has this bug** — and it is invisible on any value that happens to land on a representable
> boundary, which is why it took three separate accidents to name.

`pct()` — written at t1210 for alpha — is shared here rather than re-derived, which is the only
reason the third instance cost minutes instead of a tick. RED-proven by restoring the raw
multiplication.

⚠ **A stale gate name kept on purpose.** `G_COMPUTED_LOSSY_SEVEN` now covers nine. t1215's journal and
`CONSTELLATION.tsv` row cite that name; renaming would leave both dangling — **precisely the
two-dialect rot surface audit #60 spent a tick untangling.** A stale name with a note in its own doc
is cheaper than a dangling citation.

### Where the census ended up

```text
   215 asked · 107 silent
     15 LOSSY   →  9 CLOSED (t1215 seven, t1216 two)
                   6 the GRID family, silent ON PURPOSE (Chrome reports USED track sizes)
     92 HONEST  →  not modelled; silence is correct, and turning one into a value is a CAPABILITY
                   tick, not a serializer one
```

**Nothing in that table is unexplained** — the state a census is for, and the state it was not in
three ticks earlier. `css/css-backgrounds` **445 → 466 (+21)**.

## A rule is only as portable as the context it encodes (t1217)

`css/selectors` reports **114 subtests** of *"invalid rule parsed into CSSOM expected 0 but got 1"* —
a stylesheet rule with an invalid selector must be **dropped** from `cssRules`, and we keep it. The
tool is already in hand: `manuk_css::selector_syntax_error`, built at t1200 and calibrated against
three independent populations.

⚠⚠⚠ **And it would drop VALID rules.** Measured:

```text
   selector_syntax_error("[foo[")        →  REJECT   ✓ correct, and what the 114 want
   selector_syntax_error("x|lang")       →  REJECT   ✗ VALID in a sheet declaring @namespace x
   selector_syntax_error("[x|lang='A']") →  REJECT   ✗ same
   selector_syntax_error("*|lang")       →  accept   ✓
```

The validator's rule is *"any non-`*` prefix is UNDECLARED, therefore invalid"*, which is **correct
for `querySelector`** — there is no `@namespace` in scope there, which is exactly why WPT lists
`ns|div` among its invalid selectors. **A stylesheet has `@namespace`.** And `css/selectors` already
records the matching failure: `@namespace x '…'; [x|lang='A']` reports *"rule didn't parse into CSSOM
expected 2 but got 1"* — **a rule we are already dropping and should not.**

The correct move is a namespace-aware variant, `selector_syntax_error_in_sheet(sel,
declared_prefixes)`, with the declarations threaded to the rule-insertion point.

> **A rule is only as portable as the context it encodes.**

Second instance in one session: t1210 nearly ported colour conversions that already existed, because
`values.rs::parse_color` is the *MinimalCascade fallback* rather than the engine. Both times the
check that caught it was **running the thing rather than reading it**.

## One-at-a-time is fine if you have candidates to ask (t1218)

CSSOM says a `CSSStyleDeclaration` exposes an IDL attribute for **every supported property**, set or
not. `el.style`'s `Proxy` answered `has` with *"is this property currently SET"*, so t1171 measured:

```text
   'display' in el.style                     FALSE   ← and 27 other names, 0/28
   el.style.gridTemplateColumns = '1fr 2fr'  → reads back "1fr 2fr"   ✓ set/get works
```

**`'prop' in el.style` is THE CSS feature-detection idiom.** Answering `false` for a feature the
engine *has* makes a page take its fallback **against a working engine**.

### The blocker t1171 named, and the half that was missing

> *"…`supports_condition` answers one declaration at a time and is not enumerable, so there is no
> list to hand the Proxy. Building that registry is the tick; guessing a list would re-create the
> `PARSE_ONLY_LONGHANDS` drift."*

**One-at-a-time is fine if you have candidates to ask.** `CANDIDATE_PROPERTIES` (263 names) is asked,
and `supported_property_names()` keeps whatever `supports_condition` — **the same evaluator
`@supports` and `CSS.supports()` use** — says yes to. The registry is *the oracle's answer*, not a
list anyone wrote down, which is exactly what the warning against guessing was protecting.

**Cost measured before it was paid:** 263 calls = **21ms**, spent **lazily on the first `in`**. A page
that never feature-detects never pays, and that measurement is why the design is lazy.

⚠ **It cannot regress anything, by construction.** The registry is a **lower bound** — a name outside
the candidate list is never asked and answers as before; a property that IS set answers `true`
regardless, because the registry is consulted only after the set-check fails. **`false` can become
`true`, never the reverse.** `agreesWithSupports` pins the guarantee that matters:
`('display' in el.style) === CSS.supports('display','flex')`.

**Measured:** `css/css-grid` **2421 → 2443 (+22)**, `dom` unchanged, 0 crashes. ⚠ `css/cssom` reports
`FILES 0` — the sparse checkout still omits it (t1176), so the area this most directly serves cannot
be measured at all.

## A shorthand SETS its longhands — that is what the word means, and `el.style` never did it (t1257)

CSSOM §6.7 gives a declaration block an IDL attribute for every *supported* property, and the
block's setter for a shorthand *"sets the longhand properties"*. So this is `'1px'` in every browser:

```js
el.style.margin = '1px 2px';
el.style.marginTop;            // '1px'   — ours: ''
```

`el.style` here is a Proxy over the style **attribute**, parsed into a flat dict keyed by the name
the author wrote. A shorthand stored ONE entry under its own name, and `margin-top` is a different
key. Every longhand of every shorthand read `''` — through `cssText`, the IDL setter, `setProperty`
and `setAttribute('style', …)` alike, because all four land in that same dict.

### The CONTROL row said it was not a grid bug

This was found aiming at `css/css-grid/parsing`, whose top failure signature is
`e.style.cssText = grid …` reading back `""` (310 subtests in that area alone). The obvious
hypothesis is *"the `grid` shorthand is not parsed"* — so the probe carried a control row,
`margin: 1px 2px` → `marginTop`, on the theory that a working shorthand would separate the two:

```text
    probe                                         expected   ours
    grid: 150px 100px / 200px 300px → rows       "150px 100px"   ""
    margin: 1px 2px → marginTop                  "1px"           ""    ← CONTROL, failed too
    margin-top: 1px → marginTop                  "1px"           "1px"  ← CONTROL, passed
```

And layout was never involved: the same document lays out `grid: 150px 100px / 200px 300px`
byte-for-byte identically to the longhand spelling. **`margin`, `padding`, `border`, `background`,
`font`, `flex`, `gap`, `inset`, `place-items`, `transition`, `animation`** — all of it, on every
page. A one-area histogram found a whole-CSSOM defect because one row in the battery was not about
grid.

### The expansion is Stylo's, asked for — not reimplemented

`parse_style_attribute` has **already** expanded the shorthand by the time it returns; the longhand
values were sitting in the `PropertyDeclarationBlock` and nothing asked for them.
`stylo_engine::expand_declaration` enumerates them with `ShorthandId::longhands()` and serializes
each through the SAME `property_value_to_css` that `serialize_declaration` uses. That matters: a
longhand read must be byte-identical whether the author wrote the longhand or the shorthand, and two
serialisers cannot promise that. Same hook shape as `SerializeDeclFn`, same reason (`manuk-js` must
not depend on the CSS engine), same conservative polarity — with no engine installed the expansion
is empty and a longhand read is exactly where it was.

### A READ-side overlay, and the control rows are what forces that

The map is layered over the read, not written into storage. `cssText` must still round-trip the
author's own text, and `length` / `item(i)` still enumerate what was **declared**:

```text
    el.style.cssText = 'margin: 1px 2px'
      el.style.margin      '1px 2px'        the shorthand still reads back
      el.style.cssText     'margin: 1px 2px' the author's own text, not four longhands
      el.style.length      1                DECLARATIONS, not longhands
      el.style.marginTop   '1px'            the fix
```

A fix that expanded into storage passes the first three assertions of the gate and silently changes
all four of these.

### Declaration ORDER is the whole of the cascade rule this surface owes

Two rows, and only an in-order merge satisfies both:

```text
    'margin: 5px; margin-top: 9px'   → marginTop '9px'   (fails if the read prefers the expansion)
    'margin-top: 9px; margin: 5px'   → marginTop '5px'   (fails if the read prefers a direct entry)
```

The first spelling is the one a reader expects and the second is the one that catches a
short-circuit, which is why the gate carries both. The expansion map is therefore built by walking
the declarations **in order**, merging a shorthand's longhands and a direct longhand alike at the
position each was written.

### Ledger — same-hour OLD-BINARY control

```text
  area                     old            new            Δ
  css/css-grid            3891/10911     4125/10911    +234
  css/css-values          2199/4153      2240/4201      +41   ⚠ denominator +48: more tests RAN
  css/cssom               2785/3498      2794/3502       +9   ⚠ denominator +4, same effect
  css/css-flexbox         1504/3907      1538/3907      +34
  css/css-sizing          1094/2409      1097/2409       +3
  dom                     8142/10503     8142/10503       =   CONTROL
```

⚠ On `css-values` and `cssom` the DENOMINATOR moved, so read the count and say why: a helper that
reads a longhand back no longer gets `''`, so files that previously aborted at their first assertion
now report their remaining subtests. That is real, and it is not the same kind of number as
css-grid's `+234` at a fixed denominator.

### Named residual, measured and not fixed

`getComputedStyle(el).gridTemplateRows` is **`undefined`** — not `''`, undefined: the property is not
on the computed-style object at all. That is a different surface (the computed-style mirror, not the
declaration block) and a different tick; recorded here so the next reader does not rediscover it from
the same probe.

## The cascade held the whole `grid-*` family and `getComputedStyle` published NONE of it (t1269)

`getComputedStyle(el).gridAutoFlow` was `undefined`. Not `""`, not the initial value — **absent**;
`'gridTemplateAreas' in getComputedStyle(el)` was `false` and
`getPropertyValue('grid-auto-rows')` returned `""`, for an element the very same `ComputedStyle` had
just laid out as a grid container. `ComputedStyle` has carried `grid_auto_flow`, `grid_auto_rows`,
`grid_auto_columns`, `grid_template_areas`, `grid_column` and `grid_row` as typed fields since grid
landed, and `engine/layout/src/taffy_tree.rs` consumes every one. Only the CSSOM object declined.

That is invariant **I3** failing on the property family a grid layout is *made of*, and it is the
`transform` shape again — applied for sixty ticks before the number reached JS. **Not a wrong value,
an absent one**, which is the harder half to notice because the page looks right.

`css/css-grid`: **7484 → 7687 on comparable solo runs (+203)**, and the assertions it clears name
themselves — `assert_true: grid-auto-rows doesn't seem to be supported in the computed style`,
`assert_in_array: gridTemplateAreas value undefined not in array ["none"]`.

### Two serialisation rules that are Chrome's answer, not shortcuts

- **The shorthand omits a trailing initial component.** `grid-column: 1` reads back `"1"`, not
  `"1 / auto"`; `grid-row: span 2` likewise. Only a genuinely two-sided placement keeps the slash.
- **`grid-template-areas` is a RECONSTRUCTION, and that is the honest direction.** The cascade stores
  Stylo's resolved line *rects*, not the author's rows, so the value is rebuilt: `"head head" "nav
  main" "foot foot"`, one space between cells, `.` for a cell no named area covers. Re-emitting the
  author's exact bytes would be the *wrong* answer — Chrome normalises too.

### ⚠⚠⚠ `grid-template-columns` / `-rows` are STILL absent, on purpose, and the gate asserts the absence

Theirs is one of the few resolved values CSSOM §5.1 defines as the **used** value, so Chrome answers a
grid container with laid-out track sizes in px: `repeat(3, 1fr)` on a 900px grid reads back
`"300px 300px 300px"`. This engine holds the *specified* tracks only. Publishing them from the cascade
would answer `"repeat(3, 1fr)"` — a **wrong answer of the right type**, which every caller then does
arithmetic on. `undefined` at least tells the truth (t608: *a name is defined IFF the thing it names
exists*), and `G_GRID_COMPUTED_STYLE` assertion (5) pins it so nobody "completes the family" by
accident.

⭐ **The withholding has a fully specified exit, which is what makes it a decision rather than a
limitation.** Taffy computes the used tracks and offers them through
`LayoutGridContainer::set_detailed_grid_info` — a trait method whose **default body is the no-op we
inherit today** — under a `detailed_layout_info` feature that taffy 0.12 **already enables by
default**. `DetailedGridTracksInfo::sizes` is the `Vec<f32>`. Option 1 on the borrowed-engine ladder:
no fork, no patch, one trait method.

⚠ **The hazard to design around first is ours, not taffy's.** `solve_subtree` also runs during
INTRINSIC MEASUREMENT, so a side-table keyed by node id will be written by a probe whose outputs are
contractually discarded — exactly how `pre_transform_rect` was poisoned permanently at t1120 by being
first-write-wins. Ask *which passes can write* before choosing the polarity, and remember the §9.1
grid re-solve runs after the main pass, so "last write wins" is not automatically the final one either.

## A media query evaluates in FOUR states, and a `bool` gets `not` backwards (tick 1276)

`media_query_matches` returned `bool`. Media Queries Level 4 needs **four** values, and the two
missing ones are not pedantry — they are the only way `not` can be right.

| State | What it is | Why a `bool` cannot hold it |
|---|---|---|
| `Unknown` | `<general-enclosed>`: a well-formed `( … )` naming a feature this UA does not recognise | MQ4 §3.2 gives it Kleene logic. `not Unknown` is **`Unknown`**. Collapse it to `false` first and negate second and you answer **`true`** |
| `Invalid` | a grammar failure | MQ4: such a query *"must be replaced with `not all`"*, and the replacement is at the **whole-query** level, so it survives an enclosing `not`. `not )` is false, not "the negation of a false thing" |

Both collapse to `false` at the top level, which is exactly why the old evaluator looked correct on
every query that contained no `not` — and every query on the web that *does* contain one was
inverted. `@media not (some-feature-from-2029) { … }` applied a sheet written for a browser we are
not.

⭐ **The old code documented its default as the safe direction, and it was safe in only half of the
sentence.** The comment read *"an unknown feature evaluates FALSE — the safe direction, because the
alternative is applying a dark-scheme or print sheet to a light screen."* That is true of a bare
`(unknown)`. Put a `not` in front and `false` becomes `true` and the sheet is applied — the exact
outcome the comment claims to prevent. **A default is only safe with respect to the operators that
can wrap it.**

### `or` did not exist, and nested parens failed by the same shape

`split_media_terms` split on ` and ` only. So `(min-width:0) or (min-width:99999px)` reached the
feature lookup as ONE term, and its outer parens were removed by
`strip_prefix('(') + strip_suffix(')')` — which for a two-block string strips the **first** block's
open and the **last** block's close, yielding the nonsense `min-width:0) or (min-width:99999px`.
Feature lookup failed; the query was false. **Every `or` query on the open web evaluated FALSE.**
`((a) or (b)) and (c)` is the same shape and failed identically. The fix is `strip_outer_parens`,
which only strips when the closing paren is the one that opened the string.

### A `<media-condition>` is NOT a `<media-query>` — one string, two correct answers

```
<media-query>     = <media-condition> | [ not | only ]? <media-type> [ and <media-condition> ]?
<media-condition> = <media-not> | <media-in-parens> [ <media-and>* | <media-or>* ]
```

A **condition cannot contain a media type**. `sizes` takes the condition production, so
`sizes="not print 100vw, 1px"` resolves to a **1px** slot — the first `<source-size>` is a grammar
error and is discarded — while the identical text after `@media` is a query that matches on screen.
`sizes` was calling `media_matches` (the query production), answering `100vw`, and fetching a
different bitmap for the same page. `manuk_css` now exposes both entry points and the caller picks.

Mixing `and` and `or` at one level without parentheses is a **syntax error**, not a precedence
question: `a and b or c` has no agreed reading, so the spec refuses to guess and so do we.

### An out-of-range value invalidates the FEATURE; it does not merely fail to match it

`(min-width: -1px)` used to parse to `-1` and answer **true** — an 800px viewport is `>= -1`. A
negative length is not a valid `<media-feature>`, so the block is `<general-enclosed>` → `Unknown` →
false, and crucially `not (min-width: -1px)` does not become true. Same for a unitless non-zero:
`(min-width: 600)` is not a length, though `(min-width: 0)` still is.

**Measured:** WPT `html/semantics/embedded-content/the-img-element/sizes` **512/795 → 632/795**
(+120), with `css/cssom`, `css/css-values` and `css/css-backgrounds` numerators byte-identical.
Gated by `G_MEDIA_GRAMMAR`, which drives `matchMedia`, `sizes` and a real `@media` block in one page
so that the CSS path and the JS path are proven to reach the *same* evaluator.

⚠ **Still wrong, measured here and deliberately left for its own tick:** the **boolean context** of
the `no-preference` family. `(prefers-reduced-motion)` with no value returns `true`, but the boolean
form asks *"is it engaged?"* and we are a no-preference browser, so it must be `false`. The idiom
`@media (prefers-reduced-motion) { * { animation: none !important } }` is everywhere, and today it
kills every animation on the page.

## `( feature )` and `( feature: default )` are different questions (tick 1277)

MQ4 §2.4's **boolean context** — a media feature written with no value — asks *"is this feature
**engaged**?"*. It is not shorthand for *"does its default value match?"*, and `eval_feature` had no
boolean branch at all: an empty value fell through to the value comparison, where a few arms carried
an `is_empty()` escape hatch and the rest did not.

| Written | We answered | Correct | Why |
|---|---|---|---|
| `(prefers-reduced-motion)` | **true** | `false` | the "false" value is `no-preference`, and that is us |
| `(prefers-contrast)`, `(forced-colors)`, `(inverted-colors)` | false | `false` | right, but by luck — no `is_empty()` arm |
| `(orientation)`, `(prefers-color-scheme)` | **false** | `true` | their value sets contain no "false" value, so they always match |
| `(scripting)` | **false** | `true` | we run scripts |
| `(width)`, `(height)` | **unknown** | `true` | non-zero |

⭐ **The loud one was switched on.** `@media (prefers-reduced-motion) { *, *::before, *::after
{ animation: none !important } }` is in most modern CSS resets, and we **matched** it — so every
animation on such a page was disabled, by a browser with no reduced-motion preference at all, one
tick after the engine learned to interpolate keyframes.

⭐⭐ **It hid because the common spelling was always right.** `(prefers-reduced-motion: reduce)` — the
form nearly every site writes — took the value path and answered correctly throughout. **A bug in the
rare spelling of a common feature is invisible to any test that reaches for the common spelling**,
which is what a hand-written media-query test does by reflex.

### A keyword outside the feature's own value set is INVALID, not merely non-matching

`(orientation: sideways)` is `<general-enclosed>` → `Unknown` → false, and therefore
`not (orientation: sideways)` is **false**. Answering a plain `false` here negates to a positive
match — the identical shape as tick 1276's `not (unknown-feature)`, surviving one level deeper
because the *feature* was known and only its *value* was not. Every keyword feature now carries its
allowed set, the same way lengths carry their non-negative range.

⚠ **This mechanism has no local WPT tree.** `css/mediaqueries` is not in the checkout (`ls ~/wpt/css`
— no such directory), so the correction is gated (`G_MEDIA_GRAMMAR`, 44 rows, RED on eight
mutations, eight of the rows CONTROLS) and **measured at zero**. The controls — `sizes` 632/795,
`css/cssom` 2794, `css/css-backgrounds` 4087, `css/css-sizing` 2840 — are byte-identical, which is
all a suite without the relevant tree can honestly say.

## A `<link rel=stylesheet>` is SCRIPT-blocking, and the sheet list exists in THREE places (t1296)

**The half we honoured and the half we did not.** `<link rel=stylesheet>` is render-blocking *and*
script-blocking. This engine honoured the paint half — the final layout always contained the author's
sheets — and skipped the script half: `Page::from_dom` cascades, lays out, and **then** runs the
document's blocking `<script>`s, while both async constructors applied the fetched external CSS only
after `from_dom` had returned.

The minimal form is four lines, and note that nothing here is a cascade bug:

```html
<link rel="stylesheet" href="local.css">   <!--  .k { display: grid }  -->
<div class="k" id="k"></div>
<script> /* at parse time: display = "block" — at `load`: display = "grid" */ </script>
```

⚠ **It is a MEASUREMENT bug and the page is the one measuring.** A carousel reading `offsetWidth`, a
framework snapshotting `getBoundingClientRect` before hydration, a breakpoint check on
`document.body.clientWidth` — all get UA-default geometry, write a wrong answer into the DOM, and
**no later cascade can undo it**: the sheet arrives afterwards and restyles a tree the page already
mis-built. This is invisible to every screenshot, box dump and fidelity score, because the *final*
paint is correct.

### The design, and why the obvious cheap version does not reach

t714–t719 already tried three shapes of this and reverted two; the table lives in
`engine/page/tests/g_css_before_lifecycle.rs`. The landed t719 design starts the sheet fetches before
the parse and, at the apply point, takes only handles that have **already finished** — never
awaiting. Moving that harvest one phase earlier (before `from_dom`) is equally free, and it banks
**nothing** on a fast origin:

```text
   html parse 0ms · external scripts 2ms · module graph prefetch 0ms   <- the whole head start
   the sheet lands at ~5ms
```

So a wait is required, and it is affordable because of **what makes it conditional**:

| bound | why it holds |
|---|---|
| only when a blocking `<script>` exists | with none, nothing can observe the difference — and this is precisely the counter-example (*"dead sheets and NO scripts"*) that killed the t716 design |
| never past `nav_start + load_budget()` | `G_LOAD`'s 2x-page bound is arithmetically untouched |
| never more than `load_budget() / 4` | a slow origin cannot convert this into the phase t715 reverted |

The already-fetched bytes reach construction through `PENDING_EXTERNAL_CSS`, a thread-local seed of
exactly the same shape as `PENDING_CSP` and the external-`<script>` set: the fact is known before
`from_dom` and needed *during* it, so seed, then construct. `from_dom` takes it once, so it can
neither leak into the next navigation nor into a subframe.

### ⚠⚠⚠ THE SHEET LIST EXISTS THREE TIMES, AND TWO OF THEM ARE NOT THE CASCADE

This is the part worth carrying forward. With the seed in place the debug line read `seeded=1
bytes=22 sheets=1` — the author's sheet **was** in the first cascade — and the probe still printed
`display: block`. The three lists:

| # | where | built from |
|---|---|---|
| 1 | `from_dom`'s initial cascade | `initial_sheets_with_external` (light tree — shadow `<style>` stays scoped via `collect_shadow_stylesheets`) |
| 2 | `Page::apply_stylesheets` | `collect_style_sources` (flat tree) + the caller's URL→text map |
| 3 | **`ReflowScope::install`**, for the forced reflow a blocking script triggers | the map `from_dom` hands it — which was `HashMap::new()` |

`getComputedStyle` **forces a reflow**, list 3 rebuilds the cascade from an empty external map, and
the document is silently un-styled *by the very call made to measure it*. The comment above it said
this was "a fact rather than an omission — nothing has been fetched", which was true when written and
which my own change in the same tick made false.

> **A stale comment converts from documenting a limitation to justifying a bug the moment the
> limitation lifts.** When you make a "nothing has been fetched here" comment false, grep for its
> siblings before believing the fix is inert.

### What it was worth, and how the number had to be read

Old binary vs new, same hour, isolated runs, **fixed** denominators: `css/css-grid/parsing` +44,
`grid-model` +16, `grid-definition` +16, `layout-algorithm` +6 — **+82, zero losses**, with
`css/css-values/calc-size` (no external CSS) as an unmoved control.

⚠ The whole-area total *fell* (`7022/14257 -> 6991/14101`) and that reading is an artefact: the
entire delta is two `*-interpolation.html` files whose **subtest count** changed, and the same
directory run in isolation gives `1094/1840` on **both** binaries. A transition-timing test's subtest
count depends on its position in the batch, so only fixed-denominator directories are comparable
across a full-area run.

## Sampling an animation at a time is not a clock (t1301)

An engine with no compositor timeline cannot show you the in-between frames of an animation as time
passes. It can still answer *"what value does this animation HAVE at time T"* — and those are different
questions. Conflating them cost this project a standing mis-ranked lever for two constitution checks.

### What the web's animation tests actually do

WPT's `css/support/interpolation-testcommon.js` backs all 194 `*-interpolation.html` files across twelve
areas. It has four legs, and **not one of them advances a clock**:

| leg | how it pins the sample |
|---|---|
| CSS Transitions | `transition-duration: 100s` + `transition-delay: **-50s**` — 50s in at t=0 |
| CSS Transitions with `all` | same |
| CSS Animations | `animation-duration: 100s` + `animation-delay: **-50s**` |
| Web Animations | `animation.pause(); animation.currentTime = 50 * 1000` |

Every leg lands at progress **0.5 at time zero**, then maps 0.5 onto the progress it actually wants with
`createEasing(at)`, which emits exactly four shapes: `steps(1, end)` for 0, `steps(1, start)` for 1,
`linear` for 0.5, and `cubic-bezier(0, b, 1, b)` otherwise.

⚠⚠⚠ **So "we need an animation clock first" was priced from the word *animation*, not from the failing
mechanism.** A clock is real work for pages that genuinely animate; it is not what gates these subtests,
and ranking it as a prerequisite deferred a fix that was available immediately. This is the same rule
constitution check #117 wrote for refusals — *justify it by the failing message, not by the subject* —
applied one level up, to a **prerequisite** instead of a refusal.

### The defect

`element.animate()` fast-forwarded: a microtask applied the LAST keyframe whenever `fill` was
`forwards`/`both`, and `currentTime` was a plain data property. A test could write `50000`, read `50000`
straight back, and the element would not move — the *wrong answer of the right type*. That is why a
probe case asserting `currentTime` round-trips PASSED while the case asserting the sampled value FAILED.

`currentTime` is now an accessor whose setter samples: progress `currentTime / duration`, through the
effect's easing, then the value between the two surrounding keyframes.

⚠ **The guard is half the fix, and without it the sampling is invisible.** The fast-forward is queued on
a microtask; the harness pauses and seeks *synchronously*, so the microtask ran afterwards and stamped
the last keyframe over the sample just taken. `_settle` now refuses a `paused` or `idle` animation.

### Interpolable vs discrete is decided by the numeric SKELETON

Split each endpoint into its numbers and the text between them. `10px 20px` and `20px 30px` share the
skeleton `["", "px ", "px"]`, so the numbers interpolate pairwise. `1fr 1fr 1fr` and `2fr 2fr` do not —
different token counts — so the pair is **discrete** and holds `from` until progress 0.5, then takes
`to`. That is precisely WPT's `expectFlip`, and getting it wrong is invisible: interpolating a
non-interpolable pair still yields a plausible string, and roughly half the assertions in every
interpolation file expect the untouched `from` value.

### ⚠⚠ Two ways this gate was vacuous, both found by running the mutation rather than reasoning

- The midpoint case first read the computed value **inline**. Read there it is `0.5` even with the
  fast-forward still armed, so removing the `_settle` guard left the gate **green**. It now reads after a
  microtask — the window WPT measures in.
- The assertions are `contains` checks, and **`steps:0` is a prefix of the wrong answer `steps:0.5`**, so
  ignoring easing also stayed green. Numeric readings are bracketed now: `steps:[0]`, `midsample:[0.5]`.

### ⚠⚠⚠ And read the LEG's distinct failing names, never the area total

`css/css-grid` run twice on the **same binary** gave `7243/13928` and `7487/14215` — a ±250 spread,
larger than the +242 this fix appeared to buy. The stable key is distinct failing subtest names for the
leg the fix touches: **Web Animations 282 → 111 → 110**, against a same-binary churn floor of ~40 names.
An area total on this area cannot resolve a change this size; do not quote one.

## A rule's `.style` is the member that does the work (t1302)

`document.styleSheets[0].cssRules[0]` carried `cssText`, `selectorText`, `type`, `parentStyleSheet` and
`parentRule` — and no `style`. Every part a reader *inspects* was correct, so the object looked
finished; the one member that *mutates* anything was absent, and
`rule.style.setProperty('color', c)` threw `TypeError` on the property access before it could reach the
cascade. That is the canonical CSSOM write: theme switchers, design-token editors and CSS-in-JS
runtimes all perform it.

### A view over the element's text, never a parallel model

The whole `<style>`-sheet bridge is a view over the element's own `textContent`, because that text *is*
the cascade's source of truth — writing it re-cascades through machinery that already exists. `style`
follows the same rule: reads re-parse the rule's declaration block, and `setProperty`/`removeProperty`
splice that block back into `el.textContent`.

⚠⚠ **The read/write pair is bound to the rule's INDEX, not to its text.** `__syncRules` rebuilds the
rule objects whenever the source string changes — and a write *is* such a change. Bind the text and you
hand the caller a rule whose `.style` addresses a sheet that no longer exists; bind the index and both
halves stay live across the rebuild the write itself triggered. Same class as *a value derived from a
snapshot is wrong for everything created after it*, one subsystem over.

### Three decisions worth not re-deriving

- **A `Proxy`, not a fixed property list.** `CSSStyleDeclaration`'s IDL surface is every CSS property
  there is. Enumerating a subset makes `rule.style.color` work while `rule.style.rowGap` is silently
  `undefined` — the false-presence shape this bridge exists to avoid.
- **`setProperty(name, '')` removes** (CSSOM §setProperty step 5). Otherwise clearing an override
  leaves `color: ` behind, which parses as nothing and drops the whole rule on the next round-trip.
- **At-rules get no `.style`.** `CSSMediaRule` has no declaration block; an empty one answers a
  question the spec says to answer with `undefined`.

Declaration splitting tracks **paren** depth, so `rgb(1, 2, 3)` and `url(data:…;base64,…)` survive —
the same lesson rule splitting already learned about brace depth and `@media`.

### ⚠ The subtest count is not the claim

`css/cssom` moved `2794 → 2802` and `css/css-values` `3322 → 3363`, on stable denominators. Small, and
reported small: WPT's CSSOM tests overwhelmingly drive `el.style`, which already worked. `rule.style` is
what *pages* use. The load-bearing assertion is a **box** — an element reaching 321px because a rule was
mutated through `cssRules[0].style.setProperty` — because a read-only view passes every API-shape check
and moves nothing.

### Still out of scope, deliberately

`<link>`ed sheets remain absent from `document.styleSheets`, and `<link>.sheet` stays `undefined` rather
than `null` (t663: for an applied linked sheet, `null` is a lie that reads as honest). A linked sheet's
text does not exist on the JS side, so the view has nothing to view. Publishing fetched stylesheet text
to JS is the tick that unlocks it.

## Interpolating a value TEXTUALLY is a second implementation, and it is 4.9× worse (t1303)

t1301 taught `element.animate()` to sample at a time, and interpolated the value **in JavaScript** by
splitting each endpoint into its numbers and the text between them. Matching skeletons interpolate
pairwise; mismatched ones flip discretely at progress 0.5.

That law is right for simple values — `0`→`1`, `10px 20px`→`20px 30px` — and it is what moved
`css/css-grid`'s Web Animations leg 282 → 110 and `css/css-transforms` +605. **It is the wrong law for
any value with structure.**

`transform` interpolates per transform-function, with `none` as the identity and the shorter list padded
with identities (css-transforms-1 §Interpolation of Transforms). So:

```text
   from [none] to [translate(200px) rotate(720deg)] at 0.25
     want  matrix(-1, 0, 0, -1, 50, 0)     got  none
```

The skeleton test correctly notices the two strings share no structure, and then does the wrong thing.

### ⚠⚠⚠ The number that exposes it is a SIBLING LEG

WPT's interpolation harness runs the same expectations through several legs. In `css/css-transforms`:

| leg | interpolator | failing |
|---|---|---|
| CSS Animations | Stylo's `Animate::animate(Procedure::Interpolate)` | **92** |
| Web Animations | the JS numeric-skeleton path | **450** |

Same properties, same expectations, **4.9×**. `engine/css/src/animation.rs` has borrowed the correct,
per-property-type interpolation for many ticks — its own doc says everything numeric is *"borrowed, per
the ladder in STATUS.md — option 1, no fork"* — and a second implementation was written in JavaScript
anyway. **A duplicate that is right on the easy half reads as working**; only a sibling doing the same
job better makes it visible.

The repair is to route the Web Animations sample through that same Stylo path (cascade with
`property: from`, cascade with `property: to`, `Animate` at the progress, serialize back) via a host
hook from the prelude, keeping the skeleton path only as the fallback for values Stylo declines.

### ⚠ And measure a STABLE-denominator area before pricing a shared-harness fix

t1301 priced itself at −171 distinct failing names in `css/css-grid`, correctly refusing that area's
total because the same binary twice spread ±250. It never ran `css/css-transforms`, whose denominator is
identical run-to-run — where the same fix is **+605** on a number needing no error bar. A fix to a
harness *leg* moves every area that harness backs; find the area that can actually resolve it.

## The SVG property set is absent from the cascade, and `CSS.supports` is honest about it (t1305)

SVG renders in this engine because **resvg parses presentation attributes itself**: `<rect fill="red">`
is correct, and so is every attribute-styled graphic. What does not exist is the CSS half —
`getComputedStyle(el).fill` for an element styled by a CSS rule returns `""`.

Measured: `CSS.supports` is **false for all 14** of `fill`, `stroke-width`, `stroke-dasharray`,
`stroke-dashoffset`, `fill-rule`, `color-interpolation`, `path-length`, `cx`, `cy`, `r`, `rx`, `ry`,
`x`, `y`. Not incomplete — absent.

### Cause: a codegen gate in the dependency, not a pref

```toml
# stylo-0.19.0/properties/longhands.toml
[stroke-width]
type   = "SVGWidth"
struct = "inherited_svg"
engine = "gecko"          # ← same on fill, stroke, fill-rule, cx, cy, r, rx, ry, x, y, …
```

Stylo generates its SVG longhands for the **gecko** build only; we build **servo**. So this is not
ladder option 1 (there is no runtime pref) and not really option 2 either: it is ~30 longhands *plus* the
`inherited_svg` style struct the servo build lacks, and STATUS.md already re-priced "take Gecko's answer"
for `:has()` — the workspace depends on `stylo = "0.19"` from crates.io, so it means `[patch.crates-io]`
→ a local fork, re-applied on every bump.

⭐ **`CSS.supports` answering `false` is therefore HONEST** — a genuine absence honestly reported, not the
false-presence shape. Nothing here is gated, because a gate on today's `""` would pin the engine to a bug
and a gate on the right value would be permanently red.

### The viable path is option 3, and it has a clean seam

Resolve the SVG property set in **our own** cascade layer — the same selector engine that backs
`querySelectorAll`, which STATUS.md already nominated for the `:has()` supplement — and hand the resolved
values to resvg **as presentation attributes**, the interface it already consumes. CSS must win over the
attribute; that is the whole point of `.icon { fill: currentColor }`. Subsystem-sized; scope it as one.

### ⚠ A verdict does not travel between areas

`svg`'s top failure message is `assert_true: 'from' value should be supported` (522 subtests, 29%).
Constitution check #120 examined that *same message*, priced it at 2,024 subtests in
`css-grid`/`css-values`, found it was `CSS.supports` over `<flow-tolerance>` and `calc-size()` —
pre-shipping features — and correctly declined it. In `svg` the same message is `stroke-width`,
`fill-rule` and the SVG2 geometry properties, all shipped in Chrome for years. Check #117's rule (*cite
the failing message, not the test's subject*) needs one more clause: **and not the same message's verdict
in a different area.**

## The opacity reveal-hack must not overwrite a PLACED animation (t1307)

`stylo_map.rs` forces `opacity: 0` to `1` on any element that declares an animation. The rule is real and
measured — **52 of 237 corpus sites pair `opacity: 0` with an animation**, and the commonest animation on
the web is a fade-in whose base rule hides the element, so a static renderer that shows the first frame
literally renders *nothing*. `prefers-reduced-motion: reduce` is the same idea blessed by the spec: show
the destination, skip the journey.

⚠⚠⚠ **But its comment opens *"We cannot animate"*, and that has been false since
`engine/css/src/animation.rs` landed.** `s.opacity` at that point is the value Stylo's `Animate` produced
for the element's *current position*, not the base rule. So the branch stopped rescuing a base rule and
started **overwriting a correctly computed one**, for every animation an author had placed at a
transparent point.

### The narrowing, and why the original win survives

The distinction is already in the CSS, so no heuristic is needed:

| delay | meaning | correct static answer |
|---|---|---|
| `>= 0` | the animation has **not started**; `opacity: 0` is the journey's first frame | show the destination — the hack fires |
| `< 0` | the author placed it **partway through** deliberately | the value at that point — the hack must not fire |

A negative delay is the device WPT's entire interpolation harness uses, and it is how the real web
expresses staggered list entrances (`animation-delay: calc(-0.1s * var(--i))`), out-of-phase tickers, and
scrubbed animations. `animation_delay_at` is read from Stylo rather than re-derived (I2, ladder option 1).

### ⚠ How it was found: a value wrong in ONE property

The symptom was `steps(1, end)` reading opacity `1` where it must read `0` — reproducible from a plain
stylesheet, with no script — which reads exactly like a broken easing function, and was diagnosed as one
for a whole tick (t1306). The seven-arm probe that settled it:

```text
   linear 0.5 ✓   ease not-an-endpoint ✓   steps(1, start) 1 ✓
   steps(2, end) 0.5 ✓   steps(4, end) 0.5 ✓
   steps(1, end) on a LENGTH → 0px ✓        ← the tell
   steps(1, end) on OPACITY  → 1  ✗
```

> **A value wrong in one property and right in every other names the SPECIAL CASE, not the shared path.**
> Nothing about an easing function knows which property it is easing, so the fault could not live there.
> Before blaming a shared path, run the same input through a second property.

### ⚠ The audit this opens

**Which workaround comments name a limitation the engine no longer has?** *"We cannot animate"* was false
for many ticks, silently, and cost a tick to misdiagnose. Every such comment is a checkable claim about
the engine, and a workaround whose premise has died does not merely become dead code — it starts
corrupting whatever replaced it.

## A PAUSED animation is not an ABSENT one (t1308)

`samples_for` used to skip any animation whose `animation-play-state` is `paused`, and
`iteration_progress`'s doc filed it in the not-running space *"(we have no way to have started it)"*.

**That conflates *not advancing* with *not existing*.** `paused` freezes the timeline; it does not delete
the animation, and a frozen animation still has the position its `animation-delay` gives it. The document
clock is **0** for a static render, so nothing advances for a *running* animation either — `paused`
therefore changes nothing at all about the value to compute, and the skip was pure loss.

What it cost, measured on one declaration (`animation: k 100s -50s linear paused forwards`):

| property | got | want |
|---|---|---|
| `opacity` | `1` (the initial value) | `0.6` |
| `width` | **`784px`** — `width: auto` filling the container | `70px` |

The element cascaded **as though it had no animation at all**, so every animated property was lost, not
just opacity.

### ⭐⭐⭐ One extra probe arm decides which organ to open

This is the companion to t1307, and together they form a rule:

> **A value wrong in ONE property and right in every other names the SPECIAL CASE.**
> **A value wrong in ALL of them names the SHARED PATH.**

t1307's symptom was `steps(1, end)` giving the wrong opacity — and the *right* `0px` on a length, which
exonerated the shared easing code and convicted the opacity reveal-hack. t1308's symptom was wrong on
opacity *and* on width, which convicted the shared sampling path. Same probe shape, opposite conclusion,
and neither is guessable from a single-property reading.

### The correct behaviour, including at rest

With `delay: 0` and `paused`, progress is 0 and the element sits at its **first keyframe** — not at the
base rule and not at the end. It composes with t1307's narrowing: a non-negative delay landing on
`opacity: 0` still triggers the reveal, a negative one does not.

⚠ Real pages that write this declaration: `pause-on-hover` marquees and tickers, paused-by-default
spinners, and every CSS-driven scrubber.

⚠ **And two dead premises in two ticks, both in the animation path** (*"We cannot animate"*, *"we have no
way to have started it"*). Both were true once, both stopped being true when `crate::animation` landed,
and neither was re-read. **A workaround's comment is a checkable claim about the engine** — sweeping them
is now a standing item.

## `element.animate()` is a synthesized CSS animation, and `currentTime` is a negative delay (t1309)

There is one interpolator in this engine and it is Stylo's `Animate`. `element.animate()` reaches it by
**expressing itself as CSS**: a `@keyframes` block written into a shared `<style>` element, `animation-*`
set inline, and `currentTime` written as **`animation-delay: -<currentTime>ms`** — seeking to T is starting
T ago. No native bridge, no host hook.

```text
   animate([{opacity:0},{opacity:1}], {duration:100000, easing:'linear'})  +  currentTime = 50000
     ⇒  @keyframes __manuk-waapi-0 { 0% { opacity: 0 } 100% { opacity: 1 } }
        animation-name: __manuk-waapi-0; animation-duration: 100000ms;
        animation-timing-function: linear; animation-delay: -50000ms
```

### Why: the number that proves the duplicate is gone is an EQUALITY

t1301 interpolated strings in JavaScript instead, which was a second implementation of something the
engine already owned. WPT's interpolation harness runs the same expectations through several legs, so the
duplicate was measurable directly — and so was its removal:

| leg | interpolator | before | after |
|---|---|---|---|
| CSS Animations | Stylo's `Animate` | 92 | **92** |
| Web Animations | JS strings → *now the same CSS path* | 450 | **92** |

**92 and 92.** The two legs now fail on precisely the same expectations, because they go through one
interpolator; a residual difference would have meant a residual duplicate. `css/css-transforms`
`3016 → 3669` (+653) on a denominator that has been stable at ~5500 across four runs — the
stable-denominator area to price this kind of fix on.

### ⚠ Three construction notes, each paid for

- **Do NOT set `animation-play-state`.** The first cut set it from the Animation's `playState`, and
  `paused` **suppressed the animation entirely** — every paused case read its un-animated default. t1308
  fixed that skip in the engine, but setting it here is still meaningless: the clock is 0, so nothing
  advances either way, and the negative delay already carries the position.
- **`@keyframes` text goes through `<style>.textContent`, not the CSSOM bridge's `insertRule`** — that
  keeps the prelude free of an ordering dependency on the bridge.
- **`cancel()` must clear the whole declaration**, or a cancelled fade leaves its element frozen mid-fade
  forever.

### ⭐ The gate survived a total change of mechanism

Every `g_web_animations` case written for the JS interpolator — `midsample:[0.5]`, both discrete cases,
and `steps:[0]` — is green through this completely different implementation. That is the strongest
available statement that those assertions describe **behaviour** rather than **implementation**.

### ⚠⚠ Removing a duplicate is an INTEGRATION, and the wall may not be watching

The first attempt (t1306) was reverted because deleting the duplicate went red on `steps(1, end)` — the
duplicate's hand-rolled easing had been the only reason that assertion was ever green, while the engine's
own path was wrong for a different reason (the opacity reveal-hack, t1307) and `paused` was being skipped
(t1308). **Fix what the duplicate was covering first.**

⚠ And this tick found that t1308 had landed a **real regression invisibly**:
`g_keyframe_interpolation`'s `n3` control asserted a *paused* animation reads its base rule — pinning the
very bug t1308 fixed — and the wall was green anyway, because `verify.sh` names ~19 gates explicitly while
`engine/page/tests/` holds **492**. When changing a shared path, run the gates that *mention* it; do not
infer coverage from a green wall.

## A CSS TRANSITION NEEDS A MEMORY, WHICH IS THE ONE THING AN ANIMATION DOES NOT (t1310)

`@keyframes` closed at t1301 and Web Animations at t1309, both through the same interpolation core.
The **four transition legs** of `css/support/interpolation-testcommon.js` were still untouched, and
they were the largest named mechanism left in `css/css-transforms`: **594 failing subtests on each of
the two plain legs**, the same idiom one property over —
`transition-duration: 100s; transition-delay: -50s` places a transition at its half-way point at time
zero, and the timing function is warped so the fixed sample eases to the tested progress.

**What made this a different build from the animation one.** An animation's two endpoints are both
written down in the stylesheet, so sampling one is a pure function of the cascade. A transition's
`from` endpoint is *the value the element had before the style change* — it appears in no rule, no
keyframe, no declaration anywhere in the document. The only place it can come from is **what the last
cascade published**. So `transition.rs` owns a per-node before-change table (`PREV`, rebuilt and
published wholesale each pass so a removed node stops being remembered) and `animation.rs` owns no
state at all. Everything numeric is still borrowed: `ComputedValues::transition_properties` expands
`all` and drops `none`, `LonghandId::is_animatable` / `is_discrete_animatable` classify, `Animate`
interpolates, `ComputedTimingFunction::calculate_output` eases.

### ⚠⚠⚠ THE GUARD: sample only when the elapsed time is GENUINELY POSITIVE

The document clock is **0** — nothing in this engine advances `animation::time_ms` yet. So an ordinary
`transition: width .3s` has `elapsed = 0 - 0 = 0` and sits at progress **0**, which is its *start*
value. A sampler without this guard renders every hover, accordion, drawer and menu on the real web in
the state it was **leaving**, on the majority of the corpus. With it, a transition is sampled only when
its delay is NEGATIVE — the author placing it in the past, which is exactly the WPT idiom and
essentially absent from real pages. This is not a workaround for a missing clock: it is what a clock at
0 *means*. `g_transition_interpolation`'s `n1` is that control.

### ⚠⚠⚠ THE BEFORE-CHANGE VALUE IS KEPT WHILE THE TRANSITION RUNS

Overwriting it every pass makes a transition survive exactly one cascade: the next recalc sees
`before == after` and answers the end state. The harness makes that failure **certain** rather than
unlikely — it runs `interpolate()` on *every* target and only then `measure()`s them, so the first
target is re-cascaded once per later target before anybody reads it. The gate reproduces that ordering
on purpose, and `t1` sits in front of thirteen later rows for exactly this reason.

### ⭐⭐⭐ DISCRETENESS IS A PROPERTY OF THE VALUE PAIR, NOT ONLY OF THE PROPERTY ID

`transition-behavior` looked like a single test on `LonghandId::is_discrete_animatable`. It is not.
`transform` is a continuous property and
`matrix3d(2,0,0,0, 0,2,0,0, 0,0,0,0, 0,0,0,1) → matrix(3,0,0,3,0,0)` still has no midpoint: the
from-matrix is **singular**, so the spec makes *that pair* discrete. The behavior keyword therefore has
to be consulted a **second time, on the pair**.

⚠ **And `Ok` from `Animate` is not the same thing as an interpolated value.** When two transform lists
do not match function-for-function, Stylo returns `Ok` carrying a *deferred*
`InterpolateMatrix { from_list, to_list, progress }` — and its servo build never resolves it
(`generics/transform.rs` answers `Transform3D::identity()` under a `TODO`), while our `stylo_map.rs`
drops the operation on its `_ => {}` arm. **The value that reaches the page is the identity matrix: an
element that should be half-way between two transforms sits completely untransformed.**
`animation::is_unresolvable` names that case and routes it into the discrete arm, so the page gets one
of the two real endpoints instead of no transform at all — strictly better at every progress, exactly
right at 0 and 1. Resolving `InterpolateMatrix` properly is a named, open lever.

**How this arm was found is the reusable part: it was a measured REGRESSION against the tick's own
same-hour control run, not a reading of the spec.** The first cut interpolated the singular pair, and
the control showed the two plain transition legs going from 0 failures to 7 on that file — 22 subtests
across four legs — while the area total was up by a thousand. *An area total large enough to be proud
of will hide a regression that a per-leg histogram against the old binary shows immediately.*

### Measured (same binary twice, same hour, `css/css-transforms`, denominator 5500 in both)

```text
   before   3697 / 5500 = 67.2%
   after    4735 / 5500 = 86.1%       +1038

   failures by harness leg              BEFORE   AFTER
     CSS Transitions                      594      85
     CSS Transitions with transition:all  594      85
     CSS Transitions + allow-discrete       3       0
     CSS Transitions all + allow-discrete   3       0
     CSS Animations                       151     144   ← the is_unresolvable fix, not transitions
     Web Animations                       151     144   ← ditto
```

⭐ **The two plain transition legs are 85 and 85 — exactly equal**, the same equality signature t1309
used for Web Animations vs CSS Animations: two legs that run the same expectations over the same
properties now fail on precisely the same ones, because they go through **one** sampler. A residual
difference would have meant a residual second path. There is none.

### ⚠⚠⚠ A 20-MINUTE SWEEP OVER A TREE THAT IS BEING EDITED IS NOT A BASELINE

The post-tick sweep credited this change with **+3,183**, including `css/selectors +382` — an area with
two files that so much as mention `transition`. The old-binary control settled it in one run:
`css/selectors` measures **4139 on the pre-tick binary and 4139 on the post-tick one**, deterministic
and flat. The banked mark of 3757 came from a sweep that ran ~20 minutes across 25 areas *while three
ticks' code was landing in the working tree*, so an area measured 3rd and an area measured 10th were
scored against **different source trees**.

Re-measured on the pre-tick binary, the same hour, the attributable total is **+2,458**:

```text
   css/css-transforms  3697→4735  +1038      css/css-position  1004→1166  +162
   css/css-sizing      3860→4320   +460 ⚠    css/css-ui         891→1003  +112
   css/css-backgrounds 4461→4891   +430      css/selectors     4139→4139     0  ← CONTROL
   css/css-flexbox     2594→2850   +256
   ⚠ css-sizing's denominator moved 5892→5850; the fixed-denominator read is +418.
```

**The lesson generalises past this tick: an area's banked mark and a fresh reading of the same code can
differ by hundreds, so a tick may only difference against a baseline it took ITSELF, on the same
binary, in the same hour.** And when six areas move, the row that proves it was the mechanism is the
one that *should not have moved and did not*.

## A COLOUR THAT IS NOT LEGACY sRGB KEEPS ITS OWN FUNCTION — and the engine was throwing the space away (t1312)

CSS Color 4 splits colour serialization in two, and the split is not cosmetic:

| kind | examples | serializes as |
|---|---|---|
| **legacy sRGB** | `#663399`, `rebeccapurple`, `rgb()`, `hsl()`, `hwb()` | `rgb(102, 51, 153)` — 0–255 **integers** |
| **everything else** | `color()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `color-mix()`, relative `rgb(from …)` | its own function — 0–1 **floats** |

⚠⚠⚠ **Every colour in this engine reached the CSSOM through `Rgba { r, g, b, a: u8 }`**, so the
colour SPACE was discarded at the `stylo_map` boundary and one `format!("rgb({}, {}, {})")` served
them all. `color: rgb(from rebeccapurple r g b)` — the identity relative colour — read back as
`rgb(102, 51, 153)` where every browser says `color(srgb 0.4 0.2 0.6)`. Same colour, wrong space,
**off by a factor of 255 to anything that parses the numbers** — which is exactly how WPT reported it,
1,169 times in one file: *"expected 0.4 but got 102"*.

And for a wide-gamut colour it is not even the same colour: `color(display-p3 1 0 0)` is outside sRGB
and has no `rgb()` spelling at all.

### ⭐ The whole rule was already in the borrowed engine

Stylo's `impl ToCss for AbsoluteColor` (`stylo-0.19.0/color/to_css.rs:74`) is CSS Color 4
§Serializing verbatim — the legacy branch, the `lab()`/`lch()`/`oklab()`/`oklch()` branch, the
`color(<space> …)` branch for every predefined space, `none` components, and the alpha rule. The
engine was **computing that value on every cascade and discarding it**. One `Option<String>` on
`ComputedStyle` publishes it. Ladder option 1, no fork, nothing re-derived.

### ⚠ The discriminator is Stylo's FLAG, not our keyword list

`ColorFlags::IS_LEGACY_SRGB` is set by the colour parser when the author wrote a hex, a named colour,
`rgb()`, `hsl()` or `hwb()`, and it is the same flag the spec's serialization branches on — so the
prediction cannot drift from the branch it predicts. **The colour SPACE would have been the wrong
test**: `hsl()` computes in `ColorSpace::Hsl`, which is not sRGB, and yet is legacy. `g_modern_color_
serialization`'s `n3` is that row.

### ⚠ The legacy path is deliberately untouched

`color_css` is `None` for a legacy colour, so every hex, named colour, `rgb()` and `hsl()` on the open
web keeps the byte-for-byte answer it already had — including the alpha serialization fitted against
Chrome at t1205, which a second implementation would have quietly re-derived.

**Measured: `css/css-color` 7856 → 10473 on a denominator of 11,337 in both runs — +2,617, 69.3% →
92.4%**, from about fifteen lines. Three files carried 2,585 of it
(`color-computed-relative-color` 1,169, `color-computed-color-mix-function` 948,
`color-computed-color-function` 468) and all three failed the same single way.
