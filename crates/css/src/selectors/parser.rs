pub(super) use super::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, Combinator, CombinedSelector, ComplexSelector, CompoundSelector,
    IdSelector, InvalidSelectorList, InvalidSelectorReason, SelectorIdent, SelectorList,
    SelectorListParseResult, SelectorString, SelectorStructureError, SubclassSelector,
    TypeSelector, UnsupportedSelectorFeature, UnsupportedSelectorList,
};
pub(super) use crate::syntax::{
    CssBlockKind, CssComponentValue, CssHashKind, CssInput, CssSpan, CssToken, CssTokenKind,
    CssTokenText,
};

mod attribute;
mod convert;
mod list;
mod segment;
mod simple;
mod spans;
mod trivia;

pub use list::{parse_selector_list, parse_selector_list_with_limits};
