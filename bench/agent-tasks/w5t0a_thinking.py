#!/usr/bin/env python3
"""W5.T0.a: why does manuk's Haiku think more? Per task, thinking tokens of manuk vs playwright+preload, next to the
size of the observation the model had just read.

  python3 -I w5t0a_thinking.py TRANSCRIPT_DIR ROWS_DIR OUT_PREFIX [--shuffle-join]

TRANSCRIPT_DIR holds <arm>_<task>_r<run>.jsonl (claude -p stream-json); ROWS_DIR holds the B4 <arm>.tsv and
<arm>.tsv.turns.tsv. --shuffle-join is the red mutant: it attaches each task's turn log to the next task before the join.

Sources, all exact (no apportioning):
  thinking_tokens  = result.usage.output_tokens_details.thinking_tokens of the run (transcript).
  obs bytes        = turns.tsv result_bytes. Turn i's result_bytes are the tool results turn i issued, so the observation
                     the model read before turn i+1 is turn i's result_bytes. obs_prev of a run = mean over turns 1..n-1.
  join check       = turns.tsv result_bytes summed per run must equal the text bytes of the tool results recomputed from
                     the transcript of the same run; a shuffled join fails this on nearly every run.
Output: OUT_PREFIX.tsv, one row per task (both arms, eligible = both arms pass >= 2 of 3 runs). Summary to stdout.
"""
import csv, collections, json, math, os, sys

argv = sys.argv[1:]
shuffle = "--shuffle-join" in argv
args = [a for a in argv if not a.startswith("--")]
tdir, rdir, out = args
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")
ARMS = {"manuk": "manuk", "playwright+preload": "playwright_preload"}  # arm -> rows file stem; transcripts use the arm name


def transcript_facts(path):
    """thinking tokens (exact, from the result line) and the text bytes of every tool result (the join check)."""
    think, res_bytes, final = 0, 0, None
    for line in open(path):
        d = json.loads(line)
        if d["type"] == "user":
            for b in d["message"]["content"]:
                if isinstance(b, dict) and b.get("type") == "tool_result":
                    c = b.get("content")
                    if isinstance(c, list):
                        res_bytes += sum(len(x.get("text", "").encode()) for x in c if isinstance(x, dict))
                    elif isinstance(c, str):
                        res_bytes += len(c.encode())
        elif d["type"] == "result":
            final = d["usage"]
    think = final.get("output_tokens_details", {}).get("thinking_tokens", 0)
    return think, res_bytes, final["output_tokens"]


def load_turns(fa):
    turns = collections.defaultdict(list)  # (task, run) -> [result_bytes in turn order]
    for r in csv.DictReader(open(f"{rdir}/{fa}.tsv.turns.tsv"), delimiter="\t"):
        if r["chrome_ver"] != PIN:
            raise SystemExit(f"RED wrong chrome_ver {r['chrome_ver']} in {fa}.tsv.turns.tsv")
        turns[(r["task"], int(r["run"]))].append((int(r["turn"]), int(r["result_bytes"])))
    return {k: [b for _, b in sorted(v)] for k, v in turns.items()}


def load_rows(fa):
    rows = {}
    for r in csv.DictReader(open(f"{rdir}/{fa}.tsv"), delimiter="\t"):
        if r["chrome_ver"] != PIN:
            raise SystemExit(f"RED wrong chrome_ver {r['chrome_ver']} in {fa}.tsv")
        rows[(r["task"], int(r["run"]))] = r
    return rows


def rank(xs):
    order = sorted(range(len(xs)), key=lambda i: xs[i])
    r, i = [0.0] * len(xs), 0
    while i < len(xs):
        j = i
        while j + 1 < len(xs) and xs[order[j + 1]] == xs[order[i]]:
            j += 1
        for k in range(i, j + 1):
            r[order[k]] = (i + j) / 2 + 1
        i = j + 1
    return r


def pearson(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sx = math.sqrt(sum((x - mx) ** 2 for x in xs))
    sy = math.sqrt(sum((y - my) ** 2 for y in ys))
    return sxy / (sx * sy) if sx and sy else float("nan")


def spearman(xs, ys):
    return pearson(rank(xs), rank(ys))


def mean(xs):
    return sum(xs) / len(xs) if xs else 0.0


rows = {a: load_rows(fa) for a, fa in ARMS.items()}
turns = {a: load_turns(fa) for a, fa in ARMS.items()}
if shuffle:
    # red mutant: each task's observation log is taken from the next task (sorted), i.e. a broken join key
    for a in turns:
        tasks = sorted({t for t, _ in turns[a]})
        nxt = {t: tasks[(i + 1) % len(tasks)] for i, t in enumerate(tasks)}
        turns[a] = {(nxt[t], r): v for (t, r), v in turns[a].items()}

bad = 0
per_run = {a: {} for a in ARMS}  # (task, run) -> (thinking, obs_prev, n_turns, pass)
for a, fa in ARMS.items():
    for (task, run), r in sorted(rows[a].items()):
        fn = f"{tdir}/{a}_{task}_r{run}.jsonl"
        if not os.path.exists(fn):
            print(f"RED missing transcript {fn}")
            bad += 1
            continue
        think, res_bytes, out_tok = transcript_facts(fn)
        tb = turns[a].get((task, run))
        if tb is None or sum(tb) != res_bytes or think > out_tok:
            bad += 1
            print(f"RED join {a} {task} r{run}: turns bytes {sum(tb) if tb else None} vs transcript {res_bytes}")
            continue
        n = len(tb)
        obs = mean(tb[:-1]) if n > 1 else 0.0
        per_run[a][(task, run)] = (think, obs, n, r["pass"] == "1")

tasks = sorted({t for a in ARMS for (t, _) in per_run[a]})
fields = ["task", "slice", "eligible", "pass_manuk", "pass_pw", "thinking_manuk", "thinking_pw", "thinking_gap",
          "obs_prev_manuk", "obs_prev_pw", "obs_gap", "turns_manuk", "turns_pw", "chrome_ver"]
table = []
for t in tasks:
    m = [per_run["manuk"].get((t, r)) for r in (1, 2, 3)]
    p = [per_run["playwright+preload"].get((t, r)) for r in (1, 2, 3)]
    if not all(m) or not all(p):
        print(f"RED incomplete runs for {t}")
        bad += 1
        continue
    slice_ = rows["manuk"][(t, 1)]["slice"]
    pm, pp = sum(x[3] for x in m), sum(x[3] for x in p)
    th_m, th_p = mean([x[0] for x in m]), mean([x[0] for x in p])
    ob_m, ob_p = mean([x[1] for x in m]), mean([x[1] for x in p])
    table.append(dict(task=t, slice=slice_, eligible=int(pm >= 2 and pp >= 2), pass_manuk=pm, pass_pw=pp,
                      thinking_manuk=round(th_m, 1), thinking_pw=round(th_p, 1), thinking_gap=round(th_m - th_p, 1),
                      obs_prev_manuk=round(ob_m, 1), obs_prev_pw=round(ob_p, 1), obs_gap=round(ob_m - ob_p, 1),
                      turns_manuk=mean([x[2] for x in m]), turns_pw=mean([x[2] for x in p]), chrome_ver=PIN))

with open(out + ".tsv", "w") as f:
    f.write("\t".join(fields) + "\n")
    for row in table:
        f.write("\t".join(str(row[k]) for k in fields) + "\n")

el = [x for x in table if x["eligible"]]
print(f"tasks {len(table)}, eligible (both arms >=2/3 pass) {len(el)}, RED {bad}" + ("  [mutant: --shuffle-join]" if shuffle else ""))
if el:
    gaps = sorted((x["thinking_gap"] for x in el), reverse=True)
    tot_pos = sum(g for g in gaps if g > 0)
    print(f"thinking gap manuk-pw, eligible: mean {mean(gaps):.1f} tok/run; tasks with gap>0: {sum(g > 0 for g in gaps)}"
          f"; top-10 tasks hold {sum(gaps[:10]) / tot_pos * 100:.0f}% of the positive gap; top-10 sum {sum(gaps[:10]):.0f}"
          f" of total {sum(gaps):.0f}")
    th = [x["thinking_gap"] for x in el]
    ob = [x["obs_gap"] for x in el]
    print(f"n={len(el)} tasks: Pearson(thinking_gap, obs_gap) = {pearson(th, ob):.2f}, "
          f"Spearman = {spearman(th, ob):.2f}")
    thm = [x["thinking_manuk"] for x in el]
    obm = [x["obs_prev_manuk"] for x in el]
    print(f"n={len(el)} tasks, manuk only: Pearson(thinking_manuk, obs_prev_manuk) = {pearson(thm, obm):.2f}, "
          f"Spearman = {spearman(thm, obm):.2f}")
    # run level, manuk: thinking per turn against the observation read before it
    xs = [v[1] for k, v in per_run["manuk"].items() if v[2] > 1]
    ys = [v[0] / v[2] for k, v in per_run["manuk"].items() if v[2] > 1]
    print(f"n={len(xs)} manuk runs: Pearson(obs_prev, thinking per turn) = {pearson(xs, ys):.2f}, "
          f"Spearman = {spearman(xs, ys):.2f}")
sys.exit(1 if bad else 0)
