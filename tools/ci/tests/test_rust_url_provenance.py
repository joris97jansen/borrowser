"""Qualified original rust-url bytes and modes in disposable physical trees."""

import copy
import hashlib
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
import check_dependency_subtrees as subtrees


class RustUrlProvenance(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="rust-url-source-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.vendor = self.root / "vendor/rust-url"
        shutil.copytree(check.ROOT / "vendor/rust-url", self.vendor, symlinks=True)
        policy = self.root / subtrees.POLICY_PATH
        policy.parent.mkdir(parents=True)
        shutil.copyfile(check.ROOT / subtrees.POLICY_PATH, policy)
        self.policy = subtrees.load_qualification(self.root)

    def test_original_source_and_all_eight_links_pass_without_changes(self):
        before = {p: e.copy() for p, e in self.policy["dependencies"]["rust-url"]["files"].items()}
        result = check.check_sources(self.root)
        self.assertEqual(result["files"], before)
        self.assertEqual(len(before), 80)
        self.assertEqual(sum(e["mode"] == "120000" for e in before.values()), 8)

    def test_each_regular_source_corruption_is_rejected(self):
        for name in ("url/src/parser.rs", "url/tests/domain.rs", "idna/src/lib.rs", "LICENSE-MIT"):
            with self.subTest(path=name):
                path = self.vendor / name
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                with self.assertRaisesRegex(ValueError, "source hash"):
                    check.check_sources(self.root)
                path.write_bytes(original)

    def test_unexpected_files_and_directories_are_rejected(self):
        for directory in (False, True):
            path = self.vendor / "unexpected"
            path.mkdir() if directory else path.write_text("unqualified")
            with self.assertRaisesRegex(ValueError, "unexpected source"):
                check.check_sources(self.root)
            path.rmdir() if directory else path.unlink()

    def test_missing_source_is_rejected(self):
        (self.vendor / "url/src/parser.rs").unlink()
        with self.assertRaisesRegex(ValueError, "inventory"):
            check.check_sources(self.root)

    def test_executable_mode_mutation_is_rejected(self):
        path = self.vendor / "url/src/parser.rs"
        path.chmod(path.stat().st_mode | 0o100)
        with self.assertRaisesRegex(ValueError, "mode/type"):
            check.check_sources(self.root)

    def test_changed_and_escaping_symlink_targets_are_rejected(self):
        path = self.vendor / "url/LICENSE-MIT"
        for target in ("../LICENSE-APACHE", "../../../outside", "/tmp/outside"):
            path.unlink()
            path.symlink_to(target)
            with self.assertRaisesRegex(ValueError, "target changed"):
                check.check_sources(self.root)

    def test_materialized_or_link_text_license_is_not_an_original_link(self):
        for contents in (b"../LICENSE-MIT", (self.vendor / "LICENSE-MIT").read_bytes()):
            path = self.vendor / "url/LICENSE-MIT"
            path.unlink()
            path.write_bytes(contents)
            with self.assertRaisesRegex(ValueError, "original license symlink"):
                check.check_sources(self.root)

    def test_source_directory_symlinks_are_rejected_without_following(self):
        path = self.vendor / "url/src"
        saved = self.root / "saved"
        path.rename(saved)
        path.symlink_to(saved, target_is_directory=True)
        with self.assertRaises(ValueError):
            check.check_sources(self.root)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "requires POSIX FIFO")
    def test_nonregular_source_is_rejected_without_opening(self):
        path = self.vendor / "url/src/parser.rs"
        path.unlink()
        os.mkfifo(path)
        with self.assertRaisesRegex(ValueError, "mode/type"):
            check.check_sources(self.root)

    def test_record_changes_cannot_authorize_source_or_inventory_changes(self):
        path = self.vendor / "url/src/parser.rs"
        path.write_bytes(path.read_bytes() + b"\n// unreviewed\n")
        record = copy.deepcopy(self.policy)
        record["dependencies"]["rust-url"]["files"]["url/src/parser.rs"]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        (self.root / subtrees.POLICY_PATH).write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "reviewed policy"):
            check.check_sources(self.root)

    def test_reviewed_fixture_policy_advancement_accepts_source_without_bypass(self):
        # Mechanism evidence only: no behavior or real owner-authorization claim.
        path = self.vendor / "url/src/parser.rs"
        path.write_bytes(path.read_bytes() + b"\n// fixture qualification\n")
        with self.assertRaisesRegex(ValueError, "source hash"):
            check.check_sources(self.root)
        record = copy.deepcopy(self.policy)
        record["dependencies"]["rust-url"]["files"]["url/src/parser.rs"]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        file = self.root / subtrees.POLICY_PATH
        file.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "reviewed policy"):
            check.check_sources(self.root)
        with mock.patch.object(subtrees, "POLICY_SHA256", hashlib.sha256(file.read_bytes()).hexdigest()):
            check.check_sources(self.root)
            # Advancing content policy does not disable mode admission.
            path.chmod(path.stat().st_mode | 0o100)
            with self.assertRaisesRegex(ValueError, "mode/type"):
                check.check_sources(self.root)

    def test_duplicate_json_keys_and_unsafe_policy_paths_are_rejected(self):
        file = self.root / subtrees.POLICY_PATH
        for contents in ('{"schema":1,"schema":1}', None):
            if contents is None:
                record = copy.deepcopy(self.policy)
                record["dependencies"]["rust-url"]["files"]["../escape"] = {"mode":"100644", "sha256":"0" * 64}
                contents = json.dumps(record)
            file.write_text(contents)
            with mock.patch.object(subtrees, "POLICY_SHA256", hashlib.sha256(file.read_bytes()).hexdigest()):
                with self.assertRaises(ValueError):
                    subtrees.load_qualification(self.root)
