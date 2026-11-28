"""Independent negative contracts for AJ8's CI inventory verifier."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("aj8_tests", Path(__file__).resolve().parents[1] / "aj8_tests.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

class Aj8InventoryTests(unittest.TestCase):
    def test_nonempty_exact_execution_and_authored_rows(self):
        result = probe.verify_inventory("anchor: test\nsecond: test\n", "test anchor ... parameter_rows=2\nok\ntest second ... ok\n", "anchor", [2])
        self.assertEqual(result["executed"], ["anchor", "second"])

    def test_empty_missing_extra_failed_ignored_and_row_mismatch_fail(self):
        cases = [
            ("", "", "anchor", []),
            ("other: test\n", "test other ... ok\n", "anchor", []),
            ("anchor: test\n", "test anchor ... ok\ntest extra ... ok\n", "anchor", []),
            ("anchor: test\n", "test anchor ... FAILED\n", "anchor", []),
            ("anchor: test\n", "test anchor ... ignored\n", "anchor", []),
            ("anchor: test\n", "test anchor ... parameter_rows=1\nok\n", "anchor", [2]),
            ("anchor: test\nanchor: test\n", "test anchor ... ok\n", "anchor", []),
        ]
        for case in cases:
            with self.subTest(case=case), self.assertRaises(RuntimeError):
                probe.verify_inventory(*case)

    def test_every_authored_member_is_required_even_with_anchor_and_rows(self):
        # Synthetic parser-verifier inputs, not engine execution evidence.
        for package, required in probe.REQUIRED_TESTS.items():
            anchor = probe.ANCHORS.get(package, required[0])
            names = sorted(set(required))
            rows = probe.PARAMETER_ROWS[package]
            markers = "".join(f"parameter_rows={count}\n" for count in rows)
            for removed in names:
                remaining = [name for name in names if name != removed]
                listed = "".join(name + ": test\n" for name in remaining)
                output = markers + "".join("test " + name + " ... ok\n" for name in remaining)
                with self.subTest(package=package, removed=removed), self.assertRaises(RuntimeError):
                    probe.verify_inventory(listed, output, anchor, rows, required)
            listed = "".join(name + ": test\n" for name in names)
            output = markers + "".join("test " + name + " ... ok\n" for name in names)
            probe.verify_inventory(listed, output, anchor, rows, required)

if __name__ == "__main__":
    unittest.main()
