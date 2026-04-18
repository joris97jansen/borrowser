use crate::syntax::CssSpan;

/// Structural invariants for selector IR construction.
///
/// These checks keep selector node spans input-local, monotonic, and
/// payload-contained across the selector subsystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorStructureError {
    EmptySelectorList,
    EmptyCompoundSelector,
    EmptyIdentifier,
    MixedInputIds,
    NonMonotonicSpans,
    PayloadSpanOutsideNode,
}

pub(super) fn ensure_same_input(a: CssSpan, b: CssSpan) -> Result<(), SelectorStructureError> {
    if a.input_id == b.input_id {
        Ok(())
    } else {
        Err(SelectorStructureError::MixedInputIds)
    }
}

pub(super) fn ensure_span_contains(
    outer: CssSpan,
    inner: CssSpan,
) -> Result<(), SelectorStructureError> {
    ensure_same_input(outer, inner)?;
    if outer.start <= inner.start && inner.end <= outer.end {
        Ok(())
    } else {
        Err(SelectorStructureError::PayloadSpanOutsideNode)
    }
}

pub(super) fn ensure_payload_span_within_node(
    node_span: CssSpan,
    payload_span: Option<CssSpan>,
) -> Result<(), SelectorStructureError> {
    if let Some(payload_span) = payload_span {
        ensure_span_contains(node_span, payload_span)?;
    }
    Ok(())
}

pub(super) fn ensure_monotonic_same_input(
    spans: impl IntoIterator<Item = CssSpan>,
) -> Result<(), SelectorStructureError> {
    let mut iter = spans.into_iter();
    let Some(mut previous) = iter.next() else {
        return Ok(());
    };

    for span in iter {
        ensure_same_input(previous, span)?;
        if span.start < previous.end {
            return Err(SelectorStructureError::NonMonotonicSpans);
        }
        previous = span;
    }

    Ok(())
}
