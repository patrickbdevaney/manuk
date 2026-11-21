#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a.a: the Chrome (CDP) backend honors aria-hidden="true" like manuk's builder
and Chrome's own AX tree (except where the element holds focus). Re-runs the scope probe (rg3baaaaaaaaaaaaaaabbaa_scope.py):
  - opendining-4 with the Select-time dropdown open: chrome lists 0 page comboboxes (as manuk does), dropdown closed: 4;
  - local page with aria-hidden root: chrome lists 1 button (the portal's), control page: 3, same as manuk.
  rg3baaaaaaaaaaaaaaabbaaa_chrome.py [--mutate aria-hidden-ignore]   exit 0 = chrome now matches manuk, 1 = not
--mutate: MANUK_CDP_MUTATE=aria-hidden-ignore makes the CDP builder ignore the attribute again, so chrome must re-list them.
Run with `python3 -I`; needs MANUK_CHROME_PIN and a release manuk-mcp."""
import argparse, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rg3baaaaaaaaaaaaaaabbaa_scope as sc

DOC = os.path.join(sc.r.ROOT, "docs/sprint/rg/RG3baaaaaaaaaaaaaaabbaaa_chrome.md")


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--mutate", choices=["aria-hidden-ignore"]); a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    if a.mutate:
        os.environ["MANUK_CDP_MUTATE"] = a.mutate
    res = {(s, k): sc.real(s, n) for s in ("manuk", "chrome") for k, n in (("closed", 7), ("open", 8))}
    loc = {(s, k): sc.local(s, v)[0] for s in ("manuk", "chrome") for k, v in (("aria-hidden", 'aria-hidden="true"'), ("control", ""))}
    out = ["## RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b.a.a.a.a", "", f"chrome_ver={pin}{' [MUTANT ' + a.mutate + ']' if a.mutate else ''}. opendining-4 comboboxes in observe(interactive):", "",
           "| backend | dropdown | comboboxes |", "|---|---|---|"]
    out += [f"| {s} | {k} | {v['interactive']} |" for (s, k), v in res.items()]
    out += ["", "Local page buttons listed (aria-hidden app root vs control):", "", "| backend | root | buttons |", "|---|---|---|"]
    out += [f"| {s} | {k} | {n} |" for (s, k), n in loc.items()]
    ok = (res[("chrome", "closed")]["interactive"] >= 4 and res[("chrome", "open")]["interactive"] == 0
          and res[("manuk", "open")]["interactive"] == 0
          and loc[("chrome", "aria-hidden")] == 1 and loc[("chrome", "control")] >= 3 and loc[("manuk", "aria-hidden")] == 1)
    out += ["", "CHROME NOW MATCHES manuk (aria-hidden honored)." if ok else "NOT MATCHING."]
    txt = "\n".join(out) + "\n"; print(txt)
    if not a.mutate:
        open(DOC, "w").write(txt)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
