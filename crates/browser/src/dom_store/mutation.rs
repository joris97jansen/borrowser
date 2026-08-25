use core_types::DomHandle;
use html::{ElementNamespace, ParserCreatedAttribute, PatchKey, internal::Id};

#[cfg(test)]
use std::cell::Cell;

use super::{
    arena::{DomArena, NodeKind, NodeRecord},
    document::DomDoc,
    store::DomStore,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DomMutationSnapshotLimits {
    pub(crate) max_exact_targets_per_dimension: usize,
    pub(crate) max_attribute_entries_per_publication: usize,
    pub(crate) max_attribute_bytes_per_publication: usize,
    pub(crate) max_text_bytes_per_publication: usize,
}

impl Default for DomMutationSnapshotLimits {
    fn default() -> Self {
        Self {
            max_exact_targets_per_dimension: 4_096,
            max_attribute_entries_per_publication: 65_536,
            max_attribute_bytes_per_publication: 16 * 1024 * 1024,
            max_text_bytes_per_publication: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DomMutationSnapshotLimit {
    ExactTargetsPerDimension,
    AttributeEntriesPerPublication,
    AttributeBytesPerPublication,
    TextBytesPerPublication,
}

impl DomMutationSnapshotLimit {
    pub(crate) fn stable_label(self) -> &'static str {
        match self {
            Self::ExactTargetsPerDimension => "exact-targets-per-dimension",
            Self::AttributeEntriesPerPublication => "attribute-entries-per-publication",
            Self::AttributeBytesPerPublication => "attribute-bytes-per-publication",
            Self::TextBytesPerPublication => "text-bytes-per-publication",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DomMutationSnapshotStorage {
    CanonicalKeys,
    AttributeMutations,
    AttributeList,
    AttributeValue,
    TextMutations,
    TextValue,
}

impl DomMutationSnapshotStorage {
    pub(crate) fn stable_label(self) -> &'static str {
        match self {
            Self::CanonicalKeys => "canonical-keys",
            Self::AttributeMutations => "attribute-mutations",
            Self::AttributeList => "attribute-list",
            Self::AttributeValue => "attribute-value",
            Self::TextMutations => "text-mutations",
            Self::TextValue => "text-value",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DomMutationPrecisionFailure {
    DocumentIdentityChanged,
    LimitExceeded {
        limit: DomMutationSnapshotLimit,
        configured: usize,
        observed: usize,
    },
    CounterExhausted {
        counter: &'static str,
    },
    Reservation {
        storage: DomMutationSnapshotStorage,
    },
}

impl DomMutationPrecisionFailure {
    pub(crate) fn stable_label(&self) -> &'static str {
        match self {
            Self::DocumentIdentityChanged => "document-identity-changed",
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::CounterExhausted { .. } => "counter-exhausted",
            Self::Reservation { .. } => "reservation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactDomMutationDetails<T> {
    Complete(Vec<T>),
    ConservativeUnavailable(DomMutationPrecisionFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactStoreAttributeMutation {
    pub(crate) node_id: Id,
    pub(crate) element_namespace: ElementNamespace,
    pub(crate) before: Option<Vec<ParserCreatedAttribute>>,
    pub(crate) after: Vec<ParserCreatedAttribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactStoreTextMutation {
    pub(crate) node_id: Id,
    pub(crate) parent_element: Option<(Id, ElementNamespace)>,
    pub(crate) before: Option<String>,
    pub(crate) after: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DomMutationSnapshotInvariantError {
    UnknownHandle(DomHandle),
    TargetNeverAllocated(PatchKey),
    LiveIdentityUnavailable(PatchKey),
    AttributeTargetNotElement(PatchKey),
    TextTargetNotText(PatchKey),
    HistoricalAttributeKindChanged(PatchKey),
    HistoricalTextKindChanged(PatchKey),
    ParentMissing(PatchKey),
}

impl DomStore {
    pub(crate) fn capture_exact_attribute_mutations(
        &self,
        committed: Option<&Self>,
        handle: DomHandle,
        keys: &[PatchKey],
        limits: &DomMutationSnapshotLimits,
    ) -> Result<
        ExactDomMutationDetails<ExactStoreAttributeMutation>,
        DomMutationSnapshotInvariantError,
    > {
        if keys.is_empty() {
            return Ok(ExactDomMutationDetails::Complete(Vec::new()));
        }
        let Some(committed) = committed else {
            return Ok(ExactDomMutationDetails::ConservativeUnavailable(
                DomMutationPrecisionFailure::DocumentIdentityChanged,
            ));
        };
        let staged_doc = self
            .docs
            .get(&handle)
            .ok_or(DomMutationSnapshotInvariantError::UnknownHandle(handle))?;
        let committed_doc = committed
            .docs
            .get(&handle)
            .ok_or(DomMutationSnapshotInvariantError::UnknownHandle(handle))?;
        capture_attribute_mutations(staged_doc, committed_doc, keys, limits)
    }

    pub(crate) fn capture_exact_text_mutations(
        &self,
        committed: Option<&Self>,
        handle: DomHandle,
        keys: &[PatchKey],
        limits: &DomMutationSnapshotLimits,
    ) -> Result<ExactDomMutationDetails<ExactStoreTextMutation>, DomMutationSnapshotInvariantError>
    {
        if keys.is_empty() {
            return Ok(ExactDomMutationDetails::Complete(Vec::new()));
        }
        let Some(committed) = committed else {
            return Ok(ExactDomMutationDetails::ConservativeUnavailable(
                DomMutationPrecisionFailure::DocumentIdentityChanged,
            ));
        };
        let staged_doc = self
            .docs
            .get(&handle)
            .ok_or(DomMutationSnapshotInvariantError::UnknownHandle(handle))?;
        let committed_doc = committed
            .docs
            .get(&handle)
            .ok_or(DomMutationSnapshotInvariantError::UnknownHandle(handle))?;
        capture_text_mutations(staged_doc, committed_doc, keys, limits)
    }
}

fn capture_attribute_mutations(
    staged: &DomDoc,
    committed: &DomDoc,
    keys: &[PatchKey],
    limits: &DomMutationSnapshotLimits,
) -> Result<ExactDomMutationDetails<ExactStoreAttributeMutation>, DomMutationSnapshotInvariantError>
{
    let keys = match canonical_keys(keys, limits) {
        Ok(keys) => keys,
        Err(failure) => return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure)),
    };
    let mut mutations = Vec::new();
    if mutations.try_reserve_exact(keys.len()).is_err() {
        return Ok(ExactDomMutationDetails::ConservativeUnavailable(
            DomMutationPrecisionFailure::Reservation {
                storage: DomMutationSnapshotStorage::AttributeMutations,
            },
        ));
    }
    let mut entry_count = 0usize;
    let mut byte_count = 0usize;
    for key in keys {
        ensure_allocated(&staged.arena, key)?;
        let Some(current) = live_record(&staged.arena, key) else {
            continue;
        };
        let NodeKind::Element {
            name, attributes, ..
        } = &current.kind
        else {
            return Err(DomMutationSnapshotInvariantError::AttributeTargetNotElement(key));
        };
        let node_id = materialized_id(&staged.arena, key)?;
        let after =
            match copy_attributes_bounded(attributes, &mut entry_count, &mut byte_count, limits) {
                Ok(value) => value,
                Err(failure) => {
                    return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure));
                }
            };
        let before = match live_record(&committed.arena, key) {
            Some(NodeRecord {
                kind: NodeKind::Element { attributes, .. },
                ..
            }) => {
                match copy_attributes_bounded(attributes, &mut entry_count, &mut byte_count, limits)
                {
                    Ok(value) => Some(value),
                    Err(failure) => {
                        return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure));
                    }
                }
            }
            Some(_) => {
                return Err(DomMutationSnapshotInvariantError::HistoricalAttributeKindChanged(key));
            }
            None => None,
        };
        mutations.push(ExactStoreAttributeMutation {
            node_id,
            element_namespace: name.namespace(),
            before,
            after,
        });
    }
    Ok(ExactDomMutationDetails::Complete(mutations))
}

fn capture_text_mutations(
    staged: &DomDoc,
    committed: &DomDoc,
    keys: &[PatchKey],
    limits: &DomMutationSnapshotLimits,
) -> Result<ExactDomMutationDetails<ExactStoreTextMutation>, DomMutationSnapshotInvariantError> {
    let keys = match canonical_keys(keys, limits) {
        Ok(keys) => keys,
        Err(failure) => return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure)),
    };
    let mut mutations = Vec::new();
    if mutations.try_reserve_exact(keys.len()).is_err() {
        return Ok(ExactDomMutationDetails::ConservativeUnavailable(
            DomMutationPrecisionFailure::Reservation {
                storage: DomMutationSnapshotStorage::TextMutations,
            },
        ));
    }
    let mut byte_count = 0usize;
    for key in keys {
        ensure_allocated(&staged.arena, key)?;
        let Some(current) = live_record(&staged.arena, key) else {
            continue;
        };
        let NodeKind::Text { text } = &current.kind else {
            return Err(DomMutationSnapshotInvariantError::TextTargetNotText(key));
        };
        let node_id = materialized_id(&staged.arena, key)?;
        let after = match copy_text_bounded(text, &mut byte_count, limits) {
            Ok(value) => value,
            Err(failure) => {
                return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure));
            }
        };
        let before = match live_record(&committed.arena, key) {
            Some(NodeRecord {
                kind: NodeKind::Text { text },
                ..
            }) => match copy_text_bounded(text, &mut byte_count, limits) {
                Ok(value) => Some(value),
                Err(failure) => {
                    return Ok(ExactDomMutationDetails::ConservativeUnavailable(failure));
                }
            },
            Some(_) => {
                return Err(DomMutationSnapshotInvariantError::HistoricalTextKindChanged(key));
            }
            None => None,
        };
        mutations.push(ExactStoreTextMutation {
            node_id,
            parent_element: direct_parent_element(&staged.arena, current.parent)?,
            before,
            after,
        });
    }
    Ok(ExactDomMutationDetails::Complete(mutations))
}

fn canonical_keys(
    keys: &[PatchKey],
    limits: &DomMutationSnapshotLimits,
) -> Result<Vec<PatchKey>, DomMutationPrecisionFailure> {
    if keys.len() > limits.max_exact_targets_per_dimension {
        return Err(DomMutationPrecisionFailure::LimitExceeded {
            limit: DomMutationSnapshotLimit::ExactTargetsPerDimension,
            configured: limits.max_exact_targets_per_dimension,
            observed: keys.len(),
        });
    }
    let mut canonical = Vec::new();
    canonical.try_reserve_exact(keys.len()).map_err(|_| {
        DomMutationPrecisionFailure::Reservation {
            storage: DomMutationSnapshotStorage::CanonicalKeys,
        }
    })?;
    canonical.extend_from_slice(keys);
    canonical.sort_unstable();
    canonical.dedup();
    Ok(canonical)
}

fn ensure_allocated(
    arena: &DomArena,
    key: PatchKey,
) -> Result<(), DomMutationSnapshotInvariantError> {
    if arena.is_allocated(key) {
        Ok(())
    } else {
        Err(DomMutationSnapshotInvariantError::TargetNeverAllocated(key))
    }
}

fn live_record(arena: &DomArena, key: PatchKey) -> Option<&NodeRecord> {
    #[cfg(test)]
    DIRECT_MUTATION_RECORD_LOOKUPS.with(|count| count.set(count.get() + 1));
    arena.live.get(&key).map(|index| &arena.nodes[*index])
}

#[cfg(test)]
thread_local! {
    static DIRECT_MUTATION_RECORD_LOOKUPS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_direct_mutation_record_lookups() {
    DIRECT_MUTATION_RECORD_LOOKUPS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn direct_mutation_record_lookups() -> usize {
    DIRECT_MUTATION_RECORD_LOOKUPS.with(Cell::get)
}

fn materialized_id(
    arena: &DomArena,
    key: PatchKey,
) -> Result<Id, DomMutationSnapshotInvariantError> {
    arena
        .materialized_node_id_for_key(key)
        .map_err(|_| DomMutationSnapshotInvariantError::LiveIdentityUnavailable(key))
}

fn direct_parent_element(
    arena: &DomArena,
    parent: Option<PatchKey>,
) -> Result<Option<(Id, ElementNamespace)>, DomMutationSnapshotInvariantError> {
    let Some(parent) = parent else {
        return Ok(None);
    };
    let record = live_record(arena, parent)
        .ok_or(DomMutationSnapshotInvariantError::ParentMissing(parent))?;
    let NodeKind::Element { name, .. } = &record.kind else {
        return Ok(None);
    };
    Ok(Some((materialized_id(arena, parent)?, name.namespace())))
}

fn copy_attributes_bounded(
    attributes: &[ParserCreatedAttribute],
    entry_count: &mut usize,
    byte_count: &mut usize,
    limits: &DomMutationSnapshotLimits,
) -> Result<Vec<ParserCreatedAttribute>, DomMutationPrecisionFailure> {
    let observed_entries = entry_count.checked_add(attributes.len()).ok_or(
        DomMutationPrecisionFailure::CounterExhausted {
            counter: "attribute-entry-count",
        },
    )?;
    if observed_entries > limits.max_attribute_entries_per_publication {
        return Err(DomMutationPrecisionFailure::LimitExceeded {
            limit: DomMutationSnapshotLimit::AttributeEntriesPerPublication,
            configured: limits.max_attribute_entries_per_publication,
            observed: observed_entries,
        });
    }
    let mut copy = Vec::new();
    copy.try_reserve_exact(attributes.len()).map_err(|_| {
        DomMutationPrecisionFailure::Reservation {
            storage: DomMutationSnapshotStorage::AttributeList,
        }
    })?;
    for attribute in attributes {
        let attribute_bytes = attribute
            .local_name()
            .len()
            .checked_add(attribute.value().len())
            .ok_or(DomMutationPrecisionFailure::CounterExhausted {
                counter: "attribute-byte-count",
            })?;
        let observed_bytes = byte_count.checked_add(attribute_bytes).ok_or(
            DomMutationPrecisionFailure::CounterExhausted {
                counter: "attribute-byte-count",
            },
        )?;
        if observed_bytes > limits.max_attribute_bytes_per_publication {
            return Err(DomMutationPrecisionFailure::LimitExceeded {
                limit: DomMutationSnapshotLimit::AttributeBytesPerPublication,
                configured: limits.max_attribute_bytes_per_publication,
                observed: observed_bytes,
            });
        }
        let mut value = String::new();
        value
            .try_reserve_exact(attribute.value().len())
            .map_err(|_| DomMutationPrecisionFailure::Reservation {
                storage: DomMutationSnapshotStorage::AttributeValue,
            })?;
        value.push_str(attribute.value());
        copy.push(ParserCreatedAttribute::new(attribute.name().clone(), value));
        *byte_count = observed_bytes;
    }
    *entry_count = observed_entries;
    Ok(copy)
}

fn copy_text_bounded(
    text: &str,
    byte_count: &mut usize,
    limits: &DomMutationSnapshotLimits,
) -> Result<String, DomMutationPrecisionFailure> {
    let observed = byte_count.checked_add(text.len()).ok_or(
        DomMutationPrecisionFailure::CounterExhausted {
            counter: "text-byte-count",
        },
    )?;
    if observed > limits.max_text_bytes_per_publication {
        return Err(DomMutationPrecisionFailure::LimitExceeded {
            limit: DomMutationSnapshotLimit::TextBytesPerPublication,
            configured: limits.max_text_bytes_per_publication,
            observed,
        });
    }
    let mut copy = String::new();
    copy.try_reserve_exact(text.len())
        .map_err(|_| DomMutationPrecisionFailure::Reservation {
            storage: DomMutationSnapshotStorage::TextValue,
        })?;
    copy.push_str(text);
    *byte_count = observed;
    Ok(copy)
}
