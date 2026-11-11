#!/usr/bin/env python3
"""W5.T4.a: cache-inclusive tokens per task-run of the diff-or-view build vs the W5.T1.b lean rows.

  python3 -I docs/sprint/w5/T4a-tokens.py BASE.tsv NEW.tsv

Rule (fixed before the runs): NEW passes iff its mean cache-inclusive tokens per task-run is not
above BASE's by more than the noise margin (3%, the lean-vs-legacy gap measured in W5.T1.b), and both
files carry one chrome_ver. Exit 0 on PASS, 1 on FAIL.
"""
import csv, sys

MARGIN = 0.03

def load(path):
    rows = [r for r in csv.DictReader(open(path), delimiter="\t") if r.get("task")]
    tok = sum(int(r["input_tokens"] or 0) + int(r["output_tokens"] or 0)
              + int(r["cache_read_tokens"] or 0) + int(r["cache_create_tokens"] or 0) for r in rows)
    return len(rows), tok / max(1, len(rows)), {r["chrome_ver"] for r in rows}

def main(base_path, new_path):
    bn, bmean, bchrome = load(base_path)
    nn, nmean, nchrome = load(new_path)
    same_chrome = len(bchrome | nchrome) == 1
    ok = same_chrome and nmean <= bmean * (1 + MARGIN)
    print(f"base\t{bn} rows\t{bmean:.0f} cache-incl tok/task-run\t{','.join(sorted(bchrome))}")
    print(f"new\t{nn} rows\t{nmean:.0f} cache-incl tok/task-run\t{','.join(sorted(nchrome))}")
    print(f"ratio new/base={nmean / bmean:.3f} margin={MARGIN} same_chrome={same_chrome}")
    print("VERDICT", "PASS" if ok else "FAIL")
    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
