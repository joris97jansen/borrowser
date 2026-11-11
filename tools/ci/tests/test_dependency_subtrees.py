"""Real offline Git integration and committed/index/physical-source contracts."""

import copy
import hashlib
import importlib.machinery
import json
import os
from pathlib import Path
import py_compile
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_dependency_subtrees as check


class DependencySubtrees(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        check.check_python_import_artifacts(check.ROOT)
        cls.temporary = tempfile.TemporaryDirectory(prefix="subtree-history-tests-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.directory = Path(cls.temporary.name)
        cls.environment = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                               GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                               GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                               GIT_AUTHOR_DATE="2001-01-01T00:00:00+0000",
                               GIT_COMMITTER_DATE="2001-01-01T00:00:00+0000",
                               PYTHONDONTWRITEBYTECODE="1", CARGO_NET_OFFLINE="true")
        cls.candidate = cls.directory / "candidate"
        cls.run_git(cls.directory, "clone", "--no-local", str(check.ROOT), str(cls.candidate))
        for relative in check.COMMITTED_INPUTS:
            target = cls.candidate / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(check.ROOT / relative, target)
        # Replace the whole directory so deleted tests cannot survive the copy.
        tests = check.COMMITTED_TESTS
        shutil.rmtree(cls.candidate / tests)
        shutil.copytree(check.ROOT / tests, cls.candidate / tests, symlinks=True)
        cls.run_git(cls.candidate, "add", "-A", "--", *check.COMMITTED_INPUTS, tests)
        cls.run_git(cls.candidate, "commit", "--allow-empty", "-m", "Disposable complete qualification inputs")
        cls.policy = check.load_qualification(cls.candidate)
        cls.candidate_sha = cls.run_git(cls.candidate, "rev-parse", "HEAD").stdout.strip()

    @classmethod
    def run_git(cls, root, *args, allowed=(0,)):
        result = subprocess.run(["git", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false",
                                 "-C", str(root), *args], capture_output=True, text=True,
                                env=cls.environment, timeout=120)
        if result.returncode not in allowed:
            raise AssertionError((args, result.returncode, result.stdout[-1000:], result.stderr[-2000:]))
        return result

    @classmethod
    def split_subtree(cls, root, prefix, *revisions):
        # git-subtree recursively visits uncached parents. Ubuntu's /bin/sh
        # (dash) reaches its function limit on the real imported histories.
        # Run the installed, unmodified script with Bash; keep the same inputs
        # and verify split identities/trees/ancestry in the calling tests.
        exec_path = cls.run_git(root, "--exec-path").stdout.strip()
        environment = dict(cls.environment, GIT_EXEC_PATH=exec_path,
                           PATH=exec_path + os.pathsep + cls.environment["PATH"])
        command = ["bash", str(Path(exec_path) / "git-subtree"), "split",
                   "--prefix=" + prefix, *revisions]
        result = subprocess.run(command, cwd=root, capture_output=True, text=True,
                                env=environment, timeout=120)
        if result.returncode:
            raise AssertionError((command, result.returncode, result.stdout[-1000:], result.stderr[-2000:]))
        return result.stdout.strip()

    def setUp(self):
        self.case = Path(tempfile.mkdtemp(dir=self.directory))
        self.repo = self.case / "host"
        self.run_git(self.case, "clone", "--shared", "--no-checkout", str(self.candidate), str(self.repo))
        self.run_git(self.repo, "checkout", "--detach", self.candidate_sha)

    def commit(self, root, message):
        self.run_git(root, "add", "-A")
        self.run_git(root, "commit", "-m", message)
        return self.run_git(root, "rev-parse", "HEAD").stdout.strip()

    def test_authentic_imports_bridge_and_current_committed_candidate(self):
        check.check_repo(self.repo)
        self.assertEqual(len(self.policy["dependencies"]["rust-url"]["files"]), 80)
        self.assertEqual(len(self.policy["dependencies"]["ureq"]["files"]), 65)

    def test_fixture_commits_the_complete_current_qualification_test_directory(self):
        expected = {}
        prefix = check.ROOT / "tools/ci/tests"
        for path in prefix.rglob("*"):
            if path.is_dir():
                continue
            self.assertFalse(path.is_symlink())
            expected[path.relative_to(prefix).as_posix()] = {
                "mode": "100755" if path.stat().st_mode & 0o111 else "100644",
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        self.assertEqual(check.git_entries(self.repo, "HEAD", "tools/ci/tests"), expected)
        check.check_repo(self.repo)

    def test_actual_approved_history_is_present_in_working_implementation(self):
        check.check_history(check.ROOT, check.load_qualification(check.ROOT))

    def test_pr_integration_and_subsequent_master_commits_keep_the_contract(self):
        (self.repo / "pr-fixture.txt").write_text("PR host change\n")
        pr = self.commit(self.repo, "Unrelated PR host change")
        self.run_git(self.repo, "checkout", "--detach", self.candidate_sha)
        (self.repo / "master-fixture.txt").write_text("Master host change\n")
        base = self.commit(self.repo, "Unrelated master host change")
        self.run_git(self.repo, "merge", "--no-ff", pr, "-m", "Synthetic PR integration fixture")
        self.assertEqual(self.run_git(self.repo, "show", "-s", "--format=%P", "HEAD").stdout.strip(), base + " " + pr)
        check.check_repo(self.repo)
        (self.repo / "master-fixture.txt").write_text("Subsequent master host change\n")
        self.commit(self.repo, "Subsequent master-push fixture")
        check.check_repo(self.repo)

    def test_wrong_import_parent_identity_is_rejected(self):
        policy = copy.deepcopy(self.policy)
        policy["history"]["C1"] = policy["history"]["C5"]
        with self.assertRaisesRegex(ValueError, "parent identity"):
            check.check_history(self.repo, policy)

    def forge_import(self, label, mutation):
        """Rebuild only a disposable structural chain, retaining real parents."""
        policy = copy.deepcopy(self.policy)
        history = policy["history"]
        source = check.URL_SOURCE if label == "C1" else check.UREQ_RELEASE
        prefix = "vendor/rust-url" if label == "C1" else "vendor/ureq"
        for number in range(int(label[1:]), 6):
            key = f"C{number}"
            original = self.policy["history"][key]
            self.run_git(self.repo, "read-tree", "--reset", "-u", original)
            if mutation == "host":
                # Carry the extra edit forward so all later scope/bridge checks
                # remain valid; the import itself must be what gets rejected.
                path = self.repo / "README.md"
                path.write_bytes(path.read_bytes() + b"\nUnrelated import host change\n")
            elif key == label:
                path = self.repo / prefix / "README.md"
                if mutation == "blob":
                    path.write_bytes(path.read_bytes() + b"\nChanged import blob\n")
                else:
                    path.chmod(path.stat().st_mode | 0o100)
            self.run_git(self.repo, "add", "-A")
            tree = self.run_git(self.repo, "write-tree").stdout.strip()
            previous = policy["baseline"] if number == 1 else history[f"C{number - 1}"]
            parents = [previous]
            if number in (1, 3, 4):
                parents.append({1: check.URL_SOURCE, 3: check.UREQ_RELEASE,
                                4: check.UREQ_MAINT_RELEASE}[number])
            arguments = [value for parent in parents for value in ("-p", parent)]
            history[key] = self.run_git(self.repo, "commit-tree", tree, *arguments,
                                       "-m", "Disposable structural substitution").stdout.strip()
        policy["dependencies"]["ureq"]["upstream_merge"] = history["C4"]
        self.run_git(self.repo, "checkout", "--detach", history["C5"])
        self.assertEqual(self.run_git(self.repo, "show", "-s", "--format=%P", history[label]).stdout.split(),
                         [policy["baseline"] if label == "C1" else history["C2"], source])
        return policy

    def test_rust_url_pristine_import_cannot_change_unrelated_host_files(self):
        policy = self.forge_import("C1", "host")
        self.assertEqual(self.run_git(self.repo, "rev-parse", policy["history"]["C1"] + ":vendor/rust-url").stdout.strip(),
                         policy["dependencies"]["rust-url"]["initial_tree"])
        with self.assertRaisesRegex(ValueError, "import scope"):
            check.check_history(self.repo, policy)

    def test_ureq_pristine_import_cannot_change_unrelated_host_files(self):
        policy = self.forge_import("C3", "host")
        self.assertEqual(self.run_git(self.repo, "rev-parse", policy["history"]["C3"] + ":vendor/ureq").stdout.strip(),
                         policy["dependencies"]["ureq"]["initial_tree"])
        with self.assertRaisesRegex(ValueError, "import scope"):
            check.check_history(self.repo, policy)

    def test_pristine_import_tree_identity_rejects_blob_and_mode_substitution(self):
        for label in ("C1", "C3"):
            for mutation in ("blob", "mode"):
                with self.subTest(import_commit=label, mutation=mutation):
                    policy = self.forge_import(label, mutation)
                    with self.assertRaisesRegex(ValueError, "pristine import"):
                        check.check_history(self.repo, policy)

    def incompatible_upstream(self, name, revision):
        policy = copy.deepcopy(self.policy)
        before = self.run_git(self.repo, "rev-parse", "HEAD").stdout.strip()
        tree = self.run_git(self.repo, "rev-parse", "HEAD^{tree}").stdout.strip()
        merge = self.run_git(self.repo, "commit-tree", tree, "-p", before, "-p", revision,
                             "-m", "Reachable incompatible upstream fixture").stdout.strip()
        self.run_git(self.repo, "checkout", "--detach", merge)
        policy["dependencies"][name].update(upstream_revision=revision, upstream_merge=merge)
        self.assertTrue(check.ancestor(self.repo, revision, "HEAD"))
        self.assertEqual(self.run_git(self.repo, "show", "-s", "--format=%P", merge).stdout.split(),
                         [before, revision])
        return policy

    def test_reachable_servo_revision_before_original_baseline_is_rejected(self):
        revision = self.run_git(self.repo, "rev-parse", check.SERVO_BASE + "^").stdout.strip()
        policy = self.incompatible_upstream("rust-url", revision)
        with self.assertRaisesRegex(ValueError, "official lineage"):
            check.check_history(self.repo, policy)

    def test_reachable_published_ureq_release_cannot_replace_maintenance_lineage(self):
        self.assertFalse(check.ancestor(self.repo, check.UREQ_MAINT_RELEASE, check.UREQ_RELEASE))
        policy = self.incompatible_upstream("ureq", check.UREQ_RELEASE)
        with self.assertRaisesRegex(ValueError, "official lineage"):
            check.check_history(self.repo, policy)

    def test_missing_required_commit_is_an_error(self):
        policy = copy.deepcopy(self.policy)
        policy["history"]["C3"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "git show failed"):
            check.check_history(self.repo, policy)

    def test_shallow_history_is_rejected_regardless_of_available_objects(self):
        shallow = self.case / "shallow"
        self.run_git(self.case, "clone", "--depth=1", "--no-local", self.repo.as_uri(), str(shallow))
        # Git may retain otherwise unreachable objects in a transferred pack.
        # Their availability never makes shallow ancestry evidence sufficient.
        with self.assertRaisesRegex(ValueError, "shallow"):
            check.check_repo(shallow)

    def test_equal_content_squash_does_not_satisfy_ancestry(self):
        self.run_git(self.repo, "checkout", "--detach", self.policy["baseline"])
        self.run_git(self.repo, "merge", "--squash", self.candidate_sha)
        self.run_git(self.repo, "commit", "-m", "Equal-content squash substitute")
        self.assertEqual(self.run_git(self.repo, "rev-parse", "HEAD^{tree}").stdout,
                         self.run_git(self.repo, "rev-parse", self.candidate_sha + "^{tree}").stdout)
        with self.assertRaisesRegex(ValueError, "not reachable"):
            check.check_repo(self.repo)

    def test_copied_tree_and_forged_subtree_trailers_do_not_satisfy_history(self):
        self.run_git(self.repo, "checkout", "--detach", self.policy["baseline"])
        self.run_git(self.repo, "read-tree", "--reset", "-u", self.candidate_sha)
        self.run_git(self.repo, "commit", "-m", "Copied source\n\ngit-subtree-dir: vendor/ureq\n"
                     "git-subtree-split: " + check.UREQ_RELEASE)
        with self.assertRaisesRegex(ValueError, "not reachable"):
            check.check_repo(self.repo)

    def test_wrong_bridge_parents_cannot_be_authorized_by_a_tree_hash(self):
        policy = copy.deepcopy(self.policy)
        policy["history"]["C4"] = policy["history"]["C3"]
        with self.assertRaisesRegex(ValueError, "parent identity"):
            check.check_history(self.repo, policy)

    def test_bridge_with_correct_parents_but_changed_contents_is_rejected(self):
        policy = copy.deepcopy(self.policy)
        history = policy["history"]
        tree = self.run_git(self.repo, "rev-parse", history["C5"] + "^{tree}").stdout.strip()
        bridge = self.run_git(self.repo, "commit-tree", tree, "-p", history["C3"],
                              "-p", check.UREQ_MAINT_RELEASE, "-m", "Invalid non-neutral bridge").stdout.strip()
        restored = self.run_git(self.repo, "commit-tree", tree, "-p", bridge,
                                "-m", "Restoration after invalid bridge").stdout.strip()
        history["C4"], history["C5"] = bridge, restored
        self.run_git(self.repo, "checkout", "--detach", restored)
        with self.assertRaisesRegex(ValueError, "not content-neutral"):
            check.check_history(self.repo, policy)

    def test_current_tree_is_checked_separately_from_import_ancestry(self):
        path = self.repo / "vendor/ureq/src/header.rs"
        path.write_bytes(path.read_bytes() + b"\n// unauthorized\n")
        self.commit(self.repo, "History intact but unqualified source")
        check.check_history(self.repo, self.policy)
        with self.assertRaisesRegex(ValueError, "committed source"):
            check.check_repo(self.repo)

    def test_staged_bad_source_with_restored_good_worktree_is_rejected(self):
        path = self.repo / "vendor/ureq/src/header.rs"
        original = path.read_bytes()
        path.write_bytes(original + b"\n// staged unqualified source\n")
        self.run_git(self.repo, "add", "vendor/ureq/src/header.rs")
        path.write_bytes(original)
        with self.assertRaisesRegex(ValueError, "staged vendor"):
            check.check_repo(self.repo)

    def test_unmerged_index_and_staged_executable_modes_are_rejected(self):
        path = "vendor/ureq/src/header.rs"
        self.run_git(self.repo, "update-index", "--chmod=+x", path)
        with self.assertRaisesRegex(ValueError, "staged vendor"):
            check.check_repo(self.repo)
        self.run_git(self.repo, "reset", "HEAD", "--", path)
        oid = self.run_git(self.repo, "rev-parse", "HEAD:" + path).stdout.strip()
        entries = "0 " + "0" * 40 + "\t" + path + "\n"
        entries += "".join(f"100644 {oid} {stage}\t{path}\n" for stage in (1, 2, 3))
        subprocess.run(["git", "-C", str(self.repo), "update-index", "--index-info"],
                       input=entries, text=True, check=True, env=self.environment)
        with self.assertRaisesRegex(ValueError, "unresolved index"):
            check.check_repo(self.repo)

    def test_worktree_content_mode_and_ignored_extras_are_rejected(self):
        path = self.repo / "vendor/ureq/src/header.rs"
        original = path.read_bytes()
        path.write_bytes(original + b"\n")
        with self.assertRaisesRegex(ValueError, "source hash"):
            check.check_repo(self.repo)
        path.write_bytes(original)
        path.chmod(path.stat().st_mode | 0o100)
        with self.assertRaisesRegex(ValueError, "mode/type"):
            check.check_repo(self.repo)
        path.chmod(0o644)
        extra = self.repo / "vendor/rust-url/target/ignored"
        extra.parent.mkdir()
        extra.write_text("unaccounted ignored file")
        with self.assertRaisesRegex(ValueError, "unexpected source directory"):
            check.check_repo(self.repo)

    def test_uncommitted_or_staged_qualification_inputs_are_rejected(self):
        path = self.repo / "Cargo.toml"
        original = path.read_bytes()
        path.write_bytes(original + b"\n# pending configuration\n")
        with self.assertRaisesRegex(ValueError, "uncommitted qualification"):
            check.check_repo(self.repo)
        self.run_git(self.repo, "add", "Cargo.toml")
        path.write_bytes(original)
        with self.assertRaisesRegex(ValueError, "staged qualification"):
            check.check_repo(self.repo)

    def test_unstaged_qualification_test_changes_are_rejected(self):
        path = self.repo / "tools/ci/tests/test_dependency_subtrees.py"
        path.write_bytes(path.read_bytes() + b"\n# Changed qualification implementation\n")
        with self.assertRaisesRegex(ValueError, "uncommitted qualification test"):
            check.check_repo(self.repo)

    def test_staged_qualification_test_changes_cannot_hide_behind_restored_worktree(self):
        relative = "tools/ci/tests/test_dependency_subtrees.py"
        path = self.repo / relative
        original = path.read_bytes()
        path.write_bytes(original + b"\n# Staged qualification implementation\n")
        self.run_git(self.repo, "add", relative)
        path.write_bytes(original)
        with self.assertRaisesRegex(ValueError, "staged qualification tests"):
            check.check_repo(self.repo)

    def test_untracked_qualification_files_including_ignored_sources_are_rejected(self):
        for relative in ("tools/ci/tests/test_untracked.py", "tools/ci/tests/__pycache__/hidden_source.py",
                         "tools/ci/untracked_helper.py", "tools/ci/untracked_helper.PY"):
            with self.subTest(path=relative):
                path = self.repo / relative
                path.parent.mkdir(exist_ok=True)
                path.write_text("# Uncommitted qualification source\n")
                with self.assertRaisesRegex(ValueError, "untracked qualification test|Python bytecode cache|"
                                                       "Unqualified Python helper"):
                    check.check_repo(self.repo)
                path.unlink()
                if path.parent.name == "__pycache__":
                    path.parent.rmdir()

    def test_qualification_test_modes_and_missing_files_are_rejected(self):
        path = self.repo / "tools/ci/tests/test_dependency_subtrees.py"
        path.chmod(path.stat().st_mode | 0o100)
        with self.assertRaisesRegex(ValueError, "uncommitted qualification test"):
            check.check_repo(self.repo)
        path.chmod(0o644)
        path.unlink()
        with self.assertRaises(OSError):
            check.check_repo(self.repo)

    def test_private_files_and_generated_artifacts_outside_tooling_are_unaffected(self):
        cache = self.repo / "target/__pycache__/private.pyc"
        cache.parent.mkdir(parents=True)
        cache.write_bytes(b"Generated artifact outside qualification Python paths")
        (self.repo / "private-local-notes.txt").write_text("Unrelated private file\n")
        check.check_repo(self.repo)

    def conflicting_cache(self, source, payload):
        """Timestamp-valid conflicting bytecode, confined to a disposable repo."""
        original, before = source.read_bytes(), source.stat()
        body = payload.encode()
        self.assertLess(len(body) + 2, len(original))
        source.write_bytes(body + b"#" + b" " * (len(original) - len(body) - 2) + b"\n")
        os.utime(source, ns=(before.st_atime_ns, before.st_mtime_ns))
        cache = Path(py_compile.compile(str(source), doraise=True,
                                       invalidation_mode=py_compile.PycInvalidationMode.TIMESTAMP))
        source.write_bytes(original)
        os.utime(source, ns=(before.st_atime_ns, before.st_mtime_ns))
        self.assertEqual(source.read_bytes(), original)
        self.assertEqual((source.stat().st_mtime_ns, source.stat().st_size),
                         (before.st_mtime_ns, before.st_size))
        return cache

    def run_entry(self, script, *args):
        return subprocess.run([sys.executable, "-I", "-B", str(self.repo / "tools/ci" / script), *args],
                              cwd=self.repo, env=self.environment, capture_output=True, text=True, timeout=120)

    def test_B_discovery_executes_conflicting_cache_but_qualified_runner_rejects_it(self):
        source = self.repo / "tools/ci/tests/test_cache_fixture.py"
        source.write_text("import unittest\nclass SourceTest(unittest.TestCase):\n"
                          "    def test_committed_source(self): self.assertTrue(True)\n#" + " " * 1024 + "\n")
        self.commit(self.repo, "Committed clean bytecode regression fixture")
        marker = self.case / "test-cache-executed"
        cache = self.conflicting_cache(source, "from pathlib import Path\nPath(" + repr(str(marker))
                                       + ").write_text('cached test executed')\nimport unittest\n"
                                       "class CachedTest(unittest.TestCase):\n"
                                       "    def test_cached_payload(self): self.assertTrue(True)\n")
        before = cache.read_bytes()
        unsafe = subprocess.run([sys.executable, "-B", "-m", "unittest", "discover", "-s", "tools/ci/tests",
                                 "-p", source.name, "-v"], cwd=self.repo, env=self.environment,
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(unsafe.returncode, 0, unsafe.stderr)
        self.assertTrue(marker.exists())
        self.assertIn("test_cached_payload", unsafe.stderr)
        self.assertNotIn("test_committed_source", unsafe.stderr)
        self.assertIn("Ran 1 test", unsafe.stderr)
        self.assertEqual(cache.read_bytes(), before)  # -B did not rewrite it.
        marker.unlink()  # Only our disposable marker, never a developer file.
        with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
            check.check_repo(self.repo)
        safe = self.run_entry("check_dependency_subtrees.py", "--tests")
        self.assertNotEqual(safe.returncode, 0)
        self.assertIn("Python bytecode cache", safe.stderr)
        self.assertFalse(marker.exists())
        self.assertEqual(cache.read_bytes(), before)

    def test_helper_cache_is_rejected_before_any_qualification_entry_imports_it(self):
        source = self.repo / "tools/ci/check_dependency_subtrees.py"
        marker = self.case / "helper-cache-executed"
        cache = self.conflicting_cache(source, "from pathlib import Path\nPath(" + repr(str(marker))
                                       + ").write_text('cached helper executed')\nraise SystemExit(0)\n")
        before = cache.read_bytes()
        for script in ("check_dependency_subtrees.py", "check_url_dependencies.py", "check_ureq_patch.py",
                       "check_rust_url_ureq_sync.py"):
            with self.subTest(entry=script):
                result = self.run_entry(script)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Python bytecode cache", result.stderr)
                if script == "check_rust_url_ureq_sync.py":
                    self.assertEqual(result.returncode, 2)
                self.assertFalse(marker.exists())
        self.assertEqual(cache.read_bytes(), before)

    def test_isolated_entry_cannot_load_repository_bytecode_as_stdlib(self):
        marker = self.case / "stdlib-shadow-executed"
        source = self.case / "json.py"
        source.write_text("from pathlib import Path\nPath(" + repr(str(marker))
                          + ").write_text('shadowed stdlib')\nraise SystemExit(0)\n")
        py_compile.compile(str(source), cfile=str(self.repo / "tools/ci/json.pyc"), doraise=True)
        result = self.run_entry("check_dependency_subtrees.py")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Python bytecode cache", result.stderr)
        self.assertFalse(marker.exists())
        raw = subprocess.run([sys.executable, "-B", str(self.repo / "tools/ci/check_dependency_subtrees.py")],
                             cwd=self.repo, env=self.environment, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(raw.returncode, 0)
        self.assertIn("python3 -I -B", raw.stderr)
        self.assertFalse(marker.exists())

    def test_nested_ignored_and_legacy_bytecode_paths_fail_closed(self):
        for relative in ("tools/ci/tests/nested/__pycache__/test_hidden.pyc",
                         "tools/ci/__PYCACHE__/HELPER.PYC",
                         "tools/ci/__pycache__/helper.pyc", "tools/ci/legacy.pyc", "tools/ci/legacy.pyo"):
            with self.subTest(path=relative):
                path = self.repo / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"Cache paths are rejected without reading their payload")
                with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
                    check.check_repo(self.repo)
                path.unlink()
                if path.parent.name.casefold() == "__pycache__":
                    path.parent.rmdir()

    def test_tracked_and_staged_bytecode_is_rejected_even_when_physically_absent(self):
        relative = "tools/ci/legacy.pyc"
        path = self.repo / relative
        path.write_bytes(b"Staged bytecode")
        self.run_git(self.repo, "add", "-f", relative)
        path.unlink()
        with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
            check.check_repo(self.repo)
        path.write_bytes(b"Staged bytecode")
        self.commit(self.repo, "Forbidden committed bytecode fixture")
        self.run_git(self.repo, "rm", relative)
        with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
            check.check_repo(self.repo)

    def test_undeclared_staged_helper_cannot_enter_qualified_import_path(self):
        path = self.repo / "tools/ci/unqualified_helper.py"
        path.write_text("# New unqualified Python input\n")
        self.run_git(self.repo, "add", str(path.relative_to(self.repo)))
        with self.assertRaisesRegex(ValueError, "Unqualified Python helper"):
            check.check_repo(self.repo)

    def test_changed_helper_source_is_rejected_before_monitor_imports_it(self):
        relative = "tools/ci/check_ureq_patch.py"
        path = self.repo / relative
        original = path.read_bytes()
        marker = self.case / "uncommitted-helper-executed"
        path.write_bytes(original + ("\nfrom pathlib import Path\nPath(" + repr(str(marker))
                                    + ").write_text('changed helper executed')\n").encode())
        result = self.run_entry("check_rust_url_ureq_sync.py")
        self.assertEqual(result.returncode, 2)
        self.assertIn("uncommitted qualification input", result.stderr)
        self.assertFalse(marker.exists())
        self.run_git(self.repo, "add", relative)
        path.write_bytes(original)
        result = self.run_entry("check_rust_url_ureq_sync.py")
        self.assertEqual(result.returncode, 2)
        self.assertIn("staged qualification input", result.stderr)
        self.assertFalse(marker.exists())

    def test_cache_symlinks_and_nonregular_entries_are_rejected_without_following(self):
        path = self.repo / "tools/ci/__pycache__"
        path.symlink_to(self.case, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
            check.check_repo(self.repo)
        path.unlink()
        path = self.repo / "tools/ci/tests/cache.pyc"
        path.symlink_to(self.case / "missing-target")
        with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
            check.check_repo(self.repo)
        path.unlink()
        if hasattr(os, "mkfifo"):
            os.mkfifo(path)
            with self.assertRaisesRegex(ValueError, "Python bytecode cache"):
                check.check_repo(self.repo)
            path.unlink()
            os.mkfifo(self.repo / "tools/ci/tests/nonregular-input")
            with self.assertRaisesRegex(ValueError, "Nonregular Python tooling"):
                check.check_repo(self.repo)

    def test_native_loader_shadows_verified_helper_but_preflight_blocks_all_entries(self):
        directory = self.repo / "tools/ci"
        source = directory / "check_dependency_subtrees.py"
        original = source.read_bytes()
        for suffix in importlib.machinery.EXTENSION_SUFFIXES:
            with self.subTest(suffix=suffix):
                native = directory / ("check_dependency_subtrees" + suffix)
                # Finder selection proves precedence without loading a binary.
                native.write_bytes(b"Import-resolution fixture: never execute")
                importlib.invalidate_caches()
                spec = importlib.machinery.PathFinder.find_spec("check_dependency_subtrees", [str(directory)])
                self.assertIsInstance(spec.loader, importlib.machinery.ExtensionFileLoader)
                self.assertEqual(Path(spec.origin), native)
                self.assertEqual(source.read_bytes(), original)
                self.run_git(self.repo, "ls-files", "--error-unmatch", str(native.relative_to(self.repo)),
                             allowed=(1,))
                with self.assertRaisesRegex(ValueError, "Native Python extension"):
                    check.check_repo(self.repo)
                for script in ("check_dependency_subtrees.py", "check_url_dependencies.py",
                               "check_ureq_patch.py", "check_rust_url_ureq_sync.py"):
                    result = self.run_entry(script)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("Native Python extension", result.stderr)
                    if script == "check_rust_url_ureq_sync.py":
                        self.assertEqual(result.returncode, 2)
                native.unlink()

    def test_native_suffixes_cover_abi_portable_case_and_nested_paths(self):
        names = ("helper.so", "helper.cpython-312-x86_64-linux-gnu.so", "helper.abi3.so",
                 "helper.cpython-314-darwin.so", "helper.pyd", "helper.cp312-win_amd64.pyd",
                 "HELPER.CP314-WIN_ARM64.PYD", "HELPER.CPYTHON-314-DARWIN.SO",
                 "tests/nested/helper.so")
        for name in names:
            with self.subTest(path=name):
                path = self.repo / "tools/ci" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"Importable extension filename, no binary loading")
                self.assertTrue(check.python_extension_path(path.relative_to(self.repo).as_posix()))
                with self.assertRaisesRegex(ValueError, "Native Python extension"):
                    check.check_repo(self.repo)
                path.unlink()

    def test_ignored_staged_and_committed_native_extensions_fail_even_when_absent(self):
        relative = "tools/ci/ignored_helper.so"
        path = self.repo / relative
        with (self.repo / ".git/info/exclude").open("a") as excludes:
            excludes.write("\n*.so\n")
        path.write_bytes(b"Ignored extension fixture")
        self.run_git(self.repo, "check-ignore", "--quiet", relative)
        with self.assertRaisesRegex(ValueError, "Native Python extension"):
            check.check_repo(self.repo)
        self.run_git(self.repo, "add", "-f", relative)
        path.unlink()
        with self.assertRaisesRegex(ValueError, "Native Python extension"):
            check.check_repo(self.repo)
        path.write_bytes(b"Ignored extension fixture")
        self.commit(self.repo, "Forbidden native extension fixture")
        self.run_git(self.repo, "rm", relative)
        with self.assertRaisesRegex(ValueError, "Native Python extension"):
            check.check_repo(self.repo)

    def test_native_symlinks_and_nonregular_entries_are_rejected_without_reads(self):
        path = self.repo / "tools/ci/helper.so"
        path.symlink_to(self.case / "missing-native-target")
        with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("Unexpected file read")):
            with self.assertRaisesRegex(ValueError, "Native Python extension"):
                check.check_repo(self.repo)
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "Native Python extension"):
            check.check_repo(self.repo)
        path.rmdir()
        if hasattr(os, "mkfifo"):
            os.mkfifo(path)
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("Unexpected file read")):
                with self.assertRaisesRegex(ValueError, "Native Python extension"):
                    check.check_repo(self.repo)

    def test_legitimate_shell_scripts_remain_supported(self):
        for name in ("local-tool.sh", "not-a-module.so.sh", "not-a-module.pyd.sh"):
            path = self.repo / "tools/ci" / name
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
            self.assertFalse(check.python_extension_path(path.relative_to(self.repo).as_posix()))
        check.check_repo(self.repo)
        self.commit(self.repo, "Legitimate non-Python tooling fixture")
        check.check_repo(self.repo)

    def test_materialized_license_is_rejected_even_with_symlinks_disabled(self):
        self.run_git(self.repo, "config", "core.symlinks", "false")
        path = self.repo / "vendor/rust-url/url/LICENSE-MIT"
        path.unlink()
        path.write_text("../LICENSE-MIT")
        self.assertFalse(self.run_git(self.repo, "diff", "--name-only").stdout)
        with self.assertRaisesRegex(ValueError, "original license symlink"):
            check.check_repo(self.repo)

    def test_actual_ureq_bridge_primary_history_and_split_are_distinct(self):
        bridge = self.policy["history"]["C4"]
        split = self.split_subtree(self.repo, "vendor/ureq", bridge)
        self.assertEqual(split, check.UREQ_RELEASE)
        self.assertTrue(check.ancestor(self.repo, check.UREQ_MAINT_RELEASE, bridge))
        self.assertFalse(check.ancestor(self.repo, check.UREQ_MAINT_RELEASE, split))
        self.assertEqual(self.run_git(self.repo, "rev-parse", split + "^{tree}").stdout,
                         self.run_git(self.repo, "rev-parse", bridge + ":vendor/ureq").stdout)

    def exercise_upstream_merge(self, name, source, local_path):
        host = self.case / (name + "-merge-host")
        self.run_git(self.case, "clone", "--shared", str(self.candidate), str(host))
        upstream = self.case / (name + "-upstream")
        self.run_git(self.case, "clone", "--no-local", str(host), str(upstream))
        self.run_git(upstream, "checkout", "--detach", source)
        header = host / ("vendor/" + name + "/" + local_path)
        header.write_bytes(header.read_bytes() + b"\n// local fixture requirement\n")
        conflict = host / ("vendor/" + name + "/fixture.txt")
        conflict.write_text("base\n")
        self.commit(host, "Local source requirement")
        (upstream / "fixture.txt").write_text("base\n")
        readme = upstream / "README.md"
        readme.write_text(readme.read_text() + "\nUpstream fixture requirement\n")
        first = self.commit(upstream, "Upstream nonconflicting change")
        self.run_git(host, "fetch", str(upstream), first)
        self.run_git(host, "subtree", "merge", "--prefix=vendor/" + name, first)
        self.assertIn("local fixture requirement", header.read_text())
        self.assertIn("Upstream fixture requirement", (host / ("vendor/" + name + "/README.md")).read_text())
        self.assertTrue(check.ancestor(host, first, "HEAD"))
        advanced = copy.deepcopy(self.policy)
        advanced["dependencies"][name].update(
            upstream_revision=first, upstream_merge=self.run_git(host, "rev-parse", "HEAD").stdout.strip())
        check.check_history(host, advanced)
        split = self.split_subtree(host, "vendor/" + name)
        # Split reconstructs the originally imported release history. The ureq
        # maintenance bridge belongs to primary history, not necessarily split.
        split_base = check.URL_SOURCE if name == "rust-url" else check.UREQ_RELEASE
        self.assertTrue(check.ancestor(host, split_base, split))
        self.assertEqual(self.run_git(host, "rev-parse", split + "^{tree}").stdout,
                         self.run_git(host, "rev-parse", "HEAD:vendor/" + name).stdout)
        conflict.write_text("local\n")
        before = self.commit(host, "Local conflicting edit")
        (upstream / "fixture.txt").write_text("upstream\n")
        second = self.commit(upstream, "Upstream conflicting edit")
        self.run_git(host, "fetch", str(upstream), second)
        self.run_git(host, "subtree", "merge", "--prefix=vendor/" + name, second, allowed=(1,))
        self.assertEqual(self.run_git(host, "diff", "--name-only", "--diff-filter=U").stdout.strip(),
                         ("vendor/" + name + "/fixture.txt"))
        self.run_git(host, "merge", "--abort")
        self.assertEqual(self.run_git(host, "rev-parse", "HEAD").stdout.strip(), before)
        self.assertFalse(self.run_git(host, "status", "--porcelain").stdout)
        self.run_git(host, "subtree", "merge", "--prefix=vendor/" + name, second, allowed=(1,))
        conflict.write_text("local\nupstream\n")
        resolved = self.commit(host, "Explicitly retain both requirements")
        self.assertEqual(self.run_git(host, "show", "-s", "--format=%P", resolved).stdout.strip(),
                         before + " " + second)
        self.assertTrue(check.ancestor(host, second, resolved))
        advanced["dependencies"][name].update(upstream_revision=second, upstream_merge=resolved)
        check.check_history(host, advanced)

    def test_ureq_upstream_merge_conflict_abort_and_explicit_resolution(self):
        self.exercise_upstream_merge("ureq", check.UREQ_MAINT_RELEASE, "src/header.rs")

    def test_rust_url_upstream_merge_conflict_abort_and_explicit_resolution(self):
        self.exercise_upstream_merge("rust-url", check.URL_SOURCE, "url/src/parser.rs")

    def test_requalified_disposable_source_still_requires_dependency_qualification(self):
        source = self.repo / "vendor/ureq/src/header.rs"
        source.write_bytes(source.read_bytes() + b"\n// harmless qualification fixture\n")
        with self.assertRaisesRegex(ValueError, "source hash"):
            check.check_repo(self.repo)
        policy = copy.deepcopy(self.policy)
        policy["dependencies"]["ureq"]["files"]["src/header.rs"]["sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
        file = self.repo / check.POLICY_PATH
        file.write_text(json.dumps(policy, indent=2, sort_keys=True) + "\n")
        with self.assertRaisesRegex(ValueError, "reviewed policy"):
            check.check_repo(self.repo)
        digest = hashlib.sha256(file.read_bytes()).hexdigest()
        verifier = self.repo / "tools/ci/check_dependency_subtrees.py"
        verifier.write_text(verifier.read_text().replace(check.POLICY_SHA256, digest))
        self.commit(self.repo, "Explicit test-policy advancement, not owner authorization")
        for script in ("check_dependency_subtrees.py", "check_url_dependencies.py", "check_ureq_patch.py"):
            command = [sys.executable, "-I", "-B", str(self.repo / "tools/ci" / script)]
            result = subprocess.run(command, cwd=self.repo, env=self.environment, capture_output=True, text=True, timeout=120)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        manifest = self.repo / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('features = ["tls", "json", "charset", "native-certs"]',
                                                        'features = ["tls", "json", "charset", "native-certs", "gzip"]'))
        result = subprocess.run([sys.executable, "-I", "-B", str(self.repo / "tools/ci/check_ureq_patch.py")],
                                cwd=self.repo, env=self.environment, capture_output=True, text=True, timeout=120)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ureq features", result.stderr)
