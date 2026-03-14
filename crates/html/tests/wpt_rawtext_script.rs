#![cfg(feature = "html5")]

mod wpt_manifest;
#[path = "support/wpt_tokenizer_suite.rs"]
mod wpt_tokenizer_suite;

#[test]
fn wpt_rawtext_script_slice() {
    wpt_tokenizer_suite::run(wpt_tokenizer_suite::TokenizerSuiteSpec::rawtext_script());
}
