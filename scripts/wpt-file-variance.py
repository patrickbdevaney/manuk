#!/usr/bin/env python3
"""Name the WPT files whose per-file (pass/total) header changes between repeated runs of one binary.

Input: repeated `manuk-wpt wpt <subset> --show-failures` logs run with MANUK_WPT_TOTALS=1, so every file
that reports subtests prints `  FILE <path>  (p/t)`. A file that prints no header in a run reported no subtests
in that run (the header is absent, not zero).

Usage:
  python3 -I scripts/wpt-file-variance.py RUN1.log RUN2.log [RUN3.log ...]
  python3 -I scripts/wpt-file-variance.py --selftest

Exit 0 when every file prints the same header (or none) in every run. Exit 1 and list each variable file
otherwise. Also prints the FILES summary line of each run so the totals can be read beside the variance.
"""
import os
import re
import sys
import tempfile

FILE_RE = re.compile(r'^  FILE (\S+)\s+\((\d+)/(\d+)\)\s*$')
SUMMARY_RE = re.compile(r'^\s*FILES\s+\d+\s+subtests \d+/\d+\s*=.*$')


def load(path):
    per_file, summary = {}, None
    with open(path, encoding='utf-8', errors='replace') as fh:
        for line in fh:
            line = line.rstrip('\n')
            m = FILE_RE.match(line)
            if m:
                per_file[m.group(1)] = (int(m.group(2)), int(m.group(3)))
            elif SUMMARY_RE.match(line):
                summary = line.strip()
    return per_file, summary


def compare(paths):
    runs = [load(p) for p in paths]
    files = sorted(set().union(*[set(pf) for pf, _ in runs]))
    variable = []
    for f in files:
        vals = [pf.get(f) for pf, _ in runs]
        if len(set(vals)) > 1:
            variable.append((f, vals))
    return runs, variable


def main(argv):
    if argv[:1] == ['--selftest']:
        return selftest()
    if len(argv) < 2:
        print(__doc__)
        return 2
    runs, variable = compare(argv)
    for p, (_, summary) in zip(argv, runs):
        print(f'{os.path.basename(p)}: {summary}')
    if not variable:
        print('no per-file variance across runs')
        return 0
    print(f'{len(variable)} file(s) with variable (p/t) across runs:')
    for f, vals in variable:
        shown = ' '.join('none' if v is None else f'{v[0]}/{v[1]}' for v in vals)
        print(f'  {f}  {shown}')
    return 1


def selftest():
    hdr = '  FILE {f}  ({p}/{t})\n'
    summ = '  FILES  2   subtests {p}/{t}  =  50.0%\n'
    a = hdr.format(f='css/a.html', p=1, t=2) + hdr.format(f='css/b.html', p=0, t=5) + summ.format(p=1, t=7)
    b = hdr.format(f='css/a.html', p=1, t=2) + summ.format(p=1, t=2)
    c = hdr.format(f='css/a.html', p=1, t=2) + hdr.format(f='css/b.html', p=0, t=5) + summ.format(p=1, t=7)
    paths = []
    try:
        for text in (a, b, c):
            fd, p = tempfile.mkstemp(suffix='.log')
            with os.fdopen(fd, 'w') as fh:
                fh.write(text)
            paths.append(p)
        _, variable = compare(paths)
        ok_red = [f for f, _ in variable] == ['css/b.html']
        _, same = compare([paths[0], paths[2]])
        ok_green = same == []
        print('selftest: variable file named' if ok_red else 'selftest FAIL: variable file not named')
        print('selftest: identical runs clean' if ok_green else 'selftest FAIL: identical runs flagged')
        return 0 if (ok_red and ok_green) else 1
    finally:
        for p in paths:
            os.unlink(p)


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
