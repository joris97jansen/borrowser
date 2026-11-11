"""Both fixed upstreams, real disposable Git histories, no network or adoption."""

import contextlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_rust_url_ureq_sync as check


class Synchronization(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="dependency-monitor-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.environment = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                                GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                                GIT_AUTHOR_DATE="2001-01-01T00:00:00+0000",
                                GIT_COMMITTER_DATE="2001-01-01T00:00:00+0000")
        self.upstream = self.directory / "url.git"
        self.ureq = self.directory / "ureq.git"
        self.host = self.directory / "host.git"
        for repo in (self.upstream, self.ureq, self.host):
            self.command(self.directory, "init", "--bare", str(repo))
        self.tree = self.command(self.upstream, "mktree", input="")
        self.base = self.commit(self.upstream, message="url base")
        self.ureq_tree = self.command(self.ureq, "mktree", input="")
        self.ureq_base = self.commit(self.ureq, message="ureq base")
        self.command(self.upstream, "update-ref", "refs/heads/main", self.base)
        self.command(self.ureq, "update-ref", "refs/heads/2.12.x-msrv1.71", self.ureq_base)
        for repo, rev in ((self.upstream, self.base), (self.ureq, self.ureq_base)):
            self.command(self.host, "fetch", str(repo), rev)
        self.command(self.host, "update-ref", "refs/heads/main", self.base)
        self.policy = {"dependencies": {"rust-url": {"upstream_revision": self.base},
                                        "ureq": {"upstream_revision": self.ureq_base}}}

    def command(self, root, *args, input=None):
        return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", "-C", str(root), *args],
                              input=input, capture_output=True, text=True, check=True,
                              env=self.environment, timeout=15).stdout.strip()

    def commit(self, root, *parents, message="fixture"):
        tree = self.tree if root == self.upstream else self.ureq_tree
        args = ["commit-tree", tree]
        for parent in parents:
            args += ["-p", parent]
        return self.command(root, *args, "-m", message)

    def advance(self, name, message="upstream change"):
        repo, base, branch = ((self.upstream, self.base, "main") if name == "rust-url" else
                              (self.ureq, self.ureq_base, "2.12.x-msrv1.71"))
        tip = self.commit(repo, base, message=message)
        self.command(repo, "update-ref", "refs/heads/" + branch, tip)
        return tip

    def observe(self, fail=None, source_error=False):
        real_git = check.git
        self.fetches = []
        def local_git(repo, *args, **kwargs):
            if args[0] == "fetch":
                if check.UPSTREAM in args:
                    self.fetches.append("rust-url")
                    if fail == "rust-url":
                        raise check.GitError("fixture URL fetch failed")
                if check.UREQ_UPSTREAM in args:
                    self.fetches.append("ureq")
                    if fail == "ureq":
                        raise check.GitError("fixture ureq fetch failed")
                args = tuple(str(self.upstream) if a == check.UPSTREAM else
                             str(self.ureq) if a == check.UREQ_UPSTREAM else a for a in args)
            return real_git(repo, *args, **kwargs)
        with (mock.patch.object(check, "git", side_effect=local_git),
              mock.patch.object(check.subtrees, "load_qualification", return_value=self.policy),
              mock.patch.object(check.subtrees, "check_repo", side_effect=ValueError("unqualified fixture") if source_error else None),
              mock.patch.object(check.url_policy, "check_workspace_records"),
              mock.patch.object(check.ureq_policy, "check_workspace_records")):
            return check.monitor(self.host)

    def test_both_current(self):
        code, summary, findings = self.observe()
        self.assertEqual(code, 0)
        self.assertFalse(findings)
        self.assertEqual(self.fetches, ["rust-url", "ureq"])
        self.assertIn("Result: Current", summary)

    def test_each_upstream_advance_is_review_available(self):
        for name in ("rust-url", "ureq"):
            tip = self.advance(name)
            code, summary, findings = self.observe()
            self.assertEqual(code, 1)
            self.assertIn(tip, summary)
            self.assertIn("not adopted", summary)
            self.assertTrue(all(level == "warning" for level, _ in findings))

    def test_local_custom_history_is_not_compared_as_official_history(self):
        local = self.commit(self.upstream, self.base, message="Borrowser-only local history")
        self.command(self.host, "fetch", str(self.upstream), local)
        code, _, _ = self.observe()
        self.assertEqual(code, 0)

    def test_rewind_divergence_and_unrelated_history_are_errors(self):
        newbase = self.advance("rust-url")
        self.command(self.host, "fetch", str(self.upstream), newbase)
        self.policy["dependencies"]["rust-url"]["upstream_revision"] = newbase
        for tip in (self.base, self.commit(self.upstream, self.base, message="divergent"),
                    self.commit(self.upstream, message="unrelated")):
            self.command(self.upstream, "update-ref", "refs/heads/main", tip)
            code, summary, _ = self.observe()
            self.assertEqual(code, 2)
            self.assertIn("divergence or rewind", summary)

    def test_each_fetch_failure_does_not_suppress_other_observation(self):
        self.advance("rust-url")
        self.advance("ureq")
        for failing in ("rust-url", "ureq"):
            code, summary, findings = self.observe(fail=failing)
            self.assertEqual(code, 2)
            self.assertEqual(self.fetches, ["rust-url", "ureq"])
            self.assertIn("Review available", summary)
            self.assertTrue(any(level == "error" for level, _ in findings))

    def test_missing_branch_is_incomplete(self):
        self.command(self.ureq, "update-ref", "-d", "refs/heads/2.12.x-msrv1.71")
        code, summary, _ = self.observe()
        self.assertEqual(code, 2)
        self.assertIn("incomplete", summary)
        self.assertEqual(self.fetches, ["rust-url", "ureq"])

    def test_unqualified_configuration_is_never_healthy(self):
        code, summary, _ = self.observe(source_error=True)
        self.assertEqual(code, 2)
        self.assertIn("Unqualified local", summary)
        self.assertEqual(self.fetches, ["rust-url", "ureq"])

    def test_unexpected_first_observation_exception_preserves_second(self):
        def observation(root, name):
            if name == "rust-url":
                raise RuntimeError("fixture unexpected exception")
            return ["ureq completed"], []
        with mock.patch.object(check, "observe", side_effect=observation) as observed:
            code, summary, _ = check.monitor(self.host)
        self.assertEqual(observed.call_count, 2)
        self.assertEqual(code, 2)
        self.assertIn("ureq completed", summary)

    def test_subjects_are_escaped_and_never_executed(self):
        marker = self.directory / "must-not-exist"
        self.advance("rust-url", f'<script>$(touch {marker})</script>\n::error::external text')
        code, summary, _ = self.observe()
        self.assertEqual(code, 1)
        self.assertIn("&lt;script&gt;", summary)
        self.assertNotIn("<script>", summary)
        self.assertFalse(marker.exists())

    def test_unexpected_top_level_failure_is_exit_two(self):
        with (mock.patch.object(check, "monitor", side_effect=RuntimeError("fixture unexpected failure")),
              mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}),
              contextlib.redirect_stdout(io.StringIO()) as output):
            self.assertEqual(check.main(), 2)
        self.assertIn("Monitoring incomplete", output.getvalue())

    def test_summary_write_errors_are_exit_two(self):
        with (mock.patch.object(check, "monitor", return_value=(0, "fixture", [])),
              mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(self.directory / "missing/summary")}),
              contextlib.redirect_stdout(io.StringIO())):
            self.assertEqual(check.main(), 2)

    def test_read_only_observation_preserves_sources_and_remote_refs(self):
        before = {repo: self.command(repo, "show-ref") for repo in (self.host, self.upstream, self.ureq)}
        self.observe()
        self.assertEqual(before, {repo: self.command(repo, "show-ref") for repo in before})

    def test_history_output_is_bounded_and_totals_are_complete(self):
        tip = self.base
        for i in range(25):
            tip = self.commit(self.upstream, tip, message=f"upstream {i}")
        self.command(self.upstream, "update-ref", "refs/heads/main", tip)
        code, summary, _ = self.observe()
        self.assertEqual(code, 1)
        self.assertIn("upstream-only: 25", summary)
        self.assertIn("5 additional commits", summary)
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
