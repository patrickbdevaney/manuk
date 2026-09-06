# The area tie-break was a proxy — and it was load-bearing

Both hit-tests break an equal-layer tie by **smaller area**. That looks like a proxy standing in for
two different questions, and right about only one:

| pair | what area says | what CSS Appendix E says |
|---|---|---|
| an ancestor and its descendant | the smaller box is the deeper element | the deeper one paints on top — **agrees** |
| two *unrelated positioned* peers on one layer | the smaller wins | they paint in **tree order** — the later wins whatever its size — **disagrees** |

So a large positioned overlay loses to the small positioned link it covers. t1465 recorded that as a
known limit; t1468 tried to close it and **was refused**.

## What the fix looked like, and why it was so appealing

`A11yNode::hit_test`'s `across` only compares candidates from **different subtrees** —
ancestor/descendant is resolved structurally by `go`, which was the whole point of the t853 rewrite.
So the area term looked deletable outright:

```rust
if b.z > a.z || (b.z == a.z) { b } else { a }   //  ← tree order wins. WRONG.
```

Every fixture agreed. Chrome-exact on eight rows across three gates, including a nesting case and an
overlay-declared-first case, plus **cssom-view 1316 → 1319** (three subtests in
`elementFromPoint-ellipsis-in-inline-box.html`) with `css/css-position` flat.

## The wall refused it: 62 links the browser cannot find

```text
  ✗ clickability 83.2% — 62 links the browser cannot find
```

**This is the t853 regression, exactly**, and t853's own comment predicts it: *"16 links on the G6
page became unclickable, because the shell walks up from whatever was hit looking for an `<a href>`
and an ancestor `<li>` has no link above it."*

The error is in the premise. `across` does not only see *positioned peers*; it sees any two
candidates from different subtrees, and most of those are **in-flow**. In-flow painting is not one
layer ordered by tree position — Appendix E splits it: block backgrounds are step 4, floats step 5,
**inline content step 7**. An inline link inside an earlier block must paint above a later block's
background, and "later tree order wins" hands the click to the later block.

⭐⭐ **Area was not a proxy for depth. It was a proxy for the whole of steps 4–7**, which happen to
order small-inside-large the same way. Replacing it needs those steps modelled, not a tie-break
swapped.

## Two mutations had already said the shape was wrong

A `depth` term was written first and mutations proved it **inert** — removing it changed no answer
anywhere. That was read as "document order subsumes depth". The wall's reading is better: *both*
terms were failing to distinguish the in-flow cases, so neither could see the difference. **An inert
guard sometimes means the fixture cannot see the case, not that the guard is redundant** — and a
corpus of 8 hand-built rows plus one WPT directory could not see it, while the clickability metric
over real pages could.

## What still stands

The parts of this thread that **did** land are unaffected, because they only ever *added* a layer
term and never touched the tie-break:

- t1465 — a positioned `z-index: auto` element is CSS 2.1 step 8, above in-flow content.
- t1466 — `elementFromPoint` and `elementsFromPoint` consult the same `manuk_css::stacking_layer`.

The step-8 peer case remains open, and it is now open *with a named cost*: closing it by tie-break
alone is worth −62 clickable links. See [[a-banner-wins-the-click]], [[one-hit-test-not-two]].
