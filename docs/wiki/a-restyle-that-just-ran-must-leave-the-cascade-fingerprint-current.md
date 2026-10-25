# A restyle that just ran must leave the cascade fingerprint current

> Landed PERF.1.2 (partial: see docs/sprint/PERF.md "PERF.1.2 results"). Gate script: `scripts/perf1-settle-gate.py`.

## Mechanism (engine/page/src/lib.rs)

- `apply_stylesheets` skips a full cascade+layout when `last_cascade` equals the fingerprint of (style sources, node count,
  viewport, dirty set). It stored the fingerprint **including the dirty set**, then cleared the dirt, so the next call hashed
  differently and re-ran a full cascade+layout over identical inputs. It now stores `clean_fp` (same inputs, no dirt).
  `cascade_fingerprint` / `cascade_sources` are the one implementation both sides use.
- `run_deferred_scripts` does its own `restyle_and_layout` and left the dirt set and the fingerprint stale, so the next
  phase (`subframes (pre-load)` -> `fetch_and_apply_stylesheets`) laid the same tree out again. It now clears the dirt, recomputes
  `has_sticky` and records the fingerprint, like every other relayout site.
- `fetch_and_apply_background_images` fetched and decoded once per NODE; it is now once per distinct url.

## What is NOT fixed (the remaining settle)

Settle that is left is JS evaluation of dynamic scripts (ordertime 1.6 s in one bundle, Wikipedia 0.5 s), forced reflows that
are published to the script but discarded by the page (the DOMContentLoaded reflow is redone by the next phase), and
one real cascade+layout per phase where scripts dirtied the tree.

## Gate hygiene

`scripts/perf1-settle-gate.py --nodes BASE NEW` compares a11y node counts per URL; a binary whose `finish_loading_inner`
returns immediately drifts 1.4-78% on 6 of 20 URLs (RED), the real change 0.04% max.
