#!/usr/bin/env python3
"""W5.B1.f gate: primary-judge exhaustion must send the WHOLE task to the fallback; both failing -> JudgeUnavailable.
Run: python3 -I bench/agent-tasks/test_judge_fallback.py   (exit 0 = pass). No network, no claude: verdict is stubbed."""
import os, sys, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_judge as rj

PRIMARY = {"REAL_JUDGE_MODEL": "gpt-oss-120b"}
FB = {"backend": "claude", "REAL_JUDGE_MODEL": rj.FALLBACK_MODEL}
ITEMS = [("r1", "a", None), ("r2", "a", None), ("r3", "a", None)]


def stub(fail_models, fail_after=0):
    calls = []
    def v(env, rubric, answer, state=None):
        m = env["REAL_JUDGE_MODEL"]
        calls.append((m, rubric))
        if m in fail_models and len([c for c in calls if c[0] == m]) > fail_after:
            raise RuntimeError("judge: retries exhausted")
        return True, "ok"
    return v, calls


class T(unittest.TestCase):
    def test_primary_ok(self):
        rj.verdict, calls = stub(set())
        vs, m = rj.judge_task(PRIMARY, ITEMS, lambda: FB)
        self.assertEqual((vs, m), ([True] * 3, "gpt-oss-120b"))

    def test_exhaustion_mid_task_rejudges_whole_task_with_fallback(self):
        rj.verdict, calls = stub({"gpt-oss-120b"}, fail_after=1)  # rubric 1 judged by primary, rubric 2 exhausts
        vs, m = rj.judge_task(PRIMARY, ITEMS, lambda: FB)
        self.assertEqual(m, rj.FALLBACK_MODEL)
        self.assertEqual([r for mm, r in calls if mm == rj.FALLBACK_MODEL], ["r1", "r2", "r3"])  # all rubrics, one judge
        self.assertEqual(vs, [True] * 3)

    def test_both_fail_is_unavailable_not_a_verdict(self):
        rj.verdict, calls = stub({"gpt-oss-120b", rj.FALLBACK_MODEL})
        with self.assertRaises(rj.JudgeUnavailable):
            rj.judge_task(PRIMARY, ITEMS, lambda: FB)


if __name__ == "__main__":
    unittest.main()
