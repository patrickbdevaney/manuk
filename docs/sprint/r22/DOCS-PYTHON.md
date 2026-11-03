# R2.2.a — attribution of the taffy 0.15 SHAPE delta
chrome_ver=145.0.7632.116. Binaries: base = HEAD tree, new = HEAD + docs/sprint/r22/taffy-0.15.patch; both carry the `<site>.manuk-paths.tsv` dump added by this node (our path+rect set next to the oracle's).

## docs.python.org — the drop did NOT reproduce
- Isolated `manuk-wpt fidelity` 5x per side: SHAPE 72.4 (261 scored) on all 10 runs; the 5 base/new `manuk-paths.tsv` pairs are byte-identical (0 differing lines), as are the oracle files and the fetched HTML.
- Inside `tri-sweep.sh --limit 10 --no-a11y --no-m2 --m1-timeout 120`, 1 run per side: docs.python 72.4 / 72.4.
- So no docs.python path differs between 0.12 and 0.15. The earlier 70.9 (R2.2 new-2/new-3, vis 92.8 vs 86.1) is not an engine effect we can reproduce; it is oracle/fetch-state variance (the oracle file was not kept in those runs, so it cannot be proven further).

## apple.com — the reproducible 0.15 delta (SHAPE 56.4 -> 55.9, matches R2.2's apple -0.4..-0.5)
Same fetched HTML (cmp identical) in both sweeps. 61 of our path rects differ (`APPLE-manuk-paths.diff`), all in `main > section:nth-of-type(2) > div:nth-of-type(N) > div:nth-of-type(2)` (the `.tile-image-wrapper`) and its `picture`/`img`/`video` descendants, N = 1..5 (tile-wrapper promo tiles):
- `body:nth-of-type(1)/div:nth-of-type(1)/main:nth-of-type(1)/section:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(2)` x: 12 -> -285 (w 582, h 580 unchanged); the sibling `div:nth-of-type(2)` of tiles 2,4 goes 606 -> 903; img inside 266 -> 563 / -328 -> -625.
- Chrome puts the tile-image-wrapper at x=12 (oracle section:nth-of-type(2)/div:nth-of-type(1)/div:nth-of-type(2) = 12 2156 582 580); base matches it, 0.15 moves it by -297 (+297 on alternating tiles).
- First sibling `div:nth-of-type(1)` (tile-content, 199 2285 207 467) is identical in both. The offset is a main-axis placement of the second flex/grid item when content overflows the 582px container.
Cause class: taffy 0.15 changed placement of an overflowing second item (negative free space / alignment); exact CSS rule not yet identified (stylesheet is external) -> follow-up R2.2.a.a.
