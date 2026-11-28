"""AJ8 wrapper may only reinterpret the exact supervised expected failures."""
import importlib.util
from pathlib import Path
import signal
import unittest

spec = importlib.util.spec_from_file_location("aj8", Path(__file__).resolve().parents[2] / "perf/aj8_stylesheet_probe.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

class Aj8SupervisorTests(unittest.TestCase):
    def result(self):
        return {"cleanup": {"verified_absent": True, "failure": None}, "failure": "exit-status", "exit": 1,
                "stdout": 'AJ8_SMOKE_RESULT primary=Err("injected loopback failure") server=Ok(()) runtime=Ok(()) cleanup_verified=true', "stderr": ""}

    def test_only_precise_result_failure_is_expected(self):
        self.assertTrue(probe.accepted(self.result(), "after-navigation", True, True))
        for failure in ["process-timeout", "case-timeout", "output-limit", "cleanup-unverified", "signal 2", "ownership-uncertain"]:
            result = self.result(); result["failure"] = failure
            self.assertFalse(probe.accepted(result, "after-navigation", True, True))
        for status in [None, 0, 2, -signal.SIGTERM, -signal.SIGABRT]:
            result = self.result(); result["exit"] = status
            self.assertFalse(probe.accepted(result, "after-navigation", True, True))

    def test_cleanup_listener_and_binary_are_independent_requirements(self):
        result = self.result()
        self.assertFalse(probe.accepted(result, "after-navigation", False, True))
        self.assertFalse(probe.accepted(result, "after-navigation", True, False))
        result["cleanup"]["verified_absent"] = False
        self.assertFalse(probe.accepted(result, "after-navigation", True, True))
        result = self.result(); result["cleanup"]["failure"] = "uncertain"
        self.assertFalse(probe.accepted(result, "after-navigation", True, True))

    def test_controlled_abort_requires_sigabrt_and_exact_marker(self):
        result = self.result(); result["exit"] = -signal.SIGABRT
        self.assertFalse(probe.accepted(result, "abort", True, True))
        result["stderr"] = "AJ8 controlled optimized-abort fixture"
        self.assertTrue(probe.accepted(result, "abort", True, True))
        result["exit"] = -signal.SIGSEGV
        self.assertFalse(probe.accepted(result, "abort", True, True))

if __name__ == "__main__":
    unittest.main()
