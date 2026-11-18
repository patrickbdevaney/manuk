#!/usr/bin/env python3
"""W5.G1.a.a.a gate: every quote in G1aa-stop-before-send.md must be verbatim in its source.

Usage: G1aa_stop_check.py [--doc PATH] [--root REPO]
Reads the fenced ```quotes block in the doc. Each line is `<source>TAB<text>`:
  task  -> must appear in the goal of bench/agent-tasks/real/tasks/gomail-3.json
  r1/r3 -> must appear verbatim in a decoded string value of
           target/real-runs/cdp-layer_gomail-3_<src>/transcript.jsonl
Exit 0 = all quotes found, 1 = a quote is missing (or the block is empty).
"""
import json
import os
import sys


def strings(node):
    # every string value in a decoded JSON tree, so escaped quotes compare as the model wrote them
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for v in node.values():
            yield from strings(v)
    elif isinstance(node, list):
        for v in node:
            yield from strings(v)


def read_quotes(doc):
    out, inside = [], False
    with open(doc, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.strip() == "```quotes":
                inside = True
                continue
            if inside and line.strip() == "```":
                break
            if inside and "\t" in line:
                src, text = line.split("\t", 1)
                out.append((src.strip(), text))
    return out


def main(argv):
    doc = "docs/sprint/w5/G1aa-stop-before-send.md"
    root = "."
    if "--doc" in argv:
        doc = argv[argv.index("--doc") + 1]
    if "--root" in argv:
        root = argv[argv.index("--root") + 1]

    quotes = read_quotes(doc)
    if not quotes:
        print(f"FAIL: no quotes block in {doc}")
        return 1

    corpus = {}
    task_path = os.path.join(root, "bench/agent-tasks/real/tasks/gomail-3.json")
    with open(task_path, encoding="utf-8") as f:
        corpus["task"] = json.load(f)["goal"]

    failed = 0
    for src, text in quotes:
        if src not in corpus:
            # src is a timestamped archive dir name (the live r<N> dirs are overwritten by reruns);
            # bare run labels such as "r1" still resolve to the live dir for older docs
            dirname = src if src.startswith("cdp-layer_") else f"cdp-layer_gomail-3_{src}"
            tr = os.path.join(root, f"target/real-runs/{dirname}/transcript.jsonl")
            vals = []
            with open(tr, encoding="utf-8") as f:
                for line in f:
                    if line.strip():
                        vals.extend(strings(json.loads(line)))
            corpus[src] = vals
        haystack = corpus[src]
        if isinstance(haystack, str):
            hit = text in haystack
        else:
            hit = any(text in v for v in haystack)
        if hit:
            print(f"ok    {src:4s} {text[:70]}")
        else:
            failed += 1
            print(f"MISS  {src:4s} {text[:70]}")

    if failed:
        print(f"FAIL: {failed} of {len(quotes)} quotes not found verbatim")
        return 1
    print(f"PASS: {len(quotes)} of {len(quotes)} quotes found verbatim")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
