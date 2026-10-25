#!/usr/bin/env python3
"""R0.1 desktop baseline: peak memory and wall time of one headless page load,
manuk (a11y-dump, the agent's own page load + a11y build) vs headless Chrome (--dump-dom).

Memory is sampled by walking the process tree every 50 ms and summing VmRSS and Pss
(from /proc/<pid>/smaps_rollup). Chrome's children share pages, so summed RSS over-counts;
Pss splits shared pages across their users and is the number to compare. Both are reported.

Usage (run with -I so no local module can shadow the stdlib):
  python3 -I scripts/r01-baseline.py --urls <file> --runs 3 --out <dir> \
      --manuk target/release/a11y-dump --chrome google-chrome

Writes <out>/runs.tsv (one row per url x engine x run) and prints a summary.
"""
import argparse
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


def children_map():
    kids = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/stat") as f:
                stat = f.read()
            ppid = int(stat.rsplit(")", 1)[1].split()[1])
        except (OSError, ValueError, IndexError):
            continue
        kids.setdefault(ppid, []).append(int(entry))
    return kids


def tree(root):
    kids = children_map()
    out, stack = [], [root]
    while stack:
        pid = stack.pop()
        out.append(pid)
        stack.extend(kids.get(pid, []))
    return out


def mem_of(pid):
    rss = pss = 0
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    rss = int(line.split()[1])  # kB
                    break
        with open(f"/proc/{pid}/smaps_rollup") as f:
            for line in f:
                if line.startswith("Pss:"):
                    pss = int(line.split()[1])
                    break
    except OSError:
        pass
    return rss, pss


def run_measured(argv, timeout_s):
    """Returns (wall_s, peak_rss_kb, peak_pss_kb, exit_code, status)."""
    t0 = time.monotonic()
    try:
        proc = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        return 0.0, 0, 0, -1, "SPAWN"
    peak_rss = peak_pss = 0
    status = "OK"
    while True:
        rc = proc.poll()
        rss_sum = pss_sum = 0
        for pid in tree(proc.pid):
            r, p = mem_of(pid)
            rss_sum += r
            pss_sum += p
        peak_rss = max(peak_rss, rss_sum)
        peak_pss = max(peak_pss, pss_sum)
        if rc is not None:
            break
        if time.monotonic() - t0 > timeout_s:
            status = "TIMEOUT"
            for pid in tree(proc.pid):
                try:
                    os.kill(pid, 9)
                except OSError:
                    pass
            proc.wait()
            break
        time.sleep(0.05)
    wall = time.monotonic() - t0
    if status == "OK" and proc.returncode != 0:
        status = "FAIL"
    return wall, peak_rss, peak_pss, proc.returncode, status


def chrome_argv(chrome, url, profile):
    return [
        chrome, "--headless=new", "--no-sandbox", "--disable-gpu", "--no-first-run",
        f"--user-data-dir={profile}", "--dump-dom", url,
    ]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--urls", required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--out", required=True)
    ap.add_argument("--manuk", required=True)
    ap.add_argument("--chrome", required=True)
    ap.add_argument("--timeout", type=float, default=60.0)
    ap.add_argument("--engines", default="manuk,chrome", help="comma list: manuk,chrome")
    args = ap.parse_args()
    engines = [e for e in args.engines.split(",") if e]

    with open(args.urls) as f:
        urls = [ln.strip() for ln in f if ln.strip() and not ln.startswith("#")]
    # Every row carries the Chrome pin it was measured under; numbers from two pins are never diffed.
    chrome_ver = os.environ.get("MANUK_CHROME_PIN")
    if not chrome_ver:
        sys.exit("MANUK_CHROME_PIN unset: refusing to write rows without a chrome_ver")
    os.makedirs(args.out, exist_ok=True)
    rows = []
    for run in range(1, args.runs + 1):
        for url in urls:
            for engine in engines:
                if engine == "manuk":
                    argv = [args.manuk, url]
                    profile = None
                else:
                    profile = tempfile.mkdtemp(prefix="r01-chrome-")
                    argv = chrome_argv(args.chrome, url, profile)
                try:
                    wall, rss, pss, rc, status = run_measured(argv, args.timeout)
                finally:
                    if profile:
                        shutil.rmtree(profile, ignore_errors=True)
                rows.append((run, url, engine, round(wall, 3), rss, pss, rc, status))
                print(f"run{run} {engine:6} {status:7} {wall:6.2f}s rss={rss/1024:7.1f}MB "
                      f"pss={pss/1024:7.1f}MB {url}", file=sys.stderr)

    with open(os.path.join(args.out, "runs.tsv"), "w") as f:
        f.write("run\turl\tengine\twall_s\tpeak_rss_kb\tpeak_pss_kb\texit\tstatus\tchrome_ver\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r + (chrome_ver,)) + "\n")

    print("\nengine  per-run totals (sum over urls of peak PSS MB) | per-run median wall s")
    for engine in engines:
        pss_totals, walls = [], []
        for run in range(1, args.runs + 1):
            sel = [r for r in rows if r[0] == run and r[2] == engine]
            pss_totals.append(sum(r[5] for r in sel) / 1024)
            walls.append(statistics.median(r[3] for r in sel))
        print(f"{engine:6}  pss_sum_MB={[round(x, 1) for x in pss_totals]}  "
              f"median_wall_s={[round(x, 2) for x in walls]}")
    ok = {e: sum(1 for r in rows if r[2] == e and r[7] == "OK") for e in engines}
    print(f"ok counts: {ok} of {len(urls) * args.runs} each")


if __name__ == "__main__":
    main()
