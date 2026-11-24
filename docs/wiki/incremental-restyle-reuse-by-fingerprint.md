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
