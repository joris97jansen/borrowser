use super::fixtures::Fixture;
use html_test_support::diff_lines;

pub(crate) fn assert_expected_lines(
    fixture: &Fixture,
    expected: &[String],
    actual: &[String],
    label: &str,
) {
    if actual != expected {
        let source = fixture.meta.source.as_deref().unwrap_or("<none>");
        panic!(
            "snapshot mismatch in rawtext/script regression '{}' [{label}]\npath: {}\ntool: {}\nseed: {}\ndate: {}\nissue: {}\nsource: {}\nguard: {}\n{}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.tool,
            fixture.meta.seed,
            fixture.meta.date,
            fixture.meta.issue,
            source,
            fixture.meta.guard,
            diff_lines(expected, actual)
        );
    }
}
