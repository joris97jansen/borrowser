use std::collections::{HashMap, HashSet};

use crate::ElementNamespace;
use crate::attributes::parser_created_attribute_lists_equal_ordered;
use crate::conformance::ObservationReservationSite;
use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomTable;
use crate::html5::tokenizer::{TextModeSpec, TokenizerControl};
use crate::html5::tree_builder::formatting::{AfeEntry, AfeMarkerKind};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ExpandedNameKey;
use crate::html5::tree_builder::{DomInvariantState, Html5TreeBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreeBuilderFinalAuditAllocation {
    OpenElementsIndex,
    ActiveFormattingIndex,
    TemplateCoordinationIndex,
    DomStructuralTraversal,
    LiveTreeStructuralProjection,
}

pub(crate) struct TreeBuilderFinalAudit {
    pub(crate) pending_table_text_empty: bool,
    pub(crate) insertion_mode_valid: bool,
    pub(crate) open_elements_consistent: bool,
    pub(crate) active_formatting_consistent: bool,
    pub(crate) template_modes_consistent: bool,
    pub(crate) form_pointer_valid: bool,
    pub(crate) parent_child_links_valid: bool,
    pub(crate) namespaces_valid: bool,
    pub(crate) template_associations_valid: bool,
    pub(crate) live_structure: DomInvariantState,
    pub(crate) active_text_mode: Option<TextModeSpec>,
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    pub(crate) pending_tokenizer_control: Option<TokenizerControl>,
    pub(crate) insertion_mode: InsertionMode,
}

impl Html5TreeBuilder {
    pub(crate) fn final_audit_for_conformance(
        &self,
        atoms: &AtomTable,
        reserve: &mut impl FnMut(ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<TreeBuilderFinalAudit, TreeBuilderFinalAuditAllocation> {
        let open_elements_consistent = self.audit_open_elements(atoms, reserve)?;
        let active_formatting_consistent = self.audit_active_formatting(atoms, reserve)?;
        let template_modes_consistent = self.audit_template_coordination(reserve)?;
        let live = self
            .live_tree
            .try_final_audit(reserve)
            .map_err(|_| TreeBuilderFinalAuditAllocation::DomStructuralTraversal)?;
        let live_structure = self
            .live_tree
            .try_invariant_state_for_final_audit(reserve)
            .map_err(|_| TreeBuilderFinalAuditAllocation::LiveTreeStructuralProjection)?;
        let insertion_mode_valid = self.original_insertion_mode.is_none()
            && self.active_text_mode.is_none()
            && self.pending_tokenizer_control.is_none()
            && !matches!(
                self.insertion_mode,
                InsertionMode::Text | InsertionMode::InTableText
            );
        let form_pointer_valid = self.form_element_pointer.is_none_or(|pointer| {
            self.live_tree
                .element_semantics_for_final_audit(pointer.key())
                .is_some_and(|(name, _)| name.is(ElementNamespace::Html, "form"))
        });
        Ok(TreeBuilderFinalAudit {
            pending_table_text_empty: self.pending_table_text.is_none(),
            insertion_mode_valid,
            open_elements_consistent,
            active_formatting_consistent,
            template_modes_consistent,
            form_pointer_valid,
            parent_child_links_valid: live.parent_child_links_valid,
            namespaces_valid: live.namespaces_valid,
            template_associations_valid: live.template_associations_valid,
            live_structure,
            active_text_mode: self.active_text_mode,
            original_insertion_mode: self.original_insertion_mode,
            pending_tokenizer_control: self.pending_tokenizer_control,
            insertion_mode: self.insertion_mode,
        })
    }

    fn audit_open_elements(
        &self,
        atoms: &AtomTable,
        reserve: &mut impl FnMut(ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<bool, TreeBuilderFinalAuditAllocation> {
        let mut keys = HashSet::new();
        reserve(ObservationReservationSite::FinalAuditOpenElementsIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::OpenElementsIndex)?;
        keys.try_reserve(self.open_elements.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::OpenElementsIndex)?;
        let mut counts = HashMap::<ExpandedNameKey, usize>::new();
        reserve(ObservationReservationSite::FinalAuditOpenElementsIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::OpenElementsIndex)?;
        counts
            .try_reserve(self.open_elements.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::OpenElementsIndex)?;
        let mut valid = true;
        for entry in self.open_elements.iter_entries() {
            if entry.key() == PatchKey::INVALID || !keys.insert(entry.key()) {
                valid = false;
            }
            let Some(atom_name) = atoms.resolve(entry.name()) else {
                valid = false;
                continue;
            };
            if u64::from(entry.name().interner_id()) != self.atom_table_id {
                valid = false;
            }
            if !self
                .live_tree
                .element_semantics_for_final_audit(entry.key())
                .is_some_and(|(name, _)| {
                    name.namespace() == entry.namespace() && name.local_name().as_str() == atom_name
                })
            {
                valid = false;
            }
            let count = counts.entry(entry.expanded_name_key()).or_insert(0);
            let Some(next) = count.checked_add(1) else {
                valid = false;
                continue;
            };
            *count = next;
        }
        if counts.len() != self.open_elements.cached_name_counts().len() {
            valid = false;
        }
        for (name, expected) in self.open_elements.cached_name_counts() {
            if counts.get(name) != Some(expected) {
                valid = false;
            }
        }
        Ok(valid)
    }

    fn audit_active_formatting(
        &self,
        atoms: &AtomTable,
        reserve: &mut impl FnMut(ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<bool, TreeBuilderFinalAuditAllocation> {
        let mut keys = HashSet::new();
        reserve(ObservationReservationSite::FinalAuditActiveFormattingIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::ActiveFormattingIndex)?;
        keys.try_reserve(self.active_formatting.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::ActiveFormattingIndex)?;
        let mut valid = true;
        for entry in self.active_formatting.entries() {
            match entry {
                AfeEntry::Element(element) => {
                    if element.key == PatchKey::INVALID || !keys.insert(element.key) {
                        valid = false;
                    }
                    let Some(atom_name) = atoms.resolve(element.name) else {
                        valid = false;
                        continue;
                    };
                    if !self
                        .live_tree
                        .element_semantics_for_final_audit(element.key)
                        .is_some_and(|(name, attrs)| {
                            name.namespace() == ElementNamespace::Html
                                && name.local_name().as_str() == atom_name
                                && parser_created_attribute_lists_equal_ordered(
                                    &element.attrs,
                                    attrs,
                                )
                        })
                    {
                        valid = false;
                    }
                }
                AfeEntry::Marker(marker) => {
                    let owner = marker.owner;
                    let Some(owner) = owner else {
                        valid = false;
                        continue;
                    };
                    let Some((name, _)) = self.live_tree.element_semantics_for_final_audit(owner)
                    else {
                        valid = false;
                        continue;
                    };
                    let owner_valid = match marker.kind {
                        AfeMarkerKind::FormattingBoundary => {
                            name.namespace() == ElementNamespace::Html
                                && matches!(
                                    name.local_name().as_str(),
                                    "applet" | "marquee" | "object"
                                )
                        }
                        AfeMarkerKind::Caption => name.is(ElementNamespace::Html, "caption"),
                        AfeMarkerKind::TableCell => {
                            name.is(ElementNamespace::Html, "td")
                                || name.is(ElementNamespace::Html, "th")
                        }
                        AfeMarkerKind::Template => {
                            name.is(ElementNamespace::Html, "template")
                                && self
                                    .live_tree
                                    .template_state_for_final_audit(owner)
                                    .is_some_and(|(is_template, contents, child_count)| {
                                        is_template && contents.is_some() && child_count == 0
                                    })
                        }
                    };
                    if !owner_valid {
                        valid = false;
                    }
                }
            }
        }
        Ok(valid)
    }

    fn audit_template_coordination(
        &self,
        reserve: &mut impl FnMut(ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<bool, TreeBuilderFinalAuditAllocation> {
        let mut open_templates = HashMap::<PatchKey, usize>::new();
        reserve(ObservationReservationSite::FinalAuditTemplateCoordinationIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        open_templates
            .try_reserve(self.open_elements.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        let mut open_template_order = Vec::new();
        reserve(ObservationReservationSite::FinalAuditTemplateCoordinationIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        open_template_order
            .try_reserve(self.open_elements.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        let mut depth = 0usize;
        let mut valid = true;
        for entry in self.open_elements.iter_entries() {
            if entry.namespace() == ElementNamespace::Html
                && entry.name() == self.known_tags.template
            {
                if open_templates.insert(entry.key(), depth).is_some() {
                    valid = false;
                }
                open_template_order.push(entry.key());
                depth = match depth.checked_add(1) {
                    Some(next) => next,
                    None => {
                        valid = false;
                        depth
                    }
                };
            }
        }
        if depth != self.template_modes.len() {
            valid = false;
        }
        for (index, mode) in self.template_modes.entries().iter().enumerate() {
            let template_state = self.live_tree.template_state_for_final_audit(mode.owner());
            if open_templates.get(&mode.owner()) != Some(&index)
                || !template_state.is_some_and(|(is_template, contents, child_count)| {
                    is_template && contents.is_some() && child_count == 0
                })
            {
                valid = false;
            }
        }
        let mut marker_owners = HashSet::new();
        reserve(ObservationReservationSite::FinalAuditTemplateCoordinationIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        marker_owners
            .try_reserve(self.active_formatting.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        let mut open_marker_order = Vec::new();
        reserve(ObservationReservationSite::FinalAuditTemplateCoordinationIndex)
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        open_marker_order
            .try_reserve(self.active_formatting.len())
            .map_err(|_| TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex)?;
        for entry in self.active_formatting.entries() {
            if let AfeEntry::Marker(marker) = entry
                && marker.kind == AfeMarkerKind::Template
            {
                let Some(owner) = marker.owner else {
                    valid = false;
                    continue;
                };
                if !marker_owners.insert(owner) {
                    valid = false;
                }
                if open_templates.contains_key(&owner) {
                    open_marker_order.push(owner);
                } else if !self
                    .live_tree
                    .template_state_for_final_audit(owner)
                    .is_some_and(|(is_template, contents, _)| is_template && contents.is_some())
                {
                    // EOF recovery deliberately permits diagnostic markers for
                    // already-closed template hosts. Their typed owner must
                    // still name a live template association.
                    valid = false;
                }
            }
        }
        if open_marker_order.as_slice() != open_template_order.as_slice() {
            valid = false;
        }
        Ok(valid)
    }
}
