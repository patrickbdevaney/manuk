# Fidelity Scoring Redesign — measuring "not jarring", not pixel-identical

**Written tick 267, from the first real 33-site stratified sweep.**
Purpose: certify Phase 0 honestly, and stop mistaking one root cause for a long tail.

---

## 0. The finding that motivates this

The sweep reported `cov(ok) 85.9%` but `PLACE(ok) 4.5%` against a ≥75% bar — which reads as
"hundreds of layout bugs, diminishing returns, a Presto-style tail." **The data says otherwise:**

| site | median dy | placement |
|---|---|---|
| microsoft.com | **23px** | **0.0%** |
| doc.rust-lang.org | **28px** | **0.0%** |
| old.reddit.com | 12px | 17.6% |
| airbnb.com | 20px | 6.2% |
| youtube.com | **0px** | 14.3% |

A **23px median with zero elements in tolerance** means the spread is tight *around* 23px: nearly
every element is off by the *same* amount. That is **one constant offset**, not many bugs. And every
site names a `FIRST DIVERGENCE` element after which the offset is inherited — the cumulative-drift
signature.

**The metric charges one root cause N times, once per downstream element.** A page shifted 23px is
not jarring to a user; it scores 0%. So `PLACE(ok) 4.5%` measures *absolute document position*, and
we need *user-perceived correctness*.

**This is the single most important correction available right now:** we have been counting SITES,
not CAUSES, and concluding "long tail" from a number that cannot distinguish them.

---

## 1. What "not jarring" actually means

Phase 0's bar is **"a user does not notice they left Chromium."** Decomposed into checkable
invariants, none of which require pixel parity:

| invariant | why it matters | how it fails today |
|---|---|---|
| **No overlap** | text on text, control under banner — the #1 "broken page" perception | occlusion between boxes that Chrome keeps disjoint |
| **No clipping / h-overflow** | content off-screen or cut is unusable | our width differs → content escapes the viewport |
| **Reading order preserved** | screen order matches DOM order | a float/grid/abspos escapes its slot |
| **Interactive targets hittable** | a button you cannot click is a dead page | occlusion-aware hit-test already exists — reuse it |
| **Stable after load** | content jumping post-load is the most jarring thing a browser does | no CLS-equivalent measured today |
| **Relative geometry** | a card 40px too tall is a bug; the whole page 23px lower is not | absolute-position scoring conflates them |

**None of these are "within 8px of Chrome's absolute Y."** That is the mismatch.

---

## 2. The redesign — three layers, cheap to expensive

### Layer 1 — SHAPE (parent-relative, the new primary gate)
Score every element against **its parent's box**, not the document origin:
`rel = (child.x - parent.x, child.y - parent.y, child.w, child.h)`.

- A constant page offset **cancels out** — microsoft's 23px shift becomes 0 error.
- A genuinely wrong box (card too tall, sidebar too wide) still fails.
- **One root cause counts once**, at the element where it originates, instead of N times downstream.

This alone is expected to move the headline from ~4.5% to something that reflects reality. It is
also the number worth gating on.

### Layer 2 — JARRING INVARIANTS (the actual Phase-0 bar)
Boolean, per page, from the same box dump: overlap count, horizontal overflow, reading-order
inversions, unhittable interactive elements, post-load shift. **These are what certify Phase 0.**
A page can be 30px offset everywhere and still pass all five — correctly.

### Layer 3 — PIXEL (diagnostic, not a gate)
Keep the current visual + absolute-placement scoring on a small corpus. It is useful for eyeballing
and for catching paint bugs shape-scoring cannot see. **Never gate on it** — it is the layer that
produced the misleading 4.5%.

---

## 3. Enabling fixes (both are gating on the gate)

**a) Selector-path keying — replaces `[id]`.**
39% of sites are unmeasurable today (9 LOW_SAMPLE + 4 NO_IDS of 33) because the probe anchors on
`id` attributes and modern React/Tailwind pages barely use them. Key on a stable path instead:
`tag[.class-signature]:nth-child(n)` from the root. Measurability goes ~61% → ~100%, and *only then*
is any Phase-0 certification honest.

**b) Root-cause clustering — the answer to "is this a long tail?"**
Group failures by their `FIRST DIVERGENCE` signature (the element where drift begins) and by the
offset value. Report **distinct causes**, not failing sites. If 40 sites fail from one 23px header
delta, the board must say **1 bug**, not 40. Until this exists we cannot tell saturation from
amplification — and we have already been fooled once.

---

## 4. Scale — testing "as much of the internet as possible"

Today: 265-site corpus, ~30s/site, screenshot + image compare. That is the bottleneck.

**Drop the screenshot for scoring.** Chrome's box geometry is obtainable directly (CDP
`DOM.getBoxModel`, or one injected script returning `getBoundingClientRect` for every element).
No PNG encode, no image diff, no pixel compare — an order of magnitude faster, and it is exactly the
data Layers 1 and 2 need. Screenshots stay only for Layer 3's small diagnostic corpus.

That makes **1000+ sites per sweep** practical, which is what "most of the internet" requires:
Tranco top-1000 stratified by category, weighted by user-minutes.

---

## 5. The Phase-0 exit certificate

Phase 0 is met when, on a category-stratified Tranco top-1000:

1. **Bar 0**: zero crashes/hangs. Non-negotiable.
2. **Jarring invariants** (Layer 2): ≥95% of sites clean on overlap, clipping, reading order,
   hit-testability, post-load stability.
3. **Shape fidelity** (Layer 1): ≥0.75 parent-relative on ≥95% of sites, ≥0.70 per category.
4. **Interactivity**: click / type / scroll / navigate succeed on ≥95%.
5. **Named exceptions only**: the honest out-of-reach set (heavy-WebGL creative apps, DRM,
   Chromium-whitelisted sites) — enumerated, not hand-waved.

Explicitly **not** required: pixel parity, absolute-position parity, WPT percentage.

---

## 6. Why this unblocks Phase 1

The worry — "we are hitting a long tail of CSS placement ticks" — is well founded *as a risk* and is
what killed Presto and EdgeHTML. But we have not yet measured whether we are actually in it, because
the instrument amplifies single causes into apparent tails. **Fix the instrument first; it is cheaper
than the tail it is currently inventing.**

If shape-scoring plus root-cause clustering shows a short list of systematic offsets, Phase 0 closes
in a handful of ticks and Phase 1 (agentic API surface, a11y tree, harness, MCP, consumer
prompt-to-action) starts on schedule. The agentic substrate is already strong — Phase 1 is largely
*exposing* what exists, not building it. If instead it shows genuinely independent per-site breakage,
we will know that too, honestly, and can price the tail before paying for it.
