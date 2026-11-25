# CPU.1.b — navigate cost of a 1500-row table (chrome_ver=145.0.7632.116) — BLOCKED, no engine change landed

Method: `../cpu1/bench.py` (fresh `manuk-mcp --deterministic --backend engine`, cpu-s over the process tree), fixtures
`../cpu1a/fx/t{1500,3000,6000}.html`, host load1 ~19-22 on 24 cores throughout. `before.txt` = unmodified binary, 3 runs;
`after.txt` = binary with the attempted change below; `nav1500-interleaved.txt` = 8 interleaved before/after runs.

## Findings
* Navigate cpu-s (3 runs, before): t1500 0.67-0.75, t3000 1.54-1.72, t6000 3.29-3.43. **6000/3000 = 2.0-2.1x: the registered
  ratio bar (<= 2.3x) was already met by the baseline** (CPU.1's 2.5/6.9 predate CPU.1.a's fix). The t1500 bar (<= 0.5) is NOT met.
* `MANUK_NET_TRACE` cascade_pass/layout_pass lines (t1500): THREE layout passes of ~80-110 ms each (page/lib.rs:9963 initial,
  :10155 after scripts, :13005 `apply_stylesheets` in the load tail). Layout is linear in rows (110 -> 424 ms for 1500 -> 6000);
  first cascade 17 -> 104 ms (5.9x for 4x, mildly super-linear). No quadratic stage remains in navigate.
* Attempt (reverted): the :13005 pass re-cascades identical inputs because `last_cascade` is `None` after `from_dom`. Calling
  `note_cascade_current` at the end of `from_dom` when every cascade source is an unconditional inline `<style>` (no @container/@import)
  removed the third pass (trace: 2 layout passes instead of 3) but NO cpu change was measurable: interleaved nav median
  0.82 before / 0.90 after (min 0.67/0.68); 12 runs of a11y-dump 0.49-0.73 vs 0.58-0.72 s. Host noise (+-0.1 s) exceeds the
  ~0.1 s expected gain. A change without a measured benefit is not landed.
* Not done: byte-identity on 20 pages, mutant (nothing to land).

Follow-up CPU.1.b.a: re-measure on a quiet host and re-apply that small change if it saves >= 0.08 cpu-s.
