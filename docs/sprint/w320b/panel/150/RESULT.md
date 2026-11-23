# W3.20.b.c.a: WebMCP.toolsAdded capture on Chrome for Testing 150

chrome_ver=150.0.7871.124 (own measurement; never diffed against 145.0.7632.116 or 149.0.7827.155).
Binary: ~/.local/chrome-cft/150.0.7871.124/chrome-linux64/chrome (downloaded from the Chrome for Testing bucket, `--version` = 150.0.7871.124).

## Result
With `WebMCP.enable` issued before `Page.navigate`, the CDP domain delivers `WebMCP.toolsAdded` events on
`googlechromelabs.github.io/webmcp-tools/demos/pizza-maker/`:

- Seven distinct tools, each in its own event (16 events in total, including repeats from the probe's
  disable/enable and the self-test):
  `set_pizza_size`, `set_pizza_style`, `toggle_layer`, `add_topping`, `remove_topping`, `manage_pizza`, `share_pizza`.
- Each event carries `name`, `description`, `inputSchema` and a `stackTrace` pointing at the demo's `script.js`.
- `document.modelContext.getTools()` returns the same seven names. `navigator.modelContext` is the same object.
- A tool registered live after enable (`zz`) also raises a `toolsAdded` event (probe self-test).

## Negative control (gate can go red)
`control-nomcp.mjs` (same probe, navigating to a `data:` page with no WebMCP registration): 0 `WebMCP.toolsAdded`
events. `check.py` exits 1 on it.

## Gate check
`python3 -I check.py probe-150.out` → toolsAdded_events=16, names match the expected seven, rc=0.
`python3 -I check.py control-nomcp-150.out` → 0 events, rc=1.

## Relation to 149
The 149 run (docs/sprint/w320b/panel/RESULT.md) got zero `toolsAdded` events under every flag combination. The
150 probe uses the same flags and the same script. The difference is the browser build, so 149's zero-event result
should be read as build-specific. This does not change the 149 row, which stays as recorded.

## Scope
One demo page (pizza-maker). W3.20.b.c needs ≥2 demo pages and a diff against manuk `tools/list`; those are
follow-up rows, not this node. No Rust was touched by this node.

Files: probe.mjs (149 probe with the binary path swapped to 150), probe-150.out (raw stdout), control-nomcp.mjs,
control-nomcp-150.out, check.py, RESULT.tsv.
