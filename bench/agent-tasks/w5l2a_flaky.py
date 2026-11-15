#!/usr/bin/env python3 -I
"""W5.L2.a: does the W5.L2 build (auto-checkpoint before state-changing acts, 'undo: restore("autoN")' on a failed
result) get the restore tool used by a real Haiku run on the W4.C3 flaky checkout?

The checkout is the c3_bon.py "clear" fixture: Submit hits GET /roll, 1 in 3 attempts succeed (schedule seeded per
run, identical to W4.C3). Each run is one `claude -p --model haiku` session on the manuk arm with the UNCHANGED
run_arm.PROMPT and no system hint (the gate forbids a prompt change). A run counts as a restore run only if the
transcript holds a restore tool_use (the model called it), not if the undo line merely appeared.

Usage: python3 -I w5l2a_flaky.py [--runs 3] [--out docs/sprint/w5/L2a-runs.tsv] [--transcripts DIR]
       python3 -I w5l2a_flaky.py --self-test     (the gate must be GREEN on a restore transcript and RED on none)
Needs MANUK_CHROME_PIN (every row carries it). MANUK_MCP_BIN overrides the binary (default target/release/manuk-mcp).
"""
import argparse, json, os, shutil, sys, tempfile, threading
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import c3_bon, run_arm  # noqa: E402  (H = the flaky checkout server, rolls = seeded schedule, FIX = task text)

FX = "clear"
CHARGES = {}  # --side-effect (W5.L2.a.a suite version): every submit attempt charges and the page shows it
COLS = ["chrome_ver", "run", "restore_calls", "checkpoint_calls", "toolsearch_restore", "undo_lines", "success",
        "turns", "cost_usd", "prompt_changed"]
GOOD = c3_bon.FIX[FX][1]  # "Order confirmed for Ada": the server's success text, seen in a tool result = success


class HSide(c3_bon.H):
    """The clear fixture plus a server-side charge per attempt: a failure reads 'declined ... charged $20 (total $N)'."""
    def do_GET(self):
        p = self.path.split("?")[0].strip("/").split("/")
        if p[0] != "roll":
            return super().do_GET()
        key = tuple(p[1:5])
        with c3_bon.LOCK:
            k = c3_bon.SCHED.get(key, 0); c3_bon.SCHED[key] = k + 1
            CHARGES[key] = CHARGES.get(key, 0) + 20
            total = CHARGES[key]
        ok = c3_bon.rolls(int(p[2]), int(p[3]))[min(k, 2)]
        t = c3_bon.FIX[p[1]][1] if ok else f"Error: payment declined. Your card was still charged $20 for this attempt (total charged: ${total}). Try again."
        b = (t + (f" Total charged: ${total}." if ok else "")).encode()
        self.send_response(200); self.send_header("Content-Type", "text/plain"); self.send_header("Content-Length", str(len(b)))
        self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(b)


def summarize(out):
    """stream-json -> counts of restore/checkpoint tool_use blocks, ToolSearch loads of restore, undo lines, success."""
    c = {"restore": 0, "checkpoint": 0, "toolsearch_restore": 0, "undo_lines": 0, "success": 0}
    for l in out.splitlines():
        if not l.startswith("{"):
            continue
        e = json.loads(l)
        msg = e.get("message") or {}
        blocks = msg.get("content") if isinstance(msg.get("content"), list) else []
        for b in blocks:
            if b.get("type") == "tool_use":
                name = b.get("name", "")
                if name.endswith("restore"):
                    c["restore"] += 1
                elif name.endswith("checkpoint"):
                    c["checkpoint"] += 1
                elif name == "ToolSearch" and "restore" in str(b.get("input", {}).get("query", "")):
                    c["toolsearch_restore"] += 1
            elif b.get("type") == "tool_result":
                txt = json.dumps(b.get("content"))
                c["undo_lines"] += txt.count("undo: restore(")
                c["success"] += int(GOOD in txt)
    c["success"] = int(c["success"] > 0)
    return c


def gate(restore_per_run):
    """W5.L2.a gate: restore used in >= 1 of the runs. -> (pass, runs_with_restore)."""
    n = sum(1 for r in restore_per_run if r > 0)
    return n >= 1, n


def _side_fail():
    """Drive HSide once on a seed whose first roll fails and return the body (self-test of the fixture)."""
    import urllib.request
    run = next(r for r in range(1, 50) if not c3_bon.rolls(r, 0)[0])
    srv = ThreadingHTTPServer(("127.0.0.1", 0), HSide)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    body = urllib.request.urlopen(f"http://127.0.0.1:{srv.server_address[1]}/roll/clear/{run}/0/t").read().decode()
    srv.shutdown()
    return body


def self_test():
    restore = '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__browser__restore","input":{"checkpoint":"auto1"}}]}}\n'
    none = '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__browser__act","input":{}}]}}\n'
    undo = ('{"type":"user","message":{"content":[{"type":"tool_result","content":"Error: payment declined. '
            'undo: restore(\\"auto1\\")"}]}}\n')
    assert summarize(restore)["restore"] == 1, "restore tool_use not counted"
    assert summarize(none)["restore"] == 0, "non-restore tool_use counted as restore"
    assert summarize(undo)["undo_lines"] == 1, "undo line not counted"
    assert gate([1, 0, 0]) == (True, 1), "green case not green"
    ok, n = gate([0, 0, 0])
    assert not ok and n == 0, "all-zero transcripts must be RED"
    assert "total charged: $20" in _side_fail(), "side-effect fixture must show the charge on a failed attempt"
    print("self-test: parser counts restore/undo; gate GREEN on 1-of-3, RED on 0-of-3 (mutation red check)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w5/L2a-runs.tsv"))
    ap.add_argument("--transcripts", default=os.path.join(ROOT, "docs/sprint/w5/L2a-transcripts"))
    ap.add_argument("--side-effect", action="store_true", help="W5.L2.a.a fixture: each attempt charges the card")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    if not os.path.exists(run_arm.mcp_bin()):
        sys.exit(f"refusing: {run_arm.mcp_bin()} missing")
    srv = ThreadingHTTPServer(("127.0.0.1", 0), HSide if a.side_effect else c3_bon.H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_address[1]}/"
    os.makedirs(a.transcripts, exist_ok=True)
    rows, restores = [], []
    for run in range(1, a.runs + 1):
        tmp = tempfile.mkdtemp(prefix="w5l2a-")
        cfg, allowed = run_arm.arm_config("manuk", tmp)
        url = f"{base}form/{FX}/{run}/0/manuk"
        prompt = run_arm.PROMPT.format(url=url, goal=c3_bon.FIX[FX][0] + (" Every submit attempt charges the card, so avoid paying more than once." if a.side_effect else ""))
        out, err, secs, _ = run_arm.run_claude(prompt, cfg, allowed, timeout=600)
        open(os.path.join(a.transcripts, f"manuk_L2a_r{run}.jsonl"), "w").write(out)
        res = run_arm.parse(out) or {}
        c = summarize(out)
        restores.append(c["restore"])
        rows.append([pin, run, c["restore"], c["checkpoint"], c["toolsearch_restore"], c["undo_lines"], c["success"],
                     res.get("num_turns", ""), f"{res.get('total_cost_usd', 0):.4f}", 0])
        print("\t".join(map(str, rows[-1])), f"err={err[-120:]!r}" if err.strip() else "", flush=True)
        shutil.rmtree(tmp, ignore_errors=True)
    ok, n = gate(restores)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    open(a.out, "w").write("\t".join(COLS) + "\n" + "\n".join("\t".join(map(str, r)) for r in rows) + "\n")
    print(f"GATE W5.L2.a: restore used in {n} of {a.runs} runs (need >=1): {'PASS' if ok else 'RED'}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
