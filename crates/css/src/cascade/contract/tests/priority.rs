use super::super::{
    CascadeImportance, CascadeOrigin, CascadeOriginBand, CascadePriority, CascadeSpecificity,
    CurrentScopeCascadePriorityBand,
};
use crate::selectors::Specificity;

#[test]
fn cascade_priority_orders_inline_style_above_selector_specificity() {
    let author_normal = CurrentScopeCascadePriorityBand::AuthorNormal.as_origin_band();
    let selector_priority = CascadePriority::new(
        author_normal,
        CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
        4,
        0,
    );
    let inline_priority =
        CascadePriority::new(author_normal, CascadeSpecificity::InlineStyle, 0, 0);

    assert!(inline_priority > selector_priority);
}

#[test]
fn current_scope_priority_bands_map_origin_and_importance_explicitly() {
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::UserAgent,
            CascadeImportance::Normal
        ),
        CurrentScopeCascadePriorityBand::UserAgentNormal
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::User,
            CascadeImportance::Normal
        ),
        CurrentScopeCascadePriorityBand::UserNormal
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::Author,
            CascadeImportance::Normal
        ),
        CurrentScopeCascadePriorityBand::AuthorNormal
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::Author,
            CascadeImportance::Important
        ),
        CurrentScopeCascadePriorityBand::AuthorImportant
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::User,
            CascadeImportance::Important
        ),
        CurrentScopeCascadePriorityBand::UserImportant
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::from_origin_and_importance(
            CascadeOrigin::UserAgent,
            CascadeImportance::Important
        ),
        CurrentScopeCascadePriorityBand::UserAgentImportant
    );
    assert_eq!(
        CurrentScopeCascadePriorityBand::AuthorImportant.as_origin_band(),
        CascadeOriginBand::AuthorImportant
    );
    assert_eq!(
        CascadeOriginBand::UserImportant.current_scope_band(),
        Some(CurrentScopeCascadePriorityBand::UserImportant)
    );
    assert_eq!(CascadeOriginBand::Animation.current_scope_band(), None);
    assert!(
        CurrentScopeCascadePriorityBand::AuthorNormal > CurrentScopeCascadePriorityBand::UserNormal
    );
    assert!(
        CurrentScopeCascadePriorityBand::UserImportant
            > CurrentScopeCascadePriorityBand::AuthorImportant
    );
}

#[test]
fn cascade_priority_current_scope_band_is_an_inspection_helper_for_future_bands() {
    let current_scope_priority = CascadePriority::new(
        CascadeOriginBand::AuthorNormal,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
        0,
    );
    let animation_priority = CascadePriority::new(
        CascadeOriginBand::Animation,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
        0,
    );
    let transition_priority = CascadePriority::new(
        CascadeOriginBand::Transition,
        CascadeSpecificity::InlineStyle,
        0,
        0,
    );

    assert_eq!(
        current_scope_priority.current_scope_band(),
        Some(CurrentScopeCascadePriorityBand::AuthorNormal)
    );
    assert_eq!(animation_priority.current_scope_band(), None);
    assert_eq!(transition_priority.current_scope_band(), None);
}

#[test]
fn cascade_origin_bands_preserve_future_css_ordering() {
    assert!(CascadeOriginBand::AuthorNormal > CascadeOriginBand::UserNormal);
    assert!(CascadeOriginBand::Animation > CascadeOriginBand::AuthorNormal);
    assert!(CascadeOriginBand::UserImportant > CascadeOriginBand::AuthorImportant);
    assert!(CascadeOriginBand::Transition > CascadeOriginBand::UserAgentImportant);
}
