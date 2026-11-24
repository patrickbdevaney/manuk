# Incremental restyle: reuse a node's style when its fingerprint did not move

> Landed PERF.1.2.b (cascade phase down on script-heavy pages; settle bar NOT met: see docs/sprint/PERF.md "PERF.1.2.b").
> Gates: `incr::tests::incremental_minimal_cascade_equals_full_after_mutations`,
> `stylo_engine::incr_tests::incremental_restyle_equals_full_cascade_after_mutations`, `MANUK_INCR_VERIFY=1`.

## Mechanism (engine/css/src/incr.rs, lib.rs `cascade_incremental`, stylo_engine.rs `IncrCache`)

- A node's walk result is a function of its own element data, its ancestors', its siblings' (only for sheets with sibling combinators or
  structural pseudo-classes), the sheets and the viewport. So the previous pass's per-node results are kept (thread-local, keyed by a hash
  of viewport + quirks + every sheet source) and only the subtrees of nodes whose **fingerprint** moved are re-walked.
- The fingerprint is computed from the `Dom` (tag, attrs, hover/focus/active, parent id; neighbour ids / has-children only when the sheet flags
  ask), NOT from the dirty bits: the bits are cleared by other consumers and a mutator that forgot `mark_dirty` would be an invisible stale style.
- A re-walked subtree inherits from its parent's *cached* entry; removed nodes are dropped from the cache; the Stylo path also keeps each
  element's `ComputedValues` so a re-walked root inherits from the real parent value.
- Whole-map passes (`:has`, MinimalCascade property recovery, effective opacity, legacy block align, shadow fill) are not incremental: they
  run on a clone of the post-walk map through the one `finish_passes` function the full cascade also calls, so the two cannot drift.
- Falls back to the full cascade: sheet list / viewport changed, `<html>` moved (root font size feeds `rem`), shadow DOM, > 1/3 nodes moved,
  animation or transition in flight (the Stylo path aborts and redoes the pass in full), container-query re-pass.

## Gotchas

- Descendant/child combinators and `:lang` depend on ancestors without the ancestor's computed style changing, so a changed node re-walks its
  WHOLE subtree; "recompute only if the parent's computed style changed" would be wrong.
- The profiler counts cascade time as `cascade`, not `settle`: a faster cascade does not move the settle bar.
- Cost of an incremental pass is dominated by the 32 ms cache clone on a 19k-node page; most remaining full passes are sheet-list changes.

## PERF.1.2.b.a: what a pass can now reuse across (chrome_ver=145.0.7632.116)

- **Sheet list moves.** The cache holds the sheets (not just hashes). A pass reuses it when the list differs only by sheets that appeared or
  vanished (LCS on source hash, `sheet_diff`); the elements those sheets' rules can match (`appended_matches`, our selector matcher over the
  moved sheets only) are re-walked, nothing else. A moved sheet with `@import/@layer/@scope/@container/@property/...` is never layered
  (full pass); `@keyframes` and `@counter-style` are safe (a pass with an animating element is never cached; `@counter-style` defines a name).
  Sibling flags may not GROW across a move (the fingerprint's shape would change).
- **Two cache slots** (`INCR_SLOTS`): the page's own callers disagree on the sheet list (some rebuild it from inline `<style>` only), so passes
  alternate between two lists. `incr_pick` takes the slot with the smallest sheet diff; a slot a pass did not consume is kept.
- **Sibling expansion by side.** Sheet flags are bits: `+ ~ :nth-child :nth-of-type :first-of-type` read the siblings BEFORE the subject (the
  LATER ones move), `:nth-last-* :last-of-type` the ones after (EARLIER), `:only-of-type` both, `:first/:last/:only-child` need only the
  neighbours (already moved through their prev/next ids in the fingerprint). Wikipedia's `:nth-*` used to expand every sibling of a changed node.
- **`<html>` class flips** (`client-nojs` -> `client-js`, feature flags) no longer force a full pass. When only the class list changed, the
  elements matched by rules that look at a changed class (`matches_mentioning`, evaluated with `<html>`'s class as it was AND as it is, via
  `CLASS_OVERRIDE`: a rule that stopped matching must restyle too) are re-walked, and `<html>` itself is restyled alone
  (`style_of_root_alone`): if its style is unchanged it counts as clean. A parsed-vs-textual `.token` count bails if the parser dropped a rule
  naming a changed class. Pointer/focus state on `<html>` is part of the snapshot, so a state change is still a full pass.
- Gotcha: matching "what changed" only in the NEW dom misses the nodes whose rule stopped matching (the old class is gone); that was the first
  INCR MISMATCH this found (Alan_Turing, `li#ca-bookmark`).
- The html-alone shortcut is in the MinimalCascade path only; the Stylo path still does a full pass when a rule that names a flipped class
  matches `<html>`.
