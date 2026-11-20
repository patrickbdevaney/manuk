#!/usr/bin/env python3
"""sprint-ledger — turn one `claude -p --output-format json` result into a docs/sprint/LEDGER.tsv row.

  sprint-ledger.py <json-file> <node> <model> <tier> <rc> <pack|full> [ledger-path]

Prints the result text (so the loop can keep appending it to loop.log and grepping it for API errors).
If the file is not parseable JSON it is printed verbatim and a row with empty token cells is still written,
so every attempt has a ledger row. input_total = input + cache_creation + cache_read (all prompt-side tokens).
"""
import json
import sys
import time
from pathlib import Path

HDR = ["ts", "node", "model", "tier", "rc", "prompt", "input_tokens", "cache_creation", "cache_read",
       "input_total", "output_tokens", "num_turns", "cost_usd", "duration_ms", "session_id", "is_error"]


def main(a: list[str]) -> int:
    src, node, model, tier, rc, mode = a[:6]
    ledger = Path(a[6]) if len(a) > 6 else Path(__file__).resolve().parent.parent / "docs/sprint/LEDGER.tsv"
    raw = Path(src).read_text(errors="replace") if Path(src).exists() else ""
    d = None
    try:
        d = json.loads(raw)
        if isinstance(d, list):  # stream-json style array: take the result event
            d = next((e for e in reversed(d) if isinstance(e, dict) and e.get("type") == "result"), None)
    except ValueError:
        d = None
    u = (d or {}).get("usage") or {}
    g = lambda k: int(u.get(k) or 0)
    cells = {"ts": time.strftime("%F %T"), "node": node, "model": model, "tier": tier, "rc": rc, "prompt": mode}
    if d is not None and u:
        cells.update(input_tokens=g("input_tokens"), cache_creation=g("cache_creation_input_tokens"),
                     cache_read=g("cache_read_input_tokens"), output_tokens=g("output_tokens"))
        cells["input_total"] = cells["input_tokens"] + cells["cache_creation"] + cells["cache_read"]
    if d is not None:
        cells.update(num_turns=d.get("num_turns", ""), cost_usd=d.get("total_cost_usd", ""),
                     duration_ms=d.get("duration_ms", ""), session_id=d.get("session_id", ""),
                     is_error=d.get("is_error", ""))
    new = not ledger.exists() or ledger.stat().st_size == 0
    with ledger.open("a") as f:
        if new:
            f.write("\t".join(HDR) + "\n")
        f.write("\t".join(str(cells.get(h, "")).replace("\t", " ").replace("\n", " ") for h in HDR) + "\n")
    print((d or {}).get("result", "") if d is not None else raw)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]) if len(sys.argv) >= 7 else 1)
