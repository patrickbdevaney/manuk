#!/usr/bin/env python3
"""W4.E1.b: attribute each URL's subres_fetch (perf1-profile.py's exclusive phase) to buckets, from MANUK_NET_TRACE files.

Buckets (exclusive, per time slice of subres_fetch, over the subresource requests active in the slice):
  discovered_late : no parse-time request is active; only requests that START more than LATE_US after the first
                    subresource request (the parse-time batch) (found by CSS url()/script/layout, not by the parse-time scan) are in flight.
  transfer        : some parse-time request is past its first byte (body streaming).
  server_ttfb     : else some parse-time request is waiting for its first byte (connect+TLS+server think time).
  cap_wait        : else every active parse-time request is queued behind the per-host connection cap
                    (HTTP/1.x host with >= CAP requests already in flight; ends when one finishes).
Usage (run with -I): w4e1b-attribute.py --out FILE TRACE...   |   --self-test
"""
import argparse, importlib.util, os, sys, statistics

HERE = os.path.dirname(os.path.abspath(__file__))
CAP = 6
LATE_US = 100_000
BUCKETS = ["cap_wait", "server_ttfb", "transfer", "discovered_late"]


def _pf():
    spec = importlib.util.spec_from_file_location("pf", os.path.join(HERE, "perf1-profile.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def host_of(u):
    return u.split("/")[2] if "//" in u else u


def attribute(pf, text, drop=None):
    ev = pf.parse_trace(text)
    t_nav = next(t for t, k, a in ev if k == "mark" and a[0] == "navigate_returned")
    reqs, bodies, iv = [], {}, {c: [] for c in pf.PRIORITY}
    for t, k, a in ev:
        if k == "req":
            reqs.append((a[0], int(a[1]), int(a[2]), a[4]))
        elif k == "body":
            bodies.setdefault(a[0], []).append(t)
        elif k == "dns":
            iv["dns"].append((t - int(a[1]), t))
        elif k == "connect":
            iv["connect_tls"].append((t - int(a[1]), t))
        elif k in ("cascade", "layout"):
            iv[k].append((t - int(a[0]), t))
    rows = []
    for url, st, ttfb, ver in reqs:
        end = st + ttfb
        for te in bodies.get(url, []):
            if te >= st + ttfb:
                end = te
                break
        rows.append(dict(url=url, host=host_of(url), st=st, hdr=st + ttfb, end=end, ver=ver))
    first = min(range(len(rows)), key=lambda i: rows[i]["st"])
    iv["doc_fetch"].append((rows[first]["st"], rows[first]["end"]))
    sub = [r for i, r in enumerate(rows) if i != first]
    first_sub = min((r["st"] for r in sub), default=0)
    for r in sub:
        r["wait_end"] = r["st"]
        if r["ver"].startswith("HTTP/1"):
            same = [o for o in sub if o["host"] == r["host"] and o is not r and o["st"] <= r["st"] < o["end"]]
            if len(same) >= CAP:
                r["wait_end"] = min(r["hdr"], sorted(o["end"] for o in same)[len(same) - CAP])
        r["late"] = r["st"] > first_sub + LATE_US
    higher = [c for c in pf.PRIORITY if c != "subres_fetch"]
    pts = {0, t_nav}
    for c in higher:
        for s, e in iv[c]:
            pts.update((max(0, min(t_nav, s)), max(0, min(t_nav, e))))
    for r in sub:
        pts.update(max(0, min(t_nav, x)) for x in (r["st"], r["wait_end"], r["hdr"], r["end"]))
    bs = sorted(pts)
    acc = {b: 0 for b in BUCKETS}
    late_in = {b: 0 for b in BUCKETS[:3]}
    for a_, b_ in zip(bs, bs[1:]):
        mid = (a_ + b_) / 2
        if any(s <= mid < e for c in higher for s, e in iv[c]):
            continue
        act = [r for r in sub if r["st"] <= mid < r["end"]]
        if not act:
            continue

        def state(r):
            return "cap_wait" if mid < r["wait_end"] else "server_ttfb" if mid < r["hdr"] else "transfer"
        early = [r for r in act if not r["late"]]
        if early:
            sts = {state(r) for r in early}
            b = "transfer" if "transfer" in sts else "server_ttfb" if "server_ttfb" in sts else "cap_wait"
        else:
            b = "discovered_late"
            sts = {state(r) for r in act}
            late_in["transfer" if "transfer" in sts else "server_ttfb" if "server_ttfb" in sts else "cap_wait"] += b_ - a_
        if b == drop:
            continue
        acc[b] += b_ - a_
    ref = pf.manuk_phases(ev, 1.0, 0, 0)
    return dict(late_in_s={b: v * 1e-6 for b, v in late_in.items()}, buckets_s={b: v * 1e-6 for b, v in acc.items()},
                subres_fetch_s=ref["phases_s"]["subres_fetch"] if ref else None,
                n_sub=len(sub), n_late=sum(r["late"] for r in sub),
                n_h1=sum(r["ver"].startswith("HTTP/1") for r in sub))


def self_test(pf):
    tr = "\n".join(["0\tepoch\t1", "100000\treq\thttp://x/\t0\t100000\t200\tHTTP/2.0\t-\tGET", "100000\tbody\thttp://x/\t1",
                    "300000\treq\thttp://x/a\t100000\t100000\t200\tHTTP/2.0\t-\tGET", "300000\tbody\thttp://x/a\t1",
                    "700000\treq\thttp://x/i\t500000\t100000\t200\tHTTP/2.0\t-\tGET", "700000\tbody\thttp://x/i\t1",
                    "800000\tmark\tnavigate_returned", "800000\tmark\ta11y_built"]) + "\n"
    r = attribute(pf, tr)
    b = r["buckets_s"]
    assert abs(b["server_ttfb"] - 0.1) < 1e-6 and abs(b["transfer"] - 0.1) < 1e-6 and abs(b["discovered_late"] - 0.2) < 1e-6, b
    d = attribute(pf, tr, drop="transfer")
    assert abs(sum(d["buckets_s"].values()) - d["subres_fetch_s"]) > 0.05, "mutation must break reconcile"
    print("self-test ok", b, "subres_fetch", r["subres_fetch_s"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("traces", nargs="*")
    ap.add_argument("--out")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--drop")
    a = ap.parse_args()
    pf = _pf()
    if a.self_test:
        return self_test(pf)
    res = {}
    for p in a.traces:
        url = os.path.basename(p).rsplit(".run", 1)[0]
        res.setdefault(url, []).append(attribute(pf, open(p).read(), a.drop))
    lines = ["url\truns\tsubres_fetch_s\tcap_wait_s\tserver_ttfb_s\ttransfer_s\tdiscovered_late_s\tbucket_sum_s\trecon_err_pct\tlate_cap_s\tlate_ttfb_s\tlate_transfer_s\tn_sub\tn_late\tn_http1\tchrome_ver"]
    ok = True
    for url, rs in res.items():
        sub = statistics.median(r["subres_fetch_s"] for r in rs)
        bs = {b: statistics.median(r["buckets_s"][b] for r in rs) for b in BUCKETS}
        tot = sum(bs.values())
        err = abs(tot - sub) / sub * 100 if sub else 0
        ok &= err <= 10
        lines.append("\t".join([url, str(len(rs)), f"{sub:.3f}"] + [f"{bs[b]:.3f}" for b in BUCKETS] +
                               [f"{tot:.3f}", f"{err:.1f}"] + [f"{statistics.median(r['late_in_s'][b] for r in rs):.3f}" for b in BUCKETS[:3]] + [str(rs[0]["n_sub"]), str(rs[0]["n_late"]), str(rs[0]["n_h1"]),
                                os.environ.get("MANUK_CHROME_PIN", "")]))
    out = "\n".join(lines) + "\n"
    print(out)
    if a.out:
        open(a.out, "w").write(out)
    sys.exit(0 if ok else 1)


main()
