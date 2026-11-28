//! Host-language facts, deliberately separate from CSS media classification.
use bus::css_decode::{
    CssDecodeFailure, CssDecodeReport, CssDecodingDecision, CssEncodingLabel,
    CssEnvironmentEncoding, CssRuntimeFailure,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineType {
    NotApplicable,
    Missing,
    Empty,
    Css,
    Unsupported,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetTitle {
    Missing,
    Empty,
    Named,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostEligibility {
    pub(crate) inline_type: InlineType,
    pub(crate) title: SheetTitle,
    pub(crate) disabled: bool,
    pub(crate) alternate: bool,
}
impl HostEligibility {
    pub fn inline_type(self) -> InlineType {
        self.inline_type
    }
    pub fn title(self) -> SheetTitle {
        self.title
    }
    pub fn disabled(self) -> bool {
        self.disabled
    }
    pub fn alternate(self) -> bool {
        self.alternate
    }
    pub(crate) fn from_node(node: &html::Node, external: bool) -> Self {
        Self {
            inline_type: if external {
                InlineType::NotApplicable
            } else {
                match node.attr("type") {
                    None => InlineType::Missing,
                    Some("") => InlineType::Empty,
                    Some(t) if t.eq_ignore_ascii_case("text/css") => InlineType::Css,
                    Some(_) => InlineType::Unsupported,
                }
            },
            title: match node.attr("title") {
                None => SheetTitle::Missing,
                Some("") => SheetTitle::Empty,
                Some(_) => SheetTitle::Named,
            },
            disabled: external && node.attr("disabled").is_some(),
            // Preserve the production DOM token membership contract.
            alternate: external && node.attr_has_token("rel", "alternate"),
        }
    }
    pub(crate) fn parses_inline(self) -> bool {
        self.inline_type != InlineType::Unsupported
    }
    pub(crate) fn participates(self) -> bool {
        self.parses_inline() && self.title != SheetTitle::Named && !self.disabled && !self.alternate
    }
}
impl Default for HostEligibility {
    fn default() -> Self {
        Self {
            inline_type: InlineType::NotApplicable,
            title: SheetTitle::Missing,
            disabled: false,
            alternate: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetDecodingOutcome {
    Selected(CssDecodingDecision),
    Rejected(CssDecodingDecision),
    Succeeded {
        decision: CssDecodingDecision,
        report: CssDecodeReport,
    },
    Failed {
        decision: CssDecodingDecision,
        report: CssDecodeReport,
        failure: CssDecodeFailure,
    },
}
impl StylesheetDecodingOutcome {
    pub(crate) fn coherent_report(self) -> bool {
        use bus::css_decode::{CssDecodeReport, CssDecodedStylesheet, CssFinalizedDecodeReport};
        match self {
            Self::Selected(_) | Self::Rejected(_) => true,
            Self::Succeeded {
                decision,
                report: CssDecodeReport::Finalized(r),
            } => CssFinalizedDecodeReport::try_new(
                decision,
                r.delivered_bytes_processed(),
                r.bom_bytes_consumed(),
                r.payload_bytes_processed(),
                r.emitted_utf8_bytes(),
                r.replacements(),
                0,
                r.eof_processed(),
            )
            .is_ok(),
            Self::Succeeded { .. } => false,
            Self::Failed {
                decision,
                report,
                failure,
            } => CssDecodedStylesheet::try_failure(decision, failure, report).is_ok(),
        }
    }
    pub(crate) fn decision(self) -> CssDecodingDecision {
        match self {
            Self::Selected(d) | Self::Rejected(d) => d,
            Self::Succeeded { decision, .. } | Self::Failed { decision, .. } => decision,
        }
    }
    pub(crate) fn succeeded(self) -> bool {
        matches!(self, Self::Succeeded { .. })
    }
    pub(crate) fn failed(self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssRuntimeReceipt {
    Done,
    Failed(CssRuntimeFailure),
}

pub(super) fn complete(parse: &css::StylesheetParse) -> bool {
    parse.completion() == css::StylesheetParseCompletion::Complete
}
pub(super) fn contributes(
    host: HostEligibility,
    media: Option<&str>,
    parse: &css::StylesheetParse,
) -> bool {
    host.participates()
        && complete(parse)
        && !css::StylesheetConditionInput::from_optional_raw_media(media).is_deferred()
}
// Canonical small facts are copied; raw labels stay in DOM/immutable response evidence.
pub(super) type LinkLabel = CssEncodingLabel;
pub(super) type Environment = Option<CssEnvironmentEncoding>;
