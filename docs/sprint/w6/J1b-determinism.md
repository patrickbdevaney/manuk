# W6.J1.b.a.a.a.a.d.c.a.a — is the css-transitions subtest denominator deterministic?

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Measurement only; no engine change in this node.

## Answer

**No, not at host load ~30, and not with the reflow budget raised.** Under the stated alternative
(`MANUK_REFLOW_BUDGET_MS=60000`), three runs of each binary give different FILES totals. The variable files are
named below, so the gate's "or the per-file variable totals are named" branch is met.

The question as written ("at low host load") is **not answered**: host 1-min load stayed between 28.7 and 41.1 on
24 cores for the whole node (`raw/J1b-determinism/load-*.txt`), and recent history shows it never below ~19. No run
was made at load < 4.

## Binaries

- **before** = `c4e30576` engine + HEAD `tests/wpt/src/main.rs` (printer: `MANUK_WPT_TOTALS=1`).
- **after** = `0df91d1a` engine + the same printer.

Both were built with `cargo build --release -p manuk-wpt` by checking out the engine paths from each commit
in the main tree, building, copying the binary to `/tmp/d-wpt/{before,after}-printer-manuk-wpt`, and restoring
`engine/` to HEAD. The `target/release/manuk-wpt` binary was rebuilt at HEAD afterwards. The binaries are not
committed; the logs are.

Run: `MANUK_WPT_TOTALS=1 MANUK_REFLOW_BUDGET_MS=60000 <bin> wpt css/css-transitions --timeout 120 --show-failures`,
three times per binary, run sequentially.

## Totals

| run | before (c4e30576) | after (0df91d1a) |
|---|---|---|
| 1 | 638/2664 (23.9%) | 688/1544 (44.6%) |
| 2 | 638/2104 (30.3%) | 747/2104 (35.5%) |
| 3 | 638/2104 (30.3%) | 706/2104 (33.6%) |

The `before` denominator takes two values (2664, 2104), and the `after` denominator takes two values (1544, 2104).
Numerators also move, on the `after` side.

## Variable files (`python3 -I scripts/wpt-file-variance.py`, exit 1)

before, one file varies:

- `properties-value-inherit-001.html`: run 1 `0/560`; runs 2 and 3 print no header (no subtests reported). This
  is the whole `before` 2664-vs-2104 gap.

after, five files vary:

- `properties-value-001.html`: run 3 `0/560`; runs 1 and 2 print no header.
- `properties-value-inherit-002.html`: run 2 `40/560`; runs 1 and 3 print no header.
- `properties-value-003.html`: `12/122`, `13/122`, `12/122`.
- `properties-value-implicit-001.html`: `7/60`, `26/60`, `26/60`.
- `animations/z-index-interpolation.html`: `250/250`, `249/250`, `249/250`.

The two 560-subtest files account for the large swings: a file reports 560 subtests in one run and no header in
another. That is the file-level variance. The small cases (`properties-value-003`, `-implicit-001`,
`z-index-interpolation`) are numerator noise on files whose denominator is stable.

## Consequences

- The earlier claim that the +1620 subtest denominator split is a property of the two commits is **not supported**.
  The 560-subtest files are non-deterministic under this host's load, so the `before` and `after` totals cannot be
  compared as fixed numbers. This matches `J1b-after-fix.md` section 4e.
- The `.c.a` gate (`attributed = 1620`) cannot be met by re-running on this host. No follow-up row is opened:
  the product sprint does not need the answer (per the FOLLOW-UP DISCIPLINE note). The open question is recorded
  here and in the JOURNAL line.
- Any future css-transitions total must be a band over runs, not a single number.

## Reproduce

```
python3 -I scripts/wpt-file-variance.py docs/sprint/w6/raw/J1b-determinism/before.{1,2,3}.log
python3 -I scripts/wpt-file-variance.py docs/sprint/w6/raw/J1b-determinism/after.{1,2,3}.log
python3 -I scripts/wpt-file-variance.py --selftest
```

Exit 1 on both real runs is the red state (variance found). The selftest checks that a varying file is named and
that two identical runs are clean.
