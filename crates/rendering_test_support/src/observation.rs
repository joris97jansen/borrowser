use std::fmt;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderingObservationOwner {
    Layout,
    Paint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutObservationProfile {
    LayoutPhaseOutput,
    LayoutSizing,
    LayoutAdvancedFlow,
    LayoutFlex,
}

impl LayoutObservationProfile {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::LayoutPhaseOutput => "layout-phase-output",
            Self::LayoutSizing => "layout-sizing",
            Self::LayoutAdvancedFlow => "layout-advanced-flow",
            Self::LayoutFlex => "layout-flex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaintObservationProfile {
    PaintSemanticArtifact,
    PaintOrder,
    PaintStackingContexts,
    PaintLayering,
    PaintOperations,
}

impl PaintObservationProfile {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::PaintSemanticArtifact => "paint-semantic-artifact",
            Self::PaintOrder => "paint-order",
            Self::PaintStackingContexts => "paint-stacking-contexts",
            Self::PaintLayering => "paint-layering",
            Self::PaintOperations => "paint-operations",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(untagged)]
pub enum RenderingObservationProfile {
    Layout(LayoutObservationProfile),
    Paint(PaintObservationProfile),
}

impl RenderingObservationProfile {
    pub const fn owner(self) -> RenderingObservationOwner {
        match self {
            Self::Layout(_) => RenderingObservationOwner::Layout,
            Self::Paint(_) => RenderingObservationOwner::Paint,
        }
    }

    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Layout(profile) => profile.stable_label(),
            Self::Paint(profile) => profile.stable_label(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationSinkFailure {
    ByteLimitExceeded {
        maximum: usize,
        observed_at_least: usize,
    },
    AllocationFailure,
}

pub struct BoundedObservationSink {
    bytes: String,
    maximum: usize,
    failure: Option<ObservationSinkFailure>,
}

#[cfg(test)]
thread_local! {
    static FAIL_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl BoundedObservationSink {
    pub fn new(maximum: usize) -> Self {
        Self {
            bytes: String::new(),
            maximum,
            failure: None,
        }
    }

    pub const fn failure(&self) -> Option<ObservationSinkFailure> {
        self.failure
    }

    pub fn finish(self) -> Result<String, ObservationSinkFailure> {
        self.failure.map_or(Ok(self.bytes), Err)
    }

    fn reserve_for(&mut self, required: usize) -> Result<(), ObservationSinkFailure> {
        #[cfg(test)]
        if FAIL_RESERVATION.with(std::cell::Cell::get) {
            return Err(ObservationSinkFailure::AllocationFailure);
        }
        if required <= self.bytes.capacity() {
            return Ok(());
        }
        let doubled = self.bytes.capacity().max(64).saturating_mul(2);
        let target = required.max(doubled).min(self.maximum);
        self.bytes
            .try_reserve(target.saturating_sub(self.bytes.len()))
            .map_err(|_| ObservationSinkFailure::AllocationFailure)
    }
}

impl fmt::Write for BoundedObservationSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.failure.is_some() {
            return Err(fmt::Error);
        }
        let Some(required) = self.bytes.len().checked_add(value.len()) else {
            self.failure = Some(ObservationSinkFailure::ByteLimitExceeded {
                maximum: self.maximum,
                observed_at_least: usize::MAX,
            });
            return Err(fmt::Error);
        };
        if required > self.maximum {
            self.failure = Some(ObservationSinkFailure::ByteLimitExceeded {
                maximum: self.maximum,
                observed_at_least: required,
            });
            return Err(fmt::Error);
        }
        if let Err(failure) = self.reserve_for(required) {
            self.failure = Some(failure);
            return Err(fmt::Error);
        }
        self.bytes.push_str(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn bounded_sink_accepts_maximum_and_rejects_plus_one_without_partial_append() {
        let mut sink = BoundedObservationSink::new(4);
        sink.write_str("1234").unwrap();
        assert_eq!(sink.finish().unwrap(), "1234");

        let mut sink = BoundedObservationSink::new(4);
        sink.write_str("12").unwrap();
        assert!(sink.write_str("345").is_err());
        assert_eq!(
            sink.finish(),
            Err(ObservationSinkFailure::ByteLimitExceeded {
                maximum: 4,
                observed_at_least: 5,
            })
        );
    }

    #[test]
    fn allocation_failure_is_distinct_and_never_returns_partial_observation() {
        FAIL_RESERVATION.with(|failure| failure.set(true));
        let mut sink = BoundedObservationSink::new(64);
        assert!(sink.write_str("owner bytes").is_err());
        FAIL_RESERVATION.with(|failure| failure.set(false));
        assert_eq!(
            sink.finish(),
            Err(ObservationSinkFailure::AllocationFailure)
        );
    }

    #[test]
    fn production_layout_writer_stops_at_the_sink_limit() {
        let dom = html::parse_document(
            "<!doctype html><html><body></body></html>",
            html::HtmlParseOptions::default(),
        )
        .unwrap()
        .document;
        let styled = css::build_style_tree(&dom, None);
        let metrics = crate::SyntheticTextMetricsV1::SyntheticTextMetricsV1;
        let layout = layout::layout_document(layout::LayoutPhaseInput::new(
            &styled, 320.0, &metrics, None,
        ));
        let mut sink = BoundedObservationSink::new(8);
        assert!(layout.write_debug_snapshot(&mut sink).is_err());
        assert!(matches!(
            sink.finish(),
            Err(ObservationSinkFailure::ByteLimitExceeded { maximum: 8, .. })
        ));
    }
}
