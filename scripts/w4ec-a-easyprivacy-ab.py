#!/usr/bin/env python3
"""W4.E2.c.a: what does EasyPrivacy (feature `lean-easyprivacy`) add over the 40-host lean list?

Two lean_probe binaries, both run with MANUK_AGENT_PROFILE=lean:
  OFF arm = built WITHOUT feature lean-easyprivacy (only the hand-audited 40-host list refuses requests)
  ON  arm = built WITH    feature lean-easyprivacy (the pinned EasyPrivacy list refuses too)
Per URL and run, both arms run back to back, the first arm alternating by run to cancel drift.
Each probe run reports tracker refusals (`blocked`), body bytes off the wire (`wire_bytes`), wall time and
FNV digests of the observation (`a11y`, `links`).

Gate (must be able to go red):
  * build-applied: the ON binary must contain the pinned EasyPrivacy text (marker `||crazyegg.com^`) and the OFF
    binary must not. Otherwise the arms are not what they are labelled and the A/B is VOID (rc 2).
  * sample: a URL gets a band verdict only with >= 3 OK runs in both arms; fewer -> INSUFFICIENT (no verdict).
  * claim rule: a difference is claimed only when the two bands are disjoint (refusals or wire bytes).
  * digest: a page is self-stable when every OK run of an arm gives the same a11y digest. On a self-stable page
    the ON and OFF digests must agree; a disagreement is RED (rc 1): the extra refusals changed what the agent
    sees.

Exit codes: 0 settled, 1 RED (digest disagreement on a self-stable page, or self-test failure), 2 VOID/INCOMPLETE.

Usage:
  python3 -I scripts/w4ec-a-easyprivacy-ab.py run --off <bin> --on <bin> --urls <file> --out <dir> [--runs 3]
  python3 -I scripts/w4ec-a-easyprivacy-ab.py report --off <bin> --on <bin> --out <dir>      (re-analyse runs.tsv)
  python3 -I scripts/w4ec-a-easyprivacy-ab.py --self-test
"""
import argparse, os, re, statistics, subprocess, sys, tempfile
from urllib.parse import urlsplit

SLD = {"co", "com", "org", "net", "gov", "ac", "edu"}


def reg_domain(host):
    """Approximate registrable domain: last two labels, three when the TLD is 2 letters and the label before
    it is a generic second-level (co.uk, com.au). No public-suffix list: documented approximation."""
    h = host.lower().rstrip(".").split(".")
    if len(h) >= 3 and len(h[-1]) == 2 and h[-2] in SLD:
        return ".".join(h[-3:])
    return ".".join(h[-2:])


def refused_hosts(cell):
    return sorted({urlsplit(u).hostname or "" for u in cell.split(",") if u})


def over_blocked(page_url, hosts):
    """Refused hosts sharing a registrable domain with the page (first-party: over-blocking)."""
    d = reg_domain(urlsplit(page_url).hostname or "")
    return [h for h in hosts if h and reg_domain(h) == d]

MARKER = b"||crazyegg.com^"
COLS = ["url", "arm", "run", "status", "blocked", "wire_bytes", "wall_ms", "a11y", "links", "refused", "chrome_ver"]
CHROME = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")


def binary_has_marker(path):
    with open(path, "rb") as f:
        return MARKER in f.read()


def check_build(off_bin, on_bin):
    """Return None if the arms are what they are labelled, else a reason string (-> VOID)."""
    if not binary_has_marker(on_bin):
        return "ON binary lacks the pinned EasyPrivacy text (feature not built in)"
    if binary_has_marker(off_bin):
        return "OFF binary contains EasyPrivacy text (feature leaked into the OFF arm)"
    return None


def probe(binary, url):
    env = dict(os.environ)
    env["MANUK_AGENT_PROFILE"] = "lean"
    try:
        out = subprocess.run([binary, url], env=env, capture_output=True, text=True, timeout=150).stdout
    except subprocess.TimeoutExpired:
        return {"status": "TIMEOUT"}
    refused = [ln[len("refused="):].strip() for ln in out.splitlines() if ln.startswith("refused=")]
    kv = dict(re.findall(r"(\w+)=(\S+)", "\n".join(l for l in out.splitlines() if not l.startswith("refused="))))
    ok = kv.get("ok") == "true" and kv.get("observed") == "true"
    if not ok:
        return {"status": "FAIL"}
    return {"status": "OK", "blocked": kv["blocked"], "wire_bytes": kv["wire_bytes"], "wall_ms": kv["wall_ms"],
            "a11y": kv["a11y"], "links": kv["links"], "refused": ",".join(refused)}


def load_rows(path):
    rows = []
    with open(path) as f:
        head = f.readline().rstrip("\n").split("\t")
        assert head == COLS, head
        for ln in f:
            rows.append(dict(zip(COLS, ln.rstrip("\n").split("\t"))))
    return rows


def write_rows(path, rows):
    with open(path, "w") as f:
        f.write("\t".join(COLS) + "\n")
        for r in rows:
            f.write("\t".join(str(r.get(c, "")) for c in COLS) + "\n")


def analyse(rows, min_ok=3):
    """Return (report_lines, rc). rc: 0 settled, 1 RED, 2 INCOMPLETE."""
    by = {}
    for r in rows:
        by.setdefault(r["url"], {}).setdefault(r["arm"], []).append(r)
    out, rc, overblock = [], 0, []
    out.append("url\tarm\tok\tblocked_band\twire_band\twall_median_ms\ta11y_stable\tverdict")
    for url in by:
        verdicts = []
        for arm in ("off", "on"):
            ok = [r for r in by[url].get(arm, []) if r["status"] == "OK"]
            if len(ok) < min_ok:
                verdicts.append(f"{arm}:INSUFFICIENT")
                rc = max(rc, 2)
                out.append(f"{url}\t{arm}\t{len(ok)}\t-\t-\t-\t-\tINSUFFICIENT")
                continue
            bl = [int(r["blocked"]) for r in ok]
            wb = [int(r["wire_bytes"]) for r in ok]
            wall = statistics.median(int(r["wall_ms"]) for r in ok)
            digests = {r["a11y"] for r in ok}
            stable = len(digests) == 1
            out.append(f"{url}\t{arm}\t{len(ok)}\t{min(bl)}..{max(bl)}\t{min(wb)}..{max(wb)}\t{wall:.0f}\t"
                       f"{'yes' if stable else 'no'}\t-")
            verdicts.append(arm)
            by[url][arm + "_ok"] = ok
        # per-host report from every ON-arm OK run (over-blocking does not need the OFF arm)
        on_hosts, off_hosts = set(), set()
        for r in by[url].get("on", []):
            if r["status"] == "OK":
                on_hosts.update(refused_hosts(r.get("refused", "")))
        for r in by[url].get("off", []):
            if r["status"] == "OK":
                off_hosts.update(refused_hosts(r.get("refused", "")))
        ob_hosts = over_blocked(url, sorted(on_hosts | off_hosts))
        out.append(f"{url}\thosts\t-\tEP-only: {','.join(sorted(on_hosts - off_hosts)) or '-'}\t"
                   f"both: {','.join(sorted(on_hosts & off_hosts)) or '-'}\t-\t-\t"
                   f"OVERBLOCK={len(ob_hosts)} {','.join(ob_hosts)}")
        overblock.extend(ob_hosts)
        if "off_ok" in by[url] and "on_ok" in by[url]:
            off, on = by[url]["off_ok"], by[url]["on_ok"]
            ob, nb = [int(r["blocked"]) for r in off], [int(r["blocked"]) for r in on]
            ow, nw = [int(r["wire_bytes"]) for r in off], [int(r["wire_bytes"]) for r in on]
            if min(nb) > max(ob):
                refuse = "ON refuses more (bands disjoint)"
            elif max(nb) < min(ob):
                refuse = "ON refuses fewer (bands disjoint)"
            else:
                refuse = "refusal bands overlap"
            if max(nw) < min(ow):
                wire = "ON wire lower (bands disjoint)"
            elif min(nw) > max(ow):
                wire = "ON wire higher (bands disjoint)"
            else:
                wire = "wire bands overlap"
            off_d = {r["a11y"] for r in off}
            on_d = {r["a11y"] for r in on}
            if len(off_d) == 1 and len(on_d) == 1:
                if off_d == on_d:
                    digest = "digest agrees (both self-stable)"
                else:
                    digest = "DIGEST DISAGREES on self-stable page"
                    rc = 1
            else:
                digest = "digest not compared (not self-stable)"
            out.append(f"{url}\tverdict\t-\t{refuse}\t{wire}\t-\t-\t{digest}")
    out.append(f"TOTAL_OVERBLOCKED_HOSTS={len(overblock)}")
    return out, rc


def run_ab(off_bin, on_bin, urls, out_dir, runs):
    os.makedirs(out_dir, exist_ok=True)
    reason = check_build(off_bin, on_bin)
    if reason:
        print(f"VOID: {reason}")
        return 2
    rows = []
    path = os.path.join(out_dir, "runs.tsv")
    for url in urls:
        for run in range(1, runs + 1):
            arms = [("off", off_bin), ("on", on_bin)] if run % 2 else [("on", on_bin), ("off", off_bin)]
            for arm, binary in arms:
                res = probe(binary, url)
                row = {"url": url, "arm": arm, "run": run, "status": res["status"], "chrome_ver": CHROME}
                row.update({k: v for k, v in res.items() if k != "status"})
                rows.append(row)
                print(f"{url} {arm} run{run} {res['status']} blocked={res.get('blocked','-')} "
                      f"wire={res.get('wire_bytes','-')}", flush=True)
                write_rows(path, rows)
    lines, rc = analyse(rows)
    with open(os.path.join(out_dir, "summary.tsv"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return rc


def self_test():
    d = tempfile.mkdtemp(prefix="w4ec-st-")
    on_p, off_p = os.path.join(d, "on.bin"), os.path.join(d, "off.bin")
    with open(on_p, "wb") as f:
        f.write(b"\x00junk" + MARKER + b"\x00")
    with open(off_p, "wb") as f:
        f.write(b"\x00junk\x00")
    fails = []
    if check_build(off_p, on_p) is not None:
        fails.append("good build pair rejected")
    if check_build(on_p, on_p) is None:  # OFF arm really the feature build: must be VOID
        fails.append("feature-leaked OFF arm accepted")
    if check_build(off_p, off_p) is None:  # ON arm really the plain build: must be VOID
        fails.append("plain ON arm accepted")

    def row(url, arm, run, blocked, wire, a11y, status="OK"):
        return {"url": url, "arm": arm, "run": run, "status": status, "blocked": blocked, "wire_bytes": wire,
                "wall_ms": 1000, "a11y": a11y, "links": "0", "chrome_ver": CHROME}

    u = "https://example.test/"
    stable = [row(u, a, r, 2 if a == "on" else 1, 1000 if a == "on" else 1100, "aa") for a in ("off", "on")
              for r in (1, 2, 3)]
    lines, rc = analyse(stable)
    if rc != 0:
        fails.append(f"agreeing stable page gave rc {rc}")
    if not any("ON refuses more" in ln for ln in lines):
        fails.append("disjoint refusal bands not claimed")
    mutant = [dict(r) for r in stable]
    for r in mutant:
        if r["arm"] == "on":
            r["a11y"] = "bb"
    _, rc = analyse(mutant)
    if rc != 1:
        fails.append("digest-disagreement mutant not RED")
    short = [row(u, a, r, 1, 1, "aa", "FAIL" if r == 3 else "OK") for a in ("off", "on") for r in (1, 2, 3)]
    _, rc = analyse(short)
    if rc != 2:
        fails.append("2-OK-run sample not INCOMPLETE")
    # per-host report: EP-only hosts named; page-host refusal counted as over-blocking (mutant RED)
    def hrow(arm, run, refused):
        r = row(u, arm, run, 1, 1, "aa")
        r["refused"] = refused
        return r
    clean = [hrow(a, r, "https://x.crazyegg.com/a.js,https://t.mouseflow.com/b" if a == "on" else "https://x.doubleclick.net/c")
             for a in ("off", "on") for r in (1, 2, 3)]
    lines, _ = analyse(clean)
    if "TOTAL_OVERBLOCKED_HOSTS=0" not in lines[-1] or not any("EP-only: t.mouseflow.com,x.crazyegg.com" in l for l in lines):
        fails.append("clean host report wrong")
    bad = [hrow(a, r, "https://cdn.example.test/a.js,https://t.mouseflow.com/b") for a in ("off", "on") for r in (1, 2, 3)]
    lines, _ = analyse(bad)
    if "TOTAL_OVERBLOCKED_HOSTS=0" in lines[-1]:
        fails.append("page-host-in-list mutant not counted as over-blocking")
    if reg_domain("news.bbc.co.uk") != "bbc.co.uk" or reg_domain("www.bbc.com") != "bbc.com":
        fails.append("reg_domain wrong")
    if fails:
        print("SELF-TEST FAIL:", *fails, sep="\n  ")
        return 1
    print("SELF-TEST OK (build-marker, digest-mutant RED, sample INCOMPLETE, over-blocking mutant RED)")
    return 0


def main():
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report"])
    ap.add_argument("--off", required=True)
    ap.add_argument("--on", required=True)
    ap.add_argument("--urls")
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    if a.cmd == "run":
        urls = [ln.strip() for ln in open(a.urls) if ln.strip() and not ln.startswith("#")]
        sys.exit(run_ab(a.off, a.on, urls, a.out, a.runs))
    reason = check_build(a.off, a.on)
    if reason:
        print(f"VOID: {reason}")
        sys.exit(2)
    lines, rc = analyse(load_rows(os.path.join(a.out, "runs.tsv")))
    print("\n".join(lines))
    sys.exit(rc)


if __name__ == "__main__":
    main()
