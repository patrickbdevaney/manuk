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

## Adding a test

Keep probes **explicitly sized** where you can, so the comparison isolates layout from font
metrics. Give each interesting element `id="p-<name>"` and a `background` so it shows up in the
screenshot. One primitive per file (block flow, flex, positioning, box model, …).
