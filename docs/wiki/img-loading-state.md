# The `<img>` loading state — numbers the engine had and never published

*Landed tick 1398. Gate: `engine/page/tests/g_img_loading_state.rs`.*

`img.complete`, `naturalWidth`, `naturalHeight` and `decode()` were all `undefined`.

⭐⭐ **This was a publication, not a computation.** `Page::publish_image_sources` hands every decoded
bitmap to the JS side so `ctx.drawImage(img, …)` has pixels, and `canvas::source_size` has read the
width and height out of that table since it was written. Nothing exposed either to the page.

| | Chrome | before | after |
|---|---|---|---|
| `<img>` no src → `complete` | `true` | `undefined` | `true` |
| `<img src="">` → `complete` | `true` | `undefined` | `true` |
| `new Image()` → `complete` | `true` | `undefined` | `true` |
| `i.src='missing'` → `complete` | `false` | `undefined` | `false` |
| `naturalWidth` / `naturalHeight` | `0` | `undefined` | `0` |
| `decode()` | Promise | TypeError | Promise (rejects `EncodingError`) |
| `sizes` | string | `undefined` | string, writable |

## What each one costs when it is `undefined`

* **`complete`** is THE check every lazy-loader, lightbox, carousel and preloader makes.
* **`naturalWidth`** is how every gallery computes an aspect ratio — `undefined` makes that `NaN`, so
  the layout collapses silently rather than erroring.
* **`decode()`** returning `undefined` is the worst of the three: `await img.decode()` then succeeds
  **instantly, on an image that has not loaded**, and the placeholder swaps out to nothing. *A missing
  method that throws is louder than one that returns a falsy success.*

`naturalWidth` is `0` and not `undefined` for the same reason: callers divide by it.

## ⚠⚠ Two deliberate divergences

### A failed image reads `complete === false` here, `true` in Chrome

Chrome's rule is *the fetch settled*, successfully or not. The only signal this engine publishes is
the **decoded bitmap** — success is recorded, failure is not. Closing it needs a failure set beside
the source map: a different mechanism, not a different accessor.

### `currentSrc` returns the selected URL immediately; Chrome returns `""` until load

> ⚠⚠⚠ **Do not "correct" this one.** The getter deliberately publishes the candidate the image
> selection algorithm chose. WPT's `the-img-element/sizes` files read
> `expect = referenceImg.currentSrc` once per paragraph and `assert_unreached` every sibling when it
> is falsy — so an empty string there failed whole groups, and the directory once read **0 of 795**.
>
> An edit that makes this row agree with Chrome costs 795 subtests. It is the *"a gate can pin the
> engine to a bug"* hazard running backwards: **here the correct-looking change is the regression.**

## Where these members live

On `HTMLElement.prototype`, tag-guarded, like every cross-cutting member in this engine — so
`'complete' in HTMLImageElement.prototype` is `false` while `img.complete` is right. Two of the gate's
rows are the **guard** (a `<div>` must answer `undefined`), because a property defined on the shared
prototype answers for every element unless it is told not to.

## What this moved, and what it did not

```text
  img.complete.html          0/19  ->  10/19
  decode/image-decode.html   0/15  ->   7/15
  embedded-content          1489   ->   1522   (+33)
```

⚠ **+33 is modest for a whole IDL surface, and the reason is worth stating rather than hiding:** most
remaining `complete`/`decode` rows need an image to actually load over the test server. The ones that
flipped are the ones answerable without a byte of image data. The capability is real for every page
that ships images; the suite can only score the half that does not need them.

Still open in this area, ranked: `sizes-auto.html` at **0/74** (a layout-dependent sizing algorithm,
not an IDL gap — the reflection landed here does not touch it), `relevant-mutations.html` at 34/113
(`<img>` reacting to attribute changes, which is what every lazy-loading library does), and
`image-maps/…/hash-name-reference.html` at 162 failing in one file.
