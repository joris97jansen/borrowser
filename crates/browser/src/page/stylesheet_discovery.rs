//! Passive authored stylesheet candidates. No loader, parsing, or retained state.
use addressing::{
    BrowserUrl, ResourceAddress, ResourceKind, UrlDiagnostic, UrlFailure, UrlParseOutcome,
};
use html::{AttributeNamespace, ElementNamespace, ElementNode, Node, internal::Id};

pub(crate) mod debug;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetDiscoveryLimit {
    OrdinaryNodes,
    OrdinaryDepth,
    Occurrences,
    AttributeEntries,
    ClassificationInputBytes,
    ResolutionPairInputBytes,
    TotalResolutionInputBytes,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetDiscoveryStorage {
    TraversalFrames,
    Occurrences,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetIndexDomain {
    Occurrence,
    Dependency,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetDiscoveryError {
    CommittedStateInconsistent,
    SourceIdentityUnavailable,
    LimitExceeded {
        dimension: StylesheetDiscoveryLimit,
        maximum: usize,
    },
    ArithmeticOverflow(StylesheetDiscoveryLimit),
    IndexOverflow(StylesheetIndexDomain),
    ReservationFailed(StylesheetDiscoveryStorage),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetDiscoverySnapshotError {
    OutputAllocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccurrenceIndex(u32);
impl OccurrenceIndex {
    pub fn get(self) -> u32 {
        self.0
    }
    fn from_count(value: usize) -> Result<Self, StylesheetDiscoveryError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| StylesheetDiscoveryError::IndexOverflow(StylesheetIndexDomain::Occurrence))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylesheetDependencyOrder(u32);
impl StylesheetDependencyOrder {
    pub fn get(self) -> u32 {
        self.0
    }
    fn from_count(value: usize) -> Result<Self, StylesheetDiscoveryError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| StylesheetDiscoveryError::IndexOverflow(StylesheetIndexDomain::Dependency))
    }
}

// Source-bearing types deliberately implement neither Debug nor Clone.
pub struct StylesheetDiscovery<'dom> {
    records: Vec<StylesheetOccurrence<'dom>>,
}
impl<'dom> StylesheetDiscovery<'dom> {
    pub fn records(&self) -> &[StylesheetOccurrence<'dom>] {
        &self.records
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn dependencies(&self) -> impl Iterator<Item = &StylesheetOccurrence<'dom>> + '_ {
        self.records.iter().filter(|r| r.dependency_order.is_some())
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn style_attributes(&self) -> impl Iterator<Item = &StylesheetOccurrence<'dom>> + '_ {
        self.records
            .iter()
            .filter(|r| matches!(r.source, StylesheetSource::StyleAttribute { .. }))
    }
}
pub struct StylesheetOccurrence<'dom> {
    occurrence: OccurrenceIndex,
    dependency_order: Option<StylesheetDependencyOrder>,
    owner: Id,
    source: StylesheetSource<'dom>,
}
impl<'dom> StylesheetOccurrence<'dom> {
    pub fn occurrence_index(&self) -> OccurrenceIndex {
        self.occurrence
    }
    pub fn dependency_order(&self) -> Option<StylesheetDependencyOrder> {
        self.dependency_order
    }
    pub fn source_node(&self) -> Id {
        self.owner
    }
    pub fn source(&self) -> &StylesheetSource<'dom> {
        &self.source
    }
}
pub enum StylesheetSource<'dom> {
    Link(LinkDiscovery<'dom>),
    StyleBlock(StyleBlockDiscovery<'dom>),
    StyleAttribute { value: &'dom str },
}
pub struct LinkDiscovery<'dom> {
    relation: RelationFacts<'dom>,
    href: HrefObservation<'dom>,
    type_hint: LinkTypeFact<'dom>,
    title: TitleFact<'dom>,
    disabled: DisabledFact<'dom>,
    media: MediaFact<'dom>,
    policies: LinkPolicyFacts<'dom>,
}
impl<'dom> LinkDiscovery<'dom> {
    pub fn relation(&self) -> &RelationFacts<'dom> {
        &self.relation
    }
    pub fn href(&self) -> &HrefObservation<'dom> {
        &self.href
    }
    pub fn type_hint(&self) -> &LinkTypeFact<'dom> {
        &self.type_hint
    }
    pub fn title(&self) -> &TitleFact<'dom> {
        &self.title
    }
    pub fn disabled(&self) -> &DisabledFact<'dom> {
        &self.disabled
    }
    pub fn media(&self) -> &MediaFact<'dom> {
        &self.media
    }
    pub fn policies(&self) -> &LinkPolicyFacts<'dom> {
        &self.policies
    }
    pub fn selection(&self) -> SetSelectionFact {
        if !self.relation.stylesheet_token {
            return SetSelectionFact::NotStylesheetRelationship;
        }
        selection(self.relation.alternate_token, &self.title)
    }
}
pub struct StyleBlockDiscovery<'dom> {
    content: InlineStyleText<'dom>,
    type_fact: StyleTypeFact<'dom>,
    title: TitleFact<'dom>,
    media: MediaFact<'dom>,
    inapplicable_link_attributes: StyleBlockLinkAttributes<'dom>,
}
impl<'dom> StyleBlockDiscovery<'dom> {
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn content(&self) -> &InlineStyleText<'dom> {
        &self.content
    }
    pub fn type_fact(&self) -> &StyleTypeFact<'dom> {
        &self.type_fact
    }
    pub fn title(&self) -> &TitleFact<'dom> {
        &self.title
    }
    pub fn media(&self) -> &MediaFact<'dom> {
        &self.media
    }
    pub fn inapplicable_link_attributes(&self) -> &StyleBlockLinkAttributes<'dom> {
        &self.inapplicable_link_attributes
    }
    pub fn selection(&self) -> SetSelectionFact {
        selection(AlternateFact::Absent, &self.title)
    }
}
pub struct HrefObservation<'dom> {
    authored: Option<&'dom str>,
    outcome: StylesheetHrefOutcome,
}
impl<'dom> HrefObservation<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        self.authored
    }
    pub fn outcome(&self) -> &StylesheetHrefOutcome {
        &self.outcome
    }
}
pub enum StylesheetHrefOutcome {
    NotStylesheetRelationship,
    Missing,
    Empty,
    Resolved {
        address: ResourceAddress,
        diagnostics: Vec<UrlDiagnostic>,
    },
    Failed(UrlFailure),
}
impl StylesheetHrefOutcome {
    pub fn diagnostics(&self) -> &[UrlDiagnostic] {
        match self {
            Self::Resolved { diagnostics, .. } => diagnostics,
            Self::Failed(f) => &f.diagnostics,
            _ => &[],
        }
    }
}
pub struct RelationFacts<'dom> {
    authored: Option<&'dom str>,
    stylesheet_token: bool,
    alternate_token: AlternateFact,
    other_tokens: bool,
}
impl<'dom> RelationFacts<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        self.authored
    }
    pub fn kind(&self) -> RelationKind {
        if self.authored.is_none() {
            RelationKind::Missing
        } else if self.stylesheet_token {
            RelationKind::Stylesheet
        } else if self.alternate_token == AlternateFact::Present || self.other_tokens {
            RelationKind::NonStylesheet
        } else {
            RelationKind::NoTokens
        }
    }
    pub fn alternate(&self) -> AlternateFact {
        self.alternate_token
    }
    pub fn other_tokens(&self) -> bool {
        self.other_tokens
    }
    fn classify(authored: Option<&'dom str>) -> Self {
        let mut result = Self {
            authored,
            stylesheet_token: false,
            alternate_token: AlternateFact::Absent,
            other_tokens: false,
        };
        if let Some(raw) = authored {
            for token in super::html_resources::html_rel_tokens(raw) {
                if token.eq_ignore_ascii_case("stylesheet") {
                    result.stylesheet_token = true;
                } else if token.eq_ignore_ascii_case("alternate") {
                    result.alternate_token = AlternateFact::Present;
                } else {
                    result.other_tokens = true;
                }
            }
        }
        result
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationKind {
    Missing,
    NoTokens,
    NonStylesheet,
    Stylesheet,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlternateFact {
    Absent,
    Present,
}
pub enum LinkTypeFact<'dom> {
    Missing,
    AdvisoryUnassessed(&'dom str),
}
pub enum StyleTypeFact<'dom> {
    Missing,
    Empty,
    Css(&'dom str),
    Other(&'dom str),
}
pub enum TitleFact<'dom> {
    Missing,
    Empty,
    Named(&'dom str),
}
pub enum DisabledFact<'dom> {
    NotStylesheetRelationship(Option<&'dom str>),
    Absent,
    Present(&'dom str),
}
pub enum MediaFact<'dom> {
    NotStylesheetRelationship(Option<&'dom str>),
    ActiveSubset(Option<&'dom str>),
    DeferredUnsupported(&'dom str),
}
pub enum PolicyFact<'dom> {
    NotAssessedForOtherRelationship(Option<&'dom str>),
    Absent,
    PresentUnenforced(&'dom str),
}
pub struct LinkPolicyFacts<'dom> {
    crossorigin: PolicyFact<'dom>,
    integrity: PolicyFact<'dom>,
    referrerpolicy: PolicyFact<'dom>,
}
impl<'dom> LinkPolicyFacts<'dom> {
    pub fn crossorigin(&self) -> &PolicyFact<'dom> {
        &self.crossorigin
    }
    pub fn integrity(&self) -> &PolicyFact<'dom> {
        &self.integrity
    }
    pub fn referrerpolicy(&self) -> &PolicyFact<'dom> {
        &self.referrerpolicy
    }
}
pub struct StyleBlockLinkAttributes<'dom> {
    disabled: Option<&'dom str>,
    crossorigin: Option<&'dom str>,
    integrity: Option<&'dom str>,
    referrerpolicy: Option<&'dom str>,
}
impl<'dom> StyleBlockLinkAttributes<'dom> {
    pub fn disabled(&self) -> Option<&'dom str> {
        self.disabled
    }
    pub fn crossorigin(&self) -> Option<&'dom str> {
        self.crossorigin
    }
    pub fn integrity(&self) -> Option<&'dom str> {
        self.integrity
    }
    pub fn referrerpolicy(&self) -> Option<&'dom str> {
        self.referrerpolicy
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlternateTitleFact {
    Missing,
    Empty,
    Named,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetSelectionFact {
    NotStylesheetRelationship,
    NoNamedOrAlternateInput,
    NamedSelectionUnsupported,
    AlternateSelectionUnsupported(AlternateTitleFact),
}

pub struct InlineStyleText<'dom> {
    children: &'dom [Node],
}
pub struct StyleTextChunks<'dom> {
    children: std::slice::Iter<'dom, Node>,
}
impl<'dom> InlineStyleText<'dom> {
    pub fn text_chunks(&self) -> StyleTextChunks<'dom> {
        StyleTextChunks {
            children: self.children.iter(),
        }
    }
}
impl<'dom> Iterator for StyleTextChunks<'dom> {
    type Item = &'dom str;
    fn next(&mut self) -> Option<Self::Item> {
        self.children.find_map(|n| match n {
            Node::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }
}

// Raw accessors preserve absent versus present-empty without source Debug.
impl<'dom> LinkTypeFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::Missing => None,
            Self::AdvisoryUnassessed(v) => Some(v),
        }
    }
}
impl<'dom> StyleTypeFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::Missing => None,
            Self::Empty => Some(""),
            Self::Css(v) | Self::Other(v) => Some(v),
        }
    }
}
impl<'dom> TitleFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::Missing => None,
            Self::Empty => Some(""),
            Self::Named(v) => Some(v),
        }
    }
}
impl<'dom> DisabledFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::NotStylesheetRelationship(v) => *v,
            Self::Absent => None,
            Self::Present(v) => Some(v),
        }
    }
}
impl<'dom> MediaFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::NotStylesheetRelationship(v) | Self::ActiveSubset(v) => *v,
            Self::DeferredUnsupported(v) => Some(v),
        }
    }
}
impl<'dom> PolicyFact<'dom> {
    pub fn authored(&self) -> Option<&'dom str> {
        match self {
            Self::NotAssessedForOtherRelationship(v) => *v,
            Self::Absent => None,
            Self::PresentUnenforced(v) => Some(v),
        }
    }
}

fn title(raw: Option<&str>) -> TitleFact<'_> {
    match raw {
        None => TitleFact::Missing,
        Some("") => TitleFact::Empty,
        Some(v) => TitleFact::Named(v),
    }
}
fn selection(alternate: AlternateFact, title: &TitleFact<'_>) -> SetSelectionFact {
    if alternate == AlternateFact::Present {
        SetSelectionFact::AlternateSelectionUnsupported(match title {
            TitleFact::Missing => AlternateTitleFact::Missing,
            TitleFact::Empty => AlternateTitleFact::Empty,
            TitleFact::Named(_) => AlternateTitleFact::Named,
        })
    } else if matches!(title, TitleFact::Named(_)) {
        SetSelectionFact::NamedSelectionUnsupported
    } else {
        SetSelectionFact::NoNamedOrAlternateInput
    }
}
fn media(raw: Option<&str>, stylesheet: bool) -> MediaFact<'_> {
    if !stylesheet {
        MediaFact::NotStylesheetRelationship(raw)
    } else if css::StylesheetConditionInput::from_optional_raw_media(raw).is_deferred() {
        MediaFact::DeferredUnsupported(raw.expect("CSS deferral requires raw media"))
    } else {
        MediaFact::ActiveSubset(raw)
    }
}
fn policy(raw: Option<&str>, stylesheet: bool) -> PolicyFact<'_> {
    if !stylesheet {
        PolicyFact::NotAssessedForOtherRelationship(raw)
    } else {
        match raw {
            None => PolicyFact::Absent,
            Some(v) => PolicyFact::PresentUnenforced(v),
        }
    }
}
fn resolution_outcome(result: Result<UrlParseOutcome, UrlFailure>) -> StylesheetHrefOutcome {
    match result {
        Ok(parsed) => StylesheetHrefOutcome::Resolved {
            address: ResourceAddress::new(parsed.url, ResourceKind::Stylesheet),
            diagnostics: parsed.diagnostics,
        },
        Err(f) => StylesheetHrefOutcome::Failed(f),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StylesheetDiscoveryLimits {
    pub(crate) nodes: usize,
    pub(crate) depth: usize,
    pub(crate) occurrences: usize,
    pub(crate) attributes: usize,
    pub(crate) classification_bytes: usize,
    pub(crate) resolution_pair_bytes: usize,
    pub(crate) resolution_total_bytes: usize,
}
impl Default for StylesheetDiscoveryLimits {
    fn default() -> Self {
        Self {
            nodes: 65_537,
            depth: 1_024,
            occurrences: 8_192,
            attributes: 262_144,
            classification_bytes: 10 * 1024 * 1024,
            resolution_pair_bytes: 64 * 1024,
            resolution_total_bytes: 10 * 1024 * 1024,
        }
    }
}
struct Budget<'a> {
    limits: &'a StylesheetDiscoveryLimits,
    nodes: usize,
    attributes: usize,
    classification: usize,
    resolutions: usize,
}
fn admit(
    current: usize,
    additional: usize,
    maximum: usize,
    dimension: StylesheetDiscoveryLimit,
) -> Result<usize, StylesheetDiscoveryError> {
    let next = current
        .checked_add(additional)
        .ok_or(StylesheetDiscoveryError::ArithmeticOverflow(dimension))?;
    if next > maximum {
        Err(StylesheetDiscoveryError::LimitExceeded { dimension, maximum })
    } else {
        Ok(next)
    }
}
impl Budget<'_> {
    fn visit(&mut self, depth: usize) -> Result<(), StylesheetDiscoveryError> {
        admit(
            0,
            depth,
            self.limits.depth,
            StylesheetDiscoveryLimit::OrdinaryDepth,
        )?;
        self.nodes = admit(
            self.nodes,
            1,
            self.limits.nodes,
            StylesheetDiscoveryLimit::OrdinaryNodes,
        )?;
        #[cfg(test)]
        work(|w| {
            w.nodes = self.nodes;
            w.depth = w.depth.max(depth);
        });
        Ok(())
    }
    fn attributes(&mut self, n: usize) -> Result<(), StylesheetDiscoveryError> {
        self.attributes = admit(
            self.attributes,
            n,
            self.limits.attributes,
            StylesheetDiscoveryLimit::AttributeEntries,
        )?;
        #[cfg(test)]
        work(|w| w.attributes = self.attributes);
        Ok(())
    }
    fn classification(&mut self, raw: Option<&str>) -> Result<(), StylesheetDiscoveryError> {
        self.classification = admit(
            self.classification,
            raw.map_or(0, str::len),
            self.limits.classification_bytes,
            StylesheetDiscoveryLimit::ClassificationInputBytes,
        )?;
        #[cfg(test)]
        work(|w| w.classification_bytes = self.classification);
        Ok(())
    }
    fn resolution(
        &mut self,
        href: &str,
        base: &BrowserUrl,
    ) -> Result<(), StylesheetDiscoveryError> {
        let pair = admit(
            href.len(),
            base.as_str().len(),
            self.limits.resolution_pair_bytes,
            StylesheetDiscoveryLimit::ResolutionPairInputBytes,
        )?;
        self.resolutions = admit(
            self.resolutions,
            pair,
            self.limits.resolution_total_bytes,
            StylesheetDiscoveryLimit::TotalResolutionInputBytes,
        )?;
        #[cfg(test)]
        work(|w| {
            w.resolution_calls += 1;
            w.resolution_bytes = self.resolutions;
        });
        Ok(())
    }
}
fn reserve_bounded<T>(
    values: &mut Vec<T>,
    additional: usize,
    maximum: usize,
    storage: StylesheetDiscoveryStorage,
) -> Result<(), StylesheetDiscoveryError> {
    let dimension = match storage {
        StylesheetDiscoveryStorage::TraversalFrames => StylesheetDiscoveryLimit::OrdinaryDepth,
        StylesheetDiscoveryStorage::Occurrences => StylesheetDiscoveryLimit::Occurrences,
    };
    let needed = admit(values.len(), additional, maximum, dimension)?;
    if needed <= values.capacity() {
        return Ok(());
    }
    let doubled = values
        .capacity()
        .checked_mul(2)
        .ok_or(StylesheetDiscoveryError::ArithmeticOverflow(dimension))?;
    let target = needed.max(doubled.max(16)).min(maximum);
    values
        .try_reserve_exact(target - values.len())
        .map_err(|_| StylesheetDiscoveryError::ReservationFailed(storage))?;
    #[cfg(test)]
    work(|w| match storage {
        StylesheetDiscoveryStorage::TraversalFrames => {
            w.frame_growths += 1;
            w.frame_capacity = w.frame_capacity.max(values.capacity());
        }
        StylesheetDiscoveryStorage::Occurrences => {
            w.record_growths += 1;
            w.record_capacity = w.record_capacity.max(values.capacity());
        }
    });
    Ok(())
}
struct Frame<'dom> {
    children: std::slice::Iter<'dom, Node>,
    depth: usize,
}

/// Caller supplies a coherently committed document/base pair. No lifecycle inference.
pub(crate) fn discover_stylesheets<'dom>(
    document: &'dom Node,
    active_base: &'dom BrowserUrl,
    limits: &StylesheetDiscoveryLimits,
) -> Result<StylesheetDiscovery<'dom>, StylesheetDiscoveryError> {
    #[cfg(test)]
    WORK.with(|w| w.set(DiscoveryWork::default()));
    let mut budget = Budget {
        limits,
        nodes: 0,
        attributes: 0,
        classification: 0,
        resolutions: 0,
    };
    let mut records = Vec::new();
    let mut frames: Vec<Frame<'dom>> = Vec::new();
    let mut current = Some((document, 0));
    let mut dependency_count = 0usize;
    while let Some((node, depth)) = current.take() {
        budget.visit(depth)?;
        if let Node::Element { element } = node {
            collect_element(
                element,
                active_base,
                &mut budget,
                &mut records,
                &mut dependency_count,
            )?;
        }
        let children = node.children().unwrap_or(&[]);
        if !children.is_empty() {
            let maximum =
                limits
                    .depth
                    .checked_add(1)
                    .ok_or(StylesheetDiscoveryError::ArithmeticOverflow(
                        StylesheetDiscoveryLimit::OrdinaryDepth,
                    ))?;
            reserve_bounded(
                &mut frames,
                1,
                maximum,
                StylesheetDiscoveryStorage::TraversalFrames,
            )?;
            frames.push(Frame {
                children: children.iter(),
                depth,
            });
            #[cfg(test)]
            work(|w| w.frames = w.frames.max(frames.len()));
        }
        while let Some(frame) = frames.last_mut() {
            if let Some(child) = frame.children.next() {
                let depth = frame.depth.checked_add(1).ok_or(
                    StylesheetDiscoveryError::ArithmeticOverflow(
                        StylesheetDiscoveryLimit::OrdinaryDepth,
                    ),
                )?;
                current = Some((child, depth));
                break;
            }
            frames.pop();
        }
    }
    Ok(StylesheetDiscovery { records })
}
#[derive(Default)]
struct Attributes<'dom> {
    rel: Option<&'dom str>,
    href: Option<&'dom str>,
    type_: Option<&'dom str>,
    title: Option<&'dom str>,
    disabled: Option<&'dom str>,
    media: Option<&'dom str>,
    crossorigin: Option<&'dom str>,
    integrity: Option<&'dom str>,
    referrerpolicy: Option<&'dom str>,
}
impl<'dom> Attributes<'dom> {
    fn from_element(element: &'dom ElementNode) -> Self {
        let mut facts = Self::default();
        for a in element.attributes() {
            #[cfg(test)]
            work(|w| w.attribute_examinations += 1);
            if a.namespace() != AttributeNamespace::None {
                continue;
            }
            let target = match a.local_name() {
                "rel" => &mut facts.rel,
                "href" => &mut facts.href,
                "type" => &mut facts.type_,
                "title" => &mut facts.title,
                "disabled" => &mut facts.disabled,
                "media" => &mut facts.media,
                "crossorigin" => &mut facts.crossorigin,
                "integrity" => &mut facts.integrity,
                "referrerpolicy" => &mut facts.referrerpolicy,
                _ => continue,
            };
            if target.is_none() {
                *target = Some(a.value());
            }
        }
        facts
    }
}
fn collect_element<'dom>(
    element: &'dom ElementNode,
    base: &BrowserUrl,
    budget: &mut Budget<'_>,
    records: &mut Vec<StylesheetOccurrence<'dom>>,
    dependency_count: &mut usize,
) -> Result<(), StylesheetDiscoveryError> {
    budget.attributes(element.attributes().len())?;
    let html = element.namespace() == ElementNamespace::Html;
    let is_link = html && element.name() == "link";
    let is_block = html && element.name() == "style";
    let raw = if is_link || is_block {
        Attributes::from_element(element)
    } else {
        Attributes::default()
    };
    let attrs = element.attributes().iter().map(|a| {
        #[cfg(test)]
        work(|w| w.attribute_examinations += 1);
        css::SelectorDomAttribute::new(a.namespace(), a.local_name(), a.value())
    });
    let attribute = css::get_inline_style(element.namespace(), attrs);
    if is_link {
        budget.classification(raw.rel)?;
    }
    let relation = RelationFacts::classify(if is_link { raw.rel } else { None });
    let stylesheet = is_link && relation.stylesheet_token;
    if is_block {
        budget.classification(raw.type_)?;
    }
    if stylesheet || is_block {
        budget.classification(raw.media)?;
    }
    let additional = usize::from(is_link || is_block) + usize::from(attribute.is_some());
    admit(
        records.len(),
        additional,
        budget.limits.occurrences,
        StylesheetDiscoveryLimit::Occurrences,
    )?;
    if additional == 0 {
        return Ok(());
    }
    if element.id() == Id::INVALID {
        return Err(StylesheetDiscoveryError::SourceIdentityUnavailable);
    }
    // Validate both index domains and storage before delegating any URL work.
    OccurrenceIndex::from_count(records.len() + additional - 1)?;
    let dependency = if stylesheet || is_block {
        Some(StylesheetDependencyOrder::from_count(*dependency_count)?)
    } else {
        None
    };
    reserve_bounded(
        records,
        additional,
        budget.limits.occurrences,
        StylesheetDiscoveryStorage::Occurrences,
    )?;
    let source = if is_link {
        let outcome = if !stylesheet {
            StylesheetHrefOutcome::NotStylesheetRelationship
        } else {
            match raw.href {
                None => StylesheetHrefOutcome::Missing,
                Some("") => StylesheetHrefOutcome::Empty,
                Some(href) => {
                    budget.resolution(href, base)?;
                    resolution_outcome(base.resolve(href))
                }
            }
        };
        Some(StylesheetSource::Link(LinkDiscovery {
            relation,
            href: HrefObservation {
                authored: raw.href,
                outcome,
            },
            type_hint: match raw.type_ {
                None => LinkTypeFact::Missing,
                Some(v) => LinkTypeFact::AdvisoryUnassessed(v),
            },
            title: title(raw.title),
            disabled: if !stylesheet {
                DisabledFact::NotStylesheetRelationship(raw.disabled)
            } else {
                match raw.disabled {
                    None => DisabledFact::Absent,
                    Some(v) => DisabledFact::Present(v),
                }
            },
            media: media(raw.media, stylesheet),
            policies: LinkPolicyFacts {
                crossorigin: policy(raw.crossorigin, stylesheet),
                integrity: policy(raw.integrity, stylesheet),
                referrerpolicy: policy(raw.referrerpolicy, stylesheet),
            },
        }))
    } else if is_block {
        Some(StylesheetSource::StyleBlock(StyleBlockDiscovery {
            content: InlineStyleText {
                children: element.children(),
            },
            type_fact: match raw.type_ {
                None => StyleTypeFact::Missing,
                Some("") => StyleTypeFact::Empty,
                Some(v) if v.eq_ignore_ascii_case("text/css") => StyleTypeFact::Css(v),
                Some(v) => StyleTypeFact::Other(v),
            },
            title: title(raw.title),
            media: media(raw.media, true),
            inapplicable_link_attributes: StyleBlockLinkAttributes {
                disabled: raw.disabled,
                crossorigin: raw.crossorigin,
                integrity: raw.integrity,
                referrerpolicy: raw.referrerpolicy,
            },
        }))
    } else {
        None
    };
    if let Some(source) = source {
        records.push(StylesheetOccurrence {
            occurrence: OccurrenceIndex::from_count(records.len())?,
            dependency_order: dependency,
            owner: element.id(),
            source,
        });
        if dependency.is_some() {
            *dependency_count =
                dependency_count
                    .checked_add(1)
                    .ok_or(StylesheetDiscoveryError::IndexOverflow(
                        StylesheetIndexDomain::Dependency,
                    ))?;
        }
    }
    if let Some(value) = attribute {
        records.push(StylesheetOccurrence {
            occurrence: OccurrenceIndex::from_count(records.len())?,
            dependency_order: None,
            owner: element.id(),
            source: StylesheetSource::StyleAttribute { value },
        });
    }
    #[cfg(test)]
    work(|w| w.records = records.len());
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct DiscoveryWork {
    pub nodes: usize,
    pub depth: usize,
    pub attributes: usize,
    pub attribute_examinations: usize,
    pub classification_bytes: usize,
    pub resolution_calls: usize,
    pub resolution_bytes: usize,
    pub frames: usize,
    pub frame_capacity: usize,
    pub record_capacity: usize,
    pub frame_growths: usize,
    pub record_growths: usize,
    pub records: usize,
}
#[cfg(test)]
thread_local! { pub(crate) static WORK: std::cell::Cell<DiscoveryWork> = const { std::cell::Cell::new(DiscoveryWork { nodes:0, depth:0, attributes:0, attribute_examinations:0, classification_bytes:0, resolution_calls:0, resolution_bytes:0, frames:0, frame_capacity:0, record_capacity:0, frame_growths:0, record_growths:0, records:0 }) }; }
#[cfg(test)]
fn work(f: impl FnOnce(&mut DiscoveryWork)) {
    WORK.with(|w| {
        let mut value = w.get();
        f(&mut value);
        w.set(value);
    });
}
