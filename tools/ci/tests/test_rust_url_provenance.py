"""Exercise the real frozen snapshot in disposable trees; no network or Cargo."""

import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_url_dependencies as check


class RustUrlProvenance(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="rust-url-integrity-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.vendor = self.root / "vendor/rust-url"
        shutil.copytree(check.ROOT / "vendor/rust-url", self.vendor)

    def rejected_before_contents(self, message):
        with (
            mock.patch.object(Path, "read_bytes", side_effect=AssertionError("content read")),
            mock.patch.object(check.subprocess, "run", side_effect=AssertionError("process started")),
        ):
            with self.assertRaisesRegex(ValueError, message):
                check.check_sources(self.root)

    def test_approved_snapshot_reconstructs_without_changing_it(self):
        before = check.file_hashes(check.inventory(self.vendor)[0])
        record = check.check_sources(self.root)
        self.assertEqual(len(record["fork"]["git_files"]), 80)
        self.assertEqual(len(record["upstream"]["git_files"]), 79)
        self.assertEqual(len(record["contributions"]), 11)
        self.assertEqual(len(record["fork"]["license_symlinks"]), 8)
        self.assertEqual(before, check.file_hashes(check.inventory(self.vendor)[0]))

    def test_original_and_added_source_corruption_are_rejected(self):
        for name in ("url/src/parser.rs", "url/tests/domain.rs", "idna/src/lib.rs"):
            with self.subTest(name=name):
                path = self.vendor / name
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                with self.assertRaisesRegex(ValueError, "source hash"):
                    check.check_sources(self.root)
                path.write_bytes(original)

    def test_source_and_record_deletions_are_rejected_before_reads(self):
        for name in ("url/src/parser.rs", "UPSTREAM.json"):
            with self.subTest(name=name):
                path = self.vendor / name
                original = path.read_bytes()
                path.unlink()
                self.rejected_before_contents("inventory changed")
                path.write_bytes(original)

    def test_additional_file_is_rejected_before_reads(self):
        (self.vendor / "UNEXPECTED").write_bytes(b"unchecked")
        self.rejected_before_contents("inventory changed")

    def test_additional_empty_directory_is_rejected_before_reads(self):
        (self.vendor / "UNEXPECTED").mkdir()
        self.rejected_before_contents("inventory changed")

    @unittest.skipIf(os.name == "nt", "Git executable mode admission requires POSIX")
    def test_executable_mode_change_is_rejected_before_reads(self):
        (self.vendor / "url/src/parser.rs").chmod(0o755)
        self.rejected_before_contents("unexpected executable")

    def test_each_record_is_independently_frozen(self):
        for name in check.RECORD_HASHES:
            with self.subTest(name=name):
                path = self.vendor / name
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                with self.assertRaisesRegex(ValueError, "frozen record"):
                    check.check_sources(self.root)
                path.write_bytes(original)

    def test_recalculated_provenance_does_not_authorize_changed_source(self):
        path = self.vendor / "url/src/parser.rs"
        path.write_bytes(path.read_bytes() + b"\n")
        record = json.loads((self.vendor / "UPSTREAM.json").read_text())
        record["fork"]["materialized_files"]["url/src/parser.rs"] = check.digest(path.read_bytes())
        (self.vendor / "UPSTREAM.json").write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "frozen record"):
            check.check_sources(self.root)

    def test_malformed_and_duplicate_key_json_are_rejected(self):
        for data in (b"{", b'{"schema":1,"schema":1}'):
            with self.subTest(data=data), self.assertRaises(ValueError):
                check.read_json(data)

    def test_license_materialization_bytes_are_frozen(self):
        (self.vendor / "url/LICENSE-MIT").write_bytes(b"../LICENSE-MIT")
        with self.assertRaisesRegex(ValueError, "source hash"):
            check.check_sources(self.root)

    def test_every_relevant_symlink_boundary_is_rejected_before_reads(self):
        for name in ("vendor", "vendor/rust-url", "vendor/rust-url/url",
                     "vendor/rust-url/url/src/parser.rs", "vendor/rust-url/UPSTREAM.json"):
            with self.subTest(name=name):
                path = self.root / name
                saved = self.root / "saved-entry"
                path.rename(saved)
                path.symlink_to(saved, target_is_directory=saved.is_dir())
                self.rejected_before_contents("symlink")
                path.unlink()
                saved.rename(path)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO requires POSIX")
    def test_nonregular_record_is_rejected_before_opening_it(self):
        path = self.vendor / "UPSTREAM.json"
        path.unlink()
        os.mkfifo(path)
        self.rejected_before_contents("nonregular")

    def test_original_license_symlinks_are_never_admitted(self):
        path = self.vendor / "url/LICENSE-MIT"
        path.unlink()
        path.symlink_to("../LICENSE-MIT")
        self.rejected_before_contents("symlink")

    def test_missing_reverse_reconstruction_is_rejected(self):
        with mock.patch.object(check, "apply_patch"):
            with self.assertRaisesRegex(ValueError, "reconstruct upstream"):
                check.check_sources(self.root)

    def test_bad_forward_replay_is_rejected(self):
        real_apply = check.apply_patch

        def corrupt_forward(directory, patch, reverse=False):
            real_apply(directory, patch, reverse)
            if not reverse:
                (directory / "url/src/lib.rs").write_bytes(b"incorrect replay")

        with mock.patch.object(check, "apply_patch", side_effect=corrupt_forward):
            with self.assertRaisesRegex(ValueError, "replay differs"):
                check.check_sources(self.root)

    def test_invalid_contribution_object_and_topology_are_rejected(self):
        original = json.loads((self.vendor / "UPSTREAM.json").read_text())
        record = copy.deepcopy(original)
        record["contributions"][0]["object"] += "altered"
        with self.assertRaisesRegex(ValueError, "contribution object"):
            check.verify_contributions(record)
        record = copy.deepcopy(original)
        record["contributions"].reverse()
        with self.assertRaisesRegex(ValueError, "topology"):
            check.verify_contributions(record)


if __name__ == "__main__":
    unittest.main()
