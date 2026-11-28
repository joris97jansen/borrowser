//! Checked stylesheet runtime facts. Selection/decoding algorithms live in runtime_css.
use std::fmt;

#[derive(Clone, Copy)]
pub struct CssEncoding {
    canonical: &'static encoding_rs::Encoding,
}
impl CssEncoding {
    pub const UTF8: Self = Self {
        canonical: encoding_rs::UTF_8,
    };
    pub fn from_encoding(canonical: &'static encoding_rs::Encoding) -> Self {
        Self { canonical }
    }
    pub fn canonical_name(self) -> &'static str {
        self.canonical.name()
    }
    pub fn is_utf8(self) -> bool {
        self == Self::UTF8
    }
    pub fn is_utf16(self) -> bool {
        std::ptr::eq(self.canonical, encoding_rs::UTF_16BE)
            || std::ptr::eq(self.canonical, encoding_rs::UTF_16LE)
    }
}
impl PartialEq for CssEncoding {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.canonical, other.canonical)
    }
}
impl Eq for CssEncoding {}
impl fmt::Debug for CssEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.is_utf8() {
            "Utf8"
        } else {
            "UnsupportedEncoding"
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssEncodingLabel {
    NotConsulted,
    Absent,
    Unknown,
    Recognized(CssEncoding),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssCharsetPrefix {
    NotConsulted,
    NoExactPrefix,
    Malformed,
    Exact(CssExactCharsetPrefix),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssExactCharsetPrefix {
    end_exclusive: u16,
    label: CssEncodingLabel,
}
impl CssExactCharsetPrefix {
    pub fn try_new(end: usize, label: CssEncodingLabel) -> Result<Self, CssDecodeEvidenceError> {
        if !(12..=1024).contains(&end)
            || !matches!(
                label,
                CssEncodingLabel::Unknown | CssEncodingLabel::Recognized(_)
            )
        {
            return Err(CssDecodeEvidenceError::Decision);
        }
        Ok(Self {
            end_exclusive: end as u16,
            label,
        })
    }
    pub fn end_exclusive(self) -> usize {
        usize::from(self.end_exclusive)
    }
    pub fn label(self) -> CssEncodingLabel {
        self.label
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssBom {
    None,
    Utf8,
    Utf16Be,
    Utf16Le,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssEnvironmentSource {
    LinkCharset,
    Document,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssEnvironmentEncoding {
    source: CssEnvironmentSource,
    encoding: CssEncoding,
}
impl CssEnvironmentEncoding {
    pub fn new(source: CssEnvironmentSource, encoding: CssEncoding) -> Self {
        Self { source, encoding }
    }
    pub fn source(self) -> CssEnvironmentSource {
        self.source
    }
    pub fn encoding(self) -> CssEncoding {
        self.encoding
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssEncodingSource {
    Bom,
    Transport,
    CharsetDeclaration,
    Environment,
    Default,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssDecodingDecision {
    delivered_bytes: usize,
    bom: CssBom,
    transport: CssEncodingLabel,
    charset_prefix: CssCharsetPrefix,
    environment: Option<CssEnvironmentEncoding>,
    selected_source: CssEncodingSource,
    selected_encoding: CssEncoding,
    effective_encoding: CssEncoding,
}
impl CssDecodingDecision {
    pub fn try_new(
        delivered_bytes: usize,
        bom: CssBom,
        transport: CssEncodingLabel,
        charset_prefix: CssCharsetPrefix,
        environment: Option<CssEnvironmentEncoding>,
        selected_source: CssEncodingSource,
        selected_encoding: CssEncoding,
    ) -> Result<Self, CssDecodeEvidenceError> {
        use CssEncodingLabel::Recognized;
        let bom_encoding = match bom {
            CssBom::None => None,
            CssBom::Utf8 if delivered_bytes >= 3 => Some(CssEncoding::UTF8),
            CssBom::Utf16Be if delivered_bytes >= 2 => {
                Some(CssEncoding::from_encoding(encoding_rs::UTF_16BE))
            }
            CssBom::Utf16Le if delivered_bytes >= 2 => {
                Some(CssEncoding::from_encoding(encoding_rs::UTF_16LE))
            }
            _ => return Err(CssDecodeEvidenceError::Bom),
        };
        let declaration_encoding = match charset_prefix {
            CssCharsetPrefix::Exact(p) => match p.label() {
                Recognized(e) => Some(e),
                _ => None,
            },
            _ => None,
        };
        let expected = if let Some(e) = bom_encoding {
            if transport != CssEncodingLabel::NotConsulted
                || charset_prefix != CssCharsetPrefix::NotConsulted
            {
                return Err(CssDecodeEvidenceError::Decision);
            }
            (CssEncodingSource::Bom, e)
        } else {
            if transport == CssEncodingLabel::NotConsulted {
                return Err(CssDecodeEvidenceError::Decision);
            }
            if let Recognized(e) = transport {
                if charset_prefix != CssCharsetPrefix::NotConsulted {
                    return Err(CssDecodeEvidenceError::Decision);
                }
                (CssEncodingSource::Transport, e)
            } else {
                if charset_prefix == CssCharsetPrefix::NotConsulted {
                    return Err(CssDecodeEvidenceError::Decision);
                }
                if let Some(e) = declaration_encoding {
                    (CssEncodingSource::CharsetDeclaration, e)
                } else if let Some(env) = environment {
                    (CssEncodingSource::Environment, env.encoding())
                } else {
                    (CssEncodingSource::Default, CssEncoding::UTF8)
                }
            }
        };
        if expected != (selected_source, selected_encoding) {
            return Err(CssDecodeEvidenceError::Decision);
        }
        if let CssCharsetPrefix::Exact(p) = charset_prefix
            && p.end_exclusive() > delivered_bytes
        {
            return Err(CssDecodeEvidenceError::BodyLength);
        }
        let effective_encoding = if selected_source == CssEncodingSource::CharsetDeclaration
            && selected_encoding.is_utf16()
        {
            CssEncoding::UTF8
        } else {
            selected_encoding
        };
        Ok(Self {
            delivered_bytes,
            bom,
            transport,
            charset_prefix,
            environment,
            selected_source,
            selected_encoding,
            effective_encoding,
        })
    }
    pub fn delivered_bytes(self) -> usize {
        self.delivered_bytes
    }
    pub fn bom(self) -> CssBom {
        self.bom
    }
    pub fn transport(self) -> CssEncodingLabel {
        self.transport
    }
    pub fn charset_prefix(self) -> CssCharsetPrefix {
        self.charset_prefix
    }
    pub fn environment(self) -> Option<CssEnvironmentEncoding> {
        self.environment
    }
    pub fn selected_source(self) -> CssEncodingSource {
        self.selected_source
    }
    pub fn selected_encoding(self) -> CssEncoding {
        self.selected_encoding
    }
    pub fn effective_encoding(self) -> CssEncoding {
        self.effective_encoding
    }
    pub fn consumed_bom_bytes(self) -> usize {
        usize::from(self.bom == CssBom::Utf8) * 3
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDecodeEvidenceError {
    Decision,
    BodyLength,
    Bom,
    Payload,
    OutputLength,
    Replacements,
    Eof,
    Bounds,
    FailureShape,
    ArithmeticOverflow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDecodeFailure {
    OutputReservation,
    OutputBound,
    ArithmeticOverflow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssRuntimeFailure {
    SessionReservation,
    DuplicateChunk,
    CommandProvenance,
    InputBound,
    InputLength,
    DecisionBodyMismatch,
    InvalidEvidence(CssDecodeEvidenceError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssReplacementCounts {
    invalid_events: usize,
    invalid_affected_bytes: usize,
    incomplete_eof_events: u8,
    incomplete_eof_affected_bytes: u8,
}
impl CssReplacementCounts {
    pub fn try_new(i: usize, ai: usize, f: u8, af: u8) -> Result<Self, CssDecodeEvidenceError> {
        let max = i
            .checked_mul(3)
            .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?;
        if ai < i || ai > max || f > 1 || (f == 0 && af != 0) || (f == 1 && !(1..=3).contains(&af))
        {
            return Err(CssDecodeEvidenceError::Replacements);
        }
        Ok(Self {
            invalid_events: i,
            invalid_affected_bytes: ai,
            incomplete_eof_events: f,
            incomplete_eof_affected_bytes: af,
        })
    }
    pub fn invalid_events(self) -> usize {
        self.invalid_events
    }
    pub fn invalid_affected_bytes(self) -> usize {
        self.invalid_affected_bytes
    }
    pub fn incomplete_eof_events(self) -> u8 {
        self.incomplete_eof_events
    }
    pub fn incomplete_eof_affected_bytes(self) -> u8 {
        self.incomplete_eof_affected_bytes
    }
    pub fn events(self) -> Option<usize> {
        self.invalid_events
            .checked_add(usize::from(self.incomplete_eof_events))
    }
    pub fn affected_bytes(self) -> Option<usize> {
        self.invalid_affected_bytes
            .checked_add(usize::from(self.incomplete_eof_affected_bytes))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssFinalizedDecodeReport {
    delivered: usize,
    bom: u8,
    payload: usize,
    emitted: usize,
    replacements: CssReplacementCounts,
}
impl CssFinalizedDecodeReport {
    // Keep independently checked units explicit at the wire construction boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        decision: CssDecodingDecision,
        delivered: usize,
        bom: u8,
        payload: usize,
        emitted: usize,
        replacements: CssReplacementCounts,
        pending: u8,
        eof: bool,
    ) -> Result<Self, CssDecodeEvidenceError> {
        if !decision.effective_encoding().is_utf8() {
            return Err(CssDecodeEvidenceError::Decision);
        }
        if delivered != decision.delivered_bytes() {
            return Err(CssDecodeEvidenceError::BodyLength);
        }
        if usize::from(bom) != decision.consumed_bom_bytes() {
            return Err(CssDecodeEvidenceError::Bom);
        }
        if delivered.checked_sub(usize::from(bom)) != Some(payload) {
            return Err(CssDecodeEvidenceError::Payload);
        }
        if pending != 0 || !eof {
            return Err(CssDecodeEvidenceError::Eof);
        }
        let affected = replacements
            .affected_bytes()
            .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?;
        let events = replacements
            .events()
            .and_then(|n| n.checked_mul(3))
            .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?;
        let expected = payload
            .checked_sub(affected)
            .and_then(|n| n.checked_add(events))
            .ok_or(CssDecodeEvidenceError::Replacements)?;
        if expected != emitted {
            return Err(CssDecodeEvidenceError::OutputLength);
        }
        if emitted
            > payload
                .checked_mul(3)
                .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?
        {
            return Err(CssDecodeEvidenceError::Bounds);
        }
        Ok(Self {
            delivered,
            bom,
            payload,
            emitted,
            replacements,
        })
    }
    pub fn delivered_bytes_processed(self) -> usize {
        self.delivered
    }
    pub fn bom_bytes_consumed(self) -> u8 {
        self.bom
    }
    pub fn payload_bytes_processed(self) -> usize {
        self.payload
    }
    pub fn emitted_utf8_bytes(self) -> usize {
        self.emitted
    }
    pub fn replacements(self) -> CssReplacementCounts {
        self.replacements
    }
    pub fn eof_processed(self) -> bool {
        true
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssInterruptedDecodeReport {
    delivered: usize,
    bom: u8,
    payload: usize,
    emitted: usize,
    replacements: Option<CssReplacementCounts>,
    pending: Option<u8>,
    eof: bool,
}
impl CssInterruptedDecodeReport {
    // Interrupted evidence has different invariants from finalized evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        decision: CssDecodingDecision,
        delivered: usize,
        bom: u8,
        payload: usize,
        emitted: usize,
        replacements: Option<CssReplacementCounts>,
        pending: Option<u8>,
        eof: bool,
    ) -> Result<Self, CssDecodeEvidenceError> {
        if !decision.effective_encoding().is_utf8() || delivered > decision.delivered_bytes() {
            return Err(CssDecodeEvidenceError::BodyLength);
        }
        if usize::from(bom) != decision.consumed_bom_bytes()
            || delivered.checked_sub(usize::from(bom)) != Some(payload)
        {
            return Err(CssDecodeEvidenceError::Payload);
        }
        if (eof && delivered != decision.delivered_bytes())
            || pending.is_some_and(|n| n > 3 || usize::from(n) > payload || (eof && n != 0))
        {
            return Err(CssDecodeEvidenceError::Eof);
        }
        if let Some(r) = replacements {
            if (!eof && r.incomplete_eof_events() != 0)
                || r.affected_bytes().is_none_or(|n| n > payload)
            {
                return Err(CssDecodeEvidenceError::Replacements);
            }
            let remaining = payload
                .checked_sub(
                    r.affected_bytes()
                        .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?,
                )
                .ok_or(CssDecodeEvidenceError::Replacements)?;
            let remaining = remaining
                .checked_sub(usize::from(pending.unwrap_or(0)))
                .ok_or(CssDecodeEvidenceError::Eof)?;
            let maximum = r
                .events()
                .and_then(|n| n.checked_mul(3))
                .and_then(|n| n.checked_add(remaining))
                .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?;
            if emitted > maximum {
                return Err(CssDecodeEvidenceError::OutputLength);
            }
        }
        if emitted
            > payload
                .checked_mul(3)
                .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?
        {
            return Err(CssDecodeEvidenceError::Bounds);
        }
        Ok(Self {
            delivered,
            bom,
            payload,
            emitted,
            replacements,
            pending,
            eof,
        })
    }
    pub fn delivered_bytes_processed(self) -> usize {
        self.delivered
    }
    pub fn bom_bytes_consumed(self) -> u8 {
        self.bom
    }
    pub fn payload_bytes_processed(self) -> usize {
        self.payload
    }
    pub fn emitted_utf8_bytes(self) -> usize {
        self.emitted
    }
    pub fn replacements(self) -> Option<CssReplacementCounts> {
        self.replacements
    }
    pub fn pending_payload_bytes(self) -> Option<u8> {
        self.pending
    }
    pub fn eof_processed(self) -> bool {
        self.eof
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDecodeReport {
    NotStarted,
    Interrupted(CssInterruptedDecodeReport),
    Finalized(CssFinalizedDecodeReport),
}
pub struct CssDecodedStylesheet {
    decision: CssDecodingDecision,
    report: CssDecodeReport,
    text: Result<String, CssDecodeFailure>,
}
impl CssDecodedStylesheet {
    pub fn try_success(
        decision: CssDecodingDecision,
        report: CssFinalizedDecodeReport,
        text: String,
    ) -> Result<Self, CssDecodeEvidenceError> {
        let checked = CssFinalizedDecodeReport::try_new(
            decision,
            report.delivered,
            report.bom,
            report.payload,
            report.emitted,
            report.replacements,
            0,
            true,
        )?;
        if text.len() != checked.emitted {
            return Err(CssDecodeEvidenceError::OutputLength);
        }
        Ok(Self {
            decision,
            report: CssDecodeReport::Finalized(checked),
            text: Ok(text),
        })
    }
    pub fn try_failure(
        decision: CssDecodingDecision,
        failure: CssDecodeFailure,
        report: CssDecodeReport,
    ) -> Result<Self, CssDecodeEvidenceError> {
        if !decision.effective_encoding().is_utf8()
            || matches!(report, CssDecodeReport::Finalized(_))
            || (failure == CssDecodeFailure::OutputReservation
                && report != CssDecodeReport::NotStarted)
            || (failure == CssDecodeFailure::OutputBound
                && !matches!(report, CssDecodeReport::Interrupted(_)))
        {
            return Err(CssDecodeEvidenceError::FailureShape);
        }
        if let CssDecodeReport::Interrupted(r) = report {
            CssInterruptedDecodeReport::try_new(
                decision,
                r.delivered,
                r.bom,
                r.payload,
                r.emitted,
                r.replacements,
                r.pending,
                r.eof,
            )?;
        }
        Ok(Self {
            decision,
            report,
            text: Err(failure),
        })
    }
    pub fn decision(&self) -> CssDecodingDecision {
        self.decision
    }
    pub fn report(&self) -> CssDecodeReport {
        self.report
    }
    pub fn text(&self) -> Result<&str, CssDecodeFailure> {
        self.text.as_deref().map_err(|e| *e)
    }
    pub fn into_parts(
        self,
    ) -> (
        CssDecodingDecision,
        CssDecodeReport,
        Result<String, CssDecodeFailure>,
    ) {
        (self.decision, self.report, self.text)
    }
}
impl fmt::Debug for CssDecodedStylesheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CssDecodedStylesheet")
            .field("decision", &self.decision)
            .field("report", &self.report)
            .field("failure", &self.text.as_ref().err())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decision(n: usize) -> CssDecodingDecision {
        CssDecodingDecision::try_new(
            n,
            CssBom::None,
            CssEncodingLabel::Absent,
            CssCharsetPrefix::NoExactPrefix,
            None,
            CssEncodingSource::Default,
            CssEncoding::UTF8,
        )
        .unwrap()
    }
    #[test]
    fn aj8_t5_checked_finalized_evidence_rejects_each_false_relationship() {
        let d = decision(4);
        let r = CssReplacementCounts::try_new(1, 1, 1, 1).unwrap();
        let rows = [
            (3, 0, 4, 8, 0, true),
            (4, 3, 1, 8, 0, true),
            (4, 0, 3, 8, 0, true),
            (4, 0, 4, 7, 0, true),
            (4, 0, 4, 8, 1, true),
            (4, 0, 4, 8, 0, false),
        ];
        for (n, b, p, o, pending, eof) in rows {
            assert!(CssFinalizedDecodeReport::try_new(d, n, b, p, o, r, pending, eof).is_err());
        }
        let report = CssFinalizedDecodeReport::try_new(d, 4, 0, 4, 8, r, 0, true).unwrap();
        assert!(CssDecodedStylesheet::try_success(d, report, "1234567".into()).is_err());
        assert!(
            CssDecodedStylesheet::try_failure(
                d,
                CssDecodeFailure::OutputBound,
                CssDecodeReport::Finalized(report)
            )
            .is_err()
        );
        assert!(CssReplacementCounts::try_new(1, 4, 0, 0).is_err());
        assert!(CssReplacementCounts::try_new(0, 0, 1, 0).is_err());
        assert!(CssReplacementCounts::try_new(0, 0, 2, 2).is_err());
        let prefix =
            CssExactCharsetPrefix::try_new(12, CssEncodingLabel::Recognized(CssEncoding::UTF8))
                .unwrap();
        assert!(
            CssDecodingDecision::try_new(
                0,
                CssBom::None,
                CssEncodingLabel::Absent,
                CssCharsetPrefix::Exact(prefix),
                None,
                CssEncodingSource::CharsetDeclaration,
                CssEncoding::UTF8
            )
            .is_err()
        );
        assert!(CssInterruptedDecodeReport::try_new(d, 0, 0, 0, 1, None, Some(0), false).is_err());
        assert!(CssInterruptedDecodeReport::try_new(d, 0, 0, 0, 0, None, Some(3), false).is_err());
        assert!(CssInterruptedDecodeReport::try_new(d, 1, 0, 1, 0, None, Some(0), true).is_err());
        assert!(
            CssDecodedStylesheet::try_failure(
                d,
                CssDecodeFailure::OutputBound,
                CssDecodeReport::NotStarted
            )
            .is_err()
        );
        println!("parameter_rows=16");
    }
    #[test]
    fn aj8_t6_canonical_decisions_and_redacted_dtos() {
        assert!(
            CssDecodingDecision::try_new(
                0,
                CssBom::Utf8,
                CssEncodingLabel::Absent,
                CssCharsetPrefix::NoExactPrefix,
                None,
                CssEncodingSource::Default,
                CssEncoding::UTF8
            )
            .is_err()
        );
        assert!(CssExactCharsetPrefix::try_new(1025, CssEncodingLabel::Unknown).is_err());
        assert!(CssExactCharsetPrefix::try_new(12, CssEncodingLabel::Absent).is_err());
        let d = decision(12);
        let r = CssReplacementCounts::try_new(0, 0, 0, 0).unwrap();
        let report = CssFinalizedDecodeReport::try_new(d, 12, 0, 12, 12, r, 0, true).unwrap();
        let value = CssDecodedStylesheet::try_success(d, report, "PRIVATE_TEXT".into()).unwrap();
        assert!(!format!("{value:?}").contains("PRIVATE_TEXT"));
        println!("parameter_rows=4");
    }
}
