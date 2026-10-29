#!/usr/bin/env python3
"""TOKENS.1.b: per task, manuk-minus-Playwright in+out tokens (3-run means) and the
dominant tool-result bucket in the manuk transcripts.

Inputs:
  docs/sprint/SCOREBOARD.tsv         manuk baseline = suite 69903c rows dated 2026-10-25 (R2.1.6),
                                     playwright = suite 69903c rows dated 2026-10-24 (R2.1.6)
  docs/sprint/tokens1/transcripts/   manuk_<task>_r<run>.jsonl (TOKENS.1 run; the code change was reverted)
  manuk rows dated 2026-10-29 (the TOKENS.1 rerun, same rows as tokens1/rows.tsv) are shown as a cross-check column
Tokens = input + output + cache_create (cache_read reported as a separate delta column).
Output: docs/sprint/tokens1b/top10.tsv, plus the top 10 printed to stdout.
Exits 1 on empty input.
"""
import csv, collections, glob, json, os, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..')
ROOT = os.path.normpath(ROOT)
SB = os.path.join(ROOT, 'docs/sprint/SCOREBOARD.tsv')
TR = os.path.join(ROOT, 'docs/sprint/tokens1/transcripts')
OUT = os.path.join(ROOT, 'docs/sprint/tokens1b/top10.tsv')
SUITE = '69903c'
CHROME = os.environ.get('MANUK_CHROME_PIN', '145.0.7632.116')

def tok(r):
    # in+out as the notes use it: uncached input + output + cache writes. cache_read is excluded
    # (it is the re-read of the schema prefix each turn, reported separately in the TSV).
    return int(r['input_tokens'] or 0) + int(r['output_tokens'] or 0) + int(r['cache_create_tokens'] or 0)

def full(r):
    return tok(r) + int(r['cache_read_tokens'] or 0)

rows = [r for r in csv.DictReader(open(SB), delimiter='\t')
        if r['suite_hash'].startswith(SUITE) and r['chrome_ver'] == CHROME]
manuk = collections.defaultdict(list)
pw = collections.defaultdict(list)
manuk_full = collections.defaultdict(list)
tokens1 = collections.defaultdict(list)  # TOKENS.1 rerun (transcript source), same metric
pw_full = collections.defaultdict(list)
for r in rows:
    if r['arm'] == 'manuk' and r['ts'].startswith('2026-10-29'):
        tokens1[r['task']].append(tok(r))
    elif r['arm'] == 'manuk' and r['ts'].startswith('2026-10-25'):
        manuk[r['task']].append(tok(r))
        manuk_full[r['task']].append(full(r))
    elif r['arm'] == 'playwright' and r['ts'].startswith('2026-10-24'):
        pw[r['task']].append(tok(r))
        pw_full[r['task']].append(full(r))
tasks = sorted(set(manuk) & set(pw))
if not tasks:
    print('no paired tasks in SCOREBOARD', file=sys.stderr)
    sys.exit(1)

def bucket_bytes(path):
    """tool name -> [calls, result bytes] for one transcript."""
    names = {}
    out = collections.defaultdict(lambda: [0, 0])
    for line in open(path):
        d = json.loads(line)
        msg = d.get('message')
        if not isinstance(msg, dict) or not isinstance(msg.get('content'), list):
            continue
        for c in msg['content']:
            if c.get('type') == 'tool_use':
                n = c['name'].replace('mcp__browser__', '')
                names[c['id']] = n
                out[n][0] += 1
            elif c.get('type') == 'tool_result':
                n = names.get(c.get('tool_use_id'), 'other')
                body = c.get('content')
                size = len(json.dumps(body)) if not isinstance(body, str) else len(body)
                out[n][1] += size
    return out

per_task = {}
for path in sorted(glob.glob(os.path.join(TR, 'manuk_*_r*.jsonl'))):
    task = os.path.basename(path).split('_')[1]
    agg = per_task.setdefault(task, collections.defaultdict(lambda: [0, 0]))
    for n, (c, b) in bucket_bytes(path).items():
        agg[n][0] += c
        agg[n][1] += b

result = []
for t in tasks:
    m = sum(manuk[t]) / len(manuk[t])
    p = sum(pw[t]) / len(pw[t])
    agg = per_task.get(t, {})
    tot = sum(b for _, b in agg.values()) or 1
    if agg:
        dom, (dc, db) = max(agg.items(), key=lambda kv: kv[1][1])
        dom_s = f'{dom} ({db / tot:.0%} of result bytes, {dc} calls)'
    else:
        dom_s = 'no transcript'
    mf = sum(manuk_full[t]) / len(manuk_full[t])
    pf = sum(pw_full[t]) / len(pw_full[t])
    t1 = sum(tokens1[t]) / len(tokens1[t]) if tokens1[t] else float('nan')
    result.append((m - p, t, m, p, dom_s, mf - pf, t1))

result.sort(key=lambda x: x[0], reverse=True)
if not result:
    sys.exit(1)
top = result[:10]
with open(OUT, 'w') as f:
    f.write('task\tmanuk_tokens_mean\tplaywright_tokens_mean\tdelta\tdelta_incl_cache_read\tmanuk_tokens1_rerun_mean\tdominant_bucket\tchrome_ver\n')
    for delta, t, m, p, dom, dfull, t1 in top:
        f.write(f'{t}\t{m:.0f}\t{p:.0f}\t{delta:+.0f}\t{dfull:+.0f}\t{t1:.0f}\t{dom}\t{CHROME}\n')
print(open(OUT).read())
