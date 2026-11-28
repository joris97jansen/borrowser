//! One-time grammar-migration audit against the immutable approved-base expectations.
//! Production hashing always consumes full v2 snapshots; the private RAII mode
//! reconstructs only the known old grammar for these six fixed regression inputs.
use crate::fuzz_regressions::{
    CssFuzzRegressionProfile, CssFuzzRegressionTool,
    render_css_fuzz_regression_summary_with_profile,
};
const OLD: [(&str, &str); 6] = [
    (
        "rg-css-cascade-selector-limit-trigger-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_cascade
profile: selector-limit-zero
seed: 0
termination: selector-matching-limit-exceeded
input-bytes: 31
decoded-bytes: 31
stylesheet-cases-observed: 2
resolved-elements-observed: 0
computed-elements-observed: 0
digest: 16659878273259462035
"###,
    ),
    (
        "rg-css-parser-malformed-recovery-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_parser
profile: default
seed: 0
termination: completed
input-bytes: 56
decoded-bytes: 56
rules-observed: 1
declarations-observed: 1
component-values-observed: 4
diagnostics-observed: 2
hit-limit: false
digest: 4399099515594142102
"###,
    ),
    (
        "rg-css-selector-matching-sibling-axis-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_selector_matching
profile: default
seed: 0
termination: completed
input-bytes: 21
decoded-bytes: 21
selector-cases-observed: 2
elements-observed: 36
parsed-cases: 2
unsupported-cases: 0
invalid-cases: 0
matched-targets-observed: 1
unmatched-targets-observed: 71
unsupported-targets-observed: 0
invalid-targets-observed: 0
limit-errors-observed: 0
digest: 5249538639164088338
"###,
    ),
    (
        "rg-css-selector-parser-attribute-combinator-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_selector_parser
profile: default
seed: 0
termination: completed
input-bytes: 62
decoded-bytes: 62
selector-cases-observed: 2
parsed-cases: 2
unsupported-cases: 0
invalid-cases: 0
resource-limit-invalid-cases: 0
digest: 2309176676957599015
"###,
    ),
    (
        "rg-css-tokenizer-malformed-comment-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_tokenizer
profile: default
seed: 0
termination: completed
input-bytes: 24
decoded-bytes: 24
tokens-observed: 2
diagnostics-observed: 1
hit-limit: false
digest: 16698978721194329519
"###,
    ),
    (
        "rg-css-values-malformed-function-2026-04-26",
        r###"version: 1
css-fuzz-regression
tool: css_values
profile: default
seed: 0
termination: completed
input-bytes: 16
decoded-bytes: 16
properties-observed: 35
value-cases-observed: 105
missing-declaration-value-cases: 0
specified-ok-cases: 57
specified-error-cases: 48
computed-ok-cases: 57
computed-error-cases: 0
digest: 17483231146213737592
"###,
    ),
];
#[test]
fn aj8_snapshot_grammar_migration_preserves_six_promoted_fuzz_behaviors() {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            super::AJ8_LEGACY_GRAMMAR_AUDIT.with(|flag| flag.set(false));
        }
    }
    assert!(!super::AJ8_LEGACY_GRAMMAR_AUDIT.with(|flag| flag.replace(true)));
    let restore = Restore;
    for (directory, expected) in OLD {
        let field = |name: &str| {
            expected
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
        };
        let tool = CssFuzzRegressionTool::parse(field("tool: ")).unwrap();
        let profile = CssFuzzRegressionProfile::parse(field("profile: ")).unwrap();
        let seed = field("seed: ").parse().unwrap();
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/regressions/css_fuzz")
            .join(directory)
            .join("input.bin");
        let bytes = std::fs::read(input).unwrap();
        let actual =
            render_css_fuzz_regression_summary_with_profile(tool, profile, &bytes, seed).unwrap();
        assert_eq!(actual, expected, "legacy behavior drift in {directory}");
    }
    drop(restore);
    println!("AJ8 legacy grammar audit rows=6");
}
