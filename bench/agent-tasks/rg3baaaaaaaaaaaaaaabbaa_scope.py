#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a: why does manuk's interactive observation drop all 4 opendining-4 page
comboboxes once the Select-time listbox is open while chrome keeps them? Hypothesis: the page is MUI; its Modal manager sets
aria-hidden="true" on every <body> child but the popup's, manuk's a11y builder (engine/a11y is_hidden) prunes an aria-hidden
subtree, the Chrome backend's CDP builder (mcp/src/cdp.rs) never reads aria-hidden.
Part A (real page): replay the first N opendining-4 steps per backend (N=7 dropdown closed, N=8 open = step #7 done) and
count combobox nodes in observe(interactive) and in observe(full, include_hidden).
Part B (isolated rule): a local page with a listbox next to an app root that is / is not aria-hidden="true"; count the
buttons each backend lists.
  rg3baaaaaaaaaaaaaaabbaa_scope.py [--mutate close-dropdown]   exit 0 = rule confirmed, 1 = not
--mutate close-dropdown: the 'open' replay stops before step #7 (dropdown closed), so manuk must list the comboboxes again.
Writes docs/sprint/rg/RG3baaaaaaaaaaaaaaabbaa_scope.md on an unmutated run. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, http.server, os, re, sys, threading
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg3b_replay as r
import rg3baaaaaa_ordinal as o
import rg3baaaaaaa_match as mm

TASK = "opendining-4"
DOC = os.path.join(r.ROOT, "docs/sprint/rg/RG3baaaaaaaaaaaaaaabbaa_scope.md")
PAGE = ('<html><body><div id="root" %s><button>Page one</button><button>Page two</button><select><option>a</option></select></div>'
        '<div id="portal"><ul role="listbox"><li role="option">x</li></ul><button>Popup close</button></div></body></html>')


def count(t, role):
    return len(re.findall(r'\] ' + role + r'\b', t))


def real(side, n):
    st = r.lg.steps(os.path.join(r.TR, f"manuk_{TASK}_r1.jsonl"))[0]
    seq = mm.flatten_ref(st, 0)[:n]; out = {}
    def after(m):
        out["interactive"] = count(m.call("observe", {"mode": "interactive"}), "combobox")
        out["full+hidden"] = count(m.call("observe", {"mode": "full", "include_hidden": True}), "combobox")
    r.resolve = o.resolve
    r.replay(side, [s[:3] for s in seq], 2, settle=2.0, after=after)
    return out


def local(side, attr):
    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(s):
            b = (PAGE % attr).encode(); s.send_response(200); s.send_header("Content-Type", "text/html"); s.send_header("Content-Length", str(len(b))); s.end_headers(); s.wfile.write(b)
        def log_message(*a): pass
    srv = http.server.HTTPServer(("127.0.0.1", 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    env, args = dict(os.environ), []
    if side == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": f"http://127.0.0.1:{srv.server_port}/"})
        t = m.call("observe", {"mode": "interactive"})
        return count(t, "button"), t
    finally:
        m.p.kill(); srv.shutdown()


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["close-dropdown"]); a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    open_n = 7 if a.mutate else 8
    res = {(s, k): real(s, n) for s in ("manuk", "chrome") for k, n in (("closed", 7), ("open", open_n))}
    loc = {(s, k): local(s, v) for s in ("manuk", "chrome") for k, v in (("aria-hidden", 'aria-hidden="true"'), ("control", ""))}
    out = ["## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a", "", f"chrome_ver={pin}. Combobox nodes in opendining-4's observation after 7 steps (Select-time dropdown closed) and after 8 (step #7 opened it){' [MUTANT: open = 7 steps]' if a.mutate else ''}:", "",
           "| backend | dropdown | observe(interactive) | observe(full, include_hidden) |", "|---|---|---|---|"]
    for (s, k), v in res.items():
        out.append(f"| {s} | {k} | {v['interactive']} | {v['full+hidden']} |")
    out += ["", "Isolated rule: local page, `<div id=root [aria-hidden=true]>` with 2 buttons + a select, beside a `role=listbox` portal with 1 button; page buttons listed by observe(interactive) (portal's own button counts 1):", "",
            "| backend | root | buttons listed |", "|---|---|---|"]
    for (s, k), (n, _) in loc.items():
        out.append(f"| {s} | {k} | {n} |")
    # rule: manuk loses the comboboxes only when the dropdown is open, chrome never; aria-hidden root: manuk drops it, chrome keeps it; control: both keep
    ok = (res[("manuk", "closed")]["interactive"] >= 4 and res[("manuk", "open")]["interactive"] == 0 and res[("manuk", "open")]["full+hidden"] == 0
          and res[("chrome", "open")]["interactive"] >= 4
          and loc[("manuk", "aria-hidden")][0] == 1 and loc[("chrome", "aria-hidden")][0] >= 3
          and loc[("manuk", "control")][0] >= 3 and loc[("chrome", "control")][0] >= 3)
    out += ["", "RULE CONFIRMED: manuk's a11y builder prunes the aria-hidden app root that the page's modal manager sets while the dropdown is open; the CDP builder does not read aria-hidden." if ok else "RULE NOT CONFIRMED."]
    txt = "\n".join(out) + "\n"; print(txt)
    if not a.mutate:
        open(DOC, "w").write(txt)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
