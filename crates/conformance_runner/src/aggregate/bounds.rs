//! Checked syntactic ceiling derivations for the AG9d binary protocols.

use super::baseline::{MAX_ADVISORY_RESULT_BYTES_V1, MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1};

pub(crate) const MAX_EXTERNAL_RECORDS_V1: usize = 256;
const B_FRAME: usize = 8;
const SECTION_FRAME: usize = 2 + B_FRAME;
const OPTION_PRESENT_FRAME: usize = 1 + B_FRAME;
const SHA256: usize = 32;
const U32: usize = 4;
const U64: usize = 8;

const fn string(maximum_payload: usize) -> usize {
    B_FRAME + maximum_payload
}

pub(crate) const BASELINE_VERSION_SECTION_MAX_V1: usize = 679;
pub(crate) const BASELINE_CAPTURE_RECORD_MAX_V1: usize = B_FRAME
    + SHA256
    + B_FRAME
    + external_test_provenance::MAX_EXTERNAL_CAPTURE_ID_PREIMAGE_BYTES_V1;
pub(crate) const BASELINE_TRACK_RECORD_MAX_V1: usize = B_FRAME
    + string(128)
    + string(128)
    + string(128)
    + string(128)
    + string(26)
    + string(128)
    + string(128)
    + string(43)
    + string(128)
    + string(128);
pub(crate) const BASELINE_DOM_VARIANT_KEY_MAX_V1: usize = string(128) + string(8) + string(9);
pub(crate) const BASELINE_VARIANT_KEY_MAX_V1: usize =
    string(128) + string(24) + string(9) + string(25) + U32;
pub(crate) const BASELINE_POINT_RECORD_MAX_V1: usize =
    B_FRAME + BASELINE_DOM_VARIANT_KEY_MAX_V1 + string(26) + string(128) + SHA256;
pub(crate) const BASELINE_OPERATION_DESCRIPTOR_MAX_V1: usize =
    string(21) + BASELINE_VARIANT_KEY_MAX_V1 + string(26) + string(9);
pub(crate) const BASELINE_SLOT_RECORD_MAX_V1: usize =
    B_FRAME + MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1;
pub(crate) const BASELINE_NOTE_RECORD_MAX_V1: usize = B_FRAME
    + string(128)
    + BASELINE_DOM_VARIANT_KEY_MAX_V1
    + string(26)
    + string(1_024)
    + OPTION_PRESENT_FRAME
    + SHA256;
pub(crate) const BASELINE_FIXED_MAX_V1: usize =
    33 + 1 + 2 + 7 * SECTION_FRAME + BASELINE_VERSION_SECTION_MAX_V1;
pub(crate) const BASELINE_ADVISORY_MAX_V1: usize = U32
    + MAX_EXTERNAL_RECORDS_V1 * BASELINE_CAPTURE_RECORD_MAX_V1
    + U32
    + MAX_EXTERNAL_RECORDS_V1 * BASELINE_TRACK_RECORD_MAX_V1
    + U32
    + MAX_EXTERNAL_RECORDS_V1 * BASELINE_POINT_RECORD_MAX_V1
    + BASELINE_OPERATION_DESCRIPTOR_MAX_V1
    + U32
    + MAX_EXTERNAL_RECORDS_V1 * BASELINE_SLOT_RECORD_MAX_V1
    + U32
    + MAX_EXTERNAL_RECORDS_V1 * BASELINE_NOTE_RECORD_MAX_V1;
pub(crate) const BASELINE_DERIVED_MAX_V1: usize =
    super::report::AGGREGATE_DETAIL_MAX_BYTES_V1 + BASELINE_FIXED_MAX_V1 + BASELINE_ADVISORY_MAX_V1;

pub(crate) const TREND_VERSION_SECTION_MAX_V1: usize = string(33) + BASELINE_VERSION_SECTION_MAX_V1;
pub(crate) const TREND_CONTEXT_MAX_V1: usize =
    string(21) + string(46) + string(18) + string(23) + 4 * SHA256;
pub(crate) const TREND_EVALUATION_SUMMARY_MAX_V1: usize =
    2 * (B_FRAME + BASELINE_OPERATION_DESCRIPTOR_MAX_V1) + 6 * U64;
pub(crate) const TREND_POPULATION_HEADER_MAX_V1: usize = 6 * U64 + U32;
pub(crate) const TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1: usize =
    BASELINE_DOM_VARIANT_KEY_MAX_V1 + string(26) + string(128);

pub(crate) const TREND_ADVISORY_REMOVED_RECORD_MAX_V1: usize = B_FRAME
    + string(7)
    + B_FRAME
    + TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1
    + OPTION_PRESENT_FRAME
    + SHA256
    + 1
    + OPTION_PRESENT_FRAME
    + 1
    + 1
    + 1;
pub(crate) const TREND_ADVISORY_ADDED_RECORD_MAX_V1: usize = B_FRAME
    + string(5)
    + B_FRAME
    + TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1
    + 1
    + OPTION_PRESENT_FRAME
    + SHA256
    + 1
    + OPTION_PRESENT_FRAME
    + 1
    + 1;
pub(crate) const TREND_ADVISORY_UNCHANGED_RECORD_MAX_V1: usize = B_FRAME
    + string(9)
    + B_FRAME
    + TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1
    + 2 * (OPTION_PRESENT_FRAME + SHA256)
    + 2 * (OPTION_PRESENT_FRAME + 1)
    + 1;
pub(crate) const TREND_NOTE_REMOVED_RECORD_MAX_V1: usize =
    B_FRAME + string(7) + B_FRAME + string(128) + OPTION_PRESENT_FRAME + SHA256 + 1;
pub(crate) const TREND_NOTE_ADDED_RECORD_MAX_V1: usize =
    B_FRAME + string(5) + B_FRAME + string(128) + 1 + OPTION_PRESENT_FRAME + SHA256;
pub(crate) const TREND_NOTE_UNCHANGED_RECORD_MAX_V1: usize =
    B_FRAME + string(9) + B_FRAME + string(128) + 2 * (OPTION_PRESENT_FRAME + SHA256);
pub(crate) const TREND_ADVISORY_POPULATION_MAX_V1: usize = MAX_EXTERNAL_RECORDS_V1
    * (TREND_ADVISORY_REMOVED_RECORD_MAX_V1 + TREND_ADVISORY_ADDED_RECORD_MAX_V1);
pub(crate) const TREND_NOTE_POPULATION_MAX_V1: usize =
    MAX_EXTERNAL_RECORDS_V1 * (TREND_NOTE_REMOVED_RECORD_MAX_V1 + TREND_NOTE_ADDED_RECORD_MAX_V1);
pub(crate) const TREND_EXTERNAL_POPULATIONS_MAX_V1: usize =
    TREND_ADVISORY_POPULATION_MAX_V1 + TREND_NOTE_POPULATION_MAX_V1;

// Every logical-case or execution-variant contribution consumes at least 306
// disjoint bytes in aggregate-detail V1. A trend union cannot contain more
// records than the combined old and new populations.
pub(crate) const AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1: usize = 306;
pub(crate) const TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1: usize =
    super::report::AGGREGATE_DETAIL_MAX_BYTES_V1 / AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1;
pub(crate) const TREND_CASE_VARIANT_RECORDS_TWO_INPUTS_MAX_V1: usize =
    2 * TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1;
pub(crate) const TREND_COMMON_RECORD_MAX_V1: usize = B_FRAME
    + string(9)
    + B_FRAME
    + BASELINE_VARIANT_KEY_MAX_V1
    + 2 * (OPTION_PRESENT_FRAME + SHA256);
pub(crate) const TREND_CASE_VARIANT_POPULATIONS_MAX_V1: usize =
    TREND_CASE_VARIANT_RECORDS_TWO_INPUTS_MAX_V1 * TREND_COMMON_RECORD_MAX_V1;

pub(crate) const TREND_FIXED_MAX_V1: usize = 30
    + 1
    + 2
    + 7 * SECTION_FRAME
    + TREND_VERSION_SECTION_MAX_V1
    + TREND_CONTEXT_MAX_V1
    + TREND_EVALUATION_SUMMARY_MAX_V1
    + 4 * TREND_POPULATION_HEADER_MAX_V1;
pub(crate) const TREND_CONSERVATIVE_SYNTACTIC_MAX_V1: usize =
    TREND_CASE_VARIANT_POPULATIONS_MAX_V1 + TREND_EXTERNAL_POPULATIONS_MAX_V1 + TREND_FIXED_MAX_V1;
pub(crate) const TREND_FROZEN_OUTPUT_CEILING_V1: usize = 74_281_149;
pub(crate) const TREND_FROZEN_HEADROOM_V1: usize =
    TREND_FROZEN_OUTPUT_CEILING_V1 - TREND_CONSERVATIVE_SYNTACTIC_MAX_V1;

const _: () = assert!(MAX_ADVISORY_RESULT_BYTES_V1 == 16_409);
const _: () = assert!(MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1 == 16_418);
const _: () = assert!(BASELINE_VERSION_SECTION_MAX_V1 == 679);
const _: () = assert!(BASELINE_CAPTURE_RECORD_MAX_V1 == 33_968);
const _: () = assert!(BASELINE_TRACK_RECORD_MAX_V1 == 1_181);
const _: () = assert!(BASELINE_DOM_VARIANT_KEY_MAX_V1 == 169);
const _: () = assert!(BASELINE_VARIANT_KEY_MAX_V1 == 222);
const _: () = assert!(BASELINE_POINT_RECORD_MAX_V1 == 379);
const _: () = assert!(BASELINE_OPERATION_DESCRIPTOR_MAX_V1 == 302);
const _: () = assert!(BASELINE_SLOT_RECORD_MAX_V1 == 16_426);
const _: () = assert!(BASELINE_NOTE_RECORD_MAX_V1 == 1_420);
const _: () = assert!(BASELINE_FIXED_MAX_V1 == 785);
const _: () = assert!(BASELINE_ADVISORY_MAX_V1 == 13_664_066);
const _: () = assert!(BASELINE_DERIVED_MAX_V1 == 47_219_283);
const _: () = assert!(TREND_VERSION_SECTION_MAX_V1 == 720);
const _: () = assert!(TREND_CONTEXT_MAX_V1 == 268);
const _: () = assert!(TREND_EVALUATION_SUMMARY_MAX_V1 == 668);
const _: () = assert!(TREND_POPULATION_HEADER_MAX_V1 == 52);
const _: () = assert!(TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1 == 339);
const _: () = assert!(B_FRAME + TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1 == 347);
const _: () = assert!(TREND_ADVISORY_REMOVED_RECORD_MAX_V1 == 424);
const _: () = assert!(TREND_ADVISORY_ADDED_RECORD_MAX_V1 == 422);
const _: () = assert!(TREND_ADVISORY_UNCHANGED_RECORD_MAX_V1 == 475);
const _: () = assert!(
    TREND_ADVISORY_REMOVED_RECORD_MAX_V1 + TREND_ADVISORY_ADDED_RECORD_MAX_V1
        > TREND_ADVISORY_UNCHANGED_RECORD_MAX_V1
);
const _: () = assert!(TREND_NOTE_REMOVED_RECORD_MAX_V1 == 209);
const _: () = assert!(TREND_NOTE_ADDED_RECORD_MAX_V1 == 207);
const _: () = assert!(TREND_NOTE_UNCHANGED_RECORD_MAX_V1 == 251);
const _: () = assert!(
    TREND_NOTE_REMOVED_RECORD_MAX_V1 + TREND_NOTE_ADDED_RECORD_MAX_V1
        > TREND_NOTE_UNCHANGED_RECORD_MAX_V1
);
const _: () = assert!(TREND_ADVISORY_POPULATION_MAX_V1 == 216_576);
const _: () = assert!(TREND_NOTE_POPULATION_MAX_V1 == 106_496);
const _: () = assert!(TREND_EXTERNAL_POPULATIONS_MAX_V1 == 323_072);
const _: () = assert!(AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1 == 306);
const _: () = assert!(TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1 == 109_655);
const _: () = assert!(
    TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1 * AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1
        <= super::report::AGGREGATE_DETAIL_MAX_BYTES_V1
);
const _: () = assert!(
    (TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1 + 1) * AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1
        > super::report::AGGREGATE_DETAIL_MAX_BYTES_V1
);
const _: () = assert!(TREND_CASE_VARIANT_RECORDS_TWO_INPUTS_MAX_V1 == 219_310);
const _: () = assert!(TREND_COMMON_RECORD_MAX_V1 == 337);
const _: () = assert!(TREND_CASE_VARIANT_POPULATIONS_MAX_V1 == 73_907_470);
const _: () = assert!(TREND_FIXED_MAX_V1 == 1_967);
const _: () = assert!(TREND_CONSERVATIVE_SYNTACTIC_MAX_V1 == 74_232_509);
const _: () = assert!(TREND_FROZEN_OUTPUT_CEILING_V1 == 74_281_149);
const _: () = assert!(TREND_FROZEN_HEADROOM_V1 == 48_640);
const _: () = assert!(TREND_CONSERVATIVE_SYNTACTIC_MAX_V1 <= TREND_FROZEN_OUTPUT_CEILING_V1);
const _: () = assert!(TREND_FROZEN_OUTPUT_CEILING_V1 == super::trend::TREND_MAX_BYTES_V1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_and_trend_ceiling_terms_match_the_frozen_derivations() {
        assert_eq!(BASELINE_FIXED_MAX_V1, 785);
        assert_eq!(BASELINE_ADVISORY_MAX_V1, 13_664_066);
        assert_eq!(
            BASELINE_DERIVED_MAX_V1,
            super::super::baseline::BASELINE_MAX_BYTES_V1
        );
        assert_eq!(AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1, 306);
        assert_eq!(TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1, 109_655);
        assert_eq!(TREND_CASE_VARIANT_RECORDS_TWO_INPUTS_MAX_V1, 219_310);
        assert_eq!(TREND_COMMON_RECORD_MAX_V1, 337);
        assert_eq!(TREND_CASE_VARIANT_POPULATIONS_MAX_V1, 73_907_470);
        assert_eq!(TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1, 339);
        assert_eq!(B_FRAME + TREND_ADVISORY_POINT_KEY_PAYLOAD_MAX_V1, 347);
        assert_eq!(TREND_ADVISORY_REMOVED_RECORD_MAX_V1, 424);
        assert_eq!(TREND_ADVISORY_ADDED_RECORD_MAX_V1, 422);
        assert_eq!(TREND_ADVISORY_UNCHANGED_RECORD_MAX_V1, 475);
        assert_eq!(TREND_NOTE_REMOVED_RECORD_MAX_V1, 209);
        assert_eq!(TREND_NOTE_ADDED_RECORD_MAX_V1, 207);
        assert_eq!(TREND_ADVISORY_POPULATION_MAX_V1, 216_576);
        assert_eq!(TREND_NOTE_POPULATION_MAX_V1, 106_496);
        assert_eq!(TREND_EXTERNAL_POPULATIONS_MAX_V1, 323_072);
        assert_eq!(TREND_FIXED_MAX_V1, 1_967);
        assert_eq!(TREND_CONSERVATIVE_SYNTACTIC_MAX_V1, 74_232_509);
        const {
            assert!(TREND_CONSERVATIVE_SYNTACTIC_MAX_V1 <= super::super::trend::TREND_MAX_BYTES_V1);
        }
        assert_eq!(TREND_FROZEN_HEADROOM_V1, 48_640);
    }

    #[test]
    fn disjoint_external_membership_is_the_maximum_union_shape() {
        const {
            assert!(
                TREND_ADVISORY_REMOVED_RECORD_MAX_V1 + TREND_ADVISORY_ADDED_RECORD_MAX_V1
                    > TREND_ADVISORY_UNCHANGED_RECORD_MAX_V1
            );
        }
        const {
            assert!(
                TREND_NOTE_REMOVED_RECORD_MAX_V1 + TREND_NOTE_ADDED_RECORD_MAX_V1
                    > TREND_NOTE_UNCHANGED_RECORD_MAX_V1
            );
        }
    }

    #[test]
    fn detail_minimum_proves_the_case_variant_population_count() {
        let detail_max = super::super::report::AGGREGATE_DETAIL_MAX_BYTES_V1;
        assert_eq!(
            detail_max / AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1,
            TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1
        );
        assert!(
            TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1
                * AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1
                <= detail_max
        );
        assert!(
            (TREND_CASE_VARIANT_COUNT_PER_INPUT_MAX_V1 + 1)
                * AGGREGATE_DETAIL_MIN_VARIANT_OR_CASE_BYTES_V1
                > detail_max
        );
    }
}
