"""Actual offline Cargo selection plus deterministic source/graph mutations."""

import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import subprocess
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_url_dependencies as check


class QualifiedUrlDependencies(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.resolutions = {label: check.load_resolution(check.ROOT / path) for label, path in
                           (("main", "Cargo.toml"), ("fuzz", "fuzz/Cargo.toml"))}

    def mutated(self, mutate, message):
        for label, original in self.resolutions.items():
            with self.subTest(workspace=label):
                metadata, lock = copy.deepcopy(original)
                mutate(metadata, lock)
                with self.assertRaisesRegex(ValueError, message):
                    check.verify_resolution(metadata, lock, label)

    @staticmethod
    def package(metadata, name):
        return next(p for p in metadata["packages"] if p["name"] == name)

    def test_actual_approved_main_and_fuzz_pass(self):
        self.assertEqual(check.APPROVED_REVISION, "92f7fb395f9ffb2f706e7c664283781fd3b294a2")
        for label, (metadata, lock) in self.resolutions.items():
            check.verify_resolution(metadata, lock, label)

    def test_registry_and_git_fallbacks_for_every_fork_package_are_rejected(self):
        for name in check.FORK_VERSIONS:
            for source in (check.REGISTRY_SOURCE, check.APPROVED_SOURCE,
                           check.APPROVED_SOURCE.replace(check.APPROVED_REVISION, "823ba428f3a9883e8873e91d6cf46b7ac822c0a7"),
                           "git+https://github.com/servo/rust-url.git#" + check.UPSTREAM_REVISION):
                with self.subTest(package=name, source=source):
                    self.mutated(lambda m, l: self.package(m, name).__setitem__("source", source), "approved local manifest")

    def test_alternate_local_manifest_is_rejected(self):
        for name in check.FORK_VERSIONS:
            self.mutated(lambda m, l: self.package(m, name).__setitem__("manifest_path", "/tmp/alternate/Cargo.toml"),
                         "approved local manifest")

    def test_duplicates_of_every_fork_package_are_rejected(self):
        for name in check.FORK_VERSIONS:
            def duplicate(metadata, lock):
                package = copy.deepcopy(self.package(metadata, name))
                package["id"] += "-duplicate"
                metadata["packages"].append(package)
                node = copy.deepcopy(next(n for n in metadata["resolve"]["nodes"] if n["id"] == self.package(metadata, name)["id"]))
                node["id"] = package["id"]
                metadata["resolve"]["nodes"].append(node)
            self.mutated(duplicate, f"expected one {name} instance")

    def test_fork_version_and_closure_version_changes_are_rejected(self):
        for name in ("url", "idna", "smallvec"):
            self.mutated(lambda m, l: self.package(m, name).__setitem__("version", "99.0.0"),
                         "unexpected .* version|closure changed")

    def test_registry_checksum_change_is_rejected(self):
        def corrupt(metadata, lock):
            next(p for p in lock["package"] if p["name"] == "smallvec")["checksum"] = "0" * 64
        self.mutated(corrupt, "checksum mismatch for smallvec")

    def test_registry_directory_replacements_in_each_workspace_are_rejected(self):
        for label, original in self.resolutions.items():
            # Each workspace/package starts with its own accepted resolution.
            for name in check.REGISTRY_PACKAGES:
                with self.subTest(workspace=label, package=name):
                    metadata, lock = copy.deepcopy(original)
                    check.verify_resolution(metadata, lock, label)
                    package = self.package(metadata, name)
                    before = copy.deepcopy(package)
                    package["manifest_path"] = f"/unapproved-directory-source/{name}/Cargo.toml"
                    self.assertEqual({**package, "manifest_path": before["manifest_path"]}, before)
                    self.assertEqual(lock, original[1])
                    with self.assertRaisesRegex(ValueError, f"{label}: unapproved registry origin for {name}:"):
                        check.verify_resolution(metadata, lock, label)
                    check.verify_resolution(*original, label)

    def test_removed_runtime_edge_is_rejected(self):
        def remove(metadata, lock):
            url = self.package(metadata, "url")["id"]
            node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == url)
            node["deps"] = [d for d in node["deps"] if d["name"] != "percent_encoding"]
        self.mutated(remove, "edges or features changed")

    def test_changed_alias_kind_or_target_is_rejected(self):
        for field in ("alias", "kind", "target"):
            def change(metadata, lock):
                node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == self.package(metadata, "url")["id"])
                dep = node["deps"][0]
                if field == "alias":
                    dep["name"] = "alternate_alias"
                else:
                    dep["dep_kinds"][0][field] = "build" if field == "kind" else "cfg(windows)"
            self.mutated(change, "edges or features changed")

    def test_redirected_and_added_edges_are_rejected_without_changing_package_set(self):
        def redirect(metadata, lock):
            form = self.package(metadata, "form_urlencoded")["id"]
            node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == form)
            node["deps"][0]["pkg"] = self.package(metadata, "url")["id"]
        self.mutated(redirect, "edges or features changed")
        def add(metadata, lock):
            node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == self.package(metadata, "url")["id"])
            node["deps"].append(copy.deepcopy(node["deps"][0]))
        self.mutated(add, "edges or features changed")

    def test_features_are_frozen_per_workspace(self):
        for name in ("url", "percent-encoding", "serde", "smallvec", "syn"):
            def change(metadata, lock):
                node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == self.package(metadata, name)["id"])
                node["features"].append("unapproved-feature")
            self.mutated(change, "edges or features changed")

    def test_workspace_feature_baselines_are_not_interchangeable(self):
        graphs = check.load_graph_policy(check.ROOT)["graphs"]
        for label, (metadata, lock) in self.resolutions.items():
            with self.assertRaisesRegex(ValueError, "edges or features changed"):
                check.verify_resolution(metadata, lock, label, expected_graph=graphs["fuzz" if label == "main" else "main"])

    def test_incomplete_and_duplicate_metadata_are_rejected(self):
        self.mutated(lambda m, l: m["resolve"]["nodes"].pop(), "incomplete Cargo")
        self.mutated(lambda m, l: m["resolve"]["nodes"].append(m["resolve"]["nodes"][0]), "duplicate Cargo")

    def test_vendor_membership_and_nonlocal_lock_are_rejected(self):
        self.mutated(lambda m, l: m["workspace_members"].append(self.package(m, "url")["id"]), "joined consumer workspace")
        def source(metadata, lock):
            next(p for p in lock["package"] if p["name"] == "url")["source"] = check.REGISTRY_SOURCE
        self.mutated(source, "missing from lock")

    def test_unselected_vendor_workspace_member_cannot_join_borrowser(self):
        def add(metadata, lock):
            package = {"id": "fixture-data-url", "name": "data-url", "version": "0.3.2",
                       "source": None, "manifest_path": str(check.ROOT / "vendor/rust-url/data-url/Cargo.toml")}
            metadata["packages"].append(package)
            metadata["resolve"]["nodes"].append({"id": package["id"], "features": [], "deps": []})
            metadata["workspace_members"].append(package["id"])
        self.mutated(add, "joined consumer workspace")

    def test_metadata_is_explicitly_locked_and_offline(self):
        real_run = check.subprocess.run
        with mock.patch.object(check.subprocess, "run", wraps=real_run) as run:
            check.load_resolution(check.ROOT / "fuzz/Cargo.toml")
        arguments = run.call_args.args[0]
        self.assertIn("--offline", arguments)
        self.assertIn("--locked", arguments)

    def test_integrity_failure_stops_the_mandatory_gate_before_cargo(self):
        with (
            mock.patch.object(check, "check_sources", side_effect=ValueError("source corruption")),
            mock.patch.object(check, "load_resolution") as cargo,
        ):
            with self.assertRaisesRegex(ValueError, "source corruption"):
                check.check_repo(check.ROOT)
        cargo.assert_not_called()

    def test_either_workspace_failure_fails_the_mandatory_entrypoint(self):
        for failing in ("main", "fuzz"):
            def reject(manifest, label, *arguments):
                if label == failing:
                    raise ValueError("injected " + failing + " graph failure")
            with mock.patch.object(check, "check_workspace", side_effect=reject):
                with self.assertRaisesRegex(ValueError, "injected " + failing):
                    check.check_repo(check.ROOT)


class CargoConfiguration(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="url-configuration-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in ("Cargo.toml", "Cargo.lock", "fuzz/Cargo.toml", "fuzz/Cargo.lock"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(check.ROOT / name, target)
        for directory in check.LOCAL_CRATES.values():
            target = self.root / "vendor/rust-url" / directory / "Cargo.toml"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(check.ROOT / "vendor/rust-url" / directory / "Cargo.toml", target)

    def test_actual_configuration_records_pass(self):
        for label in ("main", "fuzz"):
            check.check_workspace_records(self.root, label)

    def test_wrong_patch_in_either_workspace_is_rejected(self):
        for label, name in (("main", "Cargo.toml"), ("fuzz", "fuzz/Cargo.toml")):
            path = self.root / name
            original = path.read_text()
            path.write_text(original.replace('vendor/rust-url/url"', 'vendor/alternate/url"'))
            with self.assertRaisesRegex(ValueError, "path patch configuration"):
                check.check_workspace_records(self.root, label)
            path.write_text(original)

    def test_git_fallback_in_either_lock_is_rejected(self):
        for label, name in (("main", "Cargo.lock"), ("fuzz", "fuzz/Cargo.lock")):
            path = self.root / name
            original = path.read_text()
            path.write_text(original.replace('name = "url"\n', 'name = "url"\nsource = "' + check.APPROVED_SOURCE + '"\n'))
            with self.assertRaisesRegex(ValueError, "locked url"):
                check.check_workspace_records(self.root, label)
            path.write_text(original)

    def test_symlinked_manifest_components_are_rejected(self):
        for name in ("Cargo.toml", "fuzz", "vendor", "vendor/rust-url/url/Cargo.toml"):
            path = self.root / name
            saved = self.root / "saved"
            path.rename(saved)
            path.symlink_to(saved, target_is_directory=saved.is_dir())
            with self.assertRaisesRegex(ValueError, "symlink"):
                check.check_workspace_records(self.root, "fuzz" if name == "fuzz" else "main")
            path.unlink()
            saved.rename(path)


class RegistryOrigins(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="url-registry-origin-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.home = self.root / "relocated-cargo-home"
        self.manifest = self.home / "registry/src/index.crates.io-1949cf8c6b5b557f/idna_adapter-1.2.1/Cargo.toml"
        self.manifest.parent.mkdir(parents=True)
        self.manifest.write_text("# location fixture, not source-integrity evidence\n")
        self.package = {"name": "idna_adapter", "version": "1.2.1", "source": check.REGISTRY_SOURCE,
                        "manifest_path": str(self.manifest)}
        environment = mock.patch.dict(os.environ, {"CARGO_HOME": str(self.home)})
        environment.start()
        self.addCleanup(environment.stop)

    def test_relocated_sparse_and_git_registry_caches_are_accepted(self):
        for cache in ("index.crates.io-1949cf8c6b5b557f", "github.com-25cdd57fae9f0462"):
            manifest = self.home / "registry/src" / cache / "idna_adapter-1.2.1/Cargo.toml"
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text("# registry location fixture\n")
            package = {**self.package, "manifest_path": str(manifest)}
            for label in ("main", "fuzz"):
                check.verify_registry_origin(package, label)

    def test_default_empty_relative_and_explicit_home_alias_are_accepted(self):
        for value in ("", str(self.home.relative_to(self.root))):
            with mock.patch.dict(os.environ, {"CARGO_HOME": value}), \
                    mock.patch.object(Path, "home", return_value=self.root), \
                    mock.patch.object(os, "getcwd", return_value=str(self.root)):
                if not value:
                    (self.root / ".cargo").symlink_to(self.home, target_is_directory=True)
                check.verify_registry_origin(self.package, "main")
        with mock.patch.dict(os.environ, {}, clear=True), \
                mock.patch.object(Path, "home", return_value=self.root):
            check.verify_registry_origin(self.package, "main")

    def test_unknown_registry_wrong_package_and_wrong_home_are_rejected(self):
        for path in (
            str(self.manifest).replace("1949cf8c6b5b557f", "0000000000000000"),
            str(self.manifest).replace("idna_adapter-1.2.1", "idna_adapter-99.0.0"),
            str(self.manifest).replace("relocated-cargo-home/", "relocated-cargo-home-other/"),
            str(self.manifest.parent / "nested/Cargo.toml"),
            "registry/src/index.crates.io-1949cf8c6b5b557f/idna_adapter-1.2.1/Cargo.toml",
        ):
            with self.subTest(path=path):
                with self.assertRaisesRegex(ValueError, "unapproved registry origin"):
                    check.verify_registry_origin({**self.package, "manifest_path": path}, "main")
        with mock.patch.dict(os.environ, {"CARGO_HOME": str(self.root / "unavailable-home")}):
            with self.assertRaisesRegex(ValueError, "unapproved registry origin"):
                check.verify_registry_origin(self.package, "main")

    def test_symlinked_cache_components_and_nonregular_manifest_are_rejected(self):
        for path in (self.home / "registry", self.home / "registry/src", self.manifest.parent.parent,
                     self.manifest.parent, self.manifest):
            with self.subTest(path=path):
                saved = self.root / "saved"
                path.rename(saved)
                path.symlink_to(saved, target_is_directory=saved.is_dir())
                try:
                    with self.assertRaisesRegex(ValueError, "unapproved registry origin.*symlink"):
                        check.verify_registry_origin(self.package, "main")
                finally:
                    path.unlink()
                    saved.rename(path)
                check.verify_registry_origin(self.package, "main")
        self.manifest.unlink()
        with self.assertRaisesRegex(ValueError, "unapproved registry origin"):
            check.verify_registry_origin(self.package, "main")
        self.manifest.mkdir()
        with self.assertRaisesRegex(ValueError, "unapproved registry origin.*regular file"):
            check.verify_registry_origin(self.package, "main")

    def test_directory_source_marker_inside_approved_cache_is_rejected(self):
        marker = self.manifest.with_name(".cargo-checksum.json")
        for kind in ("file", "directory", "dangling-symlink"):
            with self.subTest(kind=kind):
                check.verify_registry_origin(self.package, "main")
                if kind == "file":
                    marker.write_text(json.dumps({"package": check.REGISTRY_PACKAGES["idna_adapter"][1], "files": {}}))
                elif kind == "directory":
                    marker.mkdir()
                else:
                    marker.symlink_to(self.root / "missing")
                try:
                    with self.assertRaisesRegex(ValueError, "unapproved registry origin.*directory-source checksum"):
                        check.verify_registry_origin(self.package, "main")
                finally:
                    marker.rmdir() if kind == "directory" else marker.unlink()

    def test_wrong_registry_label_is_rejected_even_at_approved_location(self):
        for source in (None, "registry+https://unapproved.example/index", check.APPROVED_SOURCE):
            with self.subTest(source=source):
                with self.assertRaisesRegex(ValueError, "unexpected registry identity"):
                    check.verify_registry_origin({**self.package, "source": source}, "main")


class CargoDirectoryReplacement(unittest.TestCase):
    def test_real_locked_offline_cargo_replacement_retains_identity_but_is_rejected(self):
        # One of the qualified 31 registry packages, with no transitive deps.
        # The required CI cache preparation already fetches this exact version.
        with tempfile.TemporaryDirectory(prefix="url-real-cargo-origin-") as temporary:
            root = Path(temporary).resolve()
            # Preserve rustup selection when Cargo runs outside the repository.
            shutil.copyfile(check.ROOT / "rust-toolchain.toml", root / "rust-toolchain.toml")
            self.assertEqual(subprocess.check_output(["cargo", "--version"], cwd=root),
                             subprocess.check_output(["cargo", "--version"], cwd=check.ROOT))
            (root / "src").mkdir()
            (root / "src/lib.rs").write_text("pub fn fixture() {}\n")
            (root / "Cargo.toml").write_text(
                '[package]\nname="url-origin-fixture"\nversion="0.0.0"\nedition="2021"\n'
                '[workspace]\n[dependencies]\nutf8_iter="=1.0.4"\n')
            checksum = check.REGISTRY_PACKAGES["utf8_iter"][1]
            (root / "Cargo.lock").write_text(
                'version = 4\n\n[[package]]\nname = "url-origin-fixture"\nversion = "0.0.0"\n'
                'dependencies = ["utf8_iter"]\n\n[[package]]\nname = "utf8_iter"\nversion = "1.0.4"\n'
                f'source = "{check.REGISTRY_SOURCE}"\nchecksum = "{checksum}"\n')
            lock = (root / "Cargo.lock").read_bytes()
            def metadata():
                result = subprocess.run(
                    ["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
                    cwd=root, check=True, capture_output=True, text=True, timeout=60)
                self.assertEqual((root / "Cargo.lock").read_bytes(), lock)
                return next(p for p in json.loads(result.stdout)["packages"] if p["name"] == "utf8_iter")
            normal = metadata()
            check.verify_registry_origin(normal, "main")
            # Both outside-cache replacement and a replacement deliberately
            # disguised as an extraction in another explicit Cargo home.
            for disguised in (False, True):
                with self.subTest(disguised_as_cache=disguised):
                    home = root / "disguised-home"
                    source_root = (home / "registry/src/index.crates.io-1949cf8c6b5b557f"
                                   if disguised else root / "replacement")
                    vendor = source_root / "utf8_iter-1.0.4"
                    shutil.copytree(Path(normal["manifest_path"]).parent, vendor)
                    source = vendor / "src/lib.rs"
                    source.write_bytes(source.read_bytes() + b"\n// disposable replacement bytes\n")
                    files = {p.relative_to(vendor).as_posix(): check.digest(p.read_bytes())
                             for p in vendor.rglob("*") if p.is_file() and p.name != ".cargo-checksum.json"}
                    (vendor / ".cargo-checksum.json").write_text(json.dumps({"package": checksum, "files": files}, sort_keys=True))
                    config = root / ".cargo/config.toml"
                    config.parent.mkdir(exist_ok=True)
                    config.write_text('[source.crates-io]\nreplace-with="fixture"\n[source.fixture]\n'
                                      f'directory={json.dumps(str(source_root))}\n')
                    # No network, new lock, or checked-in source is involved.
                    with mock.patch.dict(os.environ, {"CARGO_HOME": str(home)} if disguised else {}):
                        replaced = metadata()
                        self.assertEqual({k: normal[k] for k in ("id", "source", "version")},
                                         {k: replaced[k] for k in ("id", "source", "version")})
                        self.assertEqual(replaced["manifest_path"], str(vendor / "Cargo.toml"))
                        self.assertNotEqual(source.read_bytes(), (Path(normal["manifest_path"]).parent / "src/lib.rs").read_bytes())
                        reason = ("directory-source checksum" if disguised
                                  else "(?:not in the subpath|outside approved)")
                        with self.assertRaisesRegex(ValueError, "unapproved registry origin for utf8_iter:.*" + reason):
                            check.verify_registry_origin(replaced, "main")
                        subprocess.run(["cargo", "check", "--offline", "--locked", "--target-dir", str(root / f"target-{disguised}")],
                                       cwd=root, check=True, capture_output=True, text=True, timeout=60)
                        self.assertEqual((root / "Cargo.lock").read_bytes(), lock)
                    config.unlink()
            check.verify_registry_origin(metadata(), "main")


if __name__ == "__main__":
    unittest.main()
