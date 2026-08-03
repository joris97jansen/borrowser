use super::model::{DeliveryName, ExpectationSurface, SnapshotPath, ValidatedFixtureInvariantCode};
use super::validate::ValidatedFixtureSpec;
use crate::parser_snapshot::{CanonicalSnapshot, ParsedSnapshot};
use std::fmt::Write;

pub(super) fn compare_snapshots(
    fixture: &ValidatedFixtureSpec,
    delivery: Option<&DeliveryName>,
    expected_path: &SnapshotPath,
    expected: &ParsedSnapshot,
    actual: &CanonicalSnapshot,
) -> Result<Option<String>, ValidatedFixtureInvariantCode> {
    if expected.surface() != actual.surface() || expected.format() != actual.format() {
        return Err(ValidatedFixtureInvariantCode::ComparisonSurfaceContradiction);
    }
    let expected_records = expected.snapshot();
    let actual_records = actual.snapshot();
    let shared = expected_records
        .record_count()
        .min(actual_records.record_count());
    let first = (0..shared)
        .find(|index| {
            expected_records.record(*index).map(|record| record.line)
                != actual_records.record(*index).map(|record| record.line)
        })
        .or_else(|| {
            (expected_records.record_count() != actual_records.record_count()).then_some(shared)
        });
    let Some(first) = first else {
        return Ok(None);
    };
    let missing = "<missing>";
    let expected_record = expected_records.record(first);
    let actual_record = actual_records.record(first);
    let location = expected_record
        .map(|record| record.location)
        .or_else(|| actual_record.map(|record| record.location))
        .unwrap_or("end of snapshot");
    let mut message = String::new();
    let _ = writeln!(&mut message, "fixture: {}", fixture.id().as_str());
    let _ = writeln!(
        &mut message,
        "fixture path: {}",
        fixture.repository_relative_path()
    );
    let _ = writeln!(
        &mut message,
        "expectation surface: {}",
        expected.surface().name()
    );
    if let Some(delivery) = delivery {
        let _ = writeln!(&mut message, "transition delivery: {}", delivery.as_str());
    }
    let _ = writeln!(
        &mut message,
        "expected snapshot: {}/{}",
        fixture.repository_relative_path(),
        expected_path.as_str()
    );
    let _ = writeln!(
        &mut message,
        "snapshot format: {}",
        expected.format().name()
    );
    let _ = writeln!(
        &mut message,
        "first meaningful difference: record {} ({location})",
        first + 1
    );
    let _ = writeln!(
        &mut message,
        "expected: {}",
        expected_record.map(|record| record.line).unwrap_or(missing)
    );
    let _ = writeln!(
        &mut message,
        "actual: {}",
        actual_record.map(|record| record.line).unwrap_or(missing)
    );
    let start = first.saturating_sub(2);
    let end = (first + 3).min(
        expected_records
            .record_count()
            .max(actual_records.record_count()),
    );
    let _ = writeln!(&mut message, "nearby context:");
    for index in start..end {
        let marker = if index == first { ">" } else { " " };
        let left = expected_records
            .record(index)
            .map(|record| record.line)
            .unwrap_or(missing);
        let right = actual_records
            .record(index)
            .map(|record| record.line)
            .unwrap_or(missing);
        let _ = writeln!(&mut message, "{marker} {} expected: {left}", index + 1);
        let _ = writeln!(&mut message, "{marker} {} actual:   {right}", index + 1);
    }
    let _ = writeln!(
        &mut message,
        "expected record count: {}",
        expected_records.record_count()
    );
    let _ = writeln!(
        &mut message,
        "actual record count: {}",
        actual_records.record_count()
    );
    Ok(Some(message))
}

pub(super) const fn comparison_order() -> [ExpectationSurface; 8] {
    [
        ExpectationSurface::Tokens,
        ExpectationSurface::ParseErrors,
        ExpectationSurface::ImplementationDiagnostics,
        ExpectationSurface::DocumentMode,
        ExpectationSurface::Tree,
        ExpectationSurface::Patches,
        ExpectationSurface::Transitions,
        ExpectationSurface::UnsupportedFeatures,
    ]
}
