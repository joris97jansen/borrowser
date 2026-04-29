use std::{collections::BTreeMap, fmt::Write};

use crate::{
    InitialStyleValue, PropertyId, PropertyInheritance,
    cascade::{
        ResolvedDocumentStyle, ResolvedStyle, ResolvedValueSource, StyleResolutionError,
        StyleResolutionLimits, try_resolve_document_styles_incremental_suffix_with_limits,
        try_resolve_document_styles_with_limits,
    },
    model, property_registry,
    selectors::{SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext},
};

use html::{Node, internal::Id};

use super::{
    builder::ComputedStyleBuilder,
    style::{ComputedStyle, ComputedStyleBuildError},
    value::{ComputedValue, ComputedValueNormalizationError, normalize_specified_value},
};

/// Error returned when structured cascade output cannot be materialized into a
/// total computed style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedStyleResolutionError {
    MissingResolvedElement {
        element: SelectorDomElementId,
    },
    ResolvedElementNameMismatch {
        element: SelectorDomElementId,
        expected: String,
        actual: String,
    },
    MissingComputedParent {
        element: SelectorDomElementId,
        parent: SelectorDomElementId,
    },
    MissingComputedElementStyle {
        element_index: usize,
        element_name: String,
    },
    ComputedElementNameMismatch {
        element_index: usize,
        expected: String,
        actual: String,
    },
    ComputedElementIdentityMismatch {
        element_index: usize,
        expected: SelectorDomElementId,
        actual: SelectorDomElementId,
    },
    ExtraComputedElementStyle {
        element: SelectorDomElementId,
    },
    MissingResolvedProperty {
        property: PropertyId,
    },
    MissingInheritedParent {
        property: PropertyId,
    },
    NonInheritedPropertyMarkedInherited {
        property: PropertyId,
    },
    InitialValueMismatch {
        property: PropertyId,
        expected: InitialStyleValue,
        actual: InitialStyleValue,
    },
    WinnerMissingSpecifiedValue {
        property: PropertyId,
    },
    WinnerPropertyMismatch {
        property: PropertyId,
        value_property: PropertyId,
    },
    Normalization(ComputedValueNormalizationError),
    Build(ComputedStyleBuildError),
    StyleResolution(StyleResolutionError),
}

impl std::fmt::Display for ComputedStyleResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResolvedElement { element } => write!(
                f,
                "resolved document style is missing element selector-id={}",
                element.get()
            ),
            Self::ResolvedElementNameMismatch {
                element,
                expected,
                actual,
            } => write!(
                f,
                "resolved document style element selector-id={} expected name \"{}\", got \"{}\"",
                element.get(),
                expected,
                actual
            ),
            Self::MissingComputedParent { element, parent } => write!(
                f,
                "computed document style element selector-id={} is missing computed parent selector-id={}",
                element.get(),
                parent.get()
            ),
            Self::MissingComputedElementStyle {
                element_index,
                element_name,
            } => write!(
                f,
                "computed document style is missing element[{element_index}] name \"{element_name}\""
            ),
            Self::ComputedElementNameMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected name \"{}\", got \"{}\"",
                expected, actual
            ),
            Self::ComputedElementIdentityMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected selector-id={}, got selector-id={}",
                expected.get(),
                actual.get()
            ),
            Self::ExtraComputedElementStyle { element } => write!(
                f,
                "computed document style has extra element selector-id={}",
                element.get()
            ),
            Self::MissingResolvedProperty { property } => write!(
                f,
                "resolved style is missing property '{}'",
                property.name()
            ),
            Self::MissingInheritedParent { property } => write!(
                f,
                "resolved style marks property '{}' inherited without a parent computed style",
                property.name()
            ),
            Self::NonInheritedPropertyMarkedInherited { property } => write!(
                f,
                "resolved style marks non-inherited property '{}' inherited",
                property.name()
            ),
            Self::InitialValueMismatch {
                property,
                expected,
                actual,
            } => write!(
                f,
                "resolved style initial value for '{}' expected {}, got {}",
                property.name(),
                expected.as_debug_label(),
                actual.as_debug_label()
            ),
            Self::WinnerMissingSpecifiedValue { property } => write!(
                f,
                "resolved style winner for '{}' does not carry a parsed specified value",
                property.name()
            ),
            Self::WinnerPropertyMismatch {
                property,
                value_property,
            } => write!(
                f,
                "resolved style winner for '{}' carries specified value for '{}'",
                property.name(),
                value_property.name()
            ),
            Self::Normalization(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
            Self::StyleResolution(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ComputedStyleResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Normalization(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::StyleResolution(error) => Some(error),
            Self::MissingResolvedElement { .. }
            | Self::ResolvedElementNameMismatch { .. }
            | Self::MissingComputedParent { .. }
            | Self::MissingComputedElementStyle { .. }
            | Self::ComputedElementNameMismatch { .. }
            | Self::ComputedElementIdentityMismatch { .. }
            | Self::ExtraComputedElementStyle { .. }
            | Self::MissingResolvedProperty { .. }
            | Self::MissingInheritedParent { .. }
            | Self::NonInheritedPropertyMarkedInherited { .. }
            | Self::InitialValueMismatch { .. }
            | Self::WinnerMissingSpecifiedValue { .. }
            | Self::WinnerPropertyMismatch { .. } => None,
        }
    }
}

/// Computed style for one DOM element in a document style pass.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedElementStyle {
    pub(super) selector_element_id: SelectorDomElementId,
    pub(super) element_name: String,
    pub(super) style: ComputedStyle,
}

impl ComputedElementStyle {
    fn new(
        selector_element_id: SelectorDomElementId,
        element_name: String,
        style: ComputedStyle,
    ) -> Self {
        Self {
            selector_element_id,
            element_name,
            style,
        }
    }

    pub fn selector_element_id(&self) -> SelectorDomElementId {
        self.selector_element_id
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn style(&self) -> &ComputedStyle {
        &self.style
    }
}

/// Document-order computed-style output for the element set selector matching
/// can address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedDocumentStyle {
    pub(super) entries: Vec<ComputedElementStyle>,
}

impl ComputedDocumentStyle {
    fn new(entries: Vec<ComputedElementStyle>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[ComputedElementStyle] {
        &self.entries
    }

    pub fn get(&self, element: SelectorDomElementId) -> Option<&ComputedElementStyle> {
        self.entries
            .iter()
            .find(|entry| entry.selector_element_id == element)
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "computed-document-style").expect("write snapshot");
        for (index, entry) in self.entries.iter().enumerate() {
            writeln!(
                &mut out,
                "element[{index}]: selector-id={} name=\"{}\"",
                entry.selector_element_id.get(),
                entry.element_name
            )
            .expect("write snapshot");
            for line in entry.style.to_debug_snapshot().lines().skip(2) {
                let line = line.strip_prefix("  ").unwrap_or(line);
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComputedStyleReuseStats {
    pub hits: usize,
    pub misses: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedDocumentStyleWithStats {
    pub computed: ComputedDocumentStyle,
    pub reuse_stats: ComputedStyleReuseStats,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IncrementalComputedDocumentStyle {
    pub resolved: ResolvedDocumentStyle,
    pub computed: ComputedDocumentStyle,
    pub reused_prefix_len: usize,
    pub recomputed_len: usize,
    pub reuse_stats: ComputedStyleReuseStats,
}

/// Materializes the structured cascade handoff into a total computed style.
///
/// Rejected invalid declarations do not appear in `ResolvedStyle` winners.
/// Fallback is therefore applied by the cascade source carried in each entry:
/// another valid winner, inheritance, or the property's initial/default value.
pub fn compute_style_from_resolved_style(
    resolved_style: &ResolvedStyle,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedStyle, ComputedStyleResolutionError> {
    let mut builder = ComputedStyleBuilder::new();

    for property in property_registry().ids() {
        let entry = resolved_style
            .get(property)
            .ok_or(ComputedStyleResolutionError::MissingResolvedProperty { property })?;
        let value = computed_value_from_resolved_source(property, entry.source(), parent_style)?;
        builder
            .record(property, value)
            .map_err(ComputedStyleResolutionError::Build)?;
    }

    builder.build().map_err(ComputedStyleResolutionError::Build)
}

fn computed_value_from_resolved_source(
    property: PropertyId,
    source: &ResolvedValueSource,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedValue, ComputedStyleResolutionError> {
    match source {
        ResolvedValueSource::Winner(winner) => {
            let specified = winner
                .value
                .parsed()
                .ok_or(ComputedStyleResolutionError::WinnerMissingSpecifiedValue { property })?;
            if specified.property() != property {
                return Err(ComputedStyleResolutionError::WinnerPropertyMismatch {
                    property,
                    value_property: specified.property(),
                });
            }

            normalize_specified_value(specified)
                .map_err(ComputedStyleResolutionError::Normalization)
        }
        ResolvedValueSource::Inherited => {
            if property.metadata().inheritance != PropertyInheritance::Inherited {
                return Err(
                    ComputedStyleResolutionError::NonInheritedPropertyMarkedInherited { property },
                );
            }

            let parent = parent_style
                .ok_or(ComputedStyleResolutionError::MissingInheritedParent { property })?;
            Ok(parent.get(property).value())
        }
        ResolvedValueSource::Initial(initial) => {
            let expected = property.initial_value();
            if *initial != expected {
                return Err(ComputedStyleResolutionError::InitialValueMismatch {
                    property,
                    expected,
                    actual: *initial,
                });
            }

            Ok(ComputedValue::from_initial(property))
        }
    }
}

/// Resolves and computes document-level styles without mutating the DOM.
pub fn compute_document_styles(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_with_limits(root, sheets, &StyleResolutionLimits::default())
}

pub fn compute_document_styles_with_limits(
    root: &Node,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    let resolved = try_resolve_document_styles_with_limits(root, sheets, limits)
        .map_err(ComputedStyleResolutionError::StyleResolution)?;
    compute_document_styles_from_resolved_styles(root, &resolved)
}

pub fn compute_document_styles_from_resolved_styles_with_reuse_stats(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
) -> Result<ComputedDocumentStyleWithStats, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_with_optional_prefix(
        root,
        resolved_styles,
        None,
        0,
    )
    .map(|computed| computed.expect("full computed style pass cannot miss prefix validation"))
}

pub fn compute_document_styles_incremental_suffix_with_limits(
    root: &Node,
    sheets: &[model::StylesheetParse],
    previous_resolved: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalComputedDocumentStyle>, ComputedStyleResolutionError> {
    let Some(resolved) = try_resolve_document_styles_incremental_suffix_with_limits(
        root,
        sheets,
        previous_resolved,
        dirty_node_ids,
        limits,
    )
    .map_err(ComputedStyleResolutionError::StyleResolution)?
    else {
        return Ok(None);
    };

    let Some(computed) = compute_document_styles_from_resolved_styles_incremental_suffix(
        root,
        &resolved.resolved,
        previous_computed,
        resolved.stats.reused_prefix_len,
    )?
    else {
        return Ok(None);
    };

    let reuse_stats = computed.reuse_stats;
    Ok(Some(IncrementalComputedDocumentStyle {
        resolved: resolved.resolved,
        computed: computed.computed,
        reused_prefix_len: resolved.stats.reused_prefix_len,
        recomputed_len: resolved.stats.recomputed_len,
        reuse_stats,
    }))
}

/// Computes document-level styles from an already materialized structured
/// cascade result.
pub fn compute_document_styles_from_resolved_styles(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_with_reuse_stats(root, resolved_styles)
        .map(|computed| computed.computed)
}

fn compute_document_styles_from_resolved_styles_incremental_suffix(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    reused_prefix_len: usize,
) -> Result<Option<ComputedDocumentStyleWithStats>, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_with_optional_prefix(
        root,
        resolved_styles,
        Some(previous_computed),
        reused_prefix_len,
    )
}

fn compute_document_styles_from_resolved_styles_with_optional_prefix(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
    previous_computed: Option<&ComputedDocumentStyle>,
    reused_prefix_len: usize,
) -> Result<Option<ComputedDocumentStyleWithStats>, ComputedStyleResolutionError> {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);

    if let Some(previous_computed) = previous_computed
        && (resolved_styles.entries().len() != index.len()
            || previous_computed.entries().len() != index.len()
            || reused_prefix_len > index.len())
    {
        return Ok(None);
    }

    let mut computed_by_element = BTreeMap::new();
    let mut entries = Vec::with_capacity(index.len());
    let mut reuse_cache = ComputedStyleReuseCache::default();

    for (element_index, element) in index.elements().enumerate() {
        let resolved = resolved_styles
            .get(element)
            .ok_or(ComputedStyleResolutionError::MissingResolvedElement { element })?;
        let expected_name = context.element_name(element);
        if resolved.element_name() != expected_name {
            return Err(ComputedStyleResolutionError::ResolvedElementNameMismatch {
                element,
                expected: expected_name.to_string(),
                actual: resolved.element_name().to_string(),
            });
        }

        let parent_style =
            match context.parent_element(element) {
                Some(parent) => Some(computed_by_element.get(&parent).ok_or(
                    ComputedStyleResolutionError::MissingComputedParent { element, parent },
                )?),
                None => None,
            };

        if element_index < reused_prefix_len {
            let previous = previous_computed
                .and_then(|computed| computed.entries().get(element_index))
                .expect("validated previous computed prefix");
            if previous.selector_element_id() != element || previous.element_name() != expected_name
            {
                return Ok(None);
            }

            reuse_cache.seed(resolved.style(), parent_style, *previous.style());
            computed_by_element.insert(element, *previous.style());
            entries.push(previous.clone());
            continue;
        }

        let style = reuse_cache.lookup_or_compute(resolved.style(), parent_style)?;

        computed_by_element.insert(element, style);
        entries.push(ComputedElementStyle::new(
            element,
            expected_name.to_string(),
            style,
        ));
    }

    Ok(Some(ComputedDocumentStyleWithStats {
        computed: ComputedDocumentStyle::new(entries),
        reuse_stats: reuse_cache.stats(),
    }))
}

// Pass-local cache for computed-style materialization.
//
// Reuse is valid only while `compute_style_from_resolved_style(...)` is a pure
// function of `(ResolvedStyle, Option<ComputedStyle parent>)`. If future
// computed-value logic depends on additional environment inputs such as
// viewport units, font metrics, writing mode context, visited-link privacy
// state, container queries, or media/device state, those inputs must either be
// added to this cache key or this reuse path must be disabled for affected
// properties.
#[derive(Default)]
struct ComputedStyleReuseCache {
    entries: Vec<ComputedStyleReuseEntry>,
    stats: ComputedStyleReuseStats,
}

impl ComputedStyleReuseCache {
    fn seed(
        &mut self,
        resolved_style: &ResolvedStyle,
        parent_style: Option<&ComputedStyle>,
        computed: ComputedStyle,
    ) {
        let parent = parent_style.copied();
        if self
            .entries
            .iter()
            .any(|entry| entry.resolved == *resolved_style && entry.parent == parent)
        {
            return;
        }

        self.entries.push(ComputedStyleReuseEntry {
            resolved: resolved_style.clone(),
            parent,
            computed,
        });
    }

    fn lookup_or_compute(
        &mut self,
        resolved_style: &ResolvedStyle,
        parent_style: Option<&ComputedStyle>,
    ) -> Result<ComputedStyle, ComputedStyleResolutionError> {
        let parent = parent_style.copied();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.resolved == *resolved_style && entry.parent == parent)
        {
            self.stats.hits = self.stats.hits.saturating_add(1);
            return Ok(entry.computed);
        }

        self.stats.misses = self.stats.misses.saturating_add(1);
        let computed = compute_style_from_resolved_style(resolved_style, parent_style)?;
        self.entries.push(ComputedStyleReuseEntry {
            resolved: resolved_style.clone(),
            parent,
            computed,
        });
        Ok(computed)
    }

    fn stats(&self) -> ComputedStyleReuseStats {
        self.stats
    }
}

struct ComputedStyleReuseEntry {
    resolved: ResolvedStyle,
    parent: Option<ComputedStyle>,
    computed: ComputedStyle,
}
