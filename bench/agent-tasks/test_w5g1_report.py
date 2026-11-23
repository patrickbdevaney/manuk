#!/usr/bin/env python3
"""W5.G1.b.b fixture tests for the w5g1_report classifier. Run: python3 bench/agent-tasks/test_w5g1_report.py"""
import json, os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w5g1_report as r


def transcript(calls):
    """calls: [(tool, result_text)] -> path of a stream-json transcript that ends with a (non-error) result."""
    ev = []
    for i, (name, res) in enumerate(calls):
        if name == "say":  # an assistant text turn
            ev.append({"type": "assistant", "message": {"content": [{"type": "text", "text": res}]}})
            continue
        ev.append({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": f"t{i}", "name": "mcp__browser__" + name, "input": {}}]}})
        ev.append({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": f"t{i}", "content": res}]}})
    ev.append({"type": "result", "is_error": False, "result": "wrong answer"})
    fd, p = tempfile.mkstemp(suffix=".jsonl")
    with os.fdopen(fd, "w") as fh:
        fh.write("\n".join(json.dumps(e) for e in ev) + "\n")
    return p


NC = "no change | url: unchanged"


class T(unittest.TestCase):
    def test_key_only_is_not_tool_choice(self):
        self.assertNotEqual(r.classify(transcript([("key", "ok pressed Enter")])), "tool-choice")

    def test_no_action_is_tool_choice(self):
        self.assertEqual(r.classify(transcript([("observe", "page text")])), "tool-choice")

    def test_recovered_nochange_is_not_act(self):
        p = transcript([("act", NC), ("act", NC), ("act", "clicked; url: /next")])
        self.assertNotEqual(r.classify(p), "refusal-seen")

    def test_refusal_then_successful_scroll_is_stop_by_choice(self):
        # stated rule (W5.G1.b.d.a): any successful ACTS result (a scroll counts) after a refusal recovers it; this is
        # the dashdish-7 r3 / staynb-2 r3 shape that the hand reader labelled refusal-seen
        p = transcript([("act", "ok clicked"), ("act", "h418 refused focus (inert or disabled); nothing was focused"), ("scroll", "scrolled to y=929 of 2417 nothing new in view")])
        self.assertEqual(r.classify(p), "stop-by-choice")
        p = transcript([("act", "ok clicked"), ("act", "h418 refused focus (inert or disabled); nothing was focused"), ("find", "- button x")])
        self.assertEqual(r.classify(p), "refusal-seen")

    def test_newline_form_nochange_is_failure(self):
        NL = "no change\nurl: unchanged"
        # newline form after a success: failure result, two of them -> refusal-seen (clause b)
        self.assertEqual(r.classify(transcript([("act", "ok clicked"), ("act", NL), ("act", NL)])), "refusal-seen")
        # and it must not move the last-success window
        self.assertEqual(r.classify(transcript([("act", "ok clicked"), ("act", NL), ("act", NL), ("act", NL)])), "refusal-seen")
        # a single one is not enough
        self.assertEqual(r.classify(transcript([("act", "ok clicked"), ("act", NL)])), "stop-by-choice")

    def test_unrecovered_nochange_is_act(self):
        self.assertEqual(r.classify(transcript([("act", NC), ("act", NC)])), "refusal-seen")

    def test_nochange_after_success_is_act(self):
        p = transcript([("act", "clicked; url: /next"), ("act", NC), ("act", NC)])
        self.assertEqual(r.classify(p), "refusal-seen")

    def test_overlay_seen_but_completed_is_stop_by_choice(self):
        p = transcript([("say", "a modal overlay is in the way"), ("act", "ok: dispatched click"), ("act", "ok: dispatched click; url: /done")])
        self.assertEqual(r.classify(p), "stop-by-choice")

    def test_no_judgement_class_emitted(self):
        for calls in ([("act", "ok")], [("say", "year dropdown is not exposed to my tools"), ("act", "ok")], [("act", NC), ("act", NC)]):
            self.assertIn(r.classify(transcript(calls)), r.CLASSES)
        self.assertNotIn("perceive", r.CLASSES); self.assertNotIn("target", r.CLASSES)

    def test_report_exits_1_on_injected_perceive_row(self):
        import subprocess, csv
        d = tempfile.mkdtemp(); runs = os.path.join(d, "runs"); os.makedirs(runs)
        hdr = ["ts", "arm", "task", "run", "pass", "evals_vec", "answer"]
        for arm in r.ARMS:
            with open(os.path.join(d, arm + ".tsv"), "w") as fh:
                w = csv.writer(fh, delimiter="\t"); w.writerow(hdr)
                for run in (1, 2, 3):
                    w.writerow(["", arm, "t1", run, 0, "[0]", "wrong answer"])
                    os.makedirs(os.path.join(runs, f"{arm}_t1_r{run}"))
                    os.replace(transcript([("act", "ok: dispatched click")]), os.path.join(runs, f"{arm}_t1_r{run}", "transcript.jsonl"))
        base = ["python3", "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "w5g1_report.py"), "--runs-dir", runs, "--dir", d,
                "--causes-out", os.path.join(d, "c.tsv"), "--table-out", os.path.join(d, "t.tsv")]
        ok = subprocess.run(base, capture_output=True, text=True)
        self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
        bad = subprocess.run(base + ["--mutate", "inject-perceive"], capture_output=True, text=True)
        self.assertEqual(bad.returncode, 1, bad.stdout)

    def test_all_stale_typing_is_act(self):
        p = transcript([("act", "ok: dispatched click; url: /c"), ("type", "STALE: h1 is no longer on the page"), ("act", "ok: dispatched click"),
                        ("type", "STALE: h1 is no longer on the page"), ("handoff", "no human surface is attached to this backend")])
        self.assertEqual(r.classify(p), "refusal-seen")

    def test_typing_that_landed_is_not_act(self):
        p = transcript([("type", "STALE: h1 is no longer on the page"), ("type", "ok: typed"), ("act", "ok: dispatched click; url: /d")])
        self.assertNotEqual(r.classify(p), "refusal-seen")


if __name__ == "__main__":
    unittest.main()
