"""Execute Perf's actual inline programs with real Git in disposable repositories.

Cargo is replaced only in pipeline fault tests; those fixtures do not qualify
HTML allocations. Native hosted execution remains a separate acceptance gate.
"""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = (ROOT / ".github/workflows/perf.yml").read_text()


def workflow_program(name):
    """Extract the literal step body, following test_ci_qualification's pattern."""
    match = re.search(r"^      - name: " + re.escape(name) + r"\n", WORKFLOW, re.MULTILINE)
    if match is None:
        raise ValueError("workflow program missing: " + name)
    step = WORKFLOW[match.end():].split("\n      - ", 1)[0]
    lines = step.split("        run: |\n", 1)[1].splitlines()
    return textwrap.dedent("\n".join(lines)).strip() + "\n"


PATHS = workflow_program("Initialize allocation temporary paths")
SELECT = workflow_program("Select allocation source")
BEFORE = workflow_program("Verify allocation source before qualification")
TOOLCHAIN = workflow_program("Record active allocation toolchain")
ALLOCATE = workflow_program("Run allocation guards")
AFTER = workflow_program("Verify allocation source after qualification")
REQUIRED_GUARDS = (
    "atom_intern_reuse_does_not_allocate",
    "atom_lookup_path_does_not_allocate",
    "parse_document_attribute_values_avoid_unnecessary_allocs",
    "parse_document_plain_text_allocation_is_bounded",
    "parse_document_rawtext_allocation_is_bounded",
    "streaming_parser_chunk_sizes_do_not_cause_pathological_alloc_growth",
)


def allocation_log(names=REQUIRED_GUARDS, *, passed=None, ignored=0, filtered=0, expected_panic=()):
    """Representative native pretty output, including nocapture interleaving."""
    records = []
    for name in names:
        if name in expected_panic:
            # Native Rust 1.92 transcript shape; normalize PID/path and trim frames.
            records.append(
                f"test {name} - should panic ... \n"
                f"thread '{name}' (123) panicked at formatter-fixture.rs:15:51:\n"
                "fixture allocation unwind\nstack backtrace:\n"
                "   0: __rustc::rust_begin_unwind\n"
                f"   1: expected_panic_transcript::{name}\n"
                "note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.\n"
                "ok\n"
            )
            continue
        output = "fixture allocation measurements\nok" if name == REQUIRED_GUARDS[-1] else "ok"
        records.append(f"test {name} ... {output}\n")
    count = len(names) if passed is None else passed
    return (f"running {len(names)} tests\n" + "".join(records) +
            f"\ntest result: ok. {count} passed; 0 failed; {ignored} ignored; "
            f"0 measured; {filtered} filtered out; finished in 0.01s\n")


class PerfQualification(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="perf-qualification-tests-")
        self.addCleanup(temporary.cleanup)
        self.case = Path(temporary.name).resolve()
        self.repo = self.case / "source"
        self.repo.mkdir()
        self.runner_temp = self.case / "runner-temp"
        self.runner_temp.mkdir()
        self.output = self.case / "github-output"
        self.output.touch()
        self.env_file = self.case / "github-env"
        self.env_file.touch()
        self.environment = dict(
            os.environ,
            GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
            GIT_AUTHOR_NAME="Fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
            GIT_COMMITTER_NAME="Fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid",
            GIT_AUTHOR_DATE="2001-01-01T00:00:00+0000",
            GIT_COMMITTER_DATE="2001-01-01T00:00:00+0000",
            GITHUB_WORKSPACE=str(self.repo), RUNNER_TEMP=str(self.runner_temp),
            GITHUB_OUTPUT=str(self.output), GITHUB_ENV=str(self.env_file),
            GITHUB_EVENT_NAME="workflow_dispatch", GITHUB_REF="refs/heads/workflow-fixture",
            REQUESTED_SOURCE_SHA="", PYTHONDONTWRITEBYTECODE="1",
        )
        self.git("init", "-q")
        (self.repo / "input.txt").write_text("reviewed source\n")
        (self.repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.92.0"\n')
        self.git("add", ".")
        self.git("commit", "-qm", "Disposable source fixture")
        self.source = self.git("rev-parse", "HEAD")
        self.tree = self.git("rev-parse", "HEAD^{tree}")
        (self.repo / "input.txt").write_text("workflow revision differs from source\n")
        self.git("add", ".")
        self.git("commit", "-qm", "Disposable workflow fixture")
        self.event = self.git("rev-parse", "HEAD")
        self.git("checkout", "--detach", self.source)
        self.environment.update(
            GITHUB_SHA=self.event, WORKFLOW_SHA=self.event,
            WORKFLOW_REF="owner/repo/.github/workflows/perf.yml@refs/heads/workflow-fixture",
            SELECTED_SOURCE_SHA=self.source, EXPECTED_SOURCE_TREE=self.tree,
            ALLOCATION_OUTCOME="success",
        )
        result = self.run_program(PATHS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.environment.update(self.values(self.env_file))
        self.evidence = Path(self.environment["ALLOCATION_EVIDENCE_DIR"])
        self.target = Path(self.environment["CARGO_TARGET_DIR"])

    def git(self, *args):
        result = subprocess.run(
            ["git", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", *args],
            cwd=self.repo, env=self.environment, capture_output=True, text=True, timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    @staticmethod
    def values(path):
        return dict(line.split("=", 1) for line in path.read_text().splitlines())

    def run_program(self, program, **environment):
        return subprocess.run(
            ["bash", "-euo", "pipefail", "-c", program], cwd=self.repo,
            env=dict(self.environment, **environment), capture_output=True, text=True, timeout=30,
        )

    def assert_rejected(self, program, **environment):
        result = self.run_program(program, **environment)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse((self.evidence / "qualification-success.txt").exists())
        return result

    def test_scheduled_default_uses_event_sha(self):
        result = self.run_program(SELECT, GITHUB_EVENT_NAME="schedule", GITHUB_REF="refs/heads/master")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.values(self.output)["sha"], self.event)
        receipt = self.values(self.evidence / "source-selection.txt")
        self.assertEqual(receipt["event_ref"], "refs/heads/master")
        self.assertEqual(receipt["selected_source_sha"], self.event)

    def test_manual_default_uses_event_sha(self):
        result = self.run_program(SELECT)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.values(self.output)["sha"], self.event)

    def test_exact_override_keeps_workflow_event_and_source_identities_separate(self):
        result = self.run_program(SELECT, REQUESTED_SOURCE_SHA=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = self.values(self.evidence / "source-selection.txt")
        self.assertEqual(receipt["workflow_sha"], self.event)
        self.assertEqual(receipt["workflow_ref"], self.environment["WORKFLOW_REF"])
        self.assertEqual(receipt["event_ref"], self.environment["GITHUB_REF"])
        self.assertEqual(receipt["event_sha"], self.event)
        self.assertEqual(receipt["requested_source_sha"], self.source)
        self.assertEqual(receipt["selected_source_sha"], self.source)
        self.assertEqual(self.values(self.output)["sha"], self.source)

    def test_schedule_rejects_source_override(self):
        self.assert_rejected(SELECT, GITHUB_EVENT_NAME="schedule", REQUESTED_SOURCE_SHA=self.source)
        self.assertFalse((self.evidence / "source-selection.txt").exists())

    def test_unsupported_event_is_rejected(self):
        self.assert_rejected(SELECT, GITHUB_EVENT_NAME="pull_request")

    def test_malformed_source_input_cannot_execute_shell_or_become_a_ref(self):
        marker = self.case / "injected"
        invalid = [
            "master", "refs/heads/master", "v1.0", self.source[:12], "g" * 40,
            "A" * 40, " " + self.source, self.source + " ", self.source + "\n",
            self.source + "\nsha=" + self.event,
            "$(touch " + str(marker) + ")", "`touch " + str(marker) + "`",
        ]
        for value in invalid:
            with self.subTest(value=value):
                self.assert_rejected(SELECT, REQUESTED_SOURCE_SHA=value)
                self.assertNotIn("sha", self.values(self.output))
                self.assertFalse((self.evidence / "source-selection.txt").exists())
                self.assertFalse(marker.exists())

    def test_nonexistent_commit_is_rejected_before_qualification(self):
        self.assert_rejected(BEFORE, SELECTED_SOURCE_SHA="0" * 40)
        self.assertFalse((self.evidence / "source-before.txt").exists())

    def test_incorrect_checkout_head_is_rejected(self):
        self.assert_rejected(BEFORE, SELECTED_SOURCE_SHA=self.event)
        self.assertFalse((self.evidence / "source-before.txt").exists())

    def test_checkout_must_be_the_expected_workspace(self):
        self.assert_rejected(BEFORE, GITHUB_WORKSPACE=str(self.case))

    def test_clean_source_receipts_bind_head_and_tree(self):
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        self.assertEqual(self.values(self.output)["tree"], self.tree)
        receipt = self.values(self.evidence / "source-before.txt")
        self.assertEqual(receipt["source_sha"], self.source)
        self.assertEqual(receipt["source_tree"], self.tree)
        self.assertEqual(receipt["source_clean"], "true")
        after = self.run_program(AFTER)
        self.assertEqual(after.returncode, 0, after.stderr)
        receipt = self.values(self.evidence / "qualification-success.txt")
        self.assertEqual(receipt["qualified_source_sha"], self.source)
        self.assertEqual(receipt["qualified_source_tree"], self.tree)

    def dirty(self, state):
        if state == "untracked":
            (self.repo / "unexpected.txt").write_text("unexpected source\n")
        else:
            (self.repo / "input.txt").write_text("unexpected source\n")
            if state == "staged":
                self.git("add", "input.txt")

    def assert_dirty_rejected(self, state, program):
        self.dirty(state)
        self.assert_rejected(program)
        self.assertNotEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        receipt = "source-before.txt" if program == BEFORE else "source-after.txt"
        self.assertFalse((self.evidence / receipt).exists())

    def test_dirty_tracked_source_before_is_rejected(self):
        self.assert_dirty_rejected("tracked", BEFORE)

    def test_dirty_staged_source_before_is_rejected(self):
        self.assert_dirty_rejected("staged", BEFORE)

    def test_dirty_untracked_source_before_is_rejected(self):
        self.assert_dirty_rejected("untracked", BEFORE)

    def test_dirty_tracked_source_after_is_rejected(self):
        self.assert_dirty_rejected("tracked", AFTER)

    def test_dirty_staged_source_after_is_rejected(self):
        self.assert_dirty_rejected("staged", AFTER)

    def test_dirty_untracked_source_after_is_rejected(self):
        self.assert_dirty_rejected("untracked", AFTER)

    def test_changed_checkout_head_after_execution_is_rejected(self):
        self.git("checkout", "--detach", self.event)
        self.assert_rejected(AFTER)

    def test_wrong_tree_receipt_is_rejected(self):
        self.assert_rejected(AFTER, EXPECTED_SOURCE_TREE=self.git("rev-parse", self.event + "^{tree}"))

    def test_each_run_has_fresh_paths_outside_the_checkout(self):
        self.assertTrue(self.target.is_relative_to(self.runner_temp))
        self.assertTrue(self.evidence.is_relative_to(self.runner_temp))
        self.assertFalse(self.target.is_relative_to(self.repo))
        self.assertFalse(self.evidence.is_relative_to(self.repo))
        self.assertEqual(list(self.target.iterdir()), [])
        (self.target / "fixture-executable").write_text("not a real test executable\n")
        (self.evidence / "fixture.log").write_text("fixture evidence\n")
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), "")
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        result = self.run_program(PATHS)
        self.assertEqual(result.returncode, 0, result.stderr)
        paths = self.values(self.env_file)
        self.assertNotEqual(paths["CARGO_TARGET_DIR"], str(self.target))
        self.assertNotEqual(paths["ALLOCATION_EVIDENCE_DIR"], str(self.evidence))
        self.assertEqual(list(Path(paths["CARGO_TARGET_DIR"]).iterdir()), [])

    def cargo_fixture(self, *, listed=REQUIRED_GUARDS, execution=None, list_status="0"):
        directory = self.case / "bin"
        directory.mkdir()
        listing = directory / "listing.txt"
        listing.write_text("".join(name + ": test\n" for name in listed))
        output = directory / "execution.txt"
        output.write_text(allocation_log() if execution is None else execution)
        self.environment.update(FIXTURE_LISTING_FILE=str(listing), FIXTURE_EXECUTION_FILE=str(output),
                                FIXTURE_CARGO_LIST_STATUS=list_status)
        cargo = directory / "cargo"
        cargo.write_text(
            "#!/usr/bin/env bash\nset -euo pipefail\n"
            'if [[ " $* " == *" --list "* ]]; then\n'
            '  printf "%s\\n" "$@" > "$ALLOCATION_EVIDENCE_DIR/cargo-list-arguments.txt"\n'
            '  cat "$FIXTURE_LISTING_FILE"\n'
            '  exit "$FIXTURE_CARGO_LIST_STATUS"\n'
            'fi\n'
            'printf "%s\\n" "$@" > "$ALLOCATION_EVIDENCE_DIR/cargo-arguments.txt"\n'
            'printf "%s\\n" "$CARGO_TARGET_DIR" > "$ALLOCATION_EVIDENCE_DIR/cargo-target.txt"\n'
            "printf 'fixture cargo stdout\\n'\nprintf 'fixture cargo stderr\\n' >&2\n"
            'cat "$FIXTURE_EXECUTION_FILE"\n'
            'exit "$FIXTURE_CARGO_STATUS"\n'
        )
        cargo.chmod(0o755)
        return str(directory) + os.pathsep + self.environment["PATH"]

    def test_allocation_invocation_keeps_target_features_lock_and_serial_execution(self):
        result = self.run_program(ALLOCATE, PATH=self.cargo_fixture(), FIXTURE_CARGO_STATUS="0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.evidence / "cargo-arguments.txt").read_text().splitlines(), [
            "test", "-p", "html", "--test", "alloc_guards", "--locked", "--features",
            "html5,count-alloc", "--", "--test-threads=1", "--nocapture",
            "--format", "pretty", "--color", "never",
        ])
        self.assertEqual((self.evidence / "cargo-list-arguments.txt").read_text().splitlines(), [
            "test", "-p", "html", "--test", "alloc_guards", "--locked", "--features",
            "html5,count-alloc", "--", "--list", "--format", "terse", "--color", "never",
        ])
        self.assertEqual((self.evidence / "cargo-target.txt").read_text().strip(), str(self.target))
        log = (self.evidence / "alloc_guards.log").read_text()
        self.assertIn("fixture cargo stdout", log)
        self.assertIn("fixture cargo stderr", log)

    def assert_incomplete_allocation_rejected(self, **fixture):
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        execution = self.assert_rejected(ALLOCATE, PATH=self.cargo_fixture(**fixture), FIXTURE_CARGO_STATUS="0")
        after = self.run_program(AFTER, ALLOCATION_OUTCOME="failure")
        self.assertEqual(after.returncode, 0, after.stderr)
        self.assertEqual(self.values(self.evidence / "source-after.txt")["allocation_outcome"], "failure")
        self.assertFalse((self.evidence / "qualification-success.txt").exists())
        return execution

    def test_successful_cargo_with_zero_tests_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(execution=allocation_log((), filtered=6))

    def test_successful_cargo_with_empty_test_inventory_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(listed=(), execution=allocation_log(()))
        self.assertFalse((self.evidence / "cargo-arguments.txt").exists())

    def test_successful_cargo_without_libtest_execution_output_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(execution="")

    def test_ignored_required_guard_cannot_qualify(self):
        log = allocation_log(passed=5, ignored=1).replace(
            "fixture allocation measurements\nok", "ignored")
        self.assert_incomplete_allocation_rejected(execution=log)

    def test_filtered_required_guard_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(execution=allocation_log(REQUIRED_GUARDS[:-1], filtered=1))

    def test_missing_streaming_guard_cannot_hide_behind_successful_summary(self):
        self.assert_incomplete_allocation_rejected(execution=allocation_log(REQUIRED_GUARDS[:-1], passed=6))

    def test_required_streaming_guard_must_be_in_compiled_inventory(self):
        self.assert_incomplete_allocation_rejected(listed=REQUIRED_GUARDS[:-1],
                                                  execution=allocation_log(REQUIRED_GUARDS[:-1]))
        self.assertFalse((self.evidence / "cargo-arguments.txt").exists())

    def test_successful_cargo_without_final_summary_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(execution=allocation_log().split("test result:", 1)[0])

    def test_successful_cargo_with_incorrect_passed_count_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(execution=allocation_log(passed=0))

    def test_duplicate_success_summary_cannot_qualify(self):
        log = allocation_log()
        self.assert_incomplete_allocation_rejected(execution=log + log[log.index("test result:"):])

    def test_cargo_listing_failure_cannot_qualify(self):
        self.assert_incomplete_allocation_rejected(list_status="101")
        self.assertFalse((self.evidence / "cargo-arguments.txt").exists())

    def test_future_allocation_guard_additions_are_all_qualified(self):
        names = (*REQUIRED_GUARDS, "additional_allocation_guard")
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        execution = self.run_program(ALLOCATE, PATH=self.cargo_fixture(listed=names, execution=allocation_log(names)),
                                     FIXTURE_CARGO_STATUS="0")
        self.assertEqual(execution.returncode, 0, execution.stdout + execution.stderr)
        after = self.run_program(AFTER)
        self.assertEqual(after.returncode, 0, after.stderr)
        self.assertTrue((self.evidence / "qualification-success.txt").is_file())
        self.assertEqual((self.evidence / "alloc_guards-listed.txt").read_text(),
                         (self.evidence / "alloc_guards-executed.txt").read_text())

    def test_expected_panic_guard_with_nocapture_diagnostics_can_qualify(self):
        additional = "additional_expected_panic_allocation_guard"
        names = tuple(sorted((*REQUIRED_GUARDS, additional)))
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        execution = self.run_program(
            ALLOCATE,
            PATH=self.cargo_fixture(listed=names, execution=allocation_log(names, expected_panic=(additional,))),
            FIXTURE_CARGO_STATUS="0",
        )
        self.assertEqual(execution.returncode, 0, execution.stdout + execution.stderr)
        self.assertEqual((self.evidence / "alloc_guards-listed.txt").read_text().splitlines(), list(names))
        self.assertEqual((self.evidence / "alloc_guards-executed.txt").read_text().splitlines(), list(names))
        self.assertFalse((self.evidence / "qualification-success.txt").exists())
        after = self.run_program(AFTER)
        self.assertEqual(after.returncode, 0, after.stderr)
        receipt = self.values(self.evidence / "qualification-success.txt")
        self.assertEqual(receipt["qualified_source_sha"], self.source)
        self.assertEqual(receipt["qualified_source_tree"], self.tree)

    def test_missing_expected_panic_execution_record_cannot_qualify(self):
        names = (*REQUIRED_GUARDS, "additional_expected_panic_allocation_guard")
        self.assert_incomplete_allocation_rejected(listed=names, execution=allocation_log(passed=7))

    def test_ignored_expected_panic_guard_cannot_qualify(self):
        additional = "additional_expected_panic_allocation_guard"
        names = (*REQUIRED_GUARDS, additional)
        # Ignored tests have no test-mode annotation in Rust 1.92.
        log = allocation_log(names, passed=6, ignored=1).replace(
            f"test {additional} ... ok\n", f"test {additional} ... ignored\n")
        self.assert_incomplete_allocation_rejected(listed=names, execution=log)

    def test_expected_panic_success_with_dirty_source_cannot_issue_receipt(self):
        additional = "additional_expected_panic_allocation_guard"
        names = (*REQUIRED_GUARDS, additional)
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        execution = self.run_program(
            ALLOCATE,
            PATH=self.cargo_fixture(listed=names, execution=allocation_log(names, expected_panic=(additional,))),
            FIXTURE_CARGO_STATUS="0",
        )
        self.assertEqual(execution.returncode, 0, execution.stdout + execution.stderr)
        self.assertFalse((self.evidence / "qualification-success.txt").exists())
        self.assert_dirty_rejected("tracked", AFTER)

    def test_failed_allocation_pipeline_keeps_exit_status_and_final_source_evidence(self):
        before = self.run_program(BEFORE)
        self.assertEqual(before.returncode, 0, before.stderr)
        execution = self.run_program(ALLOCATE, PATH=self.cargo_fixture(), FIXTURE_CARGO_STATUS="101")
        self.assertEqual(execution.returncode, 101, execution.stderr)
        after = self.run_program(AFTER, ALLOCATION_OUTCOME="failure")
        self.assertEqual(after.returncode, 0, after.stderr)
        receipt = self.values(self.evidence / "source-after.txt")
        self.assertEqual(receipt["allocation_outcome"], "failure")
        self.assertEqual(receipt["source_clean"], "true")
        self.assertTrue((self.evidence / "alloc_guards.log").exists())
        self.assertFalse((self.evidence / "qualification-success.txt").exists())

    def test_incomplete_outcomes_cannot_issue_success_receipt(self):
        for outcome in ("failure", "skipped", "cancelled", "timed_out", ""):
            with self.subTest(outcome=outcome):
                result = self.run_program(AFTER, ALLOCATION_OUTCOME=outcome)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertFalse((self.evidence / "qualification-success.txt").exists())

    def test_successful_tests_with_dirty_source_cannot_issue_success_receipt(self):
        execution = self.run_program(ALLOCATE, PATH=self.cargo_fixture(), FIXTURE_CARGO_STATUS="0")
        self.assertEqual(execution.returncode, 0, execution.stderr)
        self.assert_dirty_rejected("tracked", AFTER)

    def toolchain_fixture(self):
        directory = self.case / "toolchain-bin"
        directory.mkdir()
        scripts = {
            "rustup": "printf 'fixture active toolchain\\n'\n",
            "rustc": 'printf "rustc %s (fixture)\\n" "$FIXTURE_RUSTC_VERSION"\n',
            "cargo": 'printf "cargo %s (fixture)\\n" "$FIXTURE_CARGO_VERSION"\n',
        }
        for name, body in scripts.items():
            executable = directory / name
            executable.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body)
            executable.chmod(0o755)
        return dict(PATH=str(directory) + os.pathsep + self.environment["PATH"],
                    FIXTURE_RUSTC_VERSION="1.92.0", FIXTURE_CARGO_VERSION="1.92.0")

    def test_toolchain_records_and_requires_pinned_compiler(self):
        result = self.run_program(TOOLCHAIN, **self.toolchain_fixture())
        self.assertEqual(result.returncode, 0, result.stderr)
        log = (self.evidence / "toolchain.txt").read_text()
        self.assertIn("fixture active toolchain", log)
        self.assertIn("rustc 1.92.0", log)
        self.assertIn("cargo 1.92.0", log)

    def test_unmatched_compiler_or_cargo_version_is_rejected(self):
        environment = self.toolchain_fixture()
        for key in ("FIXTURE_RUSTC_VERSION", "FIXTURE_CARGO_VERSION"):
            with self.subTest(version=key):
                self.assert_rejected(TOOLCHAIN, **dict(environment, **{key: "1.93.0"}))

    def test_repository_pin_must_match_installed_toolchain(self):
        (self.repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.91.0"\n')
        self.assert_rejected(TOOLCHAIN, **self.toolchain_fixture())

    def test_final_source_check_and_upload_are_not_suppressed_by_test_failure(self):
        after = WORKFLOW.split("      - name: Verify allocation source after qualification\n", 1)[1]
        after = after.split("\n      - ", 1)[0]
        self.assertIn("if: ${{ !cancelled() && steps.source_before.outcome == 'success' }}", after)
        upload = WORKFLOW.split("      - name: Upload allocation evidence\n", 1)[1]
        upload = upload.split("\n  html5_tokenizer_fuzz_long:", 1)[0]
        self.assertIn("if: ${{ always() && steps.paths.outcome == 'success' }}", upload)
        self.assertIn("path: ${{ steps.paths.outputs.evidence_dir }}", upload)
