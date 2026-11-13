"""Exercise the actual workflow programs and Cargo/libtest in disposable repos.

Fixtures use real Git, Cargo and libtest. A deadline-command fixture forwards
commands or injects exit 124 without making tests wait eight minutes.
No production qualification helper or alternative Rust test runner is involved.
"""

import copy
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()
INTEGRATION = "a" * 40
HEAD = "b" * 40


def workflow_program(name):
    """Read one literal run block; do not reimplement its qualification logic."""
    step = WORKFLOW.split(f"      - name: {name}\n", 1)[1]
    lines = step.split("        run: |\n", 1)[1].splitlines()
    body = []
    for line in lines:
        if line.strip() and not line.startswith("          "):
            break
        body.append(line)
    return textwrap.dedent("\n".join(body)).strip() + "\n"


GATE = workflow_program("Require every qualification job to succeed")
HEAD_GUARD = workflow_program("Qualify streaming guard on exact PR head")
LANES = workflow_program("Record native streaming guard lane equivalence")
SOURCE = workflow_program("Verify qualification source")
INTEGRATION_PROGRAM = workflow_program("Verify integration source identity")
gate_job = WORKFLOW.split("\n  ci_qualification:\n", 1)[1]
PREREQUISITES = re.findall(r"^      - ([a-z0-9_]+)$", gate_job, re.MULTILINE)


def bash(program, environment, cwd=None):
    return subprocess.run(["bash", "-euo", "pipefail", "-c", program],
                          cwd=cwd, env=dict(os.environ, **environment),
                          capture_output=True, text=True, timeout=120)


class QualificationGate(unittest.TestCase):
    def successful_results(self):
        results = {name: {"result": "success", "outputs": {}} for name in PREREQUISITES}
        results["rust"]["outputs"] = {"integration_sha": INTEGRATION, "head_sha": HEAD}
        return results

    def gate(self, results, event="pull_request"):
        return bash(GATE, {"QUALIFICATION_RESULTS": json.dumps(results),
                           "GITHUB_EVENT_NAME": event, "INTEGRATION_SHA": INTEGRATION,
                           "PR_HEAD_SHA": HEAD if event == "pull_request" else ""})

    def test_gate_covers_every_normal_job(self):
        jobs = set(re.findall(r"^  ([a-z0-9_]+):$", WORKFLOW.split("\njobs:\n", 1)[1], re.MULTILINE))
        self.assertEqual(set(PREREQUISITES), jobs - {"ci_qualification", "ag4_proxy_timing"})
        self.assertEqual(len(PREREQUISITES), len(set(PREREQUISITES)))
        self.assertIn("always()", gate_job.split("    steps:", 1)[0])

    def test_all_success_requires_matching_integration_and_head(self):
        self.assertEqual(self.gate(self.successful_results()).returncode, 0)

    def test_push_requires_integration_without_fabricating_head_execution(self):
        results = self.successful_results()
        results["rust"]["outputs"].pop("head_sha")
        self.assertEqual(self.gate(results, "push").returncode, 0)

    def test_every_non_successful_prerequisite_blocks_qualification(self):
        for name in PREREQUISITES:
            for status in ("failure", "skipped", "cancelled", "timed_out", "neutral", None):
                with self.subTest(job=name, status=status):
                    results = self.successful_results()
                    results[name]["result"] = status
                    self.assertNotEqual(self.gate(results).returncode, 0)

    def test_missing_prerequisite_or_result_blocks_qualification(self):
        for name in PREREQUISITES:
            with self.subTest(job=name):
                results = self.successful_results()
                del results[name]
                self.assertNotEqual(self.gate(results).returncode, 0)
                results = self.successful_results()
                del results[name]["result"]
                self.assertNotEqual(self.gate(results).returncode, 0)

    def test_unexpected_or_malformed_results_are_rejected(self):
        results = self.successful_results()
        results["unexpected"] = {"result": "success"}
        for value in (results, [], None, {name: None for name in PREREQUISITES}):
            with self.subTest(value=value):
                self.assertNotEqual(self.gate(value).returncode, 0)
        self.assertNotEqual(bash(GATE, {"QUALIFICATION_RESULTS": "{"}).returncode, 0)

    def test_missing_or_mismatched_source_outputs_are_rejected(self):
        for key in ("integration_sha", "head_sha"):
            for value in (None, "c" * 40, ""):
                with self.subTest(key=key, value=value):
                    results = copy.deepcopy(self.successful_results())
                    results["rust"]["outputs"][key] = value
                    self.assertNotEqual(self.gate(results).returncode, 0)


class ExactHeadGuard(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ci-1458-fixture-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repo = self.directory / "repo"
        self.repo.mkdir()
        self.runtime = self.directory / "runner"
        self.runtime.mkdir()
        self.environment = dict(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                                GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
                                GIT_AUTHOR_DATE="2001-01-01T00:00:00+0000",
                                GIT_COMMITTER_DATE="2001-01-01T00:00:00+0000",
                                CARGO_NET_OFFLINE="true", CARGO_INCREMENTAL="0",
                                RUNNER_TEMP=str(self.runtime), GITHUB_OUTPUT=str(self.runtime / "outputs"))
        self.git("init", "-q")
        self.git("config", "core.hooksPath", "/dev/null")
        self.git("config", "commit.gpgsign", "false")
        (self.repo / "Cargo.toml").write_text(
            '[workspace]\nmembers=["crates/runtime_parse", "crates/unrelated"]\nresolver="2"\n')
        self.crate = self.repo / "crates/runtime_parse"
        (self.crate / "src").mkdir(parents=True)
        (self.crate / "tests").mkdir()
        (self.crate / "Cargo.toml").write_text(
            '[package]\nname="runtime_parse"\nversion="0.0.0"\nedition="2024"\n'
            '[features]\ndefault=["html5"]\nhtml5=[]\n')
        (self.crate / "src/lib.rs").write_text('#[test]\nfn ordinary_unit_test() {}\n')
        (self.crate / "src/main.rs").write_text('fn main() {}\n#[test]\nfn ordinary_binary_test() {}\n')
        (self.crate / "tests/unrelated.rs").write_text('#[test]\nfn ordinary_integration_test() {}\n')
        self.other = self.repo / "crates/unrelated"
        (self.other / "src").mkdir(parents=True)
        (self.other / "tests").mkdir()
        (self.other / "Cargo.toml").write_text(
            '[package]\nname="unrelated"\nversion="0.0.0"\nedition="2024"\n'
            '[features]\ndefault=["html5"]\nhtml5=[]\n')
        (self.other / "src/lib.rs").write_text('#[test]\nfn another_unit_test() {}\n')
        (self.other / "tests/unrelated.rs").write_text('#[test]\nfn another_integration_test() {}\n')
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn seed_test() {}\n')
        (self.repo / "Cargo.lock").write_text(
            'version = 4\n[[package]]\nname = "runtime_parse"\nversion = "0.0.0"\n'
            '[[package]]\nname = "unrelated"\nversion = "0.0.0"\n')
        self.base = self.commit_fixture()

    def git(self, *arguments, cwd=None):
        result = subprocess.run(["git", *arguments], cwd=cwd or self.repo,
                                env=dict(os.environ, **self.environment),
                                capture_output=True, text=True, timeout=30, check=True)
        return result.stdout.strip()

    def commit_fixture(self, cwd=None):
        self.git("add", ".", cwd=cwd)
        self.git("commit", "-q", "-m", "Disposable qualification fixture", cwd=cwd)
        return self.git("rev-parse", "HEAD", cwd=cwd)

    def prepare_head(self, test_source, duplicate=False):
        test = self.crate / "tests/perf_regression.rs"
        test.write_text(test_source)
        if duplicate:
            (self.crate / "src/lib.rs").write_text(test_source)
        head = self.commit_fixture()
        self.git("reset", "--hard", self.base)
        self.git("merge", "--no-ff", "--no-commit", head)
        test.write_text('#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("wrong source tree"); }\n')
        integration = self.commit_fixture()
        return head, integration

    def execute_head(self, head, integration, repository=None, fail_phase=None, head_override=None):
        repository = repository or self.repo
        environment = dict(self.environment, PR_HEAD_SHA=head_override or head, GITHUB_SHA=integration)
        binaries = self.directory / "bin"
        binaries.mkdir()
        wrapper = binaries / "timeout"
        wrapper.write_text('#!/bin/sh\nset -eu\ntest "$1" = --kill-after=10s\n'
                           'case "$2" in 10m|8m|30s) ;; *) exit 70 ;; esac\n'
                           + (f'if [ "$2" = {fail_phase} ]; then exit 124; fi\n' if fail_phase else '')
                           + 'shift 2\nexec "$@"\n')
        wrapper.chmod(0o755)
        environment["PATH"] = str(binaries) + os.pathsep + os.environ["PATH"]
        result = bash(HEAD_GUARD, environment, repository)
        self.assertEqual(self.git("rev-parse", "HEAD", cwd=repository), integration)
        self.assertEqual(self.git("status", "--porcelain", cwd=repository), "")
        output = Path(environment["GITHUB_OUTPUT"])
        return result, output.read_text() if output.exists() else "", head

    def run_head(self, test_source, duplicate=False, force_timeout=False, head_override=None):
        head, integration = self.prepare_head(test_source, duplicate)
        return self.execute_head(head, integration, fail_phase="8m" if force_timeout else None,
                                 head_override=head_override)

    def integration_environment(self, head, integration):
        return dict(self.environment, GITHUB_EVENT_NAME="pull_request", GITHUB_SHA=integration,
                    PR_BASE_SHA=self.base, PR_HEAD_SHA=head,
                    GITHUB_OUTPUT=str(self.runtime / "integration-outputs"))

    def test_executes_exact_head_with_separate_target_and_success_record(self):
        result, output, head = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output, f"head_sha={head}\n")
        self.assertTrue((self.runtime / "ci-1458-pr-head-target").is_dir())
        self.assertFalse((self.repo / "target").exists())
        self.assertFalse((self.runtime / "ci-1458-pr-head").exists())
        artifact = json.loads((self.runtime / "ci-1458-evidence/pr-head-guard-artifact.json").read_text())["artifact"]
        self.assertEqual(artifact["target"]["name"], "perf_regression")
        self.assertEqual(artifact["target"]["kind"], ["test"])

    def test_actual_cargo_listing_with_multiple_targets_and_execution_summaries(self):
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.commit_fixture()
        command = ["cargo", "test", "--workspace", "--lib", "--bins", "--tests", "--examples", "--locked",
                   "runtime_parse_linear_streaming_guard", "--", "--exact"]
        environment = dict(os.environ, **self.environment, CARGO_TARGET_DIR=str(self.runtime / "multi-target"))
        listing = subprocess.run(command + ["--list", "--format", "terse"], cwd=self.repo,
                                 env=environment, capture_output=True, text=True, timeout=120, check=True)
        self.assertGreaterEqual(listing.stderr.count("Running "), 6)
        self.assertEqual(listing.stdout, "runtime_parse_linear_streaming_guard: test\n")
        execution = subprocess.run(command, cwd=self.repo, env=environment,
                                   capture_output=True, text=True, timeout=120, check=True)
        self.assertGreaterEqual(execution.stdout.count("test result: ok. 0 passed;"), 5)
        self.assertEqual(execution.stdout.count("test runtime_parse_linear_streaming_guard ... ok"), 1)

    def test_unrelated_tests_in_bound_target_may_be_filtered_without_losing_guard(self):
        result, output, head = self.run_head(
            '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n'
            '#[test]\nfn another_regression() {}\n')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output, f"head_sha={head}\n")
        self.assertIn("1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;", result.stdout)

    def test_custom_harness_cannot_fabricate_libtest_qualification(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[test]]\nname="perf_regression"\n'
                            'path="tests/perf_regression.rs"\nharness=false\n')
        result, output, _ = self.run_head('''fn main() {
            if std::env::args().any(|arg| arg == "--list") {
                println!("runtime_parse_linear_streaming_guard: test");
            } else {
                println!("test runtime_parse_linear_streaming_guard ... ok");
                println!("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s");
            }
        }
        ''')
        artifacts = [json.loads(line) for line in
                     (self.runtime / "ci-1458-evidence/pr-head-artifacts.jsonl").read_text().splitlines()]
        guard = next(a for a in artifacts if a.get("reason") == "compiler-artifact"
                     and a["target"]["name"] == "perf_regression")
        # On pinned Cargo 1.92.0, this field does not distinguish a custom harness.
        self.assertIs(guard["profile"]["test"], True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")
        self.assertIn("standard libtest harness", result.stderr)
        self.assertFalse((self.runtime / "ci-1458-evidence/pr-head-guard.log").exists())

    def test_explicit_standard_harness_accepts_guard_and_filtered_tests(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[test]]\nname="perf_regression"\n'
                            'path="tests/perf_regression.rs"\nharness=true\n')
        result, output, head = self.run_head(
            '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n'
            '#[test]\nfn another_regression() {}\n')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output, f"head_sha={head}\n")
        self.assertIn("1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;", result.stdout)

    def test_unrelated_custom_harness_does_not_reject_standard_guard(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[test]]\nname="unrelated_custom"\n'
                            'path="tests/unrelated_custom.rs"\nharness=false\n')
        (self.crate / "tests/unrelated_custom.rs").write_text('fn main() {}\n')
        result, output, head = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output, f"head_sha={head}\n")

    def test_same_name_in_unit_test_target_cannot_qualify_missing_integration_test(self):
        (self.crate / "src/lib.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        result, output, _ = self.run_head('#[test]\nfn other_test() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_same_name_in_different_package_cannot_qualify(self):
        (self.other / "src/lib.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        result, output, _ = self.run_head('#[test]\nfn other_test() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_same_name_in_different_integration_target_cannot_qualify(self):
        (self.crate / "tests/unrelated.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        result, output, _ = self.run_head('#[test]\nfn other_test() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_unrelated_diagnostics_cannot_impersonate_selected_test(self):
        manifest = self.other / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[test]]\nname="diagnostic"\nharness=false\n')
        (self.other / "tests/diagnostic.rs").write_text(
            'fn main() { println!("runtime_parse_linear_streaming_guard: test"); }\n')
        result, output, _ = self.run_head('#[test]\nfn other_test() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_zero_selected_tests_cannot_succeed(self):
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard_suffix() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_ignored_guard_cannot_satisfy_qualification(self):
        result, output, _ = self.run_head('#[test]\n#[ignore]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_duplicate_selection_cannot_satisfy_qualification(self):
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n', duplicate=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_guard_failure_survives_tee(self):
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("failure"); }\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_compilation_failure_cannot_satisfy_qualification(self):
        result, output, _ = self.run_head('this does not compile\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_execution_timeout_cannot_publish_success(self):
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n', force_timeout=True)
        self.assertEqual(result.returncode, 124)
        self.assertEqual(output, "")

    def test_preparation_timeout_cannot_publish_success(self):
        head, integration = self.prepare_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        result, output, _ = self.execute_head(head, integration, fail_phase="10m")
        self.assertEqual(result.returncode, 124)
        self.assertEqual(output, "")

    def test_cleanup_failure_cannot_publish_success(self):
        head, integration = self.prepare_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        result, output, _ = self.execute_head(head, integration, fail_phase="30s")
        self.assertEqual(result.returncode, 124)
        self.assertEqual(output, "")
        self.assertTrue((self.runtime / "ci-1458-pr-head").exists())

    def test_unknown_head_commit_cannot_fall_back_to_integration(self):
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n',
                                        head_override="0" * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")
        self.assertFalse((self.runtime / "ci-1458-pr-head").exists())

    def test_worktree_creation_failure_cannot_publish_success(self):
        occupied = self.runtime / "ci-1458-pr-head"
        occupied.mkdir()
        (occupied / "existing.txt").write_text("preserve existing directory")
        result, output, _ = self.run_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")
        self.assertEqual((occupied / "existing.txt").read_text(), "preserve existing directory")

    def test_modified_head_source_cannot_publish_success(self):
        result, output, _ = self.run_head(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { '
            'std::fs::write("src/lib.rs", "modified during execution").unwrap(); }\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")

    def test_head_commit_changed_during_successful_test_cannot_publish_success(self):
        result, output, _ = self.run_head(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { '
            'let result = std::process::Command::new("git").args(["reset", "--hard", "'
            + self.base + '"]).output().unwrap(); assert!(result.status.success()); }\n')
        self.assertIn("test runtime_parse_linear_streaming_guard ... ok", result.stdout)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(output, "")
        self.assertEqual(self.git("rev-parse", "HEAD", cwd=self.runtime / "ci-1458-pr-head"), self.base)

    def test_phase_limits_leave_explicit_headroom_inside_outer_timeout(self):
        settings = WORKFLOW.split("      - name: Qualify streaming guard on exact PR head\n", 1)[1]
        settings = settings.split("        run: |\n", 1)[0]
        outer = int(re.search(r"timeout-minutes: (\d+)", settings)[1]) * 60
        limits = re.findall(r"timeout --kill-after=(\d+)s (\d+)(m|s) ", HEAD_GUARD)
        self.assertEqual(len(limits), 3)
        maximum = sum(int(grace) + int(amount) * (60 if unit == "m" else 1)
                      for grace, amount, unit in limits)
        self.assertGreaterEqual(outer - maximum, 60)

    def test_integration_identity_requires_exact_merge_and_parent_order(self):
        head, integration = self.prepare_head('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        for key, bad in (("GITHUB_SHA", self.base), ("PR_BASE_SHA", "0" * 40),
                         ("PR_HEAD_SHA", self.base)):
            with self.subTest(key=key):
                environment = self.integration_environment(head, integration)
                environment[key] = bad
                result = bash(INTEGRATION_PROGRAM, environment, self.repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())
        environment = self.integration_environment(head, integration)
        result = bash(INTEGRATION_PROGRAM, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(Path(environment["GITHUB_OUTPUT"]).read_text(), f"integration_sha={integration}\n")

    def test_master_push_identity_does_not_require_pr_parents(self):
        environment = self.integration_environment("", self.base)
        environment.update(GITHUB_EVENT_NAME="push", PR_BASE_SHA="", PR_HEAD_SHA="")
        result = bash(INTEGRATION_PROGRAM, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(Path(environment["GITHUB_OUTPUT"]).read_text(), f"integration_sha={self.base}\n")

    def test_fork_head_is_available_from_targeted_merge_fetch_without_fork_remote(self):
        fork = self.directory / "fork"
        self.git("clone", "--no-local", "--quiet", str(self.repo), str(fork))
        (fork / "crates/runtime_parse/tests/perf_regression.rs").write_text(
            '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        head = self.commit_fixture(cwd=fork)
        self.git("fetch", "--quiet", str(fork), head)
        self.git("merge", "--no-ff", "--no-commit", head)
        (self.crate / "tests/perf_regression.rs").write_text(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("integration is not the head"); }\n')
        integration = self.commit_fixture()
        origin = self.directory / "base-origin.git"
        self.git("clone", "--bare", "--no-local", "--quiet", str(self.repo), str(origin))
        branch = self.git("symbolic-ref", "HEAD")
        self.git("update-ref", branch, self.base, cwd=origin)
        self.git("update-ref", "refs/pull/7/merge", integration, cwd=origin)
        checkout = self.directory / "checkout"
        checkout.mkdir()
        self.git("init", "--quiet", cwd=checkout)
        self.git("remote", "add", "origin", origin.as_uri(), cwd=checkout)
        # Match checkout@v4: all ordinary history, then a targeted SHA fetch if absent.
        self.git("fetch", "--quiet", "--no-tags", "origin", "+refs/heads/*:refs/remotes/origin/*", cwd=checkout)
        absent = subprocess.run(["git", "cat-file", "-e", head + "^{commit}"], cwd=checkout,
                                capture_output=True, text=True, timeout=30)
        self.assertNotEqual(absent.returncode, 0)
        self.git("fetch", "--quiet", "--no-tags", "origin", integration, cwd=checkout)
        self.git("checkout", "--quiet", "--detach", integration, cwd=checkout)
        self.git("cat-file", "-e", head + "^{commit}", cwd=checkout)
        self.assertEqual(self.git("remote", cwd=checkout), "origin")
        environment = self.integration_environment(head, integration)
        result = bash(INTEGRATION_PROGRAM, environment, checkout)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        result, output, _ = self.execute_head(head, integration, repository=checkout)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output, f"head_sha={head}\n")

    def test_lane_evidence_records_the_real_compiled_artifacts(self):
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.commit_fixture()
        environment = dict(self.environment, CARGO_TARGET_DIR=str(self.runtime / "lane-target"))
        result = bash(LANES, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertTrue(report["same_guard_bytes"])
        self.assertEqual(report["executed_sha"], self.git("rev-parse", "HEAD"))
        self.assertEqual([lane["lane"] for lane in report["lanes"]], ["default", "html5"])

    def test_unequal_lane_evidence_does_not_authorize_dropping_execution(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('default=["html5"]', 'default=[]'))
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.commit_fixture()
        environment = dict(self.environment, CARGO_TARGET_DIR=str(self.runtime / "lane-target"))
        result = bash(LANES, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertFalse(report["same_guard_bytes"])

    def test_source_check_requires_the_actual_executed_commit(self):
        actual = self.base
        good = bash(SOURCE, {"GITHUB_SHA": actual}, self.repo)
        bad = bash(SOURCE, {"GITHUB_SHA": "0" * 40}, self.repo)
        self.assertEqual(good.returncode, 0, good.stdout + good.stderr)
        self.assertNotEqual(bad.returncode, 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
