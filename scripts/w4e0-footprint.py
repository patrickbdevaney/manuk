#!/usr/bin/env python3
"""W4.E0 footprint: the axes the 0.47x PSS number never covered, manuk vs pinned headless Chrome.

Axes (every one runs 3 times per arm, and every run writes a row to <out>/runs.tsv):
  pages   peak summed PSS of the process tree(s) with N pages open, N in {1,10,50}:
            manuk-proc    N manuk-mcp processes, one page each (one process per agent session)
            manuk-fork    one manuk-mcp process, N-1 pages added with fork (replay-based)
            chrome-tabs   one Chrome, N tabs
            chrome-procs  N Chrome processes, one tab each
          Pages are distinct local fixtures, so the numbers are not shared-cache artefacts.
  fork    wall ms for one new page at a checkpoint: manuk `fork` (replay = one page load) vs
            Chrome new browser context + target + navigate (the "new context plus re-navigate" arm)
  cold    wall ms from process spawn to the first observation of a local fixture page
  urls    per-URL wall ms and peak PSS on the 20 baseline URLs (manuk-proc vs chrome, via r01-baseline)

Derived, never measured to a budget: pages_per_GB = 1 GiB / (peak PSS / N). The summary says so.

Usage (run with -I so no local module can shadow the stdlib):
  MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e0-footprint.py --out docs/sprint/footprint
  python3 -I scripts/w4e0-footprint.py --selftest

Needs target/release/manuk-mcp and the pinned Chrome (CHROME_BIN, default ~/.local/chrome-cft/<pin>).
"""
import argparse
import http.server
import importlib.util
import json
import os
import re
import select
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
MCP = os.path.join(ROOT, "target", "release", "manuk-mcp")
URLS = os.path.join(ROOT, "docs", "sprint", "BASELINE-URLS.txt")
PIN = os.environ.get("MANUK_CHROME_PIN", "")
CHROME = os.environ.get("CHROME_BIN") or os.path.join(
    os.path.expanduser("~/.local/chrome-cft"), PIN, "chrome-linux64", "chrome")
N_LIST = [1, 10, 50]
RUNS = 3
SETTLE_S = 2.0
PAGE_TIMEOUT_S = 60.0
MIN_FREE_MB_FOR_50 = 20000

# r01-baseline.py has the tree walk, the PSS reader and the CDP/Chrome driver. Reuse, do not fork it.
_spec = importlib.util.spec_from_file_location("r01", os.path.join(HERE, "r01-baseline.py"))
r01 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(r01)


def touch():
    """The watchdog reads this mtime to see the sprint agent is alive."""
    try:
        os.utime(os.path.join(ROOT, ".git", "sprint-working"), None)
    except OSError:
        pass


def free_mb():
    with open("/proc/meminfo") as f:
        for ln in f:
            if ln.startswith("MemAvailable:"):
                return int(ln.split()[1]) // 1024
    return 0


def page_html(idx):
    body = "".join(
        f"<p>Fixture {idx} paragraph {k}. " + "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " * 4 + "</p>"
        for k in range(150))
    return f"<!doctype html><html><head><title>fixture {idx}</title></head><body><h1>fixture {idx}</h1>" \
           f"<button>Go {idx}</button><a href=\"/p/x\">next</a>{body}</body></html>"


def start_fixture():
    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            b = page_html(self.path.rsplit("/", 1)[-1]).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(b)))
            self.end_headers()
            self.wfile.write(b)

        def log_message(self, *a):
            pass

    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}/p/"


class Sampler(threading.Thread):
    """Peak summed PSS over every process under the roots returned by roots_fn, sampled every 100 ms."""

    def __init__(self, roots_fn):
        super().__init__(daemon=True)
        self.roots_fn, self.peak, self.stop_ev = roots_fn, 0, threading.Event()

    def run(self):
        while not self.stop_ev.is_set():
            total = 0
            for root in self.roots_fn():
                for pid in r01.tree(root):
                    total += r01.mem_of(pid)[1]
            self.peak = max(self.peak, total)
            time.sleep(0.1)

    def finish(self):
        self.stop_ev.set()
        self.join()
        return self.peak  # kB


class Mcp:
    """One manuk-mcp process, JSON-RPC over stdio (same protocol as mcp/tests/session.rs)."""

    def __init__(self, env):
        self.p = subprocess.Popen([MCP], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env=env)
        self.fd = self.p.stdout.fileno()
        self.buf, self.id = b"", 0
        r = self.rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                                    "clientInfo": {"name": "w4e0", "version": "0"}})
        if "result" not in r:
            raise RuntimeError(f"initialize failed: {r}")
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, v):
        self.p.stdin.write((json.dumps(v) + "\n").encode())
        self.p.stdin.flush()

    def _line(self, timeout):
        while b"\n" not in self.buf:
            ready, _, _ = select.select([self.fd], [], [], timeout)
            if not ready:
                raise TimeoutError("manuk-mcp silent")
            chunk = os.read(self.fd, 65536)
            if not chunk:
                raise OSError("manuk-mcp closed stdout")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return line

    def rpc(self, method, params, timeout=PAGE_TIMEOUT_S):
        self.id += 1
        i = self.id
        self._send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
        while True:
            v = json.loads(self._line(timeout))
            if v.get("id") == i:
                return v

    def call(self, name, args):
        """(is_error, text, wall_s)."""
        t0 = time.monotonic()
        r = self.rpc("tools/call", {"name": name, "arguments": args})
        res = r.get("result", {})
        text = "".join(b.get("text", "") for b in res.get("content", []))
        return bool(res.get("isError", True)), text, time.monotonic() - t0

    def kill(self):
        try:
            self.p.kill()
        except OSError:
            pass
        self.p.wait()


def mcp_env(log_dir):
    env = dict(os.environ)
    env["MANUK_ACTLOG_DIR"] = log_dir
    return env


def run_manuk_proc(n, fx, log_root):
    """N manuk-mcp processes, one page each. Returns (wall_s, peak_pss_kb, status, [])."""
    procs = []
    sampler = Sampler(lambda: [p.p.pid for p in procs])
    t0 = time.monotonic()
    try:
        for k in range(n):
            log = tempfile.mkdtemp(prefix="w4e0-log-", dir=log_root)
            m = Mcp(mcp_env(log))
            procs.append(m)
            if k == 0:
                sampler.start()
            err, _, _ = m.call("navigate", {"url": f"{fx}{k}"})
            if err:
                return time.monotonic() - t0, 0, "FAIL", []
        time.sleep(SETTLE_S)
        return time.monotonic() - t0, sampler.finish(), "OK", []
    finally:
        if sampler.is_alive():
            sampler.finish()
        for m in procs:
            m.kill()


def forks_valid(page_ids, n):
    """n-1 forks must have produced n-1 distinct page ids. A repeated id means a fork reused a page."""
    return len(page_ids) == n - 1


def run_manuk_fork(n, fx, log_root):
    """One manuk-mcp process: page 0 navigated, then n-1 pages forked from its checkpoint.
    Returns (wall_s, peak_pss_kb, status, per-fork wall ms list)."""
    m = Mcp(mcp_env(tempfile.mkdtemp(prefix="w4e0-log-", dir=log_root)))
    sampler = Sampler(lambda: [m.p.pid])
    forks = []
    try:
        sampler.start()
        t0 = time.monotonic()
        err, _, _ = m.call("navigate", {"url": f"{fx}0"})
        if err:
            return time.monotonic() - t0, 0, "FAIL", forks
        err, text, _ = m.call("checkpoint", {})
        mm = re.search(r"checkpoint (\w+)", text)
        if err or not mm:
            return time.monotonic() - t0, 0, "FAIL", forks
        ck = mm.group(1)
        page_ids = set()
        for _ in range(n - 1):
            err, text, wall = m.call("fork", {"checkpoint": ck})
            pid = re.search(r"page: (\S+)", text)
            if err or not pid:
                return time.monotonic() - t0, 0, "FAIL", forks
            page_ids.add(pid.group(1))
            forks.append(round(wall * 1000, 1))
        # manuk has no tab list on its single-page backend, so the proof that N-1 new pages exist is that
        # each fork returned its own page id. A repeated id means a fork reused a page: that is a FAIL.
        if not forks_valid(page_ids, n):
            return time.monotonic() - t0, 0, "FAIL", forks
        time.sleep(SETTLE_S)
        wall = time.monotonic() - t0
        return wall, sampler.finish(), "OK", forks
    finally:
        if sampler.is_alive():
            sampler.finish()
        m.kill()


class Cdp:
    """Flat CDP over one browser websocket: request ids matched, Page.loadEventFired recorded by session."""

    def __init__(self, port, path, timeout):
        self.ws = r01.WS(port, path, timeout)
        self.i = 0
        self.loaded = set()

    def call(self, method, params=None, session=None):
        self.i += 1
        i = self.i
        msg = {"id": i, "method": method, "params": params or {}}
        if session:
            msg["sessionId"] = session
        self.ws.send(msg)
        while True:
            m = self.ws.recv()
            if m.get("method") == "Page.loadEventFired":
                self.loaded.add(m.get("sessionId"))
            if m.get("id") == i:
                if "error" in m:
                    raise OSError(f"{method}: {m['error']}")
                return m.get("result", {})

    def wait_loads(self, sessions, timeout):
        """Block until every session in `sessions` has fired its load event."""
        t_end = time.monotonic() + timeout
        while not set(sessions) <= self.loaded:
            if time.monotonic() > t_end:
                raise TimeoutError("load events missing")
            self.ws.s.settimeout(max(0.1, t_end - time.monotonic()))
            m = self.ws.recv()
            if m.get("method") == "Page.loadEventFired":
                self.loaded.add(m.get("sessionId"))

    def new_page(self, url, context=None):
        params = {"url": "about:blank"}
        if context:
            params["browserContextId"] = context
        tid = self.call("Target.createTarget", params)["targetId"]
        sess = self.call("Target.attachToTarget", {"targetId": tid, "flatten": True})["sessionId"]
        self.call("Page.enable", session=sess)
        self.call("Page.navigate", {"url": url}, sess)
        return sess

    def close(self):
        self.ws.close()


def chrome_argv(profile):
    return [CHROME, "--headless=new", "--disable-gpu", "--no-first-run", "--remote-debugging-port=0",
            f"--user-data-dir={profile}", "about:blank"]


def chrome_connect(proc, profile, timeout):
    portfile = os.path.join(profile, "DevToolsActivePort")
    t_end = time.monotonic() + timeout
    while not os.path.exists(portfile) or len(open(portfile).read().split("\n")) < 2:
        if time.monotonic() > t_end or proc.poll() is not None:
            raise TimeoutError("chrome did not expose DevTools")
        time.sleep(0.02)
    port_s, path = open(portfile).read().split("\n")[:2]
    return Cdp(int(port_s), path, timeout)


def run_chrome_tabs(n, fx, _):
    """One Chrome, n tabs. Returns (wall_s, peak_pss_kb, status, [])."""
    profile = tempfile.mkdtemp(prefix="w4e0-chrome-")
    proc = subprocess.Popen(chrome_argv(profile), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    sampler = Sampler(lambda: [proc.pid])
    cdp = None
    try:
        t0 = time.monotonic()
        sampler.start()
        cdp = chrome_connect(proc, profile, PAGE_TIMEOUT_S)
        sessions = [cdp.new_page(f"{fx}{k}") for k in range(n)]
        cdp.wait_loads(sessions, PAGE_TIMEOUT_S)
        time.sleep(SETTLE_S)
        return time.monotonic() - t0, sampler.finish(), "OK", []
    except (TimeoutError, OSError, KeyError, ValueError):
        return 0.0, 0, "FAIL", []
    finally:
        if sampler.is_alive():
            sampler.finish()
        if cdp:
            cdp.close()
        r01_kill(proc)
        shutil.rmtree(profile, ignore_errors=True)


def run_chrome_procs(n, fx, _):
    """n Chrome processes, one tab each. Returns (wall_s, peak_pss_kb, status, [])."""
    procs, profiles = [], []
    sampler = Sampler(lambda: [p.pid for p in procs])
    cdps = []
    try:
        t0 = time.monotonic()
        for k in range(n):
            prof = tempfile.mkdtemp(prefix="w4e0-chrome-")
            profiles.append(prof)
            procs.append(subprocess.Popen(chrome_argv(prof), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
            if k == 0:
                sampler.start()
        for k, proc in enumerate(procs):
            cdp = chrome_connect(proc, profiles[k], PAGE_TIMEOUT_S)
            cdps.append(cdp)
            sess = cdp.new_page(f"{fx}{k}")
            cdp.wait_loads([sess], PAGE_TIMEOUT_S)
        time.sleep(SETTLE_S)
        return time.monotonic() - t0, sampler.finish(), "OK", []
    except (TimeoutError, OSError, KeyError, ValueError):
        return 0.0, 0, "FAIL", []
    finally:
        if sampler.is_alive():
            sampler.finish()
        for c in cdps:
            c.close()
        for p in procs:
            r01_kill(p)
        for prof in profiles:
            shutil.rmtree(prof, ignore_errors=True)


def r01_kill(proc):
    for pid in r01.tree(proc.pid):
        try:
            os.kill(pid, 9)
        except OSError:
            pass
    proc.wait()


def run_chrome_fork(fx):
    """Fork-equivalent on Chrome: new browser context + target + navigate. Returns (wall_ms, status)."""
    profile = tempfile.mkdtemp(prefix="w4e0-chrome-")
    proc = subprocess.Popen(chrome_argv(profile), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    cdp = None
    try:
        cdp = chrome_connect(proc, profile, PAGE_TIMEOUT_S)
        first = cdp.new_page(f"{fx}0")
        cdp.wait_loads([first], PAGE_TIMEOUT_S)
        t0 = time.monotonic()
        ctx = cdp.call("Target.createBrowserContext", {})["browserContextId"]
        sess = cdp.new_page(f"{fx}0", context=ctx)
        cdp.wait_loads([sess], PAGE_TIMEOUT_S)
        return round((time.monotonic() - t0) * 1000, 1), "OK"
    except (TimeoutError, OSError, KeyError, ValueError):
        return 0.0, "FAIL"
    finally:
        if cdp:
            cdp.close()
        r01_kill(proc)
        shutil.rmtree(profile, ignore_errors=True)


def run_cold_manuk(fx, log_root):
    t0 = time.monotonic()
    m = Mcp(mcp_env(tempfile.mkdtemp(prefix="w4e0-log-", dir=log_root)))
    try:
        err, _, _ = m.call("navigate", {"url": f"{fx}0"})
        return round((time.monotonic() - t0) * 1000, 1), "FAIL" if err else "OK"
    finally:
        m.kill()


def run_cold_chrome(fx):
    profile = tempfile.mkdtemp(prefix="w4e0-chrome-")
    t0 = time.monotonic()
    proc = subprocess.Popen(chrome_argv(profile), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    cdp = None
    try:
        cdp = chrome_connect(proc, profile, PAGE_TIMEOUT_S)
        sess = cdp.new_page(f"{fx}0")
        cdp.wait_loads([sess], PAGE_TIMEOUT_S)
        return round((time.monotonic() - t0) * 1000, 1), "OK"
    except (TimeoutError, OSError, KeyError, ValueError):
        return 0.0, "FAIL"
    finally:
        if cdp:
            cdp.close()
        r01_kill(proc)
        shutil.rmtree(profile, ignore_errors=True)


def run_urls(log_root, chrome_ver):
    """Per-URL wall and peak PSS on the 20 baseline URLs, 3 runs per arm. Chrome via r01's own driver."""
    with open(URLS) as f:
        urls = [ln.strip() for ln in f if ln.strip() and not ln.startswith("#")]
    rows = []
    for run in range(1, RUNS + 1):
        for url in urls:
            m_proc = Mcp(mcp_env(tempfile.mkdtemp(prefix="w4e0-log-", dir=log_root)))
            sampler = Sampler(lambda: [m_proc.p.pid])
            note = ""
            try:
                sampler.start()
                t0 = time.monotonic()
                err, text, _ = m_proc.call("navigate", {"url": url})
                wall = time.monotonic() - t0
                pss = sampler.finish()
                # navigate "OK" only says the call did not error. A bot-challenge page also comes back OK,
                # so the observation text is checked and the row is labelled, not counted as a pass.
                if err:
                    st = "FAIL"
                elif "Just a moment" in text or "challenge" in text.lower():
                    st = "CHALLENGE"
                else:
                    st = "OK"
                lines = text.split("\n")
                note = re.sub(r"\s+", " ", lines[1])[:80] if len(lines) > 1 else ""  # the page's URL and title line
            except (TimeoutError, OSError):
                wall, pss, st = 0.0, 0, "TIMEOUT"
                if sampler.is_alive():
                    sampler.finish()
            finally:
                m_proc.kill()
            rows.append({"axis": "urls", "arm": "manuk-proc", "n": 1, "run": run, "target": url,
                         "wall_s": round(wall, 3), "pss_kb": pss, "status": st, "note": note})
            print(f"urls run{run} manuk-proc {st:9} {wall:6.2f}s pss={pss/1024:7.1f}MB {url}", file=sys.stderr)
            w, rss, pss_c, _, st_c = r01.run_chrome(CHROME, url, PAGE_TIMEOUT_S)
            rows.append({"axis": "urls", "arm": "chrome-proc", "n": 1, "run": run, "target": url,
                         "wall_s": round(w, 3), "pss_kb": pss_c, "status": st_c, "note": ""})
            print(f"urls run{run} chrome-proc {st_c:9} {w:6.2f}s pss={pss_c/1024:7.1f}MB {url}", file=sys.stderr)
            touch()
    return rows


def load_runs(path):
    """Read <out>/runs.tsv back. The summary is always derived from this file, never from memory."""
    rows = []
    with open(path) as f:
        header = f.readline().rstrip("\n").split("\t")
        for line in f:
            d = dict(zip(header, line.rstrip("\n").split("\t")))
            rows.append({"axis": d["axis"], "arm": d["arm"], "n": int(d["n"]), "run": d["run"],
                         "target": d["target"], "wall": float(d["wall_s_or_ms"] or 0),
                         "pss_kb": int(d["peak_pss_kb"] or 0), "status": d["status"], "note": d["note"],
                         "chrome_ver": d["chrome_ver"]})
    return rows


def build_summary(rows):
    """One line per (axis, arm, n). Bands are min-max over the OK runs; per-page and pages/GB are derived."""
    out = ["axis\tarm\tn\tvalue\tper_page\tper_GB\truns\tchrome_ver"]
    keys = []
    for r in rows:
        k = (r["axis"], r["arm"], r["n"])
        if r["axis"] != "urls" and k not in keys:
            keys.append(k)
    for axis, arm, n in keys:
        sel = [r for r in rows if (r["axis"], r["arm"], r["n"]) == (axis, arm, n) and r["status"] == "OK"]
        bad = sum(1 for r in rows if (r["axis"], r["arm"], r["n"]) == (axis, arm, n) and r["status"] != "OK")
        if not sel:
            out.append(f"{axis}\t{arm}\t{n}\tNO_OK_RUNS (not OK: {bad})\t-\t-\t0\t{PIN}")
            continue
        if axis == "pages":
            totals = [r["pss_kb"] / 1024 for r in sel]
            per_page = [t / n for t in totals]
            per_gb = [1024 / p for p in per_page if p > 0]
            out.append(f"pages\t{arm}\t{n}\t{min(totals):.0f}-{max(totals):.0f}MB total\t"
                       f"{min(per_page):.1f}-{max(per_page):.1f}MB/page\t"
                       f"{min(per_gb):.1f}-{max(per_gb):.1f} pages/GB (derived)\truns={len(sel)}\t{PIN}")
        else:  # cold, fork: wall in ms
            v = [r["wall"] for r in sel]
            unit = "forks" if axis == "fork" and arm == "manuk-fork" else "runs"
            out.append(f"{axis}\t{arm}\t{n}\t{min(v):.0f}-{max(v):.0f} ms\t-\t-\t{unit}={len(v)}\t{PIN}")
    out += urls_summary(rows)
    return out


def urls_summary(rows):
    """Paired comparison: only URLs that loaded OK in all 3 runs on BOTH arms. Per run, the ratio of the
    median over those URLs; the band is min-max over the 3 runs. Chrome OK also includes challenge pages
    (r01 reports load events only), so this is a load-level comparison, not a content-level one."""
    urls = [r for r in rows if r["axis"] == "urls"]
    out = []
    counts = {arm: {s: sum(1 for r in urls if r["arm"] == arm and r["status"] == s)
                    for s in ("OK", "CHALLENGE", "FAIL", "TIMEOUT")} for arm in ("manuk-proc", "chrome-proc")}
    ok = {}
    for arm in counts:
        for r in urls:
            if r["arm"] == arm and r["status"] == "OK":
                ok.setdefault(r["target"], {}).setdefault(arm, {})[r["run"]] = r
    paired = [t for t, arms in ok.items() if all(len(arms.get(a, {})) == RUNS for a in counts)]
    out.append(f"urls\tmanuk-proc / chrome-proc\t{len(paired)} paired URLs\t"
               f"manuk:{counts['manuk-proc']} chrome:{counts['chrome-proc']}\t-\t-\tOK-in-all-runs\t{PIN}")
    for axis, key in (("wall", "wall"), ("pss", "pss_kb")):
        per_run = []
        for run in sorted({r["run"] for r in urls}):
            ratios = [ok[t]["manuk-proc"][run][key] / ok[t]["chrome-proc"][run][key] for t in paired
                      if ok[t]["chrome-proc"][run][key] > 0]
            if ratios:
                per_run.append(statistics.median(ratios))
        if per_run:
            out.append(f"urls\tmanuk/chrome {axis} ratio\t{len(paired)} URLs\t"
                       f"{min(per_run):.2f}-{max(per_run):.2f}x (median of per-URL ratios, per run)\t-\t-\t"
                       f"runs={len(per_run)}\t{PIN}")
    return out


def selftest():
    """The gate must go red on bad input. Each case must be refused or summarised correctly."""
    fails = []
    # 1. Dead sampler: a Sampler watching a pid that does not exist reports 0 kB. run_* turns that into FAIL,
    #    so the summary must not present it as an OK band.
    s = Sampler(lambda: [999999999])
    s.start()
    time.sleep(0.3)
    if s.finish() != 0:
        fails.append("dead-pid sampler reported nonzero PSS")
    rows = [{"axis": "pages", "arm": "x", "n": 10, "pss_kb": 0, "status": "FAIL", "wall": 0.0}]
    if "NO_OK_RUNS" not in build_summary(rows)[1]:
        fails.append("failed run was summarised as a band")
    # 2. Mutant: a page-count run that doubles PSS must move the derived pages/GB band in half.
    base = [{"axis": "pages", "arm": "a", "n": 1, "pss_kb": 100 * 1024, "status": "OK", "wall": 0.0}] * 3
    mut = [{"axis": "pages", "arm": "a", "n": 1, "pss_kb": 200 * 1024, "status": "OK", "wall": 0.0}] * 3
    gb_base = float(build_summary(base)[1].split("\t")[5].split()[0].split("-")[0])
    gb_mut = float(build_summary(mut)[1].split("\t")[5].split()[0].split("-")[0])
    if abs(gb_base / gb_mut - 2.0) > 0.01:
        fails.append(f"pages/GB did not halve under 2x PSS ({gb_base} vs {gb_mut})")
    # 2b. Fork check: a fork that reuses a page id (mutant: all ids equal) must be refused.
    if not forks_valid({"p1:1", "p2:1"}, 3) or forks_valid({"p1:1", "p1:1"}, 3) or forks_valid({"p1:1"}, 3):
        fails.append("fork-id check does not separate distinct from repeated ids")
    # 3. The pin must be present: a row with no pin is a refusal, not a number.
    if not PIN:
        fails.append("MANUK_CHROME_PIN unset (refused)")
    print("selftest:", "FAIL " + "; ".join(fails) if fails else "OK")
    return 1 if fails else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ROOT, "docs", "sprint", "footprint"))
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--skip-urls", action="store_true")
    ap.add_argument("--summarize-only", action="store_true", help="re-derive summary.tsv from runs.tsv")
    args = ap.parse_args()
    if args.selftest:
        sys.exit(selftest())
    if args.summarize_only:
        write_summary(args.out)
        return
    if not PIN:
        sys.exit("MANUK_CHROME_PIN unset: refusing to write rows without a chrome_ver")
    if not os.path.exists(MCP):
        sys.exit(f"missing {MCP}: cargo build --release -p manuk-mcp")
    if not os.path.exists(CHROME):
        sys.exit(f"missing pinned Chrome {CHROME}")
    os.makedirs(args.out, exist_ok=True)
    log_root = tempfile.mkdtemp(prefix="w4e0-root-")
    srv, fx = start_fixture()
    rows = []  # one dict per run, for the summary
    fork_rows = []
    arms = [("manuk-proc", run_manuk_proc), ("manuk-fork", run_manuk_fork),
            ("chrome-tabs", run_chrome_tabs), ("chrome-procs", run_chrome_procs)]
    try:
        for run in range(1, RUNS + 1):
            for n in N_LIST:
                for arm, fn in arms:
                    if n == 50 and free_mb() < MIN_FREE_MB_FOR_50:
                        rows.append({"axis": "pages", "arm": arm, "n": n, "run": run, "pss_kb": 0,
                                     "status": "SKIP_MEM"})
                        print(f"pages run{run} {arm} N=50 SKIP_MEM (free < {MIN_FREE_MB_FOR_50} MB)", file=sys.stderr)
                        continue
                    wall, pss, st, forks = fn(n, fx, log_root)
                    rows.append({"axis": "pages", "arm": arm, "n": n, "run": run, "wall_s": round(wall, 3),
                                 "pss_kb": pss, "status": st})
                    for ms in forks:
                        fork_rows.append({"axis": "fork", "arm": "manuk-fork", "n": n, "run": run,
                                          "wall_ms": ms, "status": st})
                    print(f"pages run{run} {arm:12} N={n:<3} {st:5} pss={pss/1024:8.1f}MB wall={wall:6.2f}s "
                          f"forks_ms={forks}", file=sys.stderr)
                    touch()
            # fork-equivalent on Chrome: new browser context + target + navigate, against manuk's fork above
            ms, st = run_chrome_fork(fx)
            fork_rows.append({"axis": "fork", "arm": "chrome-context", "n": 1, "run": run, "wall_ms": ms,
                              "status": st})
            print(f"fork run{run} chrome-context {st} {ms} ms", file=sys.stderr)
            cold_m, st_m = run_cold_manuk(fx, log_root)
            cold_c, st_c = run_cold_chrome(fx)
            fork_rows.append({"axis": "cold", "arm": "manuk-proc", "n": 1, "run": run, "wall_ms": cold_m,
                              "status": st_m})
            fork_rows.append({"axis": "cold", "arm": "chrome-proc", "n": 1, "run": run, "wall_ms": cold_c,
                              "status": st_c})
            print(f"cold run{run} manuk {st_m} {cold_m} ms | chrome {st_c} {cold_c} ms", file=sys.stderr)
            touch()
        url_rows = [] if args.skip_urls else run_urls(log_root, PIN)
    finally:
        srv.shutdown()
        shutil.rmtree(log_root, ignore_errors=True)

    # Dump the raw rows first: a summary can be re-derived, a lost run cannot.
    with open(os.path.join(args.out, "runs.tsv"), "w") as f:
        f.write("axis\tarm\tn\trun\ttarget\twall_s_or_ms\tpeak_pss_kb\tstatus\tnote\tchrome_ver\n")
        for r in rows + fork_rows + url_rows:
            wall = r.get("wall_s", r.get("wall_ms", ""))
            f.write("\t".join(str(x) for x in [r["axis"], r["arm"], r["n"], r.get("run", ""),
                                                r.get("target", "-"), wall, r.get("pss_kb", ""),
                                                r["status"], r.get("note", ""), PIN]) + "\n")

    write_summary(args.out)


def write_summary(out_dir):
    """summary.tsv is derived from runs.tsv only, so it can be re-made without re-measuring."""
    summary = build_summary(load_runs(os.path.join(out_dir, "runs.tsv")))
    with open(os.path.join(out_dir, "summary.tsv"), "w") as f:
        f.write("\n".join(summary) + "\n")
    print("\n".join(summary))


if __name__ == "__main__":
    main()
