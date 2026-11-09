"""Real local Git histories and Cargo records; no GitHub or Cargo resolution."""

import contextlib
import io
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_rust_url_sync as check


class RustUrlSynchronization(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="rust-url-test-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.upstream = self.directory / "upstream.git"
        self.fork = self.directory / "fork.git"
        self.environment = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                                GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                                GIT_AUTHOR_DATE="2001-01-01T00:00:00+0000",
                                GIT_COMMITTER_DATE="2001-01-01T00:00:00+0000")
        for repo in (self.upstream, self.fork):
            self.command(repo.parent, "init", "--bare", "--initial-branch=main", str(repo))
        self.tree = self.command(self.upstream, "mktree", input="")
        self.sequence = 0
        self.base = self.commit(message="base")
        self.state(self.base, self.base, self.base)
        self.write_pins(self.base)

    def command(self, repo, *args, input=None):
        return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", "-C", str(repo), *args],
                              input=input, capture_output=True, text=True, check=True,
                              env=self.environment, timeout=10).stdout.strip()

    def commit(self, *parents, message="fixture"):
        self.sequence += 1
        arguments = ["commit-tree", self.tree]
        for parent in parents:
            arguments.extend(["-p", parent])
        return self.command(self.upstream, *arguments, "-m", f"{message} ({self.sequence})")

    def state(self, upstream, maintained, mirror):
        self.command(self.upstream, "update-ref", "refs/heads/main", upstream)
        for sha in set((maintained, mirror)):
            self.command(self.fork, "fetch", "--no-tags", str(self.upstream), sha)
        self.command(self.fork, "update-ref", "refs/heads/borrowser", maintained)
        self.command(self.fork, "update-ref", "refs/heads/main", mirror)

    def write_pins(self, pin):
        self.snapshot_pin = pin
        self.snapshot_tree = self.tree
        for label, directory in (("main", self.upstream), ("fuzz", self.upstream / "fuzz")):
            directory.mkdir(exist_ok=True)
            prefix = "" if label == "main" else "../"
            manifest = ('[workspace]\nexclude = ["vendor/rust-url"]\n[patch.crates-io]\n'
                        f'url = {{ path = "{prefix}vendor/rust-url/url" }}\n')
            if label == "main":
                manifest += 'percent-encoding = { path = "vendor/rust-url/percent_encoding" }\n'
            lock = "\n".join(f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
                             for name, version in sorted(check.url_policy.FORK_VERSIONS.items()))
            if pin is None:
                lock += 'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            (directory / "Cargo.toml").write_text(manifest)
            (directory / "Cargo.lock").write_text(lock)
        for name in check.url_policy.LOCAL_CRATES.values():
            path = self.upstream / "vendor/rust-url" / name / "Cargo.toml"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# fixture manifest; actual source validation is tested separately\n")

    def observe(self, failure=None):
        real_git = check.git

        def local_git(repo, *args, **kwargs):
            if args[0] == "fetch":
                if failure:
                    failure(args)
                args = tuple(str(self.upstream) if a == check.UPSTREAM else
                             str(self.fork) if a == check.FORK else a for a in args)
            return real_git(repo, *args, **kwargs)

        with (mock.patch.object(check, "git", side_effect=local_git),
              mock.patch.object(check.url_policy, "check_sources", return_value={
                  "fork": {"revision": self.snapshot_pin, "tree": self.snapshot_tree}})):
            # Only the approved-source accessor is substituted for synthetic Git
            # histories. Static Cargo configuration and all Git operations are real.
            return check.monitor(self.upstream)

    def test_equal_histories_are_healthy(self):
        code, summary, findings = self.observe()
        self.assertEqual(code, 0)
        self.assertEqual(findings, [])
        self.assertIn("Fork main matches upstream main", summary)
        self.assertIn("Consumer pin matches the maintained branch", summary)
        self.assertIn(self.base, summary)

    def test_intentional_fork_commits_are_allowed_and_availability_is_informational(self):
        patched = self.commit(self.base, message="maintained patch")
        self.state(self.base, patched, self.base)
        code, summary, findings = self.observe()
        self.assertEqual(code, 0)
        self.assertEqual([level for level, _ in findings], ["notice"])
        self.assertIn("Upstream ancestry incorporated; fork-only commits are permitted", summary)
        self.assertIn("qualification required before adoption", summary)
        self.assertIn(patched, summary)

    def test_upstream_advances_and_reports_exact_missing_commit(self):
        newer = self.commit(self.base, message="upstream fix")
        self.state(newer, self.base, newer)
        code, summary, findings = self.observe()
        self.assertEqual(code, 1)
        self.assertEqual(len(findings), 1)
        self.assertIn("synchronization review needed", summary)
        self.assertIn("Left-only: **1**; right-only: **0**", summary)
        self.assertIn(f"https://github.com/servo/rust-url/commit/{newer}", summary)

    def test_patched_fork_and_upstream_diverge(self):
        upstream = self.commit(self.base, message="upstream")
        patched = self.commit(self.base, message="patch")
        self.state(upstream, patched, upstream)
        self.write_pins(patched)
        code, summary, _ = self.observe()
        self.assertEqual(code, 1)
        self.assertIn("Relationship: **divergent**. Left-only: **1**; right-only: **1**", summary)

    def test_merging_upstream_restores_synchronization(self):
        upstream = self.commit(self.base, message="upstream")
        patched = self.commit(self.base, message="patch")
        merged = self.commit(patched, upstream, message="merge upstream")
        self.state(upstream, merged, upstream)
        self.write_pins(merged)
        code, summary, findings = self.observe()
        self.assertEqual(code, 0)
        self.assertEqual(findings, [])
        self.assertIn("Left-only: **0**; right-only: **2**", summary)

    def test_mirror_behind_ahead_and_divergent_require_review(self):
        first = self.commit(self.base, message="first")
        second = self.commit(self.base, message="second")
        for upstream, mirror, relation in ((first, self.base, "right-ancestor"),
                                            (self.base, first, "left-ancestor"),
                                            (first, second, "divergent")):
            with self.subTest(relation=relation):
                self.state(upstream, upstream, mirror)
                self.write_pins(upstream)
                code, summary, findings = self.observe()
                self.assertEqual(code, 1)
                self.assertEqual(findings, [("warning", "Fork main differs from upstream main; mirror review needed.")])
                mirror_section = summary.split("### Upstream (left) / fork main mirror (right)")[1]
                self.assertIn(f"Relationship: **{relation}**", mirror_section)

    def test_consumer_ahead_and_divergent_require_review(self):
        maintained = self.commit(self.base, message="maintained")
        ahead = self.commit(maintained, message="later consumer")
        divergent = self.commit(self.base, message="different consumer")
        self.state(self.base, maintained, self.base)
        for pin, relation in ((ahead, "right-ancestor"), (divergent, "divergent")):
            with self.subTest(relation=relation):
                # Make the pin fetchable without making it maintained-branch ancestry.
                self.command(self.fork, "fetch", str(self.upstream), pin)
                self.command(self.fork, "update-ref", "refs/heads/old-consumer", pin)
                self.write_pins(pin)
                code, summary, findings = self.observe()
                self.assertEqual(code, 1)
                self.assertIn("Consumer ancestry is ahead of or divergent", summary)
                self.assertEqual([level for level, _ in findings], ["warning"])
                self.assertIn(f"Relationship: **{relation}**", summary.split("### Consumer pin")[1])

    def test_each_unrelated_pair_is_an_error(self):
        unrelated = self.commit(message="different root")
        for which in ("upstream", "mirror", "consumer"):
            with self.subTest(which=which):
                if which == "upstream":
                    self.state(unrelated, self.base, unrelated)
                    self.write_pins(self.base)
                elif which == "mirror":
                    self.state(self.base, self.base, unrelated)
                    self.write_pins(self.base)
                else:
                    self.state(self.base, self.base, self.base)
                    self.command(self.fork, "fetch", str(self.upstream), unrelated)
                    self.command(self.fork, "update-ref", "refs/heads/old-consumer", unrelated)
                    self.write_pins(unrelated)
                code, summary, _ = self.observe()
                self.assertEqual(code, 2)
                self.assertIn("Unrelated histories; synchronization cannot be established", summary)

    def test_registry_configuration_is_rejected(self):
        self.write_pins(None)
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("Consumer configuration error", summary)

    def test_invalid_patch_declarations_are_configuration_errors(self):
        for declaration in ('42', '{ path = "elsewhere" }', '{ git = "https://example.invalid", rev = "bad" }'):
            self.write_pins(self.base)
            (self.upstream / "Cargo.toml").write_text(f"[patch.crates-io]\nurl = {declaration}\n")
            self.assertEqual(self.observe()[0], 2)

    def test_main_and_fuzz_source_disagreements_are_errors(self):
        for workspace, name, error in (
            ("main", "Cargo.toml", "unsupported rust-url path patch configuration"),
            ("fuzz", "fuzz/Cargo.toml", "unsupported rust-url path patch configuration"),
            ("main", "Cargo.lock", "locked url must select the approved local version"),
            ("fuzz", "fuzz/Cargo.lock", "locked url must select the approved local version"),
        ):
            with self.subTest(workspace=workspace, path=name):
                self.write_pins(self.base)
                code, _, findings = self.observe()
                self.assertEqual(code, 0)
                self.assertEqual(findings, [])

                path = self.upstream / name
                original = path.read_text()
                if name.endswith(".toml"):
                    changed = original.replace("vendor/rust-url/url", "vendor/other/url")
                else:
                    changed = original.replace('name = "url"\n', 'name = "url"\nsource = "git+unexpected"\n')
                self.assertNotEqual(changed, original)
                try:
                    path.write_text(changed)
                    code, summary, findings = self.observe()
                    self.assertEqual(code, 2)
                    self.assertEqual(findings, [("error", "Consumer configuration error.")])
                    self.assertIn(f"vendored consumer configuration: {workspace}: {error}", summary)
                finally:
                    path.write_text(original)

                code, _, findings = self.observe()
                self.assertEqual(code, 0)
                self.assertEqual(findings, [])

    def test_missing_malformed_and_ambiguous_cargo_records_are_errors(self):
        for contents in ('[patch', '[patch]\ncrates-io = "invalid"', '[patch.crates-io]\nurl = 42'):
            self.write_pins(self.base)
            (self.upstream / "Cargo.toml").write_text(contents)
            self.assertEqual(self.observe()[0], 2)
        self.write_pins(self.base)
        lock = self.upstream / "Cargo.lock"
        lock.write_text(lock.read_text() * 2)
        self.assertEqual(self.observe()[0], 2)
        lock.unlink()
        self.assertEqual(self.observe()[0], 2)

    def test_unapproved_commit_tree_is_an_error(self):
        self.snapshot_tree = "0" * 40
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("tree identity is invalid", summary)

    def test_real_snapshot_corruption_is_a_configuration_error(self):
        # Exercise the real frozen loader, rather than the synthetic accessor.
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        shutil.copytree(check.url_policy.ROOT / "vendor/rust-url", root / "vendor/rust-url")
        (root / "vendor/rust-url/url/src/lib.rs").write_bytes(b"altered")
        with self.assertRaisesRegex(ValueError, "source hash"):
            check.consumer_snapshot(root)

    def test_missing_remote_or_branch_is_never_healthy(self):
        self.command(self.fork, "update-ref", "-d", "refs/heads/main")
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("The mirror reference is unavailable", summary)
        self.assertIn("Upstream ancestry incorporated", summary)
        self.assertIn("Consumer pin matches", summary)
        self.assertIn("Not compared: a required reference is unavailable", summary)

    def test_each_remote_failure_retains_independent_results(self):
        for branch in ("refs/remotes/monitor/upstream", "refs/remotes/monitor/maintained", "refs/remotes/monitor/mirror"):
            with self.subTest(branch=branch):
                def fail(args):
                    if args[-1].endswith(branch):
                        raise check.GitError("transport unavailable")

                code, summary, _ = self.observe(fail)
                self.assertEqual(code, 2)
                self.assertIn("transport unavailable", summary)
                self.assertIn("Not compared", summary)
                self.assertIn("Relationship: **equal**", summary)

    def test_nonexistent_repository_is_reported_unavailable(self):
        self.fork = self.directory / "nonexistent.git"
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("The maintained reference is unavailable", summary)
        self.assertIn("The mirror reference is unavailable", summary)
        self.assertIn("Not compared: a required reference is unavailable", summary)

    def test_unavailable_pin_is_an_error(self):
        self.write_pins("0" * 40)
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("Consumer pin is unavailable", summary)
        self.assertIn("Fork main matches", summary)

    def test_fetch_timeout_and_comparison_error_take_precedence_over_warnings(self):
        newer = self.commit(self.base)
        self.state(newer, self.base, newer)

        def timeout(args):
            if args[-1].endswith("monitor/mirror"):
                raise check.GitError("git fetch timed out after 60 seconds")

        code, summary, _ = self.observe(timeout)
        self.assertEqual(code, 2)
        self.assertIn("synchronization review needed", summary)
        self.assertIn("timed out", summary)
        real_git = check.git

        def broken(repo, *args, **kwargs):
            if args[:2] == ("merge-base", "--is-ancestor"):
                raise check.GitError("git merge-base exited 128")
            return real_git(repo, *args, **kwargs)

        with mock.patch.object(check, "git", side_effect=broken):
            self.assertEqual(self.observe()[0], 2)

    def test_git_rejects_invalid_objects_and_does_not_treat_errors_as_false(self):
        with self.assertRaises(check.GitError):
            check.compare(self.upstream, "0" * 40, self.base)
        with mock.patch.object(check.subprocess, "Popen", side_effect=FileNotFoundError("no git")):
            with self.assertRaisesRegex(check.GitError, "unavailable"):
                check.git(self.upstream, "status")

    def test_timeout_kills_git_and_its_helper_process_group(self):
        real_popen = subprocess.Popen
        processes = []
        group_file = self.directory / "helper-group"
        program = ("import os, subprocess, sys, time; from pathlib import Path; "
                   "helper = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
                   f"Path({str(group_file)!r}).write_text(str(os.getpgid(helper.pid))); time.sleep(60)")

        def spawn(command, **kwargs):
            process = real_popen([sys.executable, "-c", program], **kwargs)
            processes.append(process)
            # Start the tested deadline only after the helper has joined the group.
            ready_by = time.monotonic() + 10
            while not group_file.exists():
                if process.poll() is not None or time.monotonic() >= ready_by:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
                    self.fail("timeout fixture helper did not start")
                time.sleep(0.01)
            return process

        with mock.patch.object(check.subprocess, "Popen", side_effect=spawn), \
                mock.patch.object(check.os, "killpg", wraps=os.killpg) as kill:
            with self.assertRaisesRegex(check.GitError, "timed out"):
                check.git(self.upstream, "fetch", timeout=1)
            kill.assert_called_once_with(processes[0].pid, signal.SIGKILL)
        self.assertEqual(int(group_file.read_text()), processes[0].pid)
        self.assertIsNotNone(processes[0].returncode)

    def test_long_history_is_counted_and_bounded(self):
        head = self.base
        for _ in range(22):
            head = self.commit(head)
        self.state(head, self.base, head)
        code, summary, _ = self.observe()
        self.assertEqual(code, 1)
        self.assertIn("Left-only: **22**", summary)
        self.assertIn("2 additional commits are not shown", summary)
        upstream_section = summary.split("### Upstream (left) / maintained fork (right)")[1].split("### Upstream (left) / fork main")[0]
        self.assertEqual(upstream_section.count("<pre>"), 20)

    def test_commit_subjects_and_diagnostics_are_escaped_and_not_executed(self):
        malicious = '<script>alert(1)</script> [link](javascript:evil) `code` | ::warning::forged\x1b\u2028\u202e'
        head = self.commit(self.base, message=malicious)
        self.state(head, self.base, head)
        code, summary, findings = self.observe()
        self.assertEqual(code, 1)
        self.assertNotIn("<script>", summary)
        self.assertIn("&lt;script&gt;", summary)
        self.assertIn("\\x1b", summary)
        self.assertIn("\\u2028\\u202e", summary)
        self.assertNotIn("\x1b", summary)
        self.assertNotIn(malicious, str(findings))

        def failure(args):
            raise check.GitError('\n::error::forged\n</pre><script>evil</script>\x00' + 'x' * 5000)

        result = self.observe(failure)
        self.assertEqual(result[0], 2)
        self.assertIn("&lt;/pre&gt;&lt;script&gt;", result[1])
        self.assertIn("[truncated]", result[1])
        summary_file = self.directory / "summary.md"
        output = io.StringIO()
        with mock.patch.object(check, "monitor", return_value=result), \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary_file)}), \
                contextlib.redirect_stdout(output):
            self.assertEqual(check.main(), 2)
        self.assertEqual(summary_file.read_text(), result[1])
        self.assertNotIn("forged", output.getvalue())
        with mock.patch.object(check, "monitor", return_value=result), \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}), \
                contextlib.redirect_stdout(io.StringIO()) as local_output:
            check.main()
        self.assertFalse(any(line.startswith("::error::forged") for line in local_output.getvalue().splitlines()))

    def test_unexpected_exception_and_summary_write_failure_exit_two(self):
        with mock.patch.object(check, "monitor", side_effect=RuntimeError("unexpected <failure>")), \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(self.directory / "fallback.md")}), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(check.main(), 2)
        self.assertIn("&lt;failure&gt;", (self.directory / "fallback.md").read_text())
        with mock.patch.object(check, "monitor", return_value=(0, "summary", [])), \
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(self.directory)}), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(check.main(), 2)


if __name__ == "__main__":
    unittest.main()
