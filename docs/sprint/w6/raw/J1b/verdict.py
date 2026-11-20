import json, sys, glob, os, statistics
# usage: verdict.py <raw_dir> [--swap]   reads t{10,120}_r{1,2,3}_<file>.json
# verdict on the pre-registered falsifier: identical pass counts at 10s and 120s -> ENGINE; 120s strictly
# more passes (beyond the 3-run band) -> BUDGET; 10s more than 120s -> INVERTED.
def load(raw, t, name):
    out = []
    for r in (1, 2, 3):
        p = os.path.join(raw, f"t{t}_r{r}_{name}.json")
        if not os.path.exists(p):
            continue
        d = json.loads(open(p).read().splitlines()[0])
        if d["total"] == 0:  # NO_REPORT / instrument flake: recorded, not scored
            continue
        out.append(d["pass"])
    return out
def verdict(lo, hi):  # lo = passes at 10s, hi = passes at 120s (lists of per-run pass counts)
    if not lo or not hi:
        return "UNMEASURABLE"
    if min(hi) > max(lo):
        return "BUDGET"
    if max(hi) == min(hi) == max(lo) == min(lo):
        return "ENGINE"
    if max(lo) > max(hi):
        return "INVERTED"
    return "MIXED"
if __name__ == "__main__":
    raw = sys.argv[1]
    swap = "--swap" in sys.argv
    for name in ["properties-value-001", "properties-value-inherit-001"]:
        lo, hi = load(raw, 10, name), load(raw, 120, name)
        if swap:
            lo, hi = hi, lo
        print(f"{name}: 10s={lo} 120s={hi} -> {verdict(lo, hi)}")
