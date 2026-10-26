# `el.style.x = v` needs a CSS parser, not the Stylo cascade

> Landed R2.1.2.

## Mechanism (engine/page/src/lib.rs `install_supports_hook`, engine/page/Cargo.toml `cssom`)

- `el.style`'s setter validates each write with `__cssSupports('(k:v)')` (tick 1181). Without `stylo`,
  `install_supports_hook` wired `|_| false`, so every `el.style.display = 'block'` / `setProperty` / `style.color =`
  was rejected as invalid and silently dropped. `style.cssText =` and `setAttribute('style')` bypass validation, so they still worked.
- `manuk-agent` (so `manuk-mcp`, `a11y-dump`, `agent-run`) had refused the `stylo` feature in 104b2cf4: the Stylo
  cascade hides a collapsed `<select>`'s options (`g_ax_tree_excludes_display_none`).
- New `manuk-page` feature `cssom = ["manuk-css/stylo"]` links Stylo's parser for the four CSSOM hooks
  (supports, serialize, expand, supported-props) and leaves the cascade on MinimalCascade. The agent takes `cssom`.
- Gate: `agent/tests/g_agent_cssom_style_write.rs`; mutant = drop `"cssom"` from agent/Cargo.toml (panel stays shut, `computed:none`).

## Measured (chrome_ver=145.0.7632.116)

- Release `manuk-mcp` 48.63 -> 50.26 MB, `a11y-dump` 48.51 -> 50.15 MB (+1.6 MB). Full `stylo` was 51.8 MB.
- a11y node counts move on pages whose scripts hide loaders/placeholders via `el.style` (Wikipedia -13, quotes.toscrape.com/scroll
  -2 "Loading..."). `a11y-score` vs Chrome on those 6 URLs: matches unchanged, pooled F1 85.2 -> 85.3%, quotes/scroll 95.2 -> 100%.
  So the 0.05% drift gate (`perf1-nodes-gate.py`) goes red on 6/20 URLs while the Chrome score does not drop.
