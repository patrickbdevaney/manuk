#!/usr/bin/env python3
"""W6.Q1 batch plane: a queue of job specs run by a detached worker (no LLM in the worker, no agent session).

  bench_queue.py enqueue bench-queue/specs/X.json   # copy the spec into bench-queue/pending/
  bench_queue.py worker [--once]                    # run pending jobs (cache-checked); the systemd-run unit's entrypoint
  bench_queue.py start                              # systemd-run --user the worker (survives the agent session)
  bench_queue.py status [JOB]                       # pending / running / done, and banded rows
  bench_queue.py self-test                          # the cache key, the band check and the intervention count can go red

A spec is JSON: {id, steps:[{name, argv:[...], out_tsv?}], inputs:[paths hashed into the key], band:{...},
reconcile:{tsv_glob, primary_judge, rerun_argv_template}}. The worker never reads task text or calls a model; the
steps do (real_run.py), exactly as before. Cache key = sha256(HEAD commit-or-binary hash, spec bytes, input file
hashes, MANUK_CHROME_PIN). A job whose key already has results/<key>/DONE is a cache hit and runs nothing.
Banded row (results/<key>/band.tsv): ts job key chrome_ver commit wall_s rows interventions auto_retries metric value band_lo band_hi in_band.
"""
import csv, collections, glob, hashlib, json, os, shutil, subprocess, sys, tempfile, time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
Q = os.path.join(ROOT, "bench-queue")
PENDING, RESULTS = os.path.join(Q, "pending"), os.path.join(Q, "results")


def sha(b):
    return hashlib.sha256(b).hexdigest()


def commit():
    return subprocess.run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()


def job_key(spec_bytes, spec, chrome, head):
    h = [head, chrome, sha(spec_bytes)]
    for p in spec.get("inputs", []):
        fp = os.path.join(ROOT, p)
        h.append(p + ":" + (sha(open(fp, "rb").read()) if os.path.isfile(fp) else "MISSING"))
    return sha("\n".join(h).encode())[:16]


def read_rows(paths):
    rows = []
    for p in paths:
        if os.path.exists(p):
            with open(p, newline="") as f:
                rows += list(csv.DictReader(f, delimiter="\t"))
    return rows


def bands(rows, arm_runs_key="run"):
    """per-run pass totals and tasks passing >=2 of the runs, from REAL-format rows (pass in 0/1)."""
    per = collections.Counter(); by = collections.defaultdict(list)
    for r in rows:
        if r.get("pass") in ("0", "1"):
            per[int(r[arm_runs_key])] += int(r["pass"]); by[r["task"]].append(int(r["pass"]))
    return [per[k] for k in sorted(per)], sum(1 for v in by.values() if sum(v) >= 2)


def intervention_count(rows, primary_judge, expected):
    """rows that would need a human/agent touch: unjudged/unscored rows, wrong-judge rows, and missing (task,run) keys."""
    seen = {(r["task"], r["run"]) for r in rows if r.get("pass") in ("0", "1") and r.get("judge_model") in ("", primary_judge)}
    return max(len(expected) - len(seen), 0)


def failure(n_steps, fails, rows):
    """W6.Q1.a.a: why a job must end FAILED instead of DONE (None = it may be DONE). A job whose every step exited
    nonzero, or whose band TSV has no rows, never produced a measurement; banding it would mint a fake in-band row."""
    if os.environ.get("BENCHQ_MUTATE_FAIL") == "1":  # gate mutant: never refuse
        return None
    if n_steps and fails == n_steps:
        return f"every step exited nonzero ({fails}/{n_steps})"
    if not rows:
        return "band TSV has 0 rows"
    return None


def check_band(value, lo, hi):
    if os.environ.get("BENCHQ_MUTATE") == "1":  # gate mutant: a band check that never fails
        return True
    return lo <= value <= hi


def run_step(step, log, env):
    t = time.time()
    with open(log, "ab") as lf:
        lf.write(f"{time.strftime('%FT%T')} STEP {step['name']}\n".encode()); lf.flush()
        rc = subprocess.run(step["argv"], cwd=ROOT, stdout=lf, stderr=subprocess.STDOUT, env=env).returncode
    return rc, time.time() - t


def infra_failed(r):
    """A row the harness broke, not the agent: is_error with 0 output tokens, or the 'Connection closed' signature
    the Playwright roots deadlock left in the answer (observed in 17 of the 18 rows moved aside by hand for W6.Q1:
    output_tokens>0, is_error=0, so the answer text is the only marker)."""
    if os.environ.get("BENCHQ_MUTATE_INFRA") == "1":  # gate mutant: no infra check
        return False
    if r.get("output_tokens") == "0" and (r.get("is_error") or "").strip().lower() in ("1", "true"):
        return True
    return "Connection closed" in (r.get("answer") or "")


def _write_tsv(path, fields, rows, append=False):
    with open(path, "a" if append else "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields, delimiter="\t")
        if not append or f.tell() == 0:
            w.writeheader()
        w.writerows(rows)


def reconcile(spec, log, env):
    """Automatic rework: move non-primary-judge rows (B2-rejudge.sh by hand) and infra-failed rows (W6.Q1 by hand)
    aside into <tsv>.fallback-dropped / <tsv>.infra-failed, so the rerun steps refill them. Returns rows moved."""
    rc = spec.get("reconcile")
    if not rc:
        return 0
    moved = 0
    for tsv in glob.glob(os.path.join(ROOT, rc["tsv_glob"])):
        rows = read_rows([tsv])
        if not rows:
            continue
        fields = list(rows[0].keys())
        judge_bad = [r for r in rows if r.get("judge_model") not in ("", rc["primary_judge"])]
        infra_bad = [r for r in rows if r not in judge_bad and infra_failed(r)]
        if not judge_bad and not infra_bad:
            continue
        keep = [r for r in rows if r not in judge_bad and r not in infra_bad]
        if judge_bad:
            _write_tsv(tsv + ".fallback-dropped", fields, judge_bad, append=True)
        if infra_bad:
            _write_tsv(tsv + ".infra-failed", fields, infra_bad, append=True)
        _write_tsv(tsv, fields, keep)
        moved += len(judge_bad) + len(infra_bad)
    return moved


def run_job(path, force=False, results=RESULTS):
    spec_bytes = open(path, "rb").read(); spec = json.loads(spec_bytes)
    chrome = os.environ.get("MANUK_CHROME_PIN", "")
    if not chrome:
        raise SystemExit("refusing: MANUK_CHROME_PIN unset")
    head = commit(); key = job_key(spec_bytes, spec, chrome, head)
    rdir = os.path.join(results, f"{spec['id']}-{key}"); os.makedirs(rdir, exist_ok=True)
    if os.path.exists(os.path.join(rdir, "DONE")) and not force:
        print(f"CACHE HIT {spec['id']} {key}"); return rdir, True
    for stale in ("FAILED",):  # a FAILED job re-runs (it has no DONE, so it is never a cache hit)
        if os.path.exists(os.path.join(rdir, stale)):
            os.remove(os.path.join(rdir, stale))
    log = os.path.join(rdir, "worker.log"); env = dict(os.environ); t0 = time.time()
    open(os.path.join(rdir, "RUNNING"), "w").write(str(os.getpid()))
    fails = 0
    for st in spec["steps"]:
        os.utime(os.path.join(ROOT, ".git", "sprint-working"), None) if os.path.exists(os.path.join(ROOT, ".git", "sprint-working")) else None
        rc, _ = run_step(st, log, env)
        if rc != 0:
            fails += 1
    auto = 0
    for _ in range(3):  # reconcile rounds
        n = reconcile(spec, log, env)
        if not n:
            break
        auto += n
        for st in spec.get("rerun_steps", spec["steps"]):
            run_step(st, log, env)  # the harness skips (arm,task,run) rows already present
    rc = spec.get("reconcile", {})
    rows = read_rows(glob.glob(os.path.join(ROOT, spec["band"]["tsv_glob"])))
    runs, t2 = bands(rows)
    expected = {(t, str(r)) for t in spec["band"].get("tasks", []) for r in range(1, spec["band"]["runs"] + 1)}
    interv = intervention_count(rows, rc.get("primary_judge", ""), expected) + fails
    n_tasks = max(len(spec["band"].get("tasks", [])), 1)
    ok_int = interv <= 0.10 * len(expected) if expected else fails == 0
    lo, hi = spec["band"]["task_pass_2of3"]
    wall = time.time() - t0
    why = failure(len(spec["steps"]), fails, rows)
    # a FAILED job writes no in-band verdict: in_band stays empty, so nothing downstream can read it as a pass
    in_band = "" if why else int(check_band(t2, lo, hi) and all(check_band(x, *spec["band"].get("per_run", [lo, hi])) for x in runs))
    with open(os.path.join(rdir, "band.tsv"), "w") as f:
        f.write("ts\tjob\tkey\tchrome_ver\tcommit\twall_s\trows\tinterventions\tauto_retries\tmetric\tvalue\tband_lo\tband_hi\tin_band\n")
        f.write("\t".join(map(str, [time.strftime("%FT%T"), spec["id"], key, chrome, head, round(wall), len(rows), interv, auto,
                "task_pass_2of3", t2, lo, hi, in_band])) + "\n")
        f.write("\t".join(map(str, [time.strftime("%FT%T"), spec["id"], key, chrome, head, round(wall), len(rows), interv, auto,
                "pass_per_run", "/".join(map(str, runs)), "", "", ""])) + "\n")
    os.remove(os.path.join(rdir, "RUNNING"))
    if why:
        open(os.path.join(rdir, "FAILED"), "w").write(why + "\n")
        print(f"FAILED {spec['id']} {key} wall={wall:.0f}s reason: {why}")
        return rdir, False
    open(os.path.join(rdir, "DONE"), "w").write(f"{int(ok_int)}\n")
    print(f"DONE {spec['id']} {key} wall={wall:.0f}s task_pass_2of3={t2} runs={runs} interventions={interv}")
    return rdir, False


def worker(once):
    while True:
        jobs = sorted(glob.glob(os.path.join(PENDING, "*.json")))
        if not jobs:
            if once:
                return
            time.sleep(30); continue
        j = jobs[0]; run_job(j)
        os.remove(j)
        if once and not glob.glob(os.path.join(PENDING, "*.json")):
            return


def self_test():
    ok = True
    spec = {"id": "t", "inputs": ["scripts/bench_queue.py"]}
    b = json.dumps(spec).encode()
    k1 = job_key(b, spec, "145", "aaa"); k2 = job_key(b, spec, "146", "aaa"); k3 = job_key(b, spec, "145", "bbb")
    k4 = job_key(b + b" ", spec, "145", "aaa")
    for name, cond in [("key stable", k1 == job_key(b, spec, "145", "aaa")), ("chrome_ver changes key", k1 != k2),
                       ("commit changes key", k1 != k3), ("spec bytes change key", k1 != k4)]:
        print(("ok  " if cond else "RED ") + name); ok &= cond
    rows = [{"task": "a", "run": "1", "pass": "1", "judge_model": "J"}, {"task": "a", "run": "2", "pass": "1", "judge_model": "J"},
            {"task": "b", "run": "1", "pass": "0", "judge_model": "J"}, {"task": "b", "run": "2", "pass": "1", "judge_model": "fallback"}]
    runs, t2 = bands(rows)
    exp = {(t, str(r)) for t in "ab" for r in (1, 2)}
    iv = intervention_count(rows, "J", exp)
    for name, cond in [("band counts a=2of2", t2 == 1), ("fallback-judged row is an intervention", iv == 1),
                       ("out-of-band value is red", not check_band(37, 38, 45)), ("in-band value is green", check_band(38, 38, 45))]:
        print(("ok  " if cond else "RED ") + name); ok &= cond
    # infra-failed rows: moved aside by reconcile and counted (W6.Q1.b)
    def row(task, pass_, judge, out_tok, err, answer):
        return {"task": task, "run": "1", "pass": pass_, "judge_model": judge, "output_tokens": out_tok, "is_error": err, "answer": answer}
    clean = row("a", "1", "J", "500", "0", "ok")
    err0 = row("b", "0", "J", "0", "1", "")
    conn = row("c", "0", "J", "569", "0", "navigation returned Connection closed")
    fallback = row("d", "1", "fallback", "500", "0", "ok")
    for name, cond in [("is_error with 0 tokens is infra", infra_failed(err0)), ("Connection closed answer is infra", infra_failed(conn)),
                       ("clean row is not infra", not infra_failed(clean))]:
        print(("ok  " if cond else "RED ") + name); ok &= cond
    tmp = tempfile.mkdtemp(prefix="bq-selftest-")
    fields = list(clean.keys())
    tsv = os.path.join(tmp, "rows.tsv")
    _write_tsv(tsv, fields, [clean, err0, conn, fallback])
    moved = reconcile({"reconcile": {"tsv_glob": os.path.join(tmp, "*.tsv"), "primary_judge": "J"}}, os.path.join(tmp, "log"), {})
    kept = read_rows([tsv]); infra_rows = read_rows([tsv + ".infra-failed"]); fb_rows = read_rows([tsv + ".fallback-dropped"])
    for name, cond in [("reconcile counts 3 moved (2 infra + 1 fallback)", moved == 3),
                       ("only the clean row stays in the TSV", [r["task"] for r in kept] == ["a"]),
                       ("infra rows land in .infra-failed", sorted(r["task"] for r in infra_rows) == ["b", "c"]),
                       ("fallback row lands in .fallback-dropped", [r["task"] for r in fb_rows] == ["d"])]:
        print(("ok  " if cond else "RED ") + name); ok &= cond
    shutil.rmtree(tmp, ignore_errors=True)
    # W6.Q1.a.a: end-to-end, a spec whose only step exits 1 and whose band TSV has no rows must end FAILED (not DONE),
    # with an empty in_band cell. Runs in a temp results dir so the real bench-queue/results is never touched.
    os.environ.setdefault("MANUK_CHROME_PIN", "145.0.7632.116")
    tmp2 = tempfile.mkdtemp(prefix="bq-failed-")
    band_tsv = os.path.join(tmp2, "band", "rows.tsv"); os.makedirs(os.path.dirname(band_tsv))
    _write_tsv(band_tsv, fields, [])  # header only: 0 rows
    spec_f = {"id": "tf", "steps": [{"name": "boom", "argv": [sys.executable, "-c", "raise SystemExit(1)"]}],
              "band": {"tsv_glob": band_tsv, "tasks": ["a"], "runs": 1, "task_pass_2of3": [0, 10]}}
    spec_path = os.path.join(tmp2, "spec.json"); open(spec_path, "w").write(json.dumps(spec_f))
    import io, contextlib
    with contextlib.redirect_stdout(io.StringIO()):
        rdir, _ = run_job(spec_path, results=os.path.join(tmp2, "results"))
    band_row = open(os.path.join(rdir, "band.tsv")).read().splitlines()[1].split("\t")
    for name, cond in [("failing step + empty TSV ends FAILED", os.path.exists(os.path.join(rdir, "FAILED"))),
                       ("FAILED job has no DONE marker", not os.path.exists(os.path.join(rdir, "DONE"))),
                       ("FAILED job band.tsv in_band is empty", band_row[-1] == "")]:
        print(("ok  " if cond else "RED ") + name); ok &= cond
    shutil.rmtree(tmp2, ignore_errors=True)
    print("SELF-TEST", "GREEN" if ok else "RED"); sys.exit(0 if ok else 1)


def status(job):
    for d in sorted(glob.glob(os.path.join(RESULTS, (job or "") + "*"))):
        st = ("DONE" if os.path.exists(d + "/DONE") else "FAILED" if os.path.exists(d + "/FAILED")
              else "RUNNING" if os.path.exists(d + "/RUNNING") else "?")
        print(os.path.basename(d), st)
        if os.path.exists(d + "/band.tsv"):
            print(open(d + "/band.tsv").read())
    print("pending:", [os.path.basename(p) for p in glob.glob(PENDING + "/*.json")])


def main():
    a = sys.argv[1:]
    if not a:
        raise SystemExit(__doc__)
    os.makedirs(PENDING, exist_ok=True); os.makedirs(RESULTS, exist_ok=True)
    if a[0] == "enqueue":
        shutil.copy(a[1], PENDING); print("queued", os.path.basename(a[1]))
    elif a[0] == "worker":
        worker("--once" in a)
    elif a[0] == "start":
        unit = "manuk-bench-worker"
        # observer 2026-11-21 (owner-approved): the worker was host-OOM-killed 3x and once took owner work with it;
        # cage it (worker + its Chrome/judge children share the unit cgroup) and make it the first OOM victim
        cmd = ["systemd-run", "--user", "--collect", f"--unit={unit}", f"--working-directory={ROOT}",
               "-p", "MemoryMax=10G", "-p", "MemoryHigh=8G", "-p", "MemorySwapMax=1G", "-p", "OOMScoreAdjust=1000",
               f"--setenv=MANUK_CHROME_PIN={os.environ.get('MANUK_CHROME_PIN','')}", f"--setenv=PATH={':'.join(dict.fromkeys(os.environ['PATH'].split(':')))}",
               f"--setenv=HOME={os.environ['HOME']}", "python3", "-I", os.path.join(ROOT, "scripts/bench_queue.py"), "worker", "--once"]
        print(subprocess.run(cmd, capture_output=True, text=True))
    elif a[0] == "status":
        status(a[1] if len(a) > 1 else None)
    elif a[0] == "self-test":
        self_test()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
