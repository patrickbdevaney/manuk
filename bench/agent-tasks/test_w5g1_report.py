#!/usr/bin/env python3
"""W5.G1.b.b fixture tests for the w5g1_report classifier. Run: python3 bench/agent-tasks/test_w5g1_report.py"""
import json, os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import w5g1_report as r


def transcript(calls):
    """calls: [(tool, result_text)] -> path of a stream-json transcript that ends with a (non-error) result."""
    ev = []
    for i, (name, res) in enumerate(calls):
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
        self.assertNotEqual(r.classify(p), "act")

    def test_unrecovered_nochange_is_act(self):
        self.assertEqual(r.classify(transcript([("act", NC), ("act", NC)])), "act")

    def test_nochange_after_success_is_act(self):
        p = transcript([("act", "clicked; url: /next"), ("act", NC), ("act", NC)])
        self.assertEqual(r.classify(p), "act")


if __name__ == "__main__":
    unittest.main()
