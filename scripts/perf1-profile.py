#!/usr/bin/env python3
"""PERF.1 turnaround profile: where does a manuk page load spend its seconds, vs Chrome?

manuk arm : target/release/a11y-dump with MANUK_NET_TRACE=<file> (engine/net/src/nettrace.rs). The
            trace carries dns / connect(+TLS) / req / body / cascade / layout / phase / mark lines.
Chrome arm: headless Chrome driven over CDP (Network.* timing + Performance.getMetrics).

Both arms are reduced to the same nine phases and a wall time:
  dns, connect_tls, doc_fetch, subres_fetch, parse, cascade, layout, script, settle.

Attribution is EXCLUSIVE and by priority, so the phases of one load add up to its load time:
  manuk : every microsecond of [first request, navigate returned] goes to the highest-priority thing
          active in it: cascade > layout > dns > connect_tls > doc_fetch > subres_fetch; time that
          none of those covers falls to the engine's own load-phase it sits in (html parse -> parse;
          the script phases -> script; everything else -> settle); time no phase covers is
          `unattributed`. Plus startup (process spawn -> first request), a11y (tree build, a11y-dump
          only) and teardown, measured from independent clocks, so the whole sums to process wall.
  Chrome: cascade/layout/script are Performance.getMetrics RecalcStyleDuration/LayoutDuration/
          ScriptDuration; the network phases are the union-by-priority of the Network.* intervals,
          shrunk by the CPU fraction (assumes CPU and network overlap independently - a stated
          approximation, Chrome gives totals not intervals); parse = main-thread TaskDuration minus
          the three above; settle = what is left of navigation->load.

Usage (run with -I):
  python3 -I scripts/perf1-profile.py --runs 3 --out docs/sprint/perf1
  python3 -I scripts/perf1-profile.py --self-test
"""
import argparse
import importlib.util
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
PHASES = ["dns", "connect_tls", "doc_fetch", "subres_fetch", "parse", "cascade", "layout", "script", "settle"]
PRIORITY = ["cascade", "layout", "dns", "connect_tls", "doc_fetch", "subres_fetch"]
PHASE_FALLBACK = {"html parse": "parse", "external scripts": "script",
                  "cascade+layout+blocking scripts": "script", "deferred scripts": "script"}


def _r01():
    spec = importlib.util.spec_from_file_location("r01", os.path.join(HERE, "r01-baseline.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ---------------------------------------------------------------- manuk trace -> phases

def parse_trace(text):
    ev = []
    for ln in text.splitlines():
        f = ln.split("\t")
        if len(f) < 2:
            continue
        try:
            ev.append((int(f[0]), f[1], f[2:]))
        except ValueError:
            continue
    return ev


def max_concurrency(intervals):
    pts = []
    for s, e in intervals:
        pts.append((s, 1))
        pts.append((e, -1))
    cur = best = 0
    for _, d in sorted(pts, key=lambda p: (p[0], p[1])):
        cur += d
        best = max(best, cur)
    return best


def manuk_phases(ev, wall_s, wall0_unix_us, end_unix_us):
    """Partition one manuk run. Returns dict(phases_s, stats, check) or None if the trace is unusable."""
    epoch = next((int(a[0]) for t, k, a in ev if k == "epoch"), None)
    t_nav = next((t for t, k, a in ev if k == "mark" and a[0] == "navigate_returned"), None)
    t_a11y = next((t for t, k, a in ev if k == "mark" and a[0] == "a11y_built"), None)
    if epoch is None or t_nav is None or t_a11y is None:
        return None
    reqs = []  # (url, start, ttfb, status, version, clen)
    bodies = {}  # url -> list of (t_end, bytes)
    iv = {c: [] for c in PRIORITY}
    phase_iv = []
    connects = 0
    for t, k, a in ev:
        if k == "req":
            reqs.append((a[0], int(a[1]), int(a[2]), a[3], a[4], a[5]))
        elif k == "body":
            bodies.setdefault(a[0], []).append((t, int(a[1])))
        elif k == "dns":
            iv["dns"].append((t - int(a[1]), t))
        elif k == "connect":
            connects += 1
            iv["connect_tls"].append((t - int(a[1]), t))
        elif k == "phase":
            phase_iv.append((t - int(a[1]), t, a[0]))
    # cascade / layout events carry only a duration; they end at their own timestamp
    for t, k, a in ev:
        if k in ("cascade", "layout"):
            iv[k].append((t - int(a[0]), t))
    req_iv = []
    for i, (url, st, ttfb, status, ver, clen) in enumerate(reqs):
        end = st + ttfb
        for (te, _) in bodies.get(url, []):
            if te >= st + ttfb:
                end = te
                break
        req_iv.append((st, end))
    if reqs:
        first = min(range(len(reqs)), key=lambda i: reqs[i][1])
        iv["doc_fetch"].append(req_iv[first])
        iv["subres_fetch"] = [r for i, r in enumerate(req_iv) if i != first]
    lo, hi = 0, t_nav
    bounds = {lo, hi}
    for c in PRIORITY:
        for s, e in iv[c]:
            bounds.update((max(lo, min(hi, s)), max(lo, min(hi, e))))
    for s, e, _ in phase_iv:
        bounds.update((max(lo, min(hi, s)), max(lo, min(hi, e))))
    bs = sorted(bounds)
    last_phase_end = max((e for _, e, _ in phase_iv), default=0)
    t_load_event = next((t for t, k, a in ev if k == "phase" and a[0] == "load event"), t_nav)
    acc = {p: 0 for p in PHASES}
    unattributed = 0
    unattr_spans = []
    settle_by = Counter()
    for a_, b_ in zip(bs, bs[1:]):
        mid = (a_ + b_) / 2
        cat = None
        for c in PRIORITY:
            if any(s <= mid < e for s, e in iv[c]):
                cat = c
                break
        if cat is None:
            name = next((n for s, e, n in phase_iv if s <= mid < e), None)
            if name is None and mid >= last_phase_end:
                # after the engine's own `load event` phase and still inside navigate(): the agent's
                # settle wait (late scripts, reflows, network idle) - the spec's `settle` phase
                name = "post-load settle"
                cat = "settle"
            elif name is None:
                unattributed += b_ - a_
                unattr_spans.append((a_, b_))
                cat = "settle"
                name = "(no phase)"
            else:
                cat = PHASE_FALLBACK.get(name, "settle")
            if cat == "settle":
                settle_by[name] += b_ - a_
        acc[cat] += b_ - a_
    us = 1e-6
    startup = max(0.0, (epoch - wall0_unix_us) * us)
    a11y = (t_a11y - t_nav) * us
    teardown = max(0.0, (end_unix_us - (epoch + t_a11y)) * us)
    load_s = t_nav * us
    total = startup + load_s + a11y + teardown
    sub_bytes = 0
    for i, (url, *_r) in enumerate(reqs):
        if i == (min(range(len(reqs)), key=lambda j: reqs[j][1]) if reqs else -1):
            continue
        b = bodies.get(url)
        sub_bytes += b[0][1] if b else 0
    n_sub = max(0, len(reqs) - 1)
    stats = {
        "requests": len(reqs), "subresources": n_sub, "subres_bytes": sub_bytes,
        "max_concurrency": max_concurrency(iv["subres_fetch"] + iv["doc_fetch"]),
        "http_versions": dict(Counter(r[4] for r in reqs)),
        "new_connections": connects, "reused_requests": max(0, len(reqs) - connects),
        "status_304": sum(1 for r in reqs if r[3] == "304"),
    }
    return {
        "load_s": load_s,
        "to_load_event_s": t_load_event * us,
        "phases_s": {p: acc[p] * us for p in PHASES},
        "unattributed_s": unattributed * us,
        "unattributed_spans_us": unattr_spans[:6],
        "settle_by_phase_s": {k: v * us for k, v in settle_by.items()},
        "startup_s": startup, "a11y_s": a11y, "teardown_s": teardown,
        "sum_s": total, "wall_s": wall_s,
        "sum_vs_wall": abs(total - wall_s) / wall_s if wall_s else 1.0,
        "stats": stats,
    }


def run_manuk(binary, url, timeout, trace_dir=None, run=0):
    tf = tempfile.NamedTemporaryFile(prefix="perf1-trace-", suffix=".tsv", delete=False)
    tf.close()
    env = dict(os.environ, MANUK_NET_TRACE=tf.name)
    t0 = time.monotonic()
    u0 = int(time.time() * 1e6)
    try:
        p = subprocess.run([binary, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           env=env, timeout=timeout)
        rc, status = p.returncode, "OK" if p.returncode == 0 else "FAIL"
    except subprocess.TimeoutExpired:
        rc, status = -1, "TIMEOUT"
    wall = time.monotonic() - t0
    u1 = int(time.time() * 1e6)
    text = open(tf.name).read()
    if trace_dir:
        os.makedirs(trace_dir, exist_ok=True)
        slug = "".join(ch if ch.isalnum() else "_" for ch in url)[:80]
        shutil.copy(tf.name, os.path.join(trace_dir, f"{slug}.run{run}.tsv"))
    os.unlink(tf.name)
    out = {"engine": "manuk", "url": url, "status": status, "rc": rc, "wall_s": wall}
    if status == "OK":
        res = manuk_phases(parse_trace(text), wall, u0, u1)
        if res is None:
            out["status"] = "NOTRACE"
        else:
            out.update(res)
    return out


# ---------------------------------------------------------------- chrome (CDP) -> phases

def chrome_phases(events, metrics, t_start, t_load):
    """events: [(method, params)] collected in order; metrics: {name: seconds}. Times are CDP monotonic s."""
    nav = max(1e-9, t_load - t_start)
    reqs = {}
    for m, p in events:
        if m == "Network.requestWillBeSent":
            reqs.setdefault(p["requestId"], {"start": p["timestamp"], "type": p.get("type"),
                                             "url": p["request"]["url"]})
            if p.get("redirectResponse"):
                reqs[p["requestId"]]["redirected"] = True
        elif m == "Network.responseReceived":
            r = reqs.setdefault(p["requestId"], {"start": p["timestamp"], "type": p.get("type"),
                                                 "url": p["response"]["url"]})
            r["timing"] = p["response"].get("timing")
            r["proto"] = p["response"].get("protocol", "?")
            r["reused"] = p["response"].get("connectionReused")
            r["fromcache"] = bool(p["response"].get("fromDiskCache"))
        elif m == "Network.loadingFinished":
            r = reqs.setdefault(p["requestId"], {"start": p["timestamp"], "type": None, "url": "?"})
            r["end"] = p["timestamp"]
            r["bytes"] = p.get("encodedDataLength", 0)
        elif m == "Network.loadingFailed":
            r = reqs.setdefault(p["requestId"], {"start": p["timestamp"], "type": None, "url": "?"})
            r["end"] = p["timestamp"]
            r["failed"] = True
    iv = {c: [] for c in PRIORITY if c not in ("cascade", "layout")}
    docs = [r for r in reqs.values() if r.get("type") == "Document" and "end" in r]
    doc = min(docs, key=lambda r: r["start"]) if docs else None
    sub_iv, conns, bytes_, protos, reused = [], 0, 0, Counter(), 0
    for r in reqs.values():
        if "end" not in r or r["end"] > t_load + 0.05:
            continue
        s, e = r["start"], r["end"]
        (iv["doc_fetch"] if r is doc else iv["subres_fetch"]).append((s, e))
        if r is not doc:
            sub_iv.append((s, e))
            bytes_ += r.get("bytes", 0) or 0
        protos[r.get("proto", "?")] += 1
        tm = r.get("timing")
        if tm and r.get("reused") is False and not r.get("fromcache"):
            conns += 1
        if r.get("reused"):
            reused += 1
        if tm:
            base = tm["requestTime"]
            if tm.get("dnsStart", -1) >= 0 and tm.get("dnsEnd", -1) >= tm["dnsStart"]:
                iv["dns"].append((base + tm["dnsStart"] / 1e3, base + tm["dnsEnd"] / 1e3))
            if tm.get("connectStart", -1) >= 0 and tm.get("connectEnd", -1) >= tm["connectStart"]:
                iv["connect_tls"].append((base + tm["connectStart"] / 1e3, base + tm["connectEnd"] / 1e3))
    lo, hi = t_start, t_load
    bounds = {lo, hi}
    for c in iv:
        for s, e in iv[c]:
            bounds.update((max(lo, min(hi, s)), max(lo, min(hi, e))))
    bs = sorted(bounds)
    net = {c: 0.0 for c in iv}
    union = 0.0
    for a_, b_ in zip(bs, bs[1:]):
        mid = (a_ + b_) / 2
        for c in ["dns", "connect_tls", "doc_fetch", "subres_fetch"]:
            if any(s <= mid < e for s, e in iv[c]):
                net[c] += b_ - a_
                union += b_ - a_
                break
    cas = min(metrics.get("RecalcStyleDuration", 0.0), nav)
    lay = min(metrics.get("LayoutDuration", 0.0), nav - cas)
    scr = min(metrics.get("ScriptDuration", 0.0), nav - cas - lay)
    cpu = cas + lay + scr
    task = metrics.get("TaskDuration", 0.0)
    parse = max(0.0, min(task, nav) - cpu)
    idle_frac = max(0.0, 1.0 - (cpu + parse) / nav)
    ph = {p: 0.0 for p in PHASES}
    ph.update(cascade=cas, layout=lay, script=scr, parse=parse)
    for c in net:
        ph[c] = net[c] * idle_frac
    ph["settle"] = max(0.0, nav - sum(ph.values()))
    stats = {"requests": len([r for r in reqs.values() if "end" in r]), "subresources": len(sub_iv),
             "subres_bytes": bytes_, "max_concurrency": max_concurrency(sub_iv + iv["doc_fetch"]),
             "http_versions": dict(protos), "new_connections": conns, "reused_requests": reused}
    return {"load_s": nav, "phases_s": ph, "stats": stats}


def run_chrome(r01, chrome, url, timeout):
    profile = tempfile.mkdtemp(prefix="perf1-chrome-")
    t0 = time.monotonic()
    argv = [chrome, "--headless=new", "--disable-gpu", "--no-first-run", "--remote-debugging-port=0",
            f"--user-data-dir={profile}", "about:blank"]
    out = {"engine": "chrome", "url": url, "status": "TIMEOUT", "wall_s": 0.0}
    try:
        proc = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        out["status"] = "SPAWN"
        return out
    try:
        deadline = t0 + timeout
        pf = os.path.join(profile, "DevToolsActivePort")
        while not os.path.exists(pf) or len(open(pf).read().split("\n")) < 2:
            if time.monotonic() > deadline or proc.poll() is not None:
                raise TimeoutError
            time.sleep(0.02)
        port_s, path = open(pf).read().split("\n")[:2]
        ws = r01.WS(int(port_s), path, timeout)
        ids = iter(range(1, 1 << 30))
        events = []

        def call(method, params=None, session=None):
            i = next(ids)
            msg = {"id": i, "method": method, "params": params or {}}
            if session:
                msg["sessionId"] = session
            ws.send(msg)
            return i

        def wait(i):
            while True:
                if time.monotonic() > deadline:
                    raise TimeoutError
                ws.s.settimeout(max(0.1, deadline - time.monotonic()))
                m = ws.recv()
                if m.get("id") == i:
                    return m
                if "method" in m:
                    events.append((m["method"], m["params"]))

        tid = wait(call("Target.createTarget", {"url": "about:blank"}))["result"]["targetId"]
        sess = wait(call("Target.attachToTarget", {"targetId": tid, "flatten": True}))["result"]["sessionId"]
        for dom in ("Page.enable", "Network.enable", "Performance.enable"):
            wait(call(dom, session=sess))
        base = {m["name"]: m["value"] for m in
                wait(call("Performance.getMetrics", session=sess))["result"]["metrics"]}
        events.clear()
        nav = wait(call("Page.navigate", {"url": url}, sess))
        if "errorText" in nav.get("result", {}):
            out["status"] = "FAIL"
            raise ValueError
        t_load = None
        while t_load is None:
            if time.monotonic() > deadline:
                raise TimeoutError
            ws.s.settimeout(max(0.1, deadline - time.monotonic()))
            m = ws.recv()
            if "method" in m:
                events.append((m["method"], m["params"]))
                if m["method"] == "Page.loadEventFired":
                    t_load = m["params"]["timestamp"]
        got = {m["name"]: m["value"] for m in
               wait(call("Performance.getMetrics", session=sess))["result"]["metrics"]}
        metrics = {k: got.get(k, 0.0) - base.get(k, 0.0) for k in
                   ("RecalcStyleDuration", "LayoutDuration", "ScriptDuration", "TaskDuration")}
        first = next((p["timestamp"] for m, p in events if m == "Network.requestWillBeSent"
                      and p.get("type") == "Document"), None)
        if first is None:
            raise ValueError
        out.update(chrome_phases(events, metrics, first, t_load))
        out["status"] = "OK"
        ws.close()
    except (TimeoutError, OSError, KeyError, ValueError):
        pass
    finally:
        out["wall_s"] = time.monotonic() - t0
        for pid in r01.tree(proc.pid):
            try:
                os.kill(pid, 9)
            except OSError:
                pass
        proc.wait()
        shutil.rmtree(profile, ignore_errors=True)
    return out


# ---------------------------------------------------------------- report

def band(xs):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "-"
    return f"{statistics.median(xs):.2f} [{min(xs):.2f}-{max(xs):.2f}]"


def report(rows, chrome_ver, commit, urls, suites):
    L = []
    L.append("# PERF.1 turnaround profile\n")
    L.append(f"chrome_ver={chrome_ver} · manuk commit {commit} · 3-run bands are `median [min-max]` seconds. "
             "Generated by `scripts/perf1-profile.py`; raw rows in `docs/sprint/perf1/runs.jsonl`.\n")
    L.append("## Method\n\nBoth arms" + __doc__.split("Usage")[0].split("Both arms", 1)[1])
    by = {}
    for r in rows:
        by.setdefault((r["url"], r["engine"]), []).append(r)
    ok = lambda rs: [r for r in rs if r["status"] == "OK"]
    # ---- per-URL tables
    L.append("\n## Per-URL phase tables\n")
    shared = []
    for url in urls:
        m, c = ok(by.get((url, "manuk"), [])), ok(by.get((url, "chrome"), []))
        tag = ", ".join(suites.get(url, []))
        L.append(f"\n### {url}  ({tag})\n")
        L.append(f"manuk OK {len(m)}/3 · chrome OK {len(c)}/3\n")
        if not m and not c:
            continue
        L.append("| phase | manuk s | chrome s | gap s (manuk-chrome medians) |\n|---|---|---|---|")
        for p in PHASES + ["load_s"]:
            mv = [r["phases_s"][p] if p != "load_s" else r["load_s"] for r in m]
            cv = [r["phases_s"][p] if p != "load_s" else r["load_s"] for r in c]
            gap = f"{statistics.median(mv) - statistics.median(cv):+.2f}" if mv and cv else "-"
            L.append(f"| {p if p != 'load_s' else '**load (first request → load event)**'} | {band(mv)} | {band(cv)} | {gap} |")
        if m:
            L.append(f"| manuk time to its own `load event` phase end (the Chrome-comparable point) | {band([r['to_load_event_s'] for r in m])} | {band([r['load_s'] for r in c])} | |")
            L.append(f"| startup (spawn → first request) | {band([r['startup_s'] for r in m])} | - | |")
            L.append(f"| a11y tree build (a11y-dump only) | {band([r['a11y_s'] for r in m])} | - | |")
            L.append(f"| teardown | {band([r['teardown_s'] for r in m])} | - | |")
            L.append(f"| unattributed (inside load; no phase/net/cascade/layout covers it) | {band([r['unattributed_s'] for r in m])} | - | |")
            L.append(f"| **sum of manuk phases** | {band([r['sum_s'] for r in m])} | | |")
            L.append(f"| **measured process wall** | {band([r['wall_s'] for r in m])} | {band([r['wall_s'] for r in c])} (browser start incl.) | |")
            L.append(f"| sum vs wall (max over runs) | {max(r['sum_vs_wall'] for r in m) * 100:.1f}% | | |")
        for eng, rs in (("manuk", m), ("chrome", c)):
            if rs:
                s = rs[0]["stats"]
                L.append(f"\n{eng} run1 subresources: n={s['subresources']} bytes={s['subres_bytes']} "
                         f"max_concurrency={s['max_concurrency']} http={s['http_versions']} "
                         f"new_conns={s['new_connections']} reused_reqs={s['reused_requests']}")
        if m and c:
            shared.append(url)
    # ---- gap ranking
    L.append("\n## Phase ranking: seconds manuk loses to Chrome\n")
    L.append("Sum over URLs where BOTH engines loaded OK in >=1 run, of (median manuk phase − median Chrome phase). "
             "`gap s` counts each URL once; `task-weighted` counts a live-suite start URL once per task that starts "
             "there (L01-L25 share 9 URLs) and a baseline-only URL once.\n")
    tot = {p: 0.0 for p in PHASES}
    sum_m = {p: 0.0 for p in PHASES}
    sum_c = {p: 0.0 for p in PHASES}
    totw = {p: 0.0 for p in PHASES}
    load_gap = 0.0
    le_gap = 0.0
    settle_tot = Counter()
    for url in shared:
        m, c = ok(by[(url, "manuk")]), ok(by[(url, "chrome")])
        w = sum(1 for t in suites.get(url, []) if t.startswith("L")) or 1
        for p in PHASES:
            d = statistics.median(r["phases_s"][p] for r in m) - statistics.median(r["phases_s"][p] for r in c)
            tot[p] += d
            sum_m[p] += statistics.median(r["phases_s"][p] for r in m)
            sum_c[p] += statistics.median(r["phases_s"][p] for r in c)
            totw[p] += d * w
        load_gap += statistics.median(r["load_s"] for r in m) - statistics.median(r["load_s"] for r in c)
        le_gap += statistics.median(r["to_load_event_s"] for r in m) - statistics.median(r["load_s"] for r in c)
        for k in set().union(*(r["settle_by_phase_s"] for r in m)):
            settle_tot[k] += statistics.median(r["settle_by_phase_s"].get(k, 0.0) for r in m)
    pos = sum(v for v in tot.values() if v > 0) or 1e-9
    L.append(f"URLs in the ranking: {len(shared)}. Total load gap (Σ manuk navigate() − Σ chrome load event): {load_gap:+.2f} s, of which {le_gap:+.2f} s is "
             f"manuk's own load event vs Chrome's, the rest is the agent's post-load settle wait.\n")
    L.append("| rank | phase | gap s | share of positive gap | task-weighted gap s | Σ manuk s | Σ chrome s |\n|---|---|---|---|---|---|---|")
    ranked = sorted(PHASES, key=lambda p: -tot[p])
    owners = []
    for i, p in enumerate(ranked, 1):
        share = tot[p] / pos * 100 if tot[p] > 0 else 0.0
        if share >= 10:
            owners.append((p, tot[p], share))
        L.append(f"| {i} | {p} | {tot[p]:+.2f} | {share:.1f}% | {totw[p]:+.2f} | {sum_m[p]:.2f} | {sum_c[p]:.2f} |")
    L.append("\nWhere manuk's `settle` seconds sit (Σ over ranked URLs of per-URL median): " + ", ".join(f"{k} {v:.1f}s" for k, v in settle_tot.most_common()))
    L.append("\nPhases owning >=10% of the positive gap: " + (", ".join(f"{p} ({s:.0f}%)" for p, _, s in owners) or "none"))
    L.append("""
## Scope and caveats

- URLs where either engine never loaded (see the `OK n/3` lines) are excluded from the ranking; they are the reach problem (NET.REACH.1), not a phase.
- manuk's clock starts at the first request, Chrome's at the Document `requestWillBeSent`; Chrome's process wall (browser start included) is shown only for the R0.1 comparison.
- Chrome's cascade/layout/script are cumulative `Performance.getMetrics` totals and its network phases are shrunk by the CPU fraction (independence assumption), so a Chrome phase is accurate to the order of tens of ms, not a few. manuk's phases are exact interval attributions of one trace.
- manuk `settle` is the time inside `navigate()` after (and around) the engine's own `load event`: `finish_loading` phases, subframes, image/mask decode. Chrome's equivalent is ~0 because the CDP load event is the stop point; a Playwright `goto` also stops at the load event.
- Reproduce: `cargo build --release -p manuk-agent --bin a11y-dump && python3 -I scripts/perf1-profile.py --runs 3 --out docs/sprint/perf1` (needs `MANUK_CHROME_PIN`), `--self-test` for the instrument's own gate.
""")
    return "\n".join(L) + "\n", owners


def synth_trace(extra_cascade_us=0):
    """A synthetic load with known phases; `extra_cascade_us` stretches the cascade and shifts
    everything after it, exactly what a real sleep inside the cascade would do."""
    x = extra_cascade_us
    return "\n".join([
        "0\tepoch\t1000000",
        "50000\tdns\th\t50000",
        "100000\tconnect\th\t100000",
        "200000\treq\thttps://h/\t0\t200000\t200\tHTTP/2.0\t-",
        "220000\tbody\thttps://h/\t100",
        "225000\tphase\thtml parse\t5000",
        "300000\treq\thttps://h/a.css\t225000\t75000\t200\tHTTP/2.0\t-",
        "310000\tbody\thttps://h/a.css\t50",
        "310000\tphase\tblocking stylesheets\t85000",
        f"{340000 + x}\tcascade\t{30000 + x}",
        f"{390000 + x}\tlayout\t50000",
        f"{400000 + x}\tphase\tcascade+layout+blocking scripts\t{90000 + x}",
        f"{410000 + x}\tphase\tload event\t10000",
        f"{410000 + x}\tmark\tnavigate_returned",
        f"{420000 + x}\tmark\ta11y_built",
    ])


def self_test():
    """Gates: phases sum to load time; sum of process phases equals wall; 500 ms injected into the
    cascade moves the cascade bucket by 0.5 s and no other bucket."""
    def run(x):
        wall = 0.6 + x * 1e-6
        return manuk_phases(parse_trace(synth_trace(x)), wall, 900_000, 1_000_000 + 420_000 + x + 80_000)

    base, bumped = run(0), run(500_000)
    ph = base["phases_s"]
    assert base["sum_vs_wall"] < 0.01, base["sum_vs_wall"]
    assert abs(ph["dns"] - 0.05) < 1e-6 and abs(ph["connect_tls"] - 0.05) < 1e-6, ph
    assert abs(ph["cascade"] - 0.03) < 1e-6 and abs(ph["layout"] - 0.05) < 1e-6, ph
    assert abs(sum(ph.values()) - base["load_s"]) < 1e-6
    delta = {p: bumped["phases_s"][p] - ph[p] for p in PHASES}
    assert abs(delta["cascade"] - 0.5) < 1e-6, delta
    assert all(abs(v) < 1e-6 for p, v in delta.items() if p != "cascade"), delta
    # a trace that lost its a11y mark is rejected, not silently summed
    assert manuk_phases(parse_trace(synth_trace().replace("a11y_built", "x")), 0.6, 0, 0) is None
    # a wall that disagrees with the trace by 50% is visible in sum_vs_wall
    off = manuk_phases(parse_trace(synth_trace()), 1.2, 900_000, 1_000_000 + 420_000 + 80_000)
    assert off["sum_vs_wall"] > 0.4
    print("self-test OK: phases sum to load time, 500 ms injected into cascade shows in cascade only")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--out", default="docs/sprint/perf1")
    ap.add_argument("--manuk", default="target/release/a11y-dump")
    ap.add_argument("--chrome", default="google-chrome")
    ap.add_argument("--timeout", type=float, default=60.0)
    ap.add_argument("--urls", help="override: file of URLs (one per line), skips the suite lookup")
    ap.add_argument("--engines", default="manuk,chrome")
    ap.add_argument("--report-only", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("MANUK_CHROME_PIN unset: refusing to write rows without a chrome_ver")
    suites = {}
    urls = []
    if a.urls:
        urls = [l.strip() for l in open(a.urls) if l.strip()]
    else:
        for l in open("docs/sprint/BASELINE-URLS.txt"):
            if l.strip():
                urls.append(l.strip())
                suites.setdefault(l.strip(), []).append("R0.1")
        for t in json.load(open("bench/agent-tasks/suite-v1.json"))["tasks"]:
            if t["start_url"].startswith("https://") and t["id"].startswith("L"):
                if t["start_url"] not in urls:
                    urls.append(t["start_url"])
                suites.setdefault(t["start_url"], []).append(t["id"])
    os.makedirs(a.out, exist_ok=True)
    raw = os.path.join(a.out, "runs.jsonl")
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    engines = a.engines.split(",")
    if not a.report_only:
        r01 = _r01()
        with open(raw, "a") as f:
            for run in range(1, a.runs + 1):
                for url in urls:
                    for eng in engines:
                        r = run_manuk(a.manuk, url, a.timeout, os.path.join(a.out, 'traces'), run) if eng == "manuk" else run_chrome(r01, a.chrome, url, a.timeout)
                        r.update(run=run, chrome_ver=pin, commit=commit)
                        f.write(json.dumps(r) + "\n")
                        f.flush()
                        print(f"run{run} {eng:6} {r['status']:8} wall={r['wall_s']:6.2f}s "
                              f"load={r.get('load_s', 0):6.2f}s {url}", file=sys.stderr)
    latest = {}
    for l in open(raw):
        r = json.loads(l)
        if r.get("chrome_ver") == pin:
            latest[(r["run"], r["url"], r["engine"])] = r  # a re-measured arm replaces its older rows
    rows = list(latest.values())
    text, owners = report(rows, pin, commit, urls, suites)
    open(os.path.join(a.out, "PERF.md"), "w").write(text)
    print(text[-1500:])


if __name__ == "__main__":
    main()
