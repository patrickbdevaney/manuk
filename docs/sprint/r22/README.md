# R2.2 — taffy 0.12 -> 0.15 (not landed)
chrome_ver=145.0.7632.116. `taffy-0.15.patch` ports engine/layout/src/taffy_tree.rs (+Cargo.toml/lock): `cargo test -p manuk-layout` 191/191 green.
M1 SHAPE (tri-sweep --limit 10 --no-a11y --no-m2 --m1-timeout 120, 6 reachable sites), 3 runs each, base = binary from HEAD tree, new = patched:
base 68.5 / 69.4 / 69.8   new 69.9 / 67.4 / 69.5   (new min 67.4 < base min 68.5)
per site: docs.python 72.4x3 -> 70.9 in 2 of 3 new runs; apple 56.4-56.6 -> 55.9-56.1; usa.gov 82.1-84.6 -> 84.3 / 70.2 / 84.3; others equal.
Raw rows: base-N.tsv / new-N.tsv. The patch is only the baseline for follow-ups; the wall (tick.sh) was not run on it.
