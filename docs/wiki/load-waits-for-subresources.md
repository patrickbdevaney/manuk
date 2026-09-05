# `load` fired before the images — so every `window.onload` handler measured an unfinished document

**Tick 1442.** `css/css-grid` failing **3725 → 3636 (−89)**; `css/css-sizing` **−2**;
`css/cssom-view` flat. `grid-minimum-size-grid-items-021.html` **72/144 → 126/144**.

## The defect

HTML's *"the end"* steps run the load event only once the document's list of in-flight fetches is
empty, and **images are on that list**. `Page::load_async` fired it before the subresource phases, and
said so in a comment:

> *"The subresource phases have not run yet, but the document and its frames are ready, which is what
> `load` waits for."*

That is not what `load` waits for.

> ⭐ **The symptom is an undecoded image, and it has a signature: one axis wholly right and the other
> wholly wrong.** An `<img>` whose natural size has not arrived has `naturalWidth === 0`, so its
> intrinsic ratio is unavailable and any height derived from a declared width comes out **zero**.
> `grid-minimum-size-grid-items-021` scored **exactly half** its subtests — every declared WIDTH
> passed, every ratio-derived HEIGHT failed.

**Half a file, split cleanly by axis, is a missing intrinsic size seen from the outside.** That
signature is worth keeping: it says "the element has no natural dimensions yet", not "the layout is
wrong".

The budget is unchanged and is what keeps the fix safe: the enhancement phase already runs under
`load_budget()`, so a page with a dead image still fires `load` on schedule with whatever arrived —
the same promise `finish_loading` already made two hundred lines below, in the same words.

## The repair the reordering demanded

Firing `load` late made `cssom-view/elementsFromPoint.html` fail a row it had been passing — **for the
wrong reason.** The test samples the centre of a squiggle whose `<path>` has `fill="none"`, and the
path had simply had no layout box yet at the old firing time. With the box present, our hit test
returned it: four elements where Chrome returns three.

`pointer-events` defaults to `visiblePainted`, and for an SVG shape "painted" means the **fill** region
when `fill` is not `none` and the **stroke** region when `stroke` is not `none` — never the bounding
box. A `fill:none` shape is a curve with a hole the size of its own bbox.

> ⚠⚠ **BOUND, STATED: this declines the bbox hit, it does not implement the stroke.** A point that
> really is ON the stroke of a `fill:none` path IS hit in Chrome and is not hit here. That needs path
> geometry this seam does not have; the gap is one-directional — we under-hit, never over-hit.

## Gate

`engine/page/tests/g_load_waits_for_subresources.rs` — four rows, driven through `Page::load_async`.
Red under T1 (fire `load` early → the filled `<rect>` row loses its box), T2 (disable the `fill:none`
rule → the path reappears) and T3 (drop the shape-tag list → the `<svg fill="none">` container
disappears from its own hit list). **Each mutation moves a different row.**

⚠ A `data:` URL image decodes during parse, so `naturalWidth` cannot discriminate the ordering in a
fixture; the filled-rect row is what sees it, and the WPT file is what measures the image half.
