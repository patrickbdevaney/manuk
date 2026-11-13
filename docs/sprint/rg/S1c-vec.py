#!/usr/bin/env python3
"""RG.S1.c.a.a.a: replay check for the evals_vec column (real_run.grade_vec).

  python3 -I docs/sprint/rg/S1c-vec.py            # exit 1 unless every replayed vec matches tsv evals_ok
  python3 -I docs/sprint/rg/S1c-vec.py --mutate   # drop "event" from the typed text; exit 1 unless vec == [1,0,1]

Rebuilds feedPostsDiff.added[0].description from the typed text in the committed E2c-ahead
networkin-3 transcripts and grades it with the task's real jmespath evals.
"""
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..'))
sys.path.insert(0, os.path.join(ROOT, 'bench', 'agent-tasks'))
import real_run  # noqa: E402

spec = importlib.util.spec_from_file_location('noise', os.path.join(HERE, 'S1c-noise.py'))
noise = importlib.util.module_from_spec(spec)
spec.loader.exec_module(noise)

task = json.load(open(os.path.join(ROOT, 'bench/agent-tasks/real/tasks/networkin-3.json')))
mutate = '--mutate' in sys.argv
rows = [r for r in __import__('csv').DictReader(open(os.path.join(HERE, 'E2c-ahead-real-manuk.tsv')), delimiter='\t') if r['task'] == 'networkin-3']
bad = 0
for r in rows:
    text = noise.typed_text(os.path.join(HERE, 'E2c-ahead-transcripts', 'manuk_networkin-3_r%s.jsonl' % r['run']))
    if mutate:
        text = text.replace('event', '').replace('Event', '')
    env = {'feedPostsDiff': {'added': [{'description': text}] if text else []}}
    vec = [int(x) for x in real_run.grade_vec(task, env)]
    want = [1, 0, 1] if mutate else None
    ok = vec == want if mutate else sum(vec) == int(r['evals_ok'])
    print('r%s vec=%s evals_ok=%s %s' % (r['run'], vec, r['evals_ok'], 'ok' if ok else 'RED'))
    bad += not ok
sys.exit(1 if bad else 0)
