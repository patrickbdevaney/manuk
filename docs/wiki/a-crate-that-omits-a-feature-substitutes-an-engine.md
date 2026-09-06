# A crate that omits a feature substitutes an engine

`manuk-agent` depends on `manuk-page` **without the `stylo` feature**. That does not mean the agent
crate has no cascade — it means it has a *different* one. Every `agent/tests/` fixture carrying a
`<style>` block is cascaded by `MinimalCascade`, while WPT and every `engine/page/tests/` gate that
asks for `--features stylo` run the real Stylo engine.

So a property implemented in one and missing in the other is **correct everywhere the suites look
and wrong everywhere the agent looks.**

## The instance

`inset` was exactly that. The four longhands were implemented; the shorthand was not:

```text
                                            Chrome    Minimal    Stylo
  .sh  { inset: 0 }              stylesheet  200x30       0x0    200x30
  .sh2 { inset: 5px 10px }       stylesheet  180x20       0x0    180x20
  style="inset: 0"               inline      200x30       0x0    200x30
  .lh  { top/right/bottom/left } CONTROL     200x30    200x30    200x30
```

⭐ **The control row is what names the bug.** The longhands were already right, from the same
stylesheet, on the same element — so this was never "absolute positioning is broken" or "the parser
drops the rule". Exactly one arm was missing from one `match`.

## How it presented, and how much it cost

Not as a CSS failure. It surfaced as an **inexplicable `0x0` box in an accessibility tree** during a
Track C drive probe, was mis-attributed to layout for a whole tick ("`inset: 0` does not size an
absolutely positioned box" — false; layout was never involved), and was only resolved by running one
fixture under both cascades and watching them disagree.

The general move: **when a fixture in one crate contradicts a fixture in another, suspect the
feature set before the code.** Two crates in one workspace can be running two implementations of the
same rule, and cargo's feature unification means the answer can also depend on *how you invoked the
test* — `cargo test -p manuk-agent` and a whole-workspace run need not agree.

## The fix for a mis-measurement is a control, not an apology

The drive-probe corpus numbers had been taken under the same MinimalCascade. Re-running all six
sites under both:

```text
                 rate   +landmark
  Stylo         77.7%       81.1%
  MinimalCascade 77.7%      81.0%
```

The site-level metric is **robust to the cascade** even though individual fixtures are not. The
number stands — and now it stands on evidence rather than on not having checked.

⚠ Until it is decided whether the agent crate should build with `stylo`, **run every new agent gate
under both**:

```sh
cargo test -p manuk-agent --test <gate>
cargo test -p manuk-agent -p manuk-page --features manuk-page/stylo --test <gate>
```

See also [[role-plus-name-is-not-an-address]], [[two-entrances-and-the-survey-method]].
