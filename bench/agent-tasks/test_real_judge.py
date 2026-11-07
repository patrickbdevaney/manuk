#!/usr/bin/env python3
"""W5.J1: retry-after cap. Run: python3 -I bench/agent-tasks/test_real_judge.py"""
import io, os, sys, unittest, contextlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_judge as rj


class RetryWait(unittest.TestCase):
    def test_cap_and_log(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(rj.retry_wait("7200", 0), 300)
        self.assertIn("capped to 300s", err.getvalue())

    def test_below_cap_untouched_and_silent(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(rj.retry_wait("67", 0), 67)
            self.assertEqual(rj.retry_wait(None, 3), 8)
            self.assertEqual(rj.retry_wait("garbage", 10), 60)
        self.assertEqual(err.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
