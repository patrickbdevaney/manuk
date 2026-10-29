#!/usr/bin/env python3
"""R2.1.5: attribute a run's input-side tokens to the calls that produced them, from stream-json transcripts.

  transcript_table.py --rows ROWS.tsv --dir DIR [--tol 0.10] [--mutate empty] [--arm-label NAME]

ROWS.tsv is run_arm.py output (same COLS). For every row there must be DIR/<arm>_<task>_r<run>.jsonl with a `result`
event. Checks (exit 1 on any failure):
  1. the transcript is non-empty and has a result event (a row with an empty transcript fails);
  2. the per-message usage summed from the transcript (input + cache_read + cache_create, one count per message id)
     is within --tol of the row's input+cache_read+cache_create tokens.
Attribution (per run): turn k's new context N_k = input_k + cache_create_k is split over what was appended since turn
k-1: the model's previous output (exact output_tokens) and each tool result (bytes/4 as a token estimate, by share).
Turn 1's N goes to 'prompt+system+tool schemas'. Each turn's full input-side total T_k = input+cache_read+cache_create
is split over buckets by their cumulative share of context so far, so the bucket totals sum to the run's total.
"""
import argparse, collections, csv, json, os, sys


def tool_bytes(content):
    """Bytes a tool result adds to context. Non-text blocks (e.g. ToolSearch's tool_reference, which expands into
    a tool schema server-side) are counted by their JSON size, or they would be invisible to the attribution."""
    if isinstance(content, str):
        return len(content.encode())
    n = 0
    for c in content or []:
        if isinstance(c, dict) and c.get("type") == "text":
            n += len((c.get("text") or "").encode())
        else:
            n += len(json.dumps(c).encode())
    return n


def analyze(events):
    """-> dict with per-bucket tokens, per-tool bytes/calls, message-sum usage, and the result event's usage."""
    usage_by_msg, order, names, res_bytes, calls = {}, [], {}, collections.Counter(), collections.Counter()
    result = None
    stream = []  # (kind, payload) in arrival order, to replay context growth
    seen_msg = set()
    for e in events:
        t = e.get("type")
        if t == "result":
            result = e
        elif t == "assistant":
            m = e["message"]
            mid = m["id"]
            for b in m.get("content", []):
                if b.get("type") == "tool_use":
                    names[b["id"]] = b["name"].split("__")[-1]
            usage_by_msg[mid] = m.get("usage", {})  # streamed chunks repeat the usage; the last one wins
            if mid not in seen_msg:
                seen_msg.add(mid)
                stream.append(("turn", mid))
        elif t == "user":
            c = e["message"].get("content")
            if isinstance(c, list):
                for b in c:
                    if b.get("type") == "tool_result":
                        name = names.get(b.get("tool_use_id"), "?")
                        n = tool_bytes(b.get("content"))
                        stream.append(("tool", (name, n)))
    # replay: pending items are what was appended since the previous turn
    cum = collections.Counter()           # cumulative new-context tokens per bucket
    bucket_ctx = collections.Counter()    # attributed input-side tokens (input + cache_read + cache_create)
    bucket_new = collections.Counter()    # attributed cache_create (new context) tokens
    pending, turn_no, prev_out = [], 0, None
    for kind, p in stream:
        if kind == "tool":
            pending.append((p[0], p[1] / 4.0))
            res_bytes[p[0]] += p[1]; calls[p[0]] += 1
            continue
        u = usage_by_msg[p]
        turn_no += 1
        n_new = u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
        t_tot = n_new + u.get("cache_read_input_tokens", 0)
        if turn_no == 1:
            items = [("prompt+system+tool schemas", 1.0)]
        else:
            items = pending
        if prev_out is not None and turn_no > 1:
            items = [("model output", prev_out)] + items
        pending = []
        w = sum(x for _, x in items) or 1.0
        for b, x in items:
            share = x / w
            cum[b] += n_new * share
            bucket_new[b] += n_new * share
        tot_cum = sum(cum.values()) or 1.0
        for b in cum:
            bucket_ctx[b] += t_tot * cum[b] / tot_cum
        prev_out = u.get("output_tokens", 0)
    msg_sum = collections.Counter()
    for u in usage_by_msg.values():
        msg_sum["input"] += u.get("input_tokens", 0)
        msg_sum["cache_read"] += u.get("cache_read_input_tokens", 0)
        msg_sum["cache_create"] += u.get("cache_creation_input_tokens", 0)
        msg_sum["output"] += u.get("output_tokens", 0)
    return {"bucket_ctx": bucket_ctx, "bucket_new": bucket_new, "res_bytes": res_bytes, "calls": calls,
            "msg_sum": msg_sum, "turns": turn_no, "result": result}


def load(path, mutate_empty):
    if mutate_empty:  # red-check: the transcript is present but empty
        return []
    out = []
    for line in open(path):
        if line.startswith("{"):
            out.append(json.loads(line))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rows", required=True); ap.add_argument("--dir", required=True)
    ap.add_argument("--tol", type=float, default=0.10)
    ap.add_argument("--mutate", choices=["empty"], help="red-check: blank the first run's transcript")
    a = ap.parse_args()
    rows = list(csv.DictReader(open(a.rows), delimiter="\t"))
    fails, agg = [], collections.defaultdict(lambda: collections.defaultdict(float))
    tot_ctx = collections.Counter()
    for i, r in enumerate(rows):
        path = os.path.join(a.dir, f"{r['arm']}_{r['task']}_r{r['run']}.jsonl")
        ev = load(path, a.mutate == "empty" and i == 0) if os.path.exists(path) else []
        if not any(e.get("type") == "result" for e in ev):
            fails.append(f"{r['arm']} {r['task']} r{r['run']}: transcript empty or has no result event")
            continue
        an = analyze(ev)
        row_in = sum(int(r[k] or 0) for k in ("input_tokens", "cache_read_tokens", "cache_create_tokens"))
        msg_in = an["msg_sum"]["input"] + an["msg_sum"]["cache_read"] + an["msg_sum"]["cache_create"]
        dev = abs(msg_in - row_in) / max(row_in, 1)
        if dev > a.tol:
            fails.append(f"{r['arm']} {r['task']} r{r['run']}: transcript sums {msg_in} vs row {row_in} ({dev:.1%})")
        for b, v in an["bucket_ctx"].items():
            agg[(r["arm"], b)]["ctx"] += v
        for b, v in an["bucket_new"].items():
            agg[(r["arm"], b)]["new"] += v
        for t, v in an["res_bytes"].items():
            agg[(r["arm"], t)]["bytes"] += v
            agg[(r["arm"], t)]["calls"] += an["calls"][t]
        tot_ctx[r["arm"]] += row_in
        print(f"# {r['arm']} {r['task']} r{r['run']}: turns={an['turns']} msg_in={msg_in} row_in={row_in} "
              f"dev={dev:.1%} result_turns={an['result'].get('num_turns')}", file=sys.stderr)
    arms = sorted({k[0] for k in agg} | set(tot_ctx))
    for arm in arms:
        print(f"\n### {arm}: {len([r for r in rows if r['arm'] == arm])} runs\n")
        print("| bucket | calls | tool-result bytes | attributed cache_create tokens | attributed input-side tokens | share |")
        print("|---|---:|---:|---:|---:|---:|")
        keys = sorted([k for k in agg if k[0] == arm], key=lambda k: -agg[k]["ctx"])
        total = sum(agg[k]["ctx"] for k in keys) or 1.0
        for k in keys:
            d = agg[k]
            print(f"| {k[1]} | {int(d['calls']) or ''} | {int(d['bytes']) or ''} | {d['new']:.0f} | {d['ctx']:.0f} | {d['ctx']/total:.1%} |")
        print(f"| **total** | | | {sum(agg[k]['new'] for k in keys):.0f} | {total:.0f} | |")
        print(f"\nRow-side input+cache tokens for these runs: {tot_ctx[arm]}; table total {total:.0f} "
              f"({abs(total - tot_ctx[arm]) / max(tot_ctx[arm], 1):.1%} off).")
    if fails:
        print("\nFAIL:\n  " + "\n  ".join(fails), file=sys.stderr)
        sys.exit(1)
    print("\nPASS: every row has a transcript; transcript sums within %.0f%% of the row." % (a.tol * 100), file=sys.stderr)


if __name__ == "__main__":
    main()
