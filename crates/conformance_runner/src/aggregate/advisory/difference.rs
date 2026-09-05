use super::model::AdvisoryComparisonFailure as Failure;
use crate::report_writer::{CanonicalReportWriter, CanonicalReportWriterFailure};

pub const MAX_ADVISORY_EXCERPT_BYTES_V1: usize = 1_024;
pub const MAX_ADVISORY_DIFFERENCE_BYTES_V1: usize = 16 * 1_024;
pub const MAX_ADVISORY_DIFFERENCE_POOL_BYTES_V1: usize = 4 * 1_024 * 1_024;

#[derive(Debug, PartialEq, Eq)]
pub enum AdvisoryDifferenceLine {
    Missing,
    Present {
        original_bytes: usize,
        excerpt: String,
        excerpt_omitted: bool,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct AdvisoryFirstDifference {
    first_byte: usize,
    one_based_line: usize,
    borrowser_bytes: usize,
    external_bytes: usize,
    borrowser_line: AdvisoryDifferenceLine,
    external_line: AdvisoryDifferenceLine,
    serialized: Vec<u8>,
}
impl AdvisoryFirstDifference {
    pub const fn first_differing_byte(&self) -> usize {
        self.first_byte
    }
    pub const fn one_based_line(&self) -> usize {
        self.one_based_line
    }
    pub const fn borrowser_byte_length(&self) -> usize {
        self.borrowser_bytes
    }
    pub const fn external_byte_length(&self) -> usize {
        self.external_bytes
    }
    pub fn borrowser_line(&self) -> &AdvisoryDifferenceLine {
        &self.borrowser_line
    }
    pub fn external_line(&self) -> &AdvisoryDifferenceLine {
        &self.external_line
    }
    pub fn serialized_bytes(&self) -> &[u8] {
        &self.serialized
    }
    /// All retained raw evidence payload, including decoded excerpts as well as
    /// the serialized evidence. Do not undercount the two representations.
    pub fn retained_bytes(&self) -> Result<usize, Failure> {
        let line_bytes = |line: &AdvisoryDifferenceLine| match line {
            AdvisoryDifferenceLine::Missing => 0,
            AdvisoryDifferenceLine::Present { excerpt, .. } => excerpt.len(),
        };
        self.serialized
            .len()
            .checked_add(line_bytes(&self.borrowser_line))
            .and_then(|n| n.checked_add(line_bytes(&self.external_line)))
            .ok_or(Failure::Resource)
    }
}
impl CanonicalReportWriterFailure for Failure {
    fn report_too_large(_: usize) -> Self {
        Self::Resource
    }
    fn allocation_failure() -> Self {
        Self::Allocation
    }
}

pub(super) fn first_difference(
    left: &[u8],
    right: &[u8],
) -> Result<AdvisoryFirstDifference, Failure> {
    first_difference_with(left, right, &mut || Ok(()))
}
fn first_difference_with(
    left: &[u8],
    right: &[u8],
    reserve: &mut impl FnMut() -> Result<(), Failure>,
) -> Result<AdvisoryFirstDifference, Failure> {
    let offset = left
        .iter()
        .zip(right)
        .position(|(l, r)| l != r)
        .unwrap_or(left.len().min(right.len()));
    if left == right {
        return Err(Failure::Invariant);
    }
    let line = left[..offset]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        .checked_add(1)
        .ok_or(Failure::Resource)?;
    let start = left[..offset]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |index| index + 1);
    let l = excerpt(left, start, reserve)?;
    let r = excerpt(right, start, reserve)?;
    reserve()?;
    let mut writer = CanonicalReportWriter::<Failure>::new(MAX_ADVISORY_DIFFERENCE_BYTES_V1)?;
    writer.line("format", "borrowser-advisory-dom-first-difference-v1")?;
    writer.number("first-differing-byte", offset)?;
    writer.number("one-based-line", line)?;
    writer.number("borrowser-byte-length", left.len())?;
    writer.number("external-byte-length", right.len())?;
    write_line(&mut writer, "borrowser", &l)?;
    write_line(&mut writer, "external", &r)?;
    Ok(AdvisoryFirstDifference {
        first_byte: offset,
        one_based_line: line,
        borrowser_bytes: left.len(),
        external_bytes: right.len(),
        borrowser_line: l,
        external_line: r,
        serialized: writer.finish(),
    })
}
fn excerpt(
    bytes: &[u8],
    start: usize,
    reserve: &mut impl FnMut() -> Result<(), Failure>,
) -> Result<AdvisoryDifferenceLine, Failure> {
    if start >= bytes.len() {
        return Ok(AdvisoryDifferenceLine::Missing);
    }
    let remaining = &bytes[start..];
    let length = remaining
        .iter()
        .position(|b| *b == b'\n')
        .unwrap_or(remaining.len());
    let line = std::str::from_utf8(&remaining[..length]).map_err(|_| Failure::InvalidArtifact)?;
    let mut end = length.min(MAX_ADVISORY_EXCERPT_BYTES_V1);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    reserve()?;
    let mut excerpt = String::new();
    excerpt.try_reserve(end).map_err(|_| Failure::Allocation)?;
    excerpt.push_str(&line[..end]);
    Ok(AdvisoryDifferenceLine::Present {
        original_bytes: length,
        excerpt,
        excerpt_omitted: end < length,
    })
}
fn write_line(
    w: &mut CanonicalReportWriter<Failure>,
    side: &str,
    line: &AdvisoryDifferenceLine,
) -> Result<(), Failure> {
    w.line("side", side)?;
    match line {
        AdvisoryDifferenceLine::Missing => w.line("line-state", "missing"),
        AdvisoryDifferenceLine::Present {
            original_bytes,
            excerpt,
            excerpt_omitted,
        } => {
            w.line("line-state", "present")?;
            w.number("original-line-bytes", *original_bytes)?;
            w.line("excerpt", excerpt)?;
            w.raw(if *excerpt_omitted {
                b"excerpt-omitted = true\n".as_slice()
            } else {
                b"excerpt-omitted = false\n".as_slice()
            })
        }
    }
}
#[derive(Default)]
pub(super) struct DifferenceBudget {
    pub bytes: usize,
}
impl DifferenceBudget {
    pub fn retain(&mut self, bytes: usize) -> Result<(), Failure> {
        if bytes > MAX_ADVISORY_DIFFERENCE_BYTES_V1 {
            return Err(Failure::Resource);
        }
        let next = self.bytes.checked_add(bytes).ok_or(Failure::Resource)?;
        if next > MAX_ADVISORY_DIFFERENCE_POOL_BYTES_V1 {
            return Err(Failure::Resource);
        }
        self.bytes = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_v1_framing_has_no_line_ending_only_difference() {
        use external_test_provenance::validate_web_observable_dom_tree_v1 as validate;
        let bytes = include_bytes!(
            "../../../../../tests/contract-vectors/web-observable-dom-tree-v1/static-document.txt"
        );
        validate(bytes).unwrap();
        let text = std::str::from_utf8(bytes).unwrap();
        assert!(validate(text.replace('\n', "\r\n").as_bytes()).is_err());
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        assert!(validate(format!("{text}\n").as_bytes()).is_err());
        // DOM newlines are escaped field data, not alternate physical framing.
        assert!(text.contains("\\n"));
    }

    #[test]
    fn deterministic_utf8_evidence_and_atomic_budgets() {
        let l = format!("same\n{}x\n", "é".repeat(600));
        let r = format!("same\n{}y\n", "é".repeat(600));
        let evidence = first_difference(l.as_bytes(), r.as_bytes()).unwrap();
        assert_eq!(evidence.first_differing_byte(), 1205);
        assert_eq!(evidence.one_based_line(), 2);
        assert_eq!(
            evidence,
            first_difference(l.as_bytes(), r.as_bytes()).unwrap()
        );
        assert!(
            matches!(evidence.borrowser_line(), AdvisoryDifferenceLine::Present { original_bytes: 1201, excerpt, excerpt_omitted: true } if excerpt.len() == 1024)
        );
        assert!(matches!(
            first_difference(b"a\n", b"a\nb\n")
                .unwrap()
                .borrowser_line(),
            AdvisoryDifferenceLine::Missing
        ));
        assert_eq!(
            first_difference_with(b"a", b"b", &mut || Err(Failure::Allocation)),
            Err(Failure::Allocation)
        );
        assert_eq!(first_difference(b"same", b"same"), Err(Failure::Invariant));
        let mut budget = DifferenceBudget::default();
        for _ in 0..256 {
            budget.retain(16 * 1024).unwrap();
        }
        assert_eq!(budget.bytes, 4 * 1024 * 1024);
        assert_eq!(budget.retain(1), Err(Failure::Resource));
        assert_eq!(budget.bytes, 4 * 1024 * 1024);
        assert_eq!(
            DifferenceBudget::default().retain(16 * 1024 + 1),
            Err(Failure::Resource)
        );
        let mut overflow = DifferenceBudget { bytes: usize::MAX };
        assert_eq!(overflow.retain(1), Err(Failure::Resource));
    }
}
