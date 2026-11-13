"""Exercise the actual workflow programs and Cargo/libtest in disposable repos.

Fixtures use real Git, Cargo and libtest. A deadline-command fixture forwards
commands or injects exit 124 without making tests wait eight minutes.
No production qualification helper or alternative Rust test runner is involved.
"""

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import textwrap
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()
WORKSPACE_COMMANDS = dict(re.findall(r"^        run: &(default_workspace_tests|html5_workspace_tests) (.+)$", WORKFLOW, re.MULTILINE))
INTEGRATION = "a" * 40
HEAD = "b" * 40
INTEGRATION_TREE = "c" * 40
HEAD_TREE = "d" * 40
SOURCE_JOBS = {"source_provenance", "rust_correctness", "rust_release", "rust_conformance", "streaming_integration"}


def workflow_program(name):
    """Read one literal run block; do not reimplement its qualification logic."""
    # A shared step may be declared as a YAML mapping anchor; read its literal body.
    match = re.search(r"^      - name: " + re.escape(name) + r"\n|^        name: " + re.escape(name) + r"\n", WORKFLOW, re.MULTILINE)
    if match is None:
        raise ValueError("workflow program missing: " + name)
    step = WORKFLOW[match.end():]
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
INVENTORIES = workflow_program("Audit streaming guard replacement inventories")
INTEGRATION_GUARD = workflow_program("Qualify streaming guard on integration source")
COMPLETE = workflow_program("Verify completed source qualification")
LEGACY = workflow_program("Preserve required Rust status")


def job_block(name):
    block = WORKFLOW.split("\n  " + name + ":\n", 1)[1]
    return re.split(r"\n  [a-z0-9_]+:\n", block, maxsplit=1)[0]


gate_job = job_block("ci_qualification")
PREREQUISITES = re.findall(r"^      - ([a-z0-9_]+)$", gate_job, re.MULTILINE)


def bash(program, environment, cwd=None):
    return subprocess.run(["bash", "-euo", "pipefail", "-c", program],
                          cwd=cwd, env=dict(os.environ, **environment),
                          capture_output=True, text=True, timeout=120)


class QualificationGate(unittest.TestCase):
    def successful_results(self):
        results = {name: {"result": "success", "outputs": {}} for name in PREREQUISITES}
        for name in SOURCE_JOBS:
            results[name]["outputs"] = {"qualified_sha": INTEGRATION, "qualified_tree": INTEGRATION_TREE}
        results["source_provenance"]["outputs"]["pr_head_tree"] = HEAD_TREE
        results["streaming_head"]["outputs"] = {"qualified_sha": HEAD, "qualified_tree": HEAD_TREE}
        return results

    def gate(self, results, event="pull_request"):
        return bash(GATE, {"QUALIFICATION_RESULTS": json.dumps(results),
                           "GITHUB_EVENT_NAME": event, "INTEGRATION_SHA": INTEGRATION,
                           "PR_HEAD_SHA": HEAD if event == "pull_request" else ""})

    def test_gate_covers_every_normal_job(self):
        jobs = set(re.findall(r"^  ([a-z0-9_]+):$", WORKFLOW.split("\njobs:\n", 1)[1], re.MULTILINE))
        self.assertEqual(set(PREREQUISITES), jobs - {"ci_qualification", "rust", "ag4_proxy_timing"})
        self.assertEqual(len(PREREQUISITES), len(set(PREREQUISITES)))
        self.assertIn("always()", gate_job.split("    steps:", 1)[0])

    def test_all_success_requires_matching_integration_and_head(self):
        self.assertEqual(self.gate(self.successful_results()).returncode, 0)

    def test_push_requires_integration_without_fabricating_head_execution(self):
        results = self.successful_results()
        results["streaming_head"] = {"result": "skipped", "outputs": {}}
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
        for name in SOURCE_JOBS | {"streaming_head"}:
            for key in ("qualified_sha", "qualified_tree"):
                for value in (None, "e" * 40, ""):
                    with self.subTest(job=name, key=key, value=value):
                        results = copy.deepcopy(self.successful_results())
                        results[name]["outputs"][key] = value
                        self.assertNotEqual(self.gate(results).returncode, 0)

    def test_master_push_only_allows_the_explicit_head_job_skip(self):
        results = self.successful_results()
        self.assertNotEqual(self.gate(results, "push").returncode, 0)
        results["streaming_head"] = {"result": "skipped", "outputs": {"qualified_sha": HEAD}}
        self.assertNotEqual(self.gate(results, "push").returncode, 0)
        results["streaming_head"]["outputs"] = {}
        self.assertEqual(self.gate(results, "push").returncode, 0)
        results["rust_release"]["result"] = "skipped"
        self.assertNotEqual(self.gate(results, "push").returncode, 0)

    def test_head_tree_is_independent_of_integration_tree_but_must_match_head_object(self):
        results = self.successful_results()
        self.assertNotEqual(INTEGRATION_TREE, HEAD_TREE)
        self.assertEqual(self.gate(results).returncode, 0)
        results["streaming_head"]["outputs"]["qualified_tree"] = INTEGRATION_TREE
        self.assertNotEqual(self.gate(results).returncode, 0)

    def test_missing_or_malformed_head_tree_and_unknown_events_fail(self):
        for value in (None, "", "not-a-tree"):
            results = self.successful_results()
            results["source_provenance"]["outputs"]["pr_head_tree"] = value
            self.assertNotEqual(self.gate(results).returncode, 0)
        self.assertNotEqual(self.gate(self.successful_results(), "workflow_dispatch").returncode, 0)

    def test_legacy_required_rust_context_cannot_bypass_the_final_gate(self):
        block = job_block("rust")
        self.assertIn("    name: Rust (ubuntu-latest)\n", block)
        self.assertIn("always()", block)
        self.assertEqual(re.findall(r"^      - ([a-z0-9_]+)$", block, re.MULTILINE), ["ci_qualification"])
        self.assertEqual(bash(LEGACY, {"QUALIFICATION_RESULT": "success"}).returncode, 0)
        for value in ("failure", "skipped", "cancelled", "timed_out", "neutral", ""):
            self.assertNotEqual(bash(LEGACY, {"QUALIFICATION_RESULT": value}).returncode, 0)
        self.assertNotEqual(bash(LEGACY, {}).returncode, 0)

    def test_guard_faults_propagate_through_both_gates(self):
        for name in ("streaming_integration", "streaming_head"):
            for fault in ("failure", "cancelled", "skipped", "timed_out", "missing", "no_result", "no_outputs", "wrong_sha", "wrong_tree"):
                with self.subTest(job=name, fault=fault):
                    results = self.successful_results()
                    if fault == "missing":
                        del results[name]
                    elif fault == "no_result":
                        del results[name]["result"]
                    elif fault == "no_outputs":
                        results[name]["outputs"] = {}
                    elif fault.startswith("wrong_"):
                        results[name]["outputs"]["qualified_" + fault[6:]] = "e" * 40
                    else:
                        results[name]["result"] = fault
                    rejected = self.gate(results)
                    self.assertNotEqual(rejected.returncode, 0)
                    self.assertNotIn("Every qualification succeeded", rejected.stdout)
                    status = "failure" if rejected.returncode else "success"
                    self.assertNotEqual(bash(LEGACY, {"QUALIFICATION_RESULT": status}).returncode, 0)

    def test_parallel_work_jobs_do_not_recreate_the_serial_chain(self):
        for name in SOURCE_JOBS | {"streaming_head"}:
            block = job_block(name)
            self.assertNotIn("    needs:", block)
            if name != "source_provenance":
                self.assertIn("      - *source_complete", block)
                self.assertIn("      - *integration_checkout\n      - *integration_identity\n      - *committed_preflight\n", block)
        self.assertIn("if: ${{ github.event_name == 'pull_request' }}", job_block("streaming_head"))
        self.assertIn("fetch-depth: 0", job_block("source_provenance"))

    def test_cache_namespaces_and_mutable_outputs_are_isolated(self):
        keys = []
        for name in SOURCE_JOBS | {"streaming_head"}:
            block = job_block(name)
            keys.append(re.search(r"^          key: (.+)$", block, re.MULTILINE)[1])
            self.assertIn("prefix-key: v1-ci-1458", block)
            self.assertNotIn("shared-key:", block)
            self.assertIn("cache-workspace-crates: 'false'", block)
            self.assertIn("cache-on-failure: 'false'", block)
            self.assertIn("cache-bin: 'false'", block)
        self.assertEqual(len(keys), len(set(keys)))
        self.assertIn("cache-targets: 'false'", job_block("streaming_head"))
        self.assertIn("cache-targets: 'false'", job_block("source_provenance"))
        self.assertIn('export CARGO_TARGET_DIR="$RUNNER_TEMP/ci-1458-pr-head-target"', HEAD_GUARD)
        self.assertNotIn("download-artifact", WORKFLOW)

    def test_installed_compiler_matches_the_repository_pin(self):
        pinned = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        self.assertIn("toolchain: '" + pinned + "'", job_block("source_provenance"))
        for name in SOURCE_JOBS | {"streaming_head"}:
            if name != "source_provenance":
                self.assertIn("      - *pinned_toolchain", job_block(name))

    def test_workspace_lanes_replace_only_the_independently_required_guard(self):
        original = 'cargo test --workspace --lib --bins --tests --examples'
        suffix = ' --locked -- --skip aws::command::tests::native_procfs_ --skip runtime_parse_linear_streaming_guard'
        self.assertEqual(WORKSPACE_COMMANDS, {"default_workspace_tests": original + suffix,
                                             "html5_workspace_tests": original + ' --features html5' + suffix})
        block = job_block("streaming_integration")
        self.assertIn('DEFAULT_WORKSPACE_TEST_COMMAND: *default_workspace_tests', block)
        self.assertIn('HTML5_WORKSPACE_TEST_COMMAND: *html5_workspace_tests', block)
        for name in ("Record native streaming guard lane equivalence", "Audit streaming guard replacement inventories",
                     "Qualify streaming guard on integration source"):
            step = block.split('      - name: ' + name + '\n', 1)[1].split('      - ', 1)[0]
            self.assertNotRegex(step, r"(?m)^        (?:if|continue-on-error):")
        self.assertLess(block.index('Record native streaming guard lane equivalence'), block.index('Audit streaming guard replacement inventories'))
        self.assertLess(block.index('Audit streaming guard replacement inventories'), block.index('Qualify streaming guard on integration source'))
        self.assertLess(block.index('Qualify streaming guard on integration source'), block.index('      - *source_complete'))
        self.assertIn("--test\", \"perf_regression", LANES)
        self.assertNotIn('"-p", "runtime_parse"', LANES)

    def test_current_qualification_commands_have_explicit_owners(self):
        # Current qualification contract, independent of historical Git objects.
        # The successful original-command migration audit is retained as evidence.
        def commands(block):
            result = []
            lines = block.splitlines()
            for index, line in enumerate(lines):
                match = re.fullmatch(r"(?:      -|       ) run: (.+)", line)
                if not match:
                    continue
                command = match[1]
                if command == "|":
                    body = []
                    for row in lines[index + 1:]:
                        if row.strip() and not row.startswith("          "):
                            break
                        body.append(row)
                    command = textwrap.dedent("\n".join(body)).strip()
                command = re.sub(r"^&\w+ ", "", command)
                command = re.sub(r"\\\n\s*", " ", command)
                result.extend(line.strip() for line in command.splitlines()
                              if re.match(r"\s*(?:cargo|make|bash|python3?|env) ", line))
            return {tuple(shlex.split(re.sub(r" --timings(?=\s|$)", "", c))) for c in result}

        owned = {
            "source_provenance": ['cargo fetch --locked', 'cargo fetch --manifest-path fuzz/Cargo.toml --locked',
                                  'make check-url-dependencies', 'make check-network-dependencies'],
            "rust_correctness": [
                'cargo fmt --all -- --check',
                'cargo clippy --workspace --all-targets --locked -- -D warnings',
                'cargo clippy -p browser --all-targets --features conformance-observation --locked -- -D warnings',
                'cargo clippy --workspace --all-targets --features html5 --locked -- -D warnings',
                'cargo clippy --workspace --all-targets --features "html5 html5-fuzzing parser-conformance parser_invariants parser-failure-injection" --locked -- -D warnings',
                'cargo build --workspace --all-targets --locked',
                *WORKSPACE_COMMANDS.values(),
                'cargo test -p html --test html5_parser_conformance --features parser-conformance --locked',
                'make test-html5-external-fixtures', 'make test-html5-parser-fatal-failures',
                'cargo test -p html --test html5_golden_tree_builder --features "html5 dom-snapshot parser-conformance" --locked',
                'cargo test -p html --test html5_golden_tree_builder_patches --features html5 --locked',
                'cargo test -p html --test html5_smoke_real_pages --features "html5 dom-snapshot" --locked',
                'cargo test -p html --test html5_rawtext_script_regressions --features "html5 dom-snapshot parser_invariants" --locked',
                'cargo test -p html --test html5_pipeline_regressions --features "html5 html5-fuzzing dom-snapshot parser_invariants" --locked',
                'cargo test -p html --test wpt_html5_tree_builder --features "html5 dom-snapshot" --locked',
                'cargo build --workspace --all-targets --features html5 --locked', 'make html-entities-check'],
            "rust_release": ['cargo build --workspace --release --locked', 'cargo build --workspace --release --features html5 --locked',
                             'make qualify-network-release', 'cargo bench -p html --bench html_bench --features html5 --no-run --locked',
                             'make compile-css-benches'],
            "rust_conformance": ['cargo build -p borrowser-conformance --locked', 'make conformance-addressing', 'make conformance-ci'],
            "ag4_nonbillable": ['python -m pip install --require-hashes -r crates/conformance/aws/requirements.lock',
                                'python -m unittest discover -s crates/conformance/aws/tests -v',
                                'python -m unittest discover -s tools/conformance/tests -v'],
        }
        for job, required in owned.items():
            actual = commands(job_block(job))
            for command in required:
                self.assertIn(tuple(shlex.split(command)), actual, (job, command))

        fuzz = {
            'html5_tokenizer_fuzz_smoke': ['html5_tokenizer_fuzz_smoke'],
            'html5_tokenizer_script_data_fuzz_smoke': ['html5_tokenizer_script_data_fuzz_smoke'],
            'html5_tokenizer_rawtext_fuzz_smoke': ['html5_tokenizer_rawtext_fuzz_smoke'],
            'html5_tokenizer_rcdata_fuzz_smoke': ['html5_tokenizer_rcdata_fuzz_smoke'],
            'html5_tree_builder_token_fuzz_smoke': ['html5_tree_builder_tokens_fuzz_smoke'],
            'html5_pipeline_fuzz_smoke': ['html5_pipeline_fuzz_smoke'],
            'css_syntax_fuzz_smoke': ['css_tokenizer_fuzz_smoke', 'css_parser_fuzz_smoke'],
            'css_pipeline_fuzz_smoke': ['css_selector_parser_fuzz_smoke', 'css_selector_matching_fuzz_smoke', 'css_cascade_fuzz_smoke', 'css_values_fuzz_smoke'],
        }
        for job, scripts in fuzz.items():
            for script in scripts:
                self.assertIn(tuple(shlex.split(f'bash ./tools/ci/{script}.sh 2>&1 | tee ci_artifacts/{script}.log')), commands(job_block(job)))
        self.assertIn(tuple(shlex.split('cargo test -p css --features css-fuzzing --test css_fuzz_regressions --locked 2>&1 | tee ci_artifacts/css_fuzz_regressions.log')),
                      commands(job_block('css_fuzz_regressions')))

        native = commands(job_block('ag2_linux_chromium'))
        for command in ('cargo build -p borrowser-conformance --locked 2>&1 | tee "$AG2_EVIDENCE/build.log"',
                        'cargo clippy -p borrowser-conformance --all-targets --locked -- -D warnings 2>&1 | tee "$AG2_EVIDENCE/clippy.log"',
                        'cargo test -p borrowser-conformance --locked native_ -- --nocapture 2>&1 | tee "$AG2_EVIDENCE/native.log"',
                        'cargo test -p borrowser-conformance --locked -- --test-threads=1 --nocapture 2>&1 | tee "$AG2_EVIDENCE/conformance.log"',
                        'cargo run -p borrowser-conformance --locked -- --chromium > "$AG2_EVIDENCE/captures.json" 2> "$AG2_EVIDENCE/capture.stderr"',
                        'cargo run -p borrowser-conformance --locked -- --compare canvas/cascade > "$AG2_EVIDENCE/comparison.json" 2> "$AG2_EVIDENCE/comparison.stderr"'):
            self.assertIn(tuple(shlex.split(command)), native)
        for target, test, serial in (
            ('--bin borrowser-conformance', 'chromium::tests::real_chromium_capture_and_scripts', True),
            ('--bin borrowser-conformance', 'chromium::tests::real_chromium_lifecycle_and_topology', True),
            ('--bin borrowser-conformance', 'comparison::tests::real_chromium_comparison_composition', True),
            ('--bin borrowser-conformance', 'chromium::tests::real_chromium_ag5_read_only_inspection', True),
            ('--bin borrowser-conformance', 'chromium::tests::real_chromium_ag5_profile', True),
            ('--test chromium_cli', 'real_chromium_cli_has_stable_independent_observations', False),
            ('--test comparison_cli', 'real_chromium_combined_cli_is_stable_and_selects_exactly', False),
            ('--bin borrowser-conformance', 'chromium::tests::real_chromium_ag7_inert_links', True),
            ('--test comparison_cli', 'real_chromium_ag7_scoped_relationships', False),
        ):
            arguments = '--exact --ignored ' + ('--test-threads=1 ' if serial else '') + '--nocapture'
            command = f'cargo test -p borrowser-conformance --locked {target} {test} -- {arguments} 2>&1 | tee -a "$AG2_EVIDENCE/conformance.log"'
            self.assertIn(tuple(shlex.split(command)), native)
        self.assertIn(tuple(shlex.split('cargo test -p borrowser-conformance --locked --test comparison_cli ag8_real_chromium_suites_are_stable -- --exact --ignored --nocapture --test-threads=1 2>&1 | tee -a "$AG2_EVIDENCE/conformance.log"')), native)
        self.assertIn(tuple(shlex.split('env TMPDIR="$RUNNER_TEMP/ag2-profiles" BORROWSER_AG4_GUEST_HARNESS="$PWD/target/debug/borrowser-conformance" python3 -m unittest discover -s crates/conformance/aws/tests -p test_guest_capture.py 2>&1 | tee -a "$AG2_EVIDENCE/conformance.log"')), native)

    def test_each_build_job_retains_its_own_timing_reports(self):
        for name in ("rust_correctness", "rust_release", "rust_conformance"):
            block = job_block(name)
            self.assertIn(" --timings", block)
            self.assertIn("path: target/cargo-timings/", block)
            self.assertIn("if-no-files-found: error", block)
        self.assertIn("--message-format=json --timings", HEAD_GUARD)
        for name in ("streaming_integration", "streaming_head"):
            self.assertIn("if-no-files-found: error", job_block(name))


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
                                DEFAULT_WORKSPACE_TEST_COMMAND=WORKSPACE_COMMANDS["default_workspace_tests"],
                                HTML5_WORKSPACE_TEST_COMMAND=WORKSPACE_COMMANDS["html5_workspace_tests"],
                                RUNNER_TEMP=str(self.runtime), GITHUB_OUTPUT=str(self.runtime / "outputs"))
        self.git("init", "-q")
        self.git("config", "core.hooksPath", "/dev/null")
        self.git("config", "commit.gpgsign", "false")
        (self.repo / "Cargo.toml").write_text(
            '[workspace]\nmembers=["crates/runtime_parse", "crates/unrelated"]\nresolver="2"\n')
        pinned = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        (self.repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel="' + pinned + '"\nprofile="minimal"\n')
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

    def test_disposable_cargo_workspace_uses_the_repository_compiler(self):
        pinned = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        for command in (["rustc", "-vV"], ["cargo", "-vV"]):
            version = subprocess.run(command, cwd=self.repo, env=dict(os.environ, **self.environment),
                                     check=True, capture_output=True, text=True, timeout=30).stdout
            self.assertIn("release: " + pinned + "\n", version)

    def test_actual_cargo_listing_with_multiple_targets_and_execution_summaries(self):
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.commit_fixture()
        command = ["cargo", "test", "--workspace", "--lib", "--bins", "--tests", "--examples", "--locked",
                   "runtime_parse_linear_streaming_guard", "--", "--exact"]
        for color in ("always", "never"):
            with self.subTest(cargo_term_color=color):
                environment = dict(os.environ, **self.environment,
                                   CARGO_TARGET_DIR=str(self.runtime / "multi-target"), CARGO_TERM_COLOR=color)
                listing = subprocess.run(command + ["--list", "--format", "terse"], cwd=self.repo,
                                         env=environment, capture_output=True, text=True, timeout=120, check=True)
                # Cargo colors diagnostics even when captured; remove only SGR styling.
                diagnostics = re.sub(r"\x1b\[[0-9;]*m", "", listing.stderr)
                if color == "always":
                    self.assertNotEqual(diagnostics, listing.stderr)
                else:
                    self.assertEqual(diagnostics, listing.stderr)
                self.assertGreaterEqual(diagnostics.count("Running "), 6)
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
        self.assertEqual([lane["lane"] for lane in report["lanes"]], ["default", "html5", "narrow"])
        self.assertTrue(report["same_workspace_configurations"])
        self.assertTrue(report["narrow_configuration_subset"])
        self.assertTrue(report["narrow_guard_bytes_equal"])

    def test_unequal_lane_evidence_does_not_authorize_dropping_execution(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('default=["html5"]', 'default=[]'))
        (self.crate / "tests/perf_regression.rs").write_text('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.commit_fixture()
        environment = dict(self.environment, CARGO_TARGET_DIR=str(self.runtime / "lane-target"))
        result = bash(LANES, environment, self.repo)
        self.assertNotEqual(result.returncode, 0)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertFalse(report["same_guard_bytes"])
        self.assertFalse(report["same_workspace_configurations"])
        self.assertIn("workspace lane equivalence failed", result.stderr)

    def test_configuration_mismatch_is_fatal_even_when_guard_bytes_match(self):
        manifest = self.other / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('default=["html5"]', 'default=[]'))
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        failed = bash(LANES, environment, self.repo)
        self.assertNotEqual(failed.returncode, 0)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertTrue(report["same_guard_bytes"])
        self.assertFalse(report["same_workspace_configurations"])
        fixture = QualificationGate()
        results = fixture.successful_results()
        results["streaming_integration"] = {"result": "failure" if failed.returncode else "success", "outputs": {}}
        rejected = fixture.gate(results)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertNotEqual(bash(LEGACY, {"QUALIFICATION_RESULT": "failure" if rejected.returncode else "success"}).returncode, 0)
        self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())

    def test_guard_hash_mismatch_is_fatal_even_when_configurations_match(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        binaries = self.directory / "cargo-wrapper"
        binaries.mkdir()
        wrapper = binaries / "cargo"
        wrapper.write_text('#!' + shutil.which("python3") + '\n'
                           'import json, subprocess, sys\n'
                           'from pathlib import Path\n'
                           'result = subprocess.run([' + repr(shutil.which("cargo")) + ', *sys.argv[1:]], capture_output=True)\n'
                           'if result.returncode == 0 and "--features" in sys.argv and "--no-run" in sys.argv:\n'
                           '    for line in result.stdout.splitlines():\n'
                           '        item = json.loads(line)\n'
                           '        if item.get("reason") == "compiler-artifact" and item["target"]["name"] == "perf_regression":\n'
                           '            with Path(item["executable"]).open("ab") as binary:\n'
                           '                binary.write(b"changed after real Cargo compilation")\n'
                           'sys.stdout.buffer.write(result.stdout)\n'
                           'sys.stderr.buffer.write(result.stderr)\n'
                           'sys.exit(result.returncode)\n')
        wrapper.chmod(0o755)
        environment["PATH"] = str(binaries) + os.pathsep + os.environ["PATH"]
        result = bash(LANES, environment, self.repo)
        self.assertNotEqual(result.returncode, 0)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertTrue(report["same_workspace_configurations"])
        self.assertFalse(report["same_guard_bytes"])
        self.assertIn("workspace lane equivalence failed", result.stderr)

    def test_narrow_workspace_recipe_keeps_features_that_package_scoping_loses(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + 'instrumented=[]\n')
        manifest = self.other / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '[dev-dependencies]\n'
                            'runtime_parse={path="../runtime_parse",features=["instrumented"]}\n')
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.repo,
                       env=dict(os.environ, **self.environment), check=True, capture_output=True, timeout=30)
        environment = self.integration_fixture(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { assert!(cfg!(feature="instrumented")); }\n')
        result = bash(LANES, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads((self.runtime / "ci-1458-evidence/lane-equivalence.json").read_text())
        self.assertTrue(report["narrow_configuration_subset"])
        self.assertTrue(report["narrow_guard_bytes_equal"])
        for lane in report["lanes"]:
            self.assertIn("instrumented", lane["artifact"]["features"])
        package_only = subprocess.run(
            ["cargo", "test", "-p", "runtime_parse", "--test", "perf_regression", "--locked", "--no-run", "--message-format=json"],
            cwd=self.repo, env=dict(os.environ, **environment), check=True, capture_output=True, text=True, timeout=120)
        artifacts = [json.loads(line) for line in package_only.stdout.splitlines()]
        guard = next(a for a in artifacts if a.get("reason") == "compiler-artifact" and a["target"]["name"] == "perf_regression")
        self.assertNotIn("instrumented", guard["features"])
        self.assertNotEqual(hashlib.sha256(Path(guard["executable"]).read_bytes()).hexdigest(), report["lanes"][0]["sha256"])

    def integration_fixture(self, source):
        (self.crate / "tests/perf_regression.rs").write_text(source)
        (self.repo / ".gitignore").write_text("/target/\n")
        revision = self.commit_fixture()
        return dict(self.environment, GITHUB_SHA=revision, GITHUB_EVENT_NAME="push",
                    CARGO_TARGET_DIR=str(self.repo / "target"))

    def test_integration_guard_executes_only_the_bound_standard_test_and_completes_source(self):
        environment = self.integration_fixture(
            '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n#[test]\nfn unrelated() {}\n')
        for program in (LANES, INVENTORIES, INTEGRATION_GUARD, COMPLETE):
            result = bash(program, environment, self.repo)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        output = Path(environment["GITHUB_OUTPUT"]).read_text()
        self.assertIn("qualified_sha=" + environment["GITHUB_SHA"], output)
        self.assertIn("qualified_tree=" + self.git("rev-parse", "HEAD^{tree}"), output)
        inventory = json.loads((self.runtime / "ci-1458-evidence/replacement-inventory.json").read_text())
        self.assertEqual([x["lane"] for x in inventory], ["default", "html5"])
        self.assertTrue(all(x["before_records"] - x["after_records"] == 1 for x in inventory))
        guard_log = (self.runtime / "ci-1458-evidence/integration-guard.log").read_text()
        self.assertIn("1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out;", guard_log)

    def test_inventory_rejects_substring_filter_that_removes_another_test(self):
        environment = self.integration_fixture(
            '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n'
            '#[test]\nfn runtime_parse_linear_streaming_guard_suffix() {}\n')
        result = bash(LANES, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        result = bash(INVENTORIES, environment, self.repo)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("remove exactly", result.stderr)
        fixture = QualificationGate()
        results = fixture.successful_results()
        results["streaming_integration"] = {"result": "failure" if result.returncode else "success", "outputs": {}}
        rejected = fixture.gate(results)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertNotEqual(bash(LEGACY, {"QUALIFICATION_RESULT": "failure" if rejected.returncode else "success"}).returncode, 0)

    def test_replacement_commands_execute_other_targets_and_exclude_only_the_guard(self):
        environment = self.integration_fixture(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("guard must run independently"); }\n')
        for command in WORKSPACE_COMMANDS.values():
            result = subprocess.run(shlex.split(command), cwd=self.repo, env=dict(os.environ, **environment),
                                    capture_output=True, text=True, timeout=120)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            for name in ("ordinary_unit_test", "ordinary_binary_test", "ordinary_integration_test",
                         "another_unit_test", "another_integration_test"):
                self.assertIn("test " + name + " ... ok", result.stdout)
            self.assertNotIn("test runtime_parse_linear_streaming_guard ...", result.stdout)
        (self.other / "tests/unrelated.rs").write_text('#[test]\nfn another_integration_test() { panic!("remaining test failed"); }\n')
        self.commit_fixture()
        for command in WORKSPACE_COMMANDS.values():
            result = subprocess.run(shlex.split(command), cwd=self.repo, env=dict(os.environ, **environment),
                                    capture_output=True, text=True, timeout=120)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("another_integration_test ... FAILED", result.stdout)

    def test_inventory_rejects_absent_commands_and_missing_guard_exclusion(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        for variable in ("DEFAULT_WORKSPACE_TEST_COMMAND", "HTML5_WORKSPACE_TEST_COMMAND"):
            for missing in (True, False):
                invalid = dict(environment)
                if missing:
                    del invalid[variable]
                else:
                    invalid[variable] = invalid[variable].rsplit(" --skip runtime_parse_linear_streaming_guard", 1)[0]
                self.assertNotEqual(bash(INVENTORIES, invalid, self.repo).returncode, 0)

    def test_inventory_is_listing_only_and_does_not_execute_any_workload(self):
        environment = self.integration_fixture(
            '#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("listing executed guard"); }\n'
            '#[test]\nfn ordinary() { panic!("listing executed ordinary test"); }\n')
        self.assertEqual(bash(LANES, environment, self.repo).returncode, 0)
        result = bash(INVENTORIES, environment, self.repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_inventory_rejects_zero_and_duplicate_guard_names(self):
        for source in ('#[test]\nfn another() {}\n',
                       '#[test]\nfn runtime_parse_linear_streaming_guard() {}\n'):
            (self.crate / "tests/perf_regression.rs").write_text(source)
            (self.other / "tests/unrelated.rs").write_text(source)
            self.commit_fixture()
            environment = dict(self.environment, CARGO_TARGET_DIR=str(self.runtime / "inventory-target"))
            result = bash(INVENTORIES, environment, self.repo)
            self.assertNotEqual(result.returncode, 0)

    def test_integration_guard_rejects_ignored_and_failing_execution(self):
        for source in ('#[test]\n#[ignore]\nfn runtime_parse_linear_streaming_guard() {}\n',
                       '#[test]\nfn runtime_parse_linear_streaming_guard() { panic!("failure"); }\n'):
            environment = self.integration_fixture(source)
            self.assertEqual(bash(LANES, environment, self.repo).returncode, 0)
            result = bash(INTEGRATION_GUARD, environment, self.repo)
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())

    def test_integration_binding_rejects_changed_artifact_receipt(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertEqual(bash(LANES, environment, self.repo).returncode, 0)
        path = self.runtime / "ci-1458-evidence/lane-equivalence.json"
        report = json.loads(path.read_text())
        report["lanes"][0]["sha256"] = "0" * 64
        path.write_text(json.dumps(report))
        result = bash(INTEGRATION_GUARD, environment, self.repo)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("artifact changed", result.stderr)

    def test_integration_binding_rejects_stale_source_evidence(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertEqual(bash(LANES, environment, self.repo).returncode, 0)
        path = self.runtime / "ci-1458-evidence/lane-equivalence.json"
        original = json.loads(path.read_text())
        for field in ("executed_sha", "executed_tree"):
            report = copy.deepcopy(original)
            report[field] = "e" * 40
            path.write_text(json.dumps(report))
            result = bash(INTEGRATION_GUARD, environment, self.repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("source identity changed", result.stderr)

    def test_integration_binding_rejects_substituted_package_target_source_and_executable(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        self.assertEqual(bash(LANES, environment, self.repo).returncode, 0)
        path = self.runtime / "ci-1458-evidence/default.jsonl"
        original = [json.loads(line) for line in path.read_text().splitlines()]
        for substitution in ("package", "target", "source", "executable"):
            with self.subTest(substitution=substitution):
                artifacts = copy.deepcopy(original)
                guard = next(a for a in artifacts if a.get("reason") == "compiler-artifact"
                             and a["target"]["name"] == "perf_regression")
                if substitution == "package":
                    guard["manifest_path"] = str(self.other / "Cargo.toml")
                elif substitution == "target":
                    guard["target"]["kind"] = ["lib"]
                elif substitution == "source":
                    guard["target"]["src_path"] = str(self.crate / "tests/unrelated.rs")
                else:
                    guard["executable"] = str(self.repo / "Cargo.lock")
                path.write_text("\n".join(json.dumps(a) for a in artifacts) + "\n")
                result = bash(INTEGRATION_GUARD, environment, self.repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())

    def test_integration_collector_rejects_custom_harness(self):
        manifest = self.crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[test]]\nname="perf_regression"\n'
                            'path="tests/perf_regression.rs"\nharness=false\n')
        environment = self.integration_fixture('fn main() {}\n')
        result = bash(LANES, environment, self.repo)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("standard libtest harness", result.stderr)

    def test_completed_source_receipt_rejects_mutation_and_wrong_commit(self):
        environment = self.integration_fixture('#[test]\nfn runtime_parse_linear_streaming_guard() {}\n')
        (self.crate / "src/lib.rs").write_text("modified source\n")
        self.assertNotEqual(bash(COMPLETE, environment, self.repo).returncode, 0)
        self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())
        self.git("restore", "--", "crates/runtime_parse/src/lib.rs")
        environment["GITHUB_SHA"] = self.base
        self.assertNotEqual(bash(COMPLETE, environment, self.repo).returncode, 0)
        self.assertFalse(Path(environment["GITHUB_OUTPUT"]).exists())

    def test_source_check_requires_the_actual_executed_commit(self):
        actual = self.base
        good = bash(SOURCE, {"GITHUB_SHA": actual}, self.repo)
        bad = bash(SOURCE, {"GITHUB_SHA": "0" * 40}, self.repo)
        self.assertEqual(good.returncode, 0, good.stdout + good.stderr)
        self.assertNotEqual(bad.returncode, 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
