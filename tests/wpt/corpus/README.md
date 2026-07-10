# Layout-parity corpus

Each `.html` here is a probe page for the **layout-parity harness** (`manuk-wpt parity`).
Any element with an `id` beginning `p-` is a **probe**: the harness compares that element's
border-box geometry (`x, y, width, height`) between Manuk and headless Chrome, and reports how
many probes agree within a pixel tolerance. Comparing *boxes* rather than pixels measures
layout correctness without cross-engine font-rasterization noise.

## Run it

```bash
# vs headless Chrome (auto-detected: google-chrome or chromium)
cargo run -p manuk-wpt -- parity --out /tmp/parity

#   --corpus DIR   test pages (default: this directory)
#   --out DIR      write <page>.manuk.png + <page>.chrome.png side-by-side for eyeballing
#   --tol PX       per-axis tolerance (default 3)
#   --width/--height  viewport (default 800×600)
```

Exit code is non-zero if any page with a reference has an out-of-tolerance probe, so it works
as a CI gate. Without Chrome installed it still writes the Manuk renders and reports
"no reference".

## Coverage

The corpus holds Manuk to Chrome parity (±3px box geometry) across the layout
primitives real pages use: block flow, the box model (`box-sizing`, borders,
margins incl. negative + auto-centering), **flexbox** (direction, wrap, gap,
grow/shrink/basis, justify-content, align-items/self, nesting), **CSS grid**
(`grid-template-columns/rows` with px/fr/%/auto + `repeat()`, gap), sizing
(`min/max-width/height`, percentage width/height, additive `calc()`),
positioning (relative, absolute incl. left+right / top+bottom stretch),
inline-block, inline padding/border, `white-space:nowrap`, and
`transform:translate`.

Known font-metric limit: exact text *width/height* vs Chrome depends on the font
stack (Manuk uses system fonts, Chrome its own default), so text-sized probes
carry a few px of delta. Probes therefore prefer explicit-sized boxes or measure a
layout *consequence* (e.g. a following block's position) rather than raw text width.

## Adding a test

Keep probes **explicitly sized** where you can, so the comparison isolates layout from font
metrics. Give each interesting element `id="p-<name>"` and a `background` so it shows up in the
screenshot. One primitive per file (block flow, flex, positioning, box model, …).
