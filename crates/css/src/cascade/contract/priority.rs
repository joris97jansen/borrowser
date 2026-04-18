use crate::selectors::Specificity;

use super::properties::CascadePropertyId;

/// Cascade precedence ordering for Borrowser's current cascade subset.
///
/// This module owns origin/importance bands, specificity handling, and the
/// deterministic comparison keys used by winner resolution. It does not own
/// source identity or resolved-style materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeOrigin {
    UserAgent,
    User,
    Author,
}

/// Importance bucket preserved by the model and consumed by cascade ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeImportance {
    Normal,
    Important,
}

/// Explicit origin/priority model emitted by Borrowser's current CSS scope.
///
/// This is the currently supported cross-product of rule origin and
/// declaration-level importance. Future cascade levels such as animations and
/// transitions remain outside this hot-path type and are integrated through
/// the broader `CascadeOriginBand` ordering model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CurrentScopeCascadePriorityBand {
    UserAgentNormal,
    UserNormal,
    AuthorNormal,
    AuthorImportant,
    UserImportant,
    UserAgentImportant,
}

impl CurrentScopeCascadePriorityBand {
    pub const fn from_origin_and_importance(
        origin: CascadeOrigin,
        importance: CascadeImportance,
    ) -> Self {
        match (origin, importance) {
            (CascadeOrigin::UserAgent, CascadeImportance::Normal) => Self::UserAgentNormal,
            (CascadeOrigin::User, CascadeImportance::Normal) => Self::UserNormal,
            (CascadeOrigin::Author, CascadeImportance::Normal) => Self::AuthorNormal,
            (CascadeOrigin::Author, CascadeImportance::Important) => Self::AuthorImportant,
            (CascadeOrigin::User, CascadeImportance::Important) => Self::UserImportant,
            (CascadeOrigin::UserAgent, CascadeImportance::Important) => Self::UserAgentImportant,
        }
    }

    pub const fn as_origin_band(self) -> CascadeOriginBand {
        match self {
            Self::UserAgentNormal => CascadeOriginBand::UserAgentNormal,
            Self::UserNormal => CascadeOriginBand::UserNormal,
            Self::AuthorNormal => CascadeOriginBand::AuthorNormal,
            Self::AuthorImportant => CascadeOriginBand::AuthorImportant,
            Self::UserImportant => CascadeOriginBand::UserImportant,
            Self::UserAgentImportant => CascadeOriginBand::UserAgentImportant,
        }
    }

    /// Debug label for the current emitted priority model.
    ///
    /// The label intentionally matches the corresponding current-scope
    /// `CascadeOriginBand` label. Debug surfaces that need to distinguish the
    /// emitted current-scope model from the broader future precedence space
    /// should do so by carrying the type context explicitly rather than by
    /// expecting different string payloads here.
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::UserAgentNormal => "user-agent-normal",
            Self::UserNormal => "user-normal",
            Self::AuthorNormal => "author-normal",
            Self::AuthorImportant => "author-important",
            Self::UserImportant => "user-important",
            Self::UserAgentImportant => "user-agent-important",
        }
    }
}

/// Ordered origin/importance band used by winner resolution.
///
/// This ordering preserves the long-term CSS cascade hierarchy Borrowser is
/// growing toward. The current engine scope emits only the bands reachable
/// through `CurrentScopeCascadePriorityBand`; animation and transition remain
/// reserved for later milestones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CascadeOriginBand {
    UserAgentNormal,
    UserNormal,
    AuthorNormal,
    Animation,
    AuthorImportant,
    UserImportant,
    UserAgentImportant,
    Transition,
}

impl CascadeOriginBand {
    pub const fn from_current_scope_band(band: CurrentScopeCascadePriorityBand) -> Self {
        band.as_origin_band()
    }

    /// Returns the matching current-scope priority band when this precedence
    /// level is emitted by today's engine path.
    ///
    /// This is an inspection helper, not a total conversion: reserved future
    /// precedence levels such as `Animation` and `Transition` intentionally
    /// return `None`.
    pub const fn current_scope_band(self) -> Option<CurrentScopeCascadePriorityBand> {
        match self {
            Self::UserAgentNormal => Some(CurrentScopeCascadePriorityBand::UserAgentNormal),
            Self::UserNormal => Some(CurrentScopeCascadePriorityBand::UserNormal),
            Self::AuthorNormal => Some(CurrentScopeCascadePriorityBand::AuthorNormal),
            Self::AuthorImportant => Some(CurrentScopeCascadePriorityBand::AuthorImportant),
            Self::UserImportant => Some(CurrentScopeCascadePriorityBand::UserImportant),
            Self::UserAgentImportant => Some(CurrentScopeCascadePriorityBand::UserAgentImportant),
            Self::Animation | Self::Transition => None,
        }
    }

    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::UserAgentNormal => "user-agent-normal",
            Self::UserNormal => "user-normal",
            Self::AuthorNormal => "author-normal",
            Self::Animation => "animation",
            Self::AuthorImportant => "author-important",
            Self::UserImportant => "user-important",
            Self::UserAgentImportant => "user-agent-important",
            Self::Transition => "transition",
        }
    }
}

/// Specificity surface consumed by cascade ordering.
///
/// Stylesheet rules carry selector-derived specificity. Inline style
/// declarations occupy a dedicated top slot within the current author-origin
/// scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CascadeSpecificity {
    Selector(Specificity),
    InlineStyle,
}

/// Fully ordered cascade comparison key for one declaration candidate.
///
/// Comparison is lexicographic by:
/// 1. origin/importance band
/// 2. selector specificity or inline-style sentinel
/// 3. rule order in stylesheet insertion/source order
/// 4. declaration order within the source rule or inline style attribute
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct CascadePriority {
    pub band: CascadeOriginBand,
    pub specificity: CascadeSpecificity,
    pub rule_order: u32,
    pub declaration_order: u32,
}

impl CascadePriority {
    pub const fn new(
        band: CascadeOriginBand,
        specificity: CascadeSpecificity,
        rule_order: u32,
        declaration_order: u32,
    ) -> Self {
        Self {
            band,
            specificity,
            rule_order,
            declaration_order,
        }
    }

    /// Returns the matching current-scope band when this priority was produced
    /// by Borrowser's current emitted origin/priority model.
    ///
    /// This remains an inspection helper only. Priorities built from reserved
    /// future precedence bands are expected to return `None`.
    pub const fn current_scope_band(self) -> Option<CurrentScopeCascadePriorityBand> {
        self.band.current_scope_band()
    }
}

/// Deterministic ordering key for cascade declaration candidates.
///
/// Sorting by this key groups candidates by property and then orders them by
/// the lexicographic cascade precedence defined by `CascadePriority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct CascadeDeclarationCandidateKey {
    pub property: CascadePropertyId,
    pub priority: CascadePriority,
}
