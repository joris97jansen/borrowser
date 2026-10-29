"""Focused negative cases over real Cargo metadata; manifests/locks stay untouched."""

import copy
import json
from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_url_dependencies as check


class QualifiedUrlDependencies(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[3]
        cls.resolutions = {
            "main": check.load_resolution(root / "Cargo.toml"),
            "fuzz": check.load_resolution(root / "fuzz/Cargo.toml"),
        }

    def test_approved_main_and_fuzz_pass(self):
        self.assertEqual(check.APPROVED_REVISION, "7dc4ecbf4d29817e723e04ae94b14f1326764f61")
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                check.verify_resolution(metadata, lock, label)

    def test_ci_control_revision_is_rejected_even_with_matching_lock(self):
        wrong = "32630e7b55f8ad75a3f92e96a48552dac755f35c"
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                # Change metadata identities and their lock entries together.
                # The CI expectation must remain independent of both.
                changed_metadata = json.loads(json.dumps(metadata).replace(check.APPROVED_REVISION, wrong))
                changed_lock = json.loads(json.dumps(lock).replace(check.APPROVED_REVISION, wrong))
                with self.assertRaisesRegex(ValueError, f"{label}: url must resolve"):
                    check.verify_resolution(changed_metadata, changed_lock, label)

    def test_other_git_revision_and_repository_are_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            for original, replacement in [
                (check.APPROVED_REVISION, "0" * 40),
                (check.APPROVED_REPOSITORY, "https://github.com/servo/rust-url.git"),
            ]:
                with self.subTest(workspace=label, replacement=replacement):
                    changed = json.loads(json.dumps(metadata).replace(original, replacement))
                    # A correct-looking real lock cannot substitute for Cargo's
                    # actual source selection in the metadata response.
                    with self.assertRaisesRegex(ValueError, "url must resolve"):
                        check.verify_resolution(changed, lock, label)

    def test_registry_url_is_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                changed = copy.deepcopy(metadata)
                package = next(p for p in changed["packages"] if p["name"] == "url")
                package["source"] = check.REGISTRY_SOURCE
                with self.assertRaisesRegex(ValueError, "url must resolve"):
                    check.verify_resolution(changed, lock, label)

    def test_registry_duplicate_is_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            for name in check.FORK_VERSIONS:
                with self.subTest(workspace=label, package=name):
                    changed = copy.deepcopy(metadata)
                    duplicate = copy.deepcopy(next(p for p in changed["packages"] if p["name"] == name))
                    duplicate["id"] = f"{check.REGISTRY_SOURCE}#{name}@{duplicate['version']}"
                    duplicate["source"] = check.REGISTRY_SOURCE
                    changed["packages"].append(duplicate)
                    with self.assertRaisesRegex(ValueError, f"expected one {name} instance"):
                        check.verify_resolution(changed, lock, label)

    def test_changed_qualified_version_is_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                changed = copy.deepcopy(metadata)
                package = next(p for p in changed["packages"] if p["name"] == "smallvec")
                package["version"] = "1.16.4"
                with self.assertRaisesRegex(ValueError, "qualified URL closure changed"):
                    check.verify_resolution(changed, lock, label)

    def test_changed_checksum_is_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                changed = copy.deepcopy(lock)
                package = next(p for p in changed["package"] if p["name"] == "smallvec")
                package["checksum"] = "0" * 64
                with self.assertRaisesRegex(ValueError, "qualified checksum mismatch for smallvec"):
                    check.verify_resolution(metadata, changed, label)

    def test_missing_runtime_edge_is_rejected(self):
        for label, (metadata, lock) in self.resolutions.items():
            with self.subTest(workspace=label):
                changed = copy.deepcopy(metadata)
                package_id = next(p["id"] for p in changed["packages"] if p["name"] == "utf8_iter")
                for node in changed["resolve"]["nodes"]:
                    node["deps"] = [d for d in node["deps"] if d["pkg"] != package_id]
                with self.assertRaisesRegex(ValueError, "qualified URL closure changed"):
                    check.verify_resolution(changed, lock, label)

    def test_each_workspace_failure_is_propagated_by_the_ci_entrypoint(self):
        for failing in ["main", "fuzz"]:
            with self.subTest(failing=failing):
                def workspace_check(manifest, label):
                    if label == failing:
                        raise ValueError(f"{label}: injected invalid source")

                with mock.patch.object(check, "check_workspace", side_effect=workspace_check) as workspace:
                    with self.assertRaisesRegex(ValueError, f"{failing}: injected invalid source"):
                        check.check_repo(Path("/unused"))
                    labels = [call.args[1] for call in workspace.call_args_list]
                    self.assertEqual(labels, ["main"] if failing == "main" else ["main", "fuzz"])


if __name__ == "__main__":
    unittest.main()
