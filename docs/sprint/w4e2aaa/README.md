# W4.E2.a.a.a — lean vs lean_image_dims->None, tool-level diff (chrome_ver=145.0.7632.116)

Probe: `agent/examples/lean_bbox.rs` (a11y bboxes, observe() semantics lines with click points, content_height, hit-test landing at each link centre).
Pages: fixtures `/lp` (picture-only links) and `/lp2` (new: unsized imgs stacked above text rows). Files: `{good-full,good-lean,mutant-lean}-{lp,lp2}.txt`.
Mutant: `lean_image_dims` returns None (source reverted after build).

Findings (measured, see files):
- good full == good lean, byte-identical, on both pages (diff empty).
- mutant lean differs on both pages: content_height 473.9 -> 177.9 (/lp), 673.9 -> 303.9 (/lp2);
  link bbox heights 90 -> 16 (/lp); observe() `@(x,y)` click points shift (Open Amber @(640,159) -> @(640,122) on /lp; @(218,322) -> @(74,174) on /lp2).
- NOT different: landing by name/handle — a click at each link's own centre resolves to the same link, so a click-by-handle task cannot see the mutant.
- Agent-visible signal = the coordinates in observe() and content_height/scroll extent. A task must click at coordinates read from observe(), or depend on scroll extent.
