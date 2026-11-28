//! CSS byte policy and bounded UTF-8 assembly. No CSS grammar or HTTP decoding.
pub use bus::css_decode::*;
use encoding_rs::Encoding;
use tools::common::MAX_STYLESHEET_BYTES;
use tools::utf8::{
    Utf8DecodeIssue, Utf8DecodeIssueKind, Utf8DecodeSink, Utf8DecoderState, finish_utf8_with_state,
    push_utf8_chunk_with_state,
};

pub fn resolve_stylesheet_label(label: Option<&str>) -> CssEncodingLabel {
    match label {
        None => CssEncodingLabel::Absent,
        Some(label) => Encoding::for_label(label.as_bytes())
            .map(CssEncoding::from_encoding)
            .map_or(CssEncodingLabel::Unknown, CssEncodingLabel::Recognized),
    }
}
pub fn stylesheet_environment(
    link: CssEncodingLabel,
    document: Option<CssEncoding>,
) -> Option<CssEnvironmentEncoding> {
    if let CssEncodingLabel::Recognized(e) = link {
        Some(CssEnvironmentEncoding::new(
            CssEnvironmentSource::LinkCharset,
            e,
        ))
    } else {
        document.map(|e| CssEnvironmentEncoding::new(CssEnvironmentSource::Document, e))
    }
}
fn bom(body: &[u8]) -> CssBom {
    if body.starts_with(b"\xef\xbb\xbf") {
        CssBom::Utf8
    } else if body.starts_with(b"\xfe\xff") {
        CssBom::Utf16Be
    } else if body.starts_with(b"\xff\xfe") {
        CssBom::Utf16Le
    } else {
        CssBom::None
    }
}
fn charset_prefix(body: &[u8]) -> Result<CssCharsetPrefix, CssDecodeEvidenceError> {
    if !body.starts_with(b"@charset \"") {
        return Ok(CssCharsetPrefix::NoExactPrefix);
    }
    let prefix = &body[..body.len().min(1024)];
    for index in 10..prefix.len() {
        let byte = prefix[index];
        if byte == b'"' {
            if prefix.get(index + 1) != Some(&b';') {
                return Ok(CssCharsetPrefix::Malformed);
            }
            // Every label byte was checked ASCII, so no lossy conversion is involved.
            let label = Encoding::for_label(&prefix[10..index])
                .map(CssEncoding::from_encoding)
                .map_or(CssEncodingLabel::Unknown, CssEncodingLabel::Recognized);
            return CssExactCharsetPrefix::try_new(index + 2, label).map(CssCharsetPrefix::Exact);
        }
        if byte > 0x7f {
            return Ok(CssCharsetPrefix::Malformed);
        }
    }
    Ok(CssCharsetPrefix::Malformed)
}
fn select_from_facts(
    body: &[u8],
    transport: CssEncodingLabel,
    environment: Option<CssEnvironmentEncoding>,
) -> Result<CssDecodingDecision, CssDecodeEvidenceError> {
    let signature = bom(body);
    let (transport, prefix, source, encoding) = match signature {
        CssBom::Utf8 => (
            CssEncodingLabel::NotConsulted,
            CssCharsetPrefix::NotConsulted,
            CssEncodingSource::Bom,
            CssEncoding::UTF8,
        ),
        CssBom::Utf16Be => (
            CssEncodingLabel::NotConsulted,
            CssCharsetPrefix::NotConsulted,
            CssEncodingSource::Bom,
            CssEncoding::from_encoding(encoding_rs::UTF_16BE),
        ),
        CssBom::Utf16Le => (
            CssEncodingLabel::NotConsulted,
            CssCharsetPrefix::NotConsulted,
            CssEncodingSource::Bom,
            CssEncoding::from_encoding(encoding_rs::UTF_16LE),
        ),
        CssBom::None => {
            if let CssEncodingLabel::Recognized(e) = transport {
                (
                    transport,
                    CssCharsetPrefix::NotConsulted,
                    CssEncodingSource::Transport,
                    e,
                )
            } else {
                let prefix = charset_prefix(body)?;
                let declared = match prefix {
                    CssCharsetPrefix::Exact(p) => p.label(),
                    _ => CssEncodingLabel::Unknown,
                };
                let (source, encoding) = if let CssEncodingLabel::Recognized(e) = declared {
                    (CssEncodingSource::CharsetDeclaration, e)
                } else if let Some(env) = environment {
                    (CssEncodingSource::Environment, env.encoding())
                } else {
                    (CssEncodingSource::Default, CssEncoding::UTF8)
                };
                (transport, prefix, source, encoding)
            }
        }
    };
    CssDecodingDecision::try_new(
        body.len(),
        signature,
        transport,
        prefix,
        environment,
        source,
        encoding,
    )
}
pub fn select_stylesheet_decoding(
    body: &[u8],
    transport_charset: Option<&str>,
    environment: Option<CssEnvironmentEncoding>,
) -> Result<CssDecodingDecision, CssDecodeEvidenceError> {
    select_from_facts(
        body,
        resolve_stylesheet_label(transport_charset),
        environment,
    )
}
pub fn validate_decision_for_body(
    body: &[u8],
    decision: &CssDecodingDecision,
) -> Result<(), CssDecodeEvidenceError> {
    if body.len() > MAX_STYLESHEET_BYTES {
        return Err(CssDecodeEvidenceError::Bounds);
    }
    if body.len() != decision.delivered_bytes() {
        return Err(CssDecodeEvidenceError::BodyLength);
    }
    let transport = if decision.bom() == CssBom::None {
        decision.transport()
    } else {
        CssEncodingLabel::Absent
    };
    if select_from_facts(body, transport, decision.environment())? != *decision {
        return Err(CssDecodeEvidenceError::Decision);
    }
    Ok(())
}
pub fn validate_decoded_stylesheet(
    expected: &CssDecodingDecision,
    body: &[u8],
    decoded: &CssDecodedStylesheet,
) -> Result<(), CssDecodeEvidenceError> {
    validate_decision_for_body(body, expected)?;
    if *expected != decoded.decision() {
        return Err(CssDecodeEvidenceError::Decision);
    }
    match (decoded.text(), decoded.report()) {
        (Ok(text), CssDecodeReport::Finalized(r)) => {
            CssFinalizedDecodeReport::try_new(
                *expected,
                r.delivered_bytes_processed(),
                r.bom_bytes_consumed(),
                r.payload_bytes_processed(),
                r.emitted_utf8_bytes(),
                r.replacements(),
                0,
                r.eof_processed(),
            )?;
            if text.len() != r.emitted_utf8_bytes() {
                return Err(CssDecodeEvidenceError::OutputLength);
            }
        }
        (Err(failure), report) => {
            if matches!(report, CssDecodeReport::Finalized(_))
                || (failure == CssDecodeFailure::OutputReservation
                    && report != CssDecodeReport::NotStarted)
                || (failure == CssDecodeFailure::OutputBound
                    && !matches!(report, CssDecodeReport::Interrupted(_)))
            {
                return Err(CssDecodeEvidenceError::FailureShape);
            }
            if let CssDecodeReport::Interrupted(r) = report {
                CssInterruptedDecodeReport::try_new(
                    *expected,
                    r.delivered_bytes_processed(),
                    r.bom_bytes_consumed(),
                    r.payload_bytes_processed(),
                    r.emitted_utf8_bytes(),
                    r.replacements(),
                    r.pending_payload_bytes(),
                    r.eof_processed(),
                )?;
            }
        }
        _ => return Err(CssDecodeEvidenceError::FailureShape),
    }
    Ok(())
}

struct Sink {
    text: String,
    output_limit: usize,
    valid_bytes: Option<usize>,
    invalid: Option<(usize, usize)>,
    incomplete: Option<(u8, u8)>,
    failure: Option<CssDecodeFailure>,
}
impl Sink {
    fn fail(&mut self, failure: CssDecodeFailure) {
        self.failure.get_or_insert(failure);
    }
    fn append(&mut self, text: &str) {
        if self.failure.is_some() {
            return;
        }
        match self.text.len().checked_add(text.len()) {
            Some(n) if n <= self.output_limit => self.text.push_str(text),
            Some(_) => self.fail(CssDecodeFailure::OutputBound),
            None => self.fail(CssDecodeFailure::ArithmeticOverflow),
        }
    }
    fn counts(&self) -> Option<CssReplacementCounts> {
        let (i, ai) = self.invalid?;
        let (f, af) = self.incomplete?;
        CssReplacementCounts::try_new(i, ai, f, af).ok()
    }
    fn pending(&self, processed: usize) -> Option<u8> {
        let affected = self.counts()?.affected_bytes()?;
        let pending = processed.checked_sub(self.valid_bytes?.checked_add(affected)?)?;
        u8::try_from(pending).ok().filter(|n| *n <= 3)
    }
}
impl Utf8DecodeSink for Sink {
    fn decoded_segment(&mut self, segment: &str) {
        self.valid_bytes = self.valid_bytes.and_then(|n| n.checked_add(segment.len()));
        if self.valid_bytes.is_none() {
            self.fail(CssDecodeFailure::ArithmeticOverflow);
        }
        self.append(segment);
    }
    fn replacement(&mut self, issue: Utf8DecodeIssue) {
        match issue.kind {
            Utf8DecodeIssueKind::InvalidSequence => {
                self.invalid = self.invalid.and_then(|(n, b)| {
                    Some((
                        n.checked_add(1)?,
                        b.checked_add(usize::from(issue.affected_byte_count.get()))?,
                    ))
                });
                if self.invalid.is_none() {
                    self.fail(CssDecodeFailure::ArithmeticOverflow);
                }
            }
            Utf8DecodeIssueKind::IncompleteSequenceAtEof => {
                self.incomplete = self.incomplete.and_then(|(n, b)| {
                    Some((
                        n.checked_add(1)?,
                        b.checked_add(issue.affected_byte_count.get())?,
                    ))
                });
                if self.incomplete.is_none() {
                    self.fail(CssDecodeFailure::ArithmeticOverflow);
                }
            }
        }
        self.append("\u{fffd}");
    }
}

pub fn decode_stylesheet(
    body: &[u8],
    decision: CssDecodingDecision,
) -> Result<CssDecodedStylesheet, CssDecodeEvidenceError> {
    decode_with_partitions(body, decision, &[body.len()], None, false)
}
// Production supplies one complete payload; partitions and refusal controls qualify
// this exact decoder boundary, not a multi-command runtime/session protocol.
fn decode_with_partitions(
    body: &[u8],
    decision: CssDecodingDecision,
    partitions: &[usize],
    output_limit: Option<usize>,
    refuse: bool,
) -> Result<CssDecodedStylesheet, CssDecodeEvidenceError> {
    validate_decision_for_body(body, &decision)?;
    if !decision.effective_encoding().is_utf8() {
        return Err(CssDecodeEvidenceError::Decision);
    }
    let bom = decision.consumed_bom_bytes();
    let payload = &body[bom..];
    let bound = payload
        .len()
        .checked_mul(3)
        .ok_or(CssDecodeEvidenceError::ArithmeticOverflow)?;
    let limit = output_limit.unwrap_or(bound).min(bound);
    let mut text = String::new();
    #[cfg(test)]
    crate::allocation_tests::before_output_reservation();
    if refuse || text.try_reserve_exact(bound).is_err() {
        return CssDecodedStylesheet::try_failure(
            decision,
            CssDecodeFailure::OutputReservation,
            CssDecodeReport::NotStarted,
        );
    }
    let mut sink = Sink {
        text,
        output_limit: limit,
        valid_bytes: Some(0),
        invalid: Some((0, 0)),
        incomplete: Some((0, 0)),
        failure: None,
    };
    let mut state = Utf8DecoderState::new();
    let mut processed = 0;
    for end in partitions
        .iter()
        .copied()
        .chain(std::iter::once(body.len()))
    {
        let end = end.saturating_sub(bom).min(payload.len());
        if end < processed {
            return Err(CssDecodeEvidenceError::Payload);
        }
        push_utf8_chunk_with_state(&mut state, &payload[processed..end], &mut sink);
        // The callback cannot stop tools' decoder. This records the WHOLE returned call.
        processed = end;
        if sink.failure.is_some() {
            break;
        }
    }
    let mut eof = false;
    if sink.failure.is_none() {
        finish_utf8_with_state(&mut state, &mut sink);
        eof = true;
    }
    if let Some(failure) = sink.failure {
        let report = CssInterruptedDecodeReport::try_new(
            decision,
            processed + bom,
            bom as u8,
            processed,
            sink.text.len(),
            sink.counts(),
            sink.pending(processed),
            eof,
        )?;
        return CssDecodedStylesheet::try_failure(
            decision,
            failure,
            CssDecodeReport::Interrupted(report),
        );
    }
    let counts = sink.counts().ok_or(CssDecodeEvidenceError::Replacements)?;
    let report = CssFinalizedDecodeReport::try_new(
        decision,
        body.len(),
        bom as u8,
        processed,
        sink.text.len(),
        counts,
        usize::from(state.has_pending_bytes()) as u8,
        eof,
    )?;
    CssDecodedStylesheet::try_success(decision, report, sink.text)
}

#[cfg(test)]
mod tests;
