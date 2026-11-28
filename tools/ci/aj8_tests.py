#!/usr/bin/env python3
"""AJ8-only listed/executed equality receipts. Does not build a release probe."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ANCHORS = {
    "bus": "css_decode::tests::aj8_t5_checked_finalized_evidence_rejects_each_false_relationship",
    "css": "model::tests::aj8_shared_semantic_classifier_and_partial_authored_resource_evidence",
    "runtime_css": "allocation_tests::aj8_worker_decoder_allocation_and_admitted_output_refusal",
    "browser": "tab::tests::aj8_stylesheets::aj8_t17_admitted_external_can_finish_while_host_becomes_inactive_without_retry",
}

# Authored required acceptance membership: preserved approved tests plus the
# correction regressions. New listed tests are also executed; none of these
# members may disappear merely because another package anchor still exists.
REQUIRED_TESTS = {'browser': ('document_style::reconcile::aj8_guards::aj8_sequential_projection_clone_and_prepared_commit_bounds',
             'tab::tests::aj8_stylesheets::aj8_a2_success_then_contradictory_failure_cannot_overwrite_facts',
             'tab::tests::aj8_stylesheets::aj8_a3_parser_integrity_preserves_detailed_first_fatal_cause',
             'tab::tests::aj8_stylesheets::aj8_a3_terminal_settlement_rejects_contradictory_retained_facts',
             'tab::tests::aj8_stylesheets::aj8_a4_failed_preparation_keeps_authoritative_owners_and_projection',
             'tab::tests::aj8_stylesheets::aj8_a4_unsupported_inline_becomes_supported_in_same_slot',
             'tab::tests::aj8_stylesheets::aj8_literal_mixed_static_stylesheet_observation',
             'tab::tests::aj8_stylesheets::aj8_observation_distinguishes_absent_empty_and_inconsistent_document',
             'tab::tests::aj8_stylesheets::aj8_parser_diagnostic_omission_is_separate_from_formatter_omission',
             'tab::tests::aj8_stylesheets::aj8_runtime_terminal_event_admission_negative_matrix',
             'tab::tests::aj8_stylesheets::aj8_t10_failed_decode_awaits_authenticated_done_and_duplicate_precedence',
             'tab::tests::aj8_stylesheets::aj8_t11_runtime_terminal_failure_keeps_ready_and_rejects_late_success',
             'tab::tests::aj8_stylesheets::aj8_t12_contradictory_decision_closes_admission_without_installing',
             'tab::tests::aj8_stylesheets::aj8_t13_order_reverse_completion_duplicates_and_sparse_rejections',
             'tab::tests::aj8_stylesheets::aj8_t14_recovery_diagnostic_caps_and_limited_parse_participation',
             'tab::tests::aj8_stylesheets::aj8_t15_observation_is_borrowed_bounded_redacted_and_allocation_bounded',
             'tab::tests::aj8_stylesheets::aj8_t16_encoding_context_replacement_retires_before_dispatch',
             'tab::tests::aj8_stylesheets::aj8_t17_admitted_external_can_finish_while_host_becomes_inactive_without_retry',
             'tab::tests::aj8_stylesheets::aj8_t7_host_type_selection_and_media_are_separate',
             'tab::tests::aj8_stylesheets::aj8_t8_prepared_first_eligibility_and_unissued_source_failure',
             'tab::tests::aj8_stylesheets::aj8_t9_same_slot_inline_eligibility_invalidates_retained_style',
             'tab::tests::aj8_stylesheets::aj8_correction_identity_provenance_and_current_inline_context',
             'tab::tests::aj8_stylesheets::aj8_correction_provenance_and_parser_length_negative_matrix',
             'tab::tests::aj8_stylesheets::aj8_correction_inline_fatal_publication_retains_coherent_preview_and_identity',
             'tab::tests::aj8_stylesheets::aj8_correction_literal_runtime_states_have_no_fabricated_pending_work',
             'tab::tests::aj8_stylesheets::aj8_correction_mixed_observation_allocation_regions'),
 'bus': ('css_decode::tests::aj8_t5_checked_finalized_evidence_rejects_each_false_relationship',
         'css_decode::tests::aj8_t6_canonical_decisions_and_redacted_dtos'),
 'css': ('model::tests::aj8_shared_semantic_classifier_and_partial_authored_resource_evidence',
         'resource_references::tests::aj8_semantic_at_rule_names_are_not_unescaped_again_by_ah8',
         'syntax::tests::aj8_parser_integrity_survives_diagnostic_suppression',
         'syntax::integrity_tests::aj8_integrity_actual_tokenizer_producer_survives_suppression_and_merge',
         'syntax::integrity_tests::aj8_integrity_structured_stylesheet_and_declaration_producers',
         'syntax::integrity_tests::aj8_integrity_actual_selector_and_semantic_name_producers',
         'syntax::integrity_tests::aj8_integrity_real_syntax_fuzz_consumers_reject_internal_faults'),
 'runtime_css': ('allocation_tests::aj8_worker_decoder_allocation_and_admitted_output_refusal',
                 'decoding::tests::aj8_a2_finalized_and_interrupted_accounting_are_distinct',
                 'decoding::tests::aj8_encoding_precedence_and_labels',
                 'decoding::tests::aj8_environment_uses_actual_aj4_encoding',
                 'decoding::tests::aj8_exact_charset_prefix_and_1024_boundary',
                 'decoding::tests::aj8_replacement_expansion_keeps_existing_parser_boundary',
                 'decoding::tests::aj8_utf8_partition_eof_and_literal_replacement',
                 'tests::aj8_a1_bounds_context_silent_cleanup_and_duplicate_done',
                 'tests::aj8_a1_complete_input_bounds_duplicates_abort_and_channels',
                 'tests::aj8_a1_initial_runtime_storage_refusal_has_no_tombstone',
                 'tests::aj8_a1_production_handoff_is_one_complete_body_then_done',
                 'allocation_tests::aj8_selection_and_report_validation_allocate_nothing'),
 'semantic-api': ('aj8_escaped_at_rule_names_keep_semantic_classification',)}

REQUIRED_TESTS["browser"] += ("tab::static_stylesheet_observation::tests::aj8_correction_record_boundaries_and_checked_coordinates",)

PARAMETER_ROWS = {"bus": [4,16], "css": [3,3,4,6,9], "runtime_css": [2,3,15,16,51], "browser": [2,2,2,5,6,9,10,14], "semantic-api": []}

def source_identity():
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"]).split(b"\0")
    rows = []
    for raw in sorted(set(paths)):
        if not raw: continue
        path = Path(os.fsdecode(raw))
        rows.append({"path": os.fsdecode(raw), "mode": oct(path.lstat().st_mode & 0o777) if path.exists() else None, "sha256": hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None})
    return hashlib.sha256(json.dumps(rows, sort_keys=True, separators=(",", ":")).encode()).hexdigest()

def verify_inventory(listed, output, anchor, expected_rows, required_tests=()):
    names = sorted(re.findall(r"^([^\s]+): test$", listed, re.M))
    if not names or anchor not in names or len(set(names)) != len(names):
        raise RuntimeError("missing/duplicate inventory or anchor")
    missing = sorted(set(required_tests) - set(names))
    if missing:
        raise RuntimeError(f"missing required acceptance tests: {missing!r}")
    executed = sorted(re.findall(r"^test ([^\s]+) \.\.\. (?:.*\n)*?ok$", output, re.M))
    if not executed:
        executed = sorted(re.findall(r"^test ([^\s]+) \.\.\. .*?\bok$", output, re.M))
    if names != executed:
        raise RuntimeError(f"listed/executed mismatch: {names!r} / {executed!r}")
    rows = sorted(map(int,re.findall(r"(?:parameter_rows=|AJ8 [a-z0-9 ]+rows=)(\d+)",output)))
    if rows != expected_rows:
        raise RuntimeError(f"parameter-row mismatch: {rows} / {expected_rows}")
    return {"listed": names, "executed": executed, "parameter_rows": rows}

def command(args, path):
    with path.open("xb") as output:
        result = subprocess.run(args, stdout=output, stderr=subprocess.STDOUT)
    path.with_suffix(".command.json").write_text(json.dumps({"argv": args, "exit_code": result.returncode}, indent=2) + "\n")
    if result.returncode:
        print(path.read_text(errors="replace")[-12000:], file=sys.stderr)
        raise RuntimeError(f"exit={result.returncode}: {args}; log={path}")
    return path.read_text(errors="strict")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--exact", action="store_true")
    args = parser.parse_args()
    args.evidence.mkdir(parents=True, exist_ok=False)
    receipt = {"head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(), "tree": subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], text=True).strip(), "diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "--binary", "HEAD"])).hexdigest(), "target": os.environ.get("CARGO_TARGET_DIR"), "release": args.release, "exact": args.exact, "inventories": {}, "required_tests": REQUIRED_TESTS}
    receipt["source_fingerprint"] = source_identity()
    try:
        packages = {"browser": ANCHORS["browser"]} if args.exact else ANCHORS
        receipt["unchanged_configurations_reused"] = ["bus", "css", "runtime_css", "semantic-api"] if args.exact else []
        for package, anchor in packages.items():
            base = ["cargo", "test", "-p", package, "--lib", "--locked"]
            if args.release:
                base += ["--release"]
            if args.exact and package == "browser":
                base += ["--features", "conformance-observation"]
            base += ["aj8_"]
            listed = command(base + ["--", "--list"], args.evidence / f"{package}-list.log")
            names = sorted(re.findall(r"^([^\s]+): test$", listed, re.M))
            if not names or anchor not in names:
                raise RuntimeError(f"missing {package} inventory/anchor")
            output = command(base + ["--", "--format", "pretty", "--color", "never", "--nocapture", "--test-threads=1"], args.evidence / f"{package}-run.log")
            expected_rows = PARAMETER_ROWS[package]
            receipt["inventories"][package] = verify_inventory(listed, output, anchor, expected_rows, REQUIRED_TESTS[package])
            print(f"{package}: {len(names)} listed/executed PASS", flush=True)
        if args.exact:
            receipt["result"] = "PASS"
            return
        base = ["cargo", "test", "-p", "css", "--test", "aj8_semantic_names", "--locked"]
        if args.release:
            base += ["--release"]
        listed = command(base + ["--", "--list"], args.evidence / "semantic-list.log")
        if "aj8_escaped_at_rule_names_keep_semantic_classification: test" not in listed:
            raise RuntimeError("missing semantic API regression")
        output = command(base, args.evidence / "semantic-run.log")
        receipt["inventories"]["semantic-api"] = verify_inventory(listed, output, "aj8_escaped_at_rule_names_keep_semantic_classification", [], REQUIRED_TESTS["semantic-api"])
        receipt["result"] = "PASS"
    except (OSError, RuntimeError, UnicodeError) as error:
        receipt["result"] = "FAIL"
        receipt["failure"] = str(error)
        raise
    finally:
        receipt["source_unchanged"] = receipt["source_fingerprint"] == source_identity()
        if not receipt["source_unchanged"]: receipt["result"] = "FAIL_SOURCE_DRIFT"
        (args.evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        if not receipt["source_unchanged"]: raise RuntimeError("AJ8 source drift during inventory execution")

if __name__ == "__main__":
    main()
