# manuk-wpt — conformance harness

CLAUDE.md makes **Web Platform Tests the ground-truth conformance signal** — not
Chromium-specific quirks, not a hand-rolled corpus. This crate is where that signal
lives and grows.

## Today

`cargo run -p manuk-wpt` runs a small built-in **layout reftest suite** expressed
directly against the engine (`run_layout_suite`). These stand in for WPT
`css/CSS2/normal-flow` reftests we can already pass, using the same methodology:
assert concrete layout facts, track pass/fail/skip, emit a report (`Report`, with
`summary()` and `to_json()` so CI can diff results over time).

```
  PASS  normal-flow/auto-width-fills-containing-block
  PASS  normal-flow/blocks-stack-vertically
  PASS  display/none-generates-no-box
  PASS  inline/text-wraps
  PASS  normal-flow/auto-margins-center
```

## Growth path: the upstream WPT runner

`find_wpt_checkout()` locates a real WPT tree via `$WPT_DIR`:

```bash
git clone https://github.com/web-platform-tests/wpt
export WPT_DIR=$PWD/wpt
cargo run -p manuk-wpt
```

The upstream runner (the larger follow-on) will iterate the relevant `css/`, `dom/`,
and layout reftest subsets, driving the engine and comparing against expected
renders / `testharness.js` assertions, folding results into the same `Report`.

## Working agreement (from CLAUDE.md)

Any change to `engine/layout`, `engine/css`, or `engine/js` should cite which WPT
(or, for now, built-in reftest) subset was run. When a feature is ambiguous even in
WPT (floats, table layout), document the chosen interpretation rather than silently
picking one.
