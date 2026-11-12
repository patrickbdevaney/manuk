import csv, sys
path, pin = sys.argv[1], sys.argv[2]
rows = list(csv.DictReader(open(path), delimiter='\t'))
bad = [r['run'] for r in rows if r['chrome_ver'] != pin or r['judge_model'] != 'openai/gpt-oss-120b']
passes = sum(int(r['pass']) for r in rows)
print(f"rows={len(rows)} pass_runs={passes}/3 bad_rows={bad}")
sys.exit(1 if bad or len(rows) != 3 or passes < 2 else 0)
