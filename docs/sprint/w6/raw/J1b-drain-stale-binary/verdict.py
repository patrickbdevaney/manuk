import json, sys, os
# usage: verdict.py <raw_dir> [--swap]   reads {default,off}_r{1,2,3}_<file>.json
# Pre-registered falsifier: pass counts identical with the drain bound removed (off == default) -> NOT_DRAIN
# (stall is not the drain budget). Off strictly more passes than default, beyond the 3-run band -> DRAIN.
# Off strictly fewer -> INVERTED. Otherwise MIXED. No scored run for an arm -> UNMEASURABLE (a hang is rc=124, total 0).
def load(raw, arm, name):
    out, hangs = [], 0
    for r in (1, 2, 3):
        p = os.path.join(raw, f"{arm}_r{r}_{name}.json")
        if not os.path.exists(p):
            hangs += 1  # no JSON written: child guard killed it (hang) or the run never finished
            continue
        line = open(p).read().splitlines()
        if not line:
            hangs += 1
            continue
        d = json.loads(line[0])
        if d["total"] == 0:  # NO_REPORT / instrument flake: recorded, not scored
            continue
        out.append(d["pass"])
    return out, hangs
def verdict(dflt, off):
    if not dflt or not off:
        return "UNMEASURABLE"
    if min(off) > max(dflt):
        return "DRAIN"
    if max(off) == min(off) == max(dflt) == min(dflt):
        return "NOT_DRAIN"
    if max(dflt) > min(off) and min(dflt) > max(off):
        return "INVERTED"
    return "MIXED"
if __name__ == "__main__":
    raw = sys.argv[1]
    swap = "--swap" in sys.argv
    for name in ["properties-value-001", "properties-value-inherit-001"]:
        dflt, dh = load(raw, "default", name)
        off, oh = load(raw, "off", name)
        if swap:
            dflt, off, dh, oh = off, dflt, oh, dh
        print(f"{name}: default={dflt} (no-result {dh}) off={off} (no-result {oh}) -> {verdict(dflt, off)}")
