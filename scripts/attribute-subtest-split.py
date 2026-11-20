#!/usr/bin/env python3
"""Attribute the change in a WPT directory's subtest denominator to files.

Input: two WPT-driver logs (before, after). Each log prints a `  FILE <path>  (p/t)` header for
every file that has at least one non-passing subtest, followed by its FAIL/Timeout lines, and one
summary line `  FILES  N  subtests P/T  =  x%`.

Attribution: dTotal = after_T - before_T. Files with a FILE header contribute their header t
exactly; files without one (all subtests passing, or no subtests reported) print no total, so
their net change is reported as a residual and is NOT attributed to any file.

Usage:
  python3 -I scripts/attribute-subtest-split.py BEFORE.log AFTER.log [--chrome-ver VER]
  python3 -I scripts/attribute-subtest-split.py --selftest

Exit 0 only when the residual is 0 (the split is complete). Exit 1 otherwise (row stays open).
"""
import os
import re
import sys
import tempfile

FILE_RE = re.compile(r'^  FILE (\S+)\s+\((\d+)/(\d+)\)\s*$')
SUMMARY_RE = re.compile(r'^\s*FILES\s+\d+\s+subtests (\d+)/(\d+)\s*=')
FAIL_RE = re.compile(r'^    (FAIL|Timeout|NOTRUN|ERROR|PRECONDITION_FAILED) (.*)$')


def parse(path):
    files, names, summary = {}, {}, None
    current = None
    with open(path, encoding='utf-8', errors='replace') as fh:
        for line in fh:
            line = line.rstrip('\n')
            m = FILE_RE.match(line)
            if m:
                current = m.group(1)
                files[current] = (int(m.group(2)), int(m.group(3)))
                names[current] = []
                continue
            m = SUMMARY_RE.match(line)
            if m:
                summary = (int(m.group(1)), int(m.group(2)))
                current = None
                continue
            if current is not None:
                f = FAIL_RE.match(line)
                if f:
                    names[current].append(f.group(1) + ' ' + f.group(2))
    if summary is None:
        raise SystemExit(f'no summary line in {path}')
    return files, names, summary


def analyse(before_path, after_path):
    bf, bn, bs = parse(before_path)
    af, an, as_ = parse(after_path)
    d_total = as_[1] - bs[1]
    d_pass = as_[0] - bs[0]
    attributed = {}
    for f in sorted(set(bf) | set(af)):
        b = bf.get(f, (0, 0))[1]
        a = af.get(f, (0, 0))[1]
        if a != b:
            attributed[f] = a - b
    attributed_total = sum(attributed.values())
    residual = d_total - attributed_total
    return {
        'before': bs, 'after': as_, 'd_total': d_total, 'd_pass': d_pass,
        'attributed': attributed, 'attributed_total': attributed_total, 'residual': residual,
        'before_names': bn, 'after_names': an,
    }


def report(res, chrome_ver):
    print(f'chrome_ver={chrome_ver}')
    print(f"before subtests {res['before'][0]}/{res['before'][1]}  after {res['after'][0]}/{res['after'][1]}")
    print(f"dTotal={res['d_total']}  dPass={res['d_pass']}")
    print('per-file attributed dTotal (FILE-header files only):')
    for f, v in sorted(res['attributed'].items(), key=lambda kv: -abs(kv[1])):
        print(f'  {v:+6d}  {f}')
    print(f"attributed={res['attributed_total']}  residual(unattributable, silent files)={res['residual']:+d}")
    verdict = 'COMPLETE' if res['residual'] == 0 else 'INCOMPLETE (row stays open)'
    print('split:', verdict)


def selftest():
    """Mutant: one extra subtest in one file must be attributed to that file and nothing else."""
    d = tempfile.mkdtemp(prefix='attr-selftest-')
    base = (
        '  FILE css/a.html  (1/4)\n'
        '    FAIL x\n'
        '      msg\n'
        '  FILE css/b.html  (0/2)\n'
        '    FAIL y\n'
        '      msg\n'
        '    FAIL z\n'
        '      msg\n'
        '  FILES  2   subtests 1/6  =  16.7%\n'
    )
    mutant = (
        '  FILE css/a.html  (1/4)\n'
        '    FAIL x\n'
        '      msg\n'
        '  FILE css/b.html  (0/3)\n'
        '    FAIL y\n'
        '      msg\n'
        '    FAIL z\n'
        '      msg\n'
        '  FILES  2   subtests 1/7  =  14.3%\n'
    )
    bp = os.path.join(d, 'before.log')
    mp = os.path.join(d, 'mutant.log')
    with open(bp, 'w') as fh:
        fh.write(base)
    with open(mp, 'w') as fh:
        fh.write(mutant)
    res = analyse(bp, mp)
    ok = res['attributed'] == {'css/b.html': 1} and res['residual'] == 0
    print('selftest mutant (one extra subtest in css/b.html):', 'GREEN' if ok else 'RED', res['attributed'], 'residual', res['residual'])
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ['--selftest']:
        return selftest()
    chrome_ver = os.environ.get('MANUK_CHROME_PIN', 'UNSET')
    args = [a for a in argv[1:] if not a.startswith('--')]
    if len(args) != 2:
        print(__doc__)
        return 2
    res = analyse(args[0], args[1])
    report(res, chrome_ver)
    for label, names in (('before', res['before_names']), ('after', res['after_names'])):
        for f in sorted(names):
            if 'non-rendered-element-001' in f:
                print(f'{label} {f}:')
                for n in names[f]:
                    print('   ', n)
    return 0 if res['residual'] == 0 else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv))
