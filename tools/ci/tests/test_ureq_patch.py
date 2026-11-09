"""The dependency source check must reject changes outside the reviewed patch."""

import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("check_ureq_patch", ROOT / "tools/ci/check_ureq_patch.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class UreqPatchIntegrity(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        shutil.copytree(ROOT / "vendor/ureq", self.root / "vendor/ureq")

    def test_reviewed_sources_pass(self):
        CHECK.check_sources(self.root)

    def assert_inventory_rejected_without_reads(self, message):
        # These are tripwires, not replacements for admission logic: call the
        # real checker and fail immediately if it reaches any content/process I/O.
        with (
            mock.patch.object(Path, "open", side_effect=AssertionError("content opened")) as opened,
            mock.patch.object(Path, "read_bytes", side_effect=AssertionError("content read")) as read,
            mock.patch.object(CHECK.subprocess, "run", side_effect=AssertionError("process started")) as process,
        ):
            with self.assertRaisesRegex(ValueError, message):
                CHECK.check_sources(self.root)
        opened.assert_not_called()
        read.assert_not_called()
        process.assert_not_called()

    def test_vendor_root_symlink_is_rejected_before_provenance_reads(self):
        external = tempfile.TemporaryDirectory()
        self.addCleanup(external.cleanup)
        source = self.root / "vendor/ureq"
        target = Path(external.name) / "ureq"
        source.rename(target)
        source.symlink_to(target, target_is_directory=True)
        self.assertFalse(target.is_relative_to(self.root))
        self.assert_inventory_rejected_without_reads("symlink")

    def test_vendor_parent_symlink_is_rejected_before_provenance_reads(self):
        external = tempfile.TemporaryDirectory()
        self.addCleanup(external.cleanup)
        source = self.root / "vendor"
        target = Path(external.name) / "vendor"
        source.rename(target)
        source.symlink_to(target, target_is_directory=True)
        self.assertFalse(target.is_relative_to(self.root))
        self.assert_inventory_rejected_without_reads("symlink")

    def test_provenance_record_symlinks_are_rejected_before_reads(self):
        external = tempfile.TemporaryDirectory()
        self.addCleanup(external.cleanup)
        for name in sorted(CHECK.RECORD_FILES):
            with self.subTest(record=name):
                source = self.root / "vendor/ureq" / name
                target = Path(external.name) / name
                source.rename(target)
                source.symlink_to(target)
                try:
                    self.assert_inventory_rejected_without_reads("symlink")
                finally:
                    source.unlink()
                    target.rename(source)

    def test_unexpected_file_is_rejected_before_any_content_or_process_io(self):
        unexpected = self.root / "vendor/ureq/UNEXPECTED"
        unexpected.write_bytes(b"must never be opened by the verifier")
        self.assert_inventory_rejected_without_reads("file inventory changed")

    def test_duplicate_inventory_entry_is_rejected_before_reads(self):
        source = self.root / "vendor/ureq/src/header.rs"
        # A real directory cannot hold duplicate names. Inject a duplicate
        # discovery result, retaining actual filesystem types and real checking.
        with mock.patch.object(Path, "rglob", return_value=iter([source, source])):
            self.assert_inventory_rejected_without_reads("duplicate entry")

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO qualification requires POSIX")
    def test_nonregular_provenance_record_is_rejected_without_opening(self):
        source = self.root / "vendor/ureq/UPSTREAM.json"
        source.unlink()
        os.mkfifo(source)
        self.assert_inventory_rejected_without_reads("unsupported entry")

    def test_unpatched_source_change_is_rejected(self):
        source = self.root / "vendor/ureq/src/middleware.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        with self.assertRaisesRegex(ValueError, "source hash mismatch"):
            CHECK.check_sources(self.root)

    def test_patched_source_change_is_rejected(self):
        source = self.root / "vendor/ureq/src/header.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        with self.assertRaisesRegex(ValueError, "source hash mismatch"):
            CHECK.check_sources(self.root)

    def test_unreviewed_file_is_rejected(self):
        (self.root / "vendor/ureq/src/extra.rs").write_text("// unreviewed file\n")
        with self.assertRaisesRegex(ValueError, "file inventory changed"):
            CHECK.check_sources(self.root)

    def test_rewriting_the_patch_record_cannot_admit_unreviewed_source(self):
        source = self.root / "vendor/ureq/src/header.rs"
        source.write_bytes(source.read_bytes() + b"\n// unreviewed change\n")
        path = self.root / "vendor/ureq/UPSTREAM.json"
        record = json.loads(path.read_text())
        record["patched_files"]["src/header.rs"] = hashlib.sha256(source.read_bytes()).hexdigest()
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "reviewed patch record changed"):
            CHECK.check_sources(self.root)

    def change_record(self, change):
        path = self.root / "vendor/ureq/UPSTREAM.json"
        record = json.loads(path.read_text())
        change(record)
        path.write_text(json.dumps(record))

    def test_patched_manifest_change_is_rejected(self):
        path = self.root / "vendor/ureq/Cargo.toml"
        path.write_text(path.read_text() + "\n# unreviewed feature change\n")
        with self.assertRaisesRegex(ValueError, "source hash mismatch"):
            CHECK.check_sources(self.root)

    def test_missing_source_is_rejected(self):
        (self.root / "vendor/ureq/src/unit.rs").unlink()
        with self.assertRaisesRegex(ValueError, "file inventory changed"):
            CHECK.check_sources(self.root)

    def test_omitted_upstream_file_cannot_be_reintroduced(self):
        (self.root / "vendor/ureq/Cargo.lock").write_text("# unexpected lockfile")
        with self.assertRaisesRegex(ValueError, "file inventory changed"):
            CHECK.check_sources(self.root)

    def test_archive_identity_is_frozen(self):
        self.change_record(lambda r: r.update(archive_sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "source/patch identity"):
            CHECK.check_sources(self.root)

    def test_published_vcs_identity_is_frozen(self):
        self.change_record(lambda r: r.update(upstream_git="0" * 40))
        with self.assertRaisesRegex(ValueError, "source/patch identity"):
            CHECK.check_sources(self.root)

    def test_published_manifest_cannot_lose_an_entry(self):
        self.change_record(lambda r: r["upstream_files"].pop("LICENSE-MIT"))
        with self.assertRaisesRegex(ValueError, "published file manifest changed"):
            CHECK.check_sources(self.root)

    def test_patch_inventory_cannot_gain_an_entry(self):
        self.change_record(lambda r: r["patched_files"].update({"src/extra.rs": "0" * 64}))
        with self.assertRaisesRegex(ValueError, "source/patch identity"):
            CHECK.check_sources(self.root)

    def test_patch_inventory_cannot_lose_an_entry(self):
        self.change_record(lambda r: r["patched_files"].pop("Cargo.toml.orig"))
        with self.assertRaisesRegex(ValueError, "source/patch identity"):
            CHECK.check_sources(self.root)

    def test_omission_inventory_is_frozen(self):
        self.change_record(lambda r: r["omitted_files"].remove("Cargo.lock"))
        with self.assertRaisesRegex(ValueError, "published file manifest changed"):
            CHECK.check_sources(self.root)

    def test_cumulative_patch_bytes_are_pinned(self):
        path = self.root / "vendor/ureq" / CHECK.PATCH_FILE
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "reviewed patch record changed"):
            CHECK.check_sources(self.root)

    def test_historical_patch_bytes_are_pinned(self):
        path = self.root / "vendor/ureq/response-header.patch"
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "reviewed patch record changed"):
            CHECK.check_sources(self.root)

    def test_patch_reconstruction_checks_upstream_not_only_patch_digest(self):
        path = self.root / "vendor/ureq" / CHECK.PATCH_FILE
        original = path.read_text()
        changed = original.replace('-    "logging",', '-    "not-the-upstream-flag",', 1)
        self.assertNotEqual(changed, original)
        path.write_text(changed)
        # Even a separately reviewed new patch digest cannot attest an invented
        # upstream baseline. The independently frozen upstream hashes still win.
        with mock.patch.object(CHECK, "PATCH_SHA256", hashlib.sha256(path.read_bytes()).hexdigest()):
            with self.assertRaisesRegex(ValueError, "does not reconstruct upstream"):
                CHECK.check_sources(self.root)

    def test_symlink_source_is_rejected(self):
        path = self.root / "vendor/ureq/src/header.rs"
        outside = self.root / "header.rs"
        path.rename(outside)
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "symlink"):
            CHECK.check_sources(self.root)

    def test_wrong_archive_bytes_are_rejected(self):
        path = self.root / "ureq.crate"
        path.write_bytes(b"not the published archive")
        with self.assertRaisesRegex(ValueError, "archive identity mismatch"):
            CHECK.check_archive(path, json.loads((self.root / "vendor/ureq/UPSTREAM.json").read_text()))


class UreqResolutionIntegrity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # One real all-feature/all-target metadata result. Negative cases mutate
        # that selected graph; they never edit the actual workspace or lockfile.
        cls.metadata = json.loads(subprocess.run(
            ["cargo", "metadata", "--all-features", "--locked", "--offline", "--format-version", "1"],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout)

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / "crates/net").mkdir(parents=True)
        for name in ["Cargo.toml", "Cargo.lock", "crates/net/Cargo.toml"]:
            shutil.copyfile(ROOT / name, self.root / name)
        self.graph = json.loads(json.dumps(self.metadata))
        self.package("ureq")["manifest_path"] = str(self.root / "vendor/ureq/Cargo.toml")

    def package(self, name):
        return next(p for p in self.graph["packages"] if p["name"] == name)

    def node(self, name):
        return next(n for n in self.graph["resolve"]["nodes"] if n["id"] == self.package(name)["id"])

    def check(self):
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps(self.graph))
        with mock.patch.object(CHECK.subprocess, "run", return_value=result):
            CHECK.check_resolution(self.root)

    def test_actual_workspace_selection_passes(self):
        CHECK.check_resolution(ROOT)

    def test_selected_graph_fixture_passes(self):
        self.check()

    def test_wrong_workspace_pin_is_rejected(self):
        path = self.root / "Cargo.toml"
        path.write_text(path.read_text().replace('version = "=2.12.1"', 'version = "=2.12.0"'))
        with self.assertRaisesRegex(ValueError, "exact local"):
            self.check()

    def test_wrong_cargo_patch_route_is_rejected(self):
        path = self.root / "Cargo.toml"
        path.write_text(path.read_text().replace('path = "vendor/ureq"', 'path = "other/ureq"'))
        with self.assertRaisesRegex(ValueError, "exact local"):
            self.check()

    def test_wrong_lock_version_is_rejected(self):
        path = self.root / "Cargo.lock"
        path.write_text(path.read_text().replace('name = "ureq"\nversion = "2.12.1"',
                                               'name = "ureq"\nversion = "2.12.0"'))
        with self.assertRaisesRegex(ValueError, "locked ureq"):
            self.check()

    def test_registry_lock_entry_cannot_bypass_local_patch(self):
        path = self.root / "Cargo.lock"
        path.write_text(path.read_text().replace('name = "ureq"\nversion = "2.12.1"',
                                               'name = "ureq"\nversion = "2.12.1"\nsource = "registry+other"'))
        with self.assertRaisesRegex(ValueError, "locked ureq"):
            self.check()

    def test_resolved_registry_ureq_is_rejected(self):
        self.package("ureq")["source"] = "registry+other"
        with self.assertRaisesRegex(ValueError, "resolved ureq"):
            self.check()

    def test_resolved_wrong_local_ureq_is_rejected(self):
        self.package("ureq")["manifest_path"] = str(self.root / "other/Cargo.toml")
        with self.assertRaisesRegex(ValueError, "resolved ureq"):
            self.check()

    def test_resolved_wrong_version_is_rejected(self):
        self.package("ureq")["version"] = "2.12.0"
        with self.assertRaisesRegex(ValueError, "resolved ureq"):
            self.check()

    def test_automatic_content_decoding_features_are_rejected(self):
        for feature in ["gzip", "brotli"]:
            with self.subTest(feature=feature):
                self.node("ureq")["features"].append(feature)
                with self.assertRaisesRegex(ValueError, "unapproved ureq features"):
                    self.check()
                self.node("ureq")["features"].remove(feature)

    def test_rustls_logging_or_provider_changes_are_rejected(self):
        for feature in ["logging", "aws_lc_rs"]:
            with self.subTest(feature=feature):
                self.node("rustls")["features"].append(feature)
                with self.assertRaisesRegex(ValueError, "unapproved rustls"):
                    self.check()
                self.node("rustls")["features"].remove(feature)

    def test_wrong_decoder_version_is_rejected(self):
        self.package("flate2")["version"] = "1.1.1"
        with self.assertRaisesRegex(ValueError, "resolved net decoder"):
            self.check()

    def test_net_must_own_normal_decoder_dependency(self):
        self.node("net")["deps"] = [d for d in self.node("net")["deps"] if d["name"] != "flate2"]
        with self.assertRaisesRegex(ValueError, "resolved net decoder"):
            self.check()


if __name__ == "__main__":
    unittest.main()
