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
import base64
import json
import os
import socket
import struct
import threading
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


class WS:
    """Minimal RFC 6455 client (text frames, stdlib only): the box has no ws/websockets module."""

    def __init__(self, port, path, timeout):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall((f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n"
                        f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        buf = b""
        while b"\r\n\r\n" not in buf:
            chunk = self.s.recv(4096)
            if not chunk:
                raise OSError("ws handshake closed")
            buf += chunk
        if b" 101 " not in buf.split(b"\r\n", 1)[0]:
            raise OSError("ws handshake refused")
        self.rest = buf.split(b"\r\n\r\n", 1)[1]

    def _read(self, n):
        while len(self.rest) < n:
            chunk = self.s.recv(65536)
            if not chunk:
                raise OSError("ws closed")
            self.rest += chunk
        out, self.rest = self.rest[:n], self.rest[n:]
        return out

    def send(self, obj):
        data = json.dumps(obj).encode()
        hdr = bytes([0x81])
        n = len(data)
        if n < 126:
            hdr += bytes([0x80 | n])
        elif n < 65536:
            hdr += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            hdr += bytes([0x80 | 127]) + struct.pack(">Q", n)
        mask = os.urandom(4)
        self.s.sendall(hdr + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def recv(self):
        msg = b""
        while True:
            b0, b1 = self._read(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._read(8))[0]
            payload = self._read(n)
            op = b0 & 0x0F
            if op == 8:
                raise OSError("ws close frame")
            if op in (1, 0):
                msg += payload
                if b0 & 0x80:
                    return json.loads(msg)

    def close(self):
        self.s.close()


def cdp_load(ws, url, deadline):
    """Navigate a fresh tab and wait for the load event. Returns (status, dom_bytes)."""
    ids = iter(range(1, 1 << 30))

    def call(method, params=None, session=None):
        i = next(ids)
        msg = {"id": i, "method": method, "params": params or {}}
        if session:
            msg["sessionId"] = session
        ws.send(msg)
        return i

    def wait_result(i):
        while True:
            if time.monotonic() > deadline:
                raise TimeoutError
            ws.s.settimeout(max(0.1, deadline - time.monotonic()))
            m = ws.recv()
            if m.get("id") == i:
                return m

    tid = wait_result(call("Target.createTarget", {"url": "about:blank"}))["result"]["targetId"]
    sess = wait_result(call("Target.attachToTarget", {"targetId": tid, "flatten": True}))["result"]["sessionId"]
    wait_result(call("Page.enable", session=sess))
    nav = wait_result(call("Page.navigate", {"url": url}, sess))
    if "errorText" in nav.get("result", {}):
        return "FAIL", 0
    loaded = False
    while not loaded:
        if time.monotonic() > deadline:
            raise TimeoutError
        ws.s.settimeout(max(0.1, deadline - time.monotonic()))
        m = ws.recv()
        if m.get("method") == "Page.loadEventFired":
            loaded = True
    dom = wait_result(call("Runtime.evaluate", {"expression": "document.documentElement.outerHTML",
                                                "returnByValue": True}, sess))
    return "OK", len(dom.get("result", {}).get("result", {}).get("value", "") or "")


def run_chrome(chrome, url, timeout_s):
    """Headless Chrome with the sandbox ON, driven over CDP because --dump-dom never exits on this box.
    Returns the same tuple as run_measured; the whole browser process tree is what is sampled."""
    profile = tempfile.mkdtemp(prefix="r01-chrome-")
    t0 = time.monotonic()
    argv = [chrome, "--headless=new", "--disable-gpu", "--no-first-run", "--remote-debugging-port=0",
            f"--user-data-dir={profile}", "about:blank"]
    try:
        proc = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        shutil.rmtree(profile, ignore_errors=True)
        return 0.0, 0, 0, -1, "SPAWN"
    peaks = [0, 0]
    stop = threading.Event()

    def sampler():
        while not stop.is_set():
            rss_sum = pss_sum = 0
            for pid in tree(proc.pid):
                r, p = mem_of(pid)
                rss_sum += r
                pss_sum += p
            peaks[0] = max(peaks[0], rss_sum)
            peaks[1] = max(peaks[1], pss_sum)
            time.sleep(0.05)

    th = threading.Thread(target=sampler, daemon=True)
    th.start()
    status, rc = "TIMEOUT", -1
    wall = None
    try:
        deadline = t0 + timeout_s
        portfile = os.path.join(profile, "DevToolsActivePort")
        while not os.path.exists(portfile) or len(open(portfile).read().split("\n")) < 2:
            if time.monotonic() > deadline or proc.poll() is not None:
                raise TimeoutError
            time.sleep(0.02)
        port_s, path = open(portfile).read().split("\n")[:2]
        ws = WS(int(port_s), path, timeout_s)
        # wall includes browser startup, to match a cold process per URL like a11y-dump.
        status, _ = cdp_load(ws, url, deadline)
        wall = time.monotonic() - t0
        rc = 0 if status == "OK" else 1
        ws.close()
    except (TimeoutError, OSError, KeyError, ValueError):
        pass  # status stays TIMEOUT: a browser that never answered is not a page load
    finally:
        wall = wall if wall is not None else time.monotonic() - t0
        stop.set()
        th.join()
        for pid in tree(proc.pid):
            try:
                os.kill(pid, 9)
            except OSError:
                pass
        proc.wait()
        shutil.rmtree(profile, ignore_errors=True)
    return wall, peaks[0], peaks[1], rc, status


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
                    wall, rss, pss, rc, status = run_measured([args.manuk, url], args.timeout)
                else:
                    wall, rss, pss, rc, status = run_chrome(args.chrome, url, args.timeout)
                rows.append((run, url, engine, round(wall, 3), rss, pss, rc, status))
                print(f"run{run} {engine:6} {status:7} {wall:6.2f}s rss={rss/1024:7.1f}MB "
                      f"pss={pss/1024:7.1f}MB {url}", file=sys.stderr)

    out_path = os.path.join(args.out, "runs.tsv")
    kept = []  # rows of engines not re-run this time survive, so one arm can be re-measured alone
    if os.path.exists(out_path):
        with open(out_path) as f:
            kept = [ln for ln in f.read().splitlines()[1:] if ln.split("\t")[2] not in engines]
    with open(out_path, "w") as f:
        f.write("run\turl\tengine\twall_s\tpeak_rss_kb\tpeak_pss_kb\texit\tstatus\tchrome_ver\n")
        for ln in kept:
            f.write(ln + "\n")
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
