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

pub(crate) fn validate_first_difference_v1(bytes: &[u8]) -> Result<(), ()> {
    if bytes.len() > MAX_ADVISORY_DIFFERENCE_BYTES_V1 {
        return Err(());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if !text.ends_with('\n') || text.contains('\r') {
        return Err(());
    }
    let mut lines = text.lines();
    if lines.next() != Some("format = \"borrowser-advisory-dom-first-difference-v1\"") {
        return Err(());
    }
    let number = |line: &str, key: &str| -> Result<usize, ()> {
        let v = line
            .strip_prefix(key)
            .and_then(|v| v.strip_prefix(" = "))
            .ok_or(())?;
        if v.is_empty()
            || (v.len() > 1 && v.starts_with('0'))
            || !v.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(());
        }
        v.parse().map_err(|_| ())
    };
    let first = number(lines.next().ok_or(())?, "first-differing-byte")?;
    let one = number(lines.next().ok_or(())?, "one-based-line")?;
    let left = number(lines.next().ok_or(())?, "borrowser-byte-length")?;
    let right = number(lines.next().ok_or(())?, "external-byte-length")?;
    if one == 0 || first >= left.max(right) {
        return Err(());
    }
    let mut parsed = Vec::new();
    parsed.try_reserve_exact(2).map_err(|_| ())?;
    for expected_side in ["side = \"borrowser\"", "side = \"external\""] {
        if lines.next() != Some(expected_side) {
            return Err(());
        }
        let line = match lines.next().ok_or(())? {
            "line-state = \"missing\"" => AdvisoryDifferenceLine::Missing,
            "line-state = \"present\"" => {
                let original_bytes = number(lines.next().ok_or(())?, "original-line-bytes")?;
                let excerpt = parse_canonical_quoted(lines.next().ok_or(())?, "excerpt")?;
                let excerpt_omitted = match lines.next() {
                    Some("excerpt-omitted = true") => true,
                    Some("excerpt-omitted = false") => false,
                    _ => return Err(()),
                };
                if excerpt.len() > MAX_ADVISORY_EXCERPT_BYTES_V1
                    || original_bytes < excerpt.len()
                    || excerpt_omitted != (original_bytes > excerpt.len())
                {
                    return Err(());
                }
                AdvisoryDifferenceLine::Present {
                    original_bytes,
                    excerpt,
                    excerpt_omitted,
                }
            }
            _ => return Err(()),
        };
        parsed.push(line);
    }
    if lines.next().is_some()
        || matches!(parsed[0], AdvisoryDifferenceLine::Missing) && first < left
        || matches!(parsed[1], AdvisoryDifferenceLine::Missing) && first < right
    {
        return Err(());
    }
    let mut writer =
        CanonicalReportWriter::<Failure>::new(MAX_ADVISORY_DIFFERENCE_BYTES_V1).map_err(|_| ())?;
    writer
        .line("format", "borrowser-advisory-dom-first-difference-v1")
        .map_err(|_| ())?;
    writer
        .number("first-differing-byte", first)
        .map_err(|_| ())?;
    writer.number("one-based-line", one).map_err(|_| ())?;
    writer
        .number("borrowser-byte-length", left)
        .map_err(|_| ())?;
    writer
        .number("external-byte-length", right)
        .map_err(|_| ())?;
    write_line(&mut writer, "borrowser", &parsed[0]).map_err(|_| ())?;
    write_line(&mut writer, "external", &parsed[1]).map_err(|_| ())?;
    (writer.finish() == bytes).then_some(()).ok_or(())
}

fn parse_canonical_quoted(line: &str, key: &str) -> Result<String, ()> {
    let encoded = line
        .strip_prefix(key)
        .and_then(|value| value.strip_prefix(" = \""))
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(())?;
    let mut output = String::new();
    output.try_reserve(encoded.len()).map_err(|_| ())?;
    let mut chars = encoded.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            if character < ' ' || character == '"' {
                return Err(());
            }
            output.push(character);
            continue;
        }
        match chars.next().ok_or(())? {
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            'u' => {
                if chars.next() != Some('{') {
                    return Err(());
                }
                let mut value = 0_u32;
                let mut digits = 0;
                let mut leading_zero = false;
                loop {
                    let digit = chars.next().ok_or(())?;
                    if digit == '}' {
                        break;
                    }
                    if !digit.is_ascii_hexdigit() || digit.is_ascii_lowercase() {
                        return Err(());
                    }
                    if digits == 0 {
                        leading_zero = digit == '0';
                    }
                    value = value
                        .checked_mul(16)
                        .and_then(|value| value.checked_add(digit.to_digit(16)?))
                        .ok_or(())?;
                    digits += 1;
                }
                let decoded = char::from_u32(value).ok_or(())?;
                if digits == 0
                    || digits > 1 && leading_zero
                    || decoded >= ' '
                    || matches!(decoded, '\n' | '\r' | '\t')
                {
                    return Err(());
                }
                output.push(decoded);
            }
            _ => return Err(()),
        }
    }
    Ok(output)
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
        validate_first_difference_v1(evidence.serialized_bytes()).unwrap();
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
        let noncanonical = evidence
            .serialized_bytes()
            .split(|byte| *byte == b'\n')
            .collect::<Vec<_>>()
            .join(&b'\r');
        assert!(validate_first_difference_v1(&noncanonical).is_err());
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
