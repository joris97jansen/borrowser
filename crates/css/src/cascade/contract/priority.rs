use crate::selectors::Specificity;

use super::order::{DeclarationOrder, StylesheetRuleOrder};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    const fn semantic_rank(self) -> u8 {
        match self {
            Self::UserAgentNormal => 0,
            Self::UserNormal => 1,
            Self::AuthorNormal => 2,
            Self::AuthorImportant => 3,
            Self::UserImportant => 4,
            Self::UserAgentImportant => 5,
        }
    }
}

impl Ord for CurrentScopeCascadePriorityBand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.semantic_rank().cmp(&other.semantic_rank())
    }
}

impl PartialOrd for CurrentScopeCascadePriorityBand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Ordered origin/importance band used by winner resolution.
///
/// This ordering preserves the long-term CSS cascade hierarchy Borrowser is
/// growing toward. The current engine scope emits only the bands reachable
/// through `CurrentScopeCascadePriorityBand`; animation and transition remain
/// reserved for later milestones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    const fn semantic_rank(self) -> u8 {
        match self {
            Self::UserAgentNormal => 0,
            Self::UserNormal => 1,
            Self::AuthorNormal => 2,
            Self::Animation => 3,
            Self::AuthorImportant => 4,
            Self::UserImportant => 5,
            Self::UserAgentImportant => 6,
            Self::Transition => 7,
        }
    }
}

impl Ord for CascadeOriginBand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.semantic_rank().cmp(&other.semantic_rank())
    }
}

impl PartialOrd for CascadeOriginBand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Declaration-kind-specific precedence inside one origin/importance band.
///
/// The enum shape represents CSS Cascade Level 5's element-attached step
/// directly. Selector specificity and stylesheet order are meaningful only
/// for style rules; inline declarations are element-attached author
/// declarations and carry only their authored declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationPrecedence {
    StyleRule {
        specificity: Specificity,
        source_order: StylesheetRuleOrder,
        declaration_order: DeclarationOrder,
    },
    ElementAttached {
        declaration_order: DeclarationOrder,
    },
}

impl CascadeDeclarationPrecedence {
    pub const fn specificity(self) -> Option<Specificity> {
        match self {
            Self::StyleRule { specificity, .. } => Some(specificity),
            Self::ElementAttached { .. } => None,
        }
    }

    pub const fn source_order(self) -> Option<StylesheetRuleOrder> {
        match self {
            Self::StyleRule { source_order, .. } => Some(source_order),
            Self::ElementAttached { .. } => None,
        }
    }

    pub const fn declaration_order(self) -> DeclarationOrder {
        match self {
            Self::StyleRule {
                declaration_order, ..
            }
            | Self::ElementAttached { declaration_order } => declaration_order,
        }
    }

    pub const fn is_element_attached(self) -> bool {
        matches!(self, Self::ElementAttached { .. })
    }

    fn semantic_cmp(self, other: Self) -> std::cmp::Ordering {
        match (self, other) {
            (
                Self::StyleRule {
                    specificity: left_specificity,
                    source_order: left_source_order,
                    declaration_order: left_declaration_order,
                },
                Self::StyleRule {
                    specificity: right_specificity,
                    source_order: right_source_order,
                    declaration_order: right_declaration_order,
                },
            ) => left_specificity
                .cmp(&right_specificity)
                .then_with(|| left_source_order.cmp(&right_source_order))
                .then_with(|| left_declaration_order.cmp(&right_declaration_order)),
            (Self::StyleRule { .. }, Self::ElementAttached { .. }) => std::cmp::Ordering::Less,
            (Self::ElementAttached { .. }, Self::StyleRule { .. }) => std::cmp::Ordering::Greater,
            (
                Self::ElementAttached {
                    declaration_order: left,
                },
                Self::ElementAttached {
                    declaration_order: right,
                },
            ) => left.cmp(&right),
        }
    }
}

/// Fully ordered cascade comparison key for one declaration candidate.
///
/// Comparison is lexicographic by:
/// 1. origin/importance band
/// 2. element-attached versus style-rule declaration
/// 3. selector specificity for style rules
/// 4. order of appearance within the applicable declaration kind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadePriority {
    band: CurrentScopeCascadePriorityBand,
    declaration: CascadeDeclarationPrecedence,
}

impl CascadePriority {
    pub(super) const fn for_style_rule(
        band: CurrentScopeCascadePriorityBand,
        specificity: Specificity,
        source_order: StylesheetRuleOrder,
        declaration_order: DeclarationOrder,
    ) -> Self {
        Self {
            band,
            declaration: CascadeDeclarationPrecedence::StyleRule {
                specificity,
                source_order,
                declaration_order,
            },
        }
    }

    pub(super) const fn for_element_attached(
        importance: CascadeImportance,
        declaration_order: DeclarationOrder,
    ) -> Self {
        let band = match importance {
            CascadeImportance::Normal => CurrentScopeCascadePriorityBand::AuthorNormal,
            CascadeImportance::Important => CurrentScopeCascadePriorityBand::AuthorImportant,
        };
        Self {
            band,
            declaration: CascadeDeclarationPrecedence::ElementAttached { declaration_order },
        }
    }

    pub const fn band(self) -> CascadeOriginBand {
        self.band.as_origin_band()
    }

    pub const fn declaration_precedence(self) -> CascadeDeclarationPrecedence {
        self.declaration
    }

    pub const fn specificity(self) -> Option<Specificity> {
        self.declaration.specificity()
    }

    pub const fn source_order(self) -> Option<StylesheetRuleOrder> {
        self.declaration.source_order()
    }

    pub const fn declaration_order(self) -> DeclarationOrder {
        self.declaration.declaration_order()
    }

    #[cfg(test)]
    pub(crate) fn from_rule_context(
        context: super::sources::CascadeRuleContext,
        importance: CascadeImportance,
        declaration_order: impl Into<DeclarationOrder>,
    ) -> Self {
        context.priority_for_declaration(importance, declaration_order)
    }

    /// Returns the current emitted origin/importance band.
    ///
    /// `CascadePriority` is intentionally restricted to current constructors,
    /// so this is always `Some`. The broader `CascadeOriginBand` inspection
    /// type is where reserved animation and transition positions return
    /// `None` from `current_scope_band()`.
    pub const fn current_scope_band(self) -> Option<CurrentScopeCascadePriorityBand> {
        Some(self.band)
    }
}

impl Ord for CascadePriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.band
            .cmp(&other.band)
            .then_with(|| self.declaration.semantic_cmp(other.declaration))
    }
}

impl PartialOrd for CascadePriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::contract::{
        CascadeRuleContext, StyleRulePosition, StylesheetOrder, StylesheetRuleOrder,
    };

    fn author_priority(
        importance: CascadeImportance,
        context: CascadeRuleContext,
        declaration_order: u32,
    ) -> CascadePriority {
        CascadePriority::from_rule_context(context, importance, declaration_order)
    }

    #[test]
    fn inline_precedence_comes_from_element_attachment_and_declaration_order() {
        let stylesheet_order =
            StylesheetRuleOrder::new(StylesheetOrder::new(99), StyleRulePosition::new(99));
        let stylesheet_context = CascadeRuleContext::for_stylesheet(
            CascadeOrigin::Author,
            Specificity::new(1, 0, 0),
            stylesheet_order,
        );

        let author_normal = author_priority(CascadeImportance::Normal, stylesheet_context, 99);
        let inline_normal = author_priority(
            CascadeImportance::Normal,
            CascadeRuleContext::for_inline_style(),
            0,
        );
        assert!(
            inline_normal > author_normal,
            "element attachment supplies this win"
        );

        let author_important =
            author_priority(CascadeImportance::Important, stylesheet_context, 99);
        assert!(
            author_important > inline_normal,
            "importance band supplies this win"
        );

        let inline_important_first = author_priority(
            CascadeImportance::Important,
            CascadeRuleContext::for_inline_style(),
            0,
        );
        assert!(
            inline_important_first > author_important,
            "element attachment supplies this important-declaration win"
        );
        let inline_important_later = author_priority(
            CascadeImportance::Important,
            CascadeRuleContext::for_inline_style(),
            1,
        );
        assert!(
            inline_important_later > inline_important_first,
            "inline declaration order resolves the remaining tie"
        );

        assert_eq!(
            stylesheet_context.specificity(),
            Some(Specificity::new(1, 0, 0))
        );
    }

    fn representative_priorities() -> Vec<CascadePriority> {
        let stylesheet = |origin, importance, stylesheet, rule, specificity, declaration| {
            CascadeRuleContext::for_stylesheet(
                origin,
                specificity,
                StylesheetRuleOrder::new(
                    StylesheetOrder::new(stylesheet),
                    StyleRulePosition::new(rule),
                ),
            )
            .priority_for_declaration(importance, DeclarationOrder::new(declaration))
        };
        vec![
            stylesheet(
                CascadeOrigin::UserAgent,
                CascadeImportance::Normal,
                0,
                0,
                Specificity::ZERO,
                0,
            ),
            stylesheet(
                CascadeOrigin::User,
                CascadeImportance::Normal,
                1,
                0,
                Specificity::C,
                0,
            ),
            stylesheet(
                CascadeOrigin::Author,
                CascadeImportance::Normal,
                2,
                0,
                Specificity::C,
                0,
            ),
            stylesheet(
                CascadeOrigin::Author,
                CascadeImportance::Normal,
                2,
                1,
                Specificity::C,
                0,
            ),
            stylesheet(
                CascadeOrigin::Author,
                CascadeImportance::Normal,
                2,
                1,
                Specificity::B,
                1,
            ),
            CascadeRuleContext::for_inline_style()
                .priority_for_declaration(CascadeImportance::Normal, DeclarationOrder::new(0)),
            stylesheet(
                CascadeOrigin::Author,
                CascadeImportance::Important,
                2,
                1,
                Specificity::B,
                0,
            ),
            CascadeRuleContext::for_inline_style()
                .priority_for_declaration(CascadeImportance::Important, DeclarationOrder::new(0)),
            stylesheet(
                CascadeOrigin::User,
                CascadeImportance::Important,
                1,
                0,
                Specificity::C,
                0,
            ),
        ]
    }

    #[test]
    fn cascade_priority_obeys_total_order_laws() {
        let priorities = representative_priorities();
        for left in &priorities {
            for right in &priorities {
                assert_eq!(
                    left.cmp(right) == std::cmp::Ordering::Equal,
                    left == right,
                    "comparison equality must be structural equality"
                );
                assert_eq!(left.cmp(right), right.cmp(left).reverse());
            }
        }
        for first in &priorities {
            for second in &priorities {
                for third in &priorities {
                    if first <= second && second <= third {
                        assert!(first <= third, "cascade priority must be transitive");
                    }
                }
            }
        }
    }

    #[test]
    fn semantically_distinct_priorities_sort_without_stability_as_a_tie_breaker() {
        let mut priorities = representative_priorities();
        priorities.reverse();
        priorities.sort_unstable();
        assert!(priorities.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
