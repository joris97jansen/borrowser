use super::{
    Tab,
    state::{DocumentLoadFailure, DocumentParseStatus},
};
use crate::dom_store::{
    DomIdentityResolutionError, DomMutationSnapshotInvariantError, DomMutationSnapshotLimits,
    DomPatchError,
};
use crate::page::{DomMutationFacts, PendingDomMutationFacts};
use bus::{CoreEvent, DocumentPublication, DocumentPublicationFailure, DocumentPublicationPayload};
use core_types::ResourceKind;

enum DocumentCommitFailure {
    Publication(DocumentPublicationFailure),
    StylesheetInventory(crate::stylesheet_fetch::StylesheetInventoryError),
    StylesheetParserIntegrity {
        owner: html::internal::Id,
        slot: core_types::StylesheetSlotId,
    },
}
impl From<DocumentPublicationFailure> for DocumentCommitFailure {
    fn from(e: DocumentPublicationFailure) -> Self {
        Self::Publication(e)
    }
}
impl DocumentCommitFailure {
    #[cfg(test)]
    fn public_category(self) -> DocumentPublicationFailure {
        match self {
            Self::Publication(e) => e,
            Self::StylesheetInventory(_) => DocumentPublicationFailure::ResourceUnavailable,
            Self::StylesheetParserIntegrity { .. } => {
                DocumentPublicationFailure::InvariantViolation
            }
        }
    }
}

impl Tab {
    /// Apply an event, returning Browser publication/commit failures.
    /// Parser execution failures remain owned by `HtmlParseFinished`.
    pub fn on_core_event(&mut self, evt: CoreEvent) -> Result<(), DocumentPublicationFailure> {
        match evt {
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                response,
            } if self.is_current(tab_id, request_id)
                || self.is_navigation_request(tab_id, request_id) =>
            {
                self.on_html_network_start(response, request_id);
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                url,
                bytes,
                ..
            } if self.is_current(tab_id, request_id)
                || self.is_navigation_request(tab_id, request_id) =>
            {
                self.on_html_network_chunk(&url, bytes, request_id);
            }

            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                response,
                body,
            } if self.is_current(tab_id, request_id)
                || self.is_navigation_request(tab_id, request_id) =>
            {
                self.on_html_network_done(response, body, request_id);
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                url: _,
                error_kind,
                response,
                body,
                error: _,
            } if self.is_current(tab_id, request_id)
                || self.is_navigation_request(tab_id, request_id) =>
            {
                self.on_html_network_error(error_kind, response, body, request_id);
            }

            CoreEvent::DomPatchUpdate {
                tab_id,
                request_id,
                publication,
            } if self.is_current(tab_id, request_id) => {
                if self.document_load.parse_status != DocumentParseStatus::Pending {
                    let failure = DocumentPublicationFailure::InvariantViolation;
                    self.on_document_publication_failure(failure.clone());
                    return Err(failure);
                }
                match self.commit_document_publication(publication, request_id) {
                    Ok(()) => {}
                    Err(DocumentCommitFailure::Publication(error)) => {
                        self.on_document_publication_failure(error.clone());
                        return Err(error);
                    }
                    Err(DocumentCommitFailure::StylesheetParserIntegrity { owner, slot }) => {
                        self.fail_document(DocumentLoadFailure::StylesheetParserInvariant(
                            super::state::StylesheetParserInvariantSource::ProspectiveInline {
                                owner,
                                slot,
                            },
                        ));
                        return Err(DocumentPublicationFailure::InvariantViolation);
                    }
                    Err(DocumentCommitFailure::StylesheetInventory(cause)) => {
                        self.fail_document(DocumentLoadFailure::StylesheetInventory(cause));
                        return Err(DocumentPublicationFailure::ResourceUnavailable);
                    }
                }
            }
            CoreEvent::HtmlParseFinished {
                tab_id,
                request_id,
                result,
                decode_report,
                diagnostics,
            } if self.is_current(tab_id, request_id) => match result {
                Err(error) => {
                    if self.document_load.parse_status == DocumentParseStatus::Pending {
                        self.document_load.decode_report = decode_report;
                        self.document_load.diagnostics = diagnostics;
                    }
                    self.fail_document(DocumentLoadFailure::Parser(error));
                }
                Ok(completion) => {
                    if self.document_load.parse_status != DocumentParseStatus::Pending
                        || self.dom_handle != Some(completion.handle)
                        || self.dom_version != completion.version
                        || self.page.document_mode != Some(completion.document_mode)
                        || self.page.dom.is_none()
                    {
                        let failure = DocumentPublicationFailure::InvariantViolation;
                        self.on_document_publication_failure(failure.clone());
                        return Err(failure);
                    }
                    if self.document_load.body_completion.is_none() {
                        self.fail_document(DocumentLoadFailure::ParserCompletionBeforeBody);
                        return Err(DocumentPublicationFailure::InvariantViolation);
                    }
                    if !decode_report.is_some_and(|report| report.eof_processed) {
                        let failure = DocumentPublicationFailure::InvariantViolation;
                        self.on_document_publication_failure(failure.clone());
                        return Err(failure);
                    }
                    if !diagnostics.as_ref().is_some_and(|r| {
                        r.completion() == html::internal::ParserInputCompletion::Finalized
                    }) {
                        let failure = DocumentPublicationFailure::InvariantViolation;
                        self.on_document_publication_failure(failure.clone());
                        return Err(failure);
                    }
                    self.document_load.diagnostics = diagnostics;
                    self.document_load.decode_report = decode_report;
                    self.document_load.parse_status = DocumentParseStatus::Complete;
                    let settlement = self.update_loading();
                    self.update_document_progress(settlement);
                    self.poke_redraw();
                }
            },

            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_start(stylesheet_slot_id, response, request_id);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_chunk(stylesheet_slot_id, url, bytes, request_id);
            }
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Image,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_start(request_id, response);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_chunk(request_id, url, bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
                body,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_done(stylesheet_slot_id, response, body, request_id);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Image,
                response,
                body,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_done(request_id, response, body);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                error_kind,
                response,
                body,
                error: _,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_error(
                    stylesheet_slot_id,
                    url,
                    error_kind,
                    response,
                    body,
                    request_id,
                );
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Image,
                url,
                error_kind,
                response: _,
                body: _,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_error(request_id, url, error_kind, error);
            }

            CoreEvent::CssDecodedBlock {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
                decoded,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_decoded_block(stylesheet_slot_id, &url, decoded, request_id);
            }
            CoreEvent::CssRuntimeFailed {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
                failure,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_runtime_failed(stylesheet_slot_id, &url, failure, request_id);
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_sheet_done(stylesheet_slot_id, &url, request_id);
            }

            _ => {}
        }
        Ok(())
    }
}

impl Tab {
    fn commit_document_publication(
        &mut self,
        publication: DocumentPublication,
        request_id: u64,
    ) -> Result<(), DocumentCommitFailure> {
        self.commit_document_publication_with_identity_resolver(
            publication,
            request_id,
            |store, handle, keys| store.resolve_mutation_node_ids(handle, keys),
        )
    }

    fn commit_document_publication_with_identity_resolver(
        &mut self,
        publication: DocumentPublication,
        request_id: u64,
        resolve_identities: impl Fn(
            &crate::dom_store::DomStore,
            core_types::DomHandle,
            &[html::PatchKey],
        ) -> Result<
            crate::dom_store::ResolvedMutationNodeIds,
            DomIdentityResolutionError,
        >,
    ) -> Result<(), DocumentCommitFailure> {
        let DocumentPublication {
            handle,
            document_mode,
            payload,
        } = publication;
        let mut staged_store = self.dom_store.clone();
        let new_handle = self.dom_handle != Some(handle);
        let mut addresses = self
            .page
            .addresses
            .clone()
            .ok_or(DocumentPublicationFailure::InvariantViolation)?;
        if new_handle {
            addresses.reset_selection();
        }
        let (dom, mutation_facts, staged_version) = match payload {
            DocumentPublicationPayload::Patch { from, to, patches } => {
                if !new_handle && self.page.document_mode != Some(document_mode) {
                    return Err(DocumentPublicationFailure::DocumentModeChanged.into());
                }
                if new_handle {
                    staged_store.clear();
                    staged_store
                        .create(handle)
                        .map_err(|_| DocumentPublicationFailure::InvalidPayload)?;
                }
                let observation = staged_store
                    .apply_recording_base_changes(
                        handle,
                        from,
                        to,
                        &patches,
                        addresses.selected_key(),
                    )
                    .map_err(map_dom_patch_error)?;
                for change in observation.changes {
                    match change {
                        crate::dom_store::BaseElementChange::Reset => addresses.reset_selection(),
                        crate::dom_store::BaseElementChange::Selection(candidate) => addresses
                            .select(candidate)
                            .map_err(|_| DocumentPublicationFailure::InvariantViolation)?,
                    }
                }
                let dom = staged_store
                    .materialize(handle)
                    .map_err(|_| DocumentPublicationFailure::MaterializationFailed)?;
                let pending_facts = PendingDomMutationFacts::from_patches(&patches, new_handle);
                let attribute_targets = resolve_identities(
                    &staged_store,
                    handle,
                    pending_facts.attribute_target_keys(),
                )
                .map_err(map_dom_identity_resolution_error)?;
                let text_targets =
                    resolve_identities(&staged_store, handle, pending_facts.text_target_keys())
                        .map_err(map_dom_identity_resolution_error)?;
                let committed_store = if pending_facts.document_replaced() {
                    None
                } else {
                    Some(&self.dom_store)
                };
                let exact_attribute_mutations = staged_store
                    .capture_exact_attribute_mutations(
                        committed_store,
                        handle,
                        pending_facts.attribute_target_keys(),
                        &DomMutationSnapshotLimits::default(),
                    )
                    .map_err(map_dom_mutation_snapshot_invariant_error)?;
                let exact_text_mutations = staged_store
                    .capture_exact_text_mutations(
                        committed_store,
                        handle,
                        pending_facts.text_target_keys(),
                        &DomMutationSnapshotLimits::default(),
                    )
                    .map_err(map_dom_mutation_snapshot_invariant_error)?;
                let facts = DomMutationFacts::resolve(
                    pending_facts,
                    attribute_targets,
                    text_targets,
                    exact_attribute_mutations,
                    exact_text_mutations,
                );
                (dom, facts, to)
            }
        };

        let prepared_styles = self
            .page
            .prepare_document_stylesheets(
                &dom,
                Some(addresses.active_base_url()),
                mutation_facts.document_replaced(),
                self.document_load
                    .encoding_decision
                    .map(|_| bus::css_decode::CssEncoding::UTF8),
            )
            .map_err(|e| match e {
                crate::document_style::StylesheetPreparationFailure::Inventory(e) => {
                    DocumentCommitFailure::StylesheetInventory(e)
                }
                crate::document_style::StylesheetPreparationFailure::ParserIntegrity {
                    owner,
                    slot,
                } => DocumentCommitFailure::StylesheetParserIntegrity { owner, slot },
            })?;
        self.page.addresses = Some(addresses);
        self.dom_store = staged_store;
        self.dom_handle = Some(handle);
        self.dom_version = staged_version;
        let (render_work, styles) =
            self.page
                .commit_dom_publication(dom, document_mode, mutation_facts, prepared_styles);
        self.retire_stylesheet_requests(styles.retired);
        self.page.update_head_metadata();
        self.page
            .seed_input_values_from_dom(&mut self.document_input.input_values);
        self.page.update_visible_text_cache();
        self.dispatch_stylesheets(request_id, styles.fetches);
        self.request_optional_render_work(styles.render_invalidation);
        self.discover_images(request_id);
        let settlement = self.update_loading();
        self.update_document_progress(settlement);
        self.request_dom_publication_render_work(render_work);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit_document_publication_with_forced_identity_failure_for_tests(
        &mut self,
        publication: DocumentPublication,
        request_id: u64,
        unavailable: html::PatchKey,
    ) -> Result<(), DocumentPublicationFailure> {
        self.commit_document_publication_with_identity_resolver(
            publication,
            request_id,
            |store, handle, keys| {
                if keys.contains(&unavailable) {
                    Err(DomIdentityResolutionError::LiveIdentityUnavailable(
                        unavailable,
                    ))
                } else {
                    store.resolve_mutation_node_ids(handle, keys)
                }
            },
        )
        .map_err(DocumentCommitFailure::public_category)
    }

    fn update_document_progress(
        &mut self,
        settlement: super::load_observation::StaticInputSettlement,
    ) {
        if matches!(
            settlement,
            super::load_observation::StaticInputSettlement::Failed
                | super::load_observation::StaticInputSettlement::EvidenceUnavailable
        ) {
            return;
        }
        let pending = self.page.pending_count();
        let phase = if self.document_load.parse_status == DocumentParseStatus::Complete {
            "Document parsed"
        } else {
            "Parsing document"
        };
        let base = if pending > 0 {
            format!("{phase} • fetching {pending} stylesheet(s)")
        } else {
            phase.to_string()
        };
        self.last_status = Some(match self.document_load.response.as_ref() {
            Some(response) => format!(
                "{base} • {}",
                super::status::response_summary(response, self.document_load.bytes_received)
            ),
            None => base,
        });
    }

    fn on_document_publication_failure(&mut self, failure: DocumentPublicationFailure) {
        self.fail_document(DocumentLoadFailure::Publication(failure));
    }
}

fn map_dom_identity_resolution_error(
    error: DomIdentityResolutionError,
) -> DocumentPublicationFailure {
    match error {
        DomIdentityResolutionError::UnknownHandle(_)
        | DomIdentityResolutionError::NeverAllocated(_)
        | DomIdentityResolutionError::LiveIdentityUnavailable(_) => {
            DocumentPublicationFailure::InvariantViolation
        }
    }
}

fn map_dom_mutation_snapshot_invariant_error(
    error: DomMutationSnapshotInvariantError,
) -> DocumentPublicationFailure {
    match error {
        DomMutationSnapshotInvariantError::UnknownHandle(_)
        | DomMutationSnapshotInvariantError::TargetNeverAllocated(_)
        | DomMutationSnapshotInvariantError::LiveIdentityUnavailable(_)
        | DomMutationSnapshotInvariantError::AttributeTargetNotElement(_)
        | DomMutationSnapshotInvariantError::TextTargetNotText(_)
        | DomMutationSnapshotInvariantError::HistoricalAttributeKindChanged(_)
        | DomMutationSnapshotInvariantError::HistoricalTextKindChanged(_)
        | DomMutationSnapshotInvariantError::ParentMissing(_) => {
            DocumentPublicationFailure::InvariantViolation
        }
    }
}

fn map_dom_patch_error(error: DomPatchError) -> DocumentPublicationFailure {
    match error {
        DomPatchError::VersionMismatch { .. } | DomPatchError::NonMonotonicVersion { .. } => {
            DocumentPublicationFailure::GenerationMismatch
        }
        DomPatchError::UnknownHandle(_) | DomPatchError::DuplicateHandle(_) => {
            DocumentPublicationFailure::InvariantViolation
        }
        DomPatchError::Protocol(_)
        | DomPatchError::InvalidKey(_)
        | DomPatchError::DuplicateKey(_)
        | DomPatchError::MissingKey(_)
        | DomPatchError::WrongNodeKind { .. }
        | DomPatchError::InvalidParent(_)
        | DomPatchError::MoveNotSupported { .. }
        | DomPatchError::IllegalMove { .. }
        | DomPatchError::InvalidSibling { .. }
        | DomPatchError::CycleDetected { .. }
        | DomPatchError::MissingRoot
        | DomPatchError::UnsupportedPatch(_) => DocumentPublicationFailure::InvalidPayload,
    }
}
